# Tooling and the workflow with Claude Code

## Summary

* All tools are free.
  Binary analysis uses **Ghidra 12.1.4** with the MCP server **pyghidra-mcp** for Claude Code (**GhidraMCP 2.0** is the alternative with the Ghidra GUI); IDA Pro and Binary Ninja are excluded.
* The product is built with **Rust 1.99.1** (edition 2024) and **Visual Studio Build Tools 2026** (MSVC 14.50 and the Windows SDK are needed for Rust on Windows).
  VC6, Wine environments and byte-matching tools are not needed.
* For assets: **Blender**, **glTF-Validator**, **KTX-Software**, **FFmpeg**; for render debugging: **RenderDoc** and **Vulkan SDK 1.4.363**; for behavior references: **Unicorn** (emulating the original's functions) and **x64dbg**; optionally **dgVoodoo2** (capturing frames of the original in RenderDoc).
* The process is built around Claude Code: `CLAUDE.md` with rules, MCP to Ghidra, hooks for formatting and tests, small tasks, token usage tracking from day one.
* Quality rests on automated checks: the readers' cross-checks against the Python prototypes, the counts of docs/02 and screenshot tests; converter golden data, oracle vectors and snapshots are planned.
  Maintainer review covers key decisions, not every line.
* The game files are never in the repository: every developer points the tools at their own GOG installation in `OriginalGame/` (§1).

## 1. Tool map

| Stage | Tool | Version | License, price | Purpose |
|---|---|---|---|---|
| Binary analysis | Ghidra | 12.1.4 (JDK 25 LTS, Temurin) | Apache-2.0, free | disassembler and decompiler; RTTI, classes, a shared labeling database |
| | pyghidra-mcp | 0.2.7 (2026-09-25) | Apache-2.0 | the MCP server: Claude Code decompiles, follows cross-references, renames functions and variables, sets types; headless, one HTTP server for every session (§3.2) |
| | GhidraMCP | 2.0 (2026-10-10) | Apache-2.0 | an alternative MCP server, a plugin for the Ghidra GUI |
| | IDA Free | current | free for non-commercial use, cloud x86 decompiler | an optional "second opinion" on hard functions |
| | Python + pefile, capstone, numpy, Pillow, lief | Python 3.15.0 (minimum 3.12); packages in `tools/requirements.txt` | open source | research scripts in `tools/analysis/` |
| Behavior references | Unicorn | 2.x, crate `unicorn-engine` 2.1.5 | GPL-2.0, development only | `opensacred-oracle` (planned, #328), a separate development-only crate run locally on your own `Sacred.exe`: the original's functions in the emulator → vectors for tests |
| | x64dbg | 2026.05.27 | GPL-3.0 | breakpoints in the original on Windows 11 (`x32dbg` for the 32-bit `Sacred.exe`), testing hypotheses |
| | dgVoodoo2 | 2.87.x | freeware | optional: the original runs on Windows 11 without it (docs/09 §2.2); RenderDoc does not capture D3D7, and through dgVoodoo2 (D3D7 → D3D11) a frame of the original can be captured when the render states recovered by `tools/analysis/render/d3d7scan.py` do not answer a question |
| Product build | Rust (rustup, cargo, clippy, rustfmt) | 1.99.1, edition 2024 | MIT or Apache-2.0 | all product code |
| | Visual Studio Build Tools 2026 | MSVC 14.50 + Windows SDK | free for individual development | the linker for Rust on Windows |
| | cargo-nextest, cargo-deny | current | open source | fast tests, dependency license checks |
| Assets | Blender | 5.2 LTS | GPL | inspecting glTF models and animations |
| | Khronos glTF-Validator | 2.0.0-dev.3.10 (latest release, a pre-release) | Apache-2.0 | checking the `.glb` files from the converter |
| | KTX-Software (`ktx`, `toktx`) | current | Apache-2.0 | KTX2, BC7, Basis Universal |
| | FFmpeg | 9.0 | LGPL/GPL | WMV → AV1 + Opus in WebM; the WAV samples of `sound.pak` → FLAC; the VTex textures of GRN files (one-frame Bink 1) → RGBA (invoked by the converter as an external program, `FFMPEG` or `ffmpeg` from `PATH`) |
| | Real-ESRGAN ncnn-vulkan, waifu2x-ncnn-vulkan | current | BSD-3 / MIT | offline upscaling (M7) |
| Rendering | Vulkan SDK (LunarG) | 1.4.363.0 | free | validation layers, vkconfig |
| | RenderDoc | 1.46 | MIT | capturing and analyzing frames of our renderer |
| Process | Claude Code | current | subscription or API | the main executor of tasks |
| | Git, GitHub Actions | current | free (standard runners cost nothing for a public repository) | history, CI |
| | `tools/usage/pr_usage.py` | — | ours, MIT or Apache-2.0 | Claude Code usage per pull request, from the transcripts (§3.5, `docs/usage.md`) |

**The game files.**
The repository holds no file of the game.
Every developer uses their own GOG installation of Sacred Gold 2.28: copied into `OriginalGame/` at the repository root or linked there (ignored by Git either way; the default `--game-dir` of the converter and of the research scripts), or passed with `--game-dir <path>`.
On Windows a link is `New-Item -ItemType SymbolicLink -Path OriginalGame -Target <installation>`, which needs Developer Mode or an elevated shell.
`cargo run -p opensacred-convert -- verify` checks it against the embedded manifest (`docs/manifest-gog-2.28.txt`).
The manifest lists only the files the game never writes (docs/02 §13), so the same installation can be played from without breaking the checks.
Nothing from it, and nothing converted from it, is ever committed (docs/08 §3, §4).

## 2. Workstation

* **OS:** Windows 11 x64 is the main platform for development and running.
  WSL2 with Ubuntu is for builds and tests on Linux.
* **GPU:** with a Vulkan 1.3 or newer driver; a mid-range graphics card for 4K tests.
* **Disk:** ≈3 GB for the GOG installation; ≈5–10 GB for the generated `assets/` (estimate); HD packs separately, up to tens of GB.
* **Setup (Windows):** Rust through rustup; Visual Studio Build Tools 2026 with the "Desktop development with C++" workload and the Windows SDK; Git; your own GOG installation in `OriginalGame/` (§1); FFmpeg, which the converter's `models`, `sound`, `video` and `all` run; glTF-Validator; Python 3.12+ for the research scripts; Ghidra 12.1.4 with JDK 25; uv for pyghidra-mcp (§3.2); x64dbg; Vulkan SDK; Blender.
* **The JDK for Ghidra is 25 LTS (Temurin), not 21.**
  Ghidra 12.1.4 accepts any 64-bit JDK of version 21 or later (`application.java.min=21`, no upper bound), and Ghidra's main branch already requires 25.
  We do not use non-LTS JDKs (26, 27): Ghidra has not been tested on them.
  If `JAVA_HOME` points to another JDK (for example, from Android Studio), the path to Temurin goes into `JAVA_HOME_OVERRIDE` in `support/launch.properties` in the Ghidra directory.
* **Ghidra has no installer:** the release zip, checked against the SHA-256 in the release notes, is unpacked into `%LOCALAPPDATA%\Programs\Ghidra\`, which needs no administrator rights.
  The GUI is then `%LOCALAPPDATA%\Programs\Ghidra\ghidra_12.1.4_PUBLIC\ghidraRun.bat` and the headless analyzer `support\analyzeHeadless.bat` next to it; on 2026-10-18 the headless import and analysis of a 32-bit PE ran on Temurin 25.
  `JAVA_HOME_OVERRIDE` names Temurin's versioned directory (`C:\Program Files\Eclipse Adoptium\jdk-25.0.4.101-hotspot`), which a Temurin update replaces; Ghidra ignores an override that no longer points to a JDK, so it is updated with Temurin.
* **x64dbg comes from winget** (`winget install --id x64dbg.x64dbg --exact`): a portable zip of the GitHub release, which winget checks against the SHA-256 in its manifest and unpacks into `%LOCALAPPDATA%\Microsoft\WinGet\Packages\x64dbg.x64dbg_Microsoft.Winget.Source_8wekyb3d8bbwe\release\`.
  `Sacred.exe` is debugged with `x32\x32dbg.exe` there (`x96dbg.exe` next to `x32\` picks the debugger by the program's bitness); on 2026-10-18 its 32-bit engine (`x32\headless.exe`) started, and `x32dbg.exe` carries a valid signature of the x64dbg author.
  winget reports the command aliases `x32dbg` and `x64dbg` but creates no links for them, so the debugger stays outside `PATH`.
* **Tools outside `PATH`** (this is how their installers set them up): Blender at `C:\Program Files\Blender Foundation\Blender 5.2\blender.exe`, RenderDoc at `C:\Program Files\RenderDoc\`, x64dbg (above).
  The Vulkan SDK's `vulkaninfo` is named `vulkaninfoSDK.exe`; the program of the same name in `PATH` is an old version from the driver.
  glTF-Validator is installed manually from the `KhronosGroup/glTF-Validator` releases into `~/.local/bin`.
* **Not every tool is needed from the start:** Ghidra and the MCP server were set up on 2026-10-18, ahead of M3; KTX-Software comes when the textures move to KTX2 (the texture converter writes PNG), the upscalers with M7.

```powershell
# Toolchain check
rustup update stable
rustc --version            # 1.99.1, the version of rust-toolchain.toml, which rustup installs in the checkout
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
* where to write notes on recovered logic (`docs/re/<subsystem>.md`) and the decision log of deviations from the original (`docs/decisions/`);
* naming conventions: the original class and function names (`cCreature::takeDamage`) are mentioned in comments next to the Rust code.

### 3.2 Ghidra MCP server

Claude Code connects to Ghidra through MCP and gets tools: decompile a function, find cross-references, rename, set a type.
The server is **pyghidra-mcp** (chosen on 2026-10-18), which needs no Ghidra GUI; **GhidraMCP 2.0**, a plugin for the GUI, stays the alternative for a developer who wants to look at Ghidra alongside.

* **One server for every session.**
  pyghidra-mcp runs as a persistent Streamable HTTP server at `http://127.0.0.1:8000/mcp`, not as a stdio server that each Claude Code session would start for itself: a Ghidra project is locked by the process that opens it, and several sessions run at once.
  Claude Code connects to it at the user scope, so the main checkout and every worktree see it; a session sees the server's tools only if it started after the connection was added.
* **The project lies outside the repository**, in `%USERPROFILE%\GhidraProjects\`: `OpenSacred.gpr`, `OpenSacred.rep\` and `OpenSacred-pyghidra-mcp\` (the ChromaDB index of the code and the strings, and a cache of GZF files).
  In a checkout, `git clean -xfd` would delete it with the names and comments it gathers over M3, and `.gitignore` does not cover the `-pyghidra-mcp` directory.
* **The server's working directory** is the project's directory too (`-WorkingDirectory` below), not the shell's current one.
  A process keeps the directory it was started from as its own for as long as it runs, and Windows does not let a process's working directory be deleted.
  On 2026-10-18 a server restarted from a worktree kept `ExitWorktree` and `Remove-Item` from deleting that worktree ("being used by another process") until it was restarted with its own working directory; started from the main checkout, as the first start is, it would pin the checkout the same way.
* **Python.**
  uv runs pyghidra-mcp on Python 3.13, which it downloads itself: `onnxruntime`, which chromadb needs, has wheels up to Python 3.14 and none for the research scripts' 3.15 (on 2026-10-18 uv could not resolve pyghidra-mcp 0.2.7 on 3.15).
* **Java.**
  pyghidra loads the JVM into its Python process and finds the JDK as Ghidra's own launchers do, so `JAVA_HOME_OVERRIDE` (§2) applies: on 2026-10-18 it loaded Temurin 25 although `JAVA_HOME` named Android Studio's runtime.
  `GHIDRA_INSTALL_DIR`, a user environment variable, names the Ghidra directory.
* **The first start** takes the binaries, imports them and names them with a suffix of their hash: `Sacred.exe-d98109`, `GameServer.exe-6233cd`.
  The analysis is Ghidra's default one, in threads, in the background: the server answers while it runs, and `list_project_binaries` shows `analysis_complete`.
  On 2026-10-18 on a Core i9-13900KF the analysis of `Sacred.exe` took 193 s, 107 s of them in the decompiler's switch analysis; the default analysis includes the Windows x86 PE RTTI analyzer (§4.1).
  pyghidra-mcp then decompiles every function into the ChromaDB index of `search_code` (11 minutes for `Sacred.exe`, with no function failing) and loads the strings (23,745 in `Sacred.exe`); the project then takes 385 MB.
  The server's Python takes ≈1.2 GB during the analysis, and the decompiler runs as separate `decompile.exe` processes.
* **Later starts** take no binaries: the server opens the project, skips the analysis and the finished index, and answers in ≈7 s.
  Binaries given again fail to import with `FileInUseException` (they are in the project already), and the server goes on without them.
* **Stopping.**
  Ctrl+C in the server's console saves and closes the project.
  A shell started before `GHIDRA_INSTALL_DIR` was set lacks it, and pyghidra then stops at once ("GHIDRA_INSTALL_DIR is not set"): take it from the user's environment first (`$env:GHIDRA_INSTALL_DIR = [Environment]::GetEnvironmentVariable('GHIDRA_INSTALL_DIR', 'User')`).
  The hidden process is stopped with `taskkill /PID <uvx> /T /F` after the `save` tool has saved the programs' latest edits; on 2026-10-18 the project opened again after two such stops, the `OpenSacred.lock` they leave behind notwithstanding.
* **The tools** (20 in 0.2.7): `decompile_function`, `disassemble`, `list_xrefs`, `gen_callgraph`, `search_code`, `search_strings`, `search_symbols_by_name`, `list_imports`, `list_exports`, `read_bytes`; `rename_function`, `rename_variable`, `set_variable_type`, `set_function_prototype`, `set_comment`, `save`; `import_binary`, `delete_project_binary`, `list_project_binaries`, `list_project_binary_metadata`.
  None of them executes arbitrary code.

```powershell
# Once: uv, the Ghidra directory, the connection
winget install --id astral-sh.uv --exact
[Environment]::SetEnvironmentVariable('GHIDRA_INSTALL_DIR', "$env:LOCALAPPDATA\Programs\Ghidra\ghidra_12.1.4_PUBLIC", 'User')
claude mcp add --transport http --scope user ghidra http://127.0.0.1:8000/mcp

# The server, as a hidden process of its own; started by hand after a reboot
$d = "$env:USERPROFILE\GhidraProjects"; New-Item -ItemType Directory -Force $d | Out-Null
$a = "--python 3.13 pyghidra-mcp@0.2.7 --transport streamable-http --project-path `"$d`" --project-name OpenSacred"
# Only on the first start, from the main checkout (where OriginalGame/ is): the binaries to import
$a += " `"$PWD\OriginalGame\Sacred.exe`" `"$PWD\OriginalGame\GameServer.exe`""
Start-Process uvx -ArgumentList $a -WorkingDirectory $d -WindowStyle Hidden -RedirectStandardOutput "$d\pyghidra-mcp.out.log" -RedirectStandardError "$d\pyghidra-mcp.err.log"
```

MCP tools that execute arbitrary code inside Ghidra (such as running a script) are run only locally.
The Ghidra project, the decompilation and anything exported from it stay on the developer's machine and are never committed: what reaches the repository is a description of the behavior in prose in `docs/re/` and our own code (docs/04 §2, docs/08 §3).

### 3.3 Task types

| Type | Input | Output | Check |
|---|---|---|---|
| Parse a format | a file from `OriginalGame/`, the facts in `docs/02-data-formats.md` | a reader in `opensacred-formats` + a converter in `opensacred-convert` | a test on all files of the installation; cross-check with the Python prototype |
| Recover a rule | the original's function in the local Ghidra project | a description of the behavior in `docs/re/`, oracle vectors, an independent implementation in `opensacred-rules` (planned, #328; until then in `opensacred-game`) | the vectors pass; clippy without warnings |
| Implement a system | notes in `docs/re/`, data from `assets/` | code in `opensacred-game` or `opensacred-rules` (planned, #328) | snapshot and screenshot tests; a manual check in the game |
| Migrate Bevy | the migration guide | an updated `opensacred-game` | everything builds, tests and screenshots match |
| Update dependencies | `cargo update` | a new `Cargo.lock` | CI is green, `cargo deny check` reports no violations |

Size rule: one task is one finished result, verifiable by a test, of 1–4 agent-hours.
Large systems are cut into such tasks in the milestone plan.

Order (since 2026-10-17): the known problems and bugs first, then the dependency upgrades, then new features; the queue is the open issues (§3.6).
Once a day the maintainer's first session starts with `cargo update` in a pull request of its own, and opens a `deps` issue for each newer incompatible release of a direct dependency or of Rust (`CLAUDE.md`, "Work order").
Updating often keeps each step small.
A `Cargo.lock` change does not cost CI a cold build: `Swatinem/rust-cache` falls back to the last cache of `main` and cargo rebuilds only the packages that changed (1.5 minutes on 2026-10-17 for a bump that Linux does not compile, against 15 for a cold build); once a day bounds those rebuilds and the cache entries that each new `Cargo.lock` leaves.

### 3.4 Guardrails

* **Claude Code hooks** (`.claude/settings.json`, script `.claude/hooks/rust_checks.py`): after an edit of a `.rs` file, `cargo fmt --all`; when Claude stops, `cargo clippy --all-targets -- -D warnings` and `cargo nextest run` for the crates with uncommitted changes and the crates that depend on them.
  On failure Claude is sent back to fix it; if the checks still fail after that retry, Claude stops and the developer gets a warning.
  A passing state is remembered in `target/`, so a turn without new changes does not rebuild anything.
  Tests on the game files and `cargo deny check` stay manual.
* **Merge guard** (`.claude/hooks/merge_guard.py`, a `PreToolUse` hook on shell commands): `gh pr merge` is refused when the pull request is labeled `needs-maintainer`, touches the guardrails (`.claude/hooks/`, `.claude/settings.json`, `.claude/settings.local.json`), `.github/`, `deny.toml`, the README of `docs/decisions/` or a screenshot reference, or was opened by someone other than the repository's owner; `--admin` is refused always (§3.6).
  A decision (another file of `docs/decisions/`, added, changed or removed), `CLAUDE.md` or the rest of `.claude/` (the skills) does not stop the merge, but the pull request must carry the label `review-after-merge`.
  Whether the change waits for the maintainer instead (§3.6) the guard cannot judge: Claude Code labels such a pull request `needs-maintainer`, and the guard refuses it by the label.
  Tests: `python -I -m unittest discover -s .claude/hooks -p "test_*.py"`.
* **Converter golden data (planned):** checksums and snapshots (`insta`) for a sample of files; any change in the output shows up in the diff.
  Today the readers are cross-checked against the Python prototypes of `tools/analysis/` and the counts of docs/02, in the tests that need the game files.
* **Oracle vectors (planned, #328):** JSON with the inputs and outputs of the original's functions, recorded by `opensacred-oracle` from the developer's own `Sacred.exe`; the Rust tests read them and, like the other tests that need the game, run locally.
  Neither the crate nor the vectors exist yet.
* **Screenshot tests:** a headless render of a scene through wgpu to PNG and comparison with a reference within a tolerance.
* **Clippy:** the default rule set as errors, plus targeted rules for this project: numeric casts via `as`, `unwrap`/`expect` outside tests, suppressing rules only through `#[expect(…, reason = "…")]`, leftover `dbg!`/`todo!`.
  The pedantic and nursery groups are not enabled wholesale: on the project's code they produce mostly stylistic noise.
  Configuration: `[workspace.lints.clippy]` in `Cargo.toml` and `clippy.toml`.
* **License checks:** our crates are MIT OR Apache-2.0; `cargo deny check licenses` allows MIT, MIT-0, Apache-2.0, BSD-2-Clause, BSD-3-Clause, BSL-1.0 (the Windows backend of Bevy's system clipboard, #189), ISC, Zlib, MPL-2.0 and Unicode-3.0 (Unicode data in `unicode-ident`).
  The future `opensacred-oracle` (#328) links Unicorn (GPL-2.0), which cannot be combined with the Apache-2.0 option: it will be a separate development-only crate, never linked into the converter or the game.
  Configuration: `deny.toml`.
* **Tests that need the game files** are marked and run only locally: CI has no game files.

### 3.5 Parallel work and tracking

* Parallel Claude Code sessions run in separate `git worktree`s, one task per tree.
  A session creates its tree on the issue's branch, `git worktree add .claude/worktrees/<slug> -b <type>/<issue>-<slug> origin/main`, and enters it with `EnterWorktree` and its `path`: Claude Code then records the branch from the first message in the tree (docs/usage.md).
* Usage is recorded per pull request since 2026-10-17 (#16): `tools/usage/pr_usage.py` sums the Claude Code transcripts' messages on the pull request's branch (tokens per model, their cost at API prices, agent-hours) and writes them into its description; the Usage workflow sums the maintainer's pull requests (contributors' are not counted) into the README's badge.
  The method is in `docs/usage.md`, with the sessions recorded before.
  After 30 days the estimates in `docs/07-roadmap-estimates.md` are recomputed from the actuals.

### 3.6 Issues and pull requests

Since 2026-10-17 every change reaches `main` through a pull request; the queue of work is the open issues.

* **Issues.**
  One task, one issue, opened from the form of its type in `.github/ISSUE_TEMPLATE/` (blank issues are off).
  The form sets the type's label; a repository of a personal account has no native issue types, so the types are labels, one per issue: `bug`, `deps`, `feature`, `research`, `infra`, `docs`.
  A milestone's work carries its milestone (M2–M8, PLAN.md §9.2); a large feature is an issue with sub-issues, one pull request per sub-issue.
  GitHub's default labels are removed: "duplicate" and "not planned" are close reasons.
* **Branches.**
  `<type>/<issue>-<slug>` from a fresh `main` (`feature/3-roofs`); without an issue, `<type>/<slug>`.
* **Pull requests.**
  The template (`.github/pull_request_template.md`) asks for `Closes #N`, what and why, the checks run locally (CI cannot run the tests on the game files), the documents updated, and the rules of docs/08 §3.
  The pull request takes its issue's label; its title, which becomes the squash commit's, keeps the history's `Scope: what` style, and the commits' messages become the body.
* **Merging.**
  Squash merge only, the branch deleted on merge.
  Claude Code merges with `gh pr merge --squash --auto`, which waits for the checks.
  The maintainer merges what changes the guardrails (`.claude/hooks/`, `.claude/settings.json`, `.claude/settings.local.json`), `.github/`, the allowed licenses (`deny.toml`), the README of `docs/decisions/` or a screenshot reference, what changes the rules or the process (below; this section among them), a decision that waits (below), and every pull request from a contributor; such a pull request is labeled `needs-maintainer`, and the merge guard (§3.4) refuses to merge it.
  A pull request that takes any other decision (adds, changes or removes a file of `docs/decisions/`), or changes `CLAUDE.md` or the rest of `.claude/` (the skills) only to describe, and nothing else of those does not wait: it is labeled `review-after-merge` and merged, and the maintainer reviews it afterwards and takes the label off, or opens an issue for what to change.
  A change to `CLAUDE.md` or a skill describes when it names the workspace, the commands, the keys, how a skill is used or a fact of the code; it changes the rules, and waits, when it touches the work order, the issues and pull requests, the prohibitions, the rules of the tests and of the code, what a hook or a permission does, or loosens any rule.
  Such a decision refines how the game follows the original, fixes or replaces an earlier decision, or departs where the play hardly changes (an engine limitation, a format's quirk).
  A decision waits when it departs from the original clearly and far, so that it can change the experience of the play seriously (the rules, formulas and balance, the controls or the camera, what the player can do, a quest's course, the game's look as a whole), or when it carries legal consequences (licenses, the use of the game's content or of others' code, what the repository or a release carries; docs/08).
  Claude Code judges which, as it opens the pull request.
  Until 2026-10-18 adding a decision waited too; more than half of the pull requests that waited that day waited for that alone, and the work went on in pull requests stacked on them (#77).
  Until 2026-10-19 changing or removing one waited; seven of the ten pull requests that waited since #77 waited for that alone, refining decisions as the work went on (#163).
  Until 2026-10-19 every change to `CLAUDE.md` or `.claude/` waited too; two of the three pull requests that waited since #164 waited for that alone, naming the game's commands, keys and the skill `game` (#200).
  The maintainer merges only a pull request based on `main`: one merged while its base was another pull request's branch lands in that branch (#41).
* **Stacks.**
  A pull request is based on another only when it cannot be written from `main`, and at most two deep; otherwise Claude Code takes other work from `main`, or waits.
  Once the base is squash-merged, its children still carry its own commits, and where they changed its lines Git sees two versions of them; `tools/stack/stack.py after <N>` merges `main` into each child with the newest of its base's commits that it holds as the merge base, so that the child keeps its changes and gains only what `main` got beyond its base, and pushes the merge.
  When two or more pull requests wait, `tools/stack/stack.py integrate --check` merges them all into a local branch `integration`, a stacked one after its base, with fmt, clippy and the tests: the order of merging and the conflicts are written into the pull requests, and the conflicts resolved on their branches, before the maintainer merges.
* **Versions.**
  Since 2026-10-17 the version follows the merged pull requests.
  On every push to `main`, the Release workflow (`.github/workflows/release.yml`) looks up the pull request of each commit since the last version: one labeled `feature` raises the minor version, one labeled `bug` the patch, the others leave it.
  A version is an annotated tag `vX.Y.Z` on the squash commit and a GitHub Release whose notes list the pull requests since the previous version by label (`.github/release.yml`); before 1.0.0 the releases are pre-releases, as the versions are initial development (SemVer §4).
  The major version is raised only at the maintainer's request, by a manual run on the head of `main` (`gh workflow run release.yml -f bump=major`).
  The base is `v0.1.0`, `main` before the workflow (677863c).
  The workflow tags after the merge, rather than each pull request bumping a version, because parallel pull requests would conflict on it and the ruleset on `main` takes no commit from a workflow.
  So the tags are the only record: `Cargo.toml` has no version (Cargo takes `0.0.0`, and the crates are not published), and the build script `build.rs` of the converter and the game gives their `--version` from `git describe`: `0.4.2` on a tag, `0.4.2-3-gabc1234` three commits after it.
* **Usage.**
  Every pull request's description ends with its Claude Code usage, written by `tools/usage/pr_usage.py` after `gh pr create` and again when the work on it is over (§3.5, `docs/usage.md`).
  Until 2026-10-17 a session's usage went into a row of `docs/usage.md` through a pull request of its own; that cost a CI run and a commit on `main` each time, and a session is not a unit of work.
* **Without an issue:** the daily `Dependencies: cargo update` pull request (`Cargo.lock` alone, label `deps`), auto-merged.
* **Protection.**
  A ruleset on `main` for everyone, the repository's administrators included: changes only through a pull request, with the Linux, Windows and cargo deny checks green; no force pushes, no deletion.
  There is no bypass, so the `cargo update` pull request goes the same way.
  A second ruleset keeps the version tags `v*` from being deleted or moved; creating one stays open, for the Release workflow.
  The branch `badges` is outside both: the Usage workflow replaces its one commit, the README's badge, on every change.

### 3.7 Driving the game

* A game started with `--remote` listens for remote control on `127.0.0.1:15702`, Bevy's Remote Protocol with the game's own methods (docs/10 §8); `tools/game/remote.py` is its client.
* Claude Code starts the game in the background so that it neither covers the maintainer's screen nor touches the maintainer's settings, history and log: windowed, muted, with files of its own, and the window opening without the focus.

  ```bash
  cargo run --release -p opensacred-game -- --remote --windowed --no-music \
    --settings <scratch>/settings.toml --log-file <scratch>/game.log      # from a worktree also --assets ../../../assets
  python -I tools/game/remote.py wait-state menu
  python -I tools/game/remote.py exec "new-game --class mage"
  python -I tools/game/remote.py wait-ready
  python -I tools/game/remote.py exec "go +6,+2"          # or: click X Y, key R, key Escape, type "…"
  python -I tools/game/remote.py screenshot <scratch>/shot.png
  python -I tools/game/remote.py state
  python -I tools/game/remote.py logs --after 0
  python -I tools/game/remote.py exec quit
  ```

* The keys and clicks go to the game through the protocol: posted to the window from outside, a click loses its cursor at once (the system reports the cursor leaving the window it is not over), and moving the system's cursor would take the maintainer's.
* A run with nothing to wait on needs no server: `--exec "new-game --class mage; wait ready; screenshot <scratch>/shot.png; quit"` or `--script FILE` (docs/10 §6).
* Screenshots and logs taken this way show the converted assets: they stay local, never in issues or pull requests (docs/08 §3).

## 4. Binary analysis

### 4.1 The Ghidra project

1. Create a local project and import the GOG `Sacred.exe` (SHA-256 `db9b86a4…71e74`) and `GameServer.exe` from your own installation.
   The GOG build has no DRM; the files are analyzed as they are.
   pyghidra-mcp does this on its first start, with Ghidra's default analysis (§3.2).
2. Recover the classes from RTTI.
   Done 2026-10-19 (#88) with `tools/ghidra/RttiClassesScript.java`: Ghidra's RTTI analyzer, in the default analysis, marked the type descriptors alone (417 in `Sacred.exe`, 259 in `GameServer.exe`), no complete object locator and one vftable, and the RecoverClassesFromRTTI script, which builds on them, recovered no class.
   The script finds the locators itself (signature 0, a type descriptor at `+12`, a hierarchy descriptor at `+16`, in `.rdata`) and each vftable after a pointer to a locator, makes each descriptor's namespace a class, labels its vftables, makes a function at each vftable slot that the analysis left without one, and moves into the class the virtual functions that only its vftables hold and the functions that write only its vftables (its constructors and destructors).
   In `Sacred.exe`: 410 locators, vftables and classes (419 classes with the namespaces that were classes already), 1,194 functions made at the slots (the analysis had not found them: 25,246 functions before, 26,440 after), 1,936 virtual functions and 714 constructors or destructors moved, 2,658 functions in classes; virtual functions held by several classes' vftables stay global.
   In `GameServer.exe`: 253 vftables and classes (261), 971 functions made (9,417 in all), 1,903 functions in classes.
   The classes' data types and their hierarchy are not built, and pyghidra-mcp's index of `search_code` does not take the new functions (it skips a finished collection).
3. Import the community names locally: the sacred-bild function export (GPL-3.0) and the sacred-sdk addresses (MIT).
   The GOG ENG addresses match them.
   The community files themselves are not copied into the repository.
   Done 2026-10-19: `tools/ghidra/community_names.py` merges the named entry points of sacred-bild's `functions_eng.tsv` (193 of its 25,337; the rest are Ghidra's own default and exception-handler names) and the 73 addresses of sacred-sdk's `engine/addresses.h` and `re/reborn_extra_syms.json` (prefixed `sdk_`) into a file outside the repository, which `ImportNamesScript.java` applied to `Sacred.exe`: 137 functions named, 92 that had the name already (54 by Ghidra's analysis, 38 thunks showing their target's), 37 data addresses labeled; every address lay in the image.
4. Mark the library code (CRT, STLport, zlib, D3DX7); it is not analyzed.
   Done 2026-10-19: Function ID's default databases match 54 functions of `Sacred.exe` and 55 of `GameServer.exe`, none of VC6's library; `MarkLibraryScript.java` tags `LIBRARY` the functions of the library block (docs/01 §3.2: 0x7F1180–0x85AB59 but the main module, 0x811000–0x818000), 1,725, `EH` the exception glue 0x85ACD2–0x88F628, 17,422 funclets, and `STL` the functions of the classes under `_STL`, 241 (197 in `GameServer.exe`).
   STLport's template instances in the game's own code have no class of their own and stay untagged.
5. Use `GameServer.exe` as an auxiliary binary: 1,176 game functions in it are byte-identical to the client, which helps confirm function boundaries.
   Done 2026-10-19: `CarryNamesScript.java` matches the two programs' functions by the MD5 of their normalized bytes, as docs/01 §5.1 does, 32 bytes or more and unique on both sides, 1,546 pairs over Ghidra's function boundaries; a function of a default name in the global namespace takes the other's name and namespace: 11 names went into `GameServer.exe` and 2 functions into classes of `Sacred.exe` (each program's classes come from its own RTTI).

The steps run on the project with the server stopped (`save`, then `taskkill`, §3.2), each through Ghidra's headless analyzer with the scripts of `tools/ghidra/` (`ProjectStatsScript.java` counts before and after; tools/README.md has the commands), then the server is started again; a Claude Code session connected before the stop reconnects with `/mcp`.

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

* A Cargo workspace with profiles: in development, dependencies are built with optimizations and our own code without.
  Bevy is linked statically; `rust-lld` (`.cargo/config.toml`) keeps the links fast on Windows.
* CI on GitHub Actions (`.github/workflows/ci.yml`), on every pull request and every push to `main`: a Linux job (fmt, clippy, tests without the game files, the screenshot test), a Windows job (the target platform: the build with `rust-lld` and the tests; no clippy, since the workspace has no Windows-only code, and no screenshot test, since the runner has no GPU) and `cargo deny`.
  The dependencies' build is cached by `Swatinem/rust-cache` and saved only on `main`, which pull requests restore: caches of every branch would push `main`'s out of the repository's 10 GB.
  Standard GitHub-hosted runners are free for a public repository, so CI minutes do not limit the checks; the tests on the game files run locally, because CI has none.
* The Release workflow (`.github/workflows/release.yml`) runs on every push to `main` and versions the merged pull requests (§3.6).
* The screenshot test runs in the Linux job on Mesa's software Vulkan (lavapipe), which is enough for it; the Windows job skips it, as its runner has no GPU.

## 6. What is excluded

| Tool | Why excluded |
|---|---|
| Visual C++ 6.0, decomp.me, reccmp, objdiff | byte-for-byte matching is not a goal |
| IDA Pro, Binary Ninja | paid |
| Wine environments for old compilers | old toolchains are not used |
| Network analyzers (TinCat protocol) | networking is out of scope |

## Open questions

1. Whether Unicorn stubs work for functions that access the game's global tables (`.data` from the image needs to be mapped).

Closed: software Vulkan in CI is enough for the screenshot test, which runs on lavapipe in the Linux job (§5).

## Sources

* The compiler, the libraries and the functions shared by the client and the server: docs/01, §3 and §5.
* Rust 1.99.1: the stable channel; Visual Studio 2026 and MSVC 14.50: devblogs.microsoft.com/cppblog; Vulkan SDK 1.4.363.0: lunarg.com.
