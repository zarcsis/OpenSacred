#!/usr/bin/env python3
"""Tests of .claude/hooks/merge_guard.py that need neither gh nor a pull request: which files wait for the
maintainer, and which commands merge.

Example
  python -I -m unittest discover -s .claude/hooks -p "test_*.py"

Dependencies: standard library.
"""
import sys, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import merge_guard  # noqa: E402


class Waiting(unittest.TestCase):
    def test_an_added_decision_does_not_wait_but_asks_for_the_review_label(self):
        waits, adds = merge_guard.waiting([('docs/decisions/2026-10-19-x.md', 'added'), ('crates/a.rs', 'modified')])
        self.assertEqual((waits, adds), ([], True))

    def test_a_changed_or_removed_decision_waits(self):
        for status in ('modified', 'removed', 'renamed'):
            waits, adds = merge_guard.waiting([('docs/decisions/2026-10-17-hud.md', status)])
            self.assertEqual((waits, adds), (['docs/decisions/2026-10-17-hud.md'], False), status)

    def test_the_decisions_readme_and_the_rules_wait(self):
        waits, adds = merge_guard.waiting([('docs/decisions/README.md', 'added'), ('CLAUDE.md', 'modified'),
                                           ('.github/workflows/ci.yml', 'modified')])
        self.assertEqual(waits, ['docs/decisions/README.md', 'CLAUDE.md', '.github/workflows/ci.yml'])
        self.assertFalse(adds)

    def test_other_files_do_not_wait(self):
        self.assertEqual(merge_guard.waiting([('docs/re/sound.md', 'modified'), ('tools/x.py', 'added')]), ([], False))


class Merges(unittest.TestCase):
    def test_only_a_command_that_starts_with_gh_pr_merge_merges(self):
        self.assertEqual(list(merge_guard.merges('gh pr merge 7 --squash --auto')), [['7', '--squash', '--auto']])
        self.assertEqual(list(merge_guard.merges('git commit -m "gh pr merge 7"')), [])


if __name__ == '__main__':
    unittest.main()
