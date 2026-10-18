#!/usr/bin/env python3
"""Claude Code usage of a pull request from the transcripts: tokens, cost at API prices and agent-hours, as a
block for the pull request's description (docs/usage.md).

Purpose (PLAN.md §9 and §10, step 8; docs/usage.md): every pull request carries what it cost. Claude Code records
the Git branch of every message in its transcripts (`~/.claude/projects/<project>/<session>.jsonl`, the subagents'
under `<session>/`), so a pull request's usage is that of the messages on its branch, in every session and worktree
of the repository. A message on `main` counts toward the session's next branch, where the task it reads or the
issue it opens is done, or, after the session's last branch, toward that branch. The branch Claude Code records is
the checkout's, so when another session in the same checkout switches it, a message keeps the branch its session was
on: a session moves to another branch only by a command of its own (`git switch`, `git checkout`, `git rebase`,
`gh pr checkout`, `gh pr merge`, entering or leaving a worktree), during it or within 30 seconds after it, as the
recorded branch follows a switch a few seconds late. Model responses are taken once
each (by message.id), and the cost is computed at prices per 1M tokens (cache write: 5 minutes = 1.25× input,
1 hour = 2× input). The cost is what the tokens would cost at API prices: development runs on a Claude
subscription, so the dollar figures are hypothetical, not money spent. Agent-hours are the time from an owner's
prompt to the agent's last action; pauses longer than 10 minutes are not counted.

Example
  python -I tools/usage/pr_usage.py 17            # print the usage of pull request #17
  python -I tools/usage/pr_usage.py 17 --update   # also write it into the description and refresh the badge

Expected output: the sessions that worked on the pull request; the block for its description (a per-model table of
requests, tokens in M and cost, the agent-hours, and a machine-readable comment that the Usage workflow sums for
the README's badge). With --update, `gh pr edit` replaces the block between its markers, or puts it before the
attribution line, and `gh workflow run usage.yml` refreshes the badge. The transcripts contain the sessions'
conversations, so their content is not printed and is not copied anywhere. Dependencies: standard library and
the GitHub CLI `gh`.
"""
import argparse, datetime as dt, glob, json, os, re, subprocess, sys

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
# Branches of no pull request: their messages count toward a neighbouring branch of the session.
NEUTRAL = {None, '', 'HEAD', 'main'}
# A session's own moves to another branch: shell commands, at the start of a command line or after a separator (not
# in an issue's text or a grep pattern), and tools; and how long after one the recorded branch may still change
# (Claude Code records the checkout's branch a few seconds late).
MOVES = re.compile(r'(?:^|[\n;&|(])\s*(?:git\s+(?:-C\s+\S+\s+)?(?:switch|checkout|rebase)|gh\s+pr\s+(?:checkout|merge))\b')
MOVING_TOOLS = {'EnterWorktree', 'ExitWorktree'}
GRACE = dt.timedelta(seconds=30)
BEGIN, END = '<!-- usage:begin -->', '<!-- usage:end -->'
ATTRIBUTION = '🤖 Generated with'
WORKFLOW = 'usage.yml'

ap = argparse.ArgumentParser(description='Claude Code usage of a pull request, from the transcripts of its branch.')
ap.add_argument('pr', type=int, help='pull request number')
ap.add_argument('--update', action='store_true',
                help='write the usage into the pull request description and refresh the badge')
args = ap.parse_args()


def run(argv, stdin=None):
    r = subprocess.run(argv, input=stdin, capture_output=True, text=True, encoding='utf-8')
    if r.returncode:
        sys.exit('%s: %s' % (' '.join(argv[:3]), r.stderr.strip()))
    return r.stdout


