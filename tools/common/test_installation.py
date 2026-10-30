"""Tests of installation.py (#1556): the developer's installation, OriginalGame/ before the registry's.

Run:
  uv run --no-project python -I -m unittest discover -s tools/common -p "test_*.py"
"""
import os
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import installation  # noqa: E402


def make_installation(parent, name):
    """A directory holding a Sacred.exe."""
    path = os.path.join(parent, name)
    os.makedirs(path)
    with open(os.path.join(path, 'Sacred.exe'), 'wb'):
        pass
    return path


class GameDir(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)

    def test_the_repository_s_link_comes_first(self):
        repository = make_installation(self.temp.name, 'OriginalGame')
        gog = make_installation(self.temp.name, 'GOG')
        with mock.patch.object(installation, 'REPOSITORY', repository), \
                mock.patch.object(installation, 'gog_dirs', return_value=[gog]):
            self.assertEqual(installation.game_dir(), repository)

    def test_without_the_link_the_registry_s_gog_installation_holding_a_sacred_exe(self):
        repository = os.path.join(self.temp.name, 'OriginalGame')
        empty = os.path.join(self.temp.name, 'Uninstalled')
        os.makedirs(empty)
        gog = make_installation(self.temp.name, 'GOG')
        with mock.patch.object(installation, 'REPOSITORY', repository), \
                mock.patch.object(installation, 'gog_dirs', return_value=[empty, gog]):
            self.assertEqual(installation.game_dir(), gog)

    def test_none_found_names_the_repository_s_link(self):
        repository = os.path.join(self.temp.name, 'OriginalGame')
        with mock.patch.object(installation, 'REPOSITORY', repository), \
                mock.patch.object(installation, 'gog_dirs', return_value=[]):
            self.assertEqual(installation.game_dir(), repository)

    def test_the_registry_reads_without_failing(self):
        # On any machine: a list of strings, empty where GOG's key is not there.
        found = installation.gog_dirs()
        self.assertIsInstance(found, list)
        self.assertTrue(all(isinstance(path, str) for path in found))


if __name__ == '__main__':
    unittest.main()
