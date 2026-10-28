#!/usr/bin/env python3
"""Claude Code hooks for the Rust workspace (.claude/settings.json; docs/03-toolchain.md, §3.4).

  fmt    PostToolUse on Edit|Write: after a .rs file is written, `cargo fmt --all`. A failure (for example, a file
         that does not parse yet in the middle of a multi-step edit) is left to the `check` hook.
  check  Stop: if the working tree has uncommitted changes to Rust sources or the workspace configuration, runs
         `cargo clippy --all-targets -- -D warnings` and `cargo nextest run` for the changed crates and the crates
         that depend on them. On failure Claude is sent back to fix it; if the checks still fail after that one
         retry, Claude may stop and the owner gets a warning. A passing state is remembered in target/, so a turn
         without new changes does not rebuild anything.

Example (the hook input is JSON on stdin)
  echo '{}' | uv run python -I .claude/hooks/rust_checks.py check

Tests on the game files (`--run-ignored all`) and `cargo deny check` stay manual (CLAUDE.md). Dependencies:
standard library, git, cargo, cargo-nextest.
"""
import hashlib, json, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
# Changes to these files affect every crate.
WORKSPACE_FILES = {'Cargo.toml', 'Cargo.lock', 'clippy.toml', 'rust-toolchain.toml', '.cargo/config.toml'}
STAMP = ROOT / 'target' / 'claude-hooks' / 'check.ok'
# How much of the cargo output goes back to Claude.
REPORT_CHARS = 6000
# Cargo progress lines carry no diagnostics.
PROGRESS = ('Compiling ', 'Checking ', 'Blocking ', 'Finished ', 'Fresh ', 'Downloading ', 'Downloaded ', 'Locking ')


def run(*args):
    return subprocess.run(args, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
                          encoding='utf-8', errors='replace')


def fmt(event):
    if str(event.get('tool_input', {}).get('file_path', '')).endswith('.rs'):
        run('cargo', 'fmt', '--all')
    return None


def changed_paths():
    """Paths with uncommitted changes, relative to the repository root, including untracked files and both sides
    of a rename."""
    entries = run('git', 'status', '--porcelain=v1', '-z', '--untracked-files=all').stdout.split('\0')
    paths, i = [], 0
    while i < len(entries):
        entry = entries[i]
        i += 1
        if len(entry) < 4:
            continue
        paths.append(entry[3:])
        if entry[0] in 'RC':  # the original path follows as a separate entry
            paths.append(entries[i])
            i += 1
    return paths


def workspace_packages():
    """{package name: (directory relative to ROOT, names of the workspace packages it depends on)}, or None if the
    manifests cannot be read; then everything is checked and cargo reports the error."""
    result = run('cargo', 'metadata', '--no-deps', '--format-version', '1')
    if result.returncode != 0:
        return None
    try:
        metadata = json.loads(result.stdout)
        root = Path(metadata['workspace_root'])
        return {p['name']: (Path(p['manifest_path']).parent.relative_to(root).as_posix(),
                            {d['name'] for d in p['dependencies'] if 'path' in d})
                for p in metadata['packages']}
    except (ValueError, KeyError):
        return None


def select(paths, packages):
    """Names of the packages to check, or None for the whole workspace."""
    if packages is None:
        return None
    selected = set()
    for path in paths:
        owner = next((name for name, (folder, _) in packages.items() if path.startswith(folder + '/')), None)
        if owner is None:
            return None  # a workspace-level file or a .rs file outside the crates
        selected.add(owner)
    # A change in a crate can break the crates that depend on it.
    while True:
        dependents = {name for name, (_, deps) in packages.items() if deps & selected} - selected
        if not dependents:
            return selected
        selected |= dependents


def fingerprint(paths):
    digest = hashlib.sha256(run('git', 'rev-parse', 'HEAD').stdout.encode())
    for path in sorted(paths):
        file = ROOT / path
        digest.update(path.encode() + b'\0' + (file.read_bytes() if file.is_file() else b'<deleted>') + b'\0')
    return digest.hexdigest()


def report(output, tail):
    text = '\n'.join(line for line in output.splitlines() if not line.strip().startswith(PROGRESS)).strip()
    if len(text) <= REPORT_CHARS:
        return text
    return '[...]\n' + text[-REPORT_CHARS:] if tail else text[:REPORT_CHARS] + '\n[...]'


def check(event):
    packages = workspace_packages()
    folders = tuple(folder + '/' for folder, _ in (packages or {}).values()) or ('crates/',)
    paths = [p for p in changed_paths() if p.endswith('.rs') or p in WORKSPACE_FILES or p.startswith(folders)]
    if not paths:
        return None
    stamp = fingerprint(paths)
    if STAMP.is_file() and STAMP.read_text(encoding='ascii').strip() == stamp:
        return None
    selected = select(paths, packages)
    scope = ['--workspace'] if selected is None else [arg for name in sorted(selected) for arg in ('-p', name)]
    steps = [(['cargo', 'clippy', '--all-targets', *scope, '--', '-D', 'warnings'], False),
             (['cargo', 'nextest', 'run', '--no-tests=pass', *scope], True)]
    for command, tail in steps:
        result = run(*command)
        if result.returncode == 0:
            continue
        shown = ' '.join(command)
        if event.get('stop_hook_active'):
            return {'systemMessage': f'Rust checks still fail after a retry: `{shown}`.'}
        return {'decision': 'block',
                'reason': f'The Stop hook ran `{shown}` and it failed. Fix this before finishing:\n\n'
                          f'{report(result.stdout, tail)}'}
    STAMP.parent.mkdir(parents=True, exist_ok=True)
    STAMP.write_text(stamp + '\n', encoding='ascii')
    checked = 'the whole workspace' if selected is None else ', '.join(sorted(selected))
    return {'systemMessage': f'Rust checks passed: clippy and nextest for {checked}.'}


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else ''
    if mode not in ('fmt', 'check'):
        sys.exit(f'usage: {Path(__file__).name} fmt|check  (hook input JSON on stdin)')
    raw = sys.stdin.read()
    event = json.loads(raw) if raw.strip() else {}
    output = fmt(event) if mode == 'fmt' else check(event)
    if output:
        print(json.dumps(output))


if __name__ == '__main__':
    main()
