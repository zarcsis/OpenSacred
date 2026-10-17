# Tooling and the workflow with Claude Code

## Summary

* All tools are free. Binary analysis uses **Ghidra 12.1.4** with an MCP server for Claude Code (**GhidraMCP 2.0** or **pyghidra-mcp**); IDA Pro and Binary Ninja are excluded.
* The product is built with **Rust 1.99.1** (edition 2024) and **Visual Studio Build Tools 2026** (MSVC 14.50 and the Windows SDK are needed for Rust on Windows). VC6, Wine environments and byte-matching tools are not needed.
* For assets: **Blender**, **glTF-Validator**, **KTX-Software**, **FFmpeg**; for render debugging: **RenderDoc** and **Vulkan SDK 1.4.363**; for behavior references: **Unicorn** (emulating the original's functions), **x64dbg** and **dgVoodoo2** (running the original on Windows 11).
* The process is built around Claude Code: `CLAUDE.md` with rules, MCP to Ghidra, hooks for formatting and tests, small tasks, token usage tracking from day one.
* Quality rests on automated checks: converter golden data, oracle vectors, snapshots and screenshot tests. Maintainer review covers key decisions, not every line.
* The game files are never in the repository: every developer points the tools at their own GOG installation in `OriginalGame/` (§1).

## 1. Tool map

| Stage | Tool | Version | License, price | Purpose |
|---|---|---|---|---|
| Binary analysis | Ghidra | 12.1.4 (JDK 25 LTS, Temurin) | Apache-2.0, free | disassembler and decompiler; RTTI, classes, a shared labeling database |
| | GhidraMCP | 2.0 (2026-10-10) | Apache-2.0 | MCP server: Claude Code reads the decompilation, renames functions and fields |
| | pyghidra-mcp | current | open source | a headless alternative without the Ghidra GUI; batch decompilation |
| | IDA Free | current | free for non-commercial use, cloud x86 decompiler | an optional "second opinion" on hard functions |
| | Python + pefile, capstone, numpy, Pillow, lief | Python 3.15.0 (minimum 3.12); packages in `tools/requirements.txt` | open source | research scripts in `tools/analysis/` |
| Behavior references | Unicorn | 2.x, crate `unicorn-engine` 2.1.5 | GPL-2.0, development only | `opensacred-oracle`, a separate development-only crate run locally on your own `Sacred.exe`: the original's functions in the emulator → vectors for tests |
| | x64dbg | current | GPL-3.0 | breakpoints in the original on Windows 11, testing hypotheses |
| | dgVoodoo2 | 2.87.x | freeware | running the original on Windows 11 (D3D7 → D3D11) for reference frames and in-game cross-checks |
| Product build | Rust (rustup, cargo, clippy, rustfmt) | 1.99.1, edition 2024 | MIT or Apache-2.0 | all product code |
| | Visual Studio Build Tools 2026 | MSVC 14.50 + Windows SDK | free for individual development | the linker for Rust on Windows |
| | sccache, cargo-nextest, cargo-deny | current | open source | build cache, fast tests, dependency license checks |
| Assets | Blender | 5.2 LTS | GPL | inspecting glTF models and animations |
| | Khronos glTF-Validator | 2.0.0-dev.3.10 (latest release, a pre-release) | Apache-2.0 | checking the `.glb` files from the converter |
| | KTX-Software (`ktx`, `toktx`) | current | Apache-2.0 | KTX2, BC7, Basis Universal |
| | FFmpeg | 9.0 | LGPL/GPL | WMV → AV1 + Opus in WebM; the VTex textures of GRN files (one-frame Bink 1) → RGBA (invoked by the converter as an external program) |
| | Real-ESRGAN ncnn-vulkan, waifu2x-ncnn-vulkan | current | BSD-3 / MIT | offline upscaling (M7) |
| Rendering | Vulkan SDK (LunarG) | 1.4.363.0 | free | validation layers, vkconfig |
| | RenderDoc | 1.46 | MIT | capturing and analyzing frames of our renderer |
| Process | Claude Code | current | subscription or API | the main executor of tasks |
| | Git, GitHub Actions | current | free (standard runners cost nothing for a public repository) | history, CI |
| | ccusage | current | open source | per-session tracking of Claude Code token usage |

**The game files.** The repository holds no file of the game. Every developer uses their own GOG installation of Sacred Gold 2.28: copied into `OriginalGame/` at the repository root or linked there (ignored by Git either way; the default `--game-dir` of the converter and of the research scripts), or passed with `--game-dir <path>`. On Windows a link is `New-Item -ItemType SymbolicLink -Path OriginalGame -Target <installation>`, which needs Developer Mode or an elevated shell. `cargo run -p opensacred-convert -- verify` checks it against the embedded manifest (`docs/manifest-gog-2.28.txt`). The manifest lists only the files the game never writes (docs/02 §13), so the same installation can be played from without breaking the checks. Nothing from it, and nothing converted from it, is ever committed (docs/08 §3, §4).

## 2. Workstation

* **OS:** Windows 11 x64 is the main platform for development and running. WSL2 with Ubuntu is for builds and tests on Linux.
* **GPU:** with a Vulkan 1.3 or newer driver; a mid-range graphics card for 4K tests.
* **Disk:** ≈3 GB for the GOG installation; ≈5–10 GB for the generated `assets/` (estimate); HD packs separately, up to tens of GB.
* **Setup (Windows):** Rust through rustup; Visual Studio Build Tools 2026 with the "Desktop development with C++" workload and the Windows SDK; Git; your own GOG installation in `OriginalGame/` (§1); Python 3.12+ for the research scripts; Ghidra 12.1.4 with JDK 25; Vulkan SDK; Blender.
* **The JDK for Ghidra is 25 LTS (Temurin), not 21.** Ghidra 12.1.4 accepts any 64-bit JDK of version 21 or later (`application.java.min=21`, no upper bound), and Ghidra's main branch already requires 25. We do not use non-LTS JDKs (26, 27): Ghidra has not been tested on them. If `JAVA_HOME` points to another JDK (for example, from Android Studio), the path to Temurin goes into `JAVA_HOME_OVERRIDE` in `support/launch.properties` in the Ghidra directory.
* **Tools outside `PATH`** (this is how their installers set them up): Blender at `C:\Program Files\Blender Foundation\Blender 5.2\blender.exe`, RenderDoc at `C:\Program Files\RenderDoc\`. The Vulkan SDK's `vulkaninfo` is named `vulkaninfoSDK.exe`; the program of the same name in `PATH` is an old version from the driver. glTF-Validator is installed manually from the `KhronosGroup/glTF-Validator` releases into `~/.local/bin`.
* **Not every tool is needed from the start:** Ghidra and the MCP server come in with M3, KTX-Software with the texture converter, the upscalers with M7.

```powershell
# Toolchain check
rustup update stable
rustc --version            # expect 1.99.1 or newer
cargo --version
cl                         # MSVC 19.50 from Build Tools 2026 (in Developer PowerShell)
cargo run -p opensacred-convert -- verify   # your installation in OriginalGame/ against the manifest
```

## 3. Workflow with Claude Code

### 3.1 The `CLAUDE.md` file

The repository root holds `CLAUDE.md` with the rules that Claude Code reads at the start of every session:

* the workspace structure and the purpose of each crate;
* commands: build, tests, running the converter, running the game;
* prohibitions: do not commit anything from `OriginalGame/`, `assets/`, `assets-hd/`; do not publish `Sacred.exe`; do not write `unsafe` outside `opensacred-oracle`;
* mandatory checks before finishing a task: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo nextest run`;
* where to write notes on recovered logic (`docs/re/<subsystem>.md`) and the decision log of deviations from the original (`docs/decisions.md`);
* naming conventions: the original class and function names (`cCreature::takeDamage`) are mentioned in comments next to the Rust code.

### 3.2 Ghidra MCP server

Claude Code connects to Ghidra through MCP and gets tools: decompile a function, find cross-references, rename, set a type. Two options:

* **GhidraMCP 2.0**: a plugin for Ghidra with a GUI; convenient when a developer is looking at Ghidra in parallel.
* **pyghidra-mcp**: a headless server; convenient for batch work without an open GUI.

```bash
# Example of connecting the headless server (check the flags against the pyghidra-mcp README)
claude mcp add ghidra -- uvx pyghidra-mcp --project-path ./re/ghidra ./OriginalGame/Sacred.exe
```

MCP tools that execute arbitrary Python inside Ghidra are run only locally. The Ghidra project, the decompilation and anything exported from it stay on the developer's machine (Ghidra's project files are ignored by Git) and are never committed: what reaches the repository is a description of the behavior in prose in `docs/re/` and our own code (docs/04 §2, docs/08 §3).

