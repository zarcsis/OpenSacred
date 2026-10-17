# Language and engine: Rust or C++, our own engine or an existing one

## Summary

* **The language is Rust for the entire product:** the game, the asset converter, the tools. One language for everything is a project goal, with a chance to be the first project of its kind (§1.3), and it is technically feasible: Bevy, wgpu, Kira and Symphonia cover all the needed subsystems in pure Rust.
* **No intermediate C++ layer.** The original is decompiled only to understand its formats and rules; the behavior is described in prose in `docs/re/`, implemented independently in Rust and checked against vectors recorded from the developer's own `Sacred.exe` (§2). Anything made locally while studying the original is never committed. VC6 is not needed.
* **The engine is Bevy 0.20 on wgpu 30.** In the matrix it scores 410 out of 500, against 375 for Godot with godot-rust, 345 for Godot with C++ and 220 for Unreal Engine 5.8.
* **Insurance:** the game rules live in the `opensacred-rules` crate with no engine dependencies. A switch to Godot with godot-rust keeps the rules and the converter.
* **Unsafe code is forbidden** in all product crates (`#![forbid(unsafe_code)]`); the only exception is the development tool `opensacred-oracle`, which calls the Unicorn emulator and is a separate crate, never linked into the converter or the game.

## 1. Why Rust

### 1.1 What it gives

| Property | What it means for the project |
|---|---|
| One language for everything | The converter and the game share data types (`opensacred-data`); no second toolchain is needed |
| Memory safety without a garbage collector | No class of bugs that code from 2006 is full of (dangling pointers, buffer overflows) |
| Cargo and crates.io | Builds and dependencies with one command; scheduled updates (`cargo update`, Dependabot) |
| wgpu | One graphics codebase for Vulkan, DX12, WebGPU and GLES, i.e. for Windows, Linux, Android and Chrome |
| Strong types and tests | Claude Code gets fast feedback from the compiler and tests; misunderstandings surface early |

### 1.2 What it costs

| Cost | How we mitigate it |
|---|---|
| Long Bevy compile times | dynamic linking of Bevy in debug, `mold`/`lld`, the sccache cache, separate crates |
| Before 1.0, Bevy breaks its API in every release | scheduled migrations; rules outside the engine |
| No mechanical path from a C++ binary to Rust (a transpiler such as cxgo handles only C) | none is wanted: each rule is described as behavior and implemented independently, with vector tests; see §2 and §3 |
| Ecosystem crates catch up with a Bevy release with a delay (for example, `bevy_hanabi` 0.19.0 is still on the Bevy 0.19 line) | upgrade to a new Bevy once the needed crates are out; keep critical subsystems in our own code |

### 1.3 Pioneering

The survey of comparable projects (docs/09 §5) found no full reimplementation of a 1998–2006 commercial game in Rust. All the projects of that kind for Windows games of that era it found are in C/C++; the only cross-language precedent is OpenNox (C → Go through the cxgo transpiler). The project may become the first example of "asset conversion + reimplementation in Rust with Bevy" for a game of this scale.

## 2. How the original's logic gets into Rust

### 2.1 The method

The original's code is translated neither into Rust nor into an intermediate C++. Every format and rule goes through four steps (the legal basis is in docs/08 §2 and §3):

1. **Understand.** Where the data alone does not tell, the functions that hold a format or a rule are decompiled in the developer's local Ghidra project (docs/03 §4), only to understand what they do.
2. **Describe.** The behavior is written down in prose: facts about the formats in `docs/02-data-formats.md`, the rules in `docs/re/<subsystem>.md` (inputs, outputs, formulas, tables, edge cases), with the original's names and addresses as references. Decompiler output is never pasted in.
3. **Implement.** The Rust code in `opensacred-formats` or `opensacred-rules` is written independently from that description, in the project's own structure (§3).
4. **Verify.** A reader is tested on every file of the installation and cross-checked with the Python prototype in `tools/analysis/`; a rule is compared with vectors that `opensacred-oracle` records from the developer's own `Sacred.exe` (§2.3) and, where the oracle cannot reach, with observations of the running original (x64dbg, dgVoodoo2; docs/03 §1).

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

`opensacred-oracle` (not written yet) loads the image of the developer's own GOG `Sacred.exe` into the memory of the Unicorn emulator (the `unicorn-engine` 2.1.5 crate, GPL-2.0), substitutes stubs for CRT and Windows calls, and executes the chosen function on a set of inputs. The results are written to JSON and become the reference for the Rust tests.

