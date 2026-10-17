#!/usr/bin/env python3
"""Claude Code session usage from its transcript: tokens, cost at API prices, agent-hours and a ready-made row
for docs/usage.md.

Purpose (PLAN.md §9 and §10, step 8; docs/usage.md): `/cost` is a built-in UI command that the agent cannot
run, and ccusage requires Node.js. The script computes the same figures from the transcript
`~/.claude/projects/<project>/<session>.jsonl`: model responses are taken without duplicates (by message.id),
cost is computed at prices per 1M tokens (cache write: 5 minutes = 1.25× input, 1 hour = 2× input). If the
transcript contains `/cost` snapshots (cost-state records), the calculation is cross-checked against them. The cost is
what the tokens would cost at API prices: development runs on a Claude subscription, so the dollar figures are
hypothetical, not money spent. Agent-hours are the time from an owner's prompt to the agent's last action; pauses longer than 10 minutes are not counted.

Example
  python3 -I tools/usage/session_cost.py                       # the newest transcript is the current session
  python3 -I tools/usage/session_cost.py <path to .jsonl or session id>

Expected output: the transcript path; the cross-check against /cost snapshots; a per-model table (requests, tokens
in M, cost); time and the number of prompts; per-agent-hour figures to cross-check against the §9 model; a row for
docs/usage.md (the owner fills in the "Task" column). The transcript contains the session's conversation, so its
content is not printed and is not copied into the repository. Dependencies: standard library.
"""
import argparse, datetime as dt, glob, json, os, sys

# $ per 1M tokens: input, output, cache read, 5-minute cache write and 1-hour cache write.
# Source: PLAN.md §9.1 and the Claude API reference (cached 2026-10-06); verified against /cost for Opus 5.5.
PRICES = {
    'claude-opus-5-5': (4.0, 20.0, 0.20, 5.0, 8.0),
    'claude-sonnet-5-5': (2.0, 10.0, 0.20, 2.5, 4.0),
    'claude-fable-5-1': (10.0, 50.0, 0.25, 12.5, 20.0),
}
NAMES = {'claude-opus-5-5': 'Opus 5.5', 'claude-sonnet-5-5': 'Sonnet 5.5', 'claude-fable-5-1': 'Fable 5.1',
         'claude-haiku-5-5': 'Haiku 5.5'}
IDLE = dt.timedelta(minutes=10)
PROJECTS = os.path.join(os.path.expanduser('~'), '.claude', 'projects')

ap = argparse.ArgumentParser(description='Token usage, API-equivalent cost and agent hours of a Claude Code session.')
ap.add_argument('session', nargs='?', help='session .jsonl path or session id (default: newest transcript)')
args = ap.parse_args()


def find_transcript(arg):
    if arg and os.path.isfile(arg):
        return arg
    pattern = os.path.join(PROJECTS, '*', (arg or '*') + '.jsonl')
    found = sorted(glob.glob(pattern), key=os.path.getmtime)
    if not found:
        sys.exit('no transcript matches %s' % pattern)
    return found[-1]


def ts(s):
    return dt.datetime.fromisoformat(s.replace('Z', '+00:00'))


def is_prompt(d):
    """An owner's prompt, not a tool result or an injected meta message."""
    if d.get('type') != 'user' or d.get('isMeta') or d.get('isSidechain'):
        return False
    c = d.get('message', {}).get('content')
    if isinstance(c, str):
        return True
    return isinstance(c, list) and not any(isinstance(b, dict) and b.get('type') == 'tool_result' for b in c)


def split(u):
    """(input, output, cache read, 5 min write, 1 h write) from a response's usage."""
    cc = u.get('cache_creation') or {}
    w1h = cc.get('ephemeral_1h_input_tokens', 0)
    w5 = cc.get('ephemeral_5m_input_tokens', u.get('cache_creation_input_tokens', 0) - w1h)
    return (u.get('input_tokens', 0), u.get('output_tokens', 0), u.get('cache_read_input_tokens', 0), w5, w1h)


def cost(model, u):
    p = PRICES.get(model)
    return sum(n * price for n, price in zip(split(u), p)) / 1e6 if p else 0.0


path = find_transcript(args.session)
session_id = os.path.splitext(os.path.basename(path))[0]
# Subagent transcripts, if Claude Code writes them separately, are in a directory named after the session.
files = [path] + sorted(glob.glob(os.path.join(os.path.dirname(path), session_id, '**', '*.jsonl'), recursive=True))

