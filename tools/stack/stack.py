#!/usr/bin/env python3
"""Stacked pull requests: bring them up to `main` after their base is squash-merged, and merge every open pull
request into one branch to see the conflicts before the merges (docs/03 §3.6).

Purpose: `main` takes squash merges only, so a pull request stacked on another still carries its base's own commits
once that base is merged. Where it changed its base's lines, Git sees two versions of them, the base's in `main` and
its own, against the old common ancestor, and stops. `after` merges `main` into it with the newest of its base's
commits that it holds as the merge base (`git merge-tree --merge-base`): it keeps its own lines and gains only what
`main` got beyond its base. `integrate` merges every open pull request into a branch of `main`, a stacked one after
its base, so that the order of the merges and their conflicts are known while they all wait. Neither touches the
working tree: the merges are made with `git merge-tree` and `git commit-tree`.

Example
  uv run python -I tools/stack/stack.py after 52             # bring the pull requests stacked on #52 up to main, push them
  uv run python -I tools/stack/stack.py after 52 --dry-run   # only print what it would do
  uv run python -I tools/stack/stack.py integrate            # the branch `integration`: main and every open pull request
  uv run python -I tools/stack/stack.py integrate --check    # also fmt, clippy and the tests on it, in a worktree

Expected output: for `after`, each open pull request that holds commits of the merged one's branch, the merge's
changes (what `main` got beyond the base) and the commit pushed, or the files in conflict; for `integrate`, the
pull requests in the order of merging, each merged or in conflict (and the ones stacked on a pull request in
conflict, skipped), then the branch. Dependencies: standard library, `git` 2.40+ and the GitHub CLI `gh`.
"""
import argparse, json, os, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REMOTE = 'origin'
MAIN = 'main'
TRAILER = 'Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>'


class Git:
    """`git` in one repository."""

    def __init__(self, root):
        self.root = root

    def run(self, *args, check=True):
        result = subprocess.run(['git', *args], cwd=self.root, capture_output=True, text=True, encoding='utf-8',
                                errors='replace')
        if check and result.returncode != 0:
            raise SystemExit(f'git {" ".join(args)}: {result.stderr.strip()}')
        return result

    def out(self, *args):
        return self.run(*args).stdout.strip()

    def is_ancestor(self, older, newer):
        return self.run('merge-base', '--is-ancestor', older, newer, check=False).returncode == 0

    def merge(self, ours, theirs, base=None):
        """The tree of merging `theirs` into `ours` (with `base` as the merge base), or None and the files in
        conflict."""
        args = ['merge-tree', '--write-tree', '--name-only', *([f'--merge-base={base}'] if base else []), ours, theirs]
        result = self.run(*args, check=False)
        lines = result.stdout.splitlines()
        if result.returncode == 0:
            return lines[0], []
        if result.returncode == 1:
            # The tree, the conflicted files, a blank line, then the messages.
            files = []
            for line in lines[1:]:
                if not line:
                    break
                files.append(line)
            return None, files
        raise SystemExit(f'git merge-tree: {result.stderr.strip()}')

    def commit(self, tree, parents, message):
        args = ['commit-tree', tree, *[a for p in parents for a in ('-p', p)], '-m', message]
        return self.out(*args)


def bring_up(git, parent_head, child_tip, main):
    """Merges `main` into a child stacked on a squash-merged parent: (the new tree, []) or (None, the files in
    conflict), with the newest of the parent's commits that the child holds as the merge base. None when the child
    holds none of the parent's commits (it is not stacked on it) or already holds `main`."""
    base = git.out('merge-base', parent_head, child_tip)
    if git.is_ancestor(base, main) or git.is_ancestor(main, child_tip):
        return None
    return git.merge(main, child_tip, base) + (base,)


def merge_order(prs):
    """The open pull requests in an order to merge them: one on `main` before the ones stacked on it."""
    by_branch = {pr['headRefName']: pr for pr in prs}
    order, seen = [], set()

    def visit(pr, path=()):
        if pr['number'] in seen:
            return
        if pr['number'] in path:
            raise SystemExit(f'pull requests stacked in a loop: {path}')
        base = by_branch.get(pr['baseRefName'])
        if base:
            visit(base, path + (pr['number'],))
        seen.add(pr['number'])
        order.append(pr)

    for pr in sorted(prs, key=lambda p: p['number']):
        visit(pr)
    return order


def gh_json(*args):
    result = subprocess.run(['gh', *args], cwd=ROOT, capture_output=True, text=True, encoding='utf-8',
                            errors='replace')
    if result.returncode != 0:
        raise SystemExit(f'gh {" ".join(args)}: {result.stderr.strip()}')
    return json.loads(result.stdout)


def open_prs():
    return gh_json('pr', 'list', '--state', 'open', '--limit', '200', '--json',
                   'number,title,headRefName,headRefOid,baseRefName')


