# OpenSacred

An open-source engine for Sacred Gold 2.28 (GOG), written in Rust: an offline converter of the user's own installation into open formats and a new game on Bevy that reads only those formats. The repository contains no game files. The main document is `PLAN.md`; formats are in `docs/02-data-formats.md`; tools and process in `docs/03-toolchain.md`; legal in `docs/08-risks-legal.md`.

## Workspace

| Path | Purpose |
|---|---|
| `crates/opensacred-formats` | reading the original formats (PAK, TEX, GRN, sectors, NetScript, `global.res`, AMH); parsing only, no writing |
| `crates/opensacred-rules` | the game's rules with no engine (ADR-04, PLAN.md §6.3): the spawning's rolls of `FillSector` (`spawning`), the fight's formulas (`fight`: the creatures' levels, health, attack, defence, damage and resistances, a strike's hit, the items' values, the regeneration, the hero's experience and levels), the ways (`ways`: the search of the fewest steps over a 64 × 64 window, the straight line's test of the cells it crosses, who stops whose step into a cell) and the creatures' decisions (`minds`: waking and dormancy, the idle action's steps, the animals' flight and strolls, the guards' posts, the townsfolk's and the children's rolls, parting and stepping aside, the pursuit of the hero, the creatures of hostile sides taking one another for their foes, and the hero's attack's choice of where it stands and when it strikes) and what a killed creature drops (`drops`: the calls and their bonus, the chance and its passes, the quality key, the kind, the type by the category, the level and the templates, gold's pile and amount, and the items by the item maker); `opensacred-game`'s systems read the world for them and carry them out |
| `crates/opensacred-data` | types of the open formats, shared by the converter and the game (`world`, `sound`, `tables`, `heroes`, `netscript`, `text`, `video`), the game's ZIP archives (`archive`) and KTX2 textures (`texture`) |
| `crates/opensacred-convert` | CLI in two stages (PLAN.md ADR-05): the extractions, GOG installation → masters in `assets-src/`, and `build`, masters → the game's `assets/`; supports `verify` (cross-check against the manifest), `textures` (TEX → PNG), `models` (GRN models and motions → glTF binary with animations), `world` (sectors, statics and sprites → postcard files), `sound` (samples → FLAC/MP3, profiles → RON), `text` (global.res → Fluent), `video` (WMV → AV1/Opus WebM), `ui` (screens, fonts, credits), `netscript` (the script trees → postcard files, listings on request), `tables` (creatures, weapons, items, motions, triggers, the combat arts, the creatures' voices → RON), `heroes` (the starting templates), `build`, `all` and `coverage` |
| `crates/opensacred-game` | the game on Bevy 0.20 (ADR-04), reading only `assets/`: the M2 vertical slice (streamed sectors with ground, liquids and static objects, the hero walking and running, the start script's NPCs, footsteps, music and ambient loops), day and night, the world's creatures spawned and living by the original's rules, the fight (the creatures' and the hero's strikes, health, death, levels, their voices and the weapons' sounds), the creatures' drops thrown where they die, a main menu with the choice of the hero and a pause menu (Esc), settings in `%APPDATA%\OpenSacred\settings.toml` (borderless or windowed, F11), the keys' help (F1), a debug overlay (F3), the log (F4 and a file), a console of the game's commands (the key under Esc), scripts (`--exec`, `--script`) and remote control (`--remote`, docs/10); `--screenshot` saves a frame and exits, `--benchmark` measures |
| `crates/opensacred-oracle` | development only, run locally (docs/04 §2.3): the developer's own `Sacred.exe` in the Unicorn emulator (GPL-2.0, the feature `emulator`; a binary that nothing depends on), the PE image at its addresses with the balance's image, stubs for the functions and imports the code calls, and the fight's functions run on sets of inputs (the level's end and the level of an experience, the hit chance and the damage through a resistance of `0x43EEE0`, the most health) → JSON vectors outside the repository, which `opensacred-rules`' `tests/oracle.rs` compares with |
| `OriginalGame/` | the developer's own GOG installation, ignored by Git — the reference, read-only |
| `docs/manifest-gog-2.28.txt` | SHA-256 of the reference installation (`sha256sum` format), embedded in `opensacred-convert` and `opensacred-oracle` |
| `build.rs` | the build script of the converter and the game: their `--version` from `git describe` (`Cargo.toml` has no version) |
| `tools/` | research Python scripts — an archive and the reference for cross-checking readers (`tools/README.md`); the game's remote-control client `tools/game/remote.py`; the original's driver `tools/original/` (Frida, docs/03 §3.8); the Ghidra project's scripts `tools/ghidra/`; the process scripts `tools/usage/` and `tools/stack/` |
| `.claude/skills/game/` | the project skill `game`: how to run the game and drive it (scripts, remote control) |
| `.claude/skills/original/` | the project skill `original`: how to run the original `Sacred.exe` in the background and drive it |

