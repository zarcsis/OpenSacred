#!/usr/bin/env python3
"""Tests of .claude/hooks/merge_guard.py that need neither gh nor a pull request: which files wait for the
maintainer, which pull requests merge (with `gh pr view` stubbed), and which commands merge.

Example
  python -I -m unittest discover -s .claude/hooks -p "test_*.py"

Dependencies: standard library.
"""
import json, subprocess, sys, unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import merge_guard  # noqa: E402


class Waiting(unittest.TestCase):
    def test_a_decision_does_not_wait_but_asks_for_the_review_label(self):
        waits, reviews = merge_guard.waiting(['docs/decisions/2026-10-17-hud.md', 'crates/a.rs'])
        self.assertEqual((waits, reviews), ([], ['docs/decisions/2026-10-17-hud.md']))

    def test_claude_md_and_a_skill_are_reviewed_after_the_merge(self):
        paths = ['CLAUDE.md', '.claude/skills/game/SKILL.md']
        self.assertEqual(merge_guard.waiting(paths), ([], paths))

    def test_the_guardrails_wait(self):
        paths = ['docs/decisions/README.md', '.claude/hooks/merge_guard.py', '.claude/settings.json',
                 '.github/workflows/ci.yml', 'deny.toml', 'crates/opensacred-game/tests/screenshots/scene.png']
        self.assertEqual(merge_guard.waiting(paths), (paths, []))

    def test_other_files_do_not_wait(self):
        self.assertEqual(merge_guard.waiting(['docs/re/sound.md', 'tools/x.py']), ([], []))


class Refusal(unittest.TestCase):
    def refusal(self, labels, paths, author='zarcsis'):
        pr = {'number': 7, 'url': 'https://github.com/zarcsis/OpenSacred/pull/7', 'author': {'login': author},
              'labels': [{'name': label} for label in labels], 'files': [{'path': path} for path in paths]}
        result = subprocess.CompletedProcess([], 0, stdout=json.dumps(pr), stderr='')
        with mock.patch.object(merge_guard.subprocess, 'run', return_value=result):
            return merge_guard.refusal(['7', '--squash', '--auto'])

    def test_a_decision_with_the_review_label_merges(self):
        self.assertIsNone(self.refusal(['feature', 'review-after-merge'], ['docs/decisions/2026-10-17-hud.md']))

    def test_a_decision_without_a_label_names_both(self):
        reason = self.refusal(['feature'], ['docs/decisions/2026-10-17-hud.md'])
        self.assertIn('review-after-merge', reason)
        self.assertIn('needs-maintainer', reason)

    def test_a_decision_labeled_for_the_maintainer_waits(self):
        reason = self.refusal(['needs-maintainer', 'review-after-merge'], ['docs/decisions/2026-10-17-hud.md'])
        self.assertIn('waits for the maintainer', reason)

    def test_claude_md_with_the_review_label_merges(self):
        self.assertIsNone(self.refusal(['infra', 'review-after-merge'], ['CLAUDE.md', '.claude/skills/game/SKILL.md']))

    def test_claude_md_without_a_label_names_both(self):
        reason = self.refusal(['infra'], ['CLAUDE.md'])
        self.assertIn('review-after-merge', reason)
        self.assertIn('needs-maintainer', reason)
        self.assertIn('`CLAUDE.md`', reason)

    def test_a_hook_waits_whatever_its_labels(self):
        reason = self.refusal(['infra', 'review-after-merge'], ['CLAUDE.md', '.claude/hooks/merge_guard.py'])
        self.assertIn('waits for the maintainer', reason)
        self.assertIn('`.claude/hooks/merge_guard.py`', reason)

    def test_a_contributors_pull_request_waits(self):
        self.assertIn('opened by someone', self.refusal(['bug'], ['crates/a.rs'], author='someone'))


class Merges(unittest.TestCase):
    def test_only_a_command_that_starts_with_gh_pr_merge_merges(self):
        self.assertEqual(list(merge_guard.merges('gh pr merge 7 --squash --auto')), [['7', '--squash', '--auto']])
        self.assertEqual(list(merge_guard.merges('git commit -m "gh pr merge 7"')), [])


if __name__ == '__main__':
    unittest.main()
