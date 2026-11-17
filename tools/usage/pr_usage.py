#!/usr/bin/env python3
"""Claude Code usage of a pull request from the transcripts: tokens, cost at API prices and agent-hours, as a
block for the pull request's description (docs/usage.md).

Purpose (PLAN.md §9; docs/usage.md): every pull request carries what it cost. Claude Code records
the Git branch of every message in its transcripts (`~/.claude/projects/<project>/<session>.jsonl`, the subagents'
under `<session>/`), so a pull request's usage is that of the messages on its branch, in every session and worktree
of the repository. A message on `main` counts toward the session's next branch, where the task it reads or the
issue it opens is done, or, after the session's last branch, toward that branch. The branch Claude Code records is
the checkout's, so when another session in the same checkout switches it, a message keeps the branch its session was
on: a session moves to another branch only by a command of its own (`git switch`, `git checkout`, `git rebase`,
`gh pr checkout`, `gh pr merge`, entering or leaving a worktree), during it or within 30 seconds after it, as the
recorded branch follows a switch a few seconds late. A session that `cd`s into a linked worktree keeps recording its
own checkout's branch, so a message in a linked worktree has the worktree's branch: the latest the session gave it
(`git worktree add -b`, `EnterWorktree`, a `git switch` or `git checkout` run in it), else the one it is on now, else
the one its directory is named after as `claim.py` names them, for a worktree removed since. A branch given under a
name that the command does not tell (`bug/$(gh issue create …)-slug`) is not known, and the worktree's messages from
then on go by the rest of that list. So does a response whose tool calls work in a linked worktree from elsewhere (a
`cd` or `git -C` into it, a file tool on a path in it), with their results. A branch the session renamed
(`git branch -m`) counts under its new name; one whose work was pushed under another name is named with --also, and
the block keeps that name for the runs after. Model responses are taken once each (by message.id), and the cost is
computed at prices per 1M tokens (cache write: 5 minutes = 1.25× input, 1 hour = 2× input), each model at its own
(PRICES). The cost is what the tokens would cost at API prices: development runs on a Claude subscription, so the
dollar figures are hypothetical, not money spent. Agent-hours are the time from an owner's prompt to the agent's
last action; a pause longer than 10 minutes counts as 10.

Example
  uv run python -I tools/usage/pr_usage.py 17            # print the usage of pull request #17
  uv run python -I tools/usage/pr_usage.py 17 --update   # also write it into the description and refresh the badge
  uv run python -I tools/usage/pr_usage.py 17 21 34 --update   # several, in one pass over the transcripts
  uv run python -I tools/usage/pr_usage.py 17 --also wip-lights --update   # with the messages of another branch name
  uv run python -I -m unittest discover -s tools/usage -p "test_*.py"   # the tests, on synthetic transcripts

Expected output: for each pull request, the sessions that worked on it and the block for its description (a
per-model table of requests, tokens in M and cost, the agent-hours, and a machine-readable comment that the Usage
workflow sums for the README's badge). With --update, `gh pr edit` replaces the block between its markers, or puts
it before the attribution line, and `gh workflow run usage.yml` refreshes the badge once. A block that would hold
fewer tokens than the one in the description is not written without --lower: Claude Code keeps a transcript 30
days, and a branch's messages count elsewhere once its worktree is on another branch. A response of a model
without a price is listed as "not priced", its tokens counted and its dollars not. The run ends with an error that
names each pull request without responses or left unwritten, and each model without a price. The transcripts
contain the sessions' conversations, so their content is not printed and is not copied anywhere. Dependencies:
standard library and the GitHub CLI `gh`.
"""
import argparse, datetime as dt, glob, json, os, re, subprocess, sys

