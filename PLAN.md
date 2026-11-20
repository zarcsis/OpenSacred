# Plan: OpenSacred

The main plan document.
The project's goal is to **play Sacred Gold 2.28 (GOG) comfortably on modern hardware**: Windows 11 x64 at 4K, later Linux, possibly Android.
The tools work on the user's own installation, the GOG release or, for the converter alone, the Steam one (#1484); the repository holds no game files (§12).
This document is self-contained; details are in `docs/01…10`, analysis scripts in `tools/` ([description](tools/README.md)).

Claim status is marked with the words **confirmed** (verified on GOG release files with a script or a disassembler), **community-reported** (not verified on a file), **estimate** (model calculation) and **hypothesis**.

## Summary

* **Approach: "convert the assets + rewrite the game", not "decompile and rebuild".**
  A toolset reads the assets of the user's own installation once and converts them into open formats (PNG/KTX2, glTF, FLAC/Ogg, AV1, RON/Fluent, Luau).
  The new game reads only these formats.
  Where the game's rules are not evident from the data, `Sacred.exe` is studied only to understand them; the behavior is described in prose and implemented independently in Rust.
  The original is the reference, followed in spirit and not copied: the game departs from it where that makes the design simpler without harm to the play, makes the play better, or repairs a defect of the original (§1.1).
* **Language: Rust for the entire product**: the game, the converter, the tools.
  VC6 and old toolchains are not needed: byte-for-byte matching is not a goal.
* **Engine: Bevy 0.20 on wgpu** (Vulkan on Windows, Linux and Android).
  The game rules live in a separate crate with no engine dependency, so if needed we can switch to Godot with godot-rust without losing the logic.
  Unreal Engine and Godot with C++ were considered and rejected (§5).
* **Multiplayer after version 1.0:** until then nothing networked, no server and no lobby, but nothing that rules multiplayer out (ADR-09).
  Compatibility with the original saves is not needed.
* **Only free tools and dependencies:** Ghidra 12.2 with an MCP server for Claude Code, Rust, Visual Studio Build Tools 2026, Vulkan SDK, Unicorn, FFmpeg, Blender, RenderDoc.
* **Solo development, almost entirely through Claude Code.**
  Every pull request records its Claude Code usage, which the README's badge sums (§9.1); no estimates of time or cost are kept.
  The first visible result, a walk through the starting region in 4K, came on 2026-10-17 (G2).
  By 2026-11-17 M0 to M5 were done and every issue then planned for M6 was closed.
  M6 has since taken what the documents' audit and the sortings of the notes found (#2787, #2845, #2950), 144 issues open on 2026-11-20; version 1.0 waits for the maintainer's own play-through after them (G5, §10).

## 1. Requirements and goals

### 1.1 Project requirements (2026-10-16)