responses, tool_uses, events, checks, last_snapshot = {}, set(), [], [], None
for i, f in enumerate(files):
    for line in open(f, encoding='utf-8'):
        d = json.loads(line)
        t = d.get('type')
        if t == 'cost-state' and i == 0:
            mine = sum(cost(m, u) for m, u in responses.values() if m == 'claude-opus-5-5')
            snap = d.get('modelUsage', {}).get('claude-opus-5-5', {}).get('costUSD')
            if snap is not None:
                checks.append(abs(mine - snap) < 0.01)
            last_snapshot = d
            continue
        if i == 0 and 'timestamp' in d and t in ('user', 'assistant'):
            events.append((ts(d['timestamp']), 'prompt' if is_prompt(d) else t))
        if t == 'assistant':
            m = d.get('message', {})
            for b in m.get('content') or []:
                if isinstance(b, dict) and b.get('type') == 'tool_use':
                    tool_uses.add(b['id'])
            if m.get('usage') and m.get('id'):
                responses[m['id']] = (m.get('model'), m['usage'])

print('transcript: %s%s' % (path, ' (+%d subagent files)' % (len(files) - 1) if len(files) > 1 else ''))
if checks:
    print('check against /cost snapshots: %d of %d match' % (sum(checks), len(checks)))

per_model = {}
for m, u in responses.values():
    row = per_model.setdefault(m, [0, 0, 0, 0, 0, 0, 0.0])
    row[0] += 1
    for k, n in enumerate(split(u)):
        row[k + 1] += n
    row[6] += cost(m, u)
print('\n%-20s %8s %8s %8s %11s %9s %9s %9s' % ('model', 'requests', 'input', 'output', 'cache read', 'write 5m', 'write 1h', 'cost $'))
for m, r in sorted(per_model.items(), key=lambda kv: -kv[1][6]):
    print('%-20s %8d %8.3f %8.3f %11.2f %9.3f %9.3f %9s' % (m, r[0], *(n / 1e6 for n in r[1:6]),
                                                           '%.2f' % r[6] if m in PRICES else 'n/a'))
total = sum(r[6] for r in per_model.values())
unpriced = [m for m in per_model if m not in PRICES]
if unpriced:
    print('not priced (add to PRICES):', ', '.join(unpriced))
side = {}
if last_snapshot:  # background calls (titles, WebFetch) do not reach the transcript but are in the /cost snapshot
    side = {m: v.get('costUSD', 0) for m, v in last_snapshot.get('modelUsage', {}).items() if m not in per_model}
    for m, c in side.items():
        print('side calls, last /cost snapshot: %s $%.2f' % (m, c))

events.sort()
start, end = events[0][0], events[-1][0]
active = dt.timedelta()
for (a, _), (b, kind) in zip(events, events[1:]):
    if kind != 'prompt':  # a pause before an owner's prompt is time spent waiting for the human
        active += min(b - a, IDLE)
hours, wall = active.total_seconds() / 3600, (end - start).total_seconds() / 3600
prompts = sum(1 for _, k in events if k == 'prompt')
print('\nwall clock %.2f h, agent active ≈%.2f h, prompts %d, tool calls %d' % (wall, hours, prompts, len(tool_uses)))
if hours:
    tot = [sum(r[k] for r in per_model.values()) for k in range(1, 6)]
    print('per agent-hour (PLAN.md §9.1 model: $8–18 Opus, output 0.2–0.5M, cache read 5–15M, write 0.5–1M):')
    print('  $%.1f, output %.2fM, cache read %.1fM, cache write %.2fM' % (total / hours, tot[1] / 1e6 / hours, tot[2] / 1e6 / hours, (tot[3] + tot[4]) / 1e6 / hours))


def num(x, nd):
    return '%.*f' % (nd, x)


main = max(per_model, key=lambda m: per_model[m][6])
r = per_model[main]
tokens = '%s / %s / read %s, write %s' % ('≈0' if r[1] < 5e4 else num(r[1] / 1e6, 2), num(r[2] / 1e6, 2),
                                          num(r[3] / 1e6, 1), num((r[4] + r[5]) / 1e6, 2))
extra = ''.join(' (+%s %s)' % (num(c, 2), NAMES.get(m, m)) for m, c in side.items() if c >= 0.005)
print('\nrow for docs/usage.md:')
print('| %s | <task> | %s | ≈%s (wall clock %s) | %s | %s%s |' % (
    start.astimezone().date().isoformat(), NAMES.get(main, main), num(hours, 1), num(wall, 1), tokens, num(total, 1), extra))