### 3.3 Task types

| Type | Input | Output | Check |
|---|---|---|---|
| Parse a format | a file from `OriginalGame/`, the facts in `docs/02-data-formats.md` | a reader in `opensacred-formats` + a converter in `opensacred-convert` | a test on all files of the installation; cross-check with the Python prototype |
| Recover a rule | the original's function in the local Ghidra project | a description of the behavior in `docs/re/`, oracle vectors, an independent implementation in `opensacred-rules` | the vectors pass; clippy without warnings |
| Implement a system | notes in `docs/re/`, data from `assets/` | code in `opensacred-game` or `opensacred-rules` | snapshot and screenshot tests; a manual check in the game |
| Migrate Bevy | the migration guide | an updated `opensacred-game` | everything builds, tests and screenshots match |
| Update dependencies | `cargo update` | a new `Cargo.lock` | CI is green, `cargo deny check` reports no violations |

Size rule: one task is one finished result, verifiable by a test, of 1–4 agent-hours. Large systems are cut into such tasks in the milestone plan.

Order (since 2026-10-17): the known problems and bugs first, then the dependency upgrades, then new features; the queue is PLAN.md §10.1. Every session starts with `cargo update` in a commit of its own, and records the newer incompatible releases of the direct dependencies and of Rust there as upgrade tasks (`CLAUDE.md`, "Work order"). Updating often keeps each step small; each `Cargo.lock` change costs CI one build without its dependency cache (about 20 minutes instead of 2), once a session.

