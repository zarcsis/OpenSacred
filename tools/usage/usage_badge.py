#!/usr/bin/env python3
"""The Claude Code usage badge: the total of the usage blocks in the pull requests' descriptions (docs/usage.md).

Purpose (docs/usage.md; .github/workflows/usage.yml): `tools/usage/pr_usage.py` writes each pull request's usage
into its description, with a machine-readable comment `<!-- usage {...} -->`. This script sums those of the pull
requests in a `gh pr list --json number,title,body` listing, adds the sessions before per-pull-request tracking
(the table of docs/usage.md), and writes a shields.io endpoint for the README's badge and, optionally, a Markdown
table of the pull requests for the workflow's run summary. The Usage workflow runs it; locally:

  gh pr list --state all --author <owner> --limit 10000 --json number,title,body > prs.json
  python -I tools/usage/usage_badge.py prs.json --badge usage.json --summary summary.md

Expected output: usage.json, {"schemaVersion": 1, "label": "Claude Code", "message": "1.03B tokens · $338 at API
prices", "color": "blue"}, and the table in summary.md. The dollars are the cost of the same tokens at API prices,
not money spent: development runs on a Claude subscription. Dependencies: standard library.
"""
import argparse, json, math, re, sys

KINDS = ('input', 'output', 'cache_read', 'cache_write')
FIELDS = KINDS + ('cost_usd', 'agent_hours')
# The sessions of 2026-10-16 and 2026-10-17 before per-pull-request tracking: the sums of the table in
# docs/usage.md (its input column is ≈0).
BEFORE = {'input': 0, 'output': 3.05e6, 'cache_read': 1018.2e6, 'cache_write': 10.66e6, 'cost_usd': 337.7,
          'agent_hours': 12.7}
BLOCK = re.compile(r'<!-- usage (\{.*?\}) -->')

ap = argparse.ArgumentParser(description='Sum the Claude Code usage blocks of pull requests into a badge.')
ap.add_argument('listing', help='JSON from gh pr list --json number,title,body')
ap.add_argument('--badge', required=True, help='shields.io endpoint JSON to write')
ap.add_argument('--summary', help='Markdown file to append the table of pull requests to')
args = ap.parse_args()


def usage(pr):
    """The pull request's usage, or None without a valid block."""
    found = BLOCK.findall(pr.get('body') or '')
    if not found:
        return None
    try:
        data = json.loads(found[-1])
        values = {k: float(data[k]) for k in FIELDS}
    except (ValueError, KeyError, TypeError):
        print('#%s: the usage block is malformed, skipped' % pr.get('number'), file=sys.stderr)
        return None
    if not all(math.isfinite(v) and v >= 0 for v in values.values()):
        print('#%s: the usage block has a negative or infinite value, skipped' % pr.get('number'), file=sys.stderr)
        return None
    return values


def tokens(row):
    return sum(row[k] for k in KINDS)


def short(n):
    for size, suffix in ((1e9, 'B'), (1e6, 'M'), (1e3, 'k')):
        if n >= size:
            return '%.3g%s' % (n / size, suffix)
    return '%d' % n


with open(args.listing, encoding='utf-8') as f:
    prs = sorted(json.load(f), key=lambda pr: pr['number'])
rows = [(pr, u) for pr in prs if (u := usage(pr)) is not None]
total = {k: BEFORE[k] + sum(u[k] for _, u in rows) for k in FIELDS}

badge = {'schemaVersion': 1, 'label': 'Claude Code',
         'message': '%s tokens · $%.0f at API prices' % (short(tokens(total)), total['cost_usd']), 'color': 'blue'}
with open(args.badge, 'w', encoding='utf-8') as f:
    json.dump(badge, f, ensure_ascii=False)
    f.write('\n')
print(badge['message'])

if args.summary:
    lines = ['## Claude Code usage', '',
             'The dollars are the cost of the same tokens at API prices, not money spent (docs/usage.md).', '',
             '| Pull request | Tokens, M | Output, M | At API prices, $ | Agent-hours |', '|---|---|---|---|---|']
    lines.append('| sessions before 2026-10-17 (docs/usage.md) | %.1f | %.2f | %.2f | %.1f |' % (
        tokens(BEFORE) / 1e6, BEFORE['output'] / 1e6, BEFORE['cost_usd'], BEFORE['agent_hours']))
    for pr, u in rows:
        title = (pr.get('title') or '').replace('|', '\\|')
        lines.append('| #%d %s | %.1f | %.2f | %.2f | %.1f |' % (
            pr['number'], title, tokens(u) / 1e6, u['output'] / 1e6, u['cost_usd'], u['agent_hours']))
    lines.append('| **total** | **%.1f** | **%.2f** | **%.2f** | **%.1f** |' % (
        tokens(total) / 1e6, total['output'] / 1e6, total['cost_usd'], total['agent_hours']))
    with open(args.summary, 'a', encoding='utf-8') as f:
        f.write('\n'.join(lines) + '\n')
