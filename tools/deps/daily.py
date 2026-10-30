#!/usr/bin/env python3
"""The daily dependency update (#1550; CLAUDE.md, "Work order"; docs/03 §3.3), which the Dependencies workflow
(.github/workflows/deps.yml) runs with its GitHub App's token: `cargo update` and `uv lock --upgrade`, the holds of
tools/deps/holds.toml put back, in a pull request `Dependencies: cargo update` that merges itself once its checks are
green, and a `deps` issue for each newer incompatible release of a direct dependency, a Python package, Rust, Python
or a source of the release's FFmpeg, unless one is open.

An update's pull request still open at the next run, its checks failed or its merge held, is closed with its branch,
and a `deps` issue tells what kept it: a session fixes the update in a pull request of its own, or holds the package
back in holds.toml. The text of the issues and pull requests is made from the names of Cargo.toml, pyproject.toml and
tools/ffmpeg/versions.toml, the registries' version numbers and the checks' names alone, so that claim.py may take the
app's issues as the sessions' own (#1421).

Example
  uv run --no-project python -I tools/deps/daily.py --dry-run   # updates the lock files here, prints the rest

Environment: GH_TOKEN, the app's token; APP_SLUG, the app's name; RUN_URL, the workflow run's page.
Dependencies: standard library, cargo, uv, git and the GitHub CLI `gh`.
"""
import argparse, datetime, json, os, re, subprocess, sys, tomllib, urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HOLDS = Path(__file__).resolve().parent / 'holds.toml'
LOCKS = ('Cargo.lock', 'uv.lock')
TITLE = 'Dependencies: cargo update'
BRANCH = 'deps/cargo-update-'
STUCK = 'Dependencies: the daily update did not merge'
RUST_STABLE = 'https://static.rust-lang.org/dist/channel-rust-stable.toml'

# `cargo update --verbose`: a package moved, added or removed, and one that a newer incompatible release leaves behind.
CARGO_CHANGE = re.compile(r'^\s*(Updating|Downgrading|Adding|Removing) (\S+) v(\S+)(?: -> v(\S+))?', re.M)
CARGO_BEHIND = re.compile(r'^\s*Unchanged (\S+) v(\S+) \(available: v([^,)\s]+)', re.M)
# `uv lock --upgrade` (`Update`, `Add`, `Remove` with --dry-run), and a direct dependency of `uv tree --outdated`.
UV_CHANGE = re.compile(r'^(Update|Add|Remove)(?:e?d)? (\S+) v(\S+)(?: -> v(\S+))?$', re.M)
UV_BEHIND = re.compile(r'^[├└]── (\S+) v(\S+)\b.*\(latest: v([^)\s]+)\)$', re.M)
# tools/ffmpeg/outdated.py: a source behind its newest release.
FFMPEG_BEHIND = re.compile(r'^(\S+) (\S+): newer: (\S+)$', re.M)
# The outcomes of a check that keep a pull request from merging: a check run's conclusion, a status's state.
FAILED = {'FAILURE', 'CANCELLED', 'TIMED_OUT', 'ACTION_REQUIRED', 'STARTUP_FAILURE', 'ERROR'}


def run(*args, check=True):
    """Runs a command in the repository's root; cargo and uv report on the standard error."""
    result = subprocess.run(args, cwd=ROOT, capture_output=True, text=True, encoding='utf-8', errors='replace')
    if check and result.returncode != 0:
        raise SystemExit(f'{" ".join(args)}: {(result.stderr or result.stdout).strip()}')
    return result


def key(version):
    """A version's numbers, for ordering: `1.99.1` before `1.100.0`."""
    return tuple(int(part) for part in re.findall(r'\d+', version))


def direct_crates(manifest):
    """The crates of `[workspace.dependencies]` by their names on crates.io (a renamed one by its `package`), without
    the workspace's own."""
    crates = set()
    for name, spec in tomllib.loads(manifest)['workspace']['dependencies'].items():
        if isinstance(spec, dict):
            if 'path' in spec:
                continue
            name = spec.get('package', name)
        crates.add(name)
    return crates


def direct_packages(project):
    """The Python packages of pyproject.toml's `dependencies`, by their names."""
    return {re.match(r'[A-Za-z0-9._-]+', requirement).group(0)
            for requirement in tomllib.loads(project)['project']['dependencies']}


def changes(pattern, output):
    """The packages that an update moved: (verb, name, old, new), `new` None for one added or removed."""
    return [m.groups() for m in pattern.finditer(output)]


def change_line(verb, name, old, new, direct):
    """A change as the pull request lists it."""
    mark = ' (direct)' if direct else ''
    if new:
        return f'- `{name}` {old} → {new}{mark}' + (', down' if verb == 'Downgrading' else '')
    return f"- `{name}` {old} {'added' if verb.startswith('Add') else 'removed'}{mark}"


def rust_latest(channel):
    """The version of the stable channel's manifest: `1.99.1` of `1.99.1 (ded0c5bd8 2026-08-29)`."""
    return tomllib.loads(channel)['pkg']['rust']['version'].split()[0]


