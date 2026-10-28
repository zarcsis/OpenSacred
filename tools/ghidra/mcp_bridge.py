#!/usr/bin/env python3
"""The ghidra MCP server of a Claude Code session, at hand whether the shared pyghidra-mcp runs or not (docs/03 §3.2).

Claude Code starts this script with every session as a stdio MCP server. When nothing answers on the server's port, it
starts pyghidra-mcp hidden, through a launcher that exits at once, so that the server is no descendant of the session's
process tree, which Claude Code ends with the session: it outlives the session and serves every later one. Then it
passes the session's messages to the server over Streamable HTTP and the server's answers back. A server that dies or
restarts during a session is started again and given a new MCP session, so that the next tool call goes through.

The server is started only for a session inside this repository or one of its worktrees; elsewhere the script only
connects to a server that runs. A file `pyghidra-mcp.paused` in the project's directory keeps it from starting the
server, for the headless scripts of tools/ghidra/, which need the project unlocked.

Example
  claude mcp add --scope user ghidra -- uv run --no-project python -I <checkout>/tools/ghidra/mcp_bridge.py
  uv run --no-project python -I tools/ghidra/mcp_bridge.py --start   # start the server if it is down, wait, exit
  uv run python -I -m unittest discover -s tools/ghidra -p "test_*.py"

Dependencies: standard library; the server needs uv (uvx), Ghidra and GHIDRA_INSTALL_DIR (docs/03 §3.2).
"""
import argparse, http.client, json, os, socket, subprocess, sys, threading, time
from pathlib import Path
from urllib.parse import urlsplit

URL = 'http://127.0.0.1:8000/mcp'
# The server of docs/03 §3.2: pyghidra-mcp on uv's Python 3.13 (onnxruntime has no wheels for 3.15).
SERVER = ['uvx', '--python', '3.13', 'pyghidra-mcp@0.2.7', '--transport', 'streamable-http']
PROJECT_NAME = 'OpenSacred'
REPOSITORY = Path(__file__).resolve().parents[2]

# Windows' process creation flags: a console of its own that shows no window, a process group of its own (no Ctrl+C
# of the session's console), and out of the job of the process that starts it.
CREATE_NEW_PROCESS_GROUP = 0x00000200
CREATE_BREAKAWAY_FROM_JOB = 0x01000000
CREATE_NO_WINDOW = 0x08000000


def log(text):
    """A line on stderr, which Claude Code keeps in its MCP log; stdout carries the protocol alone."""
    print(f'ghidra bridge: {text}', file=sys.stderr, flush=True)


def default_project_dir():
    return Path(os.environ.get('USERPROFILE') or Path.home()) / 'GhidraProjects'


def server_up(host, port):
    try:
        with socket.create_connection((host, port), timeout=1):
            return True
    except OSError:
        return False


def inside_repository(path):
    """Whether `path` is the repository or lies in it (its worktrees under .claude/worktrees/ included)."""
    try:
        Path(path).resolve().relative_to(REPOSITORY)
        return True
    except ValueError:
        return False


def ghidra_install_dir():
    """GHIDRA_INSTALL_DIR from the environment, else from the user's (a process started before it was set lacks it)."""
    value = os.environ.get('GHIDRA_INSTALL_DIR')
    if value or sys.platform != 'win32':
        return value
    import winreg
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, 'Environment') as key:
            return winreg.QueryValueEx(key, 'GHIDRA_INSTALL_DIR')[0]
    except OSError:
        return None


def alive(pid):
    """Whether the process `pid` still runs."""
    if sys.platform != 'win32':
        try:
            os.kill(pid, 0)
            return True
        except ProcessLookupError:
            return False
        except PermissionError:
            return True
    # os.kill would end it on Windows; a handle that can wait on it tells instead.
    import ctypes
    kernel32 = ctypes.windll.kernel32
    handle = kernel32.OpenProcess(0x00100000 | 0x1000, False, pid)  # SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION
    if not handle:
        return False
    try:
        return kernel32.WaitForSingleObject(handle, 0) == 0x102  # WAIT_TIMEOUT: not signalled, still running
    finally:
        kernel32.CloseHandle(handle)


def launch(args):
    """The launcher: starts `args` in the working directory with its output in the logs there, prints its PID and
    returns, so that the process it started has no running parent."""
    common = dict(stdin=subprocess.DEVNULL, stdout=open('pyghidra-mcp.out.log', 'wb'),
                  stderr=open('pyghidra-mcp.err.log', 'wb'))
    if sys.platform != 'win32':
        process = subprocess.Popen(args, start_new_session=True, **common)
    else:
        flags = CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
        try:
            process = subprocess.Popen(args, creationflags=flags | CREATE_BREAKAWAY_FROM_JOB, **common)
        except OSError:
            # A job that allows no breakaway: the server may then end with the job's owner.
            process = subprocess.Popen(args, creationflags=flags, **common)
    print(process.pid)


