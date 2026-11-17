#!/usr/bin/env python3
"""Tests of tools/stack/stack.py on a temporary repository: a pull request stacked on one that is squash-merged.

Example
  uv run python -I -m unittest discover -s tools/stack -p "test_*.py"

Dependencies: standard library, `git` 2.40+.
"""
import subprocess, sys, tempfile, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import stack  # noqa: E402


class Repo:
    """A temporary repository with commits made from file contents."""

    def __init__(self, root):
        self.git = stack.Git(root)
        self.root = root
        self.git.run('init', '--quiet', '--initial-branch=main')
        self.git.run('config', 'user.name', 'Test')
        self.git.run('config', 'user.email', 'test@example.com')
        self.git.run('config', 'core.autocrlf', 'false')

    def commit(self, files, message):
        for name, text in files.items():
            path = self.root / name
            path.write_bytes(text.encode())
            self.git.run('add', name)
        self.git.run('commit', '--quiet', '-m', message)
        return self.git.out('rev-parse', 'HEAD')

    def show(self, rev, name):
        return self.git.out('show', f'{rev}:{name}')


class BringUp(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = Repo(Path(self.tmp.name))
        r = self.repo
        self.start = r.commit({'a.txt': '1\n2\n3\n', 'b.txt': 'x\n'}, 'start')
        r.git.run('switch', '--quiet', '-c', 'parent')
        self.parent = r.commit({'a.txt': '1\nP\n3\n'}, 'parent')
        r.git.run('switch', '--quiet', '-c', 'child')
        self.child = r.commit({'a.txt': '1\nC\n3\n', 'c.txt': 'new\n'}, 'child: its own version of the line')
        # main takes the parent as one squashed commit, then another pull request.
        r.git.run('switch', '--quiet', 'main')
        r.commit({'a.txt': '1\nP\n3\n'}, 'parent (#1)')
        self.main = r.commit({'b.txt': 'y\n'}, 'other (#2)')

    def tearDown(self):
        self.tmp.cleanup()

    def test_a_plain_merge_conflicts_where_the_child_changed_the_parents_line(self):
        tree, conflicts = self.repo.git.merge(self.main, self.child)
        self.assertIsNone(tree)
        self.assertEqual(conflicts, ['a.txt'])

    def test_the_child_keeps_its_lines_and_gains_what_main_got_beyond_the_parent(self):
        tree, conflicts, base = stack.bring_up(self.repo.git, self.parent, self.child, self.main)
        self.assertEqual(conflicts, [])
        self.assertEqual(base, self.parent)
        self.assertEqual(self.repo.show(tree, 'a.txt'), '1\nC\n3')
        self.assertEqual(self.repo.show(tree, 'b.txt'), 'y')
        self.assertEqual(self.repo.show(tree, 'c.txt'), 'new')

    def test_a_pull_request_not_stacked_on_the_parent_is_left_alone(self):
        r = self.repo
        r.git.run('switch', '--quiet', '-c', 'alone', self.start)
        alone = r.commit({'d.txt': 'd\n'}, 'alone')
        self.assertIsNone(stack.bring_up(r.git, self.parent, alone, self.main))

    def test_a_child_that_holds_main_already_is_left_alone(self):
        r = self.repo
        tree, _, _ = stack.bring_up(r.git, self.parent, self.child, self.main)
        merged = r.git.commit(tree, [self.child, self.main], 'merge')
        self.assertIsNone(stack.bring_up(r.git, self.parent, merged, self.main))


class CoAuthors(unittest.TestCase):
    OPUS = 'Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>'
    MYTHOS = 'Co-Authored-By: Claude Mythos 5.2 <noreply@anthropic.com>'

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = Repo(Path(self.tmp.name))
        # main's own commit names a co-author that no branch takes.
        self.main = self.repo.commit({'a.txt': '1\n'}, f'start\n\n{self.OPUS}')
        self.repo.git.run('switch', '--quiet', '-c', 'branch')

    def tearDown(self):
        self.tmp.cleanup()

    def test_a_merge_names_the_co_authors_of_the_branchs_newest_commit_that_has_any(self):
        r = self.repo
        r.commit({'b.txt': '1\n'}, f'first\n\n{self.OPUS}')
        r.commit({'b.txt': '2\n'}, f'second\n\nIts body.\n\n{self.MYTHOS}\nCo-authored-by: Other <o@example.com>')
        tip = r.commit({'b.txt': '3\n'}, 'third, by hand')
        self.assertEqual(stack.co_authors(r.git, tip, self.main),
                         f'{self.MYTHOS}\nCo-authored-by: Other <o@example.com>')

    def test_a_branch_whose_commits_name_none_gives_none(self):
        tip = self.repo.commit({'b.txt': '1\n'}, 'by hand\n\nSigned-off-by: Test <test@example.com>')
        self.assertEqual(stack.co_authors(self.repo.git, tip, self.main), '')
        self.assertEqual(stack.co_authors(self.repo.git, self.main, self.main), '')

    def test_an_earlier_merges_trailer_is_not_taken(self):
        r = self.repo
        own = r.commit({'b.txt': '1\n'}, f'own\n\n{self.MYTHOS}')
        r.git.run('switch', '--quiet', 'main')
        main = r.commit({'a.txt': '2\n'}, 'other (#2)')
        tree, _ = r.git.merge(own, main)
        merged = r.git.commit(tree, [own, main], f'Merge main into branch\n\n{self.OPUS}')
        self.assertEqual(stack.co_authors(r.git, merged, main), self.MYTHOS)

    def test_the_trailers_are_a_paragraph_of_their_own(self):
        self.assertEqual(stack.with_trailers('Merge #1 into x', self.MYTHOS), f'Merge #1 into x\n\n{self.MYTHOS}')
        self.assertEqual(stack.with_trailers('Merge #1 into x', ''), 'Merge #1 into x')


class MergeOrder(unittest.TestCase):
    def test_a_stacked_pull_request_comes_after_its_base(self):
        prs = [
            {'number': 9, 'headRefName': 'c', 'baseRefName': 'b'},
            {'number': 5, 'headRefName': 'b', 'baseRefName': 'a'},
            {'number': 7, 'headRefName': 'x', 'baseRefName': 'main'},
            {'number': 3, 'headRefName': 'a', 'baseRefName': 'main'},
        ]
        self.assertEqual([p['number'] for p in stack.merge_order(prs)], [3, 5, 7, 9])


class IntegrationTarget(unittest.TestCase):
    def test_the_check_builds_apart_from_the_checkouts_target(self):
        self.assertEqual(stack.integration_target({}), stack.ROOT / 'target' / 'integration')
        shared = Path(tempfile.gettempdir()) / 'shared-target'
        self.assertEqual(stack.integration_target({'CARGO_TARGET_DIR': str(shared)}), shared / 'integration')


if __name__ == '__main__':
    unittest.main()