| Requirement | Consequence for the plan |
|---|---|
| The main goal is to play comfortably on modern hardware | The success criterion is completing the Sacred and Underworld campaigns in 4K without crashes, not matching the original |
| The original is the reference, followed in spirit, not copied with its flaws: a departure is worth making where it does no harm to the play and makes the design simpler, where it makes the play better, or where it repairs a defect of the original (reworded 2026-11-17, #2856: the row said that fidelity was not a priority and that the behavior may be redesigned) | The original's behavior is read and described first (`docs/re/`), so that a departure is a choice and not a guess; each is a row of `docs/decisions/` with its ground; no byte-for-byte matching, no VC6, no determinism requirements |
| Multiplayer after version 1.0 (changed 2026-10-23: no network components were planned before) | No network code, TinCat, lobby or server before 1.0; the game is built so that a host can later run it for several players (ADR-09) |
| Work only with the GOG version (changed 2026-10-30: the converter also takes a player's Steam installation, #1484) | The reference is GOG ENG 2.28; the tools read the user's own installation, the development tools (the oracle, the original's driver, the research scripts) the GOG one alone |
| The game does not have to read the original formats and saves | An offline converter into open formats; the runtime reads only those; HD and upscale packs in the future |
| Solo development almost entirely through Claude Code | The process is built around tests and text artifacts; every pull request records its usage (§9.1) |
| Modern toolchains: Visual Studio Build Tools 2026, latest Vulkan, newest dependencies | Latest stable Rust, scheduled dependency updates |
| One language for the whole project, preferably Rust | Rust everywhere |
| Only free tools and dependencies | Ghidra instead of IDA/Binary Ninja; all software and libraries are open source or free of charge |
| Platforms: Windows 11 x64; then Linux; possibly Android (changed 2026-10-23: the browser version dropped) | wgpu covers all three targets with one codebase |
| Consider an existing engine (UE, Godot) | Comparison in §5 and `docs/04-language-and-engine.md` |

### 1.2 Goals and measurable criteria

| # | Goal | Criterion |
|---|---|---|
| G1 | Game assets are available in open formats | The converter processes 100% of the files of the reference GOG installation per the manifest; models open in Blender and pass glTF-Validator; a rerun of the extraction gives the same result (stage B's lossy encoders need not, §7). 2026-10-17: `opensacred-convert coverage` gives, of the manifest's 319 files, 303 converted and 16 skipped with a reason, none left; since 2026-10-29 the manifest holds only the files the project uses, 313: 303 converted and 10 used in development, the converter reading 304 of them (#1482); tables and hero files keep the fields not yet understood as raw spans |
| G2 | Vertical slice | The hero walks through the starting region with animations, sound and UI at 3840×2160 at ≥ 60 fps on a mid-range PC. Met 2026-10-17: about 775 fps at 3840 × 2160 while the hero runs across three sectors, about 580 on 2026-10-19, and 365 to 392 on 2026-10-25 with the creatures and the fight (docs/05 §3.6, docs/10) |
| G3 | Game rules | Combat, attributes, skills, items and loot work by the recovered formulas; deviations from the original are deliberate and recorded. M3 closed on 2026-10-26 (§10); the deviations are in `docs/decisions/` |
| G4 | Campaign content | Quests, dialogues and triggers of the Sacred and Underworld campaigns run from converted scripts; both campaigns are completable from start to finish. Both were played through to their ends in Claude Code's driven runs on 2026-11-10 (#1099); the maintainer's play-through of G5 checks it again |
| G5 | Playable version 1.0 for Windows 11 x64 | Full UI, saves, settings; 4K and HiDPI; a playthrough without critical bugs; the gamepad after G5, in M8 (the maintainer's decision of 2026-11-09, #1097); version 1.0 is released once the maintainer has tested the game and says so, not when M6 closes (the maintainer's decision of 2026-11-10, #2479). The research of the shadows that it also waited for ended on 2026-11-11 (#1436); every issue of M6 was closed by 2026-11-17, the audit of the documents then filed the defects it read (#2787) and the sortings of the notes what the game does not follow yet (#2845, #2950), and the maintainer's play-through comes after them (the maintainer's decisions of 2026-11-18) |
| G6 | HD assets | The upscale pipeline produces HD packs that the game picks up on top of the base assets |
| G7 | Other platforms | Linux: the same functionality; Android: by a separate decision after G5 |
| G8 | Multiplayer | After G5: the Sacred and Underworld campaigns played together over the network; how is decided when the work starts (§13) |

**Out of scope:** networking before version 1.0 (multiplayer comes after it, G8); loading the original saves; byte-for-byte compatibility; macOS and iOS; the browser version (dropped 2026-10-23); publishing the game assets in any form.
The Steam version left this list on 2026-10-30: the converter takes a player's Steam installation, its four files that differ from the GOG ones checked against `docs/manifest-steam-2.28.txt` (#1484), while the reference and the development tools stay with the GOG release; other builds of the game are not supported.

## 2. What is already known

All figures are for the GOG release of Sacred Gold 2.28 (English).
Details: [docs/01-binary-analysis.md](docs/01-binary-analysis.md), [docs/02-data-formats.md](docs/02-data-formats.md).

| Topic | Fact | Status |
|---|---|---|
| Release | GOG Sacred Gold 2.28 ENG; `Sacred.exe` 11,919,360 B, SHA-256 `db9b86a4…71e74`, 2006-10-13; no DRM | confirmed |
| Client code | MSVC 6 SP5 + Processor Pack; CRT, STLport, zlib, D3DX7 statically linked; ≈5,900 game functions in 3.48 MB (≈180k logical lines of C++); 417 RTTI classes, 410 vtables | confirmed |
| Code modules | game logic 2,343 functions / 1.81 MB; UI 987 / 572 KB; particles 1,270 / 345 KB (178 effect classes); events 257 / 85 KB; rendering 134 / 56 KB; scripts 29 / 12 KB | estimate from code labeling |
| Original graphics | Direct3D 7, hardcoded 1024×768, orthographic projection with 0.5–2.0 zoom; the ground is 96×48 diamonds with heights; objects are sprites; characters are Granny meshes with CPU skinning; ≈9,600 draw calls per frame | confirmed |
| Containers | all PAKs parsed: 0x100 header + directory + arrays of fixed-size structures | confirmed |
| Textures | 25,535 textures: 25,307 in ARGB4444 with zlib compression and 227 in A8R8G8B8 (bytes B, G, R, A) uncompressed; no mipmaps; 22,932 are 256×256 | confirmed |
| Models | 1,574 models and 3,423 animations, all whole Granny 1.2b files; the format has been read by three independent community readers | confirmed |
| World | 100×100 sectors (6,050 filled), 24.8M floor cells with heights, 1,038,014 static objects | confirmed |
| Scripts | NetScript bytecode: 141 record types, 113 in use; the interpreter is in the client | confirmed |
| Texts | `global.res` in UTF-16LE, 23,123 strings with hash identifiers; the hash formula has been recovered | confirmed |
| Sound and video | 175 sound profiles; `sound.pak` is 392 MB with 6,598 samples: 3,194 RIFF WAV (1,825 PCM, 1,369 IMA ADPCM) and 3,404 MP3; 173 music MP3s; 10 WMV movies (WMV3 640×480, WMAv2 or WMA Pro audio) | confirmed |

## 3. Key decisions

| ID | Decision | Why |
|---|---|---|
| ADR-01 Goal | A playable modern reimplementation on the original's converted assets, with the original as its reference and improved where that is worth it, not an exact copy | Requirement (§1.1); removes the most expensive requirements (matching, determinism, save compatibility) |
| ADR-02 Strategy | Asset converter + an independent reimplementation of the game; the original's behavior is studied in the binary selectively, only where the rules are not evident from the data, described in prose and implemented anew (§4.2); since M2 that includes what the player sees and hears, the interface, the sounds, the ways and the order of the draws among it (`docs/re/`) | Reproducing the original's rendering, UI, sound, input and file handling from its code is pointless; the binary's value lies in the rules and the content semantics |
| ADR-03 Language | Rust (edition 2024, latest stable is 1.100.0) for the entire product; the Python research scripts remain as an archive | A single language, memory safety, a modern ecosystem |
| ADR-04 Engine | Bevy 0.20 (wgpu 30) for presentation; the rules crate `opensacred-rules` does not depend on the engine (created 2026-10-21 with the spawning's rolls, then the fight's formulas, the ways and the creatures' decisions moved into it from `opensacred-game`, #364) | Pure Rust, everything in code and text, which suits Claude Code; Vulkan and Android from one codebase; the fallback is Godot 4.7 with godot-rust on top of the same rules crate |
| ADR-05 Assets | Offline converter `opensacred-convert` (Rust CLI) in two stages (#694, the maintainer's choice of 2026-10-23): **extract**, the installation → lossless masters in open formats that tools edit, with the repairs of the data, in `assets-src/` (textures → PNG, models and animations → glTF 2.0, world → binary sectors + RON, tables → RON, texts → Fluent, sound and music → FLAC, video → FFV1 and FLAC in Matroska, scripts → Luau, ADR-13); **build**, the masters → the game's packed assets in `assets/` (textures → KTX2 with BC7 and mipmaps, sound → Opus in Ogg, video → AV1 and Opus in WebM, scripts → Luau bytecode, all in ZIP archives); `all` does both; the game reads only the built assets; HD packs and mods are made in the masters' formats and built by the same stage B (§7) | Requirement (§1.1); the masters can be checked and edited with third-party tools (Blender, glTF-Validator) and lose nothing, so a build from them gives what a direct conversion would; the game's formats load fast and fit the GPU |
| ADR-06 Graphics and 4K | Rendering at native resolution through wgpu (Vulkan by default on Windows, DX12 as fallback); an orthographic camera with the original's angle and adjustable zoom; the ground is real 3D terrain from a heightmap; scalable UI; HD packs on top of the base assets | The original is bound by the CPU and by 1024×768, not by the GPU; a modern renderer removes both limits |
| ADR-07 Sound, input, video | Kira via `bevy_kira_audio`, the sound and the music as Ogg Opus decoded through libopus (#701), the one C library of the sound, which the build compiles with CMake; Bevy input with every key remapped in `controls.toml` (#1553), and the gamepad after version 1.0 (M8, #1097); video: AV1 and Opus in WebM, the AV1 decoded by rav1d (#1945) | Everything is free and builds for Windows, Linux and Android |
| ADR-08 Saves and settings | Our own versioned save format, postcard through serde: a header, then the hero, the scripts and the world, in `saves/` beside the settings' file, a save of another version refused (`saves.rs`, #1095; docs/re/saves.md); settings in TOML in the user directory | The original saves are not needed; a modern format is easier to debug |
| ADR-09 Network | Multiplayer after version 1.0 (M9), no network code, server or lobby before it; until then the game does not rule it out: the rules stay apart from the engine (ADR-04), the world's state lives in the rules and the ECS rather than in presentation or input, and the scripts run on the host, as in the original (docs/02 §7.5); `GameServer.exe` is an auxiliary binary when analyzing shared functions, and a reference for the server's part once M9 starts | Requirement (§1.1), changed 2026-10-23 |
| ADR-10 Tools | Free only: Ghidra 12.2 + pyghidra-mcp (GhidraMCP 2.0 as the alternative with the GUI), Unicorn (emulating the original's functions for tests), Frida (driving the running original for Claude Code, #448), x64dbg, dgVoodoo2 (optional), RenderDoc, Vulkan SDK 1.4, Blender, FFmpeg, Visual Studio Build Tools 2026 | Requirement (§1.1); IDA Pro and Binary Ninja are excluded |
| ADR-11 Process | Claude Code as the main executor: `CLAUDE.md` with rules, MCP to Ghidra, tests as guardrails (the readers' cross-checks against the Python prototypes, oracle vectors, property tests, mutation testing of the lines a pull request changes, screenshot tests; docs/03 §3.4), small tasks, maintainer review | Solo development; quality rests on automated checks, not on manual review of every line |
| ADR-12 Repository and legal | The repository is public and holds no game bytes: no game files, converted assets, HD packs or dumps, in the repository or in CI; users supply their own copy, GOG's or Steam's, and run the converter locally; converted output is never published (§12, docs/08) | The game belongs to the rights holder; every user needs their own copy |
| ADR-13 Scripts | The converter translates the NetScript bytecode into Luau, a source file per quest, sector, region and start script, with its bytecode; the game runs only Luau, through mlua and its vendored Luau in a crate of its own (`opensacred-scripts`), sandboxed and bounded; what each record type does is the scripts' API, recovered from the original and implemented in `opensacred-rules` and `opensacred-game`; the quests' state, the variables and the triggers live in the engine and the saves, not in Luau; mods after 1.0 are Luau on the same API; the translated scripts are converted assets, never published (#720) | Decided 2026-10-23: the operands of the 113 types are decoded once, in the translator, not in an interpreter and again in a translator; the reference is the running original (#448), not an interpreter of ours; Luau rather than Rhai or Lua 5.4 for its types, its sandbox and interrupts, its bytecode compiled ahead and its speed; its WebAssembly needs Emscripten, which the dropped browser version makes moot |

## 4. Strategy

### 4.1 Pipeline

```text
OriginalGame/      ← the user's own installation (git-ignored), else the GOG or Steam one that the converter finds
   │  opensacred-convert's extractions (stage A: lossless, with the data's repairs)
   ▼
assets-src/        ← masters in open formats that tools edit (PNG, glTF, FLAC, FFV1, RON, Luau)
assets-hd/         ← HD packs and mods in the masters' formats, over assets-src/ at matching paths (M7, not made yet)
   │  opensacred-convert build (stage B: packed and encoded)
   ▼
assets/            ← the game's assets: KTX2, Opus, AV1, Luau bytecode, in ZIP archives
   │                 (all three generated locally, never committed to git)
   ▼
opensacred-game (Bevy) ───── uses ───── opensacred-rules (rules, no engine)
                                               ▲
   Ghidra + MCP ──► the behavior described in prose (docs/re/) ──► Rust, written independently
                                               ▲
   opensacred-oracle (Unicorn, local): the user's own Sacred.exe → reference vectors for tests
   tools/original (Frida, local, #448): the user's own Sacred.exe, running → observations
```

A developer runs the extractions and `build` apart, or both with `all`.
A player's run is `opensacred-convert install` (#1497), which the packaged game starts at its first start (#1498): it finds the installation, runs both stages into a working directory, puts the assets in place whole in `%LOCALAPPDATA%\OpenSacred\assets` and removes the masters.

### 4.2 How the rules are recovered

1. **A local Ghidra project on the user's own GOG `Sacred.exe`** with imported community names (sacred-bild, sacred-sdk) and RTTI.
   Claude Code reads the decompilation through the MCP server only to understand the formats and the rules.
2. **Choosing a mode for each subsystem:**
   * rules with numbers (damage, defense, resistances, experience, skills, item generation, prices) are studied in the code;
   * content semantics (NetScript opcodes, triggers, spawning) are studied in the code and the data;
   * what the player sees and hears of the original's behavior is studied in the code too, since M2: the interface's windows and their layout, the sources of the sounds, the search of the ways, the order of the draws, the effects' particle systems (`docs/re/interface.md`, `sound.md`, `movement.md`, `drawing.md`, `effects.md` and their neighbors);
   * the engine under it (the renderer, the asset loading, the files and their formats, the tools) is written from scratch on Bevy, without the original's code.
3. **A description in prose.**
   What the original does (inputs, outputs, formulas, tables, order of events) is written down per subsystem in `docs/re/`, in our own words, not as code.
4. **Reference vectors.**
   For formula functions, `opensacred-oracle` runs the functions of the user's own `Sacred.exe` in the Unicorn emulator on sets of inputs and records the outputs as vectors.
   It runs locally on any OS and does not require running the game; CI has no `Sacred.exe`, so the tests on the vectors run locally, like the other tests on the game files.
5. **Observations of the running original.**
   Where the oracle cannot reach (a rule spread over threads and frames, the engine's timing, the creatures' choices, what the screen shows), the running original is observed.
   Until 2026-10-21 that needed the maintainer at the game (#227); since 2026-10-22 the driver of #448 lets Claude Code do it itself: `tools/original/` runs the user's own `Sacred.exe` under Frida in the background, reads its state, sends its keys and clicks, runs commands, traces its functions and takes its frames (docs/03 §3.8).
   Its frames, logs and traces stay on the developer's machine; the results go into the issues and `docs/re/` as prose, and a frame of a problem, cropped to it, into its issue as an attachment (docs/08 §3).
6. **Rust, written independently, then improved where that is worth it.**
   `opensacred-rules` implements the behavior from the description and passes the vectors.
   A departure from the original is then a choice, made on one of the grounds of §1.1 and recorded in the decision log (`docs/decisions/`).
   What the crate holds is in §6.3.
   `opensacred-oracle` (#363) records the vectors of the levels' ends and the levels of an experience, the hit chance and the damage through a resistance (`0x43EEE0`), the most health, the factor of the levels and the game's roll between two bounds, which `opensacred-rules`' `tests/oracle.rs` compares with; its `fuzz` runs random inputs through the original's functions and the rules' in one process and shrinks the first mismatch (#1431).
   The first vectors, of 2026-10-21, agreed on the levels and showed five bugs in the rest, all fixed (#406, #407, #408, #417, #418); the checks in the original of #227 were worked through with the driver and closed on 2026-10-25.
7. **Nothing from the study is committed.**
   The Ghidra project, decompiler output and anything else made locally while studying the original (scratch C++ in a local `reference/` directory included) stay on the development machine; the repository holds only the prose in `docs/re/` and our own code.

### 4.3 Rejected options

| Option | Why rejected |
|---|---|
| Byte-for-byte decompilation for VC6 (LEGO Island style) | The goal is not an exact copy; doubles the cost; requires VC6 |
| Hybrid bridge inside the original's process (OpenRCT2 style) | Ties us to a 32-bit process and D3D7; needed only if parity is required |
| Loading the original formats at runtime | Not needed per the requirements (§1.1); the converter gives open formats and HD packs |
| Pure reimplementation without analyzing the binary | Combat and item rules and script semantics are undocumented; without the original's code the game will be "similar" but with different numbers |
| Translating the decompiled code into Rust | The rules are implemented independently from prose descriptions of the behavior (§4.2), which also leaves room to depart from it |

## 5. Engine: our own in Rust or an existing engine

Detailed comparison: [docs/04-language-and-engine.md](docs/04-language-and-engine.md).

### 5.1 Candidates (versions as of 2026-10-16)

| Option | Version | License | Renderer | Platforms from §1.1 |
|---|---|---|---|---|
| **Bevy + rules crate in Rust** | 0.20 (2026-10-08), wgpu 30 | MIT / Apache-2.0 | wgpu: Vulkan, DX12, WebGPU, WebGL2 | Windows, Linux, Android, Web |
| Our own engine on wgpu + winit + kira + egui | wgpu 30, winit 0.30 | MIT / Apache-2.0 | wgpu | all four, but everything else we write ourselves |
| Fyrox | 1.0.1 (2026-03) | MIT | OpenGL / WebGL2 per secondary sources | Windows, Linux, Android, Web |
| Godot + godot-rust (gdext) | Godot 4.7.2, gdext 0.5.5 | MIT | Vulkan, D3D12; WebGL2 on the web | Windows, Linux; Android and Web are experimental in gdext |
| Godot + C++ (GDExtension) | Godot 4.7.2 | MIT | Vulkan, D3D12; WebGL2 on the web | all four |
| Unreal Engine + C++ | 5.8 | free, source available, royalties on commercial products | DX12, Vulkan | Windows, Linux, Android; no web |

### 5.2 Matrix (scores 1–5, weight in parentheses)

| Criterion | Bevy | Own on wgpu | Fyrox | Godot + Rust | Godot + C++ | UE5 + C++ |
|---|---|---|---|---|---|---|
| Single language, Rust (15) | 5 | 5 | 5 | 4 | 1 | 1 |
| Convenience for Claude Code: everything in text, CLI builds, headless tests (20) | 5 | 5 | 3 | 3 | 3 | 1 |
| Own work until playable (15; 5 = least) | 3 | 1 | 3 | 4 | 4 | 3 |
| Target platforms (10) | 4 | 4 | 3 | 3 | 4 | 2 |
| Vulkan and a modern renderer (10) | 5 | 5 | 2 | 5 | 5 | 4 |
| API stability (10) | 2 | 4 | 4 | 3 | 4 | 4 |
| 2.5D isometric and skeletal animation from glTF (10) | 4 | 3 | 4 | 4 | 4 | 2 |
| Free of charge, license (5) | 5 | 5 | 5 | 5 | 5 | 3 |
| Build iteration speed (5) | 3 | 4 | 3 | 4 | 3 | 1 |
| **Total out of 500** | **410** | **395** | **350** | **375** | **345** | **220** |

Notes on the debatable scores:

* **Bevy, stability 2.**
  Before 1.0, Bevy breaks its API in every release: 0.18 (January 2026), 0.19 (June), 0.20 (October).
  A migration is a task of its own per release; every release has a migration guide.
  The project takes the newest dependencies (§1.1), so this is a price paid knowingly.
* **Own engine, work 1.**
  We would have to write the asset system, animation blending, UI and scenes, which Bevy already provides.
* **Godot, convenience for Claude Code 3.**
  `.tscn` scenes are text, but asset import and debugging are tied to the editor.
  In godot-rust, WebAssembly support is behind the `experimental-wasm` flag, and Android is experimental too.
* **UE5, convenience 1.**
  Blueprints and `.uasset` are binary and Claude Code cannot edit them; builds are heavy; there is no web export; the only language is C++.

### 5.3 Decision

**Our own Rust version built on Bevy.**
An existing engine with C++ loses on the criteria that matter most to the project: a single language and working through Claude Code.
Godot with C++ wins only on time to playable and API stability.

Sensitivity: if the weight of "work until playable" is doubled and the weight of Rust is halved, Godot + Rust catches up with Bevy.
That is why the game rules live in a separate crate with no engine dependencies.
A switch to Godot with godot-rust keeps `opensacred-rules` and `opensacred-convert`; only the presentation layer would have to be rewritten.

Triggers for revisiting: Bevy migrations regularly take more than 10 agent-hours; Bevy's animation or UI systems block the vertical slice for more than two weeks; Android needs capabilities that wgpu lacks.

## 6. Architecture

### 6.1 Crates

```text
OpenSacred/
├─ Cargo.toml                 Rust workspace (edition 2024)
├─ rust-toolchain.toml        stable, updated together with the dependencies
├─ build.rs                   the converter's and the game's --version from git describe (Cargo.toml has no version)
├─ pyproject.toml  uv.lock    the repository's Python through uv: the packages of tools/ (docs/03 §2)
├─ CLAUDE.md                  project rules for Claude Code
├─ .claude/                   Claude Code's hooks, settings and the project skills `game` and `original`
├─ .github/                   the issue forms, the pull request template and the workflows: CI, Mutants, Release, Dependencies, Usage, FFmpeg
├─ crates/
│  ├─ opensacred-formats/     reading the original formats (PAK, TEX, GRN, sectors, NetScript, global.res, AMH templates)
│  ├─ opensacred-locate/      (#1496, #1556) finding the installation: the player's GOG or Steam one for the converter, the developer's for the tools and the tests
│  ├─ opensacred-convert/     CLI: the installation → assets-src/ → assets/ (subcommands install, locate, verify, textures, models, world, sound, text, video, ui, netscript, tables, heroes, build, all, coverage)
│  ├─ opensacred-data/        open-format types shared by the converter and the game (serde), the game's ZIP archives and KTX2 textures
│  ├─ opensacred-rules/       game rules without the engine (§6.3)
│  ├─ opensacred-scripts/     (#725) the Luau runtime of the translated scripts: mlua with Luau, sandboxed, each call bounded, its records handed to the game's host
│  ├─ opensacred-game/        Bevy application: rendering, camera, input, UI, sound, saves (§6.2)
│  ├─ opensacred-docs/        (#2793; development only) the map of the code and the tests that keep the documents and the comments in line with the tree
│  └─ opensacred-oracle/      (#363; development only, run locally, Unicorn its feature `emulator`) runs functions of the user's own Sacred.exe in Unicorn, records vectors
├─ tools/analysis/  tools/triage/   Python research scripts: an archive, and the reference the Rust readers are cross-checked against
├─ tools/common/              the developer's installation found for the Python tools (#1556)
├─ tools/ghidra/              scripts of the local Ghidra project (classes, names, library marks) and the bridge to pyghidra-mcp
├─ tools/game/                the game's remote-control client and the conformance tools
├─ tools/original/            the original's driver under Frida (#448)
├─ tools/usage/  tools/stack/  tools/deps/   the process: Claude Code usage per pull request, the issues' claims and stacked pull requests, the daily dependency update
├─ tools/ffmpeg/              the release's own FFmpeg build under the LGPL and its pinned sources (#1499)
├─ release/                   the player's archive's own files: its read-me, the licenses' template, the Fira Mono license (#1500)
├─ docs/                      the plan's details (§14): format specifications, prose notes on the original's behavior (docs/re/), decision log (docs/decisions/), the manifests of the GOG and Steam installations
├─ assets-src/  assets/  assets-hd/   generated locally, in .gitignore
└─ OriginalGame/              the developer's GOG installation (git-ignored), or a link to it; without it the tools take the one the registry names
```

### 6.2 Presentation layer (Bevy)

* **World:** sectors are streamed in around the hero; the ground is a mesh of the cells with their corner heights, its tiles and floor overlays composed by the ground's shader from a pool of tile sheets that the loaded sectors share (#2484); static objects are sprites in 3D space with correct depth; water and lava are separate materials.
* **Characters:** glTF models with a skeleton; walk, run, attack and skill animations through Bevy's animation graph with blending.
* **Effects:** the original's effect classes, each written after the original's in a module of the game rather than described as data; their particle systems run on the CPU by the machinery of `opensacred_rules::particles` and are drawn as the original's billboards (docs/re/effects.md, #1092).
* **Camera:** orthographic, 2:1 dimetric as the original's; the wheel and the keypad zoom within the original's range, 0.5 to 2.0 (`iso::ZOOM`).
* **UI:** the original's interface in Bevy UI, laid out on the original's 768-pixel-high screen and scaled to the window's height, as the world is; its pieces are assembled from the original's atlases at the original's pixels and scaled once (`ui_layers`, #2636); text through Bevy's text (parley) in the original's fonts, its glyphs drawn at the window's pixels.
* **Sound:** event profiles from `sndProfiles` → RON tables → Kira.
* **Tools:** settings in TOML, the original's menus (#1120, #1123), a debug overlay and the log on the screen, a console of the game's commands, and remote control through which Claude Code drives the game ([docs/10](docs/10-game-tools.md)).

What the game holds today is listed in `docs/11-code-map.md`, a line per module; each module of `crates/opensacred-game/src/` tells what it does in its doc comment, and one with systems registers them in a plugin of its own.

### 6.3 Rules layer (`opensacred-rules`)

Pure Rust without Bevy, its only dependencies `opensacred-data` and serde: what the original's client decides, as `docs/re/` reads it, so that a switch of the engine keeps it (ADR-04).
The crate was made on 2026-10-21, when the spawning, the fight's formulas, the ways and the creatures' decisions moved out of `opensacred-game` (#328, #364).
Today it holds the spawning, the fight's formulas with the hero's experience and levels, the ways, the creatures' decisions and casts, the archers' arrows, the drops with the item maker, the combat arts, the combos and the spells, the scripts' state (the quests, the variables and the dialogues; the scripts themselves run in Luau, ADR-13) with their rewards and their Pong, the backpack, the equipment, the potions, the weapon sets and the item sets' bonuses, the containers' contents, the trade, the blacksmith and the combat master's exchange, the horses, the weather, the peace stream's music, the undead that rise again, the land explored, the regions' kills and the particle systems' machinery.
Its modules are listed in `crates/opensacred-rules/src/lib.rs` and in `docs/11-code-map.md`, and each module's doc comment tells what it holds and which part of `docs/re/` it follows.
The day's clock and the night's light are not among them: they are `opensacred-game`'s (`daynight.rs`).
The layer is tested without graphics: unit tests, property tests (#1428) and, on the developer's machine, the oracle's vectors (§4.2).
`opensacred-game` keeps the components, the resources and the systems, which read the world for the rules and carry out what they decide: the fight (`fight.rs`), the walks (`paths.rs`, `crowd.rs`), the creatures' life and the hero's attack (`ai.rs`, `attack.rs`), and the rest alike.

## 7. Asset converter

Details for each format: [docs/02-data-formats.md](docs/02-data-formats.md), the section on conversion.

| Source | Master (stage A, `assets-src/`) | Game (stage B, `assets/`) | How it is checked |
|---|---|---|---|
| `texture.pak` etc. (ARGB4444 + zlib; A8R8G8B8 uncompressed) | PNG RGBA8 | KTX2 with zstd supercompression (#700): BC7 with a full mip chain for the opaque textures that the GPU samples, 8-bit RGBA for the others; ASTC for Android later (M8, #702) | pixel comparison with a reference decoder; viewing |
| Ground tiles, MIX atlases | PNG atlases + a UV table of 18 diamonds | KTX2 in 8-bit RGBA, one level, as the game reads them on the CPU into the pool of sheets that the ground's shader draws from (#2484) | rendering a test sector |
| `models.pak` (Granny 1.2b) | glTF 2.0 (`.glb`): meshes, skeletons, weights, materials, its textures by reference to the PNGs | the same `.glb`, referring to the KTX2 textures by path | glTF-Validator, Blender, an in-game screenshot test |
| Granny animations | glTF clips; the Bip01 bone names are kept so that clips fit models with the same skeleton | in the models' `.glb` | visual check, comparing poses in key frames |
| World: `sectors.wldx`, `sectors.keyx`, `Floor.PAK`, `Static.PAK`, `Triggers.PAK` | binary sectors (serde + postcard in a zlib stream) + RON descriptions | the same files | loading all 6,050 sectors |
| `Items.pak`, `Creature.pak`, `Weapon.pak`, `bin/*.bin` | RON tables with a typed schema; done 2026-10-17 (`tables/`), the fields of unknown meaning as spans named by offset | the same files | reading back, comparing fields with the source |
| `global.res` | Fluent (`.ftl`) + a map of hash identifiers | the same files | all 23,123 strings present |
| NetScript (`bin/**`) | Luau sources, translated from an intermediate representation (#723, ADR-13); the IR and a readable listing done 2026-10-17 as one postcard file per tree with a RON index, the listing on request | the Luau bytecode | the disassembler gets through 20 directories with no unknown records: met 2026-10-17 |
| `sound.pak`, `sndProfiles.pak` | WAV (PCM and IMA ADPCM) → FLAC; the MP3 entries decoded into FLAC (#697); profiles in RON | Opus in Ogg (#701) | listening to a sample, durations |
| `mp3/*.mp3` | FLAC, decoded from the MP3 (#697) | Opus in Ogg | playback |
| `movie/*.wmv` | FFV1 video and FLAC audio in Matroska, the frames and samples the WMV decoder gives (#698) | AV1 + Opus in WebM | playback in the game |
| `font/*.ttf` | unchanged | the same files | rendering all strings |
| `Templates/*.ptx` | hero starter kits; 2026-10-17: the sections, inflated, in postcard hero files with a RON index of level, class and name, and since then the items worn, the backpack and the arts, the rest kept whole until its layout is recovered | the same files | character creation |
| Packing | loose files, which tools edit | ZIP archives by kind (stored entries for compressed data, deflate for text), read through an asset reader with a loose overlay (#699) | the game starts from the archives alone |

Principles: the converter runs in two stages (ADR-05), is incremental (a hash-based cache), runs in parallel, and writes a manifest with the source of every output file.
Done 2026-10-17: each subcommand records `assets/manifest/<command>.ron`, since #696 `assets-src/manifest/<command>.ron` and `build` its own in `assets/` (the SHA-256 of the converter executable, of every installation file it read and of every file it wrote); `all` skips a command whose record still holds (3 s to check all ten).
Identical reruns are no longer a goal (2026-10-23, #694): stage A stays reproducible by nature and keeps its tests, so that its manifests show what a change of the converter changed; stage B's lossy encoders (AV1, Opus, BC7, later ASTC) need not be, and no test asserts their reruns.
Outputs are never committed.
The converter repairs the defects of the original's data as it writes them (#348): `assets/` holds sound data, the game reads it as it is, and the HD packs made from it (M7) start clean, while `opensacred-formats` reads the original's files as they are.
Each repair is as narrow as its defect, described in docs/02 and recorded as a decision in `docs/decisions/`.
MIT-licensed community work (open-grn, grn2glb, sacred-sdk) is used as a reference with attribution; code without a license (SacredEngineRemake) is not used.

## 8. Graphics and 4K

Details: [docs/05-renderer-vulkan-4k.md](docs/05-renderer-vulkan-4k.md).

* Rendering at the screen's native resolution; 3840×2160 is the main target mode.
* Vulkan through wgpu by default on Windows and Linux; DX12 as the fallback on Windows; Vulkan or GLES on Android.
* The original's bottleneck is thousands of draw calls on the CPU.
  Tile and sprite batching, instancing and Bevy's GPU skinning remove it.
* Visible area: as in the original, whose 768-pixel-high screen at zoom 1.0 fills the window's height, but with 4K pixel density; the zoom keeps to the original's range, 0.5 to 2.0, and no wider view is offered.
* The interface is laid out on the same 768-pixel-high screen and scaled to the window's height; its pieces are assembled at the original's pixels and scaled once (#2636), and the text's glyphs are rasterized at the window's pixels.
* The base textures go to the game as KTX2 since M3 (#700), built by stage B from the PNG masters: BC7 with a full mip chain where the GPU samples them and they are opaque (docs/decisions/drawing.md, "Textures as KTX2 with mipmaps").
* HD pipeline (after version 1.0): texture upscaling with free models (Real-ESRGAN through ncnn-vulkan, or ONNX models through the `ort` crate) and separate alpha channel processing, into PNG masters in `assets-hd/`, which stage B packs as it packs the base ones.
  Upscaling all 25,535 textures ×4 gives ≈25 GB in BC7 (estimate), so HD packs are made selectively: ground, characters, UI.

## 9. Milestones and usage

### 9.1 Usage

The work is done solo and almost entirely through Claude Code.
Every pull request records the Claude Code usage of its branch: the tokens per model, the cost of the same tokens at API prices and the agent-hours; the README's badge sums them (`docs/usage.md`).
The dollars are hypothetical: development runs on a Claude subscription, so they are not money spent.

No estimates of time or cost are kept.
The plan of 2026-10-16 estimated each milestone in agent-hours, maintainer hours and dollars; those estimates were dropped on 2026-11-17 as out of date (#2846), and the recorded usage is the count that stays.

### 9.2 Milestones

What each milestone holds; its exit criterion and its status are in [docs/07-roadmap.md](docs/07-roadmap.md), and what it delivered in §10.

| Milestone | Contents |
|---|---|
| M0 Foundation | Cargo workspace, `CLAUDE.md`, CI, GOG installation manifest; done 2026-10-16 |
| M1 Converter | all formats from §7; tests and the manifest; done 2026-10-17 |
| M2 Vertical slice | window, camera, sector streaming, ground, objects, hero with animations, sound, a walk through the starting region in 4K; G2 met 2026-10-17, closed 2026-10-23 |
| M3 Rules | Ghidra + MCP with community names; character, skills and combat arts, combat, items and loot, AI, spawning; the Unicorn oracle; the converter's two stages and the game's packed assets (#694, moved from M7 on 2026-10-23); closed 2026-10-26 |
| M4 Quests and world | NetScript translated into Luau and run, dialogues, triggers, NPCs, day and night, the weather's choice (the trade and the blacksmith moved to M5 on 2026-10-26); closed 2026-10-26 |
| M5 UI and effects | full UI with the inventory, the trade and the blacksmith (from M4, 2026-10-26), 178 effects, the weather drawn, profile-driven sound, movies, saves (from M6 on 2026-11-03, ahead of the rest of M5, so that the campaigns could be played through and checked); closed 2026-11-09 |
| M6 Version 1.0 for Windows 11 | settings, 4K and HiDPI, polish, campaign playthroughs; the add-on's script forms and the difficulty's remaining AI chances, which waited for M6's Underworld campaign and bosses (from M5, 2026-11-03); the hero's export, which carries a hero from the Ancaria campaign into the Underworld's (from M9, 2026-11-10, #2358); the gamepad moved to M8 on 2026-11-09; every issue closed by 2026-11-17; since 2026-11-18 the defects that the documents' audit read (#2787) and what the notes list as not followed (#2845, #2950), before the maintainer's play-through and version 1.0 (G5) |
| M7 HD assets | upscaling, HD packs, built by stage B; its research begun (§10), the rest after version 1.0 |
| M8 Platforms | Linux; Android; the gamepad (from M6, 2026-11-09, #1097) |
| M9 Multiplayer | the campaigns played together over the network: the protocol, a host or a server, joining a game, the multiplayer script trees (docs/02 §7.2); how it is made is an open question (§13) |

## 10. Work order

The plan of 2026-10-16 for the first thirty days was M0, the converter's first formats and a Bevy application with one sector and the hero: that was done by 2026-10-17, and by 2026-11-17 M0 to M5 were done and every issue of M6 was closed.
This section records, at the level of a plan, when each milestone was done and what it delivered.
The day-by-day record is in the Git history (this section held it until 2026-11-17, for the days to 2026-10-28), the pull requests and the issues of the GitHub milestones M2–M9.
The details are in their own documents: the formats in docs/02, the original's behavior in `docs/re/`, the deliberate deviations in `docs/decisions/`, the tools and the process in docs/03, and what each crate holds today in `docs/11-code-map.md`.

* **M0 Foundation: done 2026-10-16.**
  The Cargo workspace (`opensacred-formats`, `opensacred-data`, `opensacred-convert` with its `verify`), `CLAUDE.md`, `rust-toolchain.toml`, `deny.toml`, CI on GitHub Actions, and the SHA-256 manifest of the reference GOG installation, `docs/manifest-gog-2.28.txt`, which the converter checks an installation against.
  The manifest lists 313 files: the files the game writes left it on 2026-10-17 (docs/02 §13), and the six that nothing of the project reads on 2026-10-29 (#1482).
  The process followed on 2026-10-17: every change reaches `main` through a squash-merged pull request with green checks (#1), each pull request carries its Claude Code usage (#16, docs/usage.md), and the queue is the issues (§10.1).
* **M1 Converter: done 2026-10-17**, its four exit criteria met (docs/07 §1).
  The readers of `opensacred-formats`, cross-checked against the Python prototypes of `tools/analysis/`, and the converter's extractions: the textures, the Granny models with their animations as glTF that glTF-Validator passes without errors or warnings, the 6,050 sectors of the world with the statics' sprites, the sound, the texts, the videos, the interface, the tables, the hero templates and the NetScript trees (§7, docs/02).
  `coverage` accounts for every file of the manifest (G1), and each command records a manifest of its run, by which `all` skips what still holds.
  Since then the converter took its two stages (M3), the repairs of the original's data (#348), the scripts' translation into Luau (M4), and the player's run and the Steam installation (M6).
* **M2 Vertical slice: G2 met 2026-10-17, the milestone closed 2026-10-23.**
  `opensacred-game` on Bevy 0.20: the sectors streamed about the hero with their ground, liquids and static objects under the original's 2:1 dimetric camera, the hero walking and running, the start script's NPCs and objects, footsteps, music and ambient loops, day and night, the buildings' insides and upper floors, and the screenshot test of a synthetic scene that CI runs on lavapipe (docs/05).
  The tools to test and drive the game (#165, docs/10) came on 2026-10-19 and 2026-10-20: the settings' file, the menus, the debug overlay, the log, the console, the scripts of `--exec`, and remote control (`--remote`, `tools/game/remote.py`, the skill `game`), through which Claude Code plays the game itself.
* **M3 Rules: closed 2026-10-26.**
  The Ghidra project was set up ahead of it, on 2026-10-18 and 2026-10-19 (moved from M0, as M1 and M2 worked from the data): the classes from RTTI, the community names, the library marks, and the names carried between `Sacred.exe` and `GameServer.exe` (#88; docs/03 §3.2, §4.1; `tools/ghidra/`).
  Read off the code into `docs/re/` and made: the world's creatures by the sectors' script functions and their `FillSector` records, kept with their sectors; the creatures' life and the townsfolk's day; the fight with health, death, experience and levels, the bosses and the champions, the archers; the drops and their pick-up; the heroes' items; the combat arts, each class's special weapon arts, the spells, the followers and the summonings.
  The rules moved out of the game into `opensacred-rules` on 2026-10-21 (#328, #364), and `opensacred-oracle` (#363) recorded its first vectors, which found five bugs of the rules, all fixed (§4.2).
  The driver of the running original (#448, 2026-10-22; docs/03 §3.8) lets Claude Code make the checks in the original itself (those of #227 closed on 2026-10-25), and `tools/game/compare.py` and `conformance.py` compare the two games' creatures, values, drops, strikes and arts.
  The converter took its two stages on 2026-10-23 and 2026-10-24 (#694, from M7): the masters in `assets-src/`, and `build` into KTX2 (#700), Ogg Opus (#701) and ZIP archives (#699); since 2026-10-21 it repairs the defects of the original's data as it writes them (#348).
  The maintainer's choices of 2026-10-23: multiplayer after version 1.0 (M9), the browser version dropped, and the scripts translated into Luau (ADR-09, ADR-13).
* **M4 Quests and world: closed 2026-10-26.**
  The converter translates the NetScript trees into Luau and compiles them (#723), `opensacred-scripts` runs them sandboxed and bounded (#725), `opensacred-rules` holds the quests' state, the variables and the dialogues' table (#724), and the game runs the scripts through the play (#726): the start's, the sectors' and the regions' functions, the base triggers and their teleports, the timers, the creatures and objects that the records make, move and remove, the deaths' functions, and the world's changes (`RegionChange`, `ReplaceSpawn`).
  With them came the dialogues and the icons over the creatures (#1028), the scripts' hired followers (#657) and the weather's choice (#1072).
  The dynamic quests were read and found dead in the release (docs/re/netscript.md, "The dynamic quests").
  The texts' defects are repaired with words of the project's own (#403, #685; docs/decisions/text.md).
  The trade and the blacksmith went to M5 on 2026-10-26, where the inventory's windows came.
* **M5 UI and effects: closed 2026-11-09**, with 892 issues.
  The original's main menu and choice of the hero with the difficulty levels (#1118, #1119), the interface over the world and the dialogue's window (#1087), the inventory with the equipment, the belt, the weapon sets and the tooltips (#1088), the character's sheet (#1089), the combat arts' window, the art slots and the combos (#1090), the quest book and the maps (#1091), the trade (#1066), the blacksmith (#1067) and the item sets' bonuses (#1727).
  The weather drawn and heard (#1073), the sound by the profiles (#1093), the effects of the spells, the arts and the world as the original's particle systems (#1092), the creatures' casts (#1852), and the movies where the original plays them (#1094).
  The saves came from M6 on 2026-11-03, ahead of the rest of M5, so that the campaigns could be played through and checked (#1095).
  The guardrails grew beside it, on 2026-10-29: property tests of the rules and round trips of the open formats (#1428, #1434), the oracle's `fuzz` (#1431), mutation testing of the lines a pull request changes (#1432, a required check since #2690), and the tools that keep parallel sessions apart (#1396; docs/03 §3.4, §3.5).
* **M6 Version 1.0 for Windows 11: every issue closed by 2026-11-17, 144 open on 2026-11-20 of those filed since by the documents' audit and the sortings of the notes; the milestone open until version 1.0** (G5).
  The player's release (#1494): the converter finds a GOG or Steam installation (#1484, #1496) and converts it in one run (#1497), which the packaged game starts at its first start (#1498); the Release workflow publishes the newest version at most once a day, with a 7z archive of the game, the converter and the project's own FFmpeg build (#1548, #1500, #1499).
  Every key in `controls.toml`, the defaults the original's (#1553); the original's options window and the settings behind it (#1096); 4K and HiDPI (#1098).
  The horses (#1822); the Underworld campaign with its heroes, a hero carried into it and the hero's export (#1117, #2362, #2358); the difficulty's other rules (#1125) and the Underworld's bosses' specials (#2548).
  Both campaigns were played through to their ends in Claude Code's driven runs on 2026-11-10, no critical bug stopping either (#1099, G4).
  The game's assets lost what the game never reads (#2645), the loaded sectors share the ground's tile sheets and the statics' sprites (#2484, #2485), and the interface's pieces are assembled at the original's pixels before they are scaled (#2636).
  The research of the soft shadows (#1436) ended on 2026-11-11; the next day the shadow-mapped ones left `main` for the experiments' branch (#2623), and the models' flattened shadows took soft edges (#2624).
  The audit of the documents (#2787) then read the code against its notes: the defects it found are M6's (#2797 to #2833, #2839, #2858, #2859), and so is what the notes list as not followed, sorted into 64 issues (#2845, #2886 to #2949), by the maintainer's decisions of 2026-11-18.
  The notes' other lines of what is not followed were sorted on 2026-11-20 into 110 more (#2950, #3064 to #3175), the maintainer agreeing to the sorting's recommendations the same day.
  On 2026-11-20, 99 of M6's issues having closed since 2026-11-18, 144 are open: 36 `bug`, 69 `feature` and 39 `research`; 110 of them come from #2950, 21 from #2845, 4 from the audit (#2844, #2870 to #2872) and 9 were filed while working on them.
  What remains after them is the maintainer's own play-through, after which version 1.0 is released (the decision of 2026-11-10, #2479).
* **M7 HD assets: research begun, the rest after version 1.0.**
  Done on 2026-11-12: the game's recording into one lossless Matroska file, each frame at its time, with the game's sound (#1225).
  By the maintainer's decision of 2026-11-12 `main` keeps the original's shadows alone, with softer edges, and the lighting, shading and depth experiments live on a branch of their own, kept for M7 and never merged whole (#2622).
  Open with it: the statics' sprites with a depth per texel (#1437), the sprites receiving the sun's soft shadows (#2604), and the statics placed from a depth model's view of the whole scene (#2621).
  The upscaling and the HD packs of §8 have not started.
* **M8 Platforms and M9 Multiplayer: after version 1.0.**
  M8 holds the gamepad (#1097) and the ASTC textures for Android (#702); M9 has no issue yet (§13).

### 10.1 Priority queue

Since 2026-10-17 the queue is the repository's open issues (github.com/zarcsis/OpenSacred/issues), and the milestones M2–M9 of §9.2 are GitHub milestones.
Work takes this order: the known problems and bugs (`bug`), then the dependency upgrades (`deps`), then new features (`feature`, `research`), the earliest milestone first.
The dependencies are updated every day without a session (#1550): the Dependencies workflow (`.github/workflows/deps.yml`, running `tools/deps/daily.py`) moves every package to its newest compatible version in a pull request that merges itself once its checks are green, and opens a `deps` issue for every newer incompatible release of a direct dependency, of a Python package, of Rust, of Python and of a source of the release's FFmpeg (`CLAUDE.md`, "Work order"; docs/03 §3.3).
A session takes an issue with `tools/stack/claim.py`, which refuses one that another session holds or that another account filed (`CLAUDE.md`, "Issues and pull requests").
On 2026-11-20 the queue holds M6's issues, the bugs first, and no `deps` issue; among them are what the audit left to settle (#2844, #2870 to #2872); beside them are open the research of M7 (#1437, #2604, #2621, #2622) and M8's gamepad (#1097) and ASTC textures (#702).

## 11. Top risks

The table is the plan's of 2026-10-16 with its later rows; the full register is docs/08 §7.
Where its first rows stand on 2026-11-17: the animations converted and passed their checks in M1; the rules are compared with the oracle's vectors and the running original since M3; both campaigns' scripts ran to their ends in M6 (#1099); and Bevy has had no breaking release since the start.

| Risk | Probability | Impact | What we do |
|---|---|---|---|
| Granny animations convert with errors (poses, weights) | medium | high | three open readers of the format as a reference; control poses; an early start in M1 |
| Combat and loot rules are recovered incorrectly | medium | medium | the Unicorn oracle and vectors; observations of the running original (#227, #448); a departure is a recorded choice, never an error left unnoticed |
| NetScript semantics are harder than expected | medium | high | a disassembler and listings early; opcodes are covered by frequency: 45 types account for 95% of records; the translator refuses a control flow or a type it does not know (#723), and the running original is the reference (#448) |
| Bevy breaks its API more often than is worthwhile | high | medium | scheduled updates; rules outside the engine; the fallback is Godot + godot-rust |
| Claude Code usage limits slow the pace | medium | medium | the usage tracked from day one (docs/usage.md); a cheaper model for mechanical work; small tasks |
| Maintainer burnout | medium | high | the vertical slice gives a visible result early; milestones are short |
| Claude Code misunderstands the original's behavior | medium | medium | vector tests, screenshot tests, the decision log; Claude Code's own observations of the running original (#448); maintainer review of key formulas |
| Game content gets into the repository (a test fixture, an issue attachment, converter output) | low | high | no game bytes in the repository or CI; tests use synthetic data or the user's local installation; the same rules for issues and pull requests (§12) |
| The single-player game makes multiplayer expensive after 1.0 | medium | medium | ADR-09: the rules apart from the engine, the world's state in the rules and the ECS, the scripts run on the host |

## 12. Legal and asset policy

Details: [docs/08-risks-legal.md](docs/08-risks-legal.md).
This is not legal advice.

* Every user needs their own lawful copy of the game: the GOG release, or the Steam one, which the converter takes too (#1484).
  Interoperability work (studying the formats and the behavior, writing our own code) relies on Articles 5(3) and 6 of Directive 2009/24/EC, which a contract cannot override (Article 8).
  The GOG release has no copy protection, so nothing is circumvented; the Steam release's `Sacred.exe` is wrapped in Steam's DRM, which encrypts its code: the converter reads only the data sections that the DRM leaves as they are and decrypts nothing (docs/08 §2).
* The repository and CI hold no game bytes: no game files, no `Sacred.exe`, no converted assets, HD packs, memory dumps, decompiler output or game texts.
  Tests that need the game run locally on the developer's installation, `OriginalGame/` (git-ignored) or the GOG one that the registry names (#1556); CI runs only the tests that need no game files.
* Converted output is generated locally from the user's own copy and is never published.
  The same rules apply to issues and pull requests, which carry only the frames of a problem, cropped to it, as their attachments (docs/08 §3, #1222).
* OpenSacred is not affiliated with the rights holder; Sacred is a trademark of THQ Nordic GmbH.
  The name and its use: docs/08 §6, "Name and trademark".

## 13. Open questions

1. Whether Android is needed: a decision after version 1.0; the browser version was dropped on 2026-10-23.
2. How multiplayer is made after version 1.0 (M9): our own protocol between OpenSacred games or the original's (TinCat 2, docs/09 §3.2), so that the original clients could join; a host's game or a dedicated server; which network crate.

Closed on 2026-10-16: the container and codecs of `sound.pak` (docs/02, §10.2), the channel order of format 6 textures (docs/02, §3.1), how completely the Granny animation curves carry over to glTF (all of them do, docs/02 §4.5); on 2026-10-17, where the image of the extensionless static templates of `Items.pak` lives (the `mixed.pak` sprite named at `+0x10`, docs/02 §3.3, §6.1).

## 14. Document index

| Document | Contents |
|---|---|
| [docs/01-binary-analysis.md](docs/01-binary-analysis.md) | executables: compiler, functions, classes, modules, what to analyze |
| [docs/02-data-formats.md](docs/02-data-formats.md) | data formats and conversion targets |
| [docs/03-toolchain.md](docs/03-toolchain.md) | free tools, toolchains, the workflow with Claude Code |
| [docs/04-language-and-engine.md](docs/04-language-and-engine.md) | Rust or C++, engine comparison |
| [docs/05-renderer-vulkan-4k.md](docs/05-renderer-vulkan-4k.md) | rendering on Bevy and wgpu, 4K, HD assets |
| [docs/06-platform-backends.md](docs/06-platform-backends.md) | platforms, sound, input, video, files, saves |
| [docs/07-roadmap.md](docs/07-roadmap.md) | the milestones: what each holds, its exit criterion and its status |
| [docs/08-risks-legal.md](docs/08-risks-legal.md) | risks and the legal framework |
| [docs/09-ecosystem.md](docs/09-ecosystem.md) | game versions, community work, comparable projects |
| [docs/10-game-tools.md](docs/10-game-tools.md) | tools to test and drive the game: settings, menu, overlays, log, console, remote control |
| [docs/11-code-map.md](docs/11-code-map.md) | the map of the code: a line per source file of `crates/`, generated from the file's own header |
| [docs/re/](docs/re/) | the original's behavior described in prose, per subsystem |
| [docs/decisions/](docs/decisions/) | deliberate deviations from the original's behavior, a table per subsystem ([README](docs/decisions/README.md)) |
| [docs/usage.md](docs/usage.md) | how Claude Code usage is recorded per pull request; the sessions before |
| [docs/manifest-gog-2.28.txt](docs/manifest-gog-2.28.txt), [docs/manifest-steam-2.28.txt](docs/manifest-steam-2.28.txt) | the SHA-256 of the reference GOG installation's files, and of the Steam release's four files that differ |
| [tools/README.md](tools/README.md) | the research analysis scripts, the Ghidra project's scripts, the game's remote control, the original's driver and the process tools |
| [README.md](README.md) | the project's overview: status, requirements, getting started, the keys |
| [CLAUDE.md](CLAUDE.md) | the conventions of development with Claude Code |
| [.claude/skills/game/SKILL.md](.claude/skills/game/SKILL.md), [.claude/skills/original/SKILL.md](.claude/skills/original/SKILL.md) | Claude Code's project skills: how to run and drive the game, and the original |
| [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) | the notices of the community projects whose facts the readers follow |
| [release/README.txt](release/README.txt) | the read-me of the player's archive |

## Sources

* Versions as of 2026-10-16: crates.io (bevy 0.20.0, wgpu 30.0.1, fyrox 1.0.1, godot 0.5.5, kira 0.12.5, symphonia 0.6.1, rav1d 1.1.0, egui 0.37.0, cosmic-text 0.19.0); the Rust stable channel (1.99.1); Bevy 0.20 news (bevy.org/news/bevy-0-20); Godot 4.7.2 (godotengine.org); Unreal Engine 5.8 (unrealengine.com); Vulkan SDK 1.4.357.0 (lunarg.com); Visual Studio 2026 with MSVC 14.50 (devblogs.microsoft.com/cppblog); Ghidra 12.1.4 and GhidraMCP 2.0.
* Claude prices: the Claude API reference (cached as of 2026-10-06).
