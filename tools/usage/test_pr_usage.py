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

    def write(self, path):
        """The session's transcript as a file."""
        with open(path, 'w', encoding='utf-8', newline='') as f:
            f.writelines(json.dumps(line) + '\n' for line in self.lines)

    def counted(self, existing=(), linked=None):
        """The branch each response counts toward, in order; linked holds the worktrees that exist now."""
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, 'session.jsonl')
            self.write(path)
            events = pr_usage.session_events(path, set(), TOP, linked or {}, set(existing))
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
            # A name that the command does not tell: the checkout moves, and where to is not known.
            'git switch -c "bug/$(gh issue create --title t)-x" origin/main': [(here, None, True)],
            'git checkout -b bug/$N-x': [(here, None, True)],
            'git switch $B': [(here, None, True)],
            'git checkout $B': [],
        }
        for command, moves in cases.items():
            with self.subTest(command=command):
                self.assertEqual(pr_usage.branch_moves(command, here), moves)

    def test_a_variable_is_read_where_the_command_writes_its_value_out(self):
        here = pr_usage.norm('/repository')
        cases = {
            'N=7; git switch -c bug/$N-x': 'bug/7-x',
            'N="7" && git switch -c "bug/${N}-x"': 'bug/7-x',
            "$n = 7; git switch -c bug/$n-x": 'bug/7-x',
            "$b = 'bug/7-x'; git switch -c $b": 'bug/7-x',
            'B=bug/7; C="$B-x"; git switch -c $C': 'bug/7-x',
            # A command's output, which the transcript does not tell.
            'N=$(gh issue create --title t); git switch -c bug/$N-x': None,
            'N="$(gh issue create --title t)"; git switch -c bug/$N-x': None,
            'N=`gh issue create --title t`; git switch -c bug/$N-x': None,
            '$n = (gh issue create --title t); git switch -c "bug/$n-x"': None,
            '$n = gh issue create --title t; git switch -c "bug/$n-x"': None,
            "$u = gh issue create --title t; $n = $u.Split('/')[-1]; git switch -c \"bug/$n-x\"": None,
            'N=7; N=$(gh issue create --title t); git switch -c bug/$N-x': None,
        }
        for command, branch in cases.items():
            with self.subTest(command=command):
                self.assertEqual(pr_usage.branch_moves(pr_usage.expand(command), here), [(here, branch, True)])

    def test_a_worktree_added_under_a_name_the_command_does_not_tell_has_no_branch_known(self):
        here, there = pr_usage.norm('/repository'), pr_usage.norm('/repository/wt')
        self.assertEqual(pr_usage.worktree_adds('git worktree add wt -b bug/7-x origin/main', here),
                         [(there, 'bug/7-x')])
        self.assertEqual(pr_usage.worktree_adds('git worktree add wt -b "bug/$(gh issue create)-x"', here),
                         [(there, None)])
        self.assertEqual(pr_usage.branch_renames('git branch -m bug/$N-x', here), [])


class RemovedWorktree(unittest.TestCase):
    """A worktree that `claim.py` made, which the session works in by paths from the main checkout: its
    transcript names no branch for it."""

    def session(self):
        s = Session()
        s.call('Read', file_path=os.path.join(WORKTREE, 'src', 'a.rs'))
        s.bash('cd .claude/worktrees/x && cargo test')
        return s

    def test_while_it_exists_its_messages_count_toward_the_branch_it_is_on(self):
        self.assertEqual(self.session().counted(existing={'bug/x'}, linked={WORKTREE: 'bug/2-b'}), ['bug/2-b'] * 2)

    def test_once_removed_they_count_toward_the_branch_it_is_named_after(self):
        self.assertEqual(self.session().counted(existing={'main', 'bug/x', 'bug/y'}), ['bug/x'] * 2)

    def test_a_name_that_two_branches_end_in_is_neithers(self):
        self.assertEqual(self.session().counted(existing={'bug/x', 'feature/x'}), ['worktree ' + WORKTREE] * 2)

    def test_without_a_branch_named_so_they_count_toward_no_pull_request(self):
        self.assertEqual(self.session().counted(existing={'main', 'bug/y', 'x'}), ['worktree ' + WORKTREE] * 2)

    def test_the_branch_a_session_records_in_it_comes_first(self):
        s = Session(cwd=WORKTREE, branch='bug/9-other')
        s.bash('cargo test')
        self.assertEqual(s.counted(existing={'bug/x'}), ['bug/9-other'])

    def test_a_switch_to_a_name_the_command_does_not_tell_leaves_it_to_what_is_known_now(self):
        s = Session()
        s.bash('git worktree add .claude/worktrees/x -b bug/1-a origin/main')
        s.bash('cd .claude/worktrees/x && cargo build')
        s.bash('cd .claude/worktrees/x && N=$(gh issue create --title t) && git switch -c "bug/$N-b" origin/main')
        s.bash('cd .claude/worktrees/x && cargo test')
        self.assertEqual(s.counted(linked={WORKTREE: 'bug/2-b'}), ['bug/1-a'] * 2 + ['bug/2-b'] * 2)
        self.assertEqual(s.counted(existing={'feature/x'}), ['bug/1-a'] * 2 + ['feature/x'] * 2)
        self.assertEqual(s.counted(), ['bug/1-a'] * 2 + ['worktree ' + WORKTREE] * 2)