class StartLock:
    """A lock file in the project's directory, so that two sessions starting at once start one server."""

    def __init__(self, path, timeout):
        self.path, self.timeout = path, timeout

    def __enter__(self):
        self.file = open(self.path, 'a+b')
        deadline = time.monotonic() + self.timeout
        while True:
            try:
                self._locking(lock=True)
                return self
            except OSError:
                if time.monotonic() > deadline:
                    self.file.close()
                    raise TimeoutError(f'{self.path} stays locked')
                time.sleep(0.5)

    def __exit__(self, *_):
        self._locking(lock=False)
        self.file.close()

    def _locking(self, lock):
        self.file.seek(0)
        if sys.platform == 'win32':
            import msvcrt
            msvcrt.locking(self.file.fileno(), msvcrt.LK_NBLCK if lock else msvcrt.LK_UNLCK, 1)
        else:
            import fcntl
            fcntl.flock(self.file, fcntl.LOCK_EX | fcntl.LOCK_NB if lock else fcntl.LOCK_UN)


class Server:
    """The shared pyghidra-mcp: whether it answers, and its start."""

    def __init__(self, url, project_dir, may_start, timeout):
        parts = urlsplit(url)
        self.host, self.port = parts.hostname, parts.port or 80
        self.project_dir, self.may_start, self.timeout = Path(project_dir), may_start, timeout

    def ensure(self):
        """Returns once the server answers; starts it when it is down and this session may."""
        if server_up(self.host, self.port):
            return
        if not self.may_start:
            raise RuntimeError(f'nothing answers on {self.host}:{self.port}, and a session outside '
                               f'{REPOSITORY} does not start pyghidra-mcp')
        paused = self.project_dir / 'pyghidra-mcp.paused'
        if paused.exists():
            raise RuntimeError(f'nothing answers on {self.host}:{self.port}, and {paused} keeps it from starting')
        with StartLock(self.project_dir / 'pyghidra-mcp.start.lock', self.timeout):
            if server_up(self.host, self.port):
                return
            started = time.monotonic()
            pid = self._start()
            while not server_up(self.host, self.port):
                if not alive(pid):
                    raise RuntimeError(f'pyghidra-mcp exited; see {self.project_dir / "pyghidra-mcp.err.log"}')
                if time.monotonic() - started > self.timeout:
                    raise TimeoutError(f'pyghidra-mcp does not answer after {self.timeout:.0f} s')
                time.sleep(0.5)
            log(f'pyghidra-mcp ({pid}) answers after {time.monotonic() - started:.0f} s')

    def _start(self):
        install = ghidra_install_dir()
        if not install:
            raise RuntimeError('GHIDRA_INSTALL_DIR is not set (docs/03 §3.2)')
        args = SERVER + ['--project-path', str(self.project_dir), '--project-name', PROJECT_NAME]
        log(f'starting {" ".join(args)}')
        # The project's directory as the server's working directory: a process pins its own (docs/03 §3.2).
        launcher = subprocess.run(
            [sys.executable, '-I', str(Path(__file__).resolve()), '--launch', *args],
            cwd=self.project_dir, env=dict(os.environ, GHIDRA_INSTALL_DIR=install), stdin=subprocess.DEVNULL,
            capture_output=True, text=True, timeout=60,
            creationflags=CREATE_NO_WINDOW if sys.platform == 'win32' else 0)
        if launcher.returncode:
            raise RuntimeError(f'the launcher failed: {launcher.stderr.strip()[-300:]}')
        return int(launcher.stdout)


class SessionGone(Exception):
    """The server does not know the MCP session (404): it was restarted."""


