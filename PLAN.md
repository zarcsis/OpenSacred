# Plan: OpenSacred

The main plan document.
The project's goal is to **play Sacred Gold 2.28 (GOG) comfortably on modern hardware**: Windows 11 x64 at 4K, later Linux, possibly Android.
The tools work on the user's own GOG installation; the repository holds no game files (§12).
This document is self-contained; details are in `docs/01…10`, analysis scripts in `tools/` ([description](tools/README.md)).

Claim status is marked with the words **confirmed** (verified on GOG release files with a script or a disassembler), **community-reported** (not verified on a file), **estimate** (model calculation) and **hypothesis**.

## Summary

* **Approach: "convert the assets + rewrite the game", not "decompile and rebuild".**
  A toolset reads the assets of the user's GOG installation once and converts them into open formats (PNG/KTX2, glTF, FLAC/Ogg, AV1, RON/Fluent).
  The new game reads only these formats.
  Where the game's rules are not evident from the data, `Sacred.exe` is studied only to understand them; the behavior is described in prose and implemented independently in Rust with a redesign; an exact copy of the behavior is not required.
* **Language: Rust for the entire product**: the game, the converter, the tools.
  VC6 and old toolchains are not needed: byte-for-byte matching is not a goal.
* **Engine: Bevy 0.20 on wgpu** (Vulkan on Windows, Linux and Android).
  The game rules live in a separate crate with no engine dependency, so if needed we can switch to Godot with godot-rust without losing the logic.
  Unreal Engine and Godot with C++ were considered and rejected (§5).
* **Multiplayer after version 1.0:** until then nothing networked, no server and no lobby, but nothing that rules multiplayer out (ADR-09).
  Compatibility with the original saves is not needed.
* **Only free tools and dependencies:** Ghidra 12.2 with an MCP server for Claude Code, Rust, Visual Studio Build Tools 2026, Vulkan SDK, Unicorn, FFmpeg, Blender, RenderDoc.
* **Solo development, almost entirely through Claude Code.**
  Estimates are given in hours of active agent work and in tokens (§9).
  To a playable version on Windows 11: **≈390–770 agent-hours** (estimate).
  At API prices for Claude Opus 5.5 the same tokens would cost ≈$3k–14k, center ≈$6k; development runs on a Claude subscription, so this is a hypothetical figure, not money spent (§9).
  The first visible result, a walk through the starting region in 4K, comes after ≈80–160 agent-hours.

## 1. Requirements and goals

### 1.1 Project requirements (2026-10-16)

| Requirement | Consequence for the plan |
|---|---|
| The main goal is to play comfortably on modern hardware | The success criterion is completing the Sacred and Underworld campaigns in 4K without crashes, not matching the original |
| Fidelity to the original is not a priority; modern improvements are acceptable | No byte-for-byte matching, no VC6, no determinism requirements; behavior may be redesigned |
| Multiplayer after version 1.0 (changed 2026-10-23: no network components were planned before) | No network code, TinCat, lobby or server before 1.0; the game is built so that a host can later run it for several players (ADR-09) |
| Work only with the GOG version | The reference is GOG ENG 2.28; the tools read the user's own installation |
| The game does not have to read the original formats and saves | An offline converter into open formats; the runtime reads only those; HD and upscale packs in the future |
| Solo development almost entirely through Claude Code | Estimates in agent-hours and tokens; the process is built around tests and text artifacts |
| Modern toolchains: Visual Studio Build Tools 2026, latest Vulkan, newest dependencies | Latest stable Rust, scheduled dependency updates |
| One language for the whole project, preferably Rust | Rust everywhere |
| Only free tools and dependencies | Ghidra instead of IDA/Binary Ninja; all software and libraries are open source or free of charge |
| Platforms: Windows 11 x64; then Linux; possibly Android (changed 2026-10-23: the browser version dropped) | wgpu covers all three targets with one codebase |
| Consider an existing engine (UE, Godot) | Comparison in §5 and `docs/04-language-and-engine.md` |

### 1.2 Goals and measurable criteria

| # | Goal | Criterion |
|---|---|---|
| G1 | Game assets are available in open formats | The converter processes 100% of the files of the reference GOG installation per the manifest; models open in Blender and pass glTF-Validator; a rerun gives the same result. 2026-10-17: `opensacred-convert coverage` gives, of the manifest's 319 files, 303 converted and 16 skipped with a reason, none left; tables and hero files keep the fields not yet understood as raw spans |
| G2 | Vertical slice | The hero walks through the starting region with animations, sound and UI at 3840×2160 at ≥ 60 fps on a mid-range PC. Met 2026-10-17: about 775 fps at 3840 × 2160 while the hero runs across three sectors, about 580 on 2026-10-19 (docs/05 §3.6, docs/10) |
| G3 | Game rules | Combat, attributes, skills, items and loot work by the recovered formulas; deviations from the original are deliberate and recorded |
| G4 | Campaign content | Quests, dialogues and triggers of the Sacred and Underworld campaigns run from converted scripts; both campaigns are completable from start to finish |
| G5 | Playable version 1.0 for Windows 11 x64 | Full UI, saves, settings, gamepad; 4K and HiDPI; a playthrough without critical bugs |
| G6 | HD assets | The upscale pipeline produces HD packs that the game picks up on top of the base assets |
| G7 | Other platforms | Linux: the same functionality; Android: by a separate decision after G5 |
| G8 | Multiplayer | After G5: the Sacred and Underworld campaigns played together over the network; how is decided when the work starts (§13) |