class Count(unittest.TestCase):
    """Several pull requests counted in one pass over the transcripts of a repository."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.projects, pr_usage.PROJECTS = pr_usage.PROJECTS, self.tmp.name
        self.project = Path(self.tmp.name) / ''.join(c if c.isalnum() else '-' for c in TOP)
        self.project.mkdir()

    def tearDown(self):
        pr_usage.PROJECTS = self.projects
        self.tmp.cleanup()

    def session(self, name, *branches, start=12):
        """A session that, for each of the branches in turn, takes a prompt, switches to the branch and runs its
        tests: the switch's response is on the branch before, or on `main` and so counts toward the branch."""
        s = Session()
        s.time = s.time.replace(hour=start)
        for branch in branches:
            s.line('user', 'do it', role='user')
            s.bash('git switch -c %s origin/main' % branch)
            s.branch = branch
            s.bash('cargo test')
        for line in s.lines:  # the lines and the responses of two sessions are not the same ones
            line['uuid'] = name + line['uuid']
            if 'id' in line['message']:
                line['message']['id'] = name + line['message']['id']
        s.write(self.project / (name + '.jsonl'))

    def count(self, *wanted):
        return pr_usage.count([set(names) for names in wanted], TOP, {}, set())

    def test_each_pull_request_gets_the_responses_of_its_branch(self):
        self.session('aaaaaaaa-1', 'bug/1-a', 'bug/2-b')
        self.session('bbbbbbbb-2', 'bug/2-b', start=15)
        a, b, c = self.count(['bug/1-a'], ['bug/2-b'], ['bug/3-c'])
        # The first session's switch to bug/2-b is a response on bug/1-a still.
        self.assertEqual([r[0] for r in a.per_model.values()], [3])
        self.assertEqual([r[0] for r in b.per_model.values()], [1 + 2])
        self.assertEqual(c.per_model, {})
        self.assertEqual([s for s, _ in a.sessions], ['aaaaaaaa'])
        self.assertEqual([s for s, _ in b.sessions], ['aaaaaaaa', 'bbbbbbbb'])
        self.assertEqual(c.sessions, [])
        # A response's tokens and dollars: 1 token of input and 1 of output on Opus 5.5.
        self.assertEqual(a.per_model['claude-opus-5-5'][1:6], [3, 3, 0, 0, 0])
        self.assertAlmostEqual(a.per_model['claude-opus-5-5'][6], 3 * (4 + 20) / 1e6)

    def test_a_pass_of_several_counts_each_as_a_pass_of_its_own(self):
        self.session('aaaaaaaa-1', 'bug/1-a', 'bug/2-b', 'bug/1-a')
        self.session('bbbbbbbb-2', 'bug/2-b', start=15)
        together = self.count(['bug/1-a'], ['bug/2-b'])
        for one, names in zip(together, (['bug/1-a'], ['bug/2-b'])):
            alone, = self.count(names)
            self.assertEqual((one.per_model, one.sessions, one.active, one.wall),
                             (alone.per_model, alone.sessions, alone.active, alone.wall))
        self.assertGreater(together[0].active, dt.timedelta())
        self.assertGreater(together[0].wall, together[0].active)  # the other branch's time lies between

    def test_another_name_of_the_branch_counts_with_it(self):
        self.session('aaaaaaaa-1', 'wip-a', 'bug/2-b', 'bug/1-a')
        both, old, new = self.count(['bug/1-a', 'wip-a'], ['wip-a'], ['bug/1-a'])
        self.assertEqual([r[0] for r in old.per_model.values()], [3])
        self.assertEqual([r[0] for r in new.per_model.values()], [1])
        self.assertEqual([r[0] for r in both.per_model.values()], [3 + 1])
        self.assertEqual(both.sessions, [('aaaaaaaa', old.sessions[0][1] + new.sessions[0][1])])

    def test_the_time_is_the_agents_and_a_pause_before_a_prompt_is_not(self):
        self.session('aaaaaaaa-1', 'bug/1-a', 'bug/1-a')
        a, = self.count(['bug/1-a'])
        # Twice a prompt and two responses, of two lines and a result each: 14 lines five seconds apart, and the
        # pause before the second prompt is the owner's.
        self.assertEqual(a.wall, dt.timedelta(seconds=13 * 5))
        self.assertEqual(a.active, dt.timedelta(seconds=12 * 5))


