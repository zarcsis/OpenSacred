#!/usr/bin/env python3
"""Tests of tools/deps/daily.py without the network, cargo or gh.

Example
  uv run --no-project python -I -m unittest discover -s tools/deps -p "test_*.py"

Dependencies: standard library.
"""
import json, sys, tomllib, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import daily  # noqa: E402

CARGO = """\
    Updating crates.io index
     Locking 4 packages to highest Rust 1.99 compatible versions
    Updating js-sys v0.3.106 -> v0.3.107
 Downgrading foo v1.2.0 -> v1.1.9
      Adding utf_types v1.0.0
    Removing old-thing v0.1.0
   Unchanged bevy v0.20.0 (available: v0.21.0)
   Unchanged zstd v0.13.3 (available: v0.14.3, requires Rust 1.100)
   Unchanged rand v0.8.5 (available: v0.9.2)
note: to see how you depend on a package, run `cargo tree --invert <dep>@<ver>`
"""

UV = """\
Resolved 7 packages in 302ms
Updated frida v17.24.3 -> v17.25.0
Added tiny v1.0.0
Removed gone v2.0.0
"""

TREE = """\
opensacred-tools v0
├── capstone v5.0.9 (latest: v6.0.0)
├── frida v17.25.0
└── pillow v13.0.0 (latest: v14.1.0)
"""


class Parsing(unittest.TestCase):
    def test_the_changes_of_cargo_and_uv(self):
        self.assertEqual(daily.changes(daily.CARGO_CHANGE, CARGO), [
            ('Updating', 'js-sys', '0.3.106', '0.3.107'), ('Downgrading', 'foo', '1.2.0', '1.1.9'),
            ('Adding', 'utf_types', '1.0.0', None), ('Removing', 'old-thing', '0.1.0', None)])
        self.assertEqual(daily.changes(daily.UV_CHANGE, UV), [
            ('Update', 'frida', '17.24.3', '17.25.0'), ('Add', 'tiny', '1.0.0', None), ('Remove', 'gone', '2.0.0', None)])
        self.assertEqual(daily.changes(daily.UV_CHANGE, 'Update frida v17.24.3 -> v17.25.0\n'),
                         [('Update', 'frida', '17.24.3', '17.25.0')])

    def test_the_releases_left_behind(self):
        self.assertEqual(daily.CARGO_BEHIND.findall(CARGO), [
            ('bevy', '0.20.0', '0.21.0'), ('zstd', '0.13.3', '0.14.3'), ('rand', '0.8.5', '0.9.2')])
        self.assertEqual(daily.UV_BEHIND.findall(TREE), [('capstone', '5.0.9', '6.0.0'), ('pillow', '13.0.0', '14.1.0')])
        ffmpeg = 'ffmpeg 8.0.1: up to date\nopus 1.5.2: newer: 1.6\n'
        self.assertEqual(daily.FFMPEG_BEHIND.findall(ffmpeg), [('opus', '1.5.2', '1.6')])

    def test_the_direct_dependencies(self):
        manifest = """\
[workspace.dependencies]
opensacred-data = { path = "crates/opensacred-data" }
anyhow = "1.0.104"
bevy = { version = "0.20.0", default-features = false }
renamed = { version = "1", package = "real-name" }
"""
        self.assertEqual(daily.direct_crates(manifest), {'anyhow', 'bevy', 'real-name'})
        project = '[project]\ndependencies = ["pefile>=2024.8.26", "capstone>=5.0.9,<6", "frida[tools]>=17"]\n'
        self.assertEqual(daily.direct_packages(project), {'pefile', 'capstone', 'frida'})

    def test_the_repository_parses(self):
        self.assertIn('bevy', daily.direct_crates((daily.ROOT / 'Cargo.toml').read_text(encoding='utf-8')))
        self.assertIn('frida', daily.direct_packages((daily.ROOT / 'pyproject.toml').read_text(encoding='utf-8')))
        holds = tomllib.loads(daily.HOLDS.read_text(encoding='utf-8'))
        for ecosystem in ('cargo', 'uv'):
            for name, hold in holds[ecosystem].items():
                self.assertIsInstance(hold['issue'], int, name)
                self.assertTrue(hold['version'], name)

    def test_rust_and_python(self):
        channel = '[pkg.rust]\nversion = "1.100.0 (ded0c5bd8 2026-11-27)"\n'
        self.assertEqual(daily.rust_latest(channel), '1.100.0')
        self.assertGreater(daily.key('1.100.0'), daily.key('1.99.1'))

        def entry(version, implementation='cpython'):
            major, minor, patch = (int(part) for part in version.split('a')[0].split('.'))
            return {'version': version, 'implementation': implementation,
                    'version_parts': {'major': major, 'minor': minor, 'patch': patch}}

        listing = [entry('3.16.0a1'), entry('3.15.2'), entry('3.14.6'), entry('3.17.0', 'pypy')]
        self.assertIsNone(daily.python_latest(json.dumps(listing), '3.15'))
        self.assertEqual(daily.python_latest(json.dumps(listing + [entry('3.16.0')]), '3.15'), '3.16')


class Reports(unittest.TestCase):
    def test_why_an_update_did_not_merge(self):
        failed = {'statusCheckRollup': [
            {'name': 'Linux', 'status': 'COMPLETED', 'conclusion': 'FAILURE', 'detailsUrl': 'https://ci/1'},
            {'name': 'Windows', 'status': 'COMPLETED', 'conclusion': 'SUCCESS'}]}
        self.assertEqual(daily.failures(failed), 'its checks failed: Linux (https://ci/1)')
        running = {'statusCheckRollup': [{'name': 'Windows', 'status': 'IN_PROGRESS', 'conclusion': ''}]}
        self.assertEqual(daily.failures(running), 'its checks were still running a day on: Windows')
        passed = {'statusCheckRollup': [{'context': 'cargo deny', 'state': 'SUCCESS'}], 'mergeStateStatus': 'BLOCKED'}
        self.assertEqual(daily.failures(passed), 'its checks passed, but it did not merge (merge state BLOCKED)')

    def test_the_pull_request_lists_the_direct_dependencies_first(self):
        cargo = [('Updating', 'js-sys', '0.3.106', '0.3.107'), ('Updating', 'bevy', '0.20.0', '0.20.1'),
                 ('Adding', 'utf_types', '1.0.0', None)]
        body = daily.pr_body('2026-10-31', cargo, [], {'bevy'}, [('cargo', 'luau0-src', '0.22.0', 869)], 'RUN')
        self.assertIn('- `bevy` 0.20.0 → 0.20.1 (direct)\n- `js-sys` 0.3.106 → 0.3.107\n- `utf_types` 1.0.0 added\n',
                      body)
        self.assertIn('## `uv lock --upgrade`\n\nNothing changed.\n', body)
        self.assertIn('- `luau0-src` at 0.22.0 (cargo, #869)', body)


if __name__ == '__main__':
    unittest.main()
