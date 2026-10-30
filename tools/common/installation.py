"""The developer's installation of Sacred Gold 2.28 for the tools (#1556), found as
`opensacred-locate` finds it for the Rust tools and tests: the repository's ``OriginalGame/`` where
it holds a ``Sacred.exe``, else the GOG installation that the registry names (GOG's key
``1207658688``, its 32-bit view's first), the GOG release alone (#1484); else ``OriginalGame/``
still, which the tools then report missing.

Usage from a script (``uv run python -I`` does not add the script's directory to sys.path)::

    import os, sys
    sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'common'))
    from installation import game_dir

Dependencies: standard library.
"""
import os
import sys

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
# The repository's link to the installation, ignored by Git.
REPOSITORY = os.path.join(ROOT, 'OriginalGame')
GOG_KEYS = (
    r'SOFTWARE\WOW6432Node\GOG.com\Games\1207658688',
    r'SOFTWARE\GOG.com\Games\1207658688',
)


def gog_dirs():
    """The directories that GOG's installer wrote into the registry, none outside Windows."""
    if sys.platform != 'win32':
        return []
    import winreg
    found = []
    for key in GOG_KEYS:
        try:
            with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, key) as opened:
                value, _ = winreg.QueryValueEx(opened, 'path')
        except OSError:
            continue
        if isinstance(value, str) and value.strip() and value not in found:
            found.append(value)
    return found


def game_dir():
    """The developer's installation: OriginalGame/, else the registry's GOG one, else OriginalGame/."""
    if os.path.isfile(os.path.join(REPOSITORY, 'Sacred.exe')):
        return REPOSITORY
    for found in gog_dirs():
        if os.path.isfile(os.path.join(found, 'Sacred.exe')):
            return found
    return REPOSITORY


if __name__ == '__main__':
    print(game_dir())