class Block(unittest.TestCase):
    def counted(self):
        c = pr_usage.Counted()
        c.per_model = {'claude-opus-5-5': [3, 10, 2_000_000, 30_000_000, 0, 500_000, 50.0],
                       'claude-unknown-9': [1, 0, 1_000, 0, 0, 0, 0.0]}
        c.sessions, c.active, c.wall = [('aaaaaaaa', 9)], dt.timedelta(minutes=90), dt.timedelta(hours=2)
        return c

    def test_the_figures_read_back_from_the_block(self):
        block, data = pr_usage.block_of(self.counted(), [], 'https://example.com/o/r')
        self.assertEqual(data, {'input': 10, 'output': 2_001_000, 'cache_read': 30_000_000, 'cache_write': 500_000,
                                'cost_usd': 50.0, 'agent_hours': 1.5, 'wall_hours': 2.0, 'sessions': 1})
        self.assertEqual(pr_usage.figures('Closes #1\n\n' + block), data)
        self.assertEqual(pr_usage.tokens(data), 32_501_010)
        self.assertIn('| Opus 5.5 | 3 | 0.000 | 2.000 | 30.00 | 0.500 | 50.00 |', block)
        self.assertIn('| claude-unknown-9 | 1 | 0.000 | 0.001 | 0.00 | 0.000 | not priced |', block)
        self.assertIn('≈1.5 agent-hours (wall clock 2.0) in 1 session.', block)

    def test_the_other_names_of_the_branch_stay_in_the_block(self):
        block, data = pr_usage.block_of(self.counted(), ['wip-a', 'wip-b'], 'https://example.com/o/r')
        self.assertEqual(data['also'], ['wip-a', 'wip-b'])
        self.assertEqual(pr_usage.figures(block)['also'], ['wip-a', 'wip-b'])
        self.assertIn('The work done under the names `wip-a`, `wip-b` counts here too.', block)

    def test_a_description_without_a_block_that_reads_has_no_figures(self):
        for body in (None, '', 'Closes #1', '<!-- usage {"input": 1 -->', '<!-- usage [1] -->'):
            with self.subTest(body=body):
                self.assertIsNone(pr_usage.figures(body))
        self.assertEqual(pr_usage.figures('<!-- usage {"input": 1} -->\n<!-- usage {"input": 2} -->'), {'input': 2})
        self.assertEqual(pr_usage.tokens({'input': 1, 'output': None}), 1)

    def test_the_block_takes_the_place_of_the_one_before(self):
        old = '%s\nold\n%s' % (pr_usage.BEGIN, pr_usage.END)
        new = '%s\nnew\n%s' % (pr_usage.BEGIN, pr_usage.END)
        attribution = pr_usage.ATTRIBUTION + ' [Claude Code](https://claude.com/claude-code)'
        self.assertEqual(pr_usage.spliced('Closes #1\n\n%s\n\n%s\n' % (old, attribution), new),
                         'Closes #1\n\n%s\n\n%s\n' % (new, attribution))
        self.assertEqual(pr_usage.spliced('Closes #1\n\n%s\n' % attribution, new),
                         'Closes #1\n\n%s\n\n%s\n' % (new, attribution))
        self.assertEqual(pr_usage.spliced('Closes #1\n\n', new), 'Closes #1\n\n%s\n' % new)
        self.assertEqual(pr_usage.spliced(None, new), '\n\n%s\n' % new)

    def test_a_description_goes_to_the_command_with_its_line_ends(self):
        echo = [sys.executable, '-c', 'import sys; sys.stdout.write(repr(sys.stdin.buffer.read()))']
        self.assertEqual(pr_usage.run(echo, stdin='Closes #1\n\nµ\n'), repr('Closes #1\n\nµ\n'.encode('utf-8')))
        self.assertEqual(pr_usage.run(echo), repr(b''))

    def test_the_carriage_returns_of_earlier_runs_are_dropped(self):
        self.assertEqual(pr_usage.unix('a\r\nb\r\r\r\nc\nd\re\r'), 'a\nb\nc\nd\re\r')
        block = '%s\nthe same\n%s' % (pr_usage.BEGIN, pr_usage.END)
        stored = ('Closes #1\n\n%s\n\n%s\n' % (block, pr_usage.ATTRIBUTION)).replace('\n', '\r\r\n')
        # A description stored with them is written once more, without them; after that, its block the same, the
        # one to write is the one stored, and it is left.
        cleaned = pr_usage.spliced(pr_usage.unix(stored), block)
        self.assertNotEqual(cleaned, stored)
        self.assertNotIn('\r', cleaned)
        self.assertEqual(pr_usage.spliced(pr_usage.unix(cleaned), block), cleaned)


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
