# Tools

## Summary

* This directory holds the Python scripts used to research the binary and data of Sacred Gold 2.28 (GOG), the Ghidra project's scripts (`ghidra/`), the process tools (`usage/`, `stack/`), the game's remote-control client and its conformance tools (`game/`) and the original's driver (`original/`).
  The research scripts are an archive and a reference for porting the parsing logic to Rust: the product does not use Python (PLAN.md, ADR-03).
* The research scripts only read files in `OriginalGame/` and write nothing to the repository; outputs go to a path that is passed explicitly.
  The process tools work on Git and GitHub, as their rows say: `stack/stack.py` commits and pushes branches, `usage/pr_usage.py --update` edits a pull request's description.
* The scanners read the GOG executables as is.
  Only the GOG build 2.28 is supported; the network protocol is out of scope.
* The Rust converter `opensacred-convert` does the conversion; the format-parsing scripts are kept as the reference the Rust readers are cross-checked against: the tests of `opensacred-formats` that need the game files run the prototypes and compare their output record by record.

## How to run

* [uv](https://docs.astral.sh/uv/) brings the Python of the repository's own (docs/03 §2): Python 3.15 (tested on 3.15.1), uv's interpreter rather than the system's, and the packages of `pyproject.toml` at their versions in `uv.lock`, in `.venv/` at the root.
  `uv run` makes the environment on its first run and keeps it at `uv.lock`; nothing is installed by hand.
* Run from the repository root with `uv run python -I` (isolated mode): game files are untrusted data, and the interpreter must not pick up modules from data directories.
* The scripts read your own copy of the GOG installation: `./OriginalGame` by default (ignored by Git), or the directory given with `--game-dir`.
  Output files go to the `--out` path outside the repository.

```bash
uv run python -I tools/triage/pe_triage.py OriginalGame/Sacred.exe
uv run python -I tools/analysis/pe/fn_context.py OriginalGame/Sacred.exe --str "cEngine::simThread" --max 1
uv run python -I tools/analysis/pe/pe_deep.py OriginalGame/Sacred.exe --no-disasm --out /tmp/analysis/rtti.json
```

## The Ghidra project's scripts

`tools/ghidra/` holds the Ghidra scripts (Java, compiled by Ghidra as it runs them) that build up the local project of docs/03 §4.1, the merger of the community's names, and the bridge through which every Claude Code session reaches the pyghidra-mcp server, starting it when it is down (docs/03 §3.2).
The scripts run on the project with the server stopped and `pyghidra-mcp.paused` in the project's directory, so that no session starts it again meanwhile, and their outputs stay in the project, never in the repository.

```powershell
$g = "$env:LOCALAPPDATA\Programs\Ghidra\ghidra_12.2_PUBLIC\support\analyzeHeadless.bat"
& $g "$env:USERPROFILE\GhidraProjects" OpenSacred -process Sacred.exe-d98109 -noanalysis -scriptPath tools\ghidra `
  -postScript RttiClassesScript.java `
  -postScript ImportNamesScript.java <dir>\community-names.tsv `
  -postScript MarkLibraryScript.java LIBRARY:7F1180-811000 LIBRARY:818000-85AB5A EH:85ACD2-88F629 `
  -postScript CarryNamesScript.java /GameServer.exe-6233cd `
  -postScript ProjectStatsScript.java
```

| Script | What it does |
|---|---|
| `ghidra/RttiClassesScript.java` | classes from the MSVC RTTI type descriptors: the locators, the vftables, a class for each, its virtual functions and constructors moved into it |
| `ghidra/RttiStatsScript.java` | the RTTI structures and vftables the analyzers marked, the analysis options of RTTI and Function ID |
| `ghidra/community_names.py` | merges sacred-bild's named entry points and sacred-sdk's addresses into an `address<TAB>name` file outside the repository |
| `ghidra/ImportNamesScript.java` | names the functions and labels the data of such a file |
| `ghidra/MarkLibraryScript.java` | tags the functions of address ranges (`TAG:start-end`) and of the `_STL` classes |
| `ghidra/CarryNamesScript.java` | carries names and classes from another program of the project over the functions matched by their normalized bytes |
| `ghidra/ProjectStatsScript.java` | the counts: functions by the source of their names, library, in classes; classes; vftables |
| `ghidra/mcp_bridge.py` | the ghidra MCP server of a Claude Code session (stdio): starts pyghidra-mcp when nothing answers, and passes the session's messages to it over Streamable HTTP; `--start` starts it alone; the standard library alone, its tests `test_mcp_bridge.py` on a fake server |

## The game's remote control

`tools/game/remote.py` drives an `opensacred-game` started with `--remote` (docs/10 §8, docs/03 §3.7): it runs the game's commands, reads its state and log, sends the player's keys, text and clicks, and takes screenshots, through the game's Remote Protocol on `127.0.0.1`; the standard library alone.

```bash
uv run python -I tools/game/remote.py wait-state menu
uv run python -I tools/game/remote.py exec "new-game --class dwarf"
uv run python -I tools/game/remote.py wait-ready
uv run python -I tools/game/remote.py click 900 520
uv run python -I tools/game/remote.py screenshot <scratch>/shot.png
uv run python -I tools/game/remote.py wait-log "sectors" --after 0
```

With the original running beside the game under its driver, two more compare them (docs/03 §3.7):
- `tools/game/compare.py X,Y …` puts both heroes onto each cell and compares their creatures, and `compare.py stats --types T,… --levels L,…` makes each type at each level in both and compares their fight values (#574, #587);
- `tools/game/conformance.py [--suites …]` runs the suites of checks against the original, PASS or FAIL each, the experience, the hero, the stats, the drops, the places, the regeneration, the strikes, the arts and the movement (#600).

## The original's driver

`tools/original/host.py` starts the developer's own `Sacred.exe` in the background under Frida, with `tools/original/agent.js` loaded into it, and serves the same methods on `127.0.0.1:15703`, so that `tools/game/remote.py --port 15703` drives the original as it drives the game (docs/03 §3.8, #448; the skill `original`).
One original runs on the machine: the host records its holder (`--holder`) while it runs, and a host started meanwhile refuses, naming it (#1398); tests: `uv run python -I -m unittest discover -s tools/original -p "test_*.py"`.
The agent guards the maintainer's desktop (no low-level hooks, no focus, no topmost window, no display mode, no cursor), keeps the game drawing without the focus, counts its frames and takes its screenshots, takes the player's input as its own messages, reads the world and runs commands on the render thread, and traces the original's functions (docs/re/traces.md); the host keeps the game in a job object that ends with it and clicks through the original's menus (a new game, the saves).
It needs `frida` and Pillow (`pyproject.toml`) and the installation in `OriginalGame/`, and writes only the keys of `Settings.cfg` it needs; the game writes its own files there.

```bash
uv run python -I tools/original/host.py --holder <session> --log-file <scratch>/original.log   # in the background
uv run python -I tools/game/remote.py --port 15703 state
uv run python -I tools/game/remote.py --port 15703 screenshot <scratch>/menu.png
uv run python -I tools/game/remote.py --port 15703 exec quit
```

## Scripts

| Script | Area | What it does |
|---|---|---|
| `triage/pe_triage.py` | PE | headers, sections with entropy, imports and exports, version, RTTI names |
| `analysis/common/sacred_pe.py` | shared module | in-memory PE image loader, disassembler, operand annotations |
| `analysis/pe/pe_deep.py` | PE | functions, calling conventions, import call sites, RTTI, vtables, EH |
| `analysis/pe/class_compare.py` | PE | classes shared by the client and the server, from RTTI |
| `analysis/pe/fn_census.py`, `thunk_check.py` | PE | normalization of the client's function database: own code, library, EH thunks (docs/01 §4.1–4.2) |
| `analysis/pe/fn_match.py`, `fn_census2.py`, `analysis/common/fn_match_lib.py` | PE | client functions byte-identical to `GameServer.exe` functions, by module (docs/01 §5.1) |
| `analysis/pe/fn_context.py` | PE | function context: API calls, strings, GUIDs; search by string or import |
| `analysis/pe/detail_scan.py` | PE | targeted code reports: threads, functions by string, ranges |
| `analysis/pe/fpu_scan.py` | PE | FPU control instructions, RNG calls, `VirtualProtect` |
| `analysis/pe/srand_scan.py`, `control87_callers.py` | PE | `srand` seeds and `_control87` calls with context |
| `analysis/pe/balance_keys.py` | PE, `bin/Balance.bin` | the balance text's keys, the variables they name in the balance image and their kinds, and `GoldProLevel`'s second value (docs/02 §6.3, #71, #413) |
| `analysis/pe/granny_args.py` | PE | Granny call arguments by call site |
| `analysis/pe/rsrc_dump.py` | PE | resource section: types, sizes, dialogs, large resources |
| `analysis/pe/sacred_strings.py` | PE | symbol strings and configuration keys from the data sections |
| `analysis/render/d3d7scan.py` | render | `IDirect3DDevice7` calls with recovered arguments (render states, stages) |
| `analysis/world/world_dump.py` | world | `sectors.keyx`, every sector of `sectors.wldx`, `tiles.pak`, `Static.PAK` and `Floor.PAK`: layout checks on every record, the static and floor chains of the cells; a per-sector table with cell, tail and chain fingerprints for the Rust reader |
| `analysis/world/sector_stats.py` | world | statistics of sector floor cells: heights, light, surface types (reads the cells from `+0x120`, eight cells late; docs/02 §5.2) |
| `analysis/world/static_stats.py` | world | fields of the `Static.PAK` static objects |
| `analysis/world/uv_table.py` | world | UV table of the 18 diamonds on a tile sheet |
| `analysis/formats/tex_decode.py` | formats | decoding of format 4 textures and comparison of pixel-format hypotheses |
| `analysis/formats/tex_fmt6.py` | formats | format 6 textures: sizes, alpha position, menu screens assembled from tiles in BGRA and RGBA order |
| `analysis/formats/snd_ls.py` | formats | `sound.pak` directory: WAV (PCM, IMA ADPCM) and MP3, durations, cross-check with `sndProfiles.pak` |
| `analysis/formats/global_res_dump.py` | formats | `global.res` string table: layout checks on every record (23,123 strings), a per-record table for the Rust reader |
| `analysis/formats/global_res_hash.py` | formats | resource name hash and its check against `global.res` |
| `analysis/formats/amh_dump.py` | formats | AMH hero templates (and saves with `--saves`): checks on every section, per-section rows for the Rust reader; the items of the inventory (`--items`), the items of the backpack at their cells (`--backpack`, #1285) and the combat arts of the creature section (`--arts`, #506) |
| `analysis/formats/mhp_probe.py`, `mhp_vs_mdt.py` | formats | `MOTIONS.PAK` fields and cross-check with `Models.tmp` |
| `analysis/formats/grn_dump.py` | formats | MDL containers `models*.pak` and the GRN container of every record: layout checks, cross-check with `Models.tmp`, chunk tag census; the object section's string table, data extensions, transform channels and the skeleton with bone names; meshes, weights, the form's mesh bindings, textures, materials and render passes; animation tracks (split and interleaved); a per-record table with tree, skeleton, mesh, material and animation fingerprints for the Rust readers |
| `analysis/formats/spf_names.py` | formats | `sndProfiles.pak` sound profiles |
| `analysis/formats/sound_dump.py` | formats | `sound.pak` samples (WAV chunks and formats, MP3 frames), `sndProfiles.pak` profiles (10 zeros and 8 groups of 8 ids, variants repeated in turn) and the table of sound names in `Sacred.exe`: checks on every record, per-record tables for the Rust reader |
| `analysis/formats/mixed_dump.py` | formats | `items.pak` (types, sprites, names) and `mixed.pak` (the sprites of the statics as pieces of atlases): layout checks on every record, per-record tables for the Rust readers; `--alpha` checks the sprites' texels against their size; `--assembled DIR` assembles every sprite and compares it with the converter's images (#838) |
| `analysis/formats/bin_tables_dump.py` | formats | the data tables of `bin/`, with the tables after the balance image (the drops' weights, the quality pass's values, the effects' chance parameters; #413): layouts checked to the last byte, a fingerprint per file for the Rust reader |
| `analysis/formats/arts_dump.py` | formats | the combat arts' two tables compiled into `Sacred.exe`, the spells and the weapon arts (#505): the executable checked against the manifest, checks on every record, per-record rows with every field for the Rust reader |
| `analysis/formats/tables_dump.py` | formats | the fixed-record tables `Creature.pak`, `Weapon.pak`, `MOTIONS.PAK`, `Triggers.PAK` and the overlay `Items03.pak`: checks on every record, per-record rows for the Rust readers |
| `analysis/scripts/netscript_census.py` | scripts | census of NetScript record types across all directories |
| `analysis/scripts/kwtab.py` | scripts | keyword tables of the script compiler in the client's `.data` |
| `analysis/scripts/netscript_flow.py` | scripts | the census of the NetScript control flow over the 20 trees by the walker's semantics (#722): the `IF` chains by their ending, the `ELSE` branches by their record, the `IF`s within a branch, the blocks of 41 and 42, the guards |
| `analysis/scripts/netscript_dump.py` | scripts | NetScript operands of every record with the tag table of the client's operand reader (checked against its dispatch tables), record type names from the compiler's keyword resolvers; per-file tables for the Rust reader; the operand tag sequences of each record type (`--signatures`) |
| `usage/pr_usage.py` | process | Claude Code usage of a pull request from the transcripts' messages on its branch: tokens per model, cost at API prices, agent-hours; `--update` writes them into its description and refreshes the badge (`docs/usage.md`). The dollar figures are hypothetical: development runs on a Claude subscription; tests: `uv run python -I -m unittest discover -s tools/usage -p "test_*.py"` |
| `usage/usage_badge.py` | process | the README's usage badge: sums the usage blocks of the pull requests' descriptions and the sessions before, for the Usage workflow |
| `stack/stack.py` | process | stacked pull requests: `after <PR>` brings the pull requests stacked on a squash-merged one up to `main` (merging `main` in with the newest of their base's commits as the merge base) and pushes them; `integrate` merges every open pull request into a local branch `integration`, a stacked one after its base, and prints the conflicts (`--check`: fmt, clippy and the tests on it); tests: `uv run python -I -m unittest discover -s tools/stack -p "test_*.py"` |

## Check on the GOG client (2026-10-16, Python 3.15.0)

| Check | Result |
|---|---|
| `pe_deep.py --no-disasm` | 389 imports, 417 RTTI types, 410 vtables, 3,224 slots, 2,539 EH frames (docs/01, §4) |
| `fn_context.py --str "cEngine::simThread"` | function 0x60f060 found |
| `fpu_scan.py --game-end 0x7F1000 --no-vp` | 5 `fldcw` in game code (jump-table artifacts) |
| `tex_fmt6.py` | 227 format 6 textures, bytes B, G, R, A, uncompressed; the directory `size` excludes the 80-byte header (docs/02, §3.1) |
| `snd_ls.py` | `SND` v1, 6,598 of 50,000 slots: 3,194 WAV and 3,404 MP3; directory has no gaps (docs/02, §10.2) |
| `grn_dump.py` | `models.pak` 1,572 models + 3,421 motions, `models03.pak` 2 + 2; all 4,993 non-empty GRN files pass the container checks, CRC-32 included, the skeleton checks (70,242 + 258,760 bones, bone 0 `__Root`) and the model checks: 2,555 + 207 meshes, 2,891 + 302 render passes, 258,562 animation tracks (docs/02, §4.1–4.2) |
| `sound_dump.py` | 6,598 samples (1,825 PCM, 1,369 IMA ADPCM, 3,404 MP3), 175 profiles; 102 profile ids have no sample (docs/02, §10); 6,869 sound names, every sample and profile id named |
| `mixed_dump.py` | `items.pak` 17,408 used slots of 32,768; `mixed.pak` 16,255 sprites with 209,956 pieces, each as large in its atlas as in the sprite; with `--alpha`, no texel of any alpha past a sprite's size from its pieces' smallest corner, 2,122 sprites with texels past it from their origin (docs/02, §3.3, §6.1) |
| `world_dump.py` | 6,050 sectors, 90,132 tiles, 1,038,014 statics, 6,713,136 floor overlays; every check passes; the cells' chains reach every overlay and every static but 2; the statics' mask (`+0x2B`) is 0 or one bit, 1 in 542,635 (docs/02, §5) |
| `amh_dump.py` | 8 templates, 64 sections: 40 compressed, 24 stored; with `--saves` on a clean installation 10 files, 80 sections; 46 items, 32 of them in the backpacks, 19 combat arts; every check passes (docs/02, §9) |
| `bin_tables_dump.py` | 12 files of `bin/` read to the last byte (docs/02, §6.3) |
| `arts_dump.py` | 70 spells and 94 weapon arts, every check passes (docs/02, §6.5) |
| `tables_dump.py` | 474 creatures (flags within six bits), 4,883 weapons, 3,423 motion parameter blocks, 1,246 of 2,268 triggers, one overlay item (slot 3999); every check passes (docs/02, §5.5, §6) |
| `netscript_dump.py` | 2,203,278 records in 51 non-empty files; every record consumes its body exactly with the 40-handler tag table, but the 11 type-50 records whose last tag 0x2A has no payload; 135 record types named (docs/02, §7) |

## Sources

* `docs/02-data-formats.md`: descriptions of the formats that the parsing scripts implement.
* `docs/01-binary-analysis.md`: the results of the PE scanners.