It is a local development tool: it needs the developer's own copy of the game, so it runs on the developer's machine, never in CI. Because GPL-2.0 cannot be combined with the project's Apache-2.0 option, it is a separate development-only crate, never linked into the converter or the game (docs/08 §4).

Advantages: it works on any OS, does not require running the game, and is deterministic. Limitation: it suits "pure" functions: formulas, generators, transformations. Functions that touch D3D, files or threads cannot be checked this way; for those there is x64dbg on Windows 11 (`docs/03-toolchain.md`).

## 3. Implementing the logic in Rust without unsafe

| Original construct | Rust solution |
|---|---|
| Class hierarchies with virtual functions (`cObject → cCreature → cCreatureHero`, 410 vtables) | ECS components and enumerations (`enum`) instead of inheritance; traits where polymorphism is needed |
| 178 effect classes `cParticleSystem_*` with the same structure | one effect description in data (RON) + a shared executor |
| Raw pointers, intrusive lists | indices and handles (`slotmap` 1.1.1), Bevy entities |
| Structures written to files from memory | parsing only in `opensacred-formats` by safe means: our own bounds-checked slice reader `Reader` (decision of 2026-10-16: errors name the record and the offset; data quirks are logic, not layout; zero-copy is not needed because the game does not read the original formats); after that, ordinary `serde` types |
| Global variables | Bevy resources or an explicit context in `opensacred-rules` |
| `rand()` from the CRT (LCG 214013/2531011) | `rand_pcg`; to keep the "feel" of the loot the LCG can be reproduced, exactness is not required |
| x87 numbers with 80-bit intermediate values | `f32`/`f64`; bit-exact precision is not a goal |
| The NetScript interpreter built on `switch` and `goto` | `match` on the record type over the parsed IR; no jumps by offset |
| Engine threads and critical sections | the Bevy system schedule; the rules are single-threaded and deterministic within one machine |

The `unsafe` policy: `#![forbid(unsafe_code)]` in `opensacred-formats`, `opensacred-data`, `opensacred-convert`, `opensacred-rules`, `opensacred-game`. Unsafe operations exist inside dependencies (wgpu, Unicorn), but not in our code. The only exception is `opensacred-oracle`, and only if the Unicorn crate's wrapper requires it.

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

**Bevy.** Pros: pure Rust; scenes and logic in code; a headless mode for tests; glTF with skeletons and an animation graph with blending; Bevy UI and the Feathers widget set; asset hot reloading; KTX2 with Basis Universal; Android and the web out of the box. Cons: three breaking releases in 2026 (0.18, 0.19, 0.20); no ready-made visual editor; part of the ecosystem lags behind the releases. Sacred barely needs an editor: the world and the content come from the converter.

**Our own engine on wgpu.** Pros: full control, fewer dependencies, less breakage. Cons: we would have to write the asset system, animation blending, UI, scenes and debugging tools ourselves. That is dozens of extra agent-hours with no benefit for the "play comfortably" goal.

**Fyrox 1.0.** Pros: a stable API (1.0), a built-in editor, animation state machines, UI. Cons: an OpenGL renderer, not Vulkan; the workflow is tied to the editor; a smaller community than Bevy's.

**Godot + godot-rust.** Pros: the fastest path to playable: a mature renderer, animation trees, UI on Control nodes, audio buses, export to Android and the web. The logic can be written in Rust. Cons: two worlds, Godot scenes and Rust code; godot-rust is at version 0.x; WebAssembly support is behind the `experimental-wasm` flag, and Android is experimental too; asset import and debugging are tied to the editor, which Claude Code cannot see.

**Godot + C++.** Pros: more stable than godot-rust, official support for all platforms. Cons: a second language; contradicts the "one language, Rust" goal.

**Unreal Engine 5.8 + C++.** Pros: the most powerful renderer and tools. Cons: binary `.uasset` files and Blueprints that Claude Code cannot edit; hundreds of gigabytes and long builds; no web export; the license is not open; the power is overkill for a 2006 isometric game. Rejected.

## 5. Decision and insurance

**Decision: our own Rust version built on Bevy.** Of the two paths, "our own Rust version" or "an existing engine + C++", the project takes its own Rust version, but not from scratch: on top of Bevy as a ready-made set of subsystems.

**Sensitivity.** If the weight of "work until playable" is doubled and the weight of Rust is halved, Godot + Rust scores the same as Bevy. That is why the architecture insures against a wrong choice:

* `opensacred-rules` (rules) and `opensacred-convert` (converter) do not depend on the engine;
* `opensacred-data` (asset types) consists of ordinary `serde` structures that any code can read;
* the presentation layer `opensacred-game` is thin: it connects the rules to rendering, sound and input.

**Triggers for revisiting** (if one fires, the comparison is redone):

1. a migration to a new Bevy regularly takes more than 10 agent-hours;
2. Bevy's animation or UI system blocks the vertical slice for more than two weeks;
3. Android or the web needs a capability that wgpu or Bevy lacks.

**What to do when switching to Godot:** connect `opensacred-rules` through godot-rust as a GDExtension, rewrite the presentation in Godot scenes, keep the converter. The switch is estimated at 40–80 agent-hours, depending on the milestone at which it happens.

## 6. Crate set

| Task | Crate | Version | License |
|---|---|---|---|
| Engine | bevy | 0.20.0 | MIT or Apache-2.0 |
| Graphics API (through Bevy) | wgpu | 30.0.1 | MIT or Apache-2.0 |
| Sound | bevy_kira_audio / kira | 0.27.1 / 0.12.5 | MIT or Apache-2.0 |
| Audio decoders | symphonia | 0.6.1 | MPL-2.0 |
| Input with remapping | leafwing-input-manager | 0.22.0 | MIT or Apache-2.0 |
| Debug UI and tools | bevy_egui / egui | 0.43.1 / 0.37.0 | MIT / MIT or Apache-2.0 |
| Particles (once ready for Bevy 0.20) | bevy_hanabi | 0.19.0 | MIT or Apache-2.0 |
| AV1 video | rav1d | 1.1.0 | BSD-2-Clause |
| WebM container | matroska-demuxer or webm-iterable | 0.8.1 / 0.7.1 | Zlib, MIT or Apache-2.0 / MIT |
| Serialization | serde, ron, postcard, toml | 1.0.229, 0.12.2, 1.1.3, 1.1.8 | MIT or Apache-2.0 |
| Localization | fluent | 0.17.0 | MIT or Apache-2.0 |
| Binary format parsing | our own `opensacred_formats::reader::Reader`; binrw, zerocopy and bytemuck are not used (see above) | — | — |
| Compression | miniz_oxide (zlib: decompression with an output size limit), zstd | 0.9.1, 0.14.2 | MIT, Zlib or Apache-2.0; BSD-3-Clause |
| Images and textures | png (PNG master), ktx2, intel_tex_2 (BC7) | 0.18.1, 0.5.0, 0.5.0 | MIT or Apache-2.0; Apache-2.0; MIT or Apache-2.0 |
| Converter CLI | clap, rayon, anyhow, thiserror | 4.6.7, 1.12.0, 1.0.104, 2.0.21 | MIT or Apache-2.0 |
| Tests | insta, proptest | 1.49.0, 1.12.0 | Apache-2.0; MIT or Apache-2.0 |
| Modding scripts (optional) | rhai | 1.26.1 | MIT or Apache-2.0 |
| Upscaling through ONNX (M7, optional) | ort | 2.0.0-rc.14 | MIT or Apache-2.0 |
| Oracle (development only) | unicorn-engine | 2.1.5 | GPL-2.0 |
| User directories | directories | 6.0.0 | MIT or Apache-2.0 |

Update policy: Rust stable at the latest version (currently 1.99.1), updated on release day; dependencies weekly (`cargo update` and Dependabot); Bevy when a release comes out, after the ecosystem crates we use have been updated.

## Open questions

1. Readiness of `bevy_hanabi` and other ecosystem crates for Bevy 0.20: check at the start of M2.
2. Whether oracle vectors pin down the formulas of M3 where the order of floating-point operations matters, or whether observations of the running original are needed as well: this will become clear with the first formulas of M3.
3. Whether Bevy's animation graph is enough for blending Sacred's clips (attacking while running, transitions): check in the vertical slice.
4. The Fyrox 1.0 renderer: confirm from the source code if Fyrox is ever reconsidered.

## Sources

* Precedents (OpenNox, OpenGothic, isle-portable): docs/09 §5 and the links there.
* Crate versions: crates.io as of 2026-10-16; Rust 1.99.1: the stable channel.
* Bevy 0.20: bevy.org/news/bevy-0-20 and release reviews; Godot 4.7.2: godotengine.org; godot-rust WebAssembly: docs.rs/godot 0.5.5; Fyrox 1.0: fyrox.rs/blog and secondary reviews; Unreal Engine 5.8: unrealengine.com; Visual Studio 2026 and MSVC 14.50: devblogs.microsoft.com/cppblog.