# $ per 1M tokens: input, output, cache read, 5-minute cache write and 1-hour cache write.
# Source: the Claude API's pricing page (platform.claude.com/docs/en/about-claude/pricing, read 2026-11-17);
# verified against /cost for Opus 5.5. A model that the sessions run on and that has no row here fails the run
# (main), so that its row is added with its first pull request.
PRICES = {
    'claude-opus-5-5': (4.0, 20.0, 0.20, 5.0, 8.0),
    'claude-sonnet-5-5': (2.0, 10.0, 0.10, 2.5, 4.0),
    'claude-haiku-5-5': (0.10, 0.50, 0.01, 0.125, 0.20),
    'claude-fable-5-1': (10.0, 50.0, 0.25, 12.5, 20.0),
    'claude-fable-5-2': (8.0, 40.0, 0.20, 10.0, 16.0),
    'claude-mythos-5-2': (8.0, 40.0, 0.20, 10.0, 16.0),
}
# The models priced by the prompt's length: the tokens of a request's prompt (its input, the cache's reads and
# writes among them) above which the request pays the prices beside them, each request on its own.
LONG_PROMPT = {'claude-haiku-5-5': (100_000, (0.50, 2.50, 0.05, 0.625, 1.0))}
NAMES = {'claude-opus-5-5': 'Opus 5.5', 'claude-sonnet-5-5': 'Sonnet 5.5', 'claude-haiku-5-5': 'Haiku 5.5',
         'claude-fable-5-1': 'Fable 5.1', 'claude-fable-5-2': 'Fable 5.2', 'claude-mythos-5-2': 'Mythos 5.2'}
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
# A session's own commands that give a worktree its branch or rename a branch, and what EnterWorktree reports.
ADD = re.compile(r'(?:^|[\n;&|(])\s*git\s+(?:-C\s+(\S+)\s+)?worktree\s+add\s+([^\n;&|]*)')
SWITCH = re.compile(r'(?:^|[\n;&|(])\s*git\s+(?:-C\s+(\S+)\s+)?(switch|checkout)\s+([^\n;&|]*)')
# The options of a switch that name the branch it creates.
CREATE = {'switch': ('-c', '-C', '--create', '--force-create', '--orphan'), 'checkout': ('-b', '-B', '--orphan')}
RENAME = re.compile(r'(?:^|[\n;&|(])\s*git\s+(?:-C\s+(\S+)\s+)?branch\s+(?:-[mM]|--move)\s+([^\n;&|]*)')
ENTERED = re.compile(r'(?:Created|Entered) worktree at (.+?) on branch (\S+?)\.(?:\s|$)')
# Where a shell command works: a change of directory or `git -C`; and the variables it assigns itself (Bash's
# NAME=value, PowerShell's $name = value) and uses.
WORKS_IN = re.compile(r'(?:^|[\n;&|(])\s*(?:cd|pushd|Set-Location|Push-Location)\s+(?:-(?:Literal)?Path\s+)?'
                      r'("[^"]*"|\'[^\']*\'|[^\s;&|)]+)|\bgit\s+-C\s+("[^"]*"|\'[^\']*\'|\S+)', re.I)
ASSIGN = re.compile(r'(?:^|[\n;&|(])\s*(?:\$(\w+)\s*=\s*|([A-Za-z_]\w*)=)("[^"\n]*"|\'[^\'\n]*\'|[^\s;&|]+)')
VARIABLE = re.compile(r'\$(?:\{(\w+)\}|(\w+))')
FILE_INPUTS = ('file_path', 'notebook_path', 'path')
BEGIN, END = '<!-- usage:begin -->', '<!-- usage:end -->'
FIGURES = re.compile(r'<!-- usage (\{.*?\}) -->')
ATTRIBUTION = '🤖 Generated with'
WORKFLOW = 'usage.yml'

def run(argv, stdin=None):
    """A command's output. Its input goes to it as it is, in UTF-8: a pipe in text mode writes every line end as
    CRLF on Windows, which added a carriage return to each line of a description on every run (#2853)."""
    r = subprocess.run(argv, input=None if stdin is None else stdin.encode('utf-8'), capture_output=True)
    if r.returncode:
        sys.exit('%s: %s' % (' '.join(argv[:3]), r.stderr.decode('utf-8', 'replace').strip()))
    return r.stdout.decode('utf-8')