class Bridge:
    """Passes a client's messages from stdin to the server, and the server's answers to stdout."""

    def __init__(self, url, output, ensure, drain=0.2):
        parts = urlsplit(url)
        self.host, self.port, self.path = parts.hostname, parts.port or 80, parts.path or '/'
        self.output, self.ensure = output, ensure
        # Claude Code ends the process tree 300 ms after it closes stdin, so the requests still running get this long.
        self.drain = drain
        self.write_lock = threading.Lock()
        self.session_lock = threading.Lock()
        self.session = None     # Mcp-Session-Id
        self.version = None     # the protocol version that initialize agreed
        self.initialize = None  # the client's own, to open a new session on a restarted server

    def run(self, lines):
        threads = []
        for line in lines:
            line = line.strip()
            if not line:
                continue
            try:
                message = json.loads(line)
            except ValueError:
                message = None
            if not isinstance(message, dict):
                log(f'not a JSON-RPC message: {line[:200]!r}')
                continue
            if message.get('method') == 'initialize':
                # At once: the client waits for its answer before anything else.
                self.initialize = message
                self.forward(message)
            else:
                thread = threading.Thread(target=self.forward, args=(message,), daemon=True)
                thread.start()
                threads.append(thread)
        deadline = time.monotonic() + self.drain
        for thread in threads:
            thread.join(timeout=max(0, deadline - time.monotonic()))
        self.close()

    def forward(self, message):
        """Posts a message; a server down or restarted is brought back and the message posted again, once."""
        try:
            try:
                self.post(message, self.write)
            except (ConnectionError, SessionGone) as e:
                log(f'{message.get("method", "a response")}: {e}; bringing the server back')
                used = self.session
                self.ensure()
                if message.get('method') != 'initialize':
                    self.reopen(used)
                self.post(message, self.write)
        except Exception as e:  # noqa: BLE001 - every request gets an answer, or the client waits for ever
            log(f'{message.get("method", "a response")}: {e}')
            if 'id' in message and 'method' in message:
                self.write({'jsonrpc': '2.0', 'id': message['id'],
                            'error': {'code': -32603, 'message': f'ghidra bridge: {e}'}})

    def reopen(self, used):
        """A new MCP session with the client's own initialize, unless another request opened one already."""
        with self.session_lock:
            if self.session != used or self.initialize is None:
                return
            self.session = None
            self.post(self.initialize, lambda _: None)
            self.post({'jsonrpc': '2.0', 'method': 'notifications/initialized'}, lambda _: None)

    def post(self, message, sink):
        headers = {'Content-Type': 'application/json', 'Accept': 'application/json, text/event-stream'}
        if self.session:
            headers['Mcp-Session-Id'] = self.session
        if self.version:
            headers['MCP-Protocol-Version'] = self.version
        connection = http.client.HTTPConnection(self.host, self.port)
        try:
            connection.request('POST', self.path, json.dumps(message).encode(), headers)
            response = connection.getresponse()
            if response.status == 404 and self.session:
                raise SessionGone(f'the server does not know the session {self.session}')
            if response.status >= 400:
                raise RuntimeError(f'HTTP {response.status}: {response.read()[:300].decode(errors="replace")}')
            if message.get('method') == 'initialize':
                self.session = response.getheader('Mcp-Session-Id')
            kind = (response.getheader('Content-Type') or '').split(';')[0].strip()
            if kind == 'text/event-stream':
                for data in events(response):
                    self.take(json.loads(data), message, sink)
                return
            # A notification's or a response's 202 comes with an empty body, of whatever type.
            body = response.read()
            if kind == 'application/json' and body.strip():
                body = json.loads(body)
                for answer in body if isinstance(body, list) else [body]:
                    self.take(answer, message, sink)
        finally:
            connection.close()

    def take(self, answer, message, sink):
        if message.get('method') == 'initialize' and answer.get('id') == message.get('id'):
            self.version = (answer.get('result') or {}).get('protocolVersion') or self.version
        sink(answer)

    def write(self, message):
        data = json.dumps(message, ensure_ascii=False, separators=(',', ':')).encode() + b'\n'
        with self.write_lock:
            self.output.write(data)
            self.output.flush()

    def close(self):
        """Ends the MCP session on the server, if it answers at once."""
        if not self.session:
            return
        connection = http.client.HTTPConnection(self.host, self.port, timeout=0.1)
        try:
            connection.request('DELETE', self.path, headers={'Mcp-Session-Id': self.session})
            connection.getresponse().read()
        except OSError:
            pass
        finally:
            connection.close()


def events(response):
    """The data of each server-sent event of a response, until the server ends it."""
    data = []
    while line := response.readline():
        line = line.decode().rstrip('\r\n')
        if not line:
            if data:
                yield '\n'.join(data)
                data = []
        elif line.startswith('data:'):
            data.append(line[5:].removeprefix(' '))
    if data:
        yield '\n'.join(data)


def main():
    if sys.argv[1:2] == ['--launch']:
        launch(sys.argv[2:])
        return
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('--url', default=URL, help=f'the server (default {URL})')
    parser.add_argument('--project-dir', type=Path, default=default_project_dir(),
                        help='the Ghidra project\'s directory (default %%USERPROFILE%%\\GhidraProjects)')
    parser.add_argument('--timeout', type=float, default=120, help='seconds to wait for a starting server')
    parser.add_argument('--start', action='store_true', help='start the server if it is down, wait for it, exit')
    args = parser.parse_args()
    project = os.environ.get('CLAUDE_PROJECT_DIR') or Path.cwd()
    server = Server(args.url, args.project_dir, args.start or inside_repository(project), args.timeout)
    if args.start:
        server.ensure()
        log(f'{args.url} answers')
        return
    try:
        server.ensure()
    except Exception as e:  # noqa: BLE001 - the session goes on, and its first tool call tries again
        log(str(e))
    Bridge(args.url, sys.stdout.buffer, server.ensure).run(iter(sys.stdin.buffer.readline, b''))


if __name__ == '__main__':
    main()