### 3.4 Guardrails

* **Claude Code hooks** (`.claude/settings.json`, script `.claude/hooks/rust_checks.py`): after an edit of a `.rs` file, `cargo fmt --all`; when Claude stops, `cargo clippy --all-targets -- -D warnings` and `cargo nextest run` for the crates with uncommitted changes and the crates that depend on them. On failure Claude is sent back to fix it; if the checks still fail after that retry, Claude stops and the developer gets a warning. A passing state is remembered in `target/`, so a turn without new changes does not rebuild anything. Tests on the game files and `cargo deny check` stay manual.
* **Converter golden data:** checksums and snapshots (`insta`) for a sample of files; any change in the output shows up in the diff.
* **Oracle vectors:** JSON with the inputs and outputs of the original's functions, recorded by `opensacred-oracle` from the developer's own `Sacred.exe`; the Rust tests read them and, like the other tests that need the game, run locally.
* **Screenshot tests:** a headless render of a scene through wgpu to PNG and comparison with a reference within a tolerance.
* **Clippy:** the default rule set as errors, plus targeted rules for this project: numeric casts via `as`, `unwrap`/`expect` outside tests, suppressing rules only through `#[expect(…, reason = "…")]`, leftover `dbg!`/`todo!`. The pedantic and nursery groups are not enabled wholesale: on the project's code they produce mostly stylistic noise. Configuration: `[workspace.lints.clippy]` in `Cargo.toml` and `clippy.toml`.
* **License checks:** our crates are MIT OR Apache-2.0; `cargo deny check licenses` allows MIT, MIT-0, Apache-2.0, BSD, ISC, Zlib, MPL-2.0 and Unicode-3.0 (Unicode data in `unicode-ident`). The future `opensacred-oracle` links Unicorn (GPL-2.0), which cannot be combined with the Apache-2.0 option: it will be a separate development-only crate, never linked into the converter or the game. Configuration: `deny.toml`.
* **Tests that need the game files** are marked and run only locally: CI has no game files.

