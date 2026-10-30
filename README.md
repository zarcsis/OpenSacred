# OpenSacred

[![Claude Code usage](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2Fzarcsis%2FOpenSacred%2Fbadges%2Fusage.json)](docs/usage.md)

An open-source engine for **Sacred Gold** (Sacred and its expansion Underworld, the GOG release 2.28), written in Rust on [Bevy](https://bevyengine.org/). The goal is to play Sacred and Underworld comfortably on modern hardware: Windows 11 x64 at 4K first, later Linux, possibly Android.

OpenSacred contains none of the game's files. You need your own copy of Sacred Gold from GOG:

- an offline converter reads your installation once and turns its assets into open formats in two stages: the masters (PNG, glTF, FLAC, FFV1 video, RON and postcard, Fluent, the scripts translated into Luau), then the game's assets made from them (KTX2, Ogg Opus, AV1/Opus WebM, Luau bytecode) and packed into ZIP archives;
- the engine reads only the game's assets;
- the game's rules are reimplemented from documented behavior ([docs/re/](docs/re/)); deliberate deviations from the original are recorded in [docs/decisions/](docs/decisions/).

Multiplayer comes after version 1.0; until then there is no networking. Original save files are not loaded. The full rationale is in [PLAN.md](PLAN.md).

## Status

| Milestone | State |
|---|---|
| M0 Foundation: Cargo workspace, CI, the SHA-256 manifest of the reference installation | done |
| M1 Converter: textures, Granny models and animations → glTF, world sectors, sound, texts, videos, interface, tables, hero templates, scripts | done |
| M2 Vertical slice: streamed sectors with ground, water and objects, the hero walking and running, the start script's NPCs, footsteps, music and ambience, day and night, the buildings' insides, the original's cursor; menus, settings, a console and remote control | done |
| M3 Rules: the Ghidra project and an emulator that runs the original's own formulas to check ours against; the converter's two stages and the game's packed assets; the world's creatures spawned and living as in the original, the followers and the summonings; the fight of the creatures and the hero (health, death, experience and levels, the combat arts and the spells); what the killed drop, and picking it up | done |
| M4 Quests and world: the scripts translated into Luau and run through the play (the quests, the triggers, the timers, the teleports, the world's changes), the dialogues and the icons over the creatures, the townsfolk's day, the weather's choice | done |
| M5 UI and effects: the original's main menu and choice of the hero, the difficulty levels, the interface over the world, the inventory, the character's sheet, the trade and the blacksmith, the quest log and the maps, the effects, the weather drawn, the sound by the profiles, the movies | in progress |
| M6 Version 1.0 for Windows 11: saves, the settings' menu, the gamepad, 4K and HiDPI, the campaigns played through | planned |
| M7–M9: HD assets, Linux and possibly Android, multiplayer | after version 1.0 |

It is not a game you can play through yet: the inventory is partly made, and there is no trade, no character sheet and no saving. The plan and the queue are in [PLAN.md](PLAN.md), §9–§10, and in the [milestones](https://github.com/zarcsis/OpenSacred/milestones).

## Playing a release

Each release on the [Releases](https://github.com/zarcsis/OpenSacred/releases) page carries `OpenSacred-vX.Y.Z-windows-x64.7z`: unpack it into a folder of its own and run `opensacred-game.exe`. At its first start the game finds your GOG or Steam installation of Sacred Gold 2.28 (English), or asks for its folder, and makes its assets from it once, which takes some minutes and 11 GB of free disk space for its time; they stay in `%LOCALAPPDATA%\OpenSacred\assets`. The archive holds no file of the game; its `README.txt` says the rest.

## Requirements

- **The game:** your own copy of Sacred Gold 2.28 (English) from GOG or Steam. The converter checks it against the embedded SHA-256 manifests ([docs/manifest-gog-2.28.txt](docs/manifest-gog-2.28.txt), and [docs/manifest-steam-2.28.txt](docs/manifest-steam-2.28.txt) for the four files in which the Steam release differs); other builds are not supported. The development tools (the oracle, the original's driver) take the GOG release alone.
- **Rust:** [rustup](https://rustup.rs/); the exact toolchain is pinned in [rust-toolchain.toml](rust-toolchain.toml) and installs automatically.
- **Windows:** Visual Studio Build Tools 2026 with the "Desktop development with C++" workload and the Windows SDK. The workspace links with `rust-lld` ([.cargo/config.toml](.cargo/config.toml)).
- **CMake** in `PATH`: the game's build compiles libopus with it.
- **FFmpeg** in `PATH`: the converter runs it for the sound, the videos and a few textures.
- **Cargo tools** for development: `cargo install cargo-nextest cargo-deny`, and `cargo-mutants` for mutation testing ([docs/03-toolchain.md](docs/03-toolchain.md), §3.4).
- **[uv](https://docs.astral.sh/uv/)** for the Python tools in [tools/](tools/README.md), the Claude Code hooks and the tests that cross-check against the research scripts: it installs Python 3.15 and the packages of [pyproject.toml](pyproject.toml) for the project alone, never into the system's Python; `uv run python -I <script>`.

The full tool list is in [docs/03-toolchain.md](docs/03-toolchain.md).

## Getting started

```bash
git clone https://github.com/zarcsis/OpenSacred.git
cd OpenSacred
# Copy or link your GOG installation of Sacred Gold to OriginalGame/ (ignored by Git),
# or pass --game-dir <path> to every converter command.
cargo run --release -p opensacred-convert -- verify   # check the installation against the manifest
cargo run --release -p opensacred-convert -- all      # extract everything into assets-src/, then build the game's assets/ (both ignored by Git)
cargo run --release -p opensacred-game                # the game, from its menu
```

In the game the left button moves, speaks to a creature that has something to say and attacks a hostile one (with Ctrl it strikes at once), the right button makes the selected combat art, R switches walking and running, I opens the inventory, Space drinks a healing potion from the belt, 1 to 5 choose the weapon set, the wheel zooms, Esc pauses, F1 lists the keys, F3 shows the debug overlay, F4 the log, F11 switches between borderless full screen and a window, and the key under Esc opens the console.

The converter works in two stages. The extractions `textures`, `models`, `world`, `sound`, `text`, `video`, `ui`, `tables`, `heroes` and `netscript` write the masters into `assets-src/`; `build` makes the game's `assets/` from them, packed into ZIP archives by kind, and remakes only those whose masters changed. `all` runs every extraction in order, skipping those whose output is up to date (`--force` runs them all), then `build`; `verify` checks the installation and `coverage` tells what becomes of each of its files. `--help` describes each. The game takes `--class` (a game at once, skipping the menu), `--sector X,Y`, `--zoom`, `--time HH:MM`, `--windowed`/`--borderless`, `--width`/`--height`, `--no-music`, `--settings`, `--log-file`, `--screenshot`, `--benchmark`, `--exec` and `--remote`; its `--help` lists the rest.

The converted assets are made from your copy of the game. Keep them for yourself: do not publish or share them (see [docs/08-risks-legal.md](docs/08-risks-legal.md)).

## Repository layout

| Path | Contents |
|---|---|
| `crates/opensacred-formats` | readers of the original formats (PAK, TEX, GRN, sectors, NetScript, `global.res`, AMH); parsing only |
| `crates/opensacred-rules` | the game's rules with no engine: spawning, the fight's formulas, the ways, the creatures' minds, the arrows, the drops, the combat arts and spells, the quests' state and rewards, the backpack, the equipment, the potions and the weapon sets, the weather, the music, the particles and the undead |
| `crates/opensacred-scripts` | the runtime of the scripts translated into Luau, sandboxed |
| `crates/opensacred-data` | open-format types shared by the converter and the game, the game's ZIP archives and KTX2 textures |
| `crates/opensacred-convert` | the converter CLI |
| `crates/opensacred-game` | the game on Bevy |
| `crates/opensacred-oracle` | development only: the developer's own `Sacred.exe` in the Unicorn emulator, running the original's fight formulas to record the vectors the rules' tests compare with |
| `docs/` | design documents, format descriptions, recovered behavior, decisions |
| `tools/` | research scripts (the reference prototypes for the Rust readers), the Ghidra project's scripts, the game's remote-control client, the original's driver under Frida, and the process scripts (usage, stacked pull requests) |
| `.claude/skills/` | the project skills of Claude Code: running and driving the game and the original |

## Tests

```bash
cargo nextest run --workspace                       # without the game files, as in CI
cargo nextest run --workspace --run-ignored all     # plus the tests on OriginalGame/, the Python prototypes and the GPU
cargo run --release -p opensacred-oracle --features emulator                  # record the original's fight formulas (docs/03 §3.4)
cargo nextest run -p opensacred-rules --run-ignored only -E 'binary(oracle)'   # the rules' formulas against them
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check                                    # licenses, advisories, sources
```

Tests that need the game files are marked `#[ignore]`: CI does not have them. Cross-check tests run the research scripts through `uv run python -I`.

## Documentation

| Document | Contents |
|---|---|
| [PLAN.md](PLAN.md) | the main plan: goals, decisions, architecture, estimates, risks |
| [docs/02-data-formats.md](docs/02-data-formats.md) | the original data formats and their conversion targets |
| [docs/03-toolchain.md](docs/03-toolchain.md) | tools, workstation setup, CI, the workflow with Claude Code |
| [docs/re/](docs/re/) | the original's behavior, recovered and described by subsystem |
| [docs/decisions/](docs/decisions/) | the deliberate deviations from the original, a table per subsystem |
| [docs/08-risks-legal.md](docs/08-risks-legal.md) | the legal policy: what the project publishes and what it never does |
| [docs/10-game-tools.md](docs/10-game-tools.md) | the game's commands, scripts and remote control |
| [docs/usage.md](docs/usage.md) | how the Claude Code usage of each pull request is counted |
| [tools/README.md](tools/README.md) | the research scripts and how to run them |
| [CLAUDE.md](CLAUDE.md) | conventions for development with Claude Code |

The full index is in [PLAN.md](PLAN.md), §14.

## Contributing

Work goes through [issues](https://github.com/zarcsis/OpenSacred/issues) and pull requests. Open an issue from the form of its type (bug, feature, research, dependency, infrastructure, documentation), branch from `main`, and open a pull request from the template; it is merged once the checks (Linux, Windows, cargo deny) are green and the maintainer has looked at it. CI has no game files, so run the tests on your own installation (`cargo nextest run --workspace --run-ignored all`) before opening it. The process is described in [docs/03-toolchain.md](docs/03-toolchain.md), §3.6.

## License

The code is licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option. Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions. Notices of the community projects whose facts the readers follow are in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

## Legal

OpenSacred is not affiliated with, endorsed by or sponsored by THQ Nordic, Ascaron Entertainment or GOG. "Sacred" and "Sacred Gold" are trademarks of their owner. The repository contains no files of the game and does not quote its texts; it records facts about the formats and the game's behavior. Do not attach game files, converted assets or screenshots of them to issues or pull requests. Details: [docs/08-risks-legal.md](docs/08-risks-legal.md).
