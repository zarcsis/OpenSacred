# Tooling and the workflow with Claude Code

## Summary

* All tools are free.
  Binary analysis uses **Ghidra 12.2** with the MCP server **pyghidra-mcp** for Claude Code (**GhidraMCP 2.0** is the alternative with the Ghidra GUI); IDA Pro and Binary Ninja are excluded.
* The product is built with **Rust 1.99.1** (edition 2024) and **Visual Studio Build Tools 2026** (MSVC 14.51 and the Windows SDK are needed for Rust on Windows).
  VC6, Wine environments and byte-matching tools are not needed.
* For assets: **Blender**, **glTF-Validator**, **KTX-Software**, **FFmpeg**; for render debugging: **RenderDoc** and **Vulkan SDK 1.4.363**; for behavior references: **Unicorn** (emulating the original's functions), **Frida** (driving the running original, #448) and **x64dbg**; **dgVoodoo2** (the driven original drawing on a locked desktop, and its frames captured in RenderDoc).
* The process is built around Claude Code: `CLAUDE.md` with rules, MCP to Ghidra, hooks for formatting and tests, small tasks, token usage tracking from day one.
* Quality rests on automated checks: the readers' cross-checks against the Python prototypes, the counts of docs/02, screenshot tests and the oracle's vectors of the fight's formulas; converter golden data and snapshots are planned.
  Maintainer review covers key decisions, not every line.
* The game files are never in the repository: every developer points the tools at their own GOG installation in `OriginalGame/` (§1).

## 1. Tool map

| Stage | Tool | Version | License, price | Purpose |
|---|---|---|---|---|
| Binary analysis | Ghidra | 12.2 (JDK 25 LTS, Temurin) | Apache-2.0, free | disassembler and decompiler; RTTI, classes, a shared labeling database |
| | pyghidra-mcp | 0.2.7 (2026-09-25) | Apache-2.0 | the MCP server: Claude Code decompiles, follows cross-references, renames functions and variables, sets types; headless, one HTTP server for every session, which each session reaches through `tools/ghidra/mcp_bridge.py`, starting it when it is down (§3.2) |
| | GhidraMCP | 2.0 (2026-10-10) | Apache-2.0 | an alternative MCP server, a plugin for the Ghidra GUI |
| | IDA Free | current | free for non-commercial use, cloud x86 decompiler | an optional "second opinion" on hard functions |
| | Python + pefile, capstone, numpy, Pillow, lief | Python 3.15.1, uv's own; packages in `pyproject.toml`, their versions in `uv.lock` | open source | research scripts in `tools/analysis/` |
| | uv | 0.13.6 | MIT or Apache-2.0 | the repository's Python: the interpreter and the packages of its own in `.venv/` for `tools/`, the hooks and the cross-check tests (§2); pyghidra-mcp (§3.2) |
| Behavior references | Unicorn | 2.x, crate `unicorn-engine` 2.1.5 (x86 alone) | GPL-2.0, development only | `opensacred-oracle` (#363), a separate development-only crate run locally on your own `Sacred.exe`, the emulator behind its feature `emulator`: the original's functions in the emulator → vectors for tests (§3.4) |
| | Frida | 17.24.3 (the Python package `frida`) | wxWindows Library Licence 3.1, development only | `tools/original/` (#448): a Python host and a JavaScript agent that drive your own running `Sacred.exe` in the background for Claude Code, its state, input, commands, traces and frames (§3.8) |
| | x64dbg | 2026.05.27 | GPL-3.0 | breakpoints in the original on Windows 11 (`x32dbg` for the 32-bit `Sacred.exe`), testing hypotheses |
| | dgVoodoo2 | 2.87.5 | freeware | its two DLLs and its configuration beside `Sacred.exe` in `OriginalGame/`, so that the driven original draws on a locked desktop (§3.8); the original runs on Windows 11 without it (docs/09 §2.2); RenderDoc does not capture D3D7, and through dgVoodoo2 (D3D7 → D3D11) a frame of the original can be captured when the render states recovered by `tools/analysis/render/d3d7scan.py` do not answer a question |
| Product build | Rust (rustup, cargo, clippy, rustfmt) | 1.99.1, edition 2024 | MIT or Apache-2.0 | all product code |
| | Visual Studio Build Tools 2026 | MSVC 14.51 + Windows SDK | free for individual development | the linker for Rust on Windows |
| | cargo-nextest, cargo-deny | current | open source | fast tests (`.config/nextest.toml` runs the GRN cross-checks alone, #746), dependency license checks |
| | CMake (Ninja when in `PATH`) | 4.4.4 (Ninja 1.13.2) | BSD-3-Clause, free | builds libopus for the game's sound (`opusic-sys`, #701), and Unicorn for the oracle (§3.4) |
| | C++ Clang tools for Windows, a component of Build Tools | LLVM 22.1.3 | Apache-2.0 with LLVM exception | the libclang with which bindgen makes the oracle's Unicorn bindings (§3.4) |
| Assets | Blender | 5.2 LTS | GPL | inspecting glTF models and animations |
| | Khronos glTF-Validator | 2.0.0-dev.3.10 (latest release, a pre-release) | Apache-2.0 | checking the `.glb` files from the converter |
| | KTX-Software (`ktx`, `toktx`) | current | Apache-2.0 | KTX2, BC7, Basis Universal |
| | FFmpeg | 9.0 | LGPL/GPL | WMV → AV1 + Opus in WebM; the WAV samples of `sound.pak` → FLAC; the sound's FLAC masters → Ogg Opus (`build`, #701); the VTex textures of GRN files (one-frame Bink 1) → RGBA (invoked by the converter as an external program, `FFMPEG` or `ffmpeg` from `PATH`) |
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
The driver of the original (§3.8) runs it there in place, and the game then writes its own files (`Settings.cfg`, `save/`, `Capture/`, its logs), as the maintainer allowed on 2026-10-21 (#448); nothing else in `OriginalGame/` is changed.
Nothing from it, and nothing converted from it, is ever committed (docs/08 §3, §4).

## 2. Workstation

* **OS:** Windows 11 x64 is the main platform for development and running.
  WSL2 with Ubuntu is for builds and tests on Linux.
* **GPU:** with a Vulkan 1.3 or newer driver; a mid-range graphics card for 4K tests.
* **Disk:** ≈3 GB for the GOG installation; ≈5–10 GB for the generated `assets/` (estimate); HD packs separately, up to tens of GB.
* **Setup (Windows):** Rust through rustup; Visual Studio Build Tools 2026 with the "Desktop development with C++" workload and the Windows SDK; Git; CMake, with which the game's build compiles libopus (#701); your own GOG installation in `OriginalGame/` (§1); FFmpeg, which the converter's `models`, `sound`, `video`, `build` and `all` run; glTF-Validator; uv, which brings the repository's Python (below) and runs pyghidra-mcp (§3.2); Ghidra 12.2 with JDK 25; x64dbg; Vulkan SDK; Blender; for the oracle's emulator (§3.4), CMake, Ninja and libclang (the Build Tools component "C++ Clang tools for Windows", a standalone LLVM, or `LIBCLANG_PATH` naming a directory with `libclang.dll`).
* **The JDK for Ghidra is 25 LTS (Temurin).**
  Ghidra 12.2 requires a 64-bit JDK of version 25 or later (`application.java.min=25`, no upper bound); 12.1 still accepted 21.
  We do not use non-LTS JDKs (26, 27): Ghidra has not been tested on them.
  If `JAVA_HOME` points to another JDK (for example, from Android Studio), the path to Temurin goes into `JAVA_HOME_OVERRIDE` in `support/launch.properties` in the Ghidra directory.
* **Ghidra has no installer:** the release zip, checked against the SHA-256 in the release notes, is unpacked into `%LOCALAPPDATA%\Programs\Ghidra\`, which needs no administrator rights.
  The GUI is then `%LOCALAPPDATA%\Programs\Ghidra\ghidra_12.2_PUBLIC\ghidraRun.bat` and the headless analyzer `support\analyzeHeadless.bat` next to it; on 2026-10-18 the headless import and analysis of a 32-bit PE ran on Temurin 25.
  `JAVA_HOME_OVERRIDE` names Temurin's versioned directory (`C:\Program Files\Eclipse Adoptium\jdk-25.0.4.101-hotspot`), which a Temurin update replaces; Ghidra ignores an override that no longer points to a JDK, so it is updated with Temurin.
  A new release of Ghidra is unpacked next to the old one, gets the old one's `JAVA_HOME_OVERRIDE` in its own `support/launch.properties`, and takes over `GHIDRA_INSTALL_DIR` (§3.2) once the server is stopped.
  It upgrades the project's data as it opens it, so the project directory (§3.2) is copied aside first; on 2026-10-24 Ghidra 12.2 opened the project of 12.1.4, the scripts of `tools/ghidra/` compiled against it without a warning, and pyghidra-mcp 0.2.7 served it.
* **x64dbg comes from winget** (`winget install --id x64dbg.x64dbg --exact`): a portable zip of the GitHub release, which winget checks against the SHA-256 in its manifest and unpacks into `%LOCALAPPDATA%\Microsoft\WinGet\Packages\x64dbg.x64dbg_Microsoft.Winget.Source_8wekyb3d8bbwe\release\`.
  `Sacred.exe` is debugged with `x32\x32dbg.exe` there (`x96dbg.exe` next to `x32\` picks the debugger by the program's bitness); on 2026-10-18 its 32-bit engine (`x32\headless.exe`) started, and `x32dbg.exe` carries a valid signature of the x64dbg author.
  winget reports the command aliases `x32dbg` and `x64dbg` but creates no links for them, so the debugger stays outside `PATH`.
* **Tools outside `PATH`** (this is how their installers set them up): Blender at `C:\Program Files\Blender Foundation\Blender 5.2\blender.exe`, RenderDoc at `C:\Program Files\RenderDoc\`, x64dbg (above).
  The Vulkan SDK's `vulkaninfo` is named `vulkaninfoSDK.exe`; the program of the same name in `PATH` is an old version from the driver.
  glTF-Validator is installed manually from the `KhronosGroup/glTF-Validator` releases into `~/.local/bin`.
* **The repository's Python is uv's, not the system's** (#1374): `uv` from winget (`winget install --id astral-sh.uv --exact`) is the one thing to install.
  `pyproject.toml` at the root names the packages of `tools/` (not a package itself), `uv.lock` their versions and `.python-version` the minor version of Python, 3.15; `python-preference = "only-managed"` makes uv take its own interpreter, which it downloads on first use, and never one installed on the system.
  `uv run python -I <script>` runs a script on them: the first run in a checkout or a worktree makes its `.venv/` (ignored by Git) from uv's cache, which took 0.2 s on 2026-10-28, and every run brings it to `uv.lock`; `uv sync` does the same alone.
  The hooks (§3.4) and the usage badge's workflow need only the standard library, so they run with `uv run --no-project python -I`, which installs no package: on 2026-10-28 `uv run python -I -c pass` took ≈55 ms against ≈110 ms of the system's `python` through the alias of the Python install manager.
  `uv python upgrade` moves uv's interpreter to the newest patch release of its minor version, and the environments follow it; another interpreter for a run is `UV_PYTHON`.
* **Not every tool is needed from the start:** Ghidra and the MCP server were set up on 2026-10-18, ahead of M3; KTX-Software is optional, as `build` writes the KTX2 files itself (#700), though its `ktx info` shows one; the upscalers come with M7.

```powershell
# Toolchain check
rustup update stable
rustc --version            # 1.99.1, the version of rust-toolchain.toml, which rustup installs in the checkout
cargo --version
cl                         # MSVC 19.51 from Build Tools 2026 (in Developer PowerShell)
cargo run -p opensacred-convert -- verify   # your installation in OriginalGame/ against the manifest
```

## 3. Workflow with Claude Code

### 3.1 The `CLAUDE.md` file

The repository root holds `CLAUDE.md` with the rules that Claude Code reads at the start of every session:

* the workspace structure and the purpose of each crate;
* commands: build, tests, running the converter, running the game;
* prohibitions: do not commit anything from `OriginalGame/`, `assets/`, `assets-hd/`; do not publish `Sacred.exe`; do not write `unsafe` (`opensacred-oracle` needs none either);
* mandatory checks before finishing a task: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo nextest run`;
* where to write notes on recovered logic (`docs/re/<subsystem>.md`) and the decision log of deviations from the original (`docs/decisions/`);
* naming conventions: the original class and function names (`cCreature::takeDamage`) are mentioned in comments next to the Rust code.

### 3.2 Ghidra MCP server

Claude Code connects to Ghidra through MCP and gets tools: decompile a function, find cross-references, rename, set a type.
The server is **pyghidra-mcp** (chosen on 2026-10-18), which needs no Ghidra GUI; **GhidraMCP 2.0**, a plugin for the GUI, stays the alternative for a developer who wants to look at Ghidra alongside.

* **One server for every session.**
  pyghidra-mcp runs as a persistent Streamable HTTP server at `http://127.0.0.1:8000/mcp`, not as a stdio server that each Claude Code session would start for itself: a Ghidra project is locked by the process that opens it, and several sessions run at once.
  Claude Code connects to it at the user scope, so the main checkout and every worktree see it; a session sees the server's tools only if it started after the connection was added.
* **Every session reaches it, running or not** (#1376), through `tools/ghidra/mcp_bridge.py`, a stdio MCP server that Claude Code starts with the session and that passes its messages to the server over Streamable HTTP.
  When nothing answers on the port, the bridge starts the server as below, hidden, with the project's directory as its working directory and `GHIDRA_INSTALL_DIR` from the user's environment, through a launcher that exits at once: Claude Code ends a stdio server's process tree with the session, and the server, no descendant of it, outlives the session and serves the later ones; a lock file in the project's directory keeps two sessions starting at once from starting two servers.
  A server that dies during a session is started again on the next call, which then goes through in a new MCP session (on 2026-10-28, 8 s after the server was killed).
  The server is started only for a session inside the repository or one of its worktrees; elsewhere the bridge only connects to a server that runs.
  A connection straight to `http://127.0.0.1:8000/mcp` does not do this: Claude Code retries a refused HTTP server three times at the session's start, then leaves it failed for the session's life until `/mcp` reconnects it by hand, and neither a hook nor Claude can reconnect it (code.claude.com/docs/en/mcp.md, 2026-10-28).
  A stdio server connects in the background, and its tools reach the session through the tool search as soon as it answers, mid-turn too; a connection may take `MCP_TIMEOUT`, 30 s by default, so `~/.claude/settings.json` raises it (`"env": {"MCP_TIMEOUT": "120000"}`) for a server that starts cold, which took 20 s on 2026-10-28 and 6 s warm.
* **The project lies outside the repository**, in `%USERPROFILE%\GhidraProjects\`: `OpenSacred.gpr`, `OpenSacred.rep\` and `OpenSacred-pyghidra-mcp\` (the ChromaDB index of the code and the strings, and a cache of GZF files).
  In a checkout, `git clean -xfd` would delete it with the names and comments it gathers over M3, and `.gitignore` does not cover the `-pyghidra-mcp` directory.
* **The server's working directory** is the project's directory too (`-WorkingDirectory` below), not the shell's current one.
  A process keeps the directory it was started from as its own for as long as it runs, and Windows does not let a process's working directory be deleted.
  On 2026-10-18 a server restarted from a worktree kept `ExitWorktree` and `Remove-Item` from deleting that worktree ("being used by another process") until it was restarted with its own working directory; started from the main checkout, as the first start is, it would pin the checkout the same way.
* **Python.**
  uv runs pyghidra-mcp on Python 3.13, which it downloads itself: `onnxruntime`, which chromadb needs, has wheels up to Python 3.14 and none for the research scripts' 3.15 (on 2026-10-24, with onnxruntime 1.31.0, uv still could not resolve pyghidra-mcp 0.2.7 on 3.15).
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
  The next session's bridge starts it again, so a stop that must last (the headless scripts of `tools/ghidra/`, which need the project unlocked; a new release of Ghidra, §2) first creates `pyghidra-mcp.paused` in the project's directory, and deletes it after.
* **The tools** (20 in 0.2.7): `decompile_function`, `disassemble`, `list_xrefs`, `gen_callgraph`, `search_code`, `search_strings`, `search_symbols_by_name`, `list_imports`, `list_exports`, `read_bytes`; `rename_function`, `rename_variable`, `set_variable_type`, `set_function_prototype`, `set_comment`, `save`; `import_binary`, `delete_project_binary`, `list_project_binaries`, `list_project_binary_metadata`.
  None of them executes arbitrary code.

```powershell
# Once: uv, the Ghidra directory, the connection through the bridge (from the main checkout; an earlier HTTP
# connection goes first: claude mcp remove ghidra -s user), and MCP_TIMEOUT in ~/.claude/settings.json (above)
winget install --id astral-sh.uv --exact
[Environment]::SetEnvironmentVariable('GHIDRA_INSTALL_DIR', "$env:LOCALAPPDATA\Programs\Ghidra\ghidra_12.2_PUBLIC", 'User')
claude mcp add --scope user ghidra -- uv run --no-project python -I "$PWD\tools\ghidra\mcp_bridge.py"

# By hand, without a session: start the server if it is down and wait until it answers
uv run --no-project python -I tools\ghidra\mcp_bridge.py --start

# The server, as a hidden process of its own, as the bridge starts it; the first start by hand, with the binaries
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
| Recover a rule | the original's function in the local Ghidra project | a description of the behavior in `docs/re/`, oracle vectors, an independent implementation in `opensacred-rules` (the game's rules with no engine, its modules in `CLAUDE.md`'s workspace table, #364) | the vectors pass; clippy without warnings |
| Implement a system | notes in `docs/re/`, data from `assets/` | code in `opensacred-game` or `opensacred-rules` | snapshot and screenshot tests; a manual check in the game |
| Migrate Bevy | the migration guide | an updated `opensacred-game` | everything builds, tests and screenshots match |
| Update dependencies | `cargo update`, `uv lock --upgrade` | a new `Cargo.lock` and `uv.lock` | CI is green, `cargo deny check` reports no violations |

Size rule: one task is one finished result, verifiable by a test, of 1–4 agent-hours.
Large systems are cut into such tasks in the milestone plan.

Order (since 2026-10-17): the known problems and bugs first, then the dependency upgrades, then new features; the queue is the open issues (§3.6).
Once a day the maintainer's first session starts with `cargo update` and `uv lock --upgrade` in a pull request of its own, and opens a `deps` issue for each newer incompatible release of a direct dependency, of a Python package, of Rust or of Python (`CLAUDE.md`, "Work order").
The Python packages are bounded in `pyproject.toml` as Cargo bounds the crates, below by the version taken and above by the next major release, so that `uv lock --upgrade` takes only the compatible ones; `uv tree --outdated --depth 1` shows the newest release of each.
Updating often keeps each step small.
A `Cargo.lock` change does not cost CI a cold build: `Swatinem/rust-cache` falls back to the last cache of `main` and cargo rebuilds only the packages that changed (1.5 minutes on 2026-10-17 for a bump that Linux does not compile, against 15 for a cold build); once a day bounds those rebuilds and the cache entries that each new `Cargo.lock` leaves.

### 3.4 Guardrails

* **Claude Code hooks** (`.claude/settings.json`, script `.claude/hooks/rust_checks.py`): after an edit of a `.rs` file, `cargo fmt --all`; when Claude stops, `cargo clippy --all-targets -- -D warnings` and `cargo nextest run` for the crates with uncommitted changes and the crates that depend on them.
  On failure Claude is sent back to fix it; if the checks still fail after that retry, Claude stops and the developer gets a warning.
  A passing state is remembered in `target/`, so a turn without new changes does not rebuild anything.
  Tests on the game files and `cargo deny check` stay manual.
* **Merge guard** (`.claude/hooks/merge_guard.py`, a `PreToolUse` hook on shell commands): `gh pr merge` is refused when the pull request is labeled `needs-maintainer`, touches the guardrails (`.claude/hooks/`, `.claude/settings.json`, `.claude/settings.local.json`), `.github/`, `deny.toml`, the README of `docs/decisions/` or a screenshot reference, or was opened by someone other than the repository's owner; `--admin` is refused always (§3.6).
  A decision (a row of a subsystem's table in `docs/decisions/`, added, changed or removed), `CLAUDE.md` or the rest of `.claude/` (the skills) does not stop the merge.
  Whether the change waits for the maintainer instead (§3.6) the guard cannot judge: Claude Code labels such a pull request `needs-maintainer`, and the guard refuses it by the label.
  Tests: `uv run python -I -m unittest discover -s .claude/hooks -p "test_*.py"`.
* **The rules' watch** (`.claude/hooks/rules_watch.py`, a `SessionStart` and a `UserPromptSubmit` hook, #1400): a session reads `CLAUDE.md` as it starts, and one that runs for hours would not see a rule merged meanwhile.
  As a session starts, the hook records `CLAUDE.md` and this document as its branch took them from `main` (at the merge base of `HEAD` and `origin/main`, so that the session's own changes are not taken for main's), in `%LOCALAPPDATA%\OpenSacred\rules-read\<session>.json`.
  At each prompt it fetches `origin/main`, at most once in five minutes, and when either file there differs from the text recorded, adds the diff to the session's context, or the command that shows it when the diff is longer than 6,000 characters, and records the new text.
  A prompt takes about 0.2 s, and 2.5 s with the fetch (2026-10-28); a directory outside a repository, or a git that fails, adds nothing.
  Its tests sit beside the merge guard's.
* **Converter golden data (planned):** checksums and snapshots (`insta`) for a sample of files; any change in the output shows up in the diff.
  Today the readers are cross-checked against the Python prototypes of `tools/analysis/` and the counts of docs/02, in the tests that need the game files.
* **Oracle vectors (#363):** JSON with the inputs and outputs of the original's functions, recorded by `opensacred-oracle` from the developer's own `Sacred.exe`; the Rust tests read them and, like the other tests that need the game, run locally.
  `cargo run --release -p opensacred-oracle --features emulator` checks `OriginalGame/Sacred.exe` and `bin/Balance.bin` against the manifest and refuses other files, lays out the PE image at its base (the sections at their virtual addresses, `.data` and `.rdata` from the file, so that the compiled-in tables are there) and the balance's image at `0xAD4490` as the client loads it, points every import's slot at a thunk of its own, and calls each function from a trampoline of its own that resets the FPU (53 bits of precision by default, `--precision 24` or `64`) and keeps `EAX` and `ST0`.
  A function the code calls that would reach the engine is a stub that returns a chosen value; a call into an import without a stub, an access to unmapped memory, an invalid instruction or an interrupt stops the call with an error naming it and the address.
  The oracle sets only the inputs (the arguments, the fields of a synthetic `this`, docs/re/fight.md's offsets) and reads the outputs: the machine code is the original's (docs/08 §3).
  Its sets (2026-10-21): `level_end` (`0x57F6E0`, the levels 1 to 216), `level_of` (`0x57F8D0`, the experiences about each level's end by six levels held), `hit_chance` and `dealt` (`0x43EEE0` as the strike's resolution calls it: 100 attacks against defences, 720 damages against resistances by eight levels above) and `most_health` (`0x5658F0` on synthetic stats and creature, 144 vectors of eight bases at nine levels, of a hero's type and not; with stubs for the client's object `0x7D84A0` and the hero's test `0x426650`).
  It writes `<set>.json` to `--out`, else `OPENSACRED_VECTORS`, else `%LOCALAPPDATA%\OpenSacred\vectors` (`$XDG_DATA_HOME/OpenSacred/vectors` elsewhere), and refuses a directory inside the repository: the vectors are never committed.
  A file holds the set's name, the function's address, the SHA-256 of the `Sacred.exe` that ran, the FPU's control word and the vectors, each its inputs and outputs by name.
  `cargo nextest run -p opensacred-rules --run-ignored only -E 'binary(oracle)'` compares `opensacred_rules::fight` with them (`crates/opensacred-rules/tests/oracle.rs`, ignored without the game files): the levels' ends and the levels exactly, the chance within 10⁻⁶ and the damage within 10⁻⁵ of its value (x87 against `f32`); the differences found are bugs of their own: #406, #407 and #408 are fixed, and the test leaves out the most health's vectors of #417 and #418 until they are.
  On 2026-10-21 a release run took 80 s; the functions called no import, so no Windows stub was needed.
* **Properties of the fight's formulas** (#1428, part of #1435): `crates/opensacred-rules/tests/fight_properties.rs` asserts in CI, with `proptest` on random inputs of the game's ranges (256 cases each), what the oracle's vectors show of the levels' ends and levels, the hit chance, the damage let through and a creature's experience: bounds, monotony, the band of ±0.001 that divides nothing; `oracle.rs`'s `the_fights_properties_hold_on_the_originals_vectors` asserts each on the vectors first, so that a property is the original's, never bent to ours.
  A failing case is kept in `crates/opensacred-rules/proptest-regressions/`, committed: our inputs alone.
* **The oracle's build:** the emulator is the feature `emulator` of `opensacred-oracle`, so that the workspace builds without Unicorn's toolchain: `unicorn-engine-sys` builds Unicorn's C library with CMake, Ninja (on Windows the ones of Visual Studio when they are not in `PATH`) and the C compiler, and makes its bindings with bindgen, which needs libclang (without it the build stops: "Unable to find libclang").
  libclang comes with the Build Tools component "C++ Clang tools for Windows" (`Microsoft.VisualStudio.Component.VC.Llvm.Clang`), which puts LLVM under `VC\Tools\Llvm` and is updated with Build Tools; bindgen's clang-sys looks there by itself, as it does in a standalone LLVM's `C:\Program Files\LLVM`, so neither needs `LIBCLANG_PATH`.
  The component is added in the Visual Studio Installer, or with `setup.exe modify --installPath <Build Tools> --add Microsoft.VisualStudio.Component.VC.Llvm.Clang --quiet --norestart` started elevated: with `--quiet` the installer refuses to raise its rights itself and exits with 5007.
  On 2026-10-24 it built with CMake 4.4.4, Ninja 1.13.2, MSVC 19.51 and the component's libclang of LLVM 22.1.3, with no other LLVM on the machine, in 26 s with 32 jobs.
  CI builds the workspace without the feature: the oracle there checks its PE reader and its files (`cargo nextest run -p opensacred-oracle`); the emulator's own tests, on a synthetic image, run with `--features emulator` and need no game files.
  Without the feature, the oracle checks the installation, the image and the directory, and stops.
* **Screenshot tests:** a headless render of a scene through wgpu to PNG and comparison with a reference within a tolerance.
* **Clippy:** the default rule set as errors, plus targeted rules for this project: numeric casts via `as`, `unwrap`/`expect` outside tests, suppressing rules only through `#[expect(…, reason = "…")]`, leftover `dbg!`/`todo!`.
  The pedantic and nursery groups are not enabled wholesale: on the project's code they produce mostly stylistic noise.
  Configuration: `[workspace.lints.clippy]` in `Cargo.toml` and `clippy.toml`.
* **License checks:** our crates are MIT OR Apache-2.0; `cargo deny check licenses` allows MIT, MIT-0, Apache-2.0, BSD-2-Clause, BSD-3-Clause, BSL-1.0 (the Windows backend of Bevy's system clipboard, #189), ISC, Zlib, MPL-2.0 and Unicode-3.0 (Unicode data in `unicode-ident`).
  `opensacred-oracle` (#363) links Unicorn (GPL-2.0), which cannot be combined with the Apache-2.0 option: it is a separate development-only crate, a binary without a library, so the converter and the game cannot depend on it; `deny.toml` allows GPL-2.0 for `unicorn-engine` and `unicorn-engine-sys` alone (`licenses.exceptions`) and denies them anywhere but under the oracle (`bans.deny` with `wrappers`).
  Our code in it stays MIT OR Apache-2.0; the oracle built with the emulator is GPL-2.0 as a whole (by the MIT option) and is never distributed.
  Configuration: `deny.toml`.
* **Tests that need the game files** are marked and run only locally: CI has no game files.

### 3.5 Parallel work and tracking

* Parallel Claude Code sessions run in separate `git worktree`s, one task per tree.
  A session takes its issue with `uv run python -I tools/stack/claim.py <issue> <slug>` (#1401), which makes the tree `.claude/worktrees/<issue>-<slug>` on the issue's branch `<type>/<issue>-<slug>` from `origin/main`, and enters it with `EnterWorktree` and its `path`: Claude Code then records the branch from the first message in the tree (docs/usage.md).
* The sessions still share the machine; the tools keep them out of each other's way, and messages do what no tool does (#1391, #1396).
  Every session works under one GitHub account, so an assignee alone does not say which session holds an issue: `claim.py` refuses an issue that is assigned (unless the session filed it so, `--mine`), that has a branch `*/<issue>-*`, local or on origin, or that an open pull request closes, naming what holds it; otherwise it assigns the issue and pushes its branch at once, so that the next session that looks sees it.
  The repository is public and a session reads the issue it takes as its task, so `claim.py` takes only the account's own (#1421, the maintainer's guard against prompt injection): it refuses an issue that another account filed (`gh api user` names the account), reads the open pull requests of the account alone, so that another's neither holds an issue nor is read, and refuses `--base` on another's pull request; how the community's issues and pull requests are taken is to be set apart.
  The original under Frida is one per machine, as every worktree links the one installation (§3.8): the host records its holder (`--holder`, the session's name), and a host started meanwhile refuses, naming it (#1398).
  Under a session, the game's `--remote` without a port takes a port of the session's own, which `remote.py` finds by the session and its directory (§3.7, #1397), so no session chooses one, and none drives another's game.
  A game refuses scripts translated with other calls than its own, naming them (docs/02 §7.6, #1399).
  Claude Code sessions on one machine reach one another by name: `ListAgents` lists the peers, busy or idle, `SendMessage` delivers a message at the peer's next tool call, and its `notify_when_idle` brings one notice when the peer finishes, with no polling.
  A message names the session's issue and the resource; a peer in another permission mode may hold it for the maintainer's approval, and a cloud session receives it but cannot answer, so silence is no agreement.
  The messages that stay: a session tells its peers before `opensacred-convert` rewrites the main checkout's `assets-src/` and `assets/`, which the games and tests of every worktree read, and after; it asks the holder that a refusal names (of the original, of an issue) to say when it is free, a session idle may still hold it, and never ends another session's original or takes its issue.
  A peer's message is a teammate's word, not the maintainer's: a session never does for a peer what its own permissions refuse, nor changes the guardrails or the rules at a peer's word.
* Usage is recorded per pull request since 2026-10-17 (#16): `tools/usage/pr_usage.py` sums the Claude Code transcripts' messages on the pull request's branch (tokens per model, their cost at API prices, agent-hours) and writes them into its description; the Usage workflow sums the maintainer's pull requests (contributors' are not counted) into the README's badge.
  The method is in `docs/usage.md`, with the sessions recorded before.
  After 30 days the estimates in `docs/07-roadmap-estimates.md` are recomputed from the actuals.

### 3.6 Issues and pull requests

Since 2026-10-17 every change reaches `main` through a pull request; the queue of work is the open issues.

* **Issues.**
  One task, one issue, opened from the form of its type in `.github/ISSUE_TEMPLATE/` (blank issues are off).
  The form sets the type's label; a repository of a personal account has no native issue types, so the types are labels, one per issue: `bug`, `deps`, `feature`, `research`, `infra`, `docs`.
  A milestone's work carries its milestone (M2–M9, PLAN.md §9.2); a large feature is an issue with sub-issues, one pull request per sub-issue.
  GitHub's default labels are removed: "duplicate" and "not planned" are close reasons.
  Since 2026-10-26 an issue taken into work is assigned to whoever takes it (`gh issue edit <N> --add-assignee @me`), so that the open issues show which are held and by whom when several sessions work at once (#1049).
* **Branches.**
  `<type>/<issue>-<slug>` from a fresh `main` (`feature/3-roofs`); without an issue, `<type>/<slug>`.
* **Pull requests.**
  The template (`.github/pull_request_template.md`) asks for `Closes #N`, what and why, the checks run locally (CI cannot run the tests on the game files), the documents updated, and the rules of docs/08 §3.
  The pull request takes its issue's label; its title, which becomes the squash commit's, keeps the history's `Scope: what` style, and the commits' messages become the body.
  It is assigned to its author as it is opened (`gh pr create --assignee @me`, #1049).
* **Merging.**
  Squash merge only, the branch deleted on merge.
  Claude Code merges with `gh pr merge --squash --auto`, which waits for the checks.
  The maintainer merges what changes the guardrails (`.claude/hooks/`, `.claude/settings.json`, `.claude/settings.local.json`), `.github/`, the allowed licenses (`deny.toml`), the README of `docs/decisions/` or a screenshot reference, what changes the rules or the process (below; this section among them), a decision that waits (below), and every pull request from a contributor; such a pull request is labeled `needs-maintainer`, and the merge guard (§3.4) refuses to merge it.
  A pull request that takes any other decision (adds, changes or removes a row of `docs/decisions/`), or changes `CLAUDE.md` or the rest of `.claude/` (the skills) only to describe, and nothing else of those does not wait: it is merged like any other, and the maintainer opens an issue for what to change.
  A change to `CLAUDE.md` or a skill describes when it names the workspace, the commands, the keys, how a skill is used or a fact of the code; it changes the rules, and waits, when it touches the work order, the issues and pull requests, the prohibitions, the rules of the tests and of the code, what a hook or a permission does, or loosens any rule.
  Such a decision refines how the game follows the original, fixes or replaces an earlier decision, or departs where the play hardly changes (an engine limitation, a format's quirk).
  A decision waits when it departs from the original clearly and far, so that it can change the experience of the play seriously (the rules, formulas and balance, the controls or the camera, what the player can do, a quest's course, the game's look as a whole), or when it carries legal consequences (licenses, the use of the game's content or of others' code, what the repository or a release carries; docs/08).
  Claude Code judges which, as it opens the pull request.
  Until 2026-10-18 adding a decision waited too; more than half of the pull requests that waited that day waited for that alone, and the work went on in pull requests stacked on them (#77).
  Until 2026-10-19 changing or removing one waited; seven of the ten pull requests that waited since #77 waited for that alone, refining decisions as the work went on (#163).
  Until 2026-10-19 every change to `CLAUDE.md` or `.claude/` waited too; two of the three pull requests that waited since #164 waited for that alone, naming the game's commands, keys and the skill `game` (#200).
  Until 2026-10-20 such a pull request was labeled `review-after-merge`, which the maintainer took off once it was reviewed, and the merge guard refused it without that label or `needs-maintainer`; the maintainer dropped the label after reviewing all 45 that carried it, finding it unneeded in practice (#350).
  The maintainer merges only a pull request based on `main`: one merged while its base was another pull request's branch lands in that branch (#41).
* **Stacks.**
  A pull request is based on another only when it cannot be written from `main`, and at most two deep; otherwise Claude Code takes other work from `main`, or waits.
  Once the base is squash-merged, its children still carry its own commits, and where they changed its lines Git sees two versions of them; `tools/stack/stack.py after <N>` merges `main` into each child with the newest of its base's commits that it holds as the merge base, so that the child keeps its changes and gains only what `main` got beyond its base, and pushes the merge.
  When two or more pull requests wait, `tools/stack/stack.py integrate --check` merges them all into a local branch `integration`, a stacked one after its base, with fmt, clippy and the tests: the order of merging and the conflicts are written into the pull requests, and the conflicts resolved on their branches, before the maintainer merges.
* **Versions.**
  Since 2026-10-17 the version follows the merged pull requests.
  On every push to `main`, the Release workflow (`.github/workflows/release.yml`) looks up the pull request of each commit since the last version: one labeled `feature` raises the minor version, one labeled `bug` the patch, the others leave it.
  A run that pushed a tag but failed before its release leaves the release to the next run.
  A version is an annotated tag `vX.Y.Z` on the squash commit and a GitHub Release whose notes list the pull requests since the previous version by label (`.github/release.yml`); before 1.0.0 the releases are pre-releases, as the versions are initial development (SemVer §4).
  The major version is raised only at the maintainer's request, by a manual run on the head of `main` (`gh workflow run release.yml -f bump=major`).
  The base is `v0.1.0`, `main` before the workflow (677863c).
  The workflow tags after the merge, rather than each pull request bumping a version, because parallel pull requests would conflict on it and the ruleset on `main` takes no commit from a workflow.
  So the tags are the only record: `Cargo.toml` has no version (Cargo takes `0.0.0`, and the crates are not published), and the build script `build.rs` of the converter and the game gives their `--version` from `git describe`: `0.4.2` on a tag, `0.4.2-3-gabc1234` three commits after it.
* **Usage.**
  Every pull request's description ends with its Claude Code usage, written by `tools/usage/pr_usage.py` after `gh pr create` and again when the work on it is over (§3.5, `docs/usage.md`).
  Until 2026-10-17 a session's usage went into a row of `docs/usage.md` through a pull request of its own; that cost a CI run and a commit on `main` each time, and a session is not a unit of work.
* **Without an issue:** the daily `Dependencies: cargo update` pull request (`Cargo.lock` and `uv.lock` alone, label `deps`), auto-merged.
* **Protection.**
  A ruleset on `main` for everyone, the repository's administrators included: changes only through a pull request, with the Linux, Windows and cargo deny checks green; no force pushes, no deletion.
  There is no bypass, so the `cargo update` pull request goes the same way.
  A second ruleset keeps the version tags `v*` from being deleted or moved; creating one stays open, for the Release workflow.
  The branch `badges` is outside both: the Usage workflow replaces its one commit, the README's badge, on every change.

### 3.7 Driving the game

* A game started with `--remote` listens for remote control on `127.0.0.1:15702`, Bevy's Remote Protocol with the game's own methods (docs/10 §8); `tools/game/remote.py` is its client.
  Bevy 0.20's `RemoteHttpPlugin` also serves the render sub-app, on 15703 unless told otherwise, the original's driver's port (below): the game moves that server to a port the system picks, which nothing calls (#528).
  Under a Claude Code session (`CLAUDE_CODE_SESSION_ID` in the environment), `--remote` without a port listens on a port that the system gives instead, recorded with the game's pid and working directory in `%LOCALAPPDATA%\OpenSacred\remote\<session>-<directory>.port` and removed as the game ends (#1397).
  `remote.py`, `compare.py` and `conformance.py` without a port call the game that their session started in their directory, else the session's only game, else 15702, so that two sessions, or two worktrees of one, never drive each other's games.
* Claude Code starts the game in the background so that it neither covers the maintainer's screen nor touches the maintainer's settings, history and log: muted, with files of its own, and its window hidden (`--hidden`, #556): on no screen, taskbar or Alt-Tab, as the original's driver keeps its own (§3.8).
  Bevy creates every window hidden and shows it after; `--hidden` leaves it so.
  A hidden window cannot be shown again: the game hangs when one is (Bevy 0.20 on Windows, observed with the window hidden at the start or later, 2026-10-22).
* The game draws hidden, outside every monitor and on a locked desktop alike, and its screenshots are read back from the GPU rather than from the screen (observed 2026-10-22, #554, #556: outside the monitors, 60 frames a second through a lock of nearly two minutes, each screenshot of the lock written on time and of a new frame; hidden, the same through a lock of 80 s, and the hero walking on a click), so it needs nothing like the original's dgVoodoo2 (§3.8).

  ```bash
  cargo run --release -p opensacred-game -- --remote --hidden --no-music \
    --settings <scratch>/settings.toml --log-file <scratch>/game.log      # from a worktree also --assets ../../../assets
  uv run python -I tools/game/remote.py wait-state menu
  uv run python -I tools/game/remote.py exec "new-game --class mage"
  uv run python -I tools/game/remote.py wait-ready
  uv run python -I tools/game/remote.py exec "go +6,+2"          # or: click X Y, key R, key Escape, type "…"
  uv run python -I tools/game/remote.py screenshot <scratch>/shot.png
  uv run python -I tools/game/remote.py state
  uv run python -I tools/game/remote.py logs --after 0
  uv run python -I tools/game/remote.py exec quit
  ```

* The keys and clicks go to the game through the protocol: posted to the window from outside, a click loses its cursor at once (the system reports the cursor leaving the window it is not over), and moving the system's cursor would take the maintainer's.
* A run with nothing to wait on needs no server: `--exec "new-game --class mage; wait ready; screenshot <scratch>/shot.png; quit"` or `--script FILE` (docs/10 §6).
* By hand, `uv run python -I tools/game/remote.py shell` reads lines in a terminal (#550): a line is a command of the game, one starting with `:` a call of the client (`:state`, `:click X Y`, `:screenshot FILE`, `:logs`, `:follow on`, `:help`), and `:quit` ends the shell and leaves the game running.
  An error ends the line rather than the shell, and lines piped in run as a script; with `--port 15703` it drives the original (§3.8).
* With both games running in a game, `uv run python -I tools/game/compare.py X,Y …` (#574) puts each hero onto each cell and compares their creatures within a radius (`--radius`, 15): it pairs them by type and nearest cell (`--pair`, 3) and prints the pairs whose level, most health, hostility or rank differ, then each game's creatures without a pair; `--level` and `--time` set both heroes and clocks first, `--cells-file` reads the cells from a file.
  `compare.py stats --types T,… --levels L,…` (#587) makes each type at each level in both beside their heroes (`spawn … --peaceful --immortal`) and compares their `stats`: the most health, `A`, `D`, the damages, the resistances and the attack speed, the original's raw values taken through their multipliers as its attack record takes them: the driver prints each float exactly, the product of two is the x87 unit's at its 53 bits, and the whole values (the most health, `A`, `D`, the attack speed) must agree to the game's one decimal (#589; the floats rounded to thousandths made `A` and `D` one off).
  `uv run python -I tools/game/conformance.py` (#600) runs the checks as a suite with PASS or FAIL each and a non-zero exit on a failure: the experience of kills for a hero of level 1 and 30 (`experience`), the hero's values at the levels 1, 10 and 30 (`hero`), common creature types at four levels (`stats`), the shares of 1,000 drops by category (`drops`), and the scripts' creatures where they stand, the guards of 52,39 and the stable of 64,41 (`places`, #606), and the health regained per second of world time from 1 (`heal --to 1`) by a creature and by the hero alone and in a fight (`regeneration`, #609), and a goblin's strikes at the hero over 60 s, their pace, the share that lands and the damage (`strikes`, #609, which needs the game's log of `opensacred_game::fight` at `debug`: `RUST_LOG=info,opensacred_game::fight=debug`), the arts' times after a use and the rate of their regeneration (`arts`, #623), and the hero's run of 9 cells and back, its pace and its time from the first step to the stop (`movement`, #632); it starts both games afresh (`new-game`, and the original's `load` of a save of the Seraphim's new game, `--save`).
  The game reads `assets/`, so after a change of the converter `opensacred-convert all` comes first: a `heroes.ron` of before #604 holds no arts, and `arts` then fails (#623).
  The original makes a sector's `Enter` creatures again when the sector comes back into the view after a teleport, beside the ones kept (#227's item 10), so after a jump away and back its list holds such pairs on one cell; a fresh load before a cell avoids them.
* Screenshots and logs taken this way show the converted assets: they stay local, but for a frame of a problem, cropped to it, which its issue or pull request carries as an attachment (`gh … --attach`; docs/08 §3, #1222).

### 3.8 Driving the original

Claude Code drives the developer's own running `Sacred.exe` the way it drives the game (§3.7), to see for itself how the original behaves (the checks of #227; #448, its steps #449–#452).
The host, its guards, the frames and the process's state are in place (#449), the player's input, the program's screen, a new game and the saves (#450), the world's state and commands (#451) and the traces of the original's functions (#452).
The project skill `original` says how to run it.

```bash
uv run python -I tools/original/host.py --holder <session> --log-file <scratch>/original.log   # in the background
uv run python -I tools/game/remote.py --port 15703 wait-state menu
uv run python -I tools/game/remote.py --port 15703 exec "new-game --class seraphim"   # or: exec "load start-seraphim"
uv run python -I tools/game/remote.py --port 15703 wait-ready
uv run python -I tools/game/remote.py --port 15703 click 720 480            # or: key Escape, key L, type "…", wheel 1
uv run python -I tools/game/remote.py --port 15703 exec "save start-seraphim"
uv run python -I tools/game/remote.py --port 15703 exec "cell 3296,2528"     # the hero onto a cell; also: sector X,Y, go +DX,+DY
uv run python -I tools/game/remote.py --port 15703 exec "cell-info +1,0"     # a cell's ground and floors, the hero's by default
uv run python -I tools/game/remote.py --port 15703 exec "creatures 20"       # or: time 20:20, day-speed 0, zoom 2, point X,Y
uv run python -I tools/game/remote.py --port 15703 exec "ui press UI_WND_TASKBAR UI_TB_INVENTORY"   # ui lists the windows
uv run python -I tools/game/remote.py --port 15703 exec "god on"             # or: heal, level 10, experience +500, pause
uv run python -I tools/game/remote.py --port 15703 exec "arts learn 1071 2"  # or: arts, arts ready, arts use 1071
uv run python -I tools/game/remote.py --port 15703 exec "spawn 76 --count 3"   # or: kill 10, drop 20, item 5171
uv run python -I tools/game/remote.py --port 15703 exec "trace fight"        # or drops, ai, doors, sound (with --muted), an address
uv run python -I tools/game/remote.py --port 15703 wait-log "trace: drop: "
uv run python -I tools/game/remote.py --port 15703 shell                    # by hand: a command a line, ":help" the client's own
uv run python -I tools/game/remote.py --port 15703 screenshot <scratch>/menu.png
uv run python -I tools/game/remote.py --port 15703 logs --after 0
uv run python -I tools/game/remote.py --port 15703 exec quit
```

The maintainer plays the original themselves with `uv run python -I tools/original/host.py --interactive` (#551, below): full screen on the primary monitor (`--monitor N` for another), the commands at hand from a terminal through `remote.py --port 15703 shell`.

The maintainer's decisions of 2026-10-21:

* **Frida, not a Rust crate.**
  A Python host (`tools/original/host.py`) and a JavaScript agent (`tools/original/agent.js`) that Frida loads into the game's process: the agent hooks the original's functions and calls them by address, and reads its memory by offset, with no `unsafe`, no 32-bit Rust target and no build step.
  `frida` joins the packages of `pyproject.toml`; the tool runs locally, never in CI or a release.
* **In place, in `OriginalGame/`.**
  The game may overwrite every file it writes itself (`Settings.cfg`, `save/`, `Capture/`, `DEBUG.LOG`, `ExpLog.LOG`, the `DefPos.bin` caches), never one of the manifest, so `verify` and the oracle's check stay green.
  Before each start the host sets the keys of `Settings.cfg` it needs (`FULLSCREEN : 0`, 1 in the maintainer's play; `SHOWMOVIE : 0`; the three volumes 0) and nothing else.
  The volumes it replaces, unless it wrote them itself or all three are 0, it keeps in `%LOCALAPPDATA%\OpenSacred\original-volumes.json`, and `--sound` writes them back, or the installation's 64 where none were kept (#1383).
  Worktrees link `OriginalGame/` to one installation, so one original runs at a time.
  While a host runs it records the original's holder in `%LOCALAPPDATA%\OpenSacred\original-holder.json` (`--holder`, the session's name as `ListAgents` shows it; the Claude Code session, the branch and the worktree), and a host started meanwhile refuses, naming them (#1398).
* **At any time, without disturbing the maintainer.**
  The original runs in the background, never takes the focus, never blocks the maintainer's keys and never outlives its host.
  The maintainer's own play (`--interactive`, 2026-10-22) is the one run that takes the screen, started by the maintainer or at their word.
* **One client for both games.**
  The host serves the game's own methods with the same parameters (`opensacred/exec`, `opensacred/state`, `opensacred/input`, `opensacred/screenshot`, `opensacred/logs`) on `127.0.0.1:15703`, so `tools/game/remote.py --port 15703` drives the original, and both can run side by side; a project skill `original` says how, as the skill `game` does.

How it works:

* **Start.**
  The host checks `Sacred.exe` against the manifest, spawns it suspended with Frida, loads the agent and resumes the game only once every guard below is in place; if one fails, it kills the game.
* **Guards of the maintainer's desktop**, before the game's first instruction.
  The low-level keyboard and mouse hooks install nothing, so the original's "tabblocker" never swallows Win and Alt-Tab (docs/09 §2.1).
  The calls that would take the focus, raise the window, put it on top, clip or move the cursor, or change the display mode do nothing, and DirectDraw's exclusive full-screen level ends the run.
  The window opens without activation, outside the monitors if the frames still come there, else behind the other windows.
  The maintainer's own input never reaches the game: the key and cursor queries answer from the agent's virtual keyboard and cursor, and the window procedure drops every mouse and keyboard message the agent did not send.
* **The game never outlives its host**: a job object kills it when the host's handle closes, and the agent ends it when its script is unloaded or the host's heartbeat stops.
* **Focus.**
  The original stops drawing without the focus (docs/09 §2.1), so its window procedure sees itself active and the focus queries answer with its window.
* **Input.**
  Keys and clicks are the agent's own messages to the window, which the window procedure turns into the real ones; coordinates are the window's pixels, 1024 × 768, the original's fixed resolution.
* **State and commands.**
  The agent reads the hero, the creatures near it, the clock, the zoom, the sector and the program's screen by the addresses and offsets of `docs/re/` and the Ghidra project, on the simulation thread's tick; commands that change the world (the hero onto a cell, the clock and its speed, the zoom) call the original's own functions on that thread.
* **Traces.**
  Every call of a function of the original, given by its address or by a preset's name (the fight, the drops, the regeneration, the creatures' decisions, the doors, the sounds), becomes a log record with the game's time and the wall clock's and its arguments.
* **Frames.**
  The original's own screenshot (`Capture/shotNNNN.tga`), converted to PNG by the host; the back buffer read before the flip if that fails.
* Frames, logs and traces of the original stay local, never in the repository (docs/08 §3); the results go into the issues and `docs/re/` as prose, and a frame of a problem, cropped to it, beside the game's into its issue or pull request as an attachment (`gh … --attach`).

What the first step found (2026-10-21, Windows 11, #449):

* Frida 17.24.3 spawns the 32-bit `Sacred.exe` suspended and injects into it from Claude Code's sandbox, with no command needing the sandbox disabled.
* The window outside every monitor draws all the same: 47–48 frames a second on the main menu for a minute, while another program has the foreground, and the screenshots are whole, as they come from the back buffer (docs/re/window.md).
  The game is never the foreground window and its window never topmost.
* Without the focus the original stands still rather than spinning: its render thread skips the frame, the world's updates in it as well, and sleeps (docs/re/window.md); with the window never activated, the game keeps the focus as it believes.
  At the start it waits for the foreground with no end, which the agent's answers to the focus queries end at once.
* The "tabblocker" installs its low-level keyboard hook only with the word `tabblocker` on the command line, and removes it again at once; the guard refuses it all the same (`--arg tabblocker` shows the refusal in the log).
* Stopping the host's background task ends the game within a second, the game's `quit` ends the host with its exit code, and a host that stops answering for 5 s ends the game.
* The maintainer's checks of 2026-10-22 (#546): Win, Alt-Tab, Win+Tab and Win+D work as ever while the original runs, with dgVoodoo2 and without it, and the original is not in Alt-Tab's list.
  The guards refused nothing, and the maintainer's clicks and keys did not reach the game: the hero stood on its cell.
* On a locked desktop the original draws nothing through the system's D3D7: its surfaces are lost at the lock, the frames and the agent's state stand, and a screenshot asked for waits for the unlock (docs/re/window.md).
  About 15 s after the unlock the game restores its surfaces itself and draws on.
* So dgVoodoo2 lies beside `Sacred.exe` since 2026-10-22, as the maintainer allowed on 2026-10-21: the official build 2.87.5 from its GitHub releases (the archive's SHA-256 as GitHub gives it), only its `MS/x86/DDraw.dll` and `MS/x86/D3DImm.dll`, and the archive's `dgVoodoo.conf` with `dgVoodooWatermark = false` its one change (#548).
  Through it (D3D7 → D3D11) the original drew 55 to 60 frames a second through a lock of 72 s, each screenshot taken during the lock whole and of a new frame, no surface lost.
  The game's log names the device `dgVoodoo TnL HAL`.
  dgVoodoo2's watermark, on by default, never showed in the original's screenshots, which come from the back buffer, but it did in the window as the desktop composes it (`PrintWindow` with `PW_RENDERFULLCONTENT`), and so for a player who starts the original from GOG; with the configuration it is gone there too.
  `verify` stays green, as the three files are not in the manifest; removing them brings back the system's D3D7.
* The player's input reaches the game as the agent's own messages, which the window procedures (the menus' and the game's, docs/re/window.md) take as the system's; input posted to the window from outside changes nothing (#450).
  Escape needs its character (`WM_CHAR` 27) as a keyboard sends it.
* The world's state and commands (#451, docs/re/objects.md): the objects' table and the hero, a creature's cell, health, level, experience, AI and action, the clock, the zoom; the hero onto a cell as the scripts' `Teleport` puts a player, the clock's time and rates, the zoom, the pixel of a cell, a walk by a click there.
  The agent reads and changes the world on the render thread's frame (`cEngine_handleZoomInput`), never on Frida's thread.
  A cell's pixel (`point`, and `go` that clicks it) lies at the height a creature stands at there on the hero's own level, the ground or its floor (`0x636520` plus the floor's `+0x33` · 28 units of the models' height, 768/400 · 2/√5 pixels each at zoom 1), as the game picks a click's cell at the height that shows under it (#979, docs/re/movement.md, "The cursor's point").
* The hero's state (#558, docs/re/objects.md): `god` keeps the hero's health through a strike's resolution and gives back any other loss at the next frame, beside the original's own `lord` flag, which spares only the knock-downs; `heal` refills a creature, or sets its health to N with `--to`; `level` and `experience` add experience through the original's `addExperience`, so that each level-up is play's own.
* The world's creatures and items (#559, docs/re/objects.md): `spawn` makes creatures with `FillSector`'s maker and the level setter, `kill` sends each hostile creature about the hero the hero's strike at a health of 1, so that the creature's own handler makes the death, the drops and the experience, `drop` and `item` call the drop.
  A function the agent calls from inside a hook's callback runs past every hook of the agent, the traces too (Frida's guard against recursion), so the agent replaces the frame's function, calling the original at its end, rather than hooking it, and the traces see what the commands make the game do.
* The traces (#452, docs/re/traces.md): presets for the fight's strikes, chances and damage, deaths and experience, the drops and the items made, the regeneration, the creatures' waking and idle rolls, the doors, the sounds; any address with the kinds of its arguments.
* A cell (#568, docs/re/movement.md, "Positions and layers"): `cell-info` builds a cell's position as the scripts' teleport does and asks the original's own functions of the world, on the ground and on each floor of the building there: the cell's bytes, whether it may be walked on (`0x636C10`), whether the hero may stand there (its own test, `0x4EBCA0`, the teleport's), the height it stands at (`0x636520`), a door's trigger and its state, and the objects of the cell's chain, to set beside opensacred-game's `cell-info`.
  The sounds play only at volumes above 0, so their traces take `--muted`: the game's volumes, and every volume of mss32.dll set to 0 by the agent.
  A stream's volume read back (`_AIL_stream_volume_pan`) answers the volume the game asked for, as the music's crossfades weigh the streams' own levels and, read back as 0, never ended: the music never came back after the ambient loop (#1213).
* The hero's gold, home and items (#973): `gold` reads the word `+0x46` of the hero's stats (`+0x3EE`), and `state` shows it.
  `gold N` brings it to N through the gold's handler `0x57FD80`, as a pile picked up or a price paid changes it: the handler first runs the original's integrity test (`0x590730`), and a gold written directly fails it, so that the next change kills the hero for good (seen: its gold left at 1 and no rise).
  `home` reads or sets the hero's home (`+0x2B9`), where it rises after its death; a cell's sector is filled in as a position's is.
  `wear` lists the hero's 19 slots; with a type it makes an object of it (`0x5FB530`) and puts it into its slot (`0x54ACC0`) through `0x555E00`, as a juggler takes its instrument, what the slot held being put aside.
* The weather (#1073, docs/re/weather.md): `weather rain|fog|snow|quake N` hands the renderer's handler (`0x419A70`) an event of the weather of that kind and value, kept on this machine, as the console's `setrain`, `setfog`, `setsnow` and `setquake` do, which the driver cannot type (Enter opened no console in a game of one player); `weather clear` sends the kind 1, so that every level fades.
* The interface's windows (#1340, docs/re/window.md, "The mouse's two points" and "The interface's controls").
  The interface tests a click at the mouse's point before the press, which only `WM_MOUSEMOVE` moves, so the driver's `move` and `click` post the move twice: a single move left that point where the cursor had been, and the clicks in the inventory's and the equipment's windows and on an item on the ground missed (the maintainer's real clicks gave the baseline).
  `state` shows the interface's own view in a game: the mouse's point and the point before, what the cursor holds, the controls under each point and the keyboard's focus.
  `ui` lists the windows shown, their controls' names, classes and rectangles, and a tooltip's links (the first window's and the tutorial's OK and Skip Tutorial) by the scripts' functions they run; `ui press WINDOW CONTROL` clicks the middle of one by its name, as a player does; `new-game --leave-open` leaves the first window to it.
  `point #ID` gives an object's own pixel, an item's on the ground, which a click takes up.

The maintainer's play (#551, asked for on 2026-10-22): the original on the screen, its 1024 × 768 scaled to the whole monitor with its proportions kept, played with the maintainer's own mouse and keyboard while the driver's commands stay at hand.

* **Full screen through dgVoodoo2.**
  The game's own window mode cannot be scaled: its flip copies the window's whole client rectangle from a back buffer of 1024 × 768, so a larger window fails every flip with `DDERR_INVALIDRECT` and shows black (docs/re/window.md).
  So the host writes `FULLSCREEN : 1` into `Settings.cfg` and `FullscreenAttributes = fake`, `ScalingMode = stretched_ar` into `dgVoodoo.conf`, and the background runs set them back (`FULLSCREEN : 0`, the two keys empty and `unspecified`).
  dgVoodoo2's full screen is a borderless window of the monitor's size with no display mode changed: the game's exclusive level and its `SetDisplayMode 1024x768` pass when `ddraw.dll` is dgVoodoo2's beside `Sacred.exe`, and end the run through any other.
  `--interactive` without dgVoodoo2 in place stops before the start.
* **On the screen.**
  The window opens shown and activated at the corner of the monitor asked for (the primary by default; the log lists them), where the agent keeps it.
  On 3840 × 2160 the picture is 2880 × 2160 between black bars; the screenshots still come from the back buffer, 1024 × 768.
* **The player's input.**
  The window procedure passes the system's mouse and keyboard messages; the key and cursor queries answer from the system while the game's window has the foreground, and from the agent's virtual input otherwise, so that the keys typed into a terminal never reach the game.
  The agent's own input (`click`, `go`, `key`) works throughout.
* **The cursor.**
  The game holds the cursor in its screen (`ClipCursor(0,0,1024,768)` as it is activated) and moves it (`SetCursorPos`); dgVoodoo2 maps both onto the picture below the agent's guards, which pass them as they are while the game's window has the foreground.
  The agent lets the cursor go when another window takes the foreground and holds it again on the game's return.
* **The focus.**
  Full screen, the game answers each `WM_ACTIVATEAPP` with its surfaces restored, the display mode set again and its textures and models flushed; the background runs' turning of a deactivation into an activation so took the foreground back from the first Alt-Tab and ended the game after the second.
  In the maintainer's play the game takes its first activation and no other, and keeps drawing behind another window; `WM_ACTIVATE` and `WM_NCACTIVATE` pass as the system sends them.
  The maintainer checked it on 2026-10-22: the cursor stays on the picture, Alt-Tab leaves at once and frees the cursor, the return holds it again, the game draws on.
* **The process's exceptions** are logged with where they happened (the first 20; those raised on purpose at `DEBUG`), and its own exit through `ExitProcess` is told from the agent's unloading.
* The tabblocker's hooks, the display mode changes through the system, `SendInput` and its kin, the job object and the heartbeat stay guarded as in the background runs.
  The monitors other than the primary are not checked yet.

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
* Every text file is UTF-8 without a BOM with LF line ends, on every platform (the maintainer's rule of 2026-10-24, #834; CLAUDE.md, "Code conventions"): the sources, the documents, the configuration, the converter's masters and the game's assets, what the game writes and the tools' outputs.
  `.gitattributes` stores the repository's text files with LF whatever a tool writes (`* text=auto eol=lf`, #32), and `.editorconfig` asks the editors for UTF-8 and LF.
  The code writes LF itself where a platform's default differs: `ron`'s `PrettyConfig::default()` and Python's text mode write CRLF on Windows (#835).
* CI on GitHub Actions (`.github/workflows/ci.yml`), on every pull request and every push to `main`: a Linux job (fmt, clippy, tests without the game files, the screenshot test), a Windows job (the target platform: the build with `rust-lld` and the tests; no clippy, since the workspace has no Windows-only code, and no screenshot test, since the runner has no GPU) and `cargo deny`.
  The dependencies' build is cached by `Swatinem/rust-cache` and saved only on `main`, which pull requests restore: caches of every branch would push `main`'s out of the repository's 10 GB.
  The Linux job's system packages that the runner image lacks are cached likewise, as their `.deb` files under the image's version (`ImageOS`, `ImageVersion`), and installed with `dpkg`, so the Ubuntu mirror, minutes slow in some runs, is reached once an image (#1109).
  Standard GitHub-hosted runners are free for a public repository, so CI minutes do not limit the checks; the tests on the game files run locally, because CI has none.
* The Release workflow (`.github/workflows/release.yml`) runs on every push to `main` and versions the merged pull requests (§3.6).
* The screenshot test runs in the Linux job on Mesa's software Vulkan (lavapipe), which is enough for it; the Windows job skips it, as its runner has no GPU.
* A run that fails on GitHub's side (a stalled download on the runner, a broken answer of the API) is run again by hand, `gh run rerun <id> --failed`; the workflows carry no timeouts or retries for it.
  Until 2026-10-22 the Mesa install (#534) and the release's lookup of a pull request (#515) had them; they did not keep a run green (#543) and made the workflows harder to read (#544).

## 6. What is excluded

| Tool | Why excluded |
|---|---|
| Visual C++ 6.0, decomp.me, reccmp, objdiff | byte-for-byte matching is not a goal |
| IDA Pro, Binary Ninja | paid |
| Wine environments for old compilers | old toolchains are not used |
| Network analyzers (TinCat protocol) | not before multiplayer, after version 1.0 (PLAN.md §13) |

## Open questions

None at the moment.

Closed: software Vulkan in CI is enough for the screenshot test, which runs on lavapipe in the Linux job (§5); Unicorn runs functions that read the game's global tables, with `.data` and `.rdata` mapped from the image and the balance's image loaded as the client loads it (#363, §3.4).

## Sources

* The compiler, the libraries and the functions shared by the client and the server: docs/01, §3 and §5.
* Rust 1.99.1: the stable channel; Visual Studio 2026 and MSVC 14.51: devblogs.microsoft.com/cppblog; Vulkan SDK 1.4.363.0: lunarg.com.
