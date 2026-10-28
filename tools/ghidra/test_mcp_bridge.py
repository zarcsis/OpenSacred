#!/usr/bin/env python3
"""Tests of tools/ghidra/mcp_bridge.py on a fake Streamable HTTP server: the messages both ways, its JSON and SSE
answers, the session's header, a restarted server (404) and one that is down.

Example
  uv run python -I -m unittest discover -s tools/ghidra -p "test_*.py"

Dependencies: standard library.
"""
import io, json, sys, threading, unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import mcp_bridge  # noqa: E402

VERSION = '2025-06-18'


class FakeServer:
    """A Streamable HTTP MCP server that answers initialize in JSON and tools/call in SSE, a notification first."""

    def __init__(self):
        self.sessions = set()
        self.next_session = 1
        self.posts = []  # (method or 'response', session header, protocol version header)
        fake = self

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def do_POST(self):
                message = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
                session = self.headers.get('Mcp-Session-Id')
                fake.posts.append((message.get('method', 'response'), session, self.headers.get('MCP-Protocol-Version')))
                if message.get('method') == 'initialize':
                    new = f's{fake.next_session}'
                    fake.next_session += 1
                    fake.sessions.add(new)
                    self.answer(200, 'application/json', json.dumps(
                        {'jsonrpc': '2.0', 'id': message['id'], 'result': {'protocolVersion': VERSION}}),
                        {'Mcp-Session-Id': new})
                elif session not in fake.sessions:
                    self.answer(404, 'application/json', '{}')
                elif 'id' not in message or 'method' not in message:
                    # As the MCP SDK's server: an empty body, typed JSON.
                    self.answer(202, 'application/json', '')
                else:
                    # A notification, then the answer pretty-printed over several data lines, with CRLF ends.
                    progress = json.dumps({'jsonrpc': '2.0', 'method': 'notifications/progress',
                                           'params': {'progress': 1}})
                    result = json.dumps({'jsonrpc': '2.0', 'id': message['id'],
                                         'result': {'echo': message['params']}}, indent=1)
                    body = f'event: message\r\ndata: {progress}\r\n\r\n'
                    body += ''.join(f'data: {line}\r\n' for line in result.split('\n')) + '\r\n'
                    self.answer(200, 'text/event-stream', body)

            def do_DELETE(self):
                fake.sessions.discard(self.headers.get('Mcp-Session-Id'))
                self.answer(200, None, '')

            def answer(self, status, kind, body, headers=()):
                data = body.encode()
                self.send_response(status)
                if kind:
                    self.send_header('Content-Type', kind)
                for name, value in dict(headers).items():
                    self.send_header(name, value)
                self.send_header('Content-Length', str(len(data)))
                self.end_headers()
                self.wfile.write(data)

        self.http = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.url = f'http://127.0.0.1:{self.http.server_address[1]}/mcp'
        threading.Thread(target=self.http.serve_forever, daemon=True).start()

    def restart(self):
        self.sessions.clear()

    def stop(self):
        self.http.shutdown()
        self.http.server_close()


def line(message):
    return json.dumps(message).encode() + b'\n'


INITIALIZE = {'jsonrpc': '2.0', 'id': 0, 'method': 'initialize',
              'params': {'protocolVersion': VERSION, 'capabilities': {}, 'clientInfo': {'name': 'test', 'version': '0'}}}
INITIALIZED = {'jsonrpc': '2.0', 'method': 'notifications/initialized'}


def call(n):
    return {'jsonrpc': '2.0', 'id': n, 'method': 'tools/call', 'params': {'name': 'decompile_function', 'n': n}}


class BridgeTest(unittest.TestCase):
    def setUp(self):
        self.server = FakeServer()
        self.ensured = 0

    def tearDown(self):
        self.server.stop()

    def ensure(self):
        self.ensured += 1

    def run_bridge(self, url, messages):
        output = io.BytesIO()
        bridge = mcp_bridge.Bridge(url, output, self.ensure, drain=10)
        bridge.run(line(m) for m in messages)
        return [json.loads(each) for each in output.getvalue().splitlines()], bridge

    def test_messages_both_ways(self):
        answers, bridge = self.run_bridge(self.server.url, [INITIALIZE, INITIALIZED, call(1)])
        self.assertEqual(answers[0], {'jsonrpc': '2.0', 'id': 0, 'result': {'protocolVersion': VERSION}})
        self.assertEqual(answers[1]['method'], 'notifications/progress')
        self.assertEqual(answers[2], {'jsonrpc': '2.0', 'id': 1, 'result': {'echo': call(1)['params']}})
        self.assertEqual(self.server.posts, [('initialize', None, None), ('notifications/initialized', 's1', VERSION),
                                             ('tools/call', 's1', VERSION)])
        self.assertEqual(self.ensured, 0)
        self.assertEqual(self.server.sessions, set(), 'the session is ended with stdin')

    def test_a_restarted_server_gets_a_new_session(self):
        output = io.BytesIO()
        bridge = mcp_bridge.Bridge(self.server.url, output, self.ensure, drain=10)
        bridge.forward(INITIALIZE)
        bridge.initialize = INITIALIZE
        bridge.forward(INITIALIZED)
        self.server.restart()
        bridge.forward(call(2))
        answers = [json.loads(each) for each in output.getvalue().splitlines()]
        self.assertEqual(answers[-1], {'jsonrpc': '2.0', 'id': 2, 'result': {'echo': call(2)['params']}})
        self.assertEqual(self.ensured, 1)
        self.assertEqual(bridge.session, 's2')
        self.assertEqual([p[0] for p in self.server.posts[2:]],
                         ['tools/call', 'initialize', 'notifications/initialized', 'tools/call'])
        self.assertEqual(len([a for a in answers if a.get('id') == 0]), 1, 'the new initialize is not answered twice')

    def test_a_server_down_answers_the_request_with_an_error(self):
        self.server.stop()

        def ensure():
            self.ensured += 1
            raise RuntimeError('nothing answers')

        output = io.BytesIO()
        mcp_bridge.Bridge(self.server.url, output, ensure, drain=10).forward(call(3))
        answer = json.loads(output.getvalue())
        self.assertEqual(answer['id'], 3)
        self.assertIn('nothing answers', answer['error']['message'])
        self.assertEqual(self.ensured, 1)
        self.server = FakeServer()  # for tearDown

    def test_a_notification_to_a_server_down_is_dropped(self):
        self.server.stop()
        output = io.BytesIO()
        mcp_bridge.Bridge(self.server.url, output, lambda: None, drain=10).forward(INITIALIZED)
        self.assertEqual(output.getvalue(), b'')
        self.server = FakeServer()


class RepositoryTest(unittest.TestCase):
    def test_inside_repository(self):
        self.assertTrue(mcp_bridge.inside_repository(mcp_bridge.REPOSITORY))
        self.assertTrue(mcp_bridge.inside_repository(mcp_bridge.REPOSITORY / '.claude' / 'worktrees' / 'x'))
        self.assertFalse(mcp_bridge.inside_repository(mcp_bridge.REPOSITORY.parent))


if __name__ == '__main__':
    unittest.main()