## Commands

```bash
cargo build --workspace
cargo run -p opensacred-convert -- verify                 # OriginalGame/ against the manifest
cargo run --release -p opensacred-convert -- textures     # PAK/texture*.pak → assets-src/textures/<pak>/<number>-<name>.png
cargo run --release -p opensacred-convert -- models       # PAK/models*.pak → assets-src/models/<pak>/<number>-<name>.glb (after textures; VTex textures need FFmpeg)
cargo run --release -p opensacred-convert -- world        # World/ + tiles.pak, items.pak, mixed.pak → assets-src/world/ (world.ron, sectors/, sprites.bin)
cargo run --release -p opensacred-convert -- sound        # sound.pak, sndProfiles.pak, mp3/ → assets-src/sound/ (FLAC via FFmpeg, the MP3s decoded, sound.ron)
cargo run --release -p opensacred-convert -- text         # scripts/us/global.res → assets-src/text/en-US/global.ftl
cargo run --release -p opensacred-convert -- video        # movie/*.wmv → assets-src/video/*.mkv (FFV1 + FLAC via FFmpeg; build makes AV1 + Opus WebM)
cargo run --release -p opensacred-convert -- ui           # PAK/*.bmp → PNG, fonts, credits → assets-src/ui/
cargo run --release -p opensacred-convert -- tables       # Creature/Weapon/Items/Items03/MOTIONS.PAK, Triggers.PAK, bin/*.bin, the arts and the voices of Sacred.exe → assets-src/tables/*.ron
cargo run --release -p opensacred-convert -- heroes       # Templates/*.ptx → assets-src/heroes/ (heroes.ron)
cargo run --release -p opensacred-convert -- netscript    # bin/**/{Funk,Start,Quest}Code.bin + Vectoren.bin → assets-src/netscript/ (--listing: <tree>.txt)
cargo run --release -p opensacred-convert -- build        # stage B: the game's assets in assets/ from the masters in assets-src/ (the textures into KTX2, #700; the sound into Ogg Opus, #701), packed into ZIP archives by kind, only those whose masters changed made again (#694, #699)
cargo run --release -p opensacred-convert -- all          # every extraction in order into assets-src/, skipping those whose assets-src/manifest/<command>.ron still holds, then build (--force runs all)
cargo run -p opensacred-convert -- coverage               # what becomes of each file of the manifest
cargo run --release -p opensacred-oracle --features emulator   # the fight's functions of OriginalGame/Sacred.exe in Unicorn → vectors in OPENSACRED_VECTORS, else %LOCALAPPDATA%\OpenSacred\vectors (--out, --precision 24|53|64); needs CMake, Ninja, a C compiler and libclang (LIBCLANG_PATH), docs/03 §3.4
cargo run --release -p opensacred-game                    # the game on assets/, from its menu: left mouse moves, on a hostile creature attacks (Ctrl strikes at once), right mouse on one makes the selected combat art, R walks/runs, the wheel zooms, Esc pauses, F1 the keys' help, F3 the overlay, F4 the log, F11 windowed, ~ the console (--class seraphim|gladiator|mage|wood-elf|dark-elf|daemon|dwarf starts a game at once; --sector X,Y, --zoom, --time HH:MM, --windowed, --borderless, --hidden, --width/--height, --no-music, --settings, --log-file)
cargo run --release -p opensacred-game -- --screenshot shot.png   # render one frame once the scene is ready, then exit
cargo run --release -p opensacred-game -- --windowed --no-music --exec "new-game --class mage; wait ready; screenshot shot.png; quit"   # a script of the game's commands (help lists them)
cargo run --release -p opensacred-game -- --remote --hidden --no-music --settings <scratch>/settings.toml --log-file <scratch>/game.log   # remote control on 127.0.0.1:15702, the window hidden (the skill `game`, docs/03 §3.7)
python -I tools/game/remote.py exec "go +6,+2"          # drive a game started with --remote: exec, state, wait-ready, wait-state, key, type, move, click, wheel, screenshot, logs, wait-log, call
python -I tools/original/host.py --log-file <scratch>/original.log   # the original Sacred.exe under Frida in the background, remote control on 127.0.0.1:15703 (the skill `original`, docs/03 §3.8)
python -I tools/game/remote.py --port 15703 exec "new-game --class seraphim"   # drive the original: the same calls; exec new-game, save, load, cell, creatures, stats, god, heal, level, spawn, kill, drop, trace, quit
gltf_validator assets/models                         # writes a .report.json next to every .glb
cargo nextest run --workspace                       # tests without the game files (as in CI)
cargo nextest run --workspace --run-ignored all     # plus tests on the game files and the GPU (the oracle's vectors recorded first)
cargo nextest run -p opensacred-rules --run-ignored only -E 'binary(oracle)'   # the fight's formulas against the oracle's vectors
cargo nextest run --workspace --run-ignored only -E 'test(scene)'   # the screenshot test (a GPU)
UPDATE_SCREENSHOTS=1 cargo nextest run --workspace --run-ignored only -E 'test(scene)'   # new reference
cargo deny check                                    # licenses, vulnerabilities, sources
python -I tools/analysis/formats/<script>.py --game-dir ./OriginalGame   # Python prototypes
```

