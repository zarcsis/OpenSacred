#!/usr/bin/env python3
"""The local Ghidra project of docs/03 §4.1 rebuilt from the installation, its markup dumped and compared (#3870).

Purpose: the project holds the imported `Sacred.exe` and `GameServer.exe`, the game's own files, and no markup of a
session's own, so it is copied nowhere and made again when it is lost:
- `build` makes it from the developer's installation in one go, as docs/03 §4.1 tells: both executables checked
  against docs/manifest-gog-2.28.txt, imported with Ghidra's default analysis and named as pyghidra-mcp's first start
  named them, then this directory's scripts in that section's order, over the community's names that
  `community_names.py` makes from sacred-bild and sacred-sdk at the pinned commits, fetched into a temporary
  directory; `pyghidra-mcp.paused` lies in the project's directory meanwhile (docs/03 §3.2). A project already there
  is moved aside with `--replace` alone, and never while it is open.
- `dump` writes each program's markup as sorted text lines (`DumpMarkupScript.java`) from a copy of the project, so
  that the server need not stop (its `save` first, through MCP).
- `compare` tells two dumps' differences: their lines by kind, and the functions' differing fields.
The dumps carry the community's names, and stay outside the repository like the project (docs/08 §3).

Example
  uv run python -I tools/ghidra/project.py build --project-dir C:\\Temp\\GhidraNew --dump-dir C:\\Temp\\dumps
  uv run python -I tools/ghidra/project.py dump --out C:\\Temp\\dumps --tag orig
  uv run python -I tools/ghidra/project.py compare C:\\Temp\\dumps\\Sacred.exe.orig.dump C:\\Temp\\dumps\\Sacred.exe.rebuilt.dump
  uv run python -I tools/ghidra/project.py build --replace        # in place of the server's project, the server stopped

Expected output (2026-11-26, Ghidra 12.2, a Core i9-13900KF): `build` in 5 to 8 minutes, its dump against the
project of 2026-10-18 differing in 22 of 209,642 lines for `Sacred.exe` and in 2 of 75,544 for `GameServer.exe`.
Dependencies: standard library; Ghidra (`GHIDRA_INSTALL_DIR`) and Git.
"""
import argparse
import collections
import contextlib
import hashlib
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from datetime import datetime
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'common'))
from installation import game_dir  # noqa: E402
from mcp_bridge import PROJECT_NAME, REPOSITORY, default_project_dir  # noqa: E402

MANIFEST = REPOSITORY / 'docs' / 'manifest-gog-2.28.txt'
# The executables and their programs' names of pyghidra-mcp's first start (docs/03 §3.2), which tools/README.md's
# commands and the sessions use.
PROGRAMS = {'Sacred.exe': 'Sacred.exe-d98109', 'GameServer.exe': 'GameServer.exe-6233cd'}
SACRED, SERVER = PROGRAMS.values()
# The community's sources of docs/03 §4.1, step 3, at the commits of the project of 2026-10-19 (community_names.py:
# 193 sacred-bild names, 76 sacred-sdk names, 266 addresses).
COMMUNITY = (
    ('sacred-bild', 'https://github.com/kirides/sacred-bild', 'e7d01f8681312482eab8030b9314c84f2d9f2f43'),
    ('sacred-sdk', 'https://github.com/bssth/sacred-sdk', '7b1fb4e060ab03748377b1f22828db379efbe3ca'),
)
# The library's ranges of docs/03 §4.1, step 4 (docs/01 §3.2), each end exclusive.
LIBRARY = ('LIBRARY:7F1180-811000', 'LIBRARY:818000-85AB5A', 'EH:85ACD2-88F629')
# What the headless analyzer logs when a script throws or an import fails; its exit code stays 0 for a script.
FAILED = re.compile(r'SCRIPT ERROR|Exception in thread|REPORT: Import failed')
# The fields of a dump's function line (DumpMarkupScript.java).
FIELDS = ('kind', 'entry', 'name', 'source', 'prototype', 'convention', 'noreturn', 'inline', 'custom storage',
          'varargs', 'thunk', 'purge', 'locals', 'body', 'tags', 'comment', 'repeatable comment')


def log(text):
    print(text, flush=True)


def user_environment(name):
    """A user environment variable as Windows stores it, for a shell started before it was set (docs/03 §3.2)."""
    if sys.platform != 'win32':
        return None
    import winreg
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, 'Environment') as key:
            value, _ = winreg.QueryValueEx(key, name)
    except OSError:
        return None
    return value or None