**Out of scope:** networking before version 1.0 (multiplayer comes after it, G8); loading the original saves; byte-for-byte compatibility; the Steam version; macOS and iOS; the browser version (dropped 2026-10-23); publishing the game assets in any form.

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
| ADR-01 Goal | A playable modern reimplementation on the original's converted assets, inspired by the original, not an exact copy | Requirement (§1.1); removes the most expensive requirements (matching, determinism, save compatibility) |
| ADR-02 Strategy | Asset converter + an independent reimplementation of the game; the original's behavior is studied in the binary selectively, only where the rules are not evident from the data, described in prose and implemented anew (§4.2) | Reproducing the original's rendering, UI, sound, input and file handling from its code is pointless; the binary's value lies in the rules and the content semantics |
| ADR-03 Language | Rust (edition 2024, latest stable is 1.99.1) for the entire product; the Python research scripts remain as an archive | A single language, memory safety, a modern ecosystem |
| ADR-04 Engine | Bevy 0.20 (wgpu 30) for presentation; the rules crate `opensacred-rules` does not depend on the engine (created 2026-10-21 with the spawning's rolls, then the fight's formulas, the ways and the creatures' decisions moved into it from `opensacred-game`, #364) | Pure Rust, everything in code and text, which suits Claude Code; Vulkan and Android from one codebase; the fallback is Godot 4.7 with godot-rust on top of the same rules crate |
| ADR-05 Assets | Offline converter `opensacred-convert` (Rust CLI) in two stages (#694, the maintainer's choice of 2026-10-23): **extract**, the installation → lossless masters in open formats that tools edit, with the repairs of the data, in `assets-src/` (textures → PNG, models and animations → glTF 2.0, world → binary sectors + RON, tables → RON, texts → Fluent, sound and music → FLAC, video → FFV1 and FLAC in Matroska, scripts → Luau, ADR-13); **build**, the masters → the game's packed assets in `assets/` (textures → KTX2 with BC7 and mipmaps, sound → Opus in Ogg, video → AV1 and Opus in WebM, scripts → Luau bytecode, all in ZIP archives); `all` does both; the game reads only the built assets; HD packs and mods are made in the masters' formats and built by the same stage B (§7) | Requirement (§1.1); the masters can be checked and edited with third-party tools (Blender, glTF-Validator) and lose nothing, so a build from them gives what a direct conversion would; the game's formats load fast and fit the GPU |
| ADR-06 Graphics and 4K | Rendering at native resolution through wgpu (Vulkan by default on Windows, DX12 as fallback); an orthographic camera with the original's angle and adjustable zoom; the ground is real 3D terrain from a heightmap; scalable UI; HD packs on top of the base assets | The original is bound by the CPU and by 1024×768, not by the GPU; a modern renderer removes both limits |
| ADR-07 Sound, input, video | Kira via `bevy_kira_audio`, the sound and the music as Ogg Opus decoded through libopus (#701), the one C library of the sound, which the build compiles with CMake; Bevy input with key remapping and gamepad; video: AV1 with the rav1d decoder | Everything is free and builds for Windows, Linux and Android |
| ADR-08 Saves and settings | Our own versioned save format (serde, RON or postcard); settings in TOML in the user directory | The original saves are not needed; a modern format is easier to debug |
| ADR-09 Network | Multiplayer after version 1.0 (M9), no network code, server or lobby before it; until then the game does not rule it out: the rules stay apart from the engine (ADR-04), the world's state lives in the rules and the ECS rather than in presentation or input, and the scripts run on the host, as in the original (docs/02 §7.5); `GameServer.exe` is an auxiliary binary when analyzing shared functions, and a reference for the server's part once M9 starts | Requirement (§1.1), changed 2026-10-23 |
| ADR-10 Tools | Free only: Ghidra 12.2 + pyghidra-mcp (GhidraMCP 2.0 as the alternative with the GUI), Unicorn (emulating the original's functions for tests), Frida (driving the running original for Claude Code, #448), x64dbg, dgVoodoo2 (optional), RenderDoc, Vulkan SDK 1.4, Blender, FFmpeg, Visual Studio Build Tools 2026 | Requirement (§1.1); IDA Pro and Binary Ninja are excluded |
| ADR-11 Process | Claude Code as the main executor: `CLAUDE.md` with rules, MCP to Ghidra, tests as guardrails (converter golden data, oracle vectors, screenshot tests), small tasks, maintainer review | Solo development; quality rests on automated checks, not on manual review of every line |
| ADR-12 Repository and legal | The repository is public and holds no game bytes: no game files, converted assets, HD packs or dumps, in the repository or in CI; users supply their own GOG copy and run the converter locally; converted output is never published (§12, docs/08) | The game belongs to the rights holder; every user needs their own copy |
| ADR-13 Scripts | The converter translates the NetScript bytecode into Luau, a source file per quest, sector, region and start script, with its bytecode; the game runs only Luau, through mlua and its vendored Luau in a crate of its own (`opensacred-scripts`), sandboxed and bounded; what each record type does is the scripts' API, recovered from the original and implemented in `opensacred-rules` and `opensacred-game`; the quests' state, the variables and the triggers live in the engine and the saves, not in Luau; mods after 1.0 are Luau on the same API; the translated scripts are converted assets, never published (#720) | Decided 2026-10-23: the operands of the 113 types are decoded once, in the translator, not in an interpreter and again in a translator; the reference is the running original (#448), not an interpreter of ours; Luau rather than Rhai or Lua 5.4 for its types, its sandbox and interrupts, its bytecode compiled ahead and its speed; its WebAssembly needs Emscripten, which the dropped browser version makes moot |

## 4. Strategy

### 4.1 Pipeline

```text
OriginalGame/      ← the user's own GOG installation (git-ignored)
   │  opensacred-convert extract (stage A: lossless, with the data's repairs)
   ▼
assets-src/        ← masters in open formats that tools edit (PNG, glTF, FLAC, FFV1, RON, Luau)
assets-hd/         ← HD packs and mods in the masters' formats, over assets-src/ at matching paths
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

### 4.2 How the rules are recovered

1. **A local Ghidra project on the user's own GOG `Sacred.exe`** with imported community names (sacred-bild, sacred-sdk) and RTTI.
   Claude Code reads the decompilation through the MCP server only to understand the formats and the rules.
2. **Choosing a mode for each subsystem:**
   * rules with numbers (damage, defense, resistances, experience, skills, item generation, prices) are studied in the code;
   * content semantics (NetScript opcodes, triggers, spawning) are studied in the code and the data;
   * everything else (rendering, UI, sound, input, files, camera, pathfinding) is written from scratch without reading the original's code.
3. **A description in prose.**
   What the original does (inputs, outputs, formulas, tables, order of events) is written down per subsystem in `docs/re/`, in our own words, not as code.
4. **Reference vectors.**
   For formula functions, `opensacred-oracle` runs the functions of the user's own `Sacred.exe` in the Unicorn emulator on sets of inputs and records the outputs as vectors.
   It runs locally on any OS and does not require running the game; CI has no `Sacred.exe`, so the tests on the vectors run locally, like the other tests on the game files.
5. **Observations of the running original.**
   Where the oracle cannot reach (a rule spread over threads and frames, the engine's timing, the creatures' choices, what the screen shows), the running original is observed.
   Until 2026-10-21 that needed the maintainer at the game (#227); the driver of #448 (planned) lets Claude Code do it itself: `tools/original/` runs the user's own `Sacred.exe` under Frida in the background, reads its state, sends its keys and clicks, runs commands, traces its functions and takes its frames (docs/03 §3.8).
   Its frames, logs and traces stay on the developer's machine; the results go into the issues and `docs/re/` as prose.
6. **Rust, written independently, then redesigned.**
   `opensacred-rules` implements the behavior from the description and passes the vectors.
   After that it can be changed deliberately: every deviation from the original is recorded in the decision log (`docs/decisions/`).
   Status 2026-10-21: `opensacred-rules` holds the spawning's rolls (#359), the fight's formulas with the hero's levels (#360), the ways (#361) and the creatures' decisions (#362), which `opensacred-game` calls; `opensacred-oracle` (#363) records the vectors of the levels' ends and the levels of an experience, the hit chance and the damage through a resistance (`0x43EEE0`) and the most health, which `opensacred-rules`' tests compare with: the levels agree, the others showed five bugs, of which #406, #407 and #408 are fixed and #417 and #418 open; the checks in the original are gathered in #227.
7. **Nothing from the study is committed.**
   The Ghidra project, decompiler output and anything else made locally while studying the original (scratch C++ in a local `reference/` directory included) stay on the development machine; the repository holds only the prose in `docs/re/` and our own code.

### 4.3 Rejected options

| Option | Why rejected |
|---|---|
| Byte-for-byte decompilation for VC6 (LEGO Island style) | The goal is not an exact copy; doubles the cost; requires VC6 |
| Hybrid bridge inside the original's process (OpenRCT2 style) | Ties us to a 32-bit process and D3D7; needed only if parity is required |
| Loading the original formats at runtime | Not needed per the requirements (§1.1); the converter gives open formats and HD packs |
| Pure reimplementation without analyzing the binary | Combat and item rules and script semantics are undocumented; without the original's code the game will be "similar" but with different numbers |
| Translating the decompiled code into Rust | The rules are implemented independently from prose descriptions of the behavior (§4.2), which also leaves room for the redesign |

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
  A migration is a 2–6 agent-hour task per release (estimate); every release has a migration guide.
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
├─ CLAUDE.md                  project rules for Claude Code
├─ crates/
│  ├─ opensacred-formats/     reading the original formats (PAK, TEX, GRN, sectors, NetScript, global.res, AMH templates)
│  ├─ opensacred-convert/     CLI: OriginalGame → assets-src/ → assets/ (subcommands verify, textures, models, world, sound, text, video, ui, netscript, tables, heroes, build, all, coverage)
│  ├─ opensacred-data/        open-format types shared by the converter and the game (serde)
│  ├─ opensacred-rules/       game rules without the engine: character, combat, items, AI, the quests' state (the spawning's rolls, the fight's formulas, the ways and the creatures' decisions so far, #364)
│  ├─ opensacred-scripts/     (#725, planned) the Luau runtime of the translated scripts and of mods: mlua with Luau, the scripts' API
│  ├─ opensacred-game/        Bevy application: rendering, camera, input, UI, sound, saves
│  └─ opensacred-oracle/      (#363; development only, run locally, Unicorn its feature `emulator`) runs functions of the user's own Sacred.exe in Unicorn, records vectors
├─ tools/analysis/            Python research scripts (archive, gradually replaced by Rust)
├─ tools/ghidra/              scripts of the local Ghidra project (classes, names, library marks)
├─ tools/game/                the game's remote-control client
├─ tools/usage/  tools/stack/ the process: Claude Code usage per pull request, stacked pull requests
├─ docs/                      plan, format specifications, prose notes on the original's behavior (docs/re/), decision log
├─ assets-src/  assets/  assets-hd/   generated locally, in .gitignore
└─ OriginalGame/              the user's GOG installation (git-ignored)
```

### 6.2 Presentation layer (Bevy)

* **World:** sectors are streamed in around the hero; the ground is built as a mesh from the heightmap with tile atlases; static objects are sprites in 3D space with correct depth; water and lava are separate materials.
* **Characters:** glTF models with a skeleton; walk, run, attack and skill animations through Bevy's animation graph with blending.
* **Effects:** the original's 178 effects are described as data; rendering uses GPU or CPU particles.
* **Camera:** orthographic with the original's angle; zoom with smooth approach; an extended view option.
* **UI:** a new scalable interface in Bevy UI; text through cosmic-text at native resolution; the original UI textures serve as skins.
* **Sound:** event profiles from `sndProfiles` → RON tables → Kira.
* **Tools:** settings in TOML, a stopgap menu, a debug overlay and the log on the screen, a console of the game's commands, and remote control through which Claude Code drives the game ([docs/10](docs/10-game-tools.md)).

### 6.3 Rules layer (`opensacred-rules`)

Pure Rust without Bevy: the character and attribute model, combat formulas, skills and combat arts, items and loot generation, trading and the blacksmith, creature AI, spawning, time of day and weather, the quests' state and the scripts' variables (the scripts themselves run in Luau, ADR-13).
The layer is tested without graphics.
The Bevy layer calls it through a thin ECS adapter.
Status 2026-10-21: the crate holds the spawning's rolls of `FillSector` (`spawning`, #359), the fight's formulas: the creatures' levels, health, attack, defence, damage and resistances, a strike's hit, the items' values, the regeneration, and the hero's experience and levels (`fight`, #360), the ways: the search of the fewest steps over a 64 × 64 window, the straight line's test of the cells it crosses, and who stops whose step into a cell that another creature holds (`ways`, #361), and the creatures' decisions: waking and dormancy by their timers and the hero's look, the idle action's steps, the animals' flight and strolls, the guards' posts, the townsfolk's and the children's rolls and the evening's way, parting and stepping aside, coming at the hero, keeping the band of the reach, fleeing and going home, and the hero's attack's choice of where it stands and when it strikes (`minds`, #362).
The fight's components, resources and systems stay in `opensacred-game` (`fight.rs`) and call them, and so do the walks: the world's points and the next point to head for (`paths.rs`), and the cells the creatures stand on (`crowd.rs`); and so do the creatures' life and the hero's attack (`ai.rs`, `attack.rs`): the minds as components, and the systems that find the creatures, the doors and the cells about a creature and carry the decisions out with the walks, the strikes and the turns.

## 7. Asset converter

Details for each format: [docs/02-data-formats.md](docs/02-data-formats.md), the section on conversion.

| Source | Master (stage A, `assets-src/`) | Game (stage B, `assets/`) | How it is checked |
|---|---|---|---|
| `texture.pak` etc. (ARGB4444 + zlib; A8R8G8B8 uncompressed) | PNG RGBA8 | KTX2 with zstd supercompression (#700): BC7 with a full mip chain for the opaque textures that the GPU samples, 8-bit RGBA for the others; ASTC for Android later (M8, #702) | pixel comparison with a reference decoder; viewing |
| Ground tiles, MIX atlases | PNG atlases + a UV table of 18 diamonds | KTX2 in 8-bit RGBA, one level, as the game composes them on the CPU | rendering a test sector |
| `models.pak` (Granny 1.2b) | glTF 2.0 (`.glb`): meshes, skeletons, weights, materials, its textures by reference to the PNGs | the same `.glb`, referring to the KTX2 textures by path | glTF-Validator, Blender, an in-game screenshot test |
| Granny animations | glTF clips; the Bip01 bone names are kept so that clips fit models with the same skeleton | in the models' `.glb` | visual check, comparing poses in key frames |
| World: `sectors.wldx`, `sectors.keyx`, `Floor.PAK`, `Static.PAK`, `Triggers.PAK` | binary sectors (serde + postcard in a zlib stream) + RON descriptions | the same files | loading all 6,050 sectors |
| `Items.pak`, `Creature.pak`, `Weapon.pak`, `bin/*.bin` | RON tables with a typed schema; done 2026-10-17 (`tables/`), the fields of unknown meaning as spans named by offset | the same files | reading back, comparing fields with the source |
| `global.res` | Fluent (`.ftl`) + a map of hash identifiers | the same files | all 23,123 strings present |
| NetScript (`bin/**`) | Luau sources, translated from an intermediate representation (#723, ADR-13); the IR and a readable listing done 2026-10-17 as one postcard file per tree with a RON index, the listing on request | the Luau bytecode | the disassembler gets through 20 directories with no unknown records: met 2026-10-17 |
| `sound.pak`, `sndProfiles.pak` | WAV (PCM and IMA ADPCM) → FLAC; the MP3 entries decoded into FLAC (#697); profiles in RON | Opus in Ogg (#701) | listening to a sample, durations |
| `mp3/*.mp3` | FLAC, decoded from the MP3 (#697) | Opus in Ogg | playback |
| `movie/*.wmv` | FFV1 video and FLAC audio in Matroska, the frames and samples the WMV decoder gives (#698) | AV1 + Opus in WebM | playback in the game |
| `font/*.ttf` | unchanged + a free Unicode font (Noto, OFL) for missing characters (the fallback not added yet) | the same files | rendering all strings |
| `Templates/*.ptx` | hero starter kits; 2026-10-17: the sections, inflated, in postcard hero files with a RON index of level, class and name, until their layout is recovered | the same files | character creation |
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
* Visible area: by default as in the original at zoom 1.0, but with 4K pixel density; an extended view is an option.
* The UI scales independently of the scene resolution; fonts are rasterized for the actual DPI.
* The base textures go to the game as KTX2 since M3 (#700), built by stage B from the PNG masters: BC7 with a full mip chain where the GPU samples them and they are opaque (docs/decisions/2026-10-23-textures.md).
* HD pipeline (after version 1.0): texture upscaling with free models (Real-ESRGAN through ncnn-vulkan, or ONNX models through the `ort` crate) and separate alpha channel processing, into PNG masters in `assets-hd/`, which stage B packs as it packs the base ones.
  Upscaling all 25,535 textures ×4 gives ≈25 GB in BC7 (estimate), so HD packs are made selectively: ground, characters, UI.

## 9. Estimates: Claude Code time and token cost

Dollar figures are the hypothetical cost of the same tokens at Anthropic API prices; development runs on a Claude subscription, so they are not money actually spent.

Details and formulas: [docs/07-roadmap-estimates.md](docs/07-roadmap-estimates.md).

### 9.1 Units

* **Agent-hour**: an hour of active Claude Code work on a task (the agent reads code, writes, runs tests).
  Parallel sessions add up.
* **Maintainer hours**: setting tasks, review, manual checks, playtesting.
* **Cost**: at API prices per million tokens as of 2026-10-16: Claude Opus 5.5: $4 input, $20 output, $0.20 cache read, $5 cache write; Claude Sonnet 5.5: $2 / $10 / $0.20 / $2.50; Claude Fable 5.1: $10 / $50 / $0.25 / $12.50.
* **Agent-hour cost model (estimate):** output with reasoning 0.2–0.5M tokens, cache reads 5–15M, cache writes 0.5–1M.
  For Opus 5.5 that is ≈$8–18 per agent-hour, for Sonnet 5.5 ≈$4–10, for Fable 5.1 ≈$18–40.
  On a Claude Code subscription the payment is fixed, and the pace is limited by usage limits.
* **Calibration on this project:** the planning-phase research took 16 agents, 1,502 tool calls, ≈4 hours and 5.1M tokens by the orchestrator's counter.
  A later revision of the documents took 32 agents, ≈1.5 hours, 6.6M tokens.
  What exactly the counter includes has not been verified, so the model above should be refined with real data: the Claude Code transcripts, recorded per pull request since 2026-10-17 (`docs/usage.md`).
  By 2026-10-20 the README's badge summed 4.15B tokens, ≈$1.2k at API prices: ≈37 agent-hours in 152 pull requests and 12.7 in the sessions before, about $24 per agent-hour, above the model's $8–18 (the subagents' time is not counted).
  The recomputation of §9 from these figures is due on 2026-11-15, 30 days after 2026-10-16.

### 9.2 Milestones

| Milestone | Contents | Agent-hours | Maintainer hours |
|---|---|---|---|
| M0 Foundation | Cargo workspace, `CLAUDE.md`, CI, GOG installation manifest | 6–12 | 4–8 |
| M1 Converter | all formats from §7; tests and the manifest | 80–160 | 25–50 |
| M2 Vertical slice | window, camera, sector streaming, ground, objects, hero with animations, sound, a walk through the starting region in 4K | 30–60 | 15–30 |
| M3 Rules | Ghidra + MCP with community names; character, skills and combat arts, combat, items and loot, AI, spawning; the Unicorn oracle; the converter's two stages and the game's packed assets (#694, moved from M7 on 2026-10-23; the estimates stand until M3's re-estimation) | 120–240 | 40–80 |
| M4 Quests and world | NetScript translated into Luau and run, dialogues, triggers, NPCs, trading, blacksmith, day and night, weather | 60–120 | 25–50 |
| M5 UI and effects | full UI, 178 effects, profile-driven sound, movies | 50–100 | 20–40 |
| M6 Version 1.0 for Windows 11 | saves, settings, gamepad, 4K and HiDPI, polish, campaign playthroughs | 40–80 | 100–200 |
| **Total to version 1.0** | | **≈390–770** | **≈230–460** |
| M7 HD assets | upscaling, HD packs, built by stage B | 15–30 | 10–30 |
| M8 Platforms | Linux 5–10; Android 25–50 | 30–60 | 10–20 |
| M9 Multiplayer | the campaigns played together over the network: the protocol, a host or a server, joining a game, the multiplayer script trees (docs/02 §7.2) | not estimated (§13) | not estimated |

### 9.3 Money and calendar (estimate)

Hypothetical cost at API prices (not money spent, see above):

| Model for the main work | To version 1.0 | With M7 and M8 |
|---|---|---|
| Claude Opus 5.5 | ≈$3.1k–13.9k (center ≈$6k) | ≈$3.5k–15.5k |
| Claude Sonnet 5.5 | ≈$1.6k–7.7k | ≈$1.7k–8.6k |
| Claude Fable 5.1 | ≈$7k–31k | ≈$7.8k–34.4k |

A sensible mode is Opus 5.5 for binary analysis and architecture, and Sonnet 5.5 for mechanical work (converter tables, UI layout).
The choice of model is up to the maintainer.

Calendar: in an intensive mode (≈40 agent-hours and 25 maintainer hours a week), the vertical slice comes in ≈1 month and version 1.0 in ≈2.5–4.5 months; "evenings and weekends" (≈15 and 10 hours) gives ≈2–3 and ≈6–12 months.
The pace is limited more by the maintainer's time for review and playtesting (≈230–460 hours) than by the agents' work.
The main sources of uncertainty (each up to ×2): recovering the combat and loot rules, NetScript semantics, correctness of the Granny animation conversion, how often the Bevy API breaks.

## 10. Work order: the first 30 days

1. **M0.**
   Done 2026-10-16: Cargo workspace (`opensacred-formats`, `opensacred-data`, `opensacred-convert` with the `verify` command), `CLAUDE.md`, `rust-toolchain.toml`, `deny.toml`, CI on GitHub Actions; `.gitignore` excludes `assets/`, `assets-hd/`, `target/`.
2. Done 2026-10-16: the SHA-256 manifest of the reference GOG installation, `docs/manifest-gog-2.28.txt` (`sha256sum` format); the converter checks the installation against it.
   Since 2026-10-17 it lists 319 files: the files the game writes (`Gameserver.cfg`, the saves of `save/`, the `DefPos.bin` caches) were removed, like `Settings.cfg` before them (docs/02 §13).
3. Moved to the start of M3 on 2026-10-16: install Ghidra 12.1.4, load the GOG `Sacred.exe` and `GameServer.exe`, import the community names, connect GhidraMCP or pyghidra-mcp to Claude Code.
   M1 and M2 work from the data and the community format descriptions; if a format question needs the original's code, Ghidra is installed earlier.
   Done 2026-10-18, ahead of M3: Ghidra 12.1.4 on Temurin 25, pyghidra-mcp connected to Claude Code, `Sacred.exe` and `GameServer.exe` imported and analyzed (docs/03 §3.2).
   On 2026-10-19 the rest followed (#88, #137, docs/03 §4.1): 410 classes from RTTI by `RttiClassesScript`, 137 functions named and 37 addresses labeled from sacred-bild and sacred-sdk, the library code tagged by `MarkLibraryScript`, and 1,546 functions matched between `Sacred.exe` and `GameServer.exe` by `CarryNamesScript`; the scripts are in `tools/ghidra/`.
4. **M1, textures.**
   Done 2026-10-16.
   The `opensacred-formats` crate got a shared PAK reader and a TEX decoder.
   `opensacred-convert textures` produces 25,536 PNGs: 25,534 from `texture.pak` (one directory entry is empty) and 2 from `texture03.pak`.
   Tests cross-check the pixels against `tools/analysis/formats/tex_decode.py` and `tex_fmt6.py` (docs/02 §3.1).
5. **M1, models.**
   A Granny 1.2b parser in Rust based on the open format descriptions; export of one hero model with its skeleton to glTF; checks in Blender and glTF-Validator.
   Started 2026-10-16: `opensacred_formats::mdl` reads the `models*.pak` containers, `opensacred_formats::grn` reads the GRN container (sections, CRCs, node trees) and the skeleton (bones, parents, bind-pose local transforms, names through the form's bone channels), and all of it is cross-checked on every record against the prototype `tools/analysis/formats/grn_dump.py` (docs/02 §4.1–4.2).
   The prototype also reads, on every file, the meshes, weights, the form's mesh bindings, textures, materials and render passes; the bind pose composition is confirmed by the per-bone bounds of the meshes, and every scale/shear matrix is symmetric, so its order does not matter (docs/02 §4.2).
   The same day `opensacred_formats::grn::Model` reads them in Rust and matches the prototype's mesh and material fingerprints on every record, and `opensacred-convert models` writes all 1,572 models to `.glb` files that glTF-Validator passes without errors or warnings; `SERAPHIM.GRN` draws in the six batches of the game's own trace (docs/02 §4.5).
   Animations, done 2026-10-16 (night session): `opensacred_formats::grn::Animation` reads every track and matches `grn_dump.py` on every record; `opensacred-convert models` now writes all 4,993 records, a model's file carrying the motions of its slots as glTF animations (quadratic curves as cubic splines, exact for translations and within 0.0025° for rotations; sheared bones as two nodes, exact), and glTF-Validator passes all of them without errors or warnings; Blender renders of the hero, wolf, dwarf and bear clips look right (docs/02 §4.2, §4.5).
   Embedded textures, done 2026-10-17: every texture binds by name to `texture.pak` as in the game (244 of the 270 embedded ones); the other 26 are decoded (VTex, a one-frame Bink 1 file, by FFmpeg as an external program) and stored in the `.glb`.
6. **M1, world.**
   Reading one sector of the starting region: floor cells with heights, static objects.
   Readers done 2026-10-17: `opensacred_formats::world` reads `sectors.keyx`, every sector of `sectors.wldx`, `tiles.pak`, `Static.PAK` and `Floor.PAK`, and matches the new prototype `tools/analysis/world/world_dump.py` on every sector.
   The cells start at `+0x20` of a sector stream, not at `+0x120` as first assumed, and the cells' chains reach every static and floor overlay (docs/02 §5).
   The same day `opensacred-convert world` writes all 6,050 sectors (cells, overlays, static objects) and the sprites of the statics: `mixed.pak` holds them as pieces of atlases, and `items.pak +0x10` names a type's sprite; a render of the starting sector from the ground tiles and the sprites shows the monastery, the pond, the garden and the field in place.
7. **M2, start.**
   A Bevy application with an orthographic camera, rendering one sector and the hero model at 3840×2160.
   Done 2026-10-17: `opensacred-game` (Bevy 0.20) shows the starting sector's ground (every cell's tile and floor overlays composed into one image, draped over the cells with their corner heights and light) under a 2:1 dimetric orthographic camera in the original's units, and `SERAPHIM.GRN` playing `SERA_WALK_1H`; `--screenshot` renders a frame at any resolution, 3840×2160 included.
   Bevy's features are chosen in the root `Cargo.toml` (no Wayland, `zstd_c`) and `deny.toml` admits ISC and MIT-0 for Bevy's dependencies.
   The same night the static objects followed (743 of the starting sector's 936 objects have a sprite; the monastery courtyard renders in place): each sprite is a quad in the plane of the screen at the depth of its cell's centre, and the ground lies behind every object along the line of sight, as the original draws the ground first.
   Then walking: the left mouse button sends the hero running over the ground (the height interpolated between the corners of the cell), turning toward the target, blending the idle and run clips, the camera following.
   Then sector streaming: the 3 × 3 sectors around the hero's are built in the background and spawned when ready (the eight neighbours of the start in about 1.5 s), those beyond two sectors go.
   `--benchmark 8` at 3840 × 2160 while the hero runs across three sectors: 650–775 frames a second on a development machine (a few frames a run took 30–150 ms at that size, a stall of every thread of the process: Bevy's compute pool used all the cores the other pools left, and the threads ready to run outnumbered the cores; four compute threads remove it, #2, docs/05 §3.6), after the ground moved from one image per sector (75 MB) to an atlas of the distinct stacks of layers (13 MB for the median sector).
   Then the miniatures (squares of their type's atlas; the animated ones show their first frame) and the original's draw lists approximated in depth: the types flagged as underlays lie under the ordinary sprites.
   The liquids show the frames of their sector's two liquid materials, cycling in 2,048 ms and mapped a frame over 4 × 4 cells as the client does (docs/re/liquids.md), the clear waters translucent and every surface fading at the shore until its blending is recovered (docs/decisions/2026-10-17-liquids.md).
   Footsteps through Kira (`bevy_kira_audio`): each step of the run clip, at the moments that the clip's motion parameters in `MOTIONS.PAK` give (a hypothesis the run and walk clips support: two fractions half a cycle apart), plays the next sample of `footsteps_hero_female` in the group that the cell's nature calls for, by the client's own table (docs/re/sound.md).
   Fixed the same night: a hero sent off before its model's player existed ran with the idle clip.
   The run speed comes from the hero's record in `Creature.pak` (180 units a second, a hypothesis), and the models and speeds are scaled from the units of the original's model projection (534 × 400 on 1024 × 768) to the ground's by 1.52, which sizes the hero against the sprites as in the original.
   Ahead of M4, the start script's 319 `CreateNPC` records place their creatures' models (soldiers, citizens, horses) at their `DefPos` or cell, each sector's when it comes in, playing an idle clip (docs/02 §7.3).
   Later that night: soft shadows under the characters; walking at the creature's walk speed (`R` toggles it); the client's names of the sound ids and of the profiles' groups, which make the music profile's group 0 the peace music, and its choice of the ambient loop by the ground, both playing (docs/re/sound.md); the cells' natures named by the client (docs/02 §5.2); a minimal HUD (the portrait, a health bar, the gait) and the original's cursor; the start script's 1,990 objects (barrels, chests, book stands) placed like its NPCs, and the NPCs facing their headings the client's way; the draw lists 0, 2 and 4 by the type's flags and the object's mask (world format version 3); and the screenshot test of a synthetic scene, rendered without a window, which CI runs on lavapipe (docs/05 §7).
   Fixed: the hero's clips went to the NPCs' animation players too, so the hero stopped switching clips once their models loaded.
   On 2026-10-18 the NPCs took up their weapons and shields in the hands the client gives them (#4, docs/re/equipment.md).
   Done 2026-10-18 and 2026-10-19 in some hundred pull requests (#19–#164): the frame time spikes (#2), the roofs and walls in front of the hero (#3), day and night with the night's lights (#5), the regions' music (#6), the buildings' insides and upper floors, the trees fading about the hero, and the NPCs walking where the scripts send them.
8. Usage tracking: done 2026-10-16, per pull request since 2026-10-17 (#16).
   Every pull request's description carries its Claude Code usage (agent-hours, tokens and their hypothetical cost at API prices, §9), which `tools/usage/pr_usage.py` computes from the transcripts' messages on its branch; the README's badge sums them with the sessions recorded before in `docs/usage.md`.
   After 30 days, the §9 estimates are recomputed from the actual agent-hours.
9. Issues and pull requests: done 2026-10-17 (#1).
   Every change reaches `main` through a squash-merged pull request with green checks (Linux, Windows, cargo deny); Claude Code merges, except the pull requests that wait for the maintainer (`needs-maintainer`: the guardrails, the rules, the process, the licenses, the deviations from the original that can change the play seriously or carry legal consequences, the screenshot references, and contributors' work).
   The dependencies are updated at most once a day, in an auto-merged pull request.
   The process is in docs/03 §3.6.
10. **M2, tools to test and drive the game.**
    Planned 2026-10-19 (#165), when every option of the game was a command line flag read before Bevy started, so the hero was fixed for the process and each change was a restart.
    The plan, in [docs/10](docs/10-game-tools.md): settings in `%APPDATA%\OpenSacred\settings.toml` with borderless full screen by default and F11 for a window (#167); the HUD's portrait, health bar and gait removed (#168); the log on F4 and in a file (#169); a debug overlay on F3 (#170); the game's states, so that a game starts and ends inside the process (#171); a main menu with the choice of the hero (#172) and a pause menu on Esc (#178); one set of commands for the command line's scripts (#173), the console on `~` (#174) and remote control over Bevy's Remote Protocol (#175); and a project skill or an MCP server through which Claude Code drives the game (#176).
    ≈10–20 agent-hours (estimate).
    Done the same night in ten pull requests of the game (#183–#193) and the skill's (#194): the game opens on its menu, borderless, and a game starts and ends inside the process; the console and `--exec` scripts run the game's commands, and Claude Code drives a game started with `--remote` through `tools/game/remote.py` (docs/03 §3.7, the skill `game`).
    Pasting into the console followed once the maintainer allowed BSL-1.0, the licence of the clipboard's Windows backend (#189).
11. **M2 into M3: the walk and the start's world.**
    The night of 2026-10-20, with the queue of M2 run out but for what waits on later milestones, the research and the features went on in some twenty pull requests (#205–#231), the code's reading shared with subagents of the session:
    the open buildings, the castles' walls and towers, whose upper floors show from outside (#149, #203);
    a click that sends the hero to the cell under the cursor and onto its level, as the original picks it (#204, #208);
    the hero's id, 1, by which the creatures in its way stop it only when hostile, while the NPCs stop at it and at one another (#157, #212);
    the chests' cells, closed by the scripts' trigger patches rather than by their objects (#211, #216);
    the world peopled as the original peoples it, by the sectors' `Init` and `Enter` script functions and their `FillSector` records (#215, #221, #222: the start's novices and animals, rabbits about it, goblins and thieves placed, bandits further out), standing idle, hostile ones in the hero's way;
    and the fight stance, which the NPCs take by the distance of the hero's nearest hostile and the hero by its own (#155, #219, #230).
    Their departure is docs/decisions/2026-10-20-spawning.md.
    The rules read off the code and not yet seen in the original are gathered for the maintainer in #227.
    Then (item 12): the creatures' items and packs (#228, #229) and their life while no foe is near (#232), before their fight.
12. **M3 into M4: the creatures' life.**
    The same night went on in some twenty more pull requests (#235–#271), the research shared with subagents of the session:
    the fills' creatures in their packs and with their `MakeGroup` items (#228, #229);
    the creatures' life while no foe is near, read off the code (#232, docs/re/ai.md): they wake by their timers and the hero's look and step about (#237), the animals flee and stroll (#238), the guards keep their posts (#239), and the crowded ones part and step aside for the hero (#240), a walk that cannot start standing a second (#258);
    and the townsfolk's day, read (#241) and followed in the sub-issues of #255 (#247–#254): they roll at the hero's looks within the zoom's range, walk home, to the animals, the traders and their partners, and meet them; the children play and gather about the hero; the jugglers play their instruments; they run from danger and from the Daemon (#269); and at dusk and by night the town goes into its houses.
    Their departures are docs/decisions/2026-10-20-animals.md, -idle.md, -guards.md, -crowds.md and -townsfolk.md; the checks for the maintainer are added to #227.
    Then the doors that open and close (#268) and the lines over the creatures (#270), done in #274 and #275, and the fight's rules (item 13), which the creatures' attacks, the guards' torches (#61) and the danger of a fight wait for.
13. **M3: the fight.**
    The rest of the night of 2026-10-20 read the fight off the code (#276, #281, #294, #300, #316; docs/re/fight.md, docs/re/items.md) and followed it in some twenty pull requests (#286–#319), the sub-issues of #285, the research shared with subagents of the session:
    the balance's rows by the difficulty (#277) and the creatures' health by their levels (#278);
    the hostile creatures coming at the hero and keeping the band of their reach (#279), striking with their attack clips at their attack speed (#280, #306), their hits resolved by the chance of attack against defence, the damage against the resistances and the damping, the number over the target, and death (#282);
    an NPC's attack, defence, damage and resistances by its attributes, its level, its bonuses and its skills (#281, #293), and an armed one's by its weapon, made by its template (#294, #309);
    the hero attacking at a click, its strikes chained faster (#283, #312), taking the creatures' hits by bronze's factors, regenerating, falling and rising at the start (#292), and gaining levels by its kills' experience (#300);
    the wounded creatures fleeing and the far ones going home (#304); the bosses and the champions (#316, #318); each hero at its own speed (#298), and the creatures' lines over them in a tall window (#296).
    Their departures are docs/decisions/2026-10-20-health.md, -pursuit.md, -hits.md, -hero-attack.md, -hero-health.md, -skills.md, -levels.md, -weapons.md and -bosses.md; the checks for the maintainer are added to #227.
    Next: the doors in the way, the places about the hero and the archers (#284, the last open sub-issue of #285, split in #391 and #392 after #383 read them); the hero's items and its character sheet come with the interface.
14. **The documents against the code.**
    On 2026-10-20 the documents were audited against the code, the issues and the pull requests (#321–#325), and the problems found were filed as issues of their own (#326–#333).
    Among them: the rules live in `opensacred-game`, with no `opensacred-rules` or `opensacred-oracle` yet (#328); a creature killed or moved comes back whole with its sector (#329); and the game's zoom number was the inverse of the original's (#330, since fixed).
15. **The night of 2026-10-20 into 21: the data's repairs, the kept creatures and the rules' crate.**
    The night opened with the maintainer's answers to #328, #273 and #227, and went on in some twenty pull requests (#365–#390), the work shared with subagents of the session:
    the converter repairing the original's data as it writes `assets/`, by the rule of #348: the MP3 samples' and the music's bytes before their first frame (#354, #367), the forearms' two spellings (#355), the instruments' socket (#353), the lavas' streaks (#347) and the Underworld menu's faint tile (#352), each recorded as a decision;
    the creatures that go with their sectors kept as the original keeps them, a killed monster never coming back and a fill's living ones put back where they stood (#329); the hero no longer going round at a stair it was sent to (#373); a fleeing creature fleeing to its point (#387); the hero opening and closing a door by a click, its trigger's cells with it (#372); and a command's Windows paths keeping their backslashes (#368).
    The maintainer chose to move the rules out of the game (#328): `opensacred-rules` now holds the spawning (#359), the fight's formulas and the hero's levels (#360), the ways and the cells in the way (#361) and the creatures' decisions (#362), the game keeping its systems, components and resources (#364); the oracle is #363.
    The research read the table defects that the community reports (#357, none to repair in the data), the archers' arrows, the places about the hero and the doors in the way (#383), and measured the doors' leaves and headings from the models (#273, in part); the checks in the original are a page of 68 with the shortest way to each (#227), and the problems found on the way became issues of their own (#367, #368, #371–#374, #387).
    Next: the archers (#391), the places about the hero and the doors in the way (#392), the oracle (#363).
16. **The morning of 2026-10-21: the archers, the oracle's vectors and the drops.**
    The night went on in some thirty more pull requests (#395–#436), the work shared with subagents of the session:
    the archers keeping their band and shooting (#391), putting their bows away for melee and taking them again (#396), the places about the hero and the doors in the way (#392), and a creature walking to a far target a leg at a time (#425);
    the converter repairing the models' defects that the community reports and the research confirmed in the data (#358): the dwarf's earth amulet (#398), a cannon's strap (#399) and the large dragons' tail roots (#400), each recorded as a decision;
    the oracle (#363), the original's fight's functions in Unicorn on the developer's own `Sacred.exe`, whose vectors found five departures of the rules, all fixed: the most health grows as `S^g` (#408), the weight of nearly nothing (#406), the exponent 99 levels above (#407), the level's factor above its last point (#418) and a most below 1 (#417); the crate waits for the maintainer (`deny.toml`, #422);
    the drops read (#404) and followed: the tail of `Balance.bin` (#413), the rules of what a killed creature drops (#414) and the drops falling and lying where it dies (#415), without the runes, the uniques, the sets and the pick-up yet (#412);
    and the doors' outer and inner points measured from the models and placements, with no table of the client's (#273).
    The defects whose repair changes the look, the voices or the texts wait for the maintainer (#401–#403), the scripts' objects at their exact positions waits for a look at a door in the original (#429, a draft), and the checks in the original are 83 with the night's 15 (#227).
    The maintainer chose a driver of the running original for Claude Code (#448): Frida, not a Rust crate, and the original run in place in `OriginalGame/`, where it may write its own files; the plan is in the documents (#453) and in four steps (#449–#452), after which Claude Code works the checks of #227 itself.
    Next: the townsfolk and the monsters through the doors by their points (#431), which way a door's leaf turns (#430), the driver of the original (#449–#452), and the rest of M3: the character, its skills and combat arts.
17. **The night of 2026-10-21 into 22: the original's driver, the checks in the original and the heroes' items.**
    The driver of the running original came in its four steps (#449–#452): `Sacred.exe` under Frida in the background, its input and menus, the world's state and commands, and the traces of its functions, which Claude Code then used to work the checks of #227 itself, with second Frida sessions reading the creatures' words and stats.
    The checks it ran confirmed the sectors' creatures, the guard and the captain in the way, a chest's cells, the packs, the waking steps, the guards, the fleeing, the places about the hero, the range by the zoom, the children and the coming at the hero by a clear line, and led to some twenty pull requests (#460–#493):
    a creature's strike chaining or waiting for the AI's two steps (#466), the rabbits' variants without a behaviour (#468), the damage numbers' colours and fading (#470), the doors' swing and the models' handedness confirmed (#430, #445), the hero's regeneration in its fight stance (#474), the townsfolk and the monsters by a door's points (#431), a drop rising and falling in place (#478), the trigger's state traced (#480), and an `Enter` function's trader or speaker keeping no flags (#482);
    and the heroes' items: the templates' inventory read (#486), each hero holding its template's weapon and moving with its class's clips (#488), the items' bonus slots and the cosmetic items that count for nothing (#490), and a bow's multiplier and the skill 10's health (#492), so that every class's hero of level 1 has the original's attack, defence, damage, resistances, attack speed and most health.
    A race in Bevy's spawner that an item attached at once to a scene spawned anew could hit is held off by a wait of four frames (#488); the console's tests no longer read an earlier run's settings (#484).
    The way past a wall's corner (#476) and the floor-2 doors (#459) wait for the maintainer's choice; the checks in the original left are in #227.
    Next: the skills and the attribute points spent, the combat arts, an NPC's most health by its skill 10, and the rest of the checks.
18. **The day of 2026-10-22: the combat arts read, and the game's own commands of the driver.**
    The combat arts' two tables, their fields, the runes that name them, their regeneration and their strikes were read (#500; docs/re/arts.md, #501, #502, #512), and an NPC's most health took its skill 10 (#498).
    The maintainer chose the way round a wall's corner into a house (#476, #503) and the script's models standing in their own cell's place (#459, #513), and the goblins kept the original's voices (#402, #514); the blacksmith's apron was made opaque (#401, #517), and the models' texels tested and blended by their alpha as the original's (#516, #519).
    The game took the driver's commands for itself, so that one script drives both: `cell`, `cell-info` and `nearest` (#522), `creatures` and `stats` (#523), `spawn` and `kill` (#524), `item` (#525), and `level`, `experience`, `heal`, `god` and `fly` (#526); its `--remote` keeps off the driver's port (#528).
    The original ran on a locked desktop with dgVoodoo2 beside it, its watermark off (#546, #548), and a failed CI run is rerun by hand rather than worked around (#544).
19. **The night of 2026-10-22 into 23, and the day: the conformance runs, the hero's arts, and the maintainer's choices.**
    The driver took `god`, `heal`, `level`, `experience`, `spawn`, `kill`, `drop`, `item`, `pause` and the arts (#558–#573), and `tools/game/compare.py` and `tools/game/conformance.py` compared the two games' creatures, values, drops, regeneration, strikes, arts and runs (#574–#648).
    What they found was fixed in some forty pull requests (#563–#682): the levels of the scripts' creatures and of the fills by their sectors (#540, #541, #562, #570, #582, #621), the bosses' health by their rank (#583), the fist fight (#592), `A` and `D` at the x87's 53 bits (#593), the sides that make a creature hostile (#580), the derived item types among the drops (#596), the walk and the run by the pace (#599), the regeneration, the creatures' decisions and the hero's orders on the world's tick of 0.1 s (#616, #639, #640), and the strikes' level factor and rolled damage (#615, #620).
    The hero makes its combat arts (#505–#509): the arts' tables from `Sacred.exe` (#581), a creature's arts, levels and runes (#604), the regeneration and the skills that shorten it (#613), an art's strikes (#619) and the hero's clips and strikes (#638); then the kicks and their knockback (#650, #676), the other classes' `Rundum`, `Hardhit` and `Attacke` (#649, #679), and a right click on an art not ready walking to the creature (#654).
    The creatures of hostile sides fight one another (#630), a struck creature raises its group's alarm (#669), a creature's kill earns it experience (#690), and the quests' `OnSetUp` functions place their givers (#624).
    The fight took its sound: the hero's strikes keep their pace and one click strikes once (#704, #705), the creatures and the heroes speak and their weapons sound (#713, #729–#735), the hero's death plays its music (#734), and a sound takes its volume and pan by its place (#731).
    The maintainer chose (#716, #718, #721, #694): multiplayer after version 1.0, as the milestone M9, the browser version dropped, the scripts translated into Luau (ADR-13, #720), and the converter in two stages, the extractions into lossless masters and `build` into the game's assets (#736–#739), which the game reads in layers from ZIP archives by kind (#699, #740, #741); the texts' defects are repaired with words of the project's own (#403, #683, #684).
20. **2026-10-24: the special weapon arts, the Vampiress, the assets' formats, and a night's queue.**
    `build` encodes the textures into KTX2 with BC7 and mipmaps (#700, #744) and the sound into Ogg Opus that the game decodes by libopus (#701, #745), and the mip levels' alpha is raised only, so that the models stay solid from afar (#816).
    The creatures' effects came into play (#651, #763), with the effect 53 that turns a part of one kind's damage into another's (#761, #783), and the special weapon arts were read (#653, #753, #762) and made, each class's in a dozen pull requests (#754–#760, #766–#813): the Dark Elf's boons, viper, arrowproof and panther, the Dwarf's war cry and frenzy, the Gladiator's hand of god, heroism, roar, jump and backbreaker, the Seraphim's hurled blades, whirling blow and jump, the Daemon's 1094 and forms, the Wood Elf's multi-shot, dumdum, club arrow and blades, and the Vampiress's arts.
    The hero shoots with a bow or a crossbow (#784, #785), the Wood Elf wears her own model (#789), and the Vampiress is a hero (#790) with her night form, its speeds and its record (#792–#795), her dash, claw jump, blood bite, domination, death's kiss and bloodlust.
    Every text file is UTF-8 without a BOM and with LF on every platform (#834, #835), `sound.ron` and the cutscenes' files lost what repeated their ids and names (#827, #828), and the items' sockets take the scales above them (#814).
    The session of that evening took the queue in its order (#831, #826, #830, #838, #754): a `new-game` keeps the command line's `--zoom` and `--no-music` (#840), zstd 0.14 (#841), the tables parsed once a run (#842), the statics' sprites assembled by the converter, so that the start's sectors read 14.4 MiB for their sprites instead of 23.5 and `textures.zip` drops the 10,585 atlases (#843), and the Seraphim's 1018, its effect 16 putting the item 3809 in her hands (#845).
    Found on the way: the items whose level lowers the attack speed and the pace (#844), and the Seraphim's shots of 1018 and 1019, their beams the objects 875 and 801 (#754).
    Next: the special weapon arts' objects and effects (#761, #672), the rest of the classes' arts (#754–#760) and of the checks (#227), then M4's scripts in Luau (#720).
21. **The night of 2026-10-24 into 25: the spells, the followers, and the rest of the special weapon arts.**
    The NetScript functions' control flow was read and structured (#848, #858), and the converter writes the script trees as Luau sources and compiles them into bytecode that runs against the walker (#863, #870), `luau0-src` 0.22.1 held back for its bytecode's version (#869).
    The special weapon arts went on (#849–#868): the Seraphim's 1019 and her bolts, which land as the original's (#850, #855), the Gladiator's 1016 (#852), the Daemon's charge 1091 and its blast with the art's attack (#853, #857), his ring of poison and emperor (#865) and his breath of fire (#868), the Dwarf's war frenzy (#861), guns (#867) and momentum (#866), and the Dark Elf's soulcatcher (#862); the items above their holder's level lower the speeds (#859).
    The spells were read (#876, #891) and made in some twenty pull requests (#877–#899): the cast at its share of the clip and the Mage's fireball; the Mage's skins, shields, healing, quickening, attacking spells, teleport, water form, stone and ring of frost; the Seraphim's striking, strengthening, holding and converting spells; and the Dwarf's, the Daemon's and the Wood Elf's spells.
    The followers were read (#896) and made (#900): they keep the band of 150 to 240 about the hero, step off, are put near it from afar, and fight for it, and the Wood Elf's unicorn and the Vampiress's wolf are summoned so.
    Then the Vampiress's awakening, whose raised dead raise their kills in turn (#903, #904), her master's bite (#905) and her bats (#906), with the Wood Elf's spider arrow (#902), so that every art of #756 and #758 is made.
    The Mage's whirlwind knocks back and his banishing circle sends the weak away (#901), the ring of frost slows by an NPC's time's factor, the effect 15's value (#907), and the Wood Elf's changeling turns the creatures about her into animals for a time (#908), so that every spell of #887 and #888 is made.
    The objects of #761 were read by subagents of the session and made: the Dark Elf's traps, a ball thrown in an arc that makes an explosion, a pentagram, a confusion, a poison or a fog where it lands (#913), with his spellproof (#910), so that every art of #757 is made; and the death's kiss's skull, which homes and curses each creature it touches, and its bleeding (#914).
    The checks in the original saw the hero's death and rising (#882), the master's bite's effect 14 go to its target rather than to her, the bats' clip and level, a rabbit's flight (#227, check 8), and the packs of 48,38 walking on all the time (check 7), which their reading found to be the idle action's steps moving a whole group, as the game now moves it (#912, #915); a creature's way ends in its target's cell whoever stands there (#883, #884).
    By the morning the last of #761's effects were read: 35 a marker, 16 and 17 the Seraphim's bolts' level, 50 the Daemon's form, 60 its object, and 26 a spell turned aside, which under 1046 shares its damage among the hostiles about her rather than going back to its caster (#919), so that #761 is done.
    The Daemon in his form now wades over the cells an arrow flies over where a corner lies low, loses it on a building's or a settlement's cell, sees its last seconds counted over him, and as it ends off the ground is put on the dry ground about him or drowns, his steps sounding as wings (#920, of #918); the followers keep their master's pace (#921), and of their step off the original's floor and its two rolls for the two axes were read (#922, #926).
    M4 began: the rules hold the scripts' state, the quests' flags and transitions, the variables and the dialogues' table (#916, of #724), and `opensacred-scripts` runs a tree's Luau sandboxed, a file at a time from its bytecode, each call bounded and its records handed to a host (#917, of #725).
    The texts' repairs went on in the project's own words: the four quest-log keys that resolve to nothing have lines, the fifth reference being a script's typo (#923, of #687), and the eleven gameboxes are named by their art (#925, of #686), so that of #403 only the items' names remain (#685).
    Found on the way: the hero cannot pick up the items on the ground (#911), for which the Gladiator's thrown weapon waits (#755); how the original picks them up was read, from the click and its priorities to the inventory's grid and the automatic pick-up (#924).
    The tests on the game files passed on `main` in the morning, 742 of 742.
    Waiting for the maintainer: the chest whose cell patch is cut short (#871).
    Next: the game running the scripts in Luau (#726, of #720), the hero picking up the items (#911) and with it the Gladiator's 1013 (#755), the rest of the followers (#895, #657), and the checks in the original still open (#227).
22. **2026-10-25: the M3 bugs, and the scripts in Luau.**
    The day's dependency update (#985), then six M3 bugs: the waters' crabs and fish drawn under their surface (#983), the console's spawn on the driver's cells and by the class's side (#984), an archer that gives up a hero gone beyond 800 or out of its line (#972), no stepping aside for a creature of the AI's state 0 (#977), a course found once at its order (#981, #989), and a hero landed in a wall by a bad stair stepping out (#978).
    The translated scripts keep their markers' order (#993), and the game runs its scripts in Luau (#726, of #720): the hero's tree's `StartCode` and every sector's `Init` and `Enter` at a new game's load, through `opensacred-scripts`, the records handed back to `npcs`' readers.
23. **The night of 2026-10-25 into 26: the scripts through the play, M3 closed, and the dialogues.**
    The maintainer chose Luau before the rest of M3, and the scripts now live on through the play (`scripts::Live`): the regions' `Init`s (#585), the base triggers and the hero's teleports into the caves (#1002, closing #954), the timers (#933), `SetDrop` (#930), the barred triggers (#1006), `SetObjState` (#952) and `SetHP` (#1001), with the console's `script` and `quest`.
    M3 was closed on the way: the hero picks up the items on the ground (#911), a death's gold (#976), the Gladiator's hurled weapon (#755, closing #511), the summoned followers' bit 0 and the step off's floor (#1013), and the Vampiress's start floor (#1017).
    The play's records reach the world: `CreateNPC` and `CreateObj` (#1014), the hero's home (#974), the deaths' functions and `GroupIsDead` (#656), `SetNPCState`'s heading, group and stock (#658), a sight function (#1025), `SetGroupState`, `DelNPC`, `SetOnKill` and `SetOnCollect` (#1021), `NPC_Goto` and a creature's `Teleport` (#1035), and a place at a creature (`CPOS:`, #1046).
    The dialogues, read in the original and its code (#1024): a click on a creature of a dialogue sends the hero to it, and its window shows its name, its portrait, its texts and its buttons, which run their functions (#1028, #1032); `Popup`, `InfoPlayer` (#1033) and `NPC_TalkTo` (#1037) begin talks and show texts; the icons over the creatures pulse as the original's, whose tints never show (#1039).
    The scripts' hired followers were read and made: an escort follows the hero, never fights, runs from a fight and is let go (#657).
    Read and found dead in the release: the dynamic quests, whose templates are empty and whose functions the runner refuses, so that their table, their hook, `Dialog` and the `ToDo`s never run (#1020, #1037, #1051).
    Bugs found and fixed: Luau files whose names differ in case alone overwrote each other on Windows (#1043), the runner ran the functions that the original refuses (#1051), the scripts' variables told names that differ in case apart (#1054), the Fluent literals of the texts (#1030), and a `0x60` taken for a death function (#1025).
    The sieges' and the world change's music follow the scripts' variables (#55, its music).
    The maintainer's new rule: an issue taken into work and a pull request are assigned to oneself (#1049), and all the earlier ones were assigned.
    The tests on the game files passed on `main` in the morning, 828 of 828.
    Next: the rest of #55 (the world's changes, `RegionChange` and `ReplaceSpawn`), the trades and the blacksmith of M4, and the texts' repairs that wait for the maintainer (#403, #685).

### 10.1 Priority queue

Since 2026-10-17 the queue is the repository's open issues (github.com/zarcsis/OpenSacred/issues), and the milestones M2–M9 of §9.2 are GitHub milestones.
Work takes this order: the known problems and bugs (`bug`), then the dependency upgrades (`deps`), then new features (`feature`, `research`), the earliest milestone first.
The daily dependency check opens a `deps` issue for every newer incompatible release of a direct dependency or of Rust (`CLAUDE.md`, "Work order"); on 2026-10-20 there were none: all 17 direct dependencies were at their latest releases and Rust 1.99.1 was the latest stable.

## 11. Top risks

| Risk | Probability | Impact | What we do |
|---|---|---|---|
| Granny animations convert with errors (poses, weights) | medium | high | three open readers of the format as a reference; control poses; an early start in M1 |
| Combat and loot rules are recovered incorrectly | medium | medium | the Unicorn oracle and vectors; observations of the running original (#227, #448); fidelity is not a priority, but deviations are recorded |
| NetScript semantics are harder than expected | medium | high | a disassembler and listings early; opcodes are covered by frequency: 45 types account for 95% of records; the translator refuses a control flow or a type it does not know (#723), and the running original is the reference (#448) |
| Bevy breaks its API more often than is worthwhile | high | medium | scheduled updates; rules outside the engine; the fallback is Godot + godot-rust |
| Token use exceeds the estimate (usage limits slow the pace) | medium | medium | tracking from day one; Sonnet for mechanical work; small tasks |
| Maintainer burnout | medium | high | the vertical slice gives a visible result early; milestones are short |
| Claude Code misunderstands the original's behavior | medium | medium | vector tests, screenshot tests, the decision log; Claude Code's own observations of the running original (#448); maintainer review of key formulas |
| Game content gets into the repository (a test fixture, an issue attachment, converter output) | low | high | no game bytes in the repository or CI; tests use synthetic data or the user's local installation; the same rules for issues and pull requests (§12) |
| The single-player game makes multiplayer expensive after 1.0 | medium | medium | ADR-09: the rules apart from the engine, the world's state in the rules and the ECS, the scripts run on the host |

## 12. Legal and asset policy

Details: [docs/08-risks-legal.md](docs/08-risks-legal.md).
This is not legal advice.

* Every user needs their own lawful copy of the GOG release.
  Interoperability work (studying the formats and the behavior, writing our own code) relies on Articles 5(3) and 6 of Directive 2009/24/EC, which a contract cannot override (Article 8).
  The GOG release has no copy protection, so nothing is circumvented.
* The repository and CI hold no game bytes: no game files, no `Sacred.exe`, no converted assets, HD packs, memory dumps, decompiler output or game texts.
  Tests that need the game run locally on the user's installation in `OriginalGame/` (git-ignored); CI runs only the tests that need no game files.
* Converted output is generated locally from the user's own copy and is never published.
  The same rules apply to issues and pull requests.
* OpenSacred is not affiliated with the rights holder; Sacred is a trademark of THQ Nordic GmbH.
  The name and its use: docs/08 §6, "Name and trademark".

## 13. Open questions

1. The actual cost of an agent-hour on the project's tasks (in tokens and at API prices): to be refined after 30 days.
2. Whether Android is needed: a decision after version 1.0; the browser version was dropped on 2026-10-23.
3. How multiplayer is made after version 1.0 (M9): our own protocol between OpenSacred games or the original's (TinCat 2, docs/09 §3.2), so that the original clients could join; a host's game or a dedicated server; which network crate; its estimate.

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
| [docs/07-roadmap-estimates.md](docs/07-roadmap-estimates.md) | milestones, agent-hours, tokens and their hypothetical cost at API prices |
| [docs/08-risks-legal.md](docs/08-risks-legal.md) | risks and the legal framework |
| [docs/09-ecosystem.md](docs/09-ecosystem.md) | game versions, community work, comparable projects |
| [docs/10-game-tools.md](docs/10-game-tools.md) | tools to test and drive the game: settings, menu, overlays, log, console, remote control |
| [docs/re/](docs/re/) | the original's behavior described in prose, per subsystem |
| [docs/decisions/](docs/decisions/) | deliberate deviations from the original's behavior |
| [docs/usage.md](docs/usage.md) | how Claude Code usage is recorded per pull request; the sessions before |
| [tools/README.md](tools/README.md) | research analysis scripts |

## Sources

* Versions as of 2026-10-16: crates.io (bevy 0.20.0, wgpu 30.0.1, fyrox 1.0.1, godot 0.5.5, kira 0.12.5, symphonia 0.6.1, rav1d 1.1.0, egui 0.37.0, cosmic-text 0.19.0); the Rust stable channel (1.99.1); Bevy 0.20 news (bevy.org/news/bevy-0-20); Godot 4.7.2 (godotengine.org); Unreal Engine 5.8 (unrealengine.com); Vulkan SDK 1.4.357.0 (lunarg.com); Visual Studio 2026 with MSVC 14.50 (devblogs.microsoft.com/cppblog); Ghidra 12.1.4 and GhidraMCP 2.0.
* Claude prices: the Claude API reference (cached as of 2026-10-06).
