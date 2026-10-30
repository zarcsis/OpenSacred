#!/usr/bin/env python3
"""Tests of tools/ffmpeg/outdated.py without the network.

Example
  uv run --no-project python -I -m unittest discover -s tools/ffmpeg -p "test_*.py"

Dependencies: standard library.
"""
import sys, tomllib, unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import outdated  # noqa: E402


class Newest(unittest.TestCase):
    def test_the_newest_release_by_its_numbers(self):
        refs = ['refs/tags/n9.0', 'refs/tags/n9.0.2', 'refs/tags/n10.0-dev', 'refs/tags/n9.0.10', 'refs/tags/n4.4']
        self.assertEqual(outdated.newest(refs, outdated.SOURCES['ffmpeg'][1]), '9.0.10')
        self.assertEqual(outdated.newest(['v4.2.0', 'v4.10.0', 'v4.2.0-rc1'], outdated.SOURCES['svt-av1'][1]), '4.10.0')
        self.assertIsNone(outdated.newest(['refs/tags/v1.6-rc1'], outdated.SOURCES['opus'][1]))

    def test_every_pinned_source_is_followed(self):
        pins = tomllib.loads(outdated.VERSIONS.read_text(encoding='utf-8'))
        self.assertEqual(set(pins), set(outdated.SOURCES))
        for name, pin in pins.items():
            self.assertIn(pin['version'], pin['url'], name)
            self.assertEqual(len(pin['sha256']), 64, name)


if __name__ == '__main__':
    unittest.main()