def headless_path(ghidra, platform=sys.platform):
    """Ghidra's headless analyzer in its directory, the batch file on Windows and the shell script elsewhere."""
    return Path(ghidra) / 'support' / ('analyzeHeadless.bat' if platform == 'win32' else 'analyzeHeadless')


def manifest_hashes(text):
    """The manifest's SHA-256 of each file by its path (`sha256sum` lines, the binary mark `*` allowed)."""
    hashes = {}
    for line in text.splitlines():
        m = re.match(r'^([0-9a-f]{64})\s+\*?(.+)$', line.strip())
        if m:
            hashes[m.group(2)] = m.group(1)
    return hashes


def sha256(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as f:
        for block in iter(lambda: f.read(1 << 20), b''):
            digest.update(block)
    return digest.hexdigest()


def checked_executables(game, hashes):
    """The installation's executables of PROGRAMS, each checked against the manifest's hash."""
    found = {}
    for exe in PROGRAMS:
        path = Path(game) / exe
        if not path.is_file():
            raise SystemExit(f'{path} is not there')
        digest = sha256(path)
        if digest != hashes.get(exe):
            raise SystemExit(f'{path} is not the GOG 2.28 release\'s (SHA-256 {digest}, the manifest\'s {hashes.get(exe)})')
        found[exe] = path
    return found


def project_paths(project_dir, name):
    """The project's own entries in its directory that are there: its .gpr, its .rep and pyghidra-mcp's index."""
    return [path for path in (Path(project_dir) / f'{name}.gpr', Path(project_dir) / f'{name}.rep',
                              Path(project_dir) / f'{name}-pyghidra-mcp') if path.exists()]


@contextlib.contextmanager
def paused(project_dir):
    """`pyghidra-mcp.paused` in the project's directory while it is made, so that no bridge starts the server on it;
    one that was there already stays."""
    path = Path(project_dir) / 'pyghidra-mcp.paused'
    made = not path.exists()
    if made:
        path.touch()
    try:
        yield path
    finally:
        if made:
            path.unlink(missing_ok=True)


def build_steps(executables, scripts, names, dump=None):
    """The headless runs of docs/03 §4.1 in order, each a step's name and its arguments after the project's."""
    run = ['-noanalysis', '-scriptPath', str(scripts)]
    steps = [
        ('import', ['-import', *(str(path) for path in executables.values())]),
        ('rename-sacred', ['-process', 'GameServer.exe', *run, '-postScript', 'RenameProgramScript.java',
                           '/Sacred.exe', SACRED]),
        ('rename-server', ['-process', SACRED, *run, '-postScript', 'RenameProgramScript.java',
                           '/GameServer.exe', SERVER]),
        ('server-classes', ['-process', SERVER, *run, '-postScript', 'RttiClassesScript.java',
                            '-postScript', 'MarkLibraryScript.java']),
        ('sacred', ['-process', SACRED, *run, '-postScript', 'RttiClassesScript.java',
                    '-postScript', 'ImportNamesScript.java', str(names),
                    '-postScript', 'MarkLibraryScript.java', *LIBRARY,
                    '-postScript', 'CarryNamesScript.java', f'/{SERVER}',
                    '-postScript', 'ProjectStatsScript.java']),
        ('server-names', ['-process', SERVER, *run, '-postScript', 'CarryNamesScript.java', f'/{SACRED}',
                          '-postScript', 'ProjectStatsScript.java']),
    ]
    if dump is not None:
        steps.append(('dump', dump_arguments(scripts, dump, 'rebuilt')))
    return steps


def dump_arguments(scripts, out, tag):
    return ['-process', '-noanalysis', '-readOnly', '-scriptPath', str(scripts),
            '-postScript', 'DumpMarkupScript.java', str(out), tag]


def script_lines(text):
    """The lines that the scripts printed, out of a headless run's log."""
    lines = []
    for line in text.splitlines():
        if 'Script.java> ' in line:
            line = re.sub(r'^INFO\s+', '', line.strip())
            lines.append(re.sub(r'\s*\(GhidraScript\)$', '', line))
    return lines


class Headless:
    """Ghidra's headless analyzer on one project, each run's output in a log of its own."""

    def __init__(self, ghidra, project_dir, name, logs, max_memory):
        self.program = headless_path(ghidra)
        if not self.program.is_file():
            raise SystemExit(f'{self.program} is not there')
        self.project_dir, self.name, self.logs = Path(project_dir), name, Path(logs)
        self.env = dict(os.environ, GHIDRA_HEADLESS_MAXMEM=max_memory)

    def run(self, step, arguments):
        path = self.logs / f'{step}.log'
        started = time.monotonic()
        with open(path, 'wb') as out:
            # No input: the batch file pauses for a key after an error.
            code = subprocess.call([str(self.program), str(self.project_dir), self.name, *arguments],
                                   stdin=subprocess.DEVNULL, stdout=out, stderr=subprocess.STDOUT, env=self.env)
        text = path.read_text(encoding='utf-8', errors='replace')
        if code != 0 or FAILED.search(text):
            raise SystemExit(f'Ghidra\'s run {step} failed (exit {code}); see {path}')
        for line in script_lines(text):
            log('   ' + line)
        log(f'   ({step}: {time.monotonic() - started:.0f} s)')


def stage_scripts(directory):
    """This directory's Ghidra scripts in one directory: analyzeHeadless.bat splits a -scriptPath of several at `;`."""
    for script in HERE.glob('*.java'):
        shutil.copy2(script, directory)


def fetch_community(directory):
    """The community's sources at their pinned commits, each into a directory of its own."""
    for name, url, commit in COMMUNITY:
        target = Path(directory) / name
        for command in (['git', 'init', '-q', str(target)],
                        ['git', '-C', str(target), 'fetch', '-q', '--depth', '1', url, commit],
                        ['git', '-C', str(target), 'checkout', '-q', 'FETCH_HEAD']):
            subprocess.run(command, check=True)


def ghidra_dir(given):
    found = given or os.environ.get('GHIDRA_INSTALL_DIR') or user_environment('GHIDRA_INSTALL_DIR')
    if not found:
        raise SystemExit('GHIDRA_INSTALL_DIR is not set: --ghidra-dir names the Ghidra directory')
    return Path(found)


def build(args):
    executables = checked_executables(args.game_dir or game_dir(), manifest_hashes(MANIFEST.read_text('utf-8')))
    log(f'== the installation: {", ".join(str(path) for path in executables.values())}, as the manifest has them')
    project_dir = Path(args.project_dir)
    project_dir.mkdir(parents=True, exist_ok=True)
    own = project_paths(project_dir, args.project_name)
    if own and not args.replace:
        raise SystemExit(f'a project is there already ({", ".join(map(str, own))}): --replace moves it aside first')
    if own and (project_dir / f'{args.project_name}.lock').exists():
        raise SystemExit(f'the project is open ({args.project_name}.lock): stop pyghidra-mcp first (docs/03 §3.2)')
    started = time.monotonic()
    with (tempfile.TemporaryDirectory(prefix='opensacred-ghidra-', ignore_cleanup_errors=True) as work,
          paused(project_dir)):
        work = Path(work)
        logs, scripts, community = work / 'logs', work / 'scripts', work / 'community'
        for directory in (logs, scripts, community):
            directory.mkdir()
        if own:
            aside = project_dir / f'{args.project_name}.old-{datetime.now():%Y%m%d-%H%M%S}'
            aside.mkdir()
            for path in own:
                shutil.move(str(path), str(aside / path.name))
            log(f'== the old project moved into {aside}')
        stage_scripts(scripts)
        log('== the community\'s names')
        fetch_community(community)
        names = community / 'community-names.tsv'
        subprocess.run([sys.executable, '-I', str(HERE / 'community_names.py'), '--community', str(community),
                        '--out', str(names)], check=True)
        dumps = None
        if args.dump_dir:
            dumps = Path(args.dump_dir).resolve()
            dumps.mkdir(parents=True, exist_ok=True)
        headless = Headless(ghidra_dir(args.ghidra_dir), project_dir, args.project_name, logs, args.max_memory)
        try:
            for step, arguments in build_steps(executables, scripts, names, dumps):
                log(f'== {step}')
                headless.run(step, arguments)
        except SystemExit:
            kept = Path(tempfile.gettempdir()) / f'opensacred-ghidra-logs-{datetime.now():%Y%m%d-%H%M%S}'
            shutil.copytree(logs, kept)
            log(f'the logs are kept in {kept}')
            raise
    log(f'== done in {(time.monotonic() - started) / 60:.1f} min: {project_dir / args.project_name}.gpr; '
        'pyghidra-mcp makes its index of search_code again at its next start')


def dump(args):
    project_dir = Path(args.project_dir)
    own = project_paths(project_dir, args.project_name)
    if not any(path.suffix in ('.gpr', '.rep') for path in own):
        raise SystemExit(f'no project {args.project_name} in {project_dir}')
    out = Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='opensacred-ghidra-', ignore_cleanup_errors=True) as work:
        work = Path(work)
        copy, logs, scripts = work / 'project', work / 'logs', work / 'scripts'
        for directory in (copy, logs, scripts):
            directory.mkdir()
        # A copy as the last save left it: the files of the changes in progress (*.ps) and the locks stay behind.
        shutil.copy2(project_dir / f'{args.project_name}.gpr', copy)
        shutil.copytree(project_dir / f'{args.project_name}.rep', copy / f'{args.project_name}.rep',
                        ignore=shutil.ignore_patterns('*.ps', '*.lock', '*.lock~'))
        stage_scripts(scripts)
        headless = Headless(ghidra_dir(args.ghidra_dir), copy, args.project_name, logs, args.max_memory)
        headless.run('dump', dump_arguments(scripts, out, args.tag))