### 3.5 Parallel work and tracking

* Parallel Claude Code sessions run in separate `git worktree`s, one task per tree.
* Usage is recorded at the end of every session (`/cost` or ccusage) in the `docs/usage.md` table: task, model, agent-hours, tokens. After 30 days the estimates in `docs/07-roadmap-estimates.md` are recomputed from the actuals.

## 4. Binary analysis

### 4.1 The Ghidra project

1. Create a local project and import the GOG `Sacred.exe` (SHA-256 `db9b86a4…71e74`) and `GameServer.exe` from your own installation. The GOG build has no DRM; the files are analyzed as they are.
2. Run the analysis with the MSVC RTTI analyzer enabled; recover the classes with the RecoverClassesFromRTTI script.
3. Import the community names locally: the sacred-bild function export (GPL-3.0) and the sacred-sdk addresses (MIT). The GOG ENG addresses match them. The community files themselves are not copied into the repository.
4. Mark the library code (CRT, STLport, zlib, D3DX7); it is not analyzed.
5. Use `GameServer.exe` as an auxiliary binary: 1,176 game functions in it are byte-identical to the client, which helps confirm function boundaries.

### 4.2 What to analyze and what not

| Subsystem | Analyze the code? | Why |
|---|---|---|
| Attributes, skills, combat arts, combat, damage, resistances | yes | rules with numbers |
| Items, modifiers, sets, loot generation, blacksmith, prices | yes | rules with numbers |
| Creature AI, spawning | yes, selectively | behavior parameters and tables |
| NetScript interpreter, triggers, quests | yes | content semantics |
| Time of day, weather | selectively | parameters |
| Rendering, D3D7, particles (as code) | no | written from scratch; effect parameters are taken from the data and selectively from the code |
| UI, fonts, IME | no | a new interface |
| Sound (Miles), video (amstream), network (TinCat), files | no | replaced entirely or out of scope |

Scope estimate: of the ≈5,900 game functions, on the order of 2,000–3,000 need analysis (mostly the game logic module); the rest is replaced without reading the code.

## 5. Build and CI

* A Cargo workspace with profiles: in development, dependencies are built with optimizations and our own code without; dynamic linking of Bevy for fast rebuilds.
* CI on GitHub Actions (`.github/workflows/ci.yml`): a Linux runner on every push and pull request (fmt, clippy, tests without the game files, `cargo deny`); a Windows runner is planned on a schedule and before releases. Standard GitHub-hosted runners are free for a public repository, so CI minutes do not limit the checks; the tests on the game files run locally, because CI has none.
* Render tests in CI run on software Vulkan (lavapipe) or are skipped if there is no GPU.

## 6. What is excluded

| Tool | Why excluded |
|---|---|
| Visual C++ 6.0, decomp.me, reccmp, objdiff | byte-for-byte matching is not a goal |
| IDA Pro, Binary Ninja | paid |
| Wine environments for old compilers | old toolchains are not used |
| Network analyzers (TinCat protocol) | networking is out of scope |

## Open questions

1. GhidraMCP or pyghidra-mcp as the main server: choose at the start of M3 based on convenience.
2. Whether Unicorn stubs work for functions that access the game's global tables (`.data` from the image needs to be mapped).
3. Whether software Vulkan in CI is enough for Bevy screenshot tests.

## Sources

* The compiler, the libraries and the functions shared by the client and the server: docs/01, §3 and §5.
* Rust 1.99.1: the stable channel; Visual Studio 2026 and MSVC 14.50: devblogs.microsoft.com/cppblog; Vulkan SDK 1.4.363.0: lunarg.com.
