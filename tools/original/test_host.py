#!/usr/bin/env python3
"""Tests of tools/original/host.py's record of the original's holder (#1398): one original on the machine, a host that
finds a live host's record refuses naming its holder, and a record whose host is gone is taken over.

Example
  uv run python -I -m unittest discover -s tools/original -p "test_*.py"

Dependencies: frida, Pillow (pyproject.toml), as host.py imports them; Windows.
"""
import json, os, subprocess, sys, tempfile, time, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import host  # noqa: E402


def record(pid, holder='opensacred-test'):
    return {'holder': holder, 'session': None, 'branch': 'infra/1398-original-holder', 'worktree': 'C:/w',
            'pid': pid, 'since': time.time()}


class Holder(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.path = os.path.join(self.dir.name, 'OpenSacred', 'original-holder.json')

    def tearDown(self):
        self.dir.cleanup()

    def held(self):
        with open(self.path, encoding='utf-8') as f:
            return json.load(f)

    def test_a_free_original_is_taken(self):
        mine = record(os.getpid())
        host.claim(mine, self.path)
        self.assertEqual(self.held(), mine)

    def test_a_live_holder_is_named_in_the_refusal(self):
        other = subprocess.Popen([sys.executable, '-I', '-c', 'import time; time.sleep(60)'])
        try:
            host.claim(record(other.pid, 'opensacred-e7'), self.path)
            with self.assertRaises(SystemExit) as refused:
                host.claim(record(os.getpid()), self.path)
            self.assertIn('opensacred-e7', str(refused.exception))
            self.assertIn('infra/1398-original-holder', str(refused.exception))
            self.assertEqual(self.held()['pid'], other.pid)
        finally:
            other.kill()
            other.wait()

    def test_a_record_whose_host_is_gone_is_taken_over(self):
        gone = subprocess.Popen([sys.executable, '-I', '-c', 'pass'])
        gone.wait()
        host.claim(record(gone.pid), self.path)
        mine = record(os.getpid())
        host.claim(mine, self.path)
        self.assertEqual(self.held(), mine)

    def test_a_record_being_written_refuses_and_an_old_unreadable_one_is_taken_over(self):
        os.makedirs(os.path.dirname(self.path))
        Path(self.path).write_bytes(b'')
        with self.assertRaises(SystemExit) as refused:
            host.claim(record(os.getpid()), self.path)
        self.assertIn('right now', str(refused.exception))
        old = time.time() - 2 * host.HOLDER_WRITING
        os.utime(self.path, (old, old))
        mine = record(os.getpid())
        host.claim(mine, self.path)
        self.assertEqual(self.held(), mine)

    def test_a_host_removes_its_own_record_alone(self):
        mine = record(os.getpid())
        host.claim(mine, self.path)
        host.release(dict(mine, since=mine['since'] + 1), self.path)
        self.assertTrue(os.path.exists(self.path))
        host.release(mine, self.path)
        self.assertFalse(os.path.exists(self.path))

    def test_the_holder_reads_as_words(self):
        words = host.holding(record(1234))
        self.assertEqual(words, 'opensacred-test (branch infra/1398-original-holder, worktree C:/w, host pid 1234)')
        unnamed = dict(record(1234), holder=None, session='57b5a8bd')
        self.assertTrue(host.holding(unnamed).startswith('the Claude Code session 57b5a8bd ('))


if __name__ == '__main__':
    unittest.main()