def after(args):
    git = Git(ROOT)
    parent = gh_json('pr', 'view', str(args.pr), '--json', 'number,state,headRefName,headRefOid')
    if parent['state'] != 'MERGED':
        raise SystemExit(f'#{args.pr} is {parent["state"].lower()}, not merged')
    git.run('fetch', '--quiet', '--prune', REMOTE)
    main = git.out('rev-parse', f'{REMOTE}/{MAIN}')
    found = False
    for pr in open_prs():
        tip = git.out('rev-parse', f'{REMOTE}/{pr["headRefName"]}')
        result = bring_up(git, parent['headRefOid'], tip, main)
        if result is None:
            continue
        found = True
        tree, conflicts, base = result
        if tree is None:
            print(f'#{pr["number"]} ({pr["headRefName"]}): conflict in {", ".join(conflicts)}')
            continue
        gained = git.out('diff', '--stat', tip, tree).splitlines()
        print(f'#{pr["number"]} ({pr["headRefName"]}): gains {gained[-1].strip() if gained else "nothing"}')
        if args.dry_run:
            continue
        message = (f'Merge {MAIN} (#{parent["number"]}\'s squash) into {pr["headRefName"]}\n\n'
                   f'Merged with the newest of #{parent["number"]}\'s commits that this branch holds as the base, '
                   f'so that its own\nchanges to #{parent["number"]}\'s lines stand.\n\n{TRAILER}')
        commit = git.commit(tree, [tip, main], message)
        git.run('push', '--quiet', REMOTE, f'{commit}:refs/heads/{pr["headRefName"]}')
        if pr['baseRefName'] != MAIN:
            subprocess.run(['gh', 'pr', 'edit', str(pr['number']), '--base', MAIN], cwd=ROOT, check=True,
                           capture_output=True)
        print(f'#{pr["number"]}: pushed {commit[:10]}')
    if not found:
        print(f'no open pull request holds commits of #{parent["number"]}\'s branch')


def integration_target(environ):
    """The target directory of the integration's check: `integration/` in the checkout's own.

    Not the checkout's own: the test binaries bake their tree's `CARGO_MANIFEST_DIR` into the paths
    of the game files, and cargo would take those built for the temporary tree as fresh for the
    checkout's sources when they are the same, their tests then looking for the deleted tree
    (#1502)."""
    return Path(environ.get('CARGO_TARGET_DIR', str(ROOT / 'target'))) / 'integration'


def check(git, branch):
    """fmt, clippy and the tests (without the game files) on `branch`, in a temporary worktree."""
    env = {**os.environ, 'CARGO_TARGET_DIR': str(integration_target(os.environ))}
    with tempfile.TemporaryDirectory(prefix='opensacred-integration-') as tmp:
        tree = Path(tmp) / 'tree'
        git.run('worktree', 'add', '--quiet', '--detach', str(tree), branch)
        try:
            for name, command in (('fmt', ['cargo', 'fmt', '--all', '--', '--check']),
                                  ('clippy', ['cargo', 'clippy', '--workspace', '--all-targets', '--', '-D',
                                              'warnings']),
                                  ('tests', ['cargo', 'nextest', 'run', '--workspace'])):
                result = subprocess.run(command, cwd=tree, env=env, capture_output=True, text=True,
                                        encoding='utf-8', errors='replace')
                status = 'passed' if result.returncode == 0 else 'FAILED'
                print(f'{name}: {status}')
                if result.returncode != 0:
                    print('\n'.join((result.stdout + result.stderr).splitlines()[-30:]))
                    return False
        finally:
            git.run('worktree', 'remove', '--force', str(tree), check=False)
    return True


def integrate(args):
    git = Git(ROOT)
    git.run('fetch', '--quiet', '--prune', REMOTE)
    head = git.out('rev-parse', f'{REMOTE}/{MAIN}')
    failed, merged = set(), []
    for pr in merge_order(open_prs()):
        label = f'#{pr["number"]} ({pr["headRefName"]})'
        if pr['baseRefName'] in failed:
            print(f'{label}: skipped, stacked on a pull request in conflict')
            failed.add(pr['headRefName'])
            continue
        tip = git.out('rev-parse', f'{REMOTE}/{pr["headRefName"]}')
        tree, conflicts = git.merge(head, tip)
        if tree is None:
            print(f'{label}: CONFLICT in {", ".join(conflicts)}')
            failed.add(pr['headRefName'])
            continue
        head = git.commit(tree, [head, tip], f'Merge #{pr["number"]} into {args.branch}\n\n{TRAILER}')
        merged.append(pr['number'])
        print(f'{label}: merged')
    git.run('branch', '--force', args.branch, head)
    print(f'{args.branch}: {MAIN} and {len(merged)} pull requests, {len(failed)} in conflict or skipped')
    if args.check and not check(git, args.branch):
        sys.exit(1)
    if failed:
        sys.exit(2)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    commands = parser.add_subparsers(dest='command', required=True)
    a = commands.add_parser('after', help='bring the pull requests stacked on a squash-merged one up to main')
    a.add_argument('pr', type=int, help='the merged pull request')
    a.add_argument('--dry-run', action='store_true', help='print what would be pushed, push nothing')
    a.set_defaults(run=after)
    i = commands.add_parser('integrate', help='merge every open pull request into one branch of main')
    i.add_argument('--branch', default='integration', help='the local branch to write (default: integration)')
    i.add_argument('--check', action='store_true', help='then run fmt, clippy and the tests on it')
    i.set_defaults(run=integrate)
    args = parser.parse_args()
    args.run(args)


if __name__ == '__main__':
    main()
