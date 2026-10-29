#!/usr/bin/env python3
"""Tests of .claude/hooks/rules_watch.py on temporary repositories: a session that started on `main` learns of a rule
merged later, once, and not of its own branch's changes.

Example
  uv run python -I -m unittest discover -s .claude/hooks -p "test_*.py"

Dependencies: standard library, `git` 2.40+.
"""
import subprocess, sys, tempfile, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import rules_watch  # noqa: E402


def git(cwd, *args):
    subprocess.run(['git', '-C', str(cwd), *args], check=True, capture_output=True)


class Repos:
    """A bare `origin`, a clone that pushes to it as the other sessions merge, and the session's own clone."""

    def __init__(self, root):
        self.origin, self.upstream, self.session = root / 'origin.git', root / 'upstream', root / 'session'
        git(root, 'init', '--quiet', '--bare', '--initial-branch=main', str(self.origin))
        git(root, 'clone', '--quiet', str(self.origin), str(self.upstream))
        self.configure(self.upstream)
        self.commit({'CLAUDE.md': '# Rules\n\n- One rule.\n', 'docs/03-toolchain.md': '### 3.5\n\n* Worktrees.\n'},
                    'start')
        git(root, 'clone', '--quiet', str(self.origin), str(self.session))
        self.configure(self.session)

    @staticmethod
    def configure(repo):
        for key, value in (('user.name', 'Test'), ('user.email', 'test@example.com'), ('core.autocrlf', 'false')):
            git(repo, 'config', key, value)

    def commit(self, files, message, repo=None, push=True):
        repo = repo or self.upstream
        for name, text in files.items():
            path = repo / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(text.encode())
            git(repo, 'add', name)
        git(repo, 'commit', '--quiet', '-m', message)
        if push:
            git(repo, 'push', '--quiet', 'origin', 'HEAD:main')


class Watch(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        root = Path(self.dir.name)
        self.repos = Repos(root)
        self.state = str(root / 'state')
        self.event = {'session_id': 'session-1', 'cwd': str(self.repos.session)}
        self.clock = 1000.0

    def tearDown(self):
        self.dir.cleanup()

    def now(self):
        return self.clock

    def start(self):
        rules_watch.start(self.event, self.state, self.now)

    def prompt(self, fetch_every=0):
        self.clock += 1
        return rules_watch.prompt(self.event, self.state, self.now, fetch_every)

    def test_nothing_changed_adds_nothing(self):
        self.start()
        self.assertIsNone(self.prompt())

    def test_a_rule_merged_later_is_told_once(self):
        self.start()
        self.repos.commit({'CLAUDE.md': '# Rules\n\n- One rule.\n- A new rule.\n'}, 'a rule')
        told = self.prompt()
        self.assertIn('+- A new rule.', told)
        self.assertIn('CLAUDE.md (origin/main)', told)
        self.assertIsNone(self.prompt())

    def test_a_fetch_waits_its_interval(self):
        self.start()
        self.repos.commit({'CLAUDE.md': '# Rules\n\n- Two.\n'}, 'a rule')
        self.assertIsNone(self.prompt(fetch_every=3600))
        self.assertIn('+- Two.', self.prompt())

    def test_the_sessions_own_branch_is_not_main(self):
        git(self.repos.session, 'switch', '--quiet', '-c', 'docs/1-rules')
        self.repos.commit({'CLAUDE.md': '# Rules\n\n- My own rule.\n'}, 'mine', repo=self.repos.session, push=False)
        self.start()
        self.assertIsNone(self.prompt())

    def test_a_long_change_is_named_by_its_command(self):
        self.start()
        long = '### 3.5\n\n' + ''.join(f'* Line {n} of a long change to the process.\n' for n in range(400))
        self.repos.commit({'docs/03-toolchain.md': long}, 'a long change')
        told = self.prompt()
        self.assertIn('docs/03-toolchain.md: 401 lines changed', told)
        self.assertIn('git diff ', told)
        self.assertNotIn('```diff', told)

    def test_a_session_from_before_the_hook_starts_from_main(self):
        self.repos.commit({'CLAUDE.md': '# Rules\n\n- Three.\n'}, 'a rule')
        self.assertIsNone(self.prompt())
        self.repos.commit({'CLAUDE.md': '# Rules\n\n- Four.\n'}, 'another')
        self.assertIn('+- Four.', self.prompt())

    def test_a_directory_outside_a_repository_adds_nothing(self):
        self.event['cwd'] = self.dir.name
        self.start()
        self.assertIsNone(self.prompt())


if __name__ == '__main__':
    unittest.main()