def unix(text):
    """A text with LF line ends: the carriage returns before a line end dropped, however many the runs of
    before #2853 left there."""
    return re.sub(r'\r+\n', '\n', text)


def main_checkout():
    """The main checkout, the parent of the Git directory that every worktree shares, so that a run in a worktree
    sees the same repository (its --show-toplevel is the worktree's own path)."""
    common = run(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir']).strip()
    return os.path.dirname(os.path.abspath(common))


def project_dirs(top):
    """The transcript directories of this repository and of its worktrees (.claude/worktrees/): Claude Code
    names them after the working directory, every character other than a letter or digit replaced by '-'."""
    name = re.sub(r'[^A-Za-z0-9]', '-', top).lower()
    return [d for d in sorted(glob.glob(os.path.join(PROJECTS, '*'))) if os.path.isdir(d)
            and (os.path.basename(d).lower() == name or os.path.basename(d).lower().startswith(name + '--claude-'))]


def native(path):
    """A path as the system spells it: Git Bash's /c/... is C:/... on Windows."""
    return re.sub(r'^/([A-Za-z])(?=/|$)', r'\1:', path) if os.name == 'nt' else path


def norm(path):
    # The same directory spelt with another case of the drive letter (VS Code's c:\ and C:\) or other separators.
    return os.path.normcase(os.path.normpath(native(path)))


def linked_worktrees(top):
    """The linked worktrees of the repository that exist now: {directory: the branch it is on}."""
    out, path = {}, None
    for line in run(['git', 'worktree', 'list', '--porcelain']).splitlines():
        if line.startswith('worktree '):
            path = norm(line[len('worktree '):])
        elif line.startswith('branch refs/heads/') and path != norm(top):
            out[path] = line[len('branch refs/heads/'):]
    return out


def repository_branches():
    """The names of the repository's local and remote-tracking branches that exist now."""
    out = set()
    for ref in run(['git', 'for-each-ref', '--format=%(refname)', 'refs/heads', 'refs/remotes']).splitlines():
        name = ref[len('refs/heads/'):] if ref.startswith('refs/heads/') else ref.split('/', 3)[-1]
        if name != 'HEAD':  # refs/remotes/origin/HEAD
            out.add(name)
    return out


def words(args):
    """A shell command's arguments up to a redirection, without their quotes."""
    out = []
    for w in re.findall(r'"[^"]*"|\'[^\']*\'|\S+', args):
        if re.match(r'\d*[<>]', w):
            break
        out.append(w.strip('\'"'))
    return out


def plain(name):
    """Whether a branch's name in a command is the name itself: no variable or substitution is left in it, whose
    value the transcript does not tell (`bug/$(gh issue create …)-slug`)."""
    return not re.search(r'[$`()]', name)


def located(command, m, cwd, path=''):
    """The directory a git command works in, or that a path it names is (m: a match of ADD, SWITCH or RENAME, whose
    first group is its -C path), in a shell command run in cwd: after the changes of directory earlier in the
    command, then its -C path; None when a shell variable is not resolved or the place is not known (`cd -`, `~`)."""
    parts = [c[1] for c in WORKS_IN.finditer(command, 0, m.start()) if c[1]] + [m[1] or '', path]
    parts = [native(p.strip('\'"')) for p in parts]
    parts = parts[max((i for i, p in enumerate(parts) if os.path.isabs(p)), default=0):]  # from the last absolute one
    if any('$' in p or p == '-' or p.startswith('~') for p in parts):
        return None
    return norm(os.path.join(cwd, *parts))


def worktree_adds(command, cwd):
    """(worktree, branch) of each `git worktree add` with -b or -B in a shell command run in cwd; the branch is
    None where the command does not tell its name (plain)."""
    out = []
    for m in ADD.finditer(command):
        branch, paths, it = None, [], iter(words(m[2]))
        for w in it:
            if w in ('-b', '-B'):
                branch = next(it, None)
            elif w == '--reason':
                next(it, None)
            elif not w.startswith('-'):
                paths.append(w)
        place = branch and paths and located(command, m, cwd, paths[0])
        if place:
            out.append((place, branch if plain(branch) else None))
    return out


def branch_moves(command, cwd):
    """(directory, branch, certain) of each `git switch` or `git checkout` in a shell command run in cwd that moves
    its checkout to a named branch. A `git checkout <name>` without -b may restore a path instead: not certain.
    The branch is None where a certain move does not tell its name (plain): the checkout is on another branch
    from then on, and the command does not say which."""
    out = []
    for m in SWITCH.finditer(command):
        verb, branch, names, restores, it = m[2], None, [], False, iter(words(m[3]))
        for w in it:
            option, _, value = w.partition('=')
            if option in CREATE[verb]:
                branch = value or next(it, None)
            elif w in ('--', '--detach') or verb == 'switch' and w == '-d':  # paths follow, or a commit
                restores = True
                break
            elif not w.startswith('-'):
                names.append(w)
        certain = bool(branch) or verb == 'switch'
        if not branch and not restores and len(names) == 1 and names[0] != '-':  # `-` is the previous branch
            branch = names[0]
        told = bool(branch) and plain(branch)
        place = branch and (told or certain) and located(command, m, cwd)
        if place:
            out.append((place, branch if told else None, certain))
    return out


def branch_renames(command, cwd):
    """(directory, old name or None for its current branch, new name) of each `git branch -m` in a shell
    command run in cwd."""
    out = []
    for m in RENAME.finditer(command):
        names = [w for w in words(m[2]) if not w.startswith('-')]
        place = 1 <= len(names) <= 2 and all(map(plain, names)) and located(command, m, cwd)
        if place:
            old, new = names if len(names) == 2 else (None, names[0])
            out.append((place, old, new))
    return out


def result_text(block):
    c = block.get('content')
    return c if isinstance(c, str) else ' '.join(b.get('text', '') for b in c or () if isinstance(b, dict))


def expand(command):
    """A shell command with the variables it assigns itself replaced by their values (W="…", $w = '…'), so that
    the paths it works on can be read; other variables stay as they are. A value is taken where the command
    writes it out: not a command's output (`N=$(gh issue create …)`, and in PowerShell a word without quotes,
    which is a command or an expression there, a number apart), which the transcript does not tell."""
    values = {}

    def value(m):
        return values.get((m[1] or m[2]).lower(), m[0])

    for ps, sh, v in ASSIGN.findall(command):
        written = '$(' not in v and '`' not in v and (not ps or v[0] in '\'"' or v.isdigit())
        if written:
            values[(ps or sh).lower()] = VARIABLE.sub(value, v.strip('\'"'))
        else:
            values.pop((ps or sh).lower(), None)
    return VARIABLE.sub(value, command) if values else command


def worked_in(block, cwd):
    """The directories a tool call run in cwd works in: a shell command's changes of directory, its `git -C` and
    the worktrees it adds, and a file tool's path."""
    inp = block.get('input') or {}
    out = []
    if isinstance(inp.get('command'), str):
        command = expand(inp['command'])
        out += [(a or b).strip('\'"') for a, b in WORKS_IN.findall(command)]
        out += [w for w, _ in worktree_adds(command, cwd)]
    out += [inp[k] for k in FILE_INPUTS if isinstance(inp.get(k), str)]
    return [norm(os.path.join(cwd, native(d))) for d in out if '$' not in d]


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
    """A response's cost in dollars at API prices, from its usage; 0 for a model without a price."""
    p = PRICES.get(model)
    if not p:
        return 0.0
    tokens = split(u)
    limit, above = LONG_PROMPT.get(model, (None, p))
    if limit is not None and tokens[0] + sum(tokens[2:]) > limit:
        p = above
    return sum(n * price for n, price in zip(tokens, p)) / 1e6


def records(path):
    with open(path, encoding='utf-8') as f:
        for line in f:
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


def in_worktree(place, known, top):
    """The linked worktree a directory lies in: one of the known ones, or one under the main checkout's
    .claude/worktrees/."""
    for w in known:
        if place == w or place.startswith(w + os.sep):
            return w
    base = os.path.join(norm(top), '.claude', 'worktrees') + os.sep
    return base + place[len(base):].split(os.sep)[0] if place.startswith(base) else None


def named_after(branches):
    """The branch that each worktree's directory is named after, as `claim.py` names them (the worktree
    `<N>-<slug>` of the branch `<type>/<N>-<slug>`), among the branches given: {the directory's name in lower
    case: the branch}. A name that two of the branches end in is neither's."""
    ending = {}
    for b in branches:
        tail = b.partition('/')[2].lower()
        if tail:
            ending.setdefault(tail, set()).add(b)
    return {tail: next(iter(found)) for tail, found in ending.items() if len(found) == 1}


def session_events(path, seen, top, linked, existing=()):
    """A session's messages in time order: (time, branch, kind, model, usage, from the main transcript).
    A response spans a line per content block, each with the response's usage, which is taken from the first;
    messages already seen in another transcript (a resumed session repeats its history) are skipped. The branch is
    the one the session itself is on (own_branches), followed in each transcript apart: a subagent in the session's
    checkout starts on the session's branch, one in a worktree of its own on its worktree's. In a linked worktree it
    is the worktree's, since a session that `cd`s into one keeps recording its own checkout's branch: the latest the
    session gave the worktree by then (`git worktree add -b`, EnterWorktree, or a `git switch` or `git checkout` run
    in it, from the first line of its response on), else the one it is on now (linked), else the recorded one unless
    neutral, else the one of the branches that exist now (existing) that its directory is named after (named_after:
    a worktree of `claim.py`, which the session was given and has removed since), else the worktree itself, which no
    pull request is on. A branch that the session gave it under a name its command does not tell (a substitution)
    is not known: from then on the worktree is on the one it is on now, and so on. A `git checkout <name>` without
    -b gives a branch only when the name is one: of the branches that exist now (existing), or one the session
    recorded or gave a worktree. A response whose tool calls work in one linked worktree from elsewhere (worked_in),
    and their results, are in that worktree. A branch the session renamed counts under its new name."""
    sid = os.path.splitext(os.path.basename(path))[0]
    sub = sorted(glob.glob(os.path.join(os.path.dirname(path), sid, '**', '*.jsonl'), recursive=True))
    files, given, switched, renamed = [], [], [], []
    for f in [path] + sub:
        rows, calls, entering, windows, cwd = [], {}, set(), [], None
        called, responding = {}, {}  # the directories each tool call and each response works in
        begun = {}  # the time of each response's first line
        for d in records(f):
            t = d.get('type')
            if t not in ('user', 'assistant') or 'timestamp' not in d:
                continue
            when, here = ts(d['timestamp']), d.get('cwd') and norm(d['cwd'])
            cwd = cwd or here
            m = d.get('message', {})
            content, results = m.get('content'), []
            if t == 'assistant' and m.get('id'):
                begun.setdefault(m['id'], when)
            for block in content if isinstance(content, list) else ():
                if not isinstance(block, dict):
                    continue
                if block.get('type') == 'tool_use' and here:
                    dirs = called[block.get('id')] = worked_in(block, here)
                    if m.get('id'):
                        responding.setdefault(m['id'], []).extend(dirs)
                    command = (block.get('input') or {}).get('command')
                    if isinstance(command, str):
                        command = expand(command)
                        given += [(when, w, b) for w, b in worktree_adds(command, here)]
                        switched += [(begun.get(m.get('id'), when),) + s for s in branch_moves(command, here)]
                        renamed += [(when, len(files)) + r for r in branch_renames(command, here)]
                elif block.get('type') == 'tool_result':
                    results.append(block.get('tool_use_id'))
                if moves(block) and block.get('id'):
                    calls[block['id']] = when
                    if block.get('name') == 'EnterWorktree':
                        entering.add(block['id'])
                elif block.get('type') == 'tool_result' and block.get('tool_use_id') in calls:
                    windows.append((calls.pop(block['tool_use_id']), when + GRACE))
                    entered = block['tool_use_id'] in entering and ENTERED.search(result_text(block))
                    if entered:
                        given.append((when, norm(entered[1]), entered[2]))
            if not d.get('uuid') or d['uuid'] in seen:
                continue
            seen.add(d['uuid'])
            usage = None
            # '<synthetic>' marks the messages that Claude Code writes itself, such as an interruption.
            if t == 'assistant' and m.get('usage') and m.get('id') and m['id'] not in seen \
                    and m.get('model') != '<synthetic>':
                seen.add(m['id'])
                usage = m['usage']
            kind = 'prompt' if is_prompt(d) else t
            rows.append((when, d.get('gitBranch'), kind, m.get('model'), usage, not files, here,
                         m.get('id') if t == 'assistant' else None, results))
        # A response works where its tool calls do, its first line (which carries the usage) included, and a tool
        # result where its call does.
        rows = [r[:7] + (responding.get(r[7]) or [p for c in r[8] for p in called.get(c, ())],) for r in rows]
        rows.sort(key=lambda e: e[0])
        # A moving call without its result yet (the transcript is still being written) moves the session until now.
        windows += [(a, dt.datetime.now(dt.timezone.utc)) for a in calls.values()]
        files.append((rows, windows, cwd))
    known = sorted(set(linked) | {w for _, w, _ in given}, key=len, reverse=True)
    named = set(existing) | {b for _, _, b in given if b} | {r[1] for rows, _, _ in files for r in rows}
    for when, place, b, certain in switched:
        w = in_worktree(place, known, top)
        if w and (certain or b in named):
            given.append((when, w, b))
    given.sort(key=lambda e: e[0])
    after = named_after(existing)

    def branch(place, dirs, when, recorded, own):
        w = place and in_worktree(place, known, top)
        worked = {in_worktree(p, known, top) for p in dirs} - {None}
        if len(worked) == 1 and w not in worked:  # from elsewhere: the recorded branch is the checkout's
            w, recorded = worked.pop(), None
        if not w:
            return own
        # Before the first branch the session gave it, the response that gives it (whose first line comes first).
        ties = [(t, g) for t, p, g in given if p == w]
        # A branch whose name the command did not tell (None) leaves the worktree to what is known of it now.
        b = next((g for t, g in reversed(ties) if t <= when), ties[0][1] if ties else None) or linked.get(w)
        if b or recorded not in NEUTRAL:
            return b or recorded
        return after.get(os.path.basename(w).lower()) or 'worktree ' + w

    session, checkout = [], files[0][2]
    for i, (rows, windows, cwd) in enumerate(files):
        start = None
        if i and rows and cwd == checkout:
            start = next((b for t, b in reversed(session) if t <= rows[0][0]), None)
        own = own_branches([r[:2] for r in rows], windows, start)
        branches = [branch(r[6], r[7], r[0], r[1], b) for r, b in zip(rows, own)]
        if i == 0:
            session = [(r[0], b) for r, b in zip(rows, branches)]
        files[i] = rows, branches
    names = {}
    for when, i, place, old, new in sorted(renamed, key=lambda e: e[0]):
        if old is None:  # the branch of the directory it ran in
            rows, branches = files[i]
            old = branch(place, (), when, None, next((b for r, b in zip(reversed(rows), reversed(branches))
                                                     if r[0] <= when), None))
        if old not in NEUTRAL and old != new:
            names = {k: new if v == old else v for k, v in names.items()}
            names[old] = new
    events = [(r[0], names.get(b, b)) + r[2:6] for rows, branches in files for r, b in zip(rows, branches)]
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


class Counted:
    """What the transcripts count toward a pull request: per model its requests, its tokens of the five kinds and
    their dollars; the sessions (id, messages); the agent's time and the wall clock."""

    def __init__(self):
        self.per_model, self.sessions, self.active, self.wall = {}, [], dt.timedelta(), dt.timedelta()


def count(wanted, top, linked, existing):
    """The usage of each of several pull requests in one pass over the repository's transcripts: wanted holds the
    names of each one's branches, and the answer a Counted for each, in that order."""
    out, of = [Counted() for _ in wanted], {}
    for i, names in enumerate(wanted):
        for name in names:
            of.setdefault(name, []).append(i)
    seen = set()
    for d in project_dirs(top):
        for path in sorted(glob.glob(os.path.join(d, '*.jsonl')), key=os.path.getmtime):
            events = session_events(path, seen, top, linked, existing)
            messages, span, before = {}, {}, None
            for e, owner in zip(events, owners(events)):
                mine = of.get(owner, ())
                for i in mine:
                    messages[i] = messages.get(i, 0) + 1
                    if e[2] == 'assistant' and e[4]:
                        row = out[i].per_model.setdefault(e[3], [0, 0, 0, 0, 0, 0, 0.0])
                        row[0] += 1
                        for k, n in enumerate(split(e[4])):
                            row[k + 1] += n
                        row[6] += cost(e[3], e[4])
                if not e[5]:  # time counts in the main transcript only
                    continue
                for i in mine:
                    # A pause before an owner's prompt is time spent waiting for the human.
                    if before is not None and e[2] != 'prompt':
                        out[i].active += min(e[0] - before, IDLE)
                    span[i] = (span.get(i, (e[0],))[0], e[0])
                before = e[0]
            for i, n in messages.items():
                out[i].sessions.append((os.path.basename(path)[:8], n))
            for i, (first, last) in span.items():
                out[i].wall += last - first
    return out


def figures(body):
    """The figures of the usage block in a pull request's description; None without a block that reads."""
    found = FIGURES.findall(body or '')
    try:
        data = json.loads(found[-1]) if found else None
    except ValueError:
        return None
    return data if isinstance(data, dict) else None


def tokens(data):
    """The tokens of a block's figures, every kind together."""
    return sum(data.get(k) or 0 for k in ('input', 'output', 'cache_read', 'cache_write'))


def block_of(counted, also, repo_url):
    """The block for a pull request's description and its figures, from what the transcripts count toward it and
    the other names of its branch that were counted with it."""
    per_model, sessions = counted.per_model, counted.sessions
    hours, wall_hours = counted.active.total_seconds() / 3600, counted.wall.total_seconds() / 3600
    total = [sum(r[k] for r in per_model.values()) for k in range(1, 6)]
    cost_usd = sum(r[6] for r in per_model.values())
    data = {'input': total[0], 'output': total[1], 'cache_read': total[2], 'cache_write': total[3] + total[4],
            'cost_usd': round(cost_usd, 2), 'agent_hours': round(hours, 2), 'wall_hours': round(wall_hours, 2),
            'sessions': len(sessions)}
    if also:
        data['also'] = also

    lines = [BEGIN, '## Claude Code usage', '',
             '| Model | Requests | Input, M | Output, M | Cache read, M | Cache write, M | At API prices, $ |',
             '|---|---|---|---|---|---|---|']
    for m, r in sorted(per_model.items(), key=lambda kv: -kv[1][6]):
        lines.append('| %s | %d | %.3f | %.3f | %.2f | %.3f | %s |' % (
            NAMES.get(m, m), r[0], r[1] / 1e6, r[2] / 1e6, r[3] / 1e6, (r[4] + r[5]) / 1e6,
            '%.2f' % r[6] if m in PRICES else 'not priced'))
    lines += ['', '≈%.1f agent-hours (wall clock %.1f) in %d session%s. The dollars are the cost of the same tokens '
              'at API prices, not money spent: development runs on a Claude subscription ([docs/usage.md](%s/blob/'
              'main/docs/usage.md)).' % (hours, wall_hours, len(sessions), '' if len(sessions) == 1 else 's', repo_url)]
    if also:
        lines[-1] += ' The work done under the name%s %s counts here too.' % (
            '' if len(also) == 1 else 's', ', '.join('`%s`' % a for a in also))
    lines += ['<!-- usage %s -->' % json.dumps(data, separators=(', ', ': ')), END]
    return '\n'.join(lines), data


def spliced(body, block):
    """A pull request's description with the usage block: in the place of the one it has, else before the
    attribution line, else at its end."""
    body = body or ''
    if BEGIN in body and END in body[body.index(BEGIN):]:
        start = body.index(BEGIN)
        return body[:start] + block + body[body.index(END, start) + len(END):]
    if ATTRIBUTION in body:
        i = body.rindex(ATTRIBUTION)
        return body[:i] + block + '\n\n' + body[i:]
    return body.rstrip() + '\n\n' + block + '\n'


def main():
    ap = argparse.ArgumentParser(
        description='Claude Code usage of pull requests, from the transcripts of their branches.')
    ap.add_argument('pr', type=int, nargs='+',
                    help='pull request number; several are counted in one pass over the transcripts')
    ap.add_argument('--update', action='store_true',
                    help='write the usage into the description of each pull request and refresh the badge')
    ap.add_argument('--also', action='append', default=[], metavar='BRANCH',
                    help="count this branch's messages too, for one pull request: a name its work was done under "
                         'that the transcripts do not join to its branch; the block keeps it for the next runs')
    ap.add_argument('--lower', action='store_true',
                    help='write a block even where it holds fewer tokens than the one in the description')
    args = ap.parse_args()
    numbers = list(dict.fromkeys(args.pr))
    if args.also and len(numbers) > 1:
        ap.error('--also names the branches of one pull request')

    prs = [json.loads(run(['gh', 'pr', 'view', str(n), '--json', 'number,headRefName,body,url'])) for n in numbers]
    for pr in prs:
        kept = (figures(pr['body']) or {}).get('also') or []
        pr['also'] = sorted((set(kept) | set(args.also)) - {pr['headRefName']})
    names = [{pr['headRefName'], *pr['also']} for pr in prs]
    top = main_checkout()
    counted = count(names, top, linked_worktrees(top), repository_branches().union(*names))

    failed, unpriced, written = [], set(), False
    for pr, c in zip(prs, counted):
        number, branch, url = pr['number'], pr['headRefName'], pr['url']
        print('pull request #%d, branch %s%s' % (number, branch, ''.join(' and %s' % a for a in pr['also'])))
        if not c.per_model:
            failed.append('#%d: no transcript of this repository has responses on branch %s' % (number, branch))
            print('no responses\n')
            continue
        print('sessions: %s' % ', '.join('%s (%d messages)' % s for s in c.sessions))
        block, data = block_of(c, pr['also'], url.rsplit('/pull/', 1)[0])
        print('\n' + block + '\n')
        unpriced |= {str(m) for m in c.per_model if m not in PRICES}
        if not args.update:
            continue
        described = unix(pr['body'] or '')
        before, body = figures(described), spliced(described, block)
        if before and tokens(data) < tokens(before) and not args.lower:
            # The transcripts are kept 30 days, and a branch's messages count elsewhere once its worktree is
            # another branch's or its name another's.
            failed.append('#%d: not written, its block holds %.1fM tokens and the transcripts %.1fM now; --also '
                          'names another branch of its work, --lower writes the smaller block'
                          % (number, tokens(before) / 1e6, tokens(data) / 1e6))
            print('not written: the block in the description holds more\n')
        elif body == (pr['body'] or ''):  # as it is stored: one with carriage returns is written once more, without
            print('unchanged in %s\n' % url)
        else:
            run(['gh', 'pr', 'edit', str(number), '--body-file', '-'], stdin=body)
            written = True
            print('written into %s\n' % url)
    if written:
        # The badge sums every pull request's block; the workflow runs from main, so it is missing on a branch
        # that adds it.
        r = subprocess.run(['gh', 'workflow', 'run', WORKFLOW], capture_output=True, text=True, encoding='utf-8')
        print('badge: %s' % ('refreshing (the Usage workflow)' if r.returncode == 0
                             else 'not refreshed: %s' % r.stderr.strip()))
    if unpriced:
        failed.append('no price for %s: the block counts its tokens and no dollars for them; add its row to PRICES '
                      '(tools/usage/pr_usage.py) from the pricing page and run this again'
                      % ', '.join(sorted(unpriced)))
    if failed:
        sys.exit('\n'.join(failed))


if __name__ == '__main__':
    main()
