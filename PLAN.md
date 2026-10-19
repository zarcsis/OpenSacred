# Plan: OpenSacred

The main plan document.
The project's goal is to **play Sacred Gold 2.28 (GOG) comfortably on modern hardware**: Windows 11 x64 at 4K, later Linux, possibly Android and the Chrome browser (WebAssembly).
The tools work on the user's own GOG installation; the repository holds no game files (§12).
This document is self-contained; details are in `docs/01…09`, analysis scripts in `tools/` ([description](tools/README.md)).

Claim status is marked with the words **confirmed** (verified on GOG release files with a script or a disassembler), **community-reported** (not verified on a file), **estimate** (model calculation) and **hypothesis**.

## Summary

* **Approach: "convert the assets + rewrite the game", not "decompile and rebuild".**
  A toolset reads the assets of the user's GOG installation once and converts them into open formats (PNG/KTX2, glTF, FLAC/Ogg, AV1, RON/Fluent).
  The new game reads only these formats.
  Where the game's rules are not evident from the data, `Sacred.exe` is studied only to understand them; the behavior is described in prose and implemented independently in Rust with a redesign; an exact copy of the behavior is not required.
* **Language: Rust for the entire product**: the game, the converter, the tools.
  VC6 and old toolchains are not needed: byte-for-byte matching is not a goal.
* **Engine: Bevy 0.20 on wgpu** (Vulkan on Windows, Linux and Android; WebGPU in Chrome).
  The game rules live in a separate crate with no engine dependency, so if needed we can switch to Godot with godot-rust without losing the logic.
  Unreal Engine and Godot with C++ were considered and rejected (§5).
* **Nothing networked:** no server, no multiplayer, no lobby.
  Compatibility with the original saves is not needed.
* **Only free tools and dependencies:** Ghidra 12.1 with an MCP server for Claude Code, Rust, Visual Studio Build Tools 2026, Vulkan SDK, Unicorn, FFmpeg, Blender, RenderDoc.
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
| No server or network components are planned | Network code, TinCat, lobby, `GameServer.exe` as a target: out of scope |
| Work only with the GOG version | The reference is GOG ENG 2.28; the tools read the user's own installation |
| The game does not have to read the original formats and saves | An offline converter into open formats; the runtime reads only those; HD and upscale packs in the future |
| Solo development almost entirely through Claude Code | Estimates in agent-hours and tokens; the process is built around tests and text artifacts |
| Modern toolchains: Visual Studio Build Tools 2026, latest Vulkan, newest dependencies | Latest stable Rust, scheduled dependency updates |
| One language for the whole project, preferably Rust | Rust everywhere |
| Only free tools and dependencies | Ghidra instead of IDA/Binary Ninja; all software and libraries are open source or free of charge |
| Platforms: Windows 11 x64; then Linux; possibly Android and WebAssembly in Chrome | wgpu covers all four targets with one codebase |
| Consider an existing engine (UE, Godot) | Comparison in §5 and `docs/04-language-and-engine.md` |

### 1.2 Goals and measurable criteria

| # | Goal | Criterion |
|---|---|---|
| G1 | Game assets are available in open formats | The converter processes 100% of the files of the reference GOG installation per the manifest; models open in Blender and pass glTF-Validator; a rerun gives the same result. 2026-10-17: `opensacred-convert coverage` gives 305 files converted and 37 skipped with a reason, none left; tables and hero files keep the fields not yet understood as raw spans |
| G2 | Vertical slice | The hero walks through the starting region with animations, sound and UI at 3840×2160 at ≥ 60 fps on a mid-range PC |
| G3 | Game rules | Combat, attributes, skills, items and loot work by the recovered formulas; deviations from the original are deliberate and recorded |
| G4 | Campaign content | Quests, dialogues and triggers of the Sacred and Underworld campaigns run from converted scripts; both campaigns are completable from start to finish |
| G5 | Playable version 1.0 for Windows 11 x64 | Full UI, saves, settings, gamepad; 4K and HiDPI; a playthrough without critical bugs |
| G6 | HD assets | The upscale pipeline produces HD packs that the game picks up on top of the base assets |
| G7 | Other platforms | Linux: the same functionality; Android and Chrome: by a separate decision after G5 |

