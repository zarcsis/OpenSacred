# Third-party notices

OpenSacred contains no source files copied from other projects. Some of its readers follow facts
published by the Sacred community: format layouts, constants, tag tables and the names of fields.
Where a constant table or a formula comes from a project under a license that asks for its notice,
the notice is reproduced here, and the source is named in a comment next to the code.

No file of the game is part of this repository, and none is covered by these notices.

## MIT-licensed sources

Each of the following projects is distributed under the MIT License, whose text follows the list.

* **grn2glb** (https://github.com/ancaria-dev/grn2glb), the MIT-licensed core:
  Copyright (c) 2026 karolak6612; Copyright (c) 2026 MairwunNx (Pavel Erokhin).
  The Granny 1.2b tag constants and texture-format names in
  `crates/opensacred-formats/src/grn/` and `tools/analysis/formats/grn_dump.py` follow its
  `grn_types.h`, and the order in which the readers walk a file follows its `grn_parser.cpp`. Its
  VTex codec (`src/codecs/vtex_*`, LGPL-2.1-or-later) is not used: FFmpeg runs as an external
  program.
* **sacred-sdk** (https://github.com/bssth/sacred-sdk): Copyright (c) 2026 SacredSDK contributors.
  The string hash of `global.res` (`crates/opensacred-data/src/text.rs`) and facts about the
  NetScript records.
* **OpenHeilig engine and tools** (https://github.com/openheilig/engine,
  https://github.com/openheilig/tools): Copyright (c) 2026 Roman Linev and the OpenHeilig
  contributors. Container layouts such as `mixed.pak` (`crates/opensacred-formats/src/mixed.rs`).

```text
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Facts credited without a license obligation

* **OpenHeilig research** (https://github.com/openheilig/research, CC BY-SA 4.0, Copyright (c)
  2026 Roman Linev and the OpenHeilig contributors): descriptions of the NetScript bytecode, the
  Granny files, the PAK containers and the world format. Only facts are used, in our own words; no
  text is copied. The comments that rely on it name it.
* **open-grn** (https://github.com/karolak6612/open-grn, MIT): a reference on the semantics of the
  Granny 1.2b format. No code is used.
* **sacred-bild** (https://github.com/kirides/sacred-bild, GPL-3.0): the names and addresses of the
  original's functions in its Ghidra export (`functions_eng.tsv`), which the research scripts
  (`tools/analysis/pe/`, `tools/ghidra/community_names.py`) read from a path given to them and which
  name functions in the developer's local Ghidra project (docs/03 §4.1). Only those facts are used;
  no file or code of it is in the repository.

## Binary distributions

The game links Bevy with its `default_font` feature, which embeds the Fira Mono font (SIL Open Font
License 1.1). A binary release must ship that license with the executable, together with the
licenses of the other dependencies (`cargo deny list` prints them).

The release archive carries `ffmpeg.exe`, which the converter runs as a separate program: FFmpeg
(LGPL 2.1 or later) built statically with libopus (BSD-3-Clause) and SVT-AV1 (BSD-3-Clause-Clear
and the AOMedia Patent License 1.0) by `tools/ffmpeg/build.sh` from the sources that
`tools/ffmpeg/versions.toml` pins. The archive holds its `FFMPEG.md`, which names the sources and
the configuration, and the three licenses' texts, and the release offers the sources (docs/08 §4).
