# OpenSacred

An open-source engine for Sacred Gold 2.28 (GOG), written in Rust: an offline converter of the user's own installation into open formats and a new game on Bevy that reads only those formats. The repository contains no game files. The main document is `PLAN.md`; formats are in `docs/02-data-formats.md`; tools and process in `docs/03-toolchain.md`; legal in `docs/08-risks-legal.md`.

## Workspace

| Path | Purpose |
|---|---|
| `crates/opensacred-formats` | reading the original formats (PAK, TEX, GRN, sectors, NetScript, `global.res`, AMH); parsing only, no writing |
| `crates/opensacred-data` | types of the open formats, shared by the converter and the game (`world`, `sound`) |
| `crates/opensacred-convert` | CLI: GOG installation → `assets/`; supports `verify` (cross-check against the manifest), `textures` (TEX → PNG), `models` (GRN models and motions → glTF binary with animations) `world` (sectors, statics and sprites → postcard files) `sound` (samples → FLAC/MP3, profiles → RON) `text` (global.res → Fluent) `video` (WMV → AV1/Opus WebM), `ui` (screens, fonts, credits), `netscript` (the script trees → postcard files, listings on request), `tables` (creatures, weapons, items, motions, triggers → RON), `heroes` (the starting templates), `all` and `coverage` |
| `crates/opensacred-game` | the game on Bevy 0.20 (ADR-04), reading only `assets/`: the M2 vertical slice (streamed sectors with ground, liquids and static objects, the hero walking and running, the start script's NPCs, footsteps, music and ambient loops), a main menu with the choice of the hero and a pause menu (Esc), settings in `%APPDATA%\OpenSacred\settings.toml` (borderless or windowed, F11), a debug overlay (F3), the log (F4 and a file), a console of the game's commands (the key under Esc), scripts (`--exec`, `--script`) and remote control (`--remote`, docs/10); `--screenshot` saves a frame and exits, `--benchmark` measures |
| `OriginalGame/` | the developer's own GOG installation, ignored by Git — the reference, read-only |
| `docs/manifest-gog-2.28.txt` | SHA-256 of the reference installation (`sha256sum` format), embedded in `opensacred-convert` |
| `build.rs` | the build script of the converter and the game: their `--version` from `git describe` (`Cargo.toml` has no version) |
| `tools/` | research Python scripts — an archive and the reference for cross-checking readers (`tools/README.md`); the game's remote-control client `tools/game/remote.py` |
| `.claude/skills/game/` | the project skill `game`: how to run the game and drive it (scripts, remote control) |

`opensacred-rules`, `opensacred-oracle` (PLAN.md §6.1) appear when their turn comes; do not create them in advance.

## Commands

```bash
cargo build --workspace
cargo run -p opensacred-convert -- verify                 # OriginalGame/ against the manifest
cargo run --release -p opensacred-convert -- textures     # PAK/texture*.pak → assets/textures/<pak>/<number>-<name>.png
cargo run --release -p opensacred-convert -- models       # PAK/models*.pak → assets/models/<pak>/<number>-<name>.glb (after textures; VTex textures need FFmpeg)
cargo run --release -p opensacred-convert -- world        # World/ + tiles.pak, items.pak, mixed.pak → assets/world/ (world.ron, sectors/, sprites.bin)
cargo run --release -p opensacred-convert -- sound        # sound.pak, sndProfiles.pak, mp3/ → assets/sound/ (FLAC via FFmpeg, MP3, sound.ron)
cargo run --release -p opensacred-convert -- text         # scripts/us/global.res → assets/text/en-US/global.ftl
cargo run --release -p opensacred-convert -- video        # movie/*.wmv → assets/video/*.webm (AV1 + Opus via FFmpeg)
cargo run --release -p opensacred-convert -- ui           # PAK/*.bmp → PNG, fonts, credits → assets/ui/
cargo run --release -p opensacred-convert -- tables       # Creature/Weapon/Items/Items03/MOTIONS.PAK, Triggers.PAK, bin/*.bin → assets/tables/*.ron
cargo run --release -p opensacred-convert -- heroes       # Templates/*.ptx → assets/heroes/ (heroes.ron)
cargo run --release -p opensacred-convert -- netscript    # bin/**/{Funk,Start,Quest}Code.bin + Vectoren.bin → assets/netscript/ (--listing: <tree>.txt)
cargo run --release -p opensacred-convert -- all          # every conversion in order; skips those whose assets/manifest/<command>.ron still holds (--force runs all)
cargo run -p opensacred-convert -- coverage               # what becomes of each file of the manifest
cargo run --release -p opensacred-game                    # the game on assets/, from its menu: left mouse moves, R walks/runs, the wheel zooms, Esc pauses, F3 the overlay, F4 the log, F11 windowed, ~ the console (--class seraphim|gladiator|mage|wood-elf|dark-elf|daemon|dwarf starts a game at once; --sector X,Y, --zoom, --windowed, --width/--height, --no-music, --settings, --log-file)
cargo run --release -p opensacred-game -- --screenshot shot.png   # render one frame once the scene is ready, then exit
cargo run --release -p opensacred-game -- --windowed --no-music --exec "new-game --class mage; wait ready; screenshot shot.png; quit"   # a script of the game's commands (help lists them)
cargo run --release -p opensacred-game -- --remote --windowed --no-music --settings <scratch>/settings.toml --log-file <scratch>/game.log   # remote control on 127.0.0.1:15702 (the skill `game`, docs/03 §3.7)
python -I tools/game/remote.py exec "go +6,+2"          # drive a game started with --remote: exec, state, wait-ready, key, type, click, screenshot, logs
gltf_validator assets/models                         # writes a .report.json next to every .glb
cargo nextest run --workspace                       # tests without the game files (as in CI)
cargo nextest run --workspace --run-ignored all     # plus tests on the game files and the GPU
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

- A task is an issue, opened from the form of its type, which sets one of the labels `bug`, `deps`, `feature`, `research`, `infra`, `docs`; a milestone's work also carries its milestone (M2–M8). A large feature is an issue with sub-issues, one pull request each.
- Branch `<type>/<issue>-<slug>` from a fresh `origin/main` (`git fetch && git switch -c feature/3-roofs origin/main`). Commits and the pull request's title keep the history's `Scope: what` style; the body follows `.github/pull_request_template.md` and starts with `Closes #N`; the pull request takes its issue's label.
- After the checks of "Before finishing a task": `git push -u origin <branch>`, `gh pr create`, then `gh pr merge --squash --auto`: GitHub merges once the checks are green and deletes the branch. Watch them (`gh pr checks --watch`); a red check is fixed on the branch. Then `git switch main && git pull`.
- Every pull request carries its Claude Code usage: `python -I tools/usage/pr_usage.py <N> --update` writes it into the description from the transcripts of its branch and refreshes the README's badge (docs/usage.md). Run it after `gh pr create`, and again when the work on the pull request is over: after the merge, or once the checks are green for one that waits for the maintainer.
- The maintainer merges, and the pull request is labeled `needs-maintainer` instead of merged, when it changes `CLAUDE.md`, `.claude/`, `.github/`, `deny.toml`, the README of `docs/decisions/`, a screenshot reference or the process (docs/03 §3.6), takes a decision that waits (below), or comes from someone else. The PreToolUse hook `.claude/hooks/merge_guard.py` refuses `gh pr merge` on such a pull request; do not merge it another way.
- A pull request that adds, changes or removes a decision (a file of `docs/decisions/`) and nothing else that waits does not wait: label it `review-after-merge` and merge it; the maintainer reviews it afterwards and takes the label off, or opens an issue for what to change. That is a decision that refines how the game follows the original, fixes or replaces an earlier one, or departs where the play hardly changes (an engine limitation, a format's quirk). Two kinds wait, and you judge which a decision is: one that departs from the original clearly and far, so that it can change the experience of the play seriously (the rules, formulas and balance, the controls or the camera, what the player can do, a quest's course, the game's look as a whole), and one with legal consequences (licenses, the use of the game's content or of others' code, what the repository or a release carries; docs/08). The hook cannot judge it: it only refuses a pull request that takes a decision without one of the two labels.
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

- Never commit or publish anything from `OriginalGame/`, `assets/`, `assets-hd/`, converter output or dumps. Do not modify files in `OriginalGame/`.
- Do not quote the game's texts, dialogues or license agreements in the repository; record only facts about formats and behavior.
- Never commit decompiler output or code translated from it: describe the original's behavior in prose in `docs/re/` and implement it independently (docs/08 §3). Nothing about circumventing a protection.
- The same rules hold for issues, pull requests and their attachments: no game files, converted assets or screenshots of them (docs/08 §3).
- `unsafe` is forbidden by a workspace lint; the exception is the future `opensacred-oracle`.
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
- In `opensacred-game`, a module registers its own systems, resources and observers in a `Plugin` of its own; `main.rs` keeps the window, the command line, the resources read from the assets, and one line a module's plugin, in alphabetical order. The steps of `frame::Step` order the systems across modules.

## Environment

Windows 11 x64; Rust — the version from `rust-toolchain.toml`; the `rust-lld` linker (`.cargo/config.toml`). Where tools outside `PATH` live (Blender, RenderDoc): `docs/03-toolchain.md`, §2.