**Out of scope:** multiplayer, server, lobby; loading the original saves; byte-for-byte compatibility; the Steam version; macOS and iOS; publishing the game assets in any form.

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
| ADR-04 Engine | Bevy 0.20 (wgpu 30) for presentation; the rules crate `opensacred-rules` does not depend on the engine | Pure Rust, everything in code and text, which suits Claude Code; Vulkan, WebGPU and Android from one codebase; the fallback is Godot 4.7 with godot-rust on top of the same rules crate |
| ADR-05 Assets | Offline converter `opensacred-convert` (Rust CLI): textures → PNG + KTX2, models and animations → glTF 2.0, world → binary sectors + RON, tables → RON, texts → Fluent, sound → FLAC/Ogg Vorbis (MP3 as is), video → AV1 via FFmpeg, scripts → intermediate representation (IR) | Requirement (§1.1); open formats can be checked with third-party tools (Blender, glTF-Validator) and suit HD packs |
| ADR-06 Graphics and 4K | Rendering at native resolution through wgpu (Vulkan by default on Windows, DX12 as fallback); an orthographic camera with the original's angle and adjustable zoom; the ground is real 3D terrain from a heightmap; scalable UI; HD packs on top of the base assets | The original is bound by the CPU and by 1024×768, not by the GPU; a modern renderer removes both limits |
| ADR-07 Sound, input, video | Kira via `bevy_kira_audio` with Symphonia decoders (pure Rust); Bevy input with key remapping and gamepad; video: AV1 with the rav1d decoder, the HTML video element in the browser | Everything is free and builds for Windows, Linux, Android and WebAssembly |
| ADR-08 Saves and settings | Our own versioned save format (serde, RON or postcard); settings in TOML in the user directory | The original saves are not needed; a modern format is easier to debug |
| ADR-09 Network | Not done: no server, no multiplayer; `GameServer.exe` is used only as an auxiliary binary when analyzing shared functions | Requirement (§1.1) |
| ADR-10 Tools | Free only: Ghidra 12.1 + pyghidra-mcp (GhidraMCP 2.0 as the alternative with the GUI), Unicorn (emulating the original's functions for tests), x64dbg, dgVoodoo2 (optional), RenderDoc, Vulkan SDK 1.4, Blender, FFmpeg, Visual Studio Build Tools 2026 | Requirement (§1.1); IDA Pro and Binary Ninja are excluded |
| ADR-11 Process | Claude Code as the main executor: `CLAUDE.md` with rules, MCP to Ghidra, tests as guardrails (converter golden data, oracle vectors, screenshot tests), small tasks, maintainer review | Solo development; quality rests on automated checks, not on manual review of every line |
| ADR-12 Repository and legal | The repository is public and holds no game bytes: no game files, converted assets, HD packs or dumps, in the repository or in CI; users supply their own GOG copy and run the converter locally; converted output is never published (§12, docs/08) | The game belongs to the rights holder; every user needs their own copy |

## 4. Strategy

### 4.1 Pipeline

```text
OriginalGame/      ← the user's own GOG installation (git-ignored)
   │  opensacred-convert (Rust CLI, once, and again after improvements)
   ▼
assets/            ← open formats, generated locally, never committed to git
assets-hd/         ← HD and upscale packs, override assets/ at matching paths
   │
   ▼
opensacred-game (Bevy) ───── uses ───── opensacred-rules (rules, no engine)
                                               ▲
   Ghidra + MCP ──► the behavior described in prose (docs/re/) ──► Rust, written independently
                                               ▲
   opensacred-oracle (Unicorn, local): the user's own Sacred.exe → reference vectors for tests
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
5. **Rust, written independently, then redesigned.**
   `opensacred-rules` implements the behavior from the description and passes the vectors.
   After that it can be changed deliberately: every deviation from the original is recorded in the decision log (`docs/decisions/`).
6. **Nothing from the study is committed.**
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

Triggers for revisiting: Bevy migrations regularly take more than 10 agent-hours; Bevy's animation or UI systems block the vertical slice for more than two weeks; Android or the web needs capabilities that wgpu lacks.

## 6. Architecture

### 6.1 Crates

```text
OpenSacred/
├─ Cargo.toml                 Rust workspace (edition 2024)
├─ rust-toolchain.toml        stable, updated together with the dependencies
├─ CLAUDE.md                  project rules for Claude Code
├─ crates/
│  ├─ opensacred-formats/     reading the original formats (PAK, TEX, GRN, sectors, NetScript, global.res, AMH templates)
│  ├─ opensacred-convert/     CLI: OriginalGame → assets/ (subcommands verify, textures, models, world, sound, text, video, ui, netscript, tables, heroes, all, coverage)
│  ├─ opensacred-data/        open-format types shared by the converter and the game (serde)
│  ├─ opensacred-rules/       game rules without the engine: character, combat, items, AI, quests, script VM
│  ├─ opensacred-game/        Bevy application: rendering, camera, input, UI, sound, saves
│  └─ opensacred-oracle/      (development only, run locally) runs functions of the user's own Sacred.exe in Unicorn, records vectors
├─ tools/analysis/            Python research scripts (archive, gradually replaced by Rust)
├─ docs/                      plan, format specifications, prose notes on the original's behavior (docs/re/), decision log
├─ assets/  assets-hd/        generated locally, in .gitignore
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

Pure Rust without Bevy: the character and attribute model, combat formulas, skills and combat arts, items and loot generation, trading and the blacksmith, creature AI, spawning, time of day and weather, the quest script interpreter.
The layer is tested without graphics.
The Bevy layer calls it through a thin ECS adapter.

## 7. Asset converter

Details for each format: [docs/02-data-formats.md](docs/02-data-formats.md), the section on conversion.

| Source | Target | How it is checked |
|---|---|---|
| `texture.pak` etc. (ARGB4444 + zlib; A8R8G8B8 uncompressed) | PNG RGBA8 (master) + KTX2 with BC7 for PC and ASTC/UASTC for Android and the web | pixel comparison with a reference decoder; viewing |
| Ground tiles, MIX atlases | PNG atlases + a UV table of 18 diamonds | rendering a test sector |
| `models.pak` (Granny 1.2b) | glTF 2.0 (`.glb`): meshes, skeletons, weights, materials | glTF-Validator, Blender, an in-game screenshot test |
| Granny animations | glTF clips; the Bip01 bone names are kept so that clips fit models with the same skeleton | visual check, comparing poses in key frames |
| World: `sectors.wldx`, `sectors.keyx`, `Floor.PAK`, `Static.PAK`, `Triggers.PAK` | binary sectors (serde + postcard, zstd compression) + RON descriptions | loading all 6,050 sectors, checksums |
| `Items.pak`, `Creature.pak`, `Weapon.pak`, `bin/*.bin` | RON tables with a typed schema; done 2026-10-17 (`tables/`), the fields of unknown meaning as spans named by offset | reading back, comparing fields with the source |
| `global.res` | Fluent (`.ftl`) + a map of hash identifiers | all 23,123 strings present |
| NetScript (`bin/**`) | an intermediate representation + a readable listing; done 2026-10-17 as one postcard file per tree with a RON index (RON for 2.2 million records would take hundreds of MB), the listing on request | the disassembler gets through 20 directories with no unknown records: met 2026-10-17 |
| `sound.pak`, `sndProfiles.pak` | WAV (PCM and IMA ADPCM) → FLAC; MP3 entries unchanged; profiles in RON | listening to a sample, durations |
| `mp3/*.mp3` | unchanged (MP3 is patent-free and decoded by Symphonia) or Ogg Vorbis | playback |
| `movie/*.wmv` | AV1 + Opus in WebM via FFmpeg | playback in the game and in the browser |
| `font/*.ttf` | unchanged + a free Unicode font (Noto, OFL) for missing characters | rendering all strings |
| `Templates/*.ptx` | hero starter kits in RON; 2026-10-17: the sections, inflated, in postcard hero files with a RON index of level, class and name, until their layout is recovered | character creation |

Principles: the converter is deterministic and incremental (a hash-based cache), runs in parallel, and writes a manifest with the source of every output file.
Done 2026-10-17: each subcommand records `assets/manifest/<command>.ron` (the SHA-256 of the converter executable, of every installation file it read and of every file it wrote); `all` skips a command whose record still holds (3 s to check all ten), and two full runs into empty directories write identical files (83 s, then 160 s with the deterministic video encoding).
Outputs are never committed.
MIT-licensed community work (open-grn, grn2glb, sacred-sdk) is used as a reference with attribution; code without a license (SacredEngineRemake) is not used.

## 8. Graphics and 4K

Details: [docs/05-renderer-vulkan-4k.md](docs/05-renderer-vulkan-4k.md).

* Rendering at the screen's native resolution; 3840×2160 is the main target mode.
* Vulkan through wgpu by default on Windows and Linux; DX12 as the fallback on Windows; WebGPU in Chrome; Vulkan or GLES on Android.
* The original's bottleneck is thousands of draw calls on the CPU.
  Tile and sprite batching, instancing and Bevy's GPU skinning remove it.
* Visible area: by default as in the original at zoom 1.0, but with 4K pixel density; an extended view is an option.
* The UI scales independently of the scene resolution; fonts are rasterized for the actual DPI.
* HD pipeline (after version 1.0): texture upscaling with free models (Real-ESRGAN through ncnn-vulkan, or ONNX models through the `ort` crate), separate alpha channel processing, packing into KTX2 with mipmaps.
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

### 9.2 Milestones

| Milestone | Contents | Agent-hours | Maintainer hours |
|---|---|---|---|
| M0 Foundation | Cargo workspace, `CLAUDE.md`, CI, GOG installation manifest | 6–12 | 4–8 |
| M1 Converter | all formats from §7; tests and the manifest | 80–160 | 25–50 |
| M2 Vertical slice | window, camera, sector streaming, ground, objects, hero with animations, sound, a walk through the starting region in 4K | 30–60 | 15–30 |
| M3 Rules | Ghidra + MCP with community names; character, skills and combat arts, combat, items and loot, AI, spawning; the Unicorn oracle | 120–240 | 40–80 |
| M4 Quests and world | NetScript VM, dialogues, triggers, NPCs, trading, blacksmith, day and night, weather | 60–120 | 25–50 |
| M5 UI and effects | full UI, 178 effects, profile-driven sound, movies | 50–100 | 20–40 |
| M6 Version 1.0 for Windows 11 | saves, settings, gamepad, 4K and HiDPI, polish, campaign playthroughs | 40–80 | 100–200 |
| **Total to version 1.0** | | **≈390–770** | **≈230–460** |
| M7 HD assets | upscaling, HD packs, mipmaps | 15–30 | 10–30 |
| M8 Platforms | Linux 5–10; Android 25–50; Chrome (WebAssembly) 25–50 | 55–110 | 20–40 |

### 9.3 Money and calendar (estimate)

Hypothetical cost at API prices (not money spent, see above):

| Model for the main work | To version 1.0 | With M7 and M8 |
|---|---|---|
| Claude Opus 5.5 | ≈$3.1k–13.9k (center ≈$6k) | ≈$3.7k–16.4k |
| Claude Sonnet 5.5 | ≈$1.6k–7.7k | ≈$1.8k–9.1k |
| Claude Fable 5.1 | ≈$7k–31k | ≈$8.3k–36.4k |

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
   Done 2026-10-18, ahead of M3: Ghidra 12.1.4 on Temurin 25, pyghidra-mcp connected to Claude Code, `Sacred.exe` and `GameServer.exe` imported and analyzed (docs/03 §3.2); the community names, the classes from RTTI and the marking of the library code remain (docs/03 §4.1, #88).
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
   Next: the issues of the M2 milestone (the frame time spikes #2, the roofs and walls in front of the hero #3, day and night #5, the regions' music #6).
8. Usage tracking: done 2026-10-16, per pull request since 2026-10-17 (#16).
   Every pull request's description carries its Claude Code usage (agent-hours, tokens and their hypothetical cost at API prices, §9), which `tools/usage/pr_usage.py` computes from the transcripts' messages on its branch; the README's badge sums them with the sessions recorded before in `docs/usage.md`.
   After 30 days, the §9 estimates are recomputed from the actual agent-hours.
9. Issues and pull requests: done 2026-10-17 (#1).
   Every change reaches `main` through a squash-merged pull request with green checks on Linux and Windows; Claude Code merges, except the pull requests that wait for the maintainer (`needs-maintainer`: the rules, the process, the licenses, the deviations from the original that can change the play seriously or carry legal consequences, the screenshot references, and contributors' work).
   The dependencies are updated at most once a day, in an auto-merged pull request.
   The process is in docs/03 §3.6.
10. **M2, tools to test and drive the game.**
    Planned 2026-10-19 (#165): every option of the game is a command line flag read before Bevy starts, so the hero is fixed for the process and each change is a restart.
    The plan, in [docs/10](docs/10-game-tools.md): settings in `%APPDATA%\OpenSacred\settings.toml` with borderless full screen by default and F11 for a window (#167); the HUD's portrait, health bar and gait removed (#168); the log on F4 and in a file (#169); a debug overlay on F3 (#170); the game's states, so that a game starts and ends inside the process (#171); a main menu with the choice of the hero (#172) and a pause menu on Esc (#178); one set of commands for the command line's scripts (#173), the console on `~` (#174) and remote control over Bevy's Remote Protocol (#175); and a project skill or an MCP server through which Claude Code drives the game (#176).
    ≈10–20 agent-hours (estimate).

### 10.1 Priority queue

Since 2026-10-17 the queue is the repository's open issues (github.com/zarcsis/OpenSacred/issues), and the milestones M2–M8 of §9.2 are GitHub milestones.
Work takes this order: the known problems and bugs (`bug`), then the dependency upgrades (`deps`), then new features (`feature`, `research`), the earliest milestone first.
The daily dependency check opens a `deps` issue for every newer incompatible release of a direct dependency or of Rust (`CLAUDE.md`, "Work order"); on 2026-10-17 there were none: all 15 direct dependencies were at their latest releases and Rust 1.99.1 was the latest stable.

## 11. Top risks

| Risk | Probability | Impact | What we do |
|---|---|---|---|
| Granny animations convert with errors (poses, weights) | medium | high | three open readers of the format as a reference; control poses; an early start in M1 |
| Combat and loot rules are recovered incorrectly | medium | medium | the Unicorn oracle and vectors; fidelity is not a priority, but deviations are recorded |
| NetScript semantics are harder than expected | medium | high | a disassembler and listings early; opcodes are covered by frequency: 45 types account for 95% of records |
| Bevy breaks its API more often than is worthwhile | high | medium | scheduled updates; rules outside the engine; the fallback is Godot + godot-rust |
| Token use exceeds the estimate (usage limits slow the pace) | medium | medium | tracking from day one; Sonnet for mechanical work; small tasks |
| Maintainer burnout | medium | high | the vertical slice gives a visible result early; milestones are short |
| Claude Code misunderstands the original's behavior | medium | medium | vector tests, screenshot tests, the decision log; maintainer review of key formulas |
| Game content gets into the repository (a test fixture, an issue attachment, converter output) | low | high | no game bytes in the repository or CI; tests use synthetic data or the user's local installation; the same rules for issues and pull requests (§12) |

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
2. Whether Android and the web are needed: a decision after version 1.0.

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