def python_latest(listing, pinned):
    """The newest final CPython minor version that `uv python list --all-versions --output-format json` names, if it
    is newer than `pinned` (`.python-version`, `3.15`)."""
    minors = {(entry['version_parts']['major'], entry['version_parts']['minor']) for entry in json.loads(listing)
              if entry['implementation'] == 'cpython' and re.fullmatch(r'\d+\.\d+\.\d+', entry['version'])}
    newest = max(minors, default=None)
    if newest and newest > key(pinned)[:2]:
        return f'{newest[0]}.{newest[1]}'
    return None


def failures(pr):
    """Why an update's pull request is still open: its checks failed, still run, or passed without a merge."""
    failed, running = [], []
    for check in pr.get('statusCheckRollup') or []:
        name = check.get('name') or check.get('context') or 'a check'
        outcome = check.get('conclusion') or check.get('state') or ''
        if outcome in FAILED:
            failed.append(f"{name} ({check.get('detailsUrl') or check.get('targetUrl') or 'no link'})")
        elif check.get('status', 'COMPLETED') != 'COMPLETED' or outcome == 'PENDING':
            running.append(name)
    if failed:
        return f"its checks failed: {', '.join(failed)}"
    if running:
        return f"its checks were still running a day on: {', '.join(running)}"
    return f"its checks passed, but it did not merge (merge state {pr.get('mergeStateStatus') or 'unknown'})"


def issue_body(versions, run_url):
    """The body of a newer incompatible release's issue, in the fields of the form `deps.yml`."""
    return (f'### Versions\n\n{versions}\n\n'
            '### What changes\n\nNot read yet: the release notes or the migration guide, and what of them touches our '
            'code, are for the session that takes this issue.\n\n'
            '### What breaks\n\nNot tried yet.\n\n'
            f'Filed by the Dependencies workflow ({run_url}; docs/03 §3.3).\n')


class GitHub:
    """The calls of `gh` that the update makes; with `dry_run` the writes are printed instead."""

    def __init__(self, dry_run):
        self.dry_run = dry_run

    def read(self, *args):
        return json.loads(run('gh', *args).stdout)

    def write(self, *args):
        if self.dry_run:
            print('would run: gh', ' '.join(args))
            return ''
        return run('gh', *args).stdout.strip()

    def open_issues(self):
        """The open `deps` issues: {title: number}."""
        return {issue['title']: issue['number'] for issue in self.read('issue', 'list', '--state', 'open', '--label',
                                                                       'deps', '--limit', '1000', '--json',
                                                                       'number,title')}


def close_stuck(github, slug, run_url, today):
    """Closes the app's update pull requests still open, with their branches, and tells why in a `deps` issue."""
    prs = github.read('pr', 'list', '--state', 'open', '--author', f'app/{slug}', '--json',
                      'number,headRefName,url,statusCheckRollup,mergeStateStatus')
    for pr in prs:
        if not pr['headRefName'].startswith(BRANCH):
            continue
        why = failures(pr)
        text = (f"The daily dependency update's pull request #{pr['number']} did not merge: {why}.\n"
                f'The Dependencies workflow closed it with its branch on {today} ({run_url}); each day\'s update '
                'brings the same changes back until a session fixes the cause in a pull request of its own, or holds '
                'the package back in `tools/deps/holds.toml` with this issue\'s number (docs/03 §3.3).\n')
        stuck = github.open_issues().get(STUCK)
        if stuck:
            github.write('issue', 'comment', str(stuck), '--body', text)
        else:
            github.write('issue', 'create', '--title', STUCK, '--label', 'deps', '--body', text)
        github.write('pr', 'close', str(pr['number']), '--delete-branch', '--comment',
                     f'Closed by the next daily update ({run_url}): {why}.')


def put_back(holds):
    """Puts the held packages back at their versions: (ecosystem, name, version, issue) of each."""
    held = []
    for name, hold in holds.get('cargo', {}).items():
        run('cargo', 'update', '--package', name, '--precise', hold['version'])
        held.append(('cargo', name, hold['version'], hold['issue']))
    for name, hold in holds.get('uv', {}).items():
        run('uv', 'lock', '--upgrade-package', f"{name}=={hold['version']}")
        held.append(('uv', name, hold['version'], hold['issue']))
    return held


def pr_body(today, cargo, uv, direct, held, run_url):
    """The update's pull request: the changes, the direct dependencies first, and the holds."""
    lines = [f'The daily dependency update of {today}, by the Dependencies workflow ({run_url}; docs/03 §3.3).', '']
    for title, moved in (('`cargo update`', cargo), ('`uv lock --upgrade`', uv)):
        lines.append(f'## {title}')
        lines.append('')
        if moved:
            ordered = sorted(moved, key=lambda change: change[1] not in direct)
            lines += [change_line(*change, change[1] in direct) for change in ordered]
        else:
            lines.append('Nothing changed.')
        lines.append('')
    if held:
        lines.append('## Held back')
        lines.append('')
        lines += [f'- `{name}` at {version} ({ecosystem}, #{issue})' for ecosystem, name, version, issue in held]
        lines.append('')
    lines.append('CI has no game files, so this update merges without `--run-ignored all`: the tests on the game files '
                 'run with the next task that touches them (docs/03 §3.3).')
    return '\n'.join(lines) + '\n'


