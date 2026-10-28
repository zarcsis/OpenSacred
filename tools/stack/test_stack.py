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


class MergeOrder(unittest.TestCase):
    def test_a_stacked_pull_request_comes_after_its_base(self):
        prs = [
            {'number': 9, 'headRefName': 'c', 'baseRefName': 'b'},
            {'number': 5, 'headRefName': 'b', 'baseRefName': 'a'},
            {'number': 7, 'headRefName': 'x', 'baseRefName': 'main'},
            {'number': 3, 'headRefName': 'a', 'baseRefName': 'main'},
        ]
        self.assertEqual([p['number'] for p in stack.merge_order(prs)], [3, 5, 7, 9])


if __name__ == '__main__':
    unittest.main()
