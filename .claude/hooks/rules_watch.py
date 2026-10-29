#!/usr/bin/env python3
"""Claude Code hook: a running session learns that the rules changed on `main` (#1400; .claude/settings.json;
docs/03-toolchain.md, §3.5).

A session reads CLAUDE.md as it starts, and one that runs for hours does not see a rule merged meanwhile.
SessionStart (`start`) records the text of each file of WATCHED as the session's branch took it from `main` (at the
merge base of HEAD and `origin/main`, so that the session's own changes on its branch are not taken for main's).
UserPromptSubmit (`prompt`) fetches `origin/main`, at most once in FETCH_EVERY seconds, and when a file there differs
from the text recorded, adds its diff to the session's context (or, when it is long, the command that shows it) and
records the new text. A directory that is not a repository with `origin/main`, or a git that fails, adds nothing.

Example (the hook input is JSON on stdin)
  echo '{"session_id": "s1", "cwd": "."}' | uv run --no-project python -I .claude/hooks/rules_watch.py start
  echo '{"session_id": "s1", "cwd": "."}' | uv run --no-project python -I .claude/hooks/rules_watch.py prompt

Dependencies: standard library, git.
"""
import difflib, json, os, re, subprocess, sys, tempfile, time

# The files whose rules a session follows: CLAUDE.md, loaded as it starts, and the process (docs/03 §3.5, §3.6).
WATCHED = ('CLAUDE.md', 'docs/03-toolchain.md')
# What each session read, a file by its id.
STATE = os.path.join(os.environ.get('LOCALAPPDATA') or tempfile.gettempdir(), 'OpenSacred', 'rules-read')
# A fetch at most this often, in seconds, so that a prompt waits for the network rarely.
FETCH_EVERY = 300
# A diff longer than this, in characters, is named by the command that shows it instead.
LONGEST = 6000
SESSION = re.compile(r'[A-Za-z0-9-]+')
MAIN = 'origin/main'


def git(cwd, *args, timeout=20):
    """git's output in `cwd` as text, or None when it fails."""
    try:
        done = subprocess.run(['git', '-C', cwd, *args], capture_output=True, timeout=timeout)
    except (OSError, subprocess.TimeoutExpired):
        return None
    return done.stdout.decode('utf-8', errors='replace') if done.returncode == 0 else None


def texts(cwd, commit):
    """The text of each watched file at `commit`, those it has."""
    found = {}
    for path in WATCHED:
        text = git(cwd, 'show', f'{commit}:{path}')
        if text is not None:
            found[path] = text
    return found


def state_path(state, session):
    return os.path.join(state, f'{session}.json')


def save(state, session, record):
    os.makedirs(state, exist_ok=True)
    with open(state_path(state, session), 'w', encoding='utf-8', newline='') as f:
        json.dump(record, f)


def load(state, session):
    try:
        with open(state_path(state, session), encoding='utf-8') as f:
            record = json.load(f)
    except (OSError, ValueError):
        return None
    return record if isinstance(record, dict) and isinstance(record.get('files'), dict) else None


def start(event, state=STATE, now=time.time):
    """Record what the session read: the watched files at the merge base of HEAD and origin/main."""
    session, cwd = event.get('session_id') or '', event.get('cwd') or os.getcwd()
    if not SESSION.fullmatch(session):
        return None
    base = git(cwd, 'merge-base', 'HEAD', MAIN)
    base = base.strip() if base else None
    save(state, session, {'commit': base, 'files': texts(cwd, base) if base else {}, 'fetched': now()})
    return None


def prompt(event, state=STATE, now=time.time, fetch_every=FETCH_EVERY):
    """The context to add when a watched file changed on origin/main since the session read it, else None."""
    session, cwd = event.get('session_id') or '', event.get('cwd') or os.getcwd()
    if not SESSION.fullmatch(session):
        return None
    record = load(state, session)
    if record is None:
        # A session that started before this hook: its branch's main is taken for what it read.
        start(event, state, now)
        return None
    if now() - float(record.get('fetched') or 0) >= fetch_every:
        git(cwd, 'fetch', '--quiet', 'origin', 'main')
        record['fetched'] = now()
    main = git(cwd, 'rev-parse', MAIN)
    if main is None:
        save(state, session, record)
        return None
    main = main.strip()
    read_at, changes = record.get('commit'), []
    for path, theirs in texts(cwd, main).items():
        ours = record['files'].get(path)
        record['files'][path] = theirs
        if ours is None or ours == theirs:
            continue
        diff = ''.join(difflib.unified_diff(ours.splitlines(keepends=True), theirs.splitlines(keepends=True),
                                            f'{path} (as read)', f'{path} ({MAIN})'))
        if len(diff) <= LONGEST:
            changes.append(f'```diff\n{diff}```')
        else:
            lines = sum(1 for line in diff.splitlines() if line[:1] in '+-') - 2
            command = f'git diff {read_at} {MAIN} -- {path}' if read_at else f'git show {MAIN}:{path}'
            changes.append(f'{path}: {lines} lines changed; read them with `{command}`.')
    record['commit'] = main
    save(state, session, record)
    if not changes:
        return None
    return ('The rules changed on origin/main since this session read them (#1400): follow the new text from now '
            'on.\n\n' + '\n\n'.join(changes))


def main():
    what = sys.argv[1] if len(sys.argv) > 1 else ''
    try:
        event = json.load(sys.stdin)
    except ValueError:
        return
    if what == 'start':
        start(event)
    elif what == 'prompt':
        context = prompt(event)
        if context:
            json.dump({'hookSpecificOutput': {'hookEventName': 'UserPromptSubmit', 'additionalContext': context}},
                      sys.stdout)


if __name__ == '__main__':
    main()
