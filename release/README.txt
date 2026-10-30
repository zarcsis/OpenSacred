OpenSacred
==========

An open-source engine for Sacred Gold 2.28, which plays the game from your own copy of it.
https://github.com/zarcsis/OpenSacred

This archive holds no file of the game. You need your own installation of Sacred Gold 2.28
(English) from GOG or Steam.

Playing
-------

1. Unpack the archive into a folder of its own and run opensacred-game.exe.
2. At its first start the game finds your installation of Sacred Gold (GOG's or Steam's) and makes
   its assets from it, once: it takes some minutes and 11 GB of free disk space for its time, and
   leaves 3.5 GB in %LOCALAPPDATA%\OpenSacred\assets. If it does not find the installation, it
   asks for its folder.
3. After an update of OpenSacred the game makes its assets again at its first start.

Windows may warn about the unsigned program at its first run: More info, then Run anyway.

The settings are in %APPDATA%\OpenSacred\settings.toml, the logs in
%LOCALAPPDATA%\OpenSacred\logs. To remove OpenSacred, delete this folder and
%LOCALAPPDATA%\OpenSacred.

What is in the archive
----------------------

- opensacred-game.exe, the game, and opensacred-convert.exe, the converter it runs: MIT OR
  Apache-2.0 (LICENSE-MIT, LICENSE-APACHE); THIRD-PARTY-NOTICES.md and THIRD-PARTY-LICENSES.html
  name the code and fonts they hold of others and their licenses.
- ffmpeg.exe, which the converter runs for the sound, the cutscenes and some textures: FFmpeg
  under the LGPL 2.1 or later, with libopus and SVT-AV1; FFMPEG.md names its sources, which the
  release offers beside this archive, and ffmpeg-licenses/ holds the licenses.

Sacred and Sacred Gold are trademarks of their owner. OpenSacred is not affiliated with, endorsed
by or sponsored by THQ Nordic, Ascaron Entertainment, GOG or Valve.