def open_pr(github, slug, today, body):
    """Commits the lock files on the day's branch as the app's account, pushes it and opens its pull request, which
    merges itself once its checks are green."""
    branch = f'{BRANCH}{today}'
    bot = f'{slug}[bot]'
    user_id = run('gh', 'api', f'users/{bot}', '--jq', '.id').stdout.strip()
    run('git', 'switch', '--create', branch)
    run('git', 'add', '--', *LOCKS)
    run('git', '-c', f'user.name={bot}', '-c', f'user.email={user_id}+{bot}@users.noreply.github.com', 'commit',
        '--quiet', '--message', f'{TITLE}\n\nThe daily dependency update of {today} (#1550).')
    run('git', 'push', '--force', 'origin', f'HEAD:refs/heads/{branch}')
    url = github.write('pr', 'create', '--base', 'main', '--head', branch, '--title', TITLE, '--label', 'deps',
                       '--body', body)
    github.write('pr', 'merge', url, '--squash', '--auto')
    print(f'opened {url}')


def behind(cargo_output, direct, held):
    """The newer incompatible releases: (key, title, versions) of each, `key` the start of the title that an open
    issue of the same dependency has."""
    found = []
    for name, old, new in CARGO_BEHIND.findall(cargo_output):
        if name in direct and name not in held:
            found.append((name, old, new, ''))
    tree = run('uv', 'tree', '--outdated', '--depth', '1').stdout
    for name, old, new in UV_BEHIND.findall(tree):
        if name not in held:
            found.append((name, old, new, ''))
    toolchain = tomllib.loads((ROOT / 'rust-toolchain.toml').read_text(encoding='utf-8'))['toolchain']['channel']
    with urllib.request.urlopen(urllib.request.Request(RUST_STABLE, headers={'User-Agent': 'opensacred'}),
                                timeout=60) as reply:
        rust = rust_latest(reply.read().decode('utf-8'))
    if key(rust) > key(toolchain):
        found.append(('Rust', toolchain, rust, ''))
    pinned = (ROOT / '.python-version').read_text(encoding='utf-8').strip()
    python = python_latest(run('uv', 'python', 'list', '--all-versions', '--output-format', 'json').stdout, pinned)
    if python:
        found.append(('Python', pinned, python, ''))
    ffmpeg = run('uv', 'run', '--no-project', 'python', '-I', 'tools/ffmpeg/outdated.py', check=False).stdout
    for name, old, new in FFMPEG_BEHIND.findall(ffmpeg):
        found.append((name, old, new, " for the release's FFmpeg"))
    return [(f'Dependencies: {name} ', f'Dependencies: {name} {new}{suffix}', f'{name} {old} -> {new}')
            for name, old, new, suffix in found]


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('--dry-run', action='store_true',
                        help='update the lock files here and print the pull request and the issues, without git or gh '
                             'writes')
    args = parser.parse_args()
    slug, run_url = os.environ.get('APP_SLUG'), os.environ.get('RUN_URL', 'a local run')
    if not slug and not args.dry_run:
        raise SystemExit('APP_SLUG names the GitHub App whose token GH_TOKEN holds')
    github = GitHub(args.dry_run)
    today = datetime.datetime.now(datetime.UTC).date().isoformat()

    if slug:
        close_stuck(github, slug, run_url, today)

    cargo = run('cargo', 'update', '--verbose').stderr
    uv = run('uv', 'lock', '--upgrade').stderr
    held = put_back(tomllib.loads(HOLDS.read_text(encoding='utf-8')))
    direct = direct_crates((ROOT / 'Cargo.toml').read_text(encoding='utf-8'))
    packages = direct_packages((ROOT / 'pyproject.toml').read_text(encoding='utf-8'))
    body = pr_body(today, changes(CARGO_CHANGE, cargo), changes(UV_CHANGE, uv), direct | packages, held, run_url)
    if run('git', 'status', '--porcelain', '--', *LOCKS).stdout.strip():
        if args.dry_run:
            print(f'would open {TITLE}:\n{body}')
        else:
            open_pr(github, slug, today, body)
    else:
        print('Cargo.lock and uv.lock are up to date')

    open_titles = github.open_issues()
    for prefix, title, versions in behind(cargo, direct, {name for _, name, _, _ in held}):
        if any(open_title.startswith(prefix) for open_title in open_titles):
            print(f'an issue is open for {versions}')
            continue
        github.write('issue', 'create', '--title', title, '--label', 'deps', '--body', issue_body(versions, run_url))
    return 0


if __name__ == '__main__':
    sys.exit(main())
