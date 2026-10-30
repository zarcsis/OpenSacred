#!/usr/bin/env python3
"""The newest releases of the sources that tools/ffmpeg/versions.toml pins (#1499): FFmpeg's tags
`nX.Y[.Z]` and libopus's `vX.Y[.Z]` on GitHub, SVT-AV1's releases on GitLab. The daily dependency
update runs it (CLAUDE.md, "Work order"): a source with a newer release gets a `deps` issue.

Example
  uv run --no-project python -I tools/ffmpeg/outdated.py

Prints each source, its pin and its newest release; exits with 1 when one is behind.
Dependencies: standard library.
"""
import json, re, sys, tomllib, urllib.request
from pathlib import Path

VERSIONS = Path(__file__).resolve().parent / 'versions.toml'

# Where each source's releases are listed, and the shape of a release's name there.
SOURCES = {
    'ffmpeg': ('https://api.github.com/repos/FFmpeg/FFmpeg/git/matching-refs/tags/n', r'refs/tags/n(\d+(?:\.\d+){1,2})'),
    'opus': ('https://api.github.com/repos/xiph/opus/git/matching-refs/tags/v', r'refs/tags/v(\d+(?:\.\d+){1,2})'),
    'svt-av1': ('https://gitlab.com/api/v4/projects/AOMediaCodec%2FSVT-AV1/releases?per_page=20', r'v(\d+(?:\.\d+){1,2})'),
}


def key(version):
    """A version's parts as numbers, for ordering: `9.0` before `9.0.1`."""
    return tuple(int(part) for part in version.split('.'))


def newest(names, pattern):
    """The newest version among the release names that match `pattern` whole."""
    versions = [m.group(1) for m in (re.fullmatch(pattern, name) for name in names) if m]
    return max(versions, key=key) if versions else None


def names(url):
    """The names that a listing gives: a GitHub ref's `ref`, a GitLab release's `tag_name`."""
    with urllib.request.urlopen(urllib.request.Request(url, headers={'User-Agent': 'opensacred'}), timeout=30) as reply:
        items = json.load(reply)
    return [item.get('ref') or item.get('tag_name') or '' for item in items]


def main():
    pins = tomllib.loads(VERSIONS.read_text(encoding='utf-8'))
    behind = False
    for name, (url, pattern) in SOURCES.items():
        pinned = pins[name]['version']
        latest = newest(names(url), pattern)
        state = 'up to date' if latest is None or key(latest) <= key(pinned) else f'newer: {latest}'
        behind |= state.startswith('newer')
        print(f'{name} {pinned}: {state}')
    return 1 if behind else 0


if __name__ == '__main__':
    sys.exit(main())
