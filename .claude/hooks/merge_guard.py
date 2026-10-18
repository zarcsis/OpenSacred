#!/usr/bin/env python3
"""Claude Code hook: Claude merges only the pull requests that do not wait for the maintainer (.claude/settings.json;
CLAUDE.md, "Issues and pull requests"; docs/03-toolchain.md, §3.6).

PreToolUse on Bash|PowerShell: a command that runs `gh pr merge` is refused when its pull request is labeled
`needs-maintainer`, touches a path of MAINTAINER_PATHS, or was opened by someone other than the repository's owner;
`--admin` is refused always. A pull request that cannot be looked up is refused too. Other commands pass untouched.
Merges through other routes (`gh api`, the web interface) are not seen; they are left to the rules in CLAUDE.md.

Example (the hook input is JSON on stdin)
  echo '{"tool_input": {"command": "gh pr merge 7 --squash --auto"}}' | python -I .claude/hooks/merge_guard.py

Dependencies: standard library, gh.
"""
import json, re, shlex, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LABEL = 'needs-maintainer'
# What a pull request changes here needs the maintainer's review: the rules, the hooks, CI and the process, the
# allowed licenses, the deviations from the original, the screenshot references. A path ending in '/' is a directory.
MAINTAINER_PATHS = ('CLAUDE.md', '.claude/', '.github/', 'deny.toml', 'docs/decisions.md',
                    'crates/opensacred-game/tests/screenshots/')
# How many of those paths a refusal names.
SHOWN_PATHS = 5
# `gh pr merge` options that take a value; the first other word is the pull request (a number, URL or branch).
VALUE_OPTIONS = {'-A', '--author-email', '-b', '--body', '-F', '--body-file', '--match-head-commit', '-R', '--repo',
                 '-t', '--subject'}
# `gh pr merge` where a command starts, so that a commit message or a search that mentions it passes.
MERGE = re.compile(r'(?:^|[;&|(\n]|\$\()\s*gh(?:\.exe)?\s+pr\s+merge\b([^;&|)\n]*)')


def merges(command):
    """The argument lists of every `gh pr merge` in a shell command."""
    for match in MERGE.finditer(command):
        try:
            yield shlex.split(match.group(1))
        except ValueError:  # unbalanced quotes: let the shell report it, but still look at the words
            yield match.group(1).split()


def target(args):
    """(the pull request or None for the current branch's, `--repo` or None, `--admin` given, `--disable-auto`
    given)."""
    selector = repo = None
    admin = disable = False
    i = 0
    while i < len(args):
        arg = args[i]
        name = arg.split('=', 1)[0]
        if name in VALUE_OPTIONS:
            value = arg.split('=', 1)[1] if '=' in arg else (args[i + 1] if i + 1 < len(args) else None)
            if name in ('-R', '--repo'):
                repo = value
            i += 1 if '=' in arg else 2
            continue
        if arg == '--admin':
            admin = True
        elif arg == '--disable-auto':
            disable = True
        elif not arg.startswith('-') and selector is None:
            selector = arg
        i += 1
    return selector, repo, admin, disable


def needs_maintainer(path):
    return any(path.startswith(p) if p.endswith('/') else path == p for p in MAINTAINER_PATHS)


def refusal(args):
    """Why this merge waits for the maintainer, or None."""
    selector, repo, admin, disable = target(args)
    if disable:
        return None  # turning auto-merge off merges nothing
    if admin:
        return '`--admin` bypasses the ruleset on main, which has no bypass by design.'
    command = ['gh', 'pr', 'view', *([selector] if selector else []), *(['--repo', repo] if repo else []),
               '--json', 'number,url,author,labels,files']
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, encoding='utf-8', errors='replace')
    if result.returncode != 0:
        return f'could not look up the pull request (`{" ".join(command)}`): {result.stderr.strip()}'
    try:
        pr = json.loads(result.stdout)
        owner = pr['url'].split('/')[3]
        labels = {label['name'] for label in pr['labels']}
        author = pr['author']['login']
        paths = [file['path'] for file in pr['files']]
    except (ValueError, KeyError, IndexError, TypeError) as error:
        return f'could not read `gh pr view` output: {error!r}'
    reasons = []
    if LABEL in labels:
        reasons.append(f'it is labeled `{LABEL}`')
    if author.lower() != owner.lower():
        reasons.append(f'it was opened by {author}, not by the maintainer')
    touched = [f'`{path}`' for path in paths if needs_maintainer(path)]
    if touched:
        shown = touched[:SHOWN_PATHS] + ([f'{len(touched) - SHOWN_PATHS} more'] if len(touched) > SHOWN_PATHS else [])
        reasons.append('it changes ' + ', '.join(shown))
    if not reasons:
        return None
    label = '' if LABEL in labels else f'Label it `{LABEL}` (`gh pr edit {pr["number"]} --add-label {LABEL}`), '
    return (f'pull request #{pr["number"]} waits for the maintainer: {"; ".join(reasons)}. {label}'
            f'{"tell" if label else "Tell"} the maintainer, and leave the merge to them.')


def main():
    raw = sys.stdin.read()
    event = json.loads(raw) if raw.strip() else {}
    command = str(event.get('tool_input', {}).get('command', ''))
    for args in merges(command):
        reason = refusal(args)
        if reason:
            print(json.dumps({'hookSpecificOutput': {'hookEventName': 'PreToolUse', 'permissionDecision': 'deny',
                                                     'permissionDecisionReason': 'The merge guard refused: ' + reason}}))
            return


if __name__ == '__main__':
    main()
