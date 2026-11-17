#!/usr/bin/env python3
"""Tests of tools/usage/pr_usage.py on synthetic transcripts: the branch each response of a session counts toward.

Example
  uv run python -I -m unittest discover -s tools/usage -p "test_*.py"

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


def usage(inp=0, out=0, read=0, write_5m=0, write_1h=0):
    """A response's usage as the transcripts record it."""
    return {'input_tokens': inp, 'output_tokens': out, 'cache_read_input_tokens': read,
            'cache_creation_input_tokens': write_5m + write_1h,
            'cache_creation': {'ephemeral_5m_input_tokens': write_5m, 'ephemeral_1h_input_tokens': write_1h}}


class Cost(unittest.TestCase):
    MILLION = usage(inp=1_000_000, out=1_000_000, read=1_000_000, write_5m=1_000_000, write_1h=1_000_000)

    def test_a_million_tokens_of_each_kind_cost_the_models_prices(self):
        for model, dollars in {'claude-opus-5-5': 4 + 20 + 0.20 + 5 + 8,
                               'claude-sonnet-5-5': 2 + 10 + 0.10 + 2.50 + 4,
                               'claude-fable-5-1': 10 + 50 + 0.25 + 12.50 + 20,
                               'claude-fable-5-2': 8 + 40 + 0.20 + 10 + 16,
                               'claude-mythos-5-2': 8 + 40 + 0.20 + 10 + 16}.items():
            with self.subTest(model=model):
                self.assertAlmostEqual(pr_usage.cost(model, self.MILLION), dollars)

    def test_each_kind_of_token_has_its_own_price(self):
        for kind, dollars in {'inp': 8, 'out': 40, 'read': 0.20, 'write_5m': 10, 'write_1h': 16}.items():
            with self.subTest(kind=kind):
                self.assertAlmostEqual(pr_usage.cost('claude-mythos-5-2', usage(**{kind: 1_000_000})), dollars)

    def test_a_cache_write_without_its_kinds_is_one_of_five_minutes(self):
        self.assertAlmostEqual(pr_usage.cost('claude-opus-5-5', {'cache_creation_input_tokens': 1_000_000}), 5)

    def test_a_model_priced_by_the_prompts_length_pays_more_above_it(self):
        # The prompt is the input with the cache's reads and writes; the output is not of it.
        short = usage(inp=10_000, out=500_000, read=60_000, write_5m=20_000, write_1h=10_000)
        self.assertAlmostEqual(pr_usage.cost('claude-haiku-5-5', short),
                               (10_000 * 0.10 + 500_000 * 0.50 + 60_000 * 0.01 + 20_000 * 0.125 + 10_000 * 0.20) / 1e6)
        long = dict(short, input_tokens=10_001)
        self.assertAlmostEqual(pr_usage.cost('claude-haiku-5-5', long),
                               (10_001 * 0.50 + 500_000 * 2.50 + 60_000 * 0.05 + 20_000 * 0.625 + 10_000 * 1.0) / 1e6)

    def test_another_model_keeps_its_prices_above_that_length(self):
        long = usage(inp=2, out=1_000, read=900_000, write_1h=5_000)
        self.assertAlmostEqual(pr_usage.cost('claude-mythos-5-2', long),
                               (2 * 8 + 1_000 * 40 + 900_000 * 0.20 + 5_000 * 16) / 1e6)

    def test_a_model_without_a_price_costs_nothing(self):
        self.assertEqual(pr_usage.cost('claude-unknown-9', self.MILLION), 0.0)

    def test_every_model_with_a_price_has_a_name_and_five_prices(self):
        for model, prices in list(pr_usage.PRICES.items()) + [(m, p) for m, (_, p) in pr_usage.LONG_PROMPT.items()]:
            with self.subTest(model=model):
                self.assertIn(model, pr_usage.NAMES)
                self.assertIn(model, pr_usage.PRICES)
                self.assertEqual(len(prices), 5)


if __name__ == '__main__':
    unittest.main()
