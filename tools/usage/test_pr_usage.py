#!/usr/bin/env python3
"""Tests of tools/usage/pr_usage.py on synthetic transcripts: the branch each response of a session counts toward.

Example
  python -I -m unittest discover -s tools/usage -p "test_*.py"

Dependencies: standard library.
"""
import datetime as dt, json, os, sys, tempfile, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pr_usage  # noqa: E402

TOP = pr_usage.norm(os.path.join(tempfile.gettempdir(), 'repository'))
WORKTREE = os.path.join(TOP, '.claude', 'worktrees', 'x')


class Session:
    """A session's transcript: responses that make one tool call each, and their results. Every line records the
    session's working directory and the branch Claude Code records, which the test sets as the session goes."""

    def __init__(self, cwd=TOP, branch='main'):
        self.cwd, self.branch, self.lines = cwd, branch, []
        self.time = dt.datetime(2026, 10, 18, 12, tzinfo=dt.timezone.utc)

    def line(self, kind, content, **message):
        self.time += dt.timedelta(seconds=5)
        self.lines.append({'type': kind, 'uuid': 'line-%d' % len(self.lines), 'cwd': self.cwd,
                           'gitBranch': self.branch, 'timestamp': self.time.strftime('%Y-%m-%dT%H:%M:%S.000Z'),
                           'message': dict(message, content=content)})

    def call(self, tool, result='', **inputs):
        """A response: a line of text, which carries the usage, then the tool call; then the call's result."""
        n = len(self.lines)
        response = {'role': 'assistant', 'id': 'response-%d' % n, 'model': 'claude-opus-5-5',
                    'usage': {'input_tokens': 1, 'output_tokens': 1}}
        self.line('assistant', [{'type': 'text', 'text': 'text'}], **response)
        self.line('assistant', [{'type': 'tool_use', 'id': 'call-%d' % n, 'name': tool, 'input': inputs}], **response)
        self.line('user', [{'type': 'tool_result', 'tool_use_id': 'call-%d' % n, 'content': result}], role='user')

    def bash(self, command):
        self.call('Bash', command=command)

    def enter(self, worktree, branch):
        self.call('EnterWorktree', 'Entered worktree at %s on branch %s. The session works there.' % (worktree, branch),
                  path=worktree)
        self.cwd, self.branch = worktree, branch

    def counted(self, existing=()):
        """The branch each response counts toward, in order."""
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, 'session.jsonl')
            with open(path, 'w', encoding='utf-8', newline='') as f:
                f.writelines(json.dumps(line) + '\n' for line in self.lines)
            events = pr_usage.session_events(path, set(), TOP, {}, set(existing))
        return [o for e, o in zip(events, pr_usage.owners(events)) if e[2] == 'assistant' and e[4]]


class SwitchInWorktree(unittest.TestCase):
    def test_a_switch_in_an_entered_worktree_gives_it_the_branch_it_moves_to(self):
        s = Session()
        s.bash('git worktree add .claude/worktrees/x -b bug/1-a origin/main')
        s.enter(WORKTREE, 'bug/1-a')
        s.bash('cargo build')
        s.bash('git fetch -q && git switch -q -c bug/2-b origin/main && git log --oneline -1')
        s.branch = 'bug/2-b'
        s.bash('cargo test')
        self.assertEqual(s.counted(), ['bug/1-a'] * 3 + ['bug/2-b'] * 2)

    def test_a_switch_after_a_cd_into_a_worktree_gives_it_the_branch_it_moves_to(self):
        s = Session()  # the session keeps recording its own checkout's branch, main
        s.bash('git worktree add .claude/worktrees/x -b bug/1-a origin/main')
        s.bash('cd .claude/worktrees/x')
        s.cwd = WORKTREE
        s.bash('cargo build')
        s.bash('git switch -c bug/2-b origin/main')
        s.bash('cargo test')
        s.bash('git checkout -b bug/3-c origin/main 2>&1 | tail -1')
        s.bash('cargo test')
        self.assertEqual(s.counted(), ['bug/1-a'] * 3 + ['bug/2-b'] * 2 + ['bug/3-c'] * 2)

    def test_a_switch_from_the_main_checkout_with_git_C_or_a_cd_gives_the_worktree_its_branch(self):
        s = Session()
        s.bash('git worktree add .claude/worktrees/x -b bug/1-a origin/main')
        s.bash('git -C .claude/worktrees/x switch -c bug/2-b origin/main')
        s.bash('cd .claude/worktrees/x && cargo test')
        s.bash('cd .claude/worktrees/x && git switch -c bug/3-c origin/main && cargo test')
        s.bash('cd .claude/worktrees/x; cargo test')
        self.assertEqual(s.counted(), ['bug/1-a', 'bug/2-b', 'bug/2-b', 'bug/3-c', 'bug/3-c'])

    def test_a_checkout_of_paths_keeps_the_branch_and_one_of_a_branch_moves(self):
        s = Session()
        s.bash('git worktree add .claude/worktrees/x -b bug/1-a origin/main')
        s.enter(WORKTREE, 'bug/1-a')
        s.bash('git checkout -- src/a.rs')
        s.bash('git checkout HEAD~1 -- src/b.rs')
        s.bash('git checkout src/c.rs')  # no branch of that name
        s.bash('git checkout bug/2-b')
        s.branch = 'bug/2-b'
        s.bash('cargo test')
        self.assertEqual(s.counted(existing={'main', 'bug/2-b'}), ['bug/1-a'] * 5 + ['bug/2-b'] * 2)

    def test_a_switch_in_the_main_checkout_leaves_the_worktree_its_branch(self):
        s = Session()
        s.bash('git worktree add .claude/worktrees/x -b bug/1-a origin/main')
        s.bash('git switch -c bug/9-z origin/main')
        s.branch = 'bug/9-z'
        s.bash('cargo build')
        s.bash('git -C .claude/worktrees/x log --oneline -1')
        s.bash('cargo test')
        self.assertEqual(s.counted(), ['bug/1-a', 'bug/9-z', 'bug/9-z', 'bug/1-a', 'bug/9-z'])


class BranchMoves(unittest.TestCase):
    def test_the_branch_a_command_moves_its_checkout_to(self):
        here = pr_usage.norm('/repository')
        there = pr_usage.norm('/repository/wt')
        cases = {
            'git switch -c bug/2-b origin/main': [(here, 'bug/2-b', True)],
            'git fetch -q && git switch -q -C b origin/b && git log -1': [(here, 'b', True)],
            'git switch --create=b': [(here, 'b', True)],
            'git switch b 2>&1 | tail -1': [(here, 'b', True)],
            'git switch -': [],
            'git switch --detach origin/main': [],
            'git switch -d origin/main': [],
            'git checkout -b b origin/main': [(here, 'b', True)],
            'git checkout -B b': [(here, 'b', True)],
            'git checkout b': [(here, 'b', False)],
            'git checkout -- a.rs': [],
            'git checkout HEAD -- a.rs': [],
            'git checkout a.rs b.rs': [],
            'git -C wt switch b': [(there, 'b', True)],
            'cd wt && git switch b': [(there, 'b', True)],
            'Set-Location wt; git switch b': [(there, 'b', True)],
            'cd /elsewhere && git -C /repository/wt switch b': [(there, 'b', True)],
            'cd "$X" && git switch b': [],
            'git log --grep="git switch -c x"': [],
        }
        for command, moves in cases.items():
            with self.subTest(command=command):
                self.assertEqual(pr_usage.branch_moves(command, here), moves)


if __name__ == '__main__':
    unittest.main()
