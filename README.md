# OpenSacred

[![Claude Code usage](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2Fzarcsis%2FOpenSacred%2Fbadges%2Fusage.json)](docs/usage.md)

An open-source engine for **Sacred Gold** (Sacred and its expansion Underworld, the GOG release 2.28), written in Rust on [Bevy](https://bevyengine.org/). The goal is to play Sacred and Underworld comfortably on modern hardware: Windows 11 x64 at 4K first, later Linux, possibly Android.

OpenSacred contains none of the game's files. You need your own copy of Sacred Gold from GOG:

- an offline converter reads your installation once and turns its assets into open formats (PNG and KTX2, glTF, FLAC and Ogg Opus, AV1/Opus WebM, RON, Fluent);
- the engine reads only those formats;
- the game's rules are reimplemented from documented behavior ([docs/re/](docs/re/)); deliberate deviations from the original are recorded in [docs/decisions/](docs/decisions/).

Multiplayer comes after version 1.0; until then there is no networking. Original save files are not loaded. The full rationale is in [PLAN.md](PLAN.md).

## Status

| Milestone | State |
|---|---|
| M0 Foundation: Cargo workspace, CI, the SHA-256 manifest of the reference installation | done |
| M1 Converter: textures, Granny models and animations → glTF, world sectors, sound, texts, videos, interface, tables, hero templates, scripts | done |
| M2 Vertical slice: streamed sectors with ground, water and objects, the hero walking and running, the start script's NPCs, footsteps, music and ambience, day and night, the buildings' insides, the original's cursor; menus, settings, a console and remote control | done |
| M3 Rules: the Ghidra project; the world's creatures spawned and living as in the original; the fight of the creatures and the hero, health, death, experience and levels | in progress |
| M4 Quests and world: the townsfolk's day, the doors | started |
| M5–M6: the full interface, version 1.0 for Windows 11 | planned |

It is not a game you can play through yet. The plan and the queue are in [PLAN.md](PLAN.md), §9–§10.

## Requirements

- **The game:** your own copy of Sacred Gold 2.28 (English) from GOG. The converter checks it against the embedded SHA-256 manifest ([docs/manifest-gog-2.28.txt](docs/manifest-gog-2.28.txt)); other builds are not supported.
- **Rust:** [rustup](https://rustup.rs/); the exact toolchain is pinned in [rust-toolchain.toml](rust-toolchain.toml) and installs automatically.
- **Windows:** Visual Studio Build Tools 2026 with the "Desktop development with C++" workload and the Windows SDK. The workspace links with `rust-lld` ([.cargo/config.toml](.cargo/config.toml)).
- **FFmpeg** in `PATH`: the converter runs it for the sound, the videos and a few textures.
- **Cargo tools** for development: `cargo install cargo-nextest cargo-deny`.
- **Python 3.15** (optional): only for the research scripts in [tools/](tools/README.md) and the tests that cross-check against them; `pip install -r tools/requirements.txt`.

The full tool list is in [docs/03-toolchain.md](docs/03-toolchain.md).

## Getting started

```bash
git clone https://github.com/zarcsis/OpenSacred.git
cd OpenSacred
# Copy or link your GOG installation of Sacred Gold to OriginalGame/ (ignored by Git),
# or pass --game-dir <path> to every converter command.
cargo run --release -p opensacred-convert -- verify   # check the installation against the manifest
cargo run --release -p opensacred-convert -- all      # convert everything into assets/ (ignored by Git)
cargo run --release -p opensacred-game                # the game, from its menu: the left button moves and attacks, R walks/runs, the wheel zooms, Esc pauses, F1 lists the keys
```

The converter's commands: `verify`, `textures`, `models`, `world`, `sound`, `text`, `video`, `ui`, `tables`, `heroes`, `netscript`, `all` (every conversion in order, skipping those whose output is up to date; `--force` runs them all) and `coverage` (what becomes of each file of the installation). `--help` describes each. The game takes `--class` (a game at once, skipping the menu), `--sector X,Y`, `--zoom`, `--time HH:MM`, `--windowed`/`--borderless`, `--width`/`--height`, `--no-music`, `--screenshot`, `--exec` and `--remote`; its `--help` lists the rest.

The converted assets are made from your copy of the game. Keep them for yourself: do not publish or share them (see [docs/08-risks-legal.md](docs/08-risks-legal.md)).

## Repository layout

| Path | Contents |
|---|---|
| `crates/opensacred-formats` | readers of the original formats (PAK, TEX, GRN, sectors, NetScript, `global.res`, AMH); parsing only |
| `crates/opensacred-data` | open-format types shared by the converter and the game |
| `crates/opensacred-convert` | the converter CLI |
| `crates/opensacred-game` | the game on Bevy |
| `docs/` | design documents, format descriptions, recovered behavior |
| `tools/` | research scripts (the reference prototypes for the Rust readers), the Ghidra project's scripts, the game's remote-control client, and the process scripts (usage, stacked pull requests) |

## Tests

```bash
cargo nextest run --workspace                       # without the game files, as in CI
cargo nextest run --workspace --run-ignored all     # plus the tests on OriginalGame/, the Python prototypes and the GPU
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check                                    # licenses, advisories, sources
```

Tests that need the game files are marked `#[ignore]`: CI does not have them. Cross-check tests run the interpreter from the `PYTHON` environment variable (default `python` on Windows, `python3` elsewhere).

## Documentation

| Document | Contents |
|---|---|
| [PLAN.md](PLAN.md) | the main plan: goals, decisions, architecture, estimates, risks |
| [docs/02-data-formats.md](docs/02-data-formats.md) | the original data formats and their conversion targets |
| [docs/03-toolchain.md](docs/03-toolchain.md) | tools, workstation setup, CI, the workflow with Claude Code |
| [docs/08-risks-legal.md](docs/08-risks-legal.md) | the legal policy: what the project publishes and what it never does |
| [tools/README.md](tools/README.md) | the research scripts and how to run them |
| [CLAUDE.md](CLAUDE.md) | conventions for development with Claude Code |

The full index is in [PLAN.md](PLAN.md), §14.

## Contributing

Work goes through [issues](https://github.com/zarcsis/OpenSacred/issues) and pull requests. Open an issue from the form of its type (bug, feature, research, dependency, infrastructure, documentation), branch from `main`, and open a pull request from the template; it is merged once the checks (Linux, Windows, cargo deny) are green and the maintainer has looked at it. CI has no game files, so run the tests on your own installation (`cargo nextest run --workspace --run-ignored all`) before opening it. The process is described in [docs/03-toolchain.md](docs/03-toolchain.md), §3.6.

## License

The code is licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option. Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions. Notices of the community projects whose facts the readers follow are in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

## Legal

OpenSacred is not affiliated with, endorsed by or sponsored by THQ Nordic, Ascaron Entertainment or GOG. "Sacred" and "Sacred Gold" are trademarks of their owner. The repository contains no files of the game and does not quote its texts; it records facts about the formats and the game's behavior. Do not attach game files, converted assets or screenshots of them to issues or pull requests. Details: [docs/08-risks-legal.md](docs/08-risks-legal.md).
