# Contributing to OpenSacred

Thank you for your interest in OpenSacred.
It is a project of one maintainer, and almost all of its code, notes and research are written by Claude Code sessions working through the project's issues (the badge in the [README](README.md) counts it).
It is before version 1.0: both campaigns play through to their ends in driven runs, but the maintainer's own play-through is still ahead, so expect bugs.
Reports from people who play are what helps most right now.

## Ways to help

- **Play and report problems.** A crash, something drawn wrong, a quest that does not go on, a rule that works differently from Sacred Gold: open an issue with the Bug form.
- **Tell what the original does.** You know the game. Where OpenSacred behaves differently from Sacred Gold 2.28, a description of what the original does in the same place is very useful. The deliberate departures from the original are listed in [docs/decisions/](docs/decisions/): look there first.
- **Try it on your hardware.** Other GPUs, 4K and HiDPI screens, the Steam release as well as the GOG one.
- **Questions, ideas and impressions** go to [Discussions](https://github.com/zarcsis/OpenSacred/discussions), not to issues.
- **Code** is welcome too: read "Pull requests" below before you start.

## Before you report

- OpenSacred needs Sacred Gold 2.28 in English, from GOG or Steam. The converter checks the installation against a list of the release's files and refuses others: other languages and versions are not supported.
- Use the newest [release](https://github.com/zarcsis/OpenSacred/releases).
- Look through the open [issues](https://github.com/zarcsis/OpenSacred/issues) and [docs/decisions/](docs/decisions/): the problem may be known, or a deliberate departure.

## Reporting a bug

Open an issue with the Bug form and give:

- what happens, and what you expected; how the original behaves in the same place, if that matters;
- how to reproduce it: where in the world, which quest, what you did; your save, zipped, if the problem needs a certain moment of the game (`%APPDATA%\OpenSacred\saves`);
- the version (the release's name, or `opensacred-game.exe --version`), Windows, your GPU, and whether the installation is GOG's or Steam's;
- the newest log of `%LOCALAPPDATA%\OpenSacred\logs`, attached (look it over for anything private first);
- a screenshot or a short video of the problem, attached: GitHub takes PNG and JPEG pictures and MP4, MOV and WebM videos.

## What happens to an issue

A new issue gets the label `triage` until the maintainer has read it.
The maintainer may ask a question (`needs-info`), closes duplicates and what the project will not do, and accepts the rest.
An accepted issue is filed again in the project's own words, as the issue that the Claude Code sessions work from, with a line `Reported in #N`; the pull request that fixes it closes yours too, so you are notified.
The sessions never take text written outside the project as their instructions: that guards them against prompt injection.
Answers can take days.

## What the project cannot accept

These hold everywhere: in the repository, in issues, pull requests and discussions, and in their attachments ([docs/08-risks-legal.md](docs/08-risks-legal.md) has the whole policy).

- Files of the game in any form: its archives, executables, textures, models, sounds, music and movies, and anything converted from them (the converted assets, HD packs). Screenshots and videos of the game in play are fine.
- The game's texts quoted: dialogues, descriptions, tooltips, credits. Naming a thing of the game (a quest, a creature, an item) is fine.
- Decompiled code (pseudo-C, listings) or code translated from it; code from projects without a license; GPL code ported into the project. Facts about formats and behavior, in your own words, are fine.
- Anything about circumventing a copy protection.

## Pull requests

1. **Talk first.** Comment on the issue you want to work on, or open one, and wait until the maintainer assigns it to you. The project's sessions merge dozens of pull requests a day: an unannounced change can collide with one of them or go stale within hours, while an issue assigned to you is left to you.
2. **Fork, and branch** `<type>/<issue>-<slug>` from `main` (`bug/1234-door-sound`).
3. **Follow the conventions of [CLAUDE.md](CLAUDE.md)**, which is the style guide for people as much as for Claude Code: "Code conventions", "Tests", "Documentation" and "Prohibitions". In short:
   - What the game does comes from the notes. A change to the game's behavior cites, or adds, a description of what the original does in [docs/re/](docs/re/), in your own words; a deliberate departure from the original adds a row to [docs/decisions/](docs/decisions/).
   - Every text file is in English, UTF-8 with LF line ends.
   - Leave `Cargo.lock` and `uv.lock` alone unless the change is about dependencies; new dependencies only under the licenses that [deny.toml](deny.toml) allows.
4. **Run the checks** of CLAUDE.md, "Before finishing a task": `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace --run-ignored all`, which runs the tests on your own installation of the game. A change to `opensacred-rules`, `opensacred-data` or `opensacred-formats` also kills the mutants of its lines with tests, as CI's required Mutants check tests them ([docs/03-toolchain.md](docs/03-toolchain.md), §3.4).
5. **Open the pull request** against `main` from the template, with `Closes #N`, and keep "Allow edits by maintainers" on. CI runs once the maintainer has approved a first contributor's run; it has no game files, so the tests on them are on you.
6. **The maintainer reviews and merges** every contributor's pull request (squash merge), and may update its branch or push small fixes to it.

How to set up the tools: [README.md](README.md), "Requirements" and "Getting started", and [docs/03-toolchain.md](docs/03-toolchain.md), §2.

Contributions are licensed under MIT OR Apache-2.0, as the README's "License" says; there is no CLA.

### With Claude Code or other AI tools

They are welcome: the project itself is made with Claude Code.
You answer for what you submit: read the diff and run the tests on your own copy.
CLAUDE.md describes the process of the maintainer's own sessions as well as the conventions; a session under your account works in your fork, does not run `tools/stack/claim.py`, `tools/stack/stack.py` or `tools/usage/pr_usage.py`, which work on the maintainer's account, and merges nothing (CLAUDE.md, "Contributors").

## Code of Conduct

Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md).