def project_dirs():
    """The transcript directories of this repository and of its worktrees (.claude/worktrees/): Claude Code
    names them after the working directory, every character other than a letter or digit replaced by '-'. They are
    found from the main checkout, the parent of the Git directory that every worktree shares, so a run in a worktree
    finds the same ones (its --show-toplevel is the worktree's own path)."""
    common = run(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir']).strip()
    top = os.path.dirname(os.path.abspath(common))
    name = re.sub(r'[^A-Za-z0-9]', '-', top).lower()
    return [d for d in sorted(glob.glob(os.path.join(PROJECTS, '*'))) if os.path.isdir(d)
            and (os.path.basename(d).lower() == name or os.path.basename(d).lower().startswith(name + '--claude-'))]


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


def records(path):
    for line in open(path, encoding='utf-8'):
        try:
            yield json.loads(line)
        except ValueError:  # the last line of a transcript being written
            continue


def moves(block):
    """Whether a content block is a tool call that moves the session's checkout to another branch."""
    if not isinstance(block, dict) or block.get('type') != 'tool_use':
        return False
    command = (block.get('input') or {}).get('command')
    return block.get('name') in MOVING_TOOLS or isinstance(command, str) and bool(MOVES.search(command))


def own_branches(rows, windows, own=None):
    """The branch a transcript's session is on at each of its messages (time, recorded branch), from the branch
    it starts on (own, else its first recorded one): a change of the recorded branch is the session's own during one
    of its moving calls or within GRACE after it (windows), and otherwise another session's in the same checkout,
    which the session does not follow."""
    out = []
    for t, branch in rows:
        if branch is not None and (own is None or branch != own and any(a <= t <= b for a, b in windows)):
            own = branch
        out.append(own)
    return out


def session_events(path, seen):
    """A session's messages in time order: (time, branch, kind, model, usage, from the main transcript).
    A response spans a line per content block, each with the response's usage, which is taken from the first;
    messages already seen in another transcript (a resumed session repeats its history) are skipped. The branch is
    the one the session itself is on (own_branches), followed in each transcript apart: a subagent in the session's
    checkout starts on the session's branch, one in a worktree of its own on its worktree's."""
    sid = os.path.splitext(os.path.basename(path))[0]
    sub = sorted(glob.glob(os.path.join(os.path.dirname(path), sid, '**', '*.jsonl'), recursive=True))
    events, session, checkout = [], [], None
    for i, f in enumerate([path] + sub):
        rows, calls, windows, cwd = [], {}, [], None
        for d in records(f):
            t = d.get('type')
            if t not in ('user', 'assistant') or 'timestamp' not in d:
                continue
            # The same directory spelt with another case of the drive letter (VS Code's c:\ and C:\).
            cwd = cwd or d.get('cwd') and os.path.normcase(os.path.normpath(d['cwd']))
            content = d.get('message', {}).get('content')
            for block in content if isinstance(content, list) else ():
                if moves(block) and block.get('id'):
                    calls[block['id']] = ts(d['timestamp'])
                elif isinstance(block, dict) and block.get('type') == 'tool_result' \
                        and block.get('tool_use_id') in calls:
                    windows.append((calls.pop(block['tool_use_id']), ts(d['timestamp']) + GRACE))
            if not d.get('uuid') or d['uuid'] in seen:
                continue
            seen.add(d['uuid'])
            m = d.get('message', {})
            usage = None
            # '<synthetic>' marks the messages that Claude Code writes itself, such as an interruption.
            if t == 'assistant' and m.get('usage') and m.get('id') and m['id'] not in seen \
                    and m.get('model') != '<synthetic>':
                seen.add(m['id'])
                usage = m['usage']
            kind = 'prompt' if is_prompt(d) else t
            rows.append((ts(d['timestamp']), d.get('gitBranch'), kind, m.get('model'), usage, i == 0))
        rows.sort(key=lambda e: e[0])
        # A moving call without its result yet (the transcript is still being written) moves the session until now.
        windows += [(a, dt.datetime.now(dt.timezone.utc)) for a in calls.values()]
        start = None
        if i == 0:
            checkout = cwd
        elif rows and cwd == checkout:
            start = next((b for t, b in reversed(session) if t <= rows[0][0]), None)
        branches = own_branches([r[:2] for r in rows], windows, start)
        if i == 0:
            session = [(r[0], b) for r, b in zip(rows, branches)]
        for row, branch in zip(rows, branches):
            events.append((row[0], branch) + row[2:])
    events.sort(key=lambda e: e[0])
    return events


def owners(events):
    """The branch each event counts toward: its own, or for one on a neutral branch the session's next other
    branch, failing that its previous one."""
    out, nxt = [None] * len(events), None
    for i in reversed(range(len(events))):
        if events[i][1] not in NEUTRAL:
            nxt = events[i][1]
        out[i] = nxt
    prev = None
    for i, e in enumerate(events):
        if e[1] not in NEUTRAL:
            prev = e[1]
        elif out[i] is None:
            out[i] = prev
    return out


pr = json.loads(run(['gh', 'pr', 'view', str(args.pr), '--json', 'headRefName,body,url']))
branch, url = pr['headRefName'], pr['url']
repo_url = url.rsplit('/pull/', 1)[0]

seen, per_model, sessions, active, wall = set(), {}, [], dt.timedelta(), dt.timedelta()
for d in project_dirs():
    for path in sorted(glob.glob(os.path.join(d, '*.jsonl')), key=os.path.getmtime):
        events = session_events(path, seen)
        own = owners(events)
        mine = [e for e, o in zip(events, own) if o == branch]
        if not mine:
            continue
        sessions.append((os.path.basename(path)[:8], len(mine)))
        for _, _, kind, model, usage, _ in mine:
            if kind == 'assistant' and usage:
                row = per_model.setdefault(model, [0, 0, 0, 0, 0, 0, 0.0])
                row[0] += 1
                for k, n in enumerate(split(usage)):
                    row[k + 1] += n
                row[6] += cost(model, usage)
        timeline = [(e, o) for e, o in zip(events, own) if e[5]]  # time counts in the main transcript only
        for (a, _), (b, o) in zip(timeline, timeline[1:]):
            if o == branch and b[2] != 'prompt':  # a pause before an owner's prompt is time spent waiting for the human
                active += min(b[0] - a[0], IDLE)
        times = [e[0] for e, o in timeline if o == branch]
        if times:
            wall += times[-1] - times[0]

if not per_model:
    sys.exit('no transcript of this repository has responses on branch %s' % branch)

print('pull request #%d, branch %s' % (args.pr, branch))
print('sessions: %s' % ', '.join('%s (%d messages)' % s for s in sessions))

hours, wall_hours = active.total_seconds() / 3600, wall.total_seconds() / 3600
total = [sum(r[k] for r in per_model.values()) for k in range(1, 6)]
cost_usd = sum(r[6] for r in per_model.values())
data = {'input': total[0], 'output': total[1], 'cache_read': total[2], 'cache_write': total[3] + total[4],
        'cost_usd': round(cost_usd, 2), 'agent_hours': round(hours, 2), 'wall_hours': round(wall_hours, 2),
        'sessions': len(sessions)}

lines = [BEGIN, '## Claude Code usage', '',
         '| Model | Requests | Input, M | Output, M | Cache read, M | Cache write, M | At API prices, $ |',
         '|---|---|---|---|---|---|---|']
for m, r in sorted(per_model.items(), key=lambda kv: -kv[1][6]):
    lines.append('| %s | %d | %.3f | %.3f | %.2f | %.3f | %s |' % (
        NAMES.get(m, m), r[0], r[1] / 1e6, r[2] / 1e6, r[3] / 1e6, (r[4] + r[5]) / 1e6,
        '%.2f' % r[6] if m in PRICES else 'not priced'))
lines += ['', '≈%.1f agent-hours (wall clock %.1f) in %d session%s. The dollars are the cost of the same tokens at '
          'API prices, not money spent: development runs on a Claude subscription ([docs/usage.md](%s/blob/main/'
          'docs/usage.md)).' % (hours, wall_hours, len(sessions), '' if len(sessions) == 1 else 's', repo_url),
          '<!-- usage %s -->' % json.dumps(data, separators=(', ', ': ')), END]
block = '\n'.join(lines)
print('\n' + block)

if args.update:
    body = pr['body'] or ''
    if BEGIN in body and END in body[body.index(BEGIN):]:
        start = body.index(BEGIN)
        body = body[:start] + block + body[body.index(END, start) + len(END):]
    elif ATTRIBUTION in body:
        i = body.rindex(ATTRIBUTION)
        body = body[:i] + block + '\n\n' + body[i:]
    else:
        body = body.rstrip() + '\n\n' + block + '\n'
    run(['gh', 'pr', 'edit', str(args.pr), '--body-file', '-'], stdin=body)
    print('\nwritten into %s' % url)
    # The badge sums every pull request's block; the workflow runs from main, so it is missing on a branch
    # that adds it.
    r = subprocess.run(['gh', 'workflow', 'run', WORKFLOW], capture_output=True, text=True, encoding='utf-8')
    print('badge: %s' % ('refreshing (the Usage workflow)' if r.returncode == 0
                         else 'not refreshed: %s' % r.stderr.strip()))