def compare_lines(first, second, examples=5):
    """The report of two dumps' differences: the lines that only one has, by kind, and the functions' fields."""
    a, b = collections.Counter(first), collections.Counter(second)
    only_first, only_second = a - b, b - a
    report = [f'{len(first)} and {len(second)} lines; only in the first {sum(only_first.values())}, '
              f'only in the second {sum(only_second.values())}']
    kind = lambda line: line.split('\t', 1)[0]  # noqa: E731
    totals = collections.Counter(kind(line) for line in first)
    for k in sorted({kind(line) for line in only_first} | {kind(line) for line in only_second}):
        lost = sorted(line for line in only_first.elements() if kind(line) == k)
        extra = sorted(line for line in only_second.elements() if kind(line) == k)
        report.append(f'== {k}: {totals[k]} in the first, {len(lost)} only there, {len(extra)} only in the second')
        report += ['  - ' + line[:300] for line in lost[:examples]]
        report += ['  + ' + line[:300] for line in extra[:examples]]
    functions = lambda lines: {f[1]: f for f in (line.split('\t') for line in lines) if f[0] == 'F'}  # noqa: E731
    fa, fb = functions(first), functions(second)
    report.append(f'== functions: {len(fa.keys() - fb.keys())} only in the first, '
                  f'{len(fb.keys() - fa.keys())} only in the second')
    differ, example = collections.Counter(), {}
    for entry in sorted(fa.keys() & fb.keys()):
        for i, (x, y) in enumerate(zip(fa[entry], fb[entry])):
            if x != y:
                differ[FIELDS[i]] += 1
                example.setdefault(FIELDS[i], (entry, x, y))
    for field, count in differ.most_common():
        entry, x, y = example[field]
        report.append(f'  {field}: {count}, e.g. {entry}: {x[:70]!r} -> {y[:70]!r}')
    return report


