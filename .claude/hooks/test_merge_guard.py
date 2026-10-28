#!/usr/bin/env python3
"""Tests of .claude/hooks/merge_guard.py that need neither gh nor a pull request: which files wait for the
maintainer, which pull requests merge (with `gh pr view` stubbed), and which commands merge.

Example
  uv run python -I -m unittest discover -s .claude/hooks -p "test_*.py"

Dependencies: standard library.
"""
import json, subprocess, sys, unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import merge_guard  # noqa: E402


class Waiting(unittest.TestCase):
    def test_a_decision_does_not_wait(self):
        self.assertEqual(merge_guard.waiting(['docs/decisions/interface.md', 'crates/a.rs']), [])

    def test_claude_md_and_a_skill_do_not_wait(self):
        self.assertEqual(merge_guard.waiting(['CLAUDE.md', '.claude/skills/game/SKILL.md']), [])

    def test_the_guardrails_wait(self):
        paths = ['docs/decisions/README.md', '.claude/hooks/merge_guard.py', '.claude/settings.json',
                 '.github/workflows/ci.yml', 'deny.toml', 'crates/opensacred-game/tests/screenshots/scene.png']
        self.assertEqual(merge_guard.waiting(paths), paths)

    def test_other_files_do_not_wait(self):
        self.assertEqual(merge_guard.waiting(['docs/re/sound.md', 'tools/x.py']), [])


class Refusal(unittest.TestCase):
    def refusal(self, labels, paths, author='zarcsis'):
        pr = {'number': 7, 'url': 'https://github.com/zarcsis/OpenSacred/pull/7', 'author': {'login': author},
              'labels': [{'name': label} for label in labels], 'files': [{'path': path} for path in paths]}
        result = subprocess.CompletedProcess([], 0, stdout=json.dumps(pr), stderr='')
        with mock.patch.object(merge_guard.subprocess, 'run', return_value=result):
            return merge_guard.refusal(['7', '--squash', '--auto'])

    def test_a_decision_merges(self):
        self.assertIsNone(self.refusal(['feature'], ['docs/decisions/interface.md']))

    def test_a_decision_labeled_for_the_maintainer_waits(self):
        reason = self.refusal(['needs-maintainer'], ['docs/decisions/interface.md'])
        self.assertIn('waits for the maintainer', reason)

    def test_claude_md_and_a_skill_merge(self):
        self.assertIsNone(self.refusal(['infra'], ['CLAUDE.md', '.claude/skills/game/SKILL.md']))

    def test_a_rule_of_claude_md_labeled_for_the_maintainer_waits(self):
        self.assertIn('labeled `needs-maintainer`', self.refusal(['infra', 'needs-maintainer'], ['CLAUDE.md']))

    def test_a_hook_waits_whatever_its_labels(self):
        reason = self.refusal(['infra'], ['CLAUDE.md', '.claude/hooks/merge_guard.py'])
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
