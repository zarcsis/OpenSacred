#!/usr/bin/env python3
"""Tests of tools/ghidra/project.py without Ghidra: the manifest's hashes, the executables' check, the project's place,
the pause, the runs of docs/03 §4.1 in their order, the scripts' lines of a log and the comparison of two dumps.

Example
  uv run python -I -m unittest discover -s tools/ghidra -p "test_*.py"

Dependencies: standard library.
"""
import hashlib, sys, tempfile, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import project  # noqa: E402


class Manifest(unittest.TestCase):
    def test_hashes_by_path(self):
        a, b = 'a' * 64, 'b' * 64
        hashes = project.manifest_hashes(f'{a}  Sacred.exe\n{b} *PAK/x.pak\nnot a line\n')
        self.assertEqual(hashes, {'Sacred.exe': a, 'PAK/x.pak': b})

    def test_the_executables_checked(self):
        with tempfile.TemporaryDirectory() as game:
            hashes = {}
            for exe in project.PROGRAMS:
                data = exe.encode()
                (Path(game) / exe).write_bytes(data)
                hashes[exe] = hashlib.sha256(data).hexdigest()
            found = project.checked_executables(game, hashes)
            self.assertEqual(list(found), list(project.PROGRAMS))
            hashes['GameServer.exe'] = '0' * 64
            with self.assertRaisesRegex(SystemExit, 'not the GOG 2.28'):
                project.checked_executables(game, hashes)
            (Path(game) / 'Sacred.exe').unlink()
            with self.assertRaisesRegex(SystemExit, 'is not there'):
                project.checked_executables(game, hashes)


class Place(unittest.TestCase):
    def test_the_projects_own_entries(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(project.project_paths(directory, 'P'), [])
            (Path(directory) / 'P.gpr').touch()
            (Path(directory) / 'P.rep').mkdir()
            (Path(directory) / 'Q.gpr').touch()
            self.assertEqual([p.name for p in project.project_paths(directory, 'P')], ['P.gpr', 'P.rep'])

    def test_the_pause_made_and_taken_away(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'pyghidra-mcp.paused'
            with project.paused(directory):
                self.assertTrue(path.exists())
            self.assertFalse(path.exists())

    def test_a_pause_there_already_stays(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'pyghidra-mcp.paused'
            path.touch()
            with project.paused(directory):
                pass
            self.assertTrue(path.exists())

    def test_the_headless_analyzer_by_platform(self):
        self.assertEqual(project.headless_path('G', 'win32').name, 'analyzeHeadless.bat')
        self.assertEqual(project.headless_path('G', 'linux').name, 'analyzeHeadless')


class Steps(unittest.TestCase):
    def steps(self, dump=None):
        executables = {'Sacred.exe': Path('g/Sacred.exe'), 'GameServer.exe': Path('g/GameServer.exe')}
        return project.build_steps(executables, Path('s'), Path('n.tsv'), dump)

    def scripts(self, arguments):
        return [arguments[i + 1] for i, a in enumerate(arguments) if a == '-postScript']

    def test_the_order_of_docs_03(self):
        steps = dict(self.steps())
        self.assertEqual(list(steps), ['import', 'rename-sacred', 'rename-server', 'server-classes', 'sacred',
                                       'server-names'])
        self.assertEqual(steps['import'], ['-import', str(Path('g/Sacred.exe')), str(Path('g/GameServer.exe'))])
        self.assertEqual(self.scripts(steps['server-classes']), ['RttiClassesScript.java', 'MarkLibraryScript.java'])
        self.assertEqual(self.scripts(steps['sacred']), ['RttiClassesScript.java', 'ImportNamesScript.java',
                                                         'MarkLibraryScript.java', 'CarryNamesScript.java',
                                                         'ProjectStatsScript.java'])
        self.assertEqual(self.scripts(steps['server-names']), ['CarryNamesScript.java', 'ProjectStatsScript.java'])

    def test_each_script_takes_its_arguments(self):
        steps = dict(self.steps())
        sacred = steps['sacred']
        self.assertEqual(sacred[sacred.index('ImportNamesScript.java') + 1], 'n.tsv')
        start = sacred.index('MarkLibraryScript.java') + 1
        self.assertEqual(tuple(sacred[start:start + 3]), project.LIBRARY)
        self.assertEqual(sacred[sacred.index('CarryNamesScript.java') + 1], '/GameServer.exe-6233cd')
        names = steps['server-names']
        self.assertEqual(names[names.index('CarryNamesScript.java') + 1], '/Sacred.exe-d98109')
        self.assertEqual(steps['rename-sacred'][-2:], ['/Sacred.exe', 'Sacred.exe-d98109'])
        self.assertEqual(steps['rename-server'][-2:], ['/GameServer.exe', 'GameServer.exe-6233cd'])
        for step, arguments in steps.items():
            if step != 'import':
                self.assertIn('-noanalysis', arguments, step)

    def test_the_dump_last_and_read_only(self):
        step, arguments = self.steps(Path('d'))[-1]
        self.assertEqual(step, 'dump')
        self.assertIn('-readOnly', arguments)
        self.assertEqual(arguments[-3:], ['DumpMarkupScript.java', 'd', 'rebuilt'])


class Logs(unittest.TestCase):
    def test_the_scripts_lines(self):
        text = ('INFO  HEADLESS: execution starts (HeadlessAnalyzer)\n'
                'INFO  ProjectStatsScript.java> stats Sacred.exe: 1 functions (GhidraScript)  \n'
                'ERROR Failed to create function (FunctionsXmlMgr)\n')
        self.assertEqual(project.script_lines(text), ['ProjectStatsScript.java> stats Sacred.exe: 1 functions'])

    def test_a_failed_script_found(self):
        self.assertTrue(project.FAILED.search('ERROR REPORT SCRIPT ERROR:  ImportNamesScript.java : x'))
        self.assertFalse(project.FAILED.search('ERROR Failed to create function at 0041e510'))


class Compare(unittest.TestCase):
    def test_the_differences_by_kind_and_field(self):
        f = 'F\t00401000\tFUN_00401000\tDEFAULT\tundefined FUN_00401000()\tunknown\t\t\t\t\t\tpurge=0\tlocals=4\t' \
            'body=00401000-0040100f/1/16\ttags=\t\t'
        first = [f, 'C\t00401000\tEOL\tx', 'S\t00401000\tFunction\tFUN_00401000\tDEFAULT\tprimary\t']
        second = [f.replace('tags=', 'tags=LIBRARY'), 'C\t00401000\tEOL\tx']
        report = project.compare_lines(first, second)
        self.assertEqual(report[0], '3 and 2 lines; only in the first 2, only in the second 1')
        self.assertIn('== F: 1 in the first, 1 only there, 1 only in the second', report)
        self.assertIn('== S: 1 in the first, 1 only there, 0 only in the second', report)
        self.assertNotIn('== C', '\n'.join(report))
        self.assertTrue(any(line.startswith('  tags: 1, e.g. 00401000') for line in report))

    def test_the_same_dumps(self):
        report = project.compare_lines(['a\tb'], ['a\tb'])
        self.assertEqual(report, ['1 and 1 lines; only in the first 0, only in the second 0',
                                  '== functions: 0 only in the first, 0 only in the second'])


if __name__ == '__main__':
    unittest.main()
