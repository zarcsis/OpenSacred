# Language and engine: Rust or C++, our own engine or an existing one

## Summary

* **The language is Rust for the entire product:** the game, the asset converter, the tools.
  One language for everything is a project goal, with a chance to be the first project of its kind (§1.3), and it is technically feasible: Bevy, wgpu, Kira and Symphonia cover all the needed subsystems in Rust.
  One C library comes along: Bevy decodes its KTX2 tone-mapping tables with the C zstd (`zstd_c` in `Cargo.toml`), as the Rust decoder has an open advisory, and the textures' KTX2 files are supercompressed with it (#700).
  The converter links the BC7 encoder's kernels, which `intel_tex_2` ships compiled from Intel's ISPC sources, so that no ISPC compiler is needed.
* **No intermediate C++ layer.**
  The original is decompiled only to understand its formats and rules; the behavior is described in prose in `docs/re/`, implemented independently in Rust and checked by tests written from that description and by observations of the running original (#227).
  Vectors recorded from the developer's own `Sacred.exe` by the oracle check them as well (§2.3, #363): the fight's levels, hit chance, damage through a resistance and most health so far.
  Anything made locally while studying the original is never committed.
  VC6 is not needed.
* **The engine is Bevy 0.20 on wgpu 30.**
  In the matrix it scores 410 out of 500, against 375 for Godot with godot-rust, 345 for Godot with C++ and 220 for Unreal Engine 5.8.
* **Insurance:** the game rules live in the `opensacred-rules` crate with no engine dependencies, created 2026-10-21 with the spawning's rolls, which the fight's formulas, the ways and the creatures' decisions joined from `opensacred-game` (#364).
  With it, a switch to Godot with godot-rust keeps the rules and the converter.
  The Bevy systems of `opensacred-game` call it: the fight's (`fight.rs`) the crate's formulas, the walks (`paths.rs`, `crowd.rs`) its ways, and the creatures' life and the hero's attack (`ai.rs`, `attack.rs`) its decisions, for which they read the world and which they carry out.
* **Unsafe code is forbidden** in all product crates by the workspace lint `unsafe_code = "deny"` (`Cargo.toml`), which every crate takes through `[lints] workspace = true`, the development tool `opensacred-oracle` included: it calls the Unicorn emulator through its safe wrapper, and is a separate crate, never linked into the converter or the game.

## 1. Why Rust

### 1.1 What it gives

| Property | What it means for the project |
|---|---|
| One language for everything | The converter and the game share data types (`opensacred-data`); no second toolchain is needed |
| Memory safety without a garbage collector | No class of bugs that code from 2006 is full of (dangling pointers, buffer overflows) |
| Cargo and crates.io | Builds and dependencies with one command; daily updates (`cargo update` in a pull request of its own, docs/03 §3.3) |
| wgpu | One graphics codebase for Vulkan, DX12 and GLES, i.e. for Windows, Linux and Android |
| Strong types and tests | Claude Code gets fast feedback from the compiler and tests; misunderstandings surface early |

### 1.2 What it costs

| Cost | How we mitigate it |
|---|---|
| Long Bevy compile times | dependencies optimized in debug builds (`Cargo.toml`), the `rust-lld` linker on Windows (`.cargo/config.toml`), CI restoring `main`'s build cache, separate crates |
| Before 1.0, Bevy breaks its API in every release | scheduled migrations; rules outside the engine (`opensacred-rules`, filling up, #364) |
| No mechanical path from a C++ binary to Rust (a transpiler such as cxgo handles only C) | none is wanted: each rule is described as behavior and implemented independently, with tests and the oracle's vector tests; see §2 and §3 |
| Ecosystem crates catch up with a Bevy release with a delay (for example, `bevy_hanabi` 0.19.0 is still on the Bevy 0.19 line) | upgrade to a new Bevy once the needed crates are out; keep critical subsystems in our own code |

### 1.3 Pioneering

The survey of comparable projects (docs/09 §5) found no full reimplementation of a 1998–2006 commercial game in Rust.
All the projects of that kind for Windows games of that era it found are in C/C++; the only cross-language precedent is OpenNox (C → Go through the cxgo transpiler).
The project may become the first example of "asset conversion + reimplementation in Rust with Bevy" for a game of this scale.

## 2. How the original's logic gets into Rust

### 2.1 The method

The original's code is translated neither into Rust nor into an intermediate C++.
Every format and rule goes through four steps (the legal basis is in docs/08 §2 and §3):

1. **Understand.**
   Where the data alone does not tell, the functions that hold a format or a rule are decompiled in the developer's local Ghidra project (docs/03 §4), only to understand what they do.
2. **Describe.**
   The behavior is written down in prose: facts about the formats in `docs/02-data-formats.md`, the rules in `docs/re/<subsystem>.md` (inputs, outputs, formulas, tables, edge cases), with the original's names and addresses as references.
   Decompiler output is never pasted in.
3. **Implement.**
   The Rust code in `opensacred-formats` or `opensacred-rules` is written independently from that description, in the project's own structure (§3).
   `opensacred-rules` holds the spawning's rolls, the fight's formulas, the ways and the creatures' decisions so far, which the systems of `opensacred-game` call (#364).
4. **Verify.**
   A reader is tested on every file of the installation and cross-checked with the Python prototype in `tools/analysis/`; a rule is compared with vectors that `opensacred-oracle` records from the developer's own `Sacred.exe` (§2.3) and, where the oracle cannot reach, with observations of the running original (x64dbg, and the driver through which Claude Code runs it itself, planned, #448; docs/03 §1, §3.8).
   A rule without vectors yet is checked by unit tests written from its description and by observations of the running original (#227).

Some functions are hard to understand from the decompilation alone:

* arithmetic with pointers and unions (structures written to a file directly from memory);
* bit fields, packed flags, tricky shifts;
* long functions with `goto` branches (the NetScript interpreter, item generation);
* formulas where the order of floating-point operations matters.

For those the description is checked against more vectors and edge cases, not against a compiled copy of the original's code.

### 2.2 What stays local

| What | Where it lives |
|---|---|
| The Ghidra project, its decompilation and listings | the developer's machine; Ghidra's project files are ignored by Git |
| Community name exports (the sacred-bild function list, GPL-3.0) | imported into the local Ghidra project only (docs/09 §3.3) |
| Scratch code written to check an understanding of a function, in any language | the developer's machine; never committed |
| What is committed | facts in `docs/02-data-formats.md`, descriptions of the behavior in `docs/re/`, our own Rust code and its tests |

### 2.3 The oracle: the original's functions in an emulator

`opensacred-oracle` (#363) loads the image of the developer's own GOG `Sacred.exe` into the memory of the Unicorn emulator (the `unicorn-engine` 2.1.5 crate, GPL-2.0), substitutes stubs for the calls that would reach the engine and for the imports, and executes the chosen function on a set of inputs.
The results are written to JSON outside the repository and become the reference for the Rust tests (`opensacred-rules`' `tests/oracle.rs`).
How it lays out the image, what it stubs and how it is run are in docs/03 §3.4.

It is a local development tool: it needs the developer's own copy of the game, so it runs on the developer's machine, never in CI.
Because GPL-2.0 cannot be combined with the project's Apache-2.0 option, it is a separate development-only crate, never linked into the converter or the game (docs/08 §4).

Advantages: it works on any OS, does not require running the game, and is deterministic.
Limitation: it suits "pure" functions: formulas, generators, transformations, and a method whose `this` can be laid out from docs/re's offsets (the stats of `0x5658F0`).
Its first sets (2026-10-21) covered the levels' ends and the levels of an experience, the hit chance and each kind's damage through its resistance, and the most health; five differences from `opensacred-rules` were found and filed, three of them fixed since (#406, #407, #408; #417, #418 open).
Functions that touch D3D, files or threads cannot be checked this way; for those there is x64dbg on Windows 11 (`docs/03-toolchain.md`) and the running original that Claude Code drives under Frida (#448, planned; docs/03 §3.8).

## 3. Implementing the logic in Rust without unsafe

| Original construct | Rust solution |
|---|---|
| Class hierarchies with virtual functions (`cObject → cCreature → cCreatureHero`, 410 vtables) | ECS components and enumerations (`enum`) instead of inheritance; traits where polymorphism is needed |
| 178 effect classes `cParticleSystem_*` with the same structure | one effect description in data (RON) + a shared executor |
| Raw pointers, intrusive lists | indices and Bevy entities |
| Structures written to files from memory | parsing only in `opensacred-formats` by safe means: our own bounds-checked slice reader `Reader` (decision of 2026-10-16: errors name the record and the offset; data quirks are logic, not layout; zero-copy is not needed because the game does not read the original formats); after that, ordinary `serde` types |
| Global variables | Bevy resources or an explicit context in `opensacred-rules` |
| `rand()` from the CRT (LCG 214013/2531011) | a small generator of our own (SplitMix64, `opensacred-rules`'s `spawning.rs`), seeded by the sector and the record, so that a sector that comes back has the same creatures; to keep the "feel" of the loot the LCG can be reproduced, exactness is not required |
| x87 numbers with 80-bit intermediate values | `f32`/`f64`; bit-exact precision is not a goal |
| The NetScript interpreter built on `switch` and `goto` | the scripts translated by the converter into structured Luau, run through mlua (PLAN.md ADR-13) |
| Engine threads and critical sections | the Bevy system schedule; in `opensacred-rules` (#364) the rules are to be single-threaded and deterministic within one machine; today they run as Bevy systems of `opensacred-game` on each frame's time |

The `unsafe` policy: the workspace lint `unsafe_code = "deny"` (`Cargo.toml`, `[workspace.lints.rust]`), which `opensacred-formats`, `opensacred-data`, `opensacred-rules`, `opensacred-convert` and `opensacred-game` take through `[lints] workspace = true`.
Unsafe operations exist inside dependencies (wgpu, Unicorn), but not in our code.
`opensacred-oracle` takes the lint too: the `unicorn-engine` crate's safe wrapper is enough for it, and it would be the exception only if a wrapper required `unsafe`.

## 4. Engine comparison

### 4.1 Candidates

| Option | Version (2026-10-16) | License | Renderer | Logic language |
|---|---|---|---|---|
| Bevy + our own rules crate | bevy 0.20.0 (2026-10-08), wgpu 30.0.1 | MIT or Apache-2.0 | wgpu: Vulkan, DX12, Metal, WebGPU, WebGL2 | Rust |
| Our own engine | wgpu 30, winit 0.30.13, kira 0.12.5, egui 0.37.0, cosmic-text 0.19.0 | MIT or Apache-2.0 | wgpu | Rust |
| Fyrox | 1.0.1 (2026-03-28) | MIT | OpenGL and WebGL2 (per secondary sources; Vulkan is not claimed) | Rust, has an editor |
| Godot + godot-rust | Godot 4.7.2, crate `godot` 0.5.5 | MIT | Vulkan (Forward+, Mobile), D3D12; WebGL2 on the web (Compatibility) | Rust through GDExtension + Godot scenes |
| Godot + C++ | Godot 4.7.2, godot-cpp | MIT | the same | C++ |
| Unreal Engine | 5.8 | free; source available under the Epic license; royalties on commercial products | DX12, Vulkan | C++ and Blueprints |

### 4.2 Criteria and weights

| Criterion | Weight | Why this weight |
|---|---|---|
| Single language, Rust | 15 | a project goal |
| Convenience for Claude Code | 20 | development is almost entirely through Claude Code: everything must be text, build from the command line and be testable without a graphical editor |
| Amount of own work until playable | 15 | a solo project; the more the engine provides, the faster the result |
| Target platforms (Windows 11, Linux, Android, Chrome) | 10 | Windows is mandatory, the rest is a prospect |
| Vulkan and a modern renderer | 10 | a project requirement |
| API stability | 10 | frequent breakage eats agent-hours |
| 2.5D isometric and skeletal animation from glTF | 10 | the essence of Sacred's graphics |
| Free of charge, license | 5 | a mandatory condition; all candidates are free |
| Build iteration speed | 5 | affects the agent's working pace |

### 4.3 Matrix

| Criterion (weight) | Bevy | Own | Fyrox | Godot + Rust | Godot + C++ | UE5 + C++ |
|---|---|---|---|---|---|---|
| Rust (15) | 5 | 5 | 5 | 4 | 1 | 1 |
| Claude Code (20) | 5 | 5 | 3 | 3 | 3 | 1 |
| Work until playable (15) | 3 | 1 | 3 | 4 | 4 | 3 |
| Platforms (10) | 4 | 4 | 3 | 3 | 4 | 2 |
| Vulkan (10) | 5 | 5 | 2 | 5 | 5 | 4 |
| Stability (10) | 2 | 4 | 4 | 3 | 4 | 4 |
| Isometric and animation (10) | 4 | 3 | 4 | 4 | 4 | 2 |
| License (5) | 5 | 5 | 5 | 5 | 5 | 3 |
| Build (5) | 3 | 4 | 3 | 4 | 3 | 1 |
| **Total out of 500** | **410** | **395** | **350** | **375** | **345** | **220** |

### 4.4 Option by option

**Bevy.**
Pros: pure Rust; scenes and logic in code; a headless mode for tests; glTF with skeletons and an animation graph with blending; Bevy UI and the Feathers widget set; asset hot reloading; KTX2 with Basis Universal; Android and the web out of the box.
Cons: three breaking releases in 2026 (0.18, 0.19, 0.20); no ready-made visual editor; part of the ecosystem lags behind the releases.
Sacred barely needs an editor: the world and the content come from the converter.

**Our own engine on wgpu.**
Pros: full control, fewer dependencies, less breakage.
Cons: we would have to write the asset system, animation blending, UI, scenes and debugging tools ourselves.
That is dozens of extra agent-hours with no benefit for the "play comfortably" goal.

**Fyrox 1.0.**
Pros: a stable API (1.0), a built-in editor, animation state machines, UI.
Cons: an OpenGL renderer, not Vulkan; the workflow is tied to the editor; a smaller community than Bevy's.

**Godot + godot-rust.**
Pros: the fastest path to playable: a mature renderer, animation trees, UI on Control nodes, audio buses, export to Android and the web.
The logic can be written in Rust.
Cons: two worlds, Godot scenes and Rust code; godot-rust is at version 0.x; WebAssembly support is behind the `experimental-wasm` flag, and Android is experimental too; asset import and debugging are tied to the editor, which Claude Code cannot see.

**Godot + C++.**
Pros: more stable than godot-rust, official support for all platforms.
Cons: a second language; contradicts the "one language, Rust" goal.

**Unreal Engine 5.8 + C++.**
Pros: the most powerful renderer and tools.
Cons: binary `.uasset` files and Blueprints that Claude Code cannot edit; hundreds of gigabytes and long builds; no web export; the license is not open; the power is overkill for a 2006 isometric game.
Rejected.

## 5. Decision and insurance

**Decision: our own Rust version built on Bevy.**
Of the two paths, "our own Rust version" or "an existing engine + C++", the project takes its own Rust version, but not from scratch: on top of Bevy as a ready-made set of subsystems.

**Sensitivity.**
If the weight of "work until playable" is doubled and the weight of Rust is halved, Godot + Rust scores the same as Bevy.
That is why the architecture insures against a wrong choice:

* `opensacred-rules` (rules) and `opensacred-convert` (converter) do not depend on the engine;
* `opensacred-data` (asset types) consists of ordinary `serde` structures that any code can read;
* the presentation layer `opensacred-game` is to be thin: it connects the rules to rendering, sound and input.
  `opensacred-game` still holds the parts of the rules followed so far that its systems weave into reading the world (#364).

**Triggers for revisiting** (if one fires, the comparison is redone):

1. a migration to a new Bevy regularly takes more than 10 agent-hours;
2. Bevy's animation or UI system blocks the vertical slice for more than two weeks;
3. Android needs a capability that wgpu or Bevy lacks.

**What to do when switching to Godot:** connect `opensacred-rules` through godot-rust as a GDExtension, rewrite the presentation in Godot scenes, keep the converter.
The switch is estimated at 40–80 agent-hours, depending on the milestone at which it happens.

## 6. Crate set

### 6.1 In use

The direct dependencies of `[workspace.dependencies]` in `Cargo.toml` and what Bevy brings for the project's needs, at their versions in `Cargo.lock` on 2026-10-20:

| Task | Crate | Version | License |
|---|---|---|---|
| Engine | bevy | 0.20.0 | MIT or Apache-2.0 |
| Graphics API (through Bevy) | wgpu | 30.0.1 | MIT or Apache-2.0 |
| Text (through Bevy) | parley | 0.11.1 | MIT or Apache-2.0 |
| Sound | bevy_kira_audio / kira | 0.27.1 / 0.12.5 | MIT or Apache-2.0 |
| Audio decoders (through Kira: FLAC, MP3) | symphonia | 0.6.1 | MPL-2.0 |
| Serialization | serde, ron, postcard, serde_json | 1.0.229, 0.12.2, 1.1.3, 1.0.154 | MIT or Apache-2.0 |
| Settings file (comments and order kept) | toml_edit | 0.25.17 | MIT or Apache-2.0 |
| The game's commands (shell-like splitting) | shlex | 2.0.1 | MIT or Apache-2.0 |
| Binary format parsing | our own `opensacred_formats::reader::Reader`; binrw, zerocopy and bytemuck are not used (see above) | — | — |
| Compression | miniz_oxide (zlib: decompression with an output size limit, and the zlib stream of the postcard files) | 0.9.1 | MIT, Zlib or Apache-2.0 |
| Checksums | sha2 (the manifest), crc32fast | 0.11.1, 1.5.2 | MIT or Apache-2.0 |
| Images and textures | png (PNG masters); intel_tex_2 (the converter's BC7 encoder, #700); zstd (the KTX2 files' supercompression, the C library); ktx2 (Bevy's reader, which the tests check our writer against) | 0.18.1, 0.5.0, 0.13.3, 0.5.0 | MIT or Apache-2.0; MIT or Apache-2.0; MIT (zstd-sys and the C library BSD-3-Clause); Apache-2.0 |
| Converter CLI | clap, rayon, anyhow, thiserror | 4.6.7, 1.12.0, 1.0.104, 2.0.21 | MIT or Apache-2.0 |
| Tests (the converter's Fluent output, its BC7 decoded) | fluent-syntax, bcdec_rs | 0.12.0, 0.2.1 | MIT or Apache-2.0; MIT |

The game reads `global.ftl` with a reader of its own (`opensacred_data::text`), finds the user's directories from the environment (`settings.rs`), takes input from Bevy's `ButtonInput`, draws its debug panels and console in Bevy UI, and rolls with a generator of its own (§3).

### 6.2 Planned, not yet a dependency

Versions as of 2026-10-16.

| Task | Crate | Version | License |
|---|---|---|---|
| Input with remapping | leafwing-input-manager | 0.22.0 | MIT or Apache-2.0 |
| Debug UI and tools | bevy_egui / egui | 0.43.1 / 0.37.0 | MIT / MIT or Apache-2.0 |
| Particles (once ready for Bevy 0.20) | bevy_hanabi | 0.19.0 | MIT or Apache-2.0 |
| AV1 video | rav1d | 1.1.0 | BSD-2-Clause |
| WebM container | matroska-demuxer or webm-iterable | 0.8.1 / 0.7.1 | Zlib, MIT or Apache-2.0 / MIT |
| Localization at run time | fluent | 0.17.0 | MIT or Apache-2.0 |
| The sound's Opus (M3, #701): a decoder | libopus through a binding (chosen with #701) | — | BSD-3-Clause |
| The game's archives (M3, #699) | none: a reader and a writer of their own in `opensacred-data` over miniz_oxide and crc32fast, already dependencies (store and deflate are all they need) | 0.9.1, 1.5.3 | MIT or Zlib or Apache-2.0; MIT or Apache-2.0 |
| Tests | insta, proptest | 1.49.0, 1.12.0 | Apache-2.0; MIT or Apache-2.0 |
| Scripts: the translated NetScript, and mods after 1.0 (#725) | mlua with its `luau` feature (Luau vendored) | 0.12.2 | MIT |
| Upscaling through ONNX (M7, optional) | ort | 2.0.0-rc.14 | MIT or Apache-2.0 |
| Oracle (development only) | unicorn-engine | 2.1.5 | GPL-2.0 |

Luau took Rhai's place on 2026-10-23 (PLAN.md ADR-13, #720): its types let a definition file of the scripts' API check scripts in luau-lsp, its sandbox and interrupts come built in, the converter compiles its bytecode ahead, and it is faster.
It costs a C++ build of Luau's vendored sources, confined to the scripts' crate, and its WebAssembly needs Emscripten, which no longer matters since the browser version was dropped.

Update policy: Rust stable at the latest version (currently 1.99.1), bumped together with the dependencies; dependencies daily, in a `Dependencies: cargo update` pull request, and a `deps` issue for each newer incompatible release of a direct dependency or of Rust (`CLAUDE.md`, "Work order"; docs/03 §3.3); Bevy when a release comes out, after the ecosystem crates we use have been updated.

## Open questions

1. Readiness of `bevy_hanabi` and other ecosystem crates for Bevy 0.20: check at the start of M2.
2. Whether oracle vectors pin down the formulas of M3 where the order of floating-point operations matters, or whether observations of the running original are needed as well.
   The first vectors (#363) pin the whole-number results exactly and the floats of `0x43EEE0` within 10⁻⁵ of `f32` arithmetic, at the runtime's 53 bits of precision.
   The running original computes its world at 53 bits too: the driver's `fpu` (#589) reads the x87 control word 0x027F on the render thread's frame, in the stats' recomputation `0x5796A0` and in the strike `0x56B130`, all on that thread (through dgVoodoo2); only the main thread runs at 24 bits, which `WinMain` sets.
   Both of the game's cooperative levels with DirectDraw carry `DDSCL_FPUSETUP` (`0xC11` full screen, `0xC08` in a window; docs/re/window.md, #551), by which Direct3D leaves the FPU as it finds it, so the system's Direct3D does not lower it either (**hypothesis** by the flags; measured through dgVoodoo2 alone).
   So a whole number the original truncates can differ from one of `f32` arithmetic by one: `fight::combat` computes `A`, `D` and their multipliers at 53 bits (#593).
3. Whether Bevy's animation graph is enough for blending Sacred's clips (attacking while running, transitions): check in the vertical slice.
4. The Fyrox 1.0 renderer: confirm from the source code if Fyrox is ever reconsidered.

## Sources

* Precedents (OpenNox, OpenGothic, isle-portable): docs/09 §5 and the links there.
* Crate versions: `Cargo.lock` on 2026-10-20 (§6.1), crates.io as of 2026-10-16 (§4.1, §6.2); Rust 1.99.1: the stable channel and `rust-toolchain.toml`.
* Bevy 0.20: bevy.org/news/bevy-0-20 and release reviews; Godot 4.7.2: godotengine.org; godot-rust WebAssembly: docs.rs/godot 0.5.5; Fyrox 1.0: fyrox.rs/blog and secondary reviews; Unreal Engine 5.8: unrealengine.com; Visual Studio 2026 and MSVC 14.50: devblogs.microsoft.com/cppblog.