## Work order

- The queue is the open issues (PLAN.md §10.1). Priorities: first the bugs (`bug`), then the dependency upgrades (`deps`), then new features (`feature`, `research`), the earliest milestone first. A problem found along the way becomes an issue of its own.
- Once a day, at the start of the maintainer's first session of the day, before other work, update the dependencies. Skip it if, in the last 24 hours, `main` got a `Dependencies: cargo update` commit (`git fetch && git log origin/main --since=24.hours --grep='^Dependencies: cargo update'`) or such a pull request is still open. A contributor's change leaves `Cargo.lock` alone unless the task is about dependencies.
  1. `cargo update --verbose`: it moves every package to its newest compatible version and lists, as `Unchanged … (available: …)`, those that a newer incompatible release leaves behind.
  2. If `Cargo.lock` changed: `cargo deny check`, clippy and `cargo nextest run --workspace --run-ignored all`; a patch release of `png`, `miniz_oxide`, `ron` or `postcard` can change the converter's output.
  3. A pull request with the new `Cargo.lock` alone: branch `deps/cargo-update-<date>`, title `Dependencies: cargo update`, the notable bumps in the body, label `deps`, no issue; merged with `gh pr merge --squash --auto`. An update that breaks something is fixed, or held back with `cargo update -p <crate> --precise <old version>` and the reason written in a `deps` issue.
  4. Every newer incompatible release of a direct dependency (`[workspace.dependencies]`; `cargo search <crate>` shows the latest) and of Rust (`rustup check` against `rust-toolchain.toml`) gets a `deps` issue of its own, unless one is open (docs/03 §3.3).

## Issues and pull requests

`main` takes changes only through squash-merged pull requests with green checks (Linux, Windows, cargo deny); the process is in docs/03 §3.6.