def read_dump(path):
    with open(path, encoding='utf-8', newline='') as f:
        return [line for line in f.read().split('\n') if line]


def compare(args):
    for line in compare_lines(read_dump(args.first), read_dump(args.second), args.examples):
        print(line)


def main():
    parser = argparse.ArgumentParser(description='The local Ghidra project: rebuilt, its markup dumped and compared.')
    commands = parser.add_subparsers(dest='command', required=True)

    def project_options(command):
        command.add_argument('--project-dir', default=default_project_dir(),
                             help='the project\'s directory (default %%USERPROFILE%%\\GhidraProjects)')
        command.add_argument('--project-name', default=PROJECT_NAME)
        command.add_argument('--ghidra-dir', help='Ghidra\'s directory (default GHIDRA_INSTALL_DIR)')
        command.add_argument('--max-memory', default='6G', help='the headless analyzer\'s heap')

    command = commands.add_parser('build', help='rebuild the project from the installation (docs/03 §4.1)')
    project_options(command)
    command.add_argument('--game-dir', help='the installation (default the one tools/common/installation.py finds)')
    command.add_argument('--replace', action='store_true', help='move a project already there aside first')
    command.add_argument('--dump-dir', help='write each program\'s markup dump there afterwards, tagged rebuilt')
    command.set_defaults(run=build)

    command = commands.add_parser('dump', help='dump each program\'s markup from a copy of the project')
    project_options(command)
    command.add_argument('--out', required=True, help='the directory of the dumps, outside the repository')
    command.add_argument('--tag', default='orig', help='the dumps\' tag: <executable>.<tag>.dump')
    command.set_defaults(run=dump)

    command = commands.add_parser('compare', help='compare two dumps of one executable')
    command.add_argument('first')
    command.add_argument('second')
    command.add_argument('--examples', type=int, default=5, help='lines shown of each kind')
    command.set_defaults(run=compare)

    args = parser.parse_args()
    args.run(args)


if __name__ == '__main__':
    main()
