#!/usr/bin/env python3
"""Take an issue into work (#1401; CLAUDE.md, "Issues and pull requests"; docs/03 §3.5): refuse one that a session
holds, else assign it, name its branch after it, push the branch as the claim and add its worktree.

Purpose: every session works under one GitHub account, so an assignee alone does not say which session holds an issue,
and the steps of taking one, repeated by hand, are easy to get wrong (#1210 lost usage to a branch renamed later). An
issue is held when it is assigned (unless --mine: this session filed it assigned), when a branch `<type>/<N>-<slug>`
exists, locally or on origin, or when an open pull request closes it (`Closes #N`) or comes from such a branch; a
closed issue, or one without exactly one type label, is refused too. Otherwise origin is fetched, the issue assigned
to the account, the branch `<label>/<N>-<SLUG>` made from origin/main (or --base) in the worktree
`.claude/worktrees/<N>-<SLUG>` of the main checkout, and pushed at once, so that a session that looks next sees it; a
branch that another session pushed meanwhile for the same issue is reported. CI runs on pull requests and on `main`
alone, so the pushed branch costs no run.

Example
  uv run python -I tools/stack/claim.py 1401 claim              # then EnterWorktree with the path it prints
  uv run python -I tools/stack/claim.py 1402 holder --base origin/infra/1401-claim   # a stacked one
  uv run python -I tools/stack/claim.py 1401 claim --dry-run    # only the checks

Expected output: the issue's title and what holds it, or its branch and the worktree's path. Dependencies: standard
library, `git` 2.40+ and the GitHub CLI `gh`.
"""
import argparse, json, re, subprocess, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from stack import MAIN, REMOTE, Git  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
# The labels of the issue forms' types, which name a branch's first part (CLAUDE.md, "Issues and pull requests").
TYPES = ('bug', 'deps', 'feature', 'research', 'infra', 'docs')
SLUG = re.compile(r'[a-z0-9]+(?:-[a-z0-9]+)*')


class GitHub:
    """The calls of `gh` that taking an issue makes, in the repository at `root`."""

    def __init__(self, root):
        self.root = root

    def run(self, *args):
        result = subprocess.run(['gh', *args], cwd=self.root, capture_output=True, text=True, encoding='utf-8',
                                errors='replace')
        if result.returncode != 0:
            raise SystemExit(f'gh {" ".join(args)}: {result.stderr.strip()}')
        return result.stdout

    def issue(self, number):
        return json.loads(self.run('issue', 'view', str(number), '--json', 'number,title,state,assignees,labels'))

    def open_prs(self):
        return json.loads(self.run('pr', 'list', '--state', 'open', '--limit', '500', '--json',
                                   'number,headRefName,body'))

    def assign(self, number):
        self.run('issue', 'edit', str(number), '--add-assignee', '@me')


def of_issue(branch, number):
    """Whether `branch` is named after the issue `number`: `<type>/<number>-<slug>`."""
    return re.fullmatch(rf'[^/]+/{number}-.+', branch) is not None


def branches(git, number):
    """The branches named after the issue `number`, local or on origin as last fetched."""
    local = git.out('branch', '--list', f'*/{number}-*', '--format=%(refname:short)').split()
    remote = git.out('branch', '--remotes', '--list', f'{REMOTE}/*/{number}-*', '--format=%(refname:short)').split()
    found = set(local) | {name.removeprefix(f'{REMOTE}/') for name in remote}
    return sorted(name for name in found if of_issue(name, number))


def pushed(git, number):
    """The branches named after the issue `number` on origin now."""
    heads = git.out('ls-remote', '--heads', REMOTE, f'refs/heads/*/{number}-*').splitlines()
    names = [line.split('refs/heads/', 1)[1] for line in heads if 'refs/heads/' in line]
    return sorted(name for name in names if of_issue(name, number))


def holders(issue, prs, named, mine=False):
    """What holds the issue: its state, its assignees, the branches named after it and the open pull requests that
    close it or come from such a branch."""
    number, reasons = issue['number'], []
    if issue['state'] != 'OPEN':
        reasons.append(f"it is {issue['state'].lower()}")
    assignees = [a['login'] for a in issue.get('assignees') or []]
    if assignees and not mine:
        reasons.append(f"it is assigned to {', '.join(assignees)}, and every session works under that account")
    reasons += [f'the branch {name}' for name in named]
    closing = re.compile(rf'(?i)\b(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?)\s+#{number}\b')
    for pr in prs:
        if closing.search(pr.get('body') or '') or of_issue(pr['headRefName'], number):
            reasons.append(f"the open pull request #{pr['number']} ({pr['headRefName']})")
    return reasons


def kind(issue):
    """The issue's type, from its one type label."""
    labels = [label['name'] for label in issue.get('labels') or [] if label['name'] in TYPES]
    if len(labels) != 1:
        raise SystemExit(f"#{issue['number']} has {len(labels)} type labels ({', '.join(labels) or 'none'}), not one "
                         f"of {', '.join(TYPES)}")
    return labels[0]


def take(number, slug, base, root=ROOT, github=None, dry_run=False, mine=False):
    """Take the issue `number` into work as `<type>/<number>-<slug>` from `base`: the worktree's path."""
    if not SLUG.fullmatch(slug):
        raise SystemExit(f'a slug is lower-case words joined by hyphens, not {slug!r}')
    git, github = Git(root), github or GitHub(root)
    git.run('fetch', '--quiet', REMOTE)
    issue = github.issue(number)
    reasons = holders(issue, github.open_prs(), branches(git, number), mine)
    if reasons:
        raise SystemExit(f"#{number} ({issue['title']}) is held: {'; '.join(reasons)}. Leave it to its holder, or ask "
                         'the peer sessions (ListAgents, SendMessage)')
    branch = f'{kind(issue)}/{number}-{slug}'
    checkout = Path(git.out('rev-parse', '--path-format=absolute', '--git-common-dir')).parent
    tree = checkout / '.claude' / 'worktrees' / f'{number}-{slug}'
    if tree.exists():
        raise SystemExit(f'{tree} exists already')
    if dry_run:
        print(f"#{number} ({issue['title']}) is free: it would be {branch} from {base} in {tree}")
        return tree
    github.assign(number)
    git.run('worktree', 'add', '--quiet', '--no-track', '-b', branch, str(tree), base)
    Git(tree).run('push', '--quiet', '--set-upstream', REMOTE, branch)
    print(f"took #{number} ({issue['title']}): {branch} from {base}, pushed")
    others = [name for name in pushed(git, number) if name != branch]
    if others:
        print(f"but another branch of #{number} is on {REMOTE} too: {', '.join(others)}; ask the peer sessions which "
              'keeps it (ListAgents, SendMessage)')
    print(f'worktree {tree}: enter it with EnterWorktree and this path')
    return tree


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    parser.add_argument('number', type=int, help='the issue')
    parser.add_argument('slug', help="the branch's and the worktree's name after the number, as `claim`")
    parser.add_argument('--base', default=f'{REMOTE}/{MAIN}', help=f'the branch to start from ({REMOTE}/{MAIN})')
    parser.add_argument('--mine', action='store_true',
                        help='the issue is assigned by this session itself, which filed it so: its assignee holds it '
                             'not; its branches and pull requests still do')
    parser.add_argument('--dry-run', action='store_true', help='only the checks')
    args = parser.parse_args()
    take(args.number, args.slug, args.base, dry_run=args.dry_run, mine=args.mine)


if __name__ == '__main__':
    main()
