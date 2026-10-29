#!/usr/bin/env python3
"""Tests of tools/game/remote.py's search for this Claude Code session's game (#1397): the key of a session's record
as the game makes it, the port found without --port, and a wait's search for a game that records its port late.

Example
  uv run python -I -m unittest discover -s tools/game -p "test_*.py"

Dependencies: the standard library.
"""
import json, os, sys, tempfile, unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import remote  # noqa: E402

SESSION = '57b5a8bd-0dfa-4826-be0a-8d52cf51dcd0'


class Key(unittest.TestCase):
    def test_a_session_keys_its_record_as_the_game_does(self):
        # crates/opensacred-game's remote.rs expects the same key.
        key = remote.record_key(SESSION, r'c:\users\dev\opensacred')
        self.assertEqual(key, f'{SESSION}-6bacac6e8af29895')
        if os.name == 'nt':
            self.assertEqual(remote.record_key(SESSION, 'C:/Users/Dev/OpenSacred'), key)


class GamePort(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.records = self.dir.name

    def tearDown(self):
        self.dir.cleanup()

    def record(self, directory, port):
        path = os.path.join(self.records, remote.record_key(SESSION, directory) + '.port')
        with open(path, 'w', encoding='utf-8', newline='') as f:
            json.dump({'port': port, 'pid': 4242, 'dir': directory}, f)
            f.write('\n')

    def port(self, directory, session=SESSION):
        return remote.game_port(session, directory, self.records)

    def test_outside_a_session_the_port_is_15702(self):
        self.record('w1', 50001)
        self.assertEqual(self.port('w1', session=''), (remote.PORT, None))

    def test_the_game_started_here_is_found(self):
        self.record('w1', 50001)
        self.record('w2', 50002)
        port, found = self.port('w2')
        self.assertEqual(port, 50002)
        self.assertIn('w2', found)

    def test_the_sessions_only_game_is_found_from_elsewhere(self):
        self.record('w1', 50001)
        self.assertEqual(self.port('elsewhere')[0], 50001)

    def test_several_games_started_elsewhere_ask_for_a_port(self):
        self.record('w1', 50001)
        self.record('w2', 50002)
        with self.assertRaises(ConnectionError) as asked:
            self.port('elsewhere')
        self.assertIn('--port', str(asked.exception))

    def test_a_session_with_no_game_falls_back_to_15702(self):
        self.assertEqual(self.port('w1'), (remote.PORT, None))

    def test_another_sessions_games_are_not_its_own(self):
        self.record('w1', 50001)
        self.assertEqual(self.port('w1', session='another-session'), (remote.PORT, None))


class Wait(unittest.TestCase):
    def poll(self, port, ports):
        """The ports that a wait for the menu polls, given `port`, of a game that answers on the ports `ports`."""
        polled = []

        def call(port, method, params=None):
            polled.append(port)
            if port not in ports:
                raise ConnectionError('no game')
            return {'state': 'menu'}

        with mock.patch.object(remote, 'call', call), mock.patch.object(remote.time, 'sleep', lambda _: None):
            snapshot = remote.wait(port, 5, lambda s: s['state'] == 'menu', 'menu')
        self.assertEqual(snapshot, {'state': 'menu'})
        return polled

    def test_a_wait_finds_a_sessions_game_that_records_its_port_late(self):
        # The game records its port after the client has looked for it (#1442): the first polls find no record and
        # take the default port, the next its own.
        found = iter([remote.PORT, remote.PORT, 50001])
        self.assertEqual(self.poll(lambda: next(found), {50001}), [remote.PORT, remote.PORT, 50001])

    def test_a_wait_polls_a_port_given_as_it_is(self):
        self.assertEqual(self.poll(50001, {50001}), [50001])


if __name__ == '__main__':
    unittest.main()
