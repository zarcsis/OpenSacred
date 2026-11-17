#!/usr/bin/env python3
"""Tests of tools/usage/usage_badge.py on synthetic listings: the pull requests it sums into the badge.

Example
  uv run python -I -m unittest discover -s tools/usage -p "test_*.py"

Dependencies: standard library.
"""
import json, subprocess, sys, tempfile, unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent / 'usage_badge.py'
# The sessions before per-pull-request tracking, which every badge starts from (BEFORE of the script).
BEFORE_TOKENS, BEFORE_DOLLARS = 3.05e6 + 1018.2e6 + 10.66e6, 337.7


def pull_request(number, author, tokens=None, dollars=0.0):
    """A pull request of a listing, with a usage block of that many cache-read tokens when it has one."""
    body = 'Closes #1\n'
    if tokens is not None:
        data = {'input': 0, 'output': 0, 'cache_read': tokens, 'cache_write': 0, 'cost_usd': dollars,
                'agent_hours': 1.0, 'wall_hours': 1.0, 'sessions': 1}
        body += '<!-- usage:begin -->\n<!-- usage %s -->\n<!-- usage:end -->\n' % json.dumps(data)
    return {'number': number, 'title': 'Title %d' % number, 'body': body,
            'author': {'login': author, 'is_bot': False} if author else None}


def badge(prs, *options):
    """The script's run on a listing: (its exit code, the badge's message or None, the summary, its error)."""
    with tempfile.TemporaryDirectory() as tmp:
        listing, out, summary = (Path(tmp) / name for name in ('prs.json', 'usage.json', 'summary.md'))
        listing.write_text(json.dumps(prs), encoding='utf-8', newline='')
        result = subprocess.run([sys.executable, '-I', str(SCRIPT), str(listing), '--badge', str(out), '--summary',
                                 str(summary), *options], capture_output=True, text=True, encoding='utf-8')
        message = json.loads(out.read_text(encoding='utf-8'))['message'] if out.exists() else None
        return result.returncode, message, summary.read_text(encoding='utf-8') if summary.exists() else '', \
            result.stderr


def message(tokens, dollars):
    return '%.3gB tokens · $%.0f at API prices' % ((BEFORE_TOKENS + tokens) / 1e9, BEFORE_DOLLARS + dollars)


class Badge(unittest.TestCase):
    def test_the_blocks_sum_with_the_sessions_before(self):
        prs = [pull_request(2, 'owner', 2e9, 500.0), pull_request(3, 'owner', 1e9, 250.25),
               pull_request(4, 'owner')]  # no block
        code, text, summary, _ = badge(prs)
        self.assertEqual((code, text), (0, message(3e9, 750.25)))
        self.assertIn('| #2 Title 2 | 2000.0 |', summary)
        self.assertNotIn('#4', summary)

    def test_an_account_alone_counts_when_named(self):
        prs = [pull_request(2, 'owner', 2e9, 500.0), pull_request(3, 'guest', 1e9, 250.0),
               pull_request(4, 'app/updates', 1e9, 250.0), pull_request(5, None, 1e9, 250.0)]
        code, text, summary, _ = badge(prs, '--author', 'owner')
        self.assertEqual((code, text), (0, message(2e9, 500.0)))
        self.assertIn('#2', summary)
        for other in ('#3', '#4', '#5'):
            self.assertNotIn(other, summary)
        self.assertEqual(badge(prs)[1], message(5e9, 1250.0))  # every pull request of the listing without it

    def test_more_pull_requests_than_a_search_answers_all_count(self):
        prs = [pull_request(n, 'owner', 1e6, 1.0) for n in range(1, 1501)]
        self.assertEqual(badge(prs, '--author', 'owner')[:2], (0, message(1500e6, 1500.0)))

    def test_a_listing_without_authors_fails_when_an_account_is_named(self):
        prs = [pull_request(2, 'owner', 2e9, 500.0)]
        del prs[0]['author']
        code, text, _, error = badge(prs, '--author', 'owner')
        self.assertNotEqual(code, 0)
        self.assertIsNone(text)
        self.assertIn('author', error)

    def test_a_malformed_block_is_skipped(self):
        prs = [pull_request(2, 'owner', 2e9, 500.0), pull_request(3, 'owner', -1, 250.0)]
        prs.append(dict(pull_request(4, 'owner'), body='<!-- usage {"input": 1} -->'))
        code, text, _, error = badge(prs)
        self.assertEqual((code, text), (0, message(2e9, 500.0)))
        self.assertIn('#3', error)
        self.assertIn('#4', error)


if __name__ == '__main__':
    unittest.main()