- A task is an issue, opened from the form of its type, which sets one of the labels `bug`, `deps`, `feature`, `research`, `infra`, `docs`; a milestone's work also carries its milestone (M2–M9). A large feature is an issue with sub-issues, one pull request each.
- Branch `<type>/<issue>-<slug>` from a fresh `origin/main` (`git fetch && git switch -c feature/3-roofs origin/main`). Commits and the pull request's title keep the history's `Scope: what` style; the body follows `.github/pull_request_template.md` and starts with `Closes #N`; the pull request takes its issue's label.
- After the checks of "Before finishing a task": `git push -u origin <branch>`, `gh pr create`, then `gh pr merge --squash --auto`: GitHub merges once the checks are green and deletes the branch. Watch them (`gh pr checks --watch`); a red check is fixed on the branch. Then `git switch main && git pull`.
- Every pull request carries its Claude Code usage: `python -I tools/usage/pr_usage.py <N> --update` writes it into the description from the transcripts of its branch and refreshes the README's badge (docs/usage.md). Run it after `gh pr create`, and again when the work on the pull request is over: after the merge, or once the checks are green for one that waits for the maintainer.
- The maintainer merges, and the pull request is labeled `needs-maintainer` instead of merged, when it changes the guardrails (`.claude/hooks/`, `.claude/settings.json`, `.claude/settings.local.json`), `.github/`, `deny.toml`, the README of `docs/decisions/` or a screenshot reference, changes the rules or the process (below; docs/03 §3.6), takes a decision that waits (below), or comes from someone else. The PreToolUse hook `.claude/hooks/merge_guard.py` refuses `gh pr merge` on such a pull request; do not merge it another way.
- A pull request that changes `CLAUDE.md` or the rest of `.claude/` (the skills) only to describe — the workspace, the commands, the keys, how a skill is used, a fact of the code — and nothing else that waits does not wait either: merge it like any other. One that changes the rules waits: the work order, the issues and pull requests, the prohibitions, the rules of the tests and of the code, what a hook or a permission does, or any rule loosened. You judge which; the hook cannot, it refuses such a pull request only by its `needs-maintainer` label.
- A pull request that adds, changes or removes a decision (a file of `docs/decisions/`) and nothing else that waits does not wait: merge it like any other; the maintainer opens an issue for what to change. That is a decision that refines how the game follows the original, fixes or replaces an earlier one, or departs where the play hardly changes (an engine limitation, a format's quirk). Two kinds wait, and you judge which a decision is: one that departs from the original clearly and far, so that it can change the experience of the play seriously (the rules, formulas and balance, the controls or the camera, what the player can do, a quest's course, the game's look as a whole), and one with legal consequences (licenses, the use of the game's content or of others' code, what the repository or a release carries; docs/08). The hook cannot judge it: it refuses such a pull request only by its `needs-maintainer` label.
- The label sets the version (docs/03 §3.6): after a merge, the Release workflow tags the squash commit `vX.Y.Z` and publishes a GitHub Release; `feature` raises the minor version, `bug` the patch, the other labels leave it. The major version only when the maintainer asks: `gh workflow run release.yml -f bump=major`.
- Without an issue: the daily `Dependencies: cargo update` pull request, merged with `--auto`.
- No deep stacks. A pull request is based on another only when it cannot be written from `main`, and at most two deep; otherwise take other work from `main`, or wait.
- The maintainer merges only pull requests based on `main`. Once one is squash-merged, `python -I tools/stack/stack.py after <N>` brings those stacked on it up to `main` (it merges `main` in with the newest of their base's commits as the merge base, so that their own changes to its lines stand); tell the maintainer when they are ready.
- When two or more pull requests wait, `python -I tools/stack/stack.py integrate --check` merges them all into a local branch `integration`: write the order of merging and the conflicts it finds into the pull requests, and resolve those conflicts on their branches.

## Before finishing a task

1. `cargo fmt --all`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo nextest run --workspace`; if the task touches the game files, with `--run-ignored all`.
4. After changing dependencies, `cargo deny check`.

Hooks in `.claude/settings.json` automate steps 1–3 without the game files: `cargo fmt` after every edit of a `.rs` file; clippy and nextest for the changed crates and their dependents when you stop. A failed check comes back as the reason you cannot stop yet; fix it. `--run-ignored all` and `cargo deny check` stay manual.

## Prohibitions

- Never commit or publish anything from `OriginalGame/`, `assets/`, `assets-hd/`, converter output or dumps. Do not modify files in `OriginalGame/`; the one exception is the original that the driver of #448 runs there, which writes its own files (`Settings.cfg`, `save/`, `Capture/`, its logs), never one of the manifest (docs/03 §3.8).
- Do not quote the game's texts, dialogues or license agreements in the repository; record only facts about formats and behavior.
- Never commit decompiler output or code translated from it: describe the original's behavior in prose in `docs/re/` and implement it independently (docs/08 §3). Nothing about circumventing a protection.
- The same rules hold for issues, pull requests and their attachments: no game files, converted assets or screenshots of them (docs/08 §3).
- `unsafe` is forbidden by a workspace lint, in `opensacred-oracle` too: its Unicorn wrapper needs none, and only a wrapper that did would make it the exception.
- Do not use code from unlicensed projects (SacredEngineRemake); do not port GPL code (sacred-bild) — facts only. MIT borrowings carry attribution.
- New dependencies only with licenses allowed in `deny.toml`.

## Tests

- A test that needs the game files is marked `#[ignore = "needs the game files in OriginalGame/"]`: CI has no game files.
- The screenshot test of `opensacred-game` renders a synthetic world in the converter's formats, with no game files, and is ignored because it needs a GPU adapter; CI runs it on Mesa's lavapipe. A deliberate change to the scene's look comes with a new reference (`UPDATE_SCREENSHOTS=1`); look at it before committing.
- Expected numbers (record counts, sizes, hashes) are taken from `docs/02-data-formats.md`, where they are confirmed by the `tools/` scripts.
- A new format reader is cross-checked against the Python prototype from `tools/analysis/`. Cross-check tests run the interpreter from `PYTHON` (default `python` or `python3`); dependencies are in `tools/requirements.txt`. The glTF test runs glTF-Validator from `GLTF_VALIDATOR` (default `gltf_validator`).

## Language

- Everything in the repository is in English: code, comments, documents, commit messages, PR descriptions, issues.
- Talk to users in English unless they ask for another language.

## Documentation

- Claim statuses: **confirmed**, **community-reported**, **estimate**, **hypothesis** (PLAN.md).
- A newly found fact about a format goes into `docs/02-data-formats.md`; a resolved open question is removed from the lists in PLAN.md §13 and in `docs/`.
- Recovered logic of the original goes into `docs/re/<subsystem>.md`.
- Deliberate deviations from the original's behavior go into `docs/decisions/`, a file each (`docs/decisions/README.md`).
- Claude Code usage goes into each pull request's description, not into a file ("Issues and pull requests"; docs/usage.md).
- One sentence per line in the documents (`docs/`, `PLAN.md`, `tools/README.md`): two changes to one paragraph then touch different lines. A list item's later sentences are indented to stay in it.

## Code conventions

- Identifiers, CLI and error messages, doc comments, comments and configuration files (`*.toml`, CI) are in English, like everything else.
- Original names and addresses (`cEngine::simThread`, `0x60f060`) are mentioned in comments next to the code that implements the same behavior.
- Dependency versions live in `[workspace.dependencies]` of the root `Cargo.toml`; crates pull them in with `workspace = true`.
- Clippy rules live in `[workspace.lints.clippy]`. Numbers are converted with `From`/`TryFrom`, not `as`; outside tests, no `unwrap`/`expect` — return an error with context (file, record). A rule is suppressed only with `#[expect(clippy::…, reason = "…")]` and only when the code is genuinely better that way; do not rewrite code to satisfy a rule at the expense of clarity.
- Paths inside the installation use the case from the manifest (`PAK/`, `World/`).
- A defect of the original's data (garbage or broken pixels, a record that is wrong or points at nothing, junk in a file) is repaired by `opensacred-convert` as it writes `assets/`, never worked around in `opensacred-game`; `opensacred-formats` still reads the files as they are. A repair is as narrow as the defect (named files, series or records), tested, described in `docs/02-data-formats.md` next to the format and recorded as a decision in `docs/decisions/`, which waits for the maintainer when the repair changes the play (a table's values, a script).
- In `opensacred-game`, a module registers its own systems, resources and observers in a `Plugin` of its own; `main.rs` keeps the window, the command line, the resources read from the assets, and one line a module's plugin, in alphabetical order. The steps of `frame::Step` order the systems across modules.

## Environment

Windows 11 x64; Rust — the version from `rust-toolchain.toml`; the `rust-lld` linker (`.cargo/config.toml`). Where tools outside `PATH` live (Blender, RenderDoc): `docs/03-toolchain.md`, §2.
