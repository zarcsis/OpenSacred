#!/usr/bin/env python3
"""Tests of tools/stack/claim.py on temporary repositories, with `gh` stood in for: an issue that no session holds is
taken (assigned, its branch pushed, its worktree added), and one that a session holds is refused, naming what holds it.

Example
  uv run python -I -m unittest discover -s tools/stack -p "test_*.py"

Dependencies: standard library, `git` 2.40+.
"""
import subprocess, sys, tempfile, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import claim  # noqa: E402


def git(cwd, *args):
    return subprocess.run(['git', '-C', str(cwd), *args], check=True, capture_output=True, text=True).stdout


ACCOUNT = 'zarcsis'


class FakeGitHub:
    """The account, the issue and the open pull requests that `gh` would give, and the assignments asked for; the
    pull requests of `open_prs` filtered by their author, as `gh pr list --author` does."""

    def __init__(self, issue, prs=()):
        self.issue_, self.prs, self.assigned, self.read = issue, list(prs), [], []

    def account(self):
        return ACCOUNT

    def issue(self, number):
        return self.issue_

    def open_prs(self, author):
        self.read.append(author)
        return [pr for pr in self.prs if pr['author']['login'] == author]

    def pr_of(self, branch):
        return next((pr['author']['login'] for pr in self.prs if pr['headRefName'] == branch), None)

    def assign(self, number):
        self.assigned.append(number)


def issue(number=7, state='OPEN', assignees=(), labels=('infra',), author=ACCOUNT):
    return {'number': number, 'title': 'A task', 'state': state, 'assignees': [{'login': a} for a in assignees],
            'labels': [{'name': name} for name in labels], 'author': {'login': author}}


def pr(number, head, body='', author=ACCOUNT):
    return {'number': number, 'headRefName': head, 'body': body, 'author': {'login': author}}


class Take(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        root = Path(self.dir.name)
        self.origin, self.checkout = root / 'origin.git', root / 'checkout'
        git(root, 'init', '--quiet', '--bare', '--initial-branch=main', str(self.origin))
        git(root, 'clone', '--quiet', str(self.origin), str(self.checkout))
        for key, value in (('user.name', 'Test'), ('user.email', 'test@example.com'), ('core.autocrlf', 'false')):
            git(self.checkout, 'config', key, value)
        (self.checkout / 'README.md').write_bytes(b'A repository.\n')
        git(self.checkout, 'add', 'README.md')
        git(self.checkout, 'commit', '--quiet', '-m', 'start')
        git(self.checkout, 'push', '--quiet', 'origin', 'main')

    def tearDown(self):
        # A worktree's files are the checkout's; the directory goes as a whole.
        self.dir.cleanup()

    def take(self, github, slug='claim', **options):
        return claim.take(7, slug, 'origin/main', root=self.checkout, github=github, **options)

    def refused(self, github, **options):
        with self.assertRaises(SystemExit) as refused:
            self.take(github, **options)
        return str(refused.exception)

    def test_a_free_issue_is_taken(self):
        github = FakeGitHub(issue())
        tree = self.take(github)
        self.assertEqual(tree, self.checkout / '.claude' / 'worktrees' / '7-claim')
        self.assertEqual(git(tree, 'branch', '--show-current').strip(), 'infra/7-claim')
        self.assertIn('refs/heads/infra/7-claim', git(self.checkout, 'ls-remote', '--heads', 'origin'))
        self.assertEqual(github.assigned, [7])

    def test_an_issue_taken_by_another_session_is_refused_by_its_branch(self):
        self.take(FakeGitHub(issue()))
        reason = self.refused(FakeGitHub(issue()), slug='other')
        self.assertIn('the branch infra/7-claim', reason)

    def test_an_assigned_issue_is_refused_unless_this_session_filed_it(self):
        self.assertIn('assigned to zarcsis', self.refused(FakeGitHub(issue(assignees=['zarcsis']))))
        self.take(FakeGitHub(issue(assignees=['zarcsis'])), mine=True)

    def test_an_open_pull_request_that_closes_it_holds_it(self):
        prs = [pr(9, 'feature/70-other', 'Closes #7\n\n## What and why')]
        self.assertIn('the open pull request #9', self.refused(FakeGitHub(issue(), prs)))
        unrelated = [pr(10, 'feature/70-other', 'Closes #70')]
        self.take(FakeGitHub(issue(), unrelated))

    def test_an_issue_of_another_account_is_refused(self):
        reason = self.refused(FakeGitHub(issue(author='someone')))
        self.assertIn('was filed by someone, not by zarcsis', reason)
        self.assertIn('was filed by app/someones-app', self.refused(FakeGitHub(issue(author='app/someones-app'))))

    def test_an_issue_of_the_dependency_update_is_taken(self):
        self.take(FakeGitHub(issue(author='app/opensacred-deps')))

    def test_a_pull_request_of_another_account_is_not_read(self):
        github = FakeGitHub(issue(), [pr(9, 'feature/70-other', 'Closes #7', author='someone')])
        self.take(github)
        self.assertEqual(github.read, [ACCOUNT])

    def test_a_stack_on_another_accounts_pull_request_is_refused(self):
        github = FakeGitHub(issue(), [pr(9, 'feature/70-other', author='someone')])
        with self.assertRaises(SystemExit) as refused:
            claim.take(7, 'claim', 'origin/feature/70-other', root=self.checkout, github=github)
        self.assertIn('a pull request by someone', str(refused.exception))

    def test_a_closed_issue_or_one_without_a_type_is_refused(self):
        self.assertIn('it is closed', self.refused(FakeGitHub(issue(state='CLOSED'))))
        self.assertIn('0 type labels', self.refused(FakeGitHub(issue(labels=('needs-maintainer',)))))

    def test_a_dry_run_changes_nothing(self):
        github = FakeGitHub(issue())
        tree = self.take(github, dry_run=True)
        self.assertFalse(tree.exists())
        self.assertEqual(github.assigned, [])
        self.assertNotIn('infra/7-claim', git(self.checkout, 'ls-remote', '--heads', 'origin'))

    def test_a_slug_is_lower_case_words(self):
        self.assertIn('a slug is', self.refused(FakeGitHub(issue()), slug='Bad Slug'))


if __name__ == '__main__':
    unittest.main()
