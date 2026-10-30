# Legal policy and risks

## Summary

* OpenSacred is an independently written engine and converter.
  The repository contains no file of the game and nothing derived from one: every user converts their own copy of Sacred Gold 2.28, and the converted assets stay on that user's machine.
* The work on the original's formats and behavior relies on the exceptions of Directive 2009/24/EC: Article 5(3) (observing and studying the program one is entitled to use) and Article 6 (decompilation for the interoperability of an independently created program).
  Under Article 8, a contract cannot exclude them.
* The GOG build has no copy protection.
  The project circumvents nothing, and publishes nothing about circumventing a protection.
  The Steam build wraps `Sacred.exe` in Steam's DRM, which encrypts its code: the converter reads only its data sections, which the DRM leaves as they are, and decrypts nothing (docs/01 §1, #1484).
* What reaches the repository, issues and pull requests: our own code (MIT OR Apache-2.0) and facts about the formats and the behavior, in our own words.
  Never: game files, converted assets, decompiler output, game texts (§3).
  Issues and pull requests also carry frames of a problem, cropped to it, as their attachments, never in the repository's git (§3).
* Sacred is a trademark of its owner; OpenSacred is not affiliated with THQ Nordic, Ascaron or GOG (§6).

## 1. Disclaimer

This is the project's policy, not legal advice.
The applicable law depends on where you live; when in doubt, consult a lawyer.

## 2. Legal basis

| Action | Basis |
|---|---|
| Running, observing and testing one's own copy to determine the ideas and principles behind it | Directive 2009/24/EC, Art. 5(3) |
| Decompiling the parts of the code that hold the information needed to read the data and reproduce the behavior in an independently created program | Art. 6(1): only by a person entitled to use the copy, only for information not readily available otherwise, only the parts needed |
| What may be done with that information | Art. 6(2): it may not be used for other goals, given to others except where necessary for interoperability, or used for a program substantially similar in its expression. Hence the rules of §3: the information is published as facts and descriptions of behavior, never as code |
| Contract terms against these exceptions | Art. 8: null and void |
| Technical protection | not involved: the GOG `Sacred.exe` has no DRM (docs/01 §1–2); the Steam one's DRM encrypts its code, which the project neither reads nor decrypts, reading only the data sections it leaves as they are (#1484). Means of circumventing a protection are covered by Art. 7(1)(c); the project has none and publishes none |
| Format facts | not protected by copyright; code, texts and assets are |

The text of the Directive: https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32009L0024.

## 3. What the project publishes and what it never does

The same rules hold for the repository, issues, pull requests, discussions and their attachments, but for one kind of attachment: the frames of a problem (below).

| Published | Never published |
|---|---|
| Our own code under MIT OR Apache-2.0 | Any file of the game, in any form: executables, archives, models, textures, sounds, movies, saves |
| Facts about the formats (docs/02) and descriptions of the original's behavior in our own words (docs/re/) | Converted assets, atlases, glTF models, HD packs; screenshots or recordings of converted assets in the repository |
| Frames of a problem, of the game or of the original, cropped to it, as attachments of its issue or pull request (below) | Recordings with sound, and frames or sequences that show the assets for their own sake, anywhere |
| Addresses, offsets, identifiers, counts and hashes needed to read the data or to check a reader | Decompiler output (pseudo-C, listings) or code translated from it; memory dumps |
| Facts of the original's behavior that its code keeps as tables by the creature type, which voice, portrait or idle noise each type takes, written as the converter's constants, numbers only, and checked against the user's own copy by tests on the game files (#714, #1489; the maintainer's choice on #1484) | The game's own data sections, tables or resources copied whole |
| Words of the project's own that the converter puts in place of the game's texts' defects, named by the strings' ids and the original words' hashes (docs/02 §8, #403) | Another release's or a mod's wording of those texts |
| The SHA-256 manifest of the reference installation (`docs/manifest-gog-2.28.txt`), and of the Steam release's four files that differ from it (`docs/manifest-steam-2.28.txt`) | Texts of the game, its dialogues, credits or license agreements |
| Synthetic test data, such as the screenshot test's world | Code from projects without a license or under an incompatible one; leaked builds, sources or SDKs |
| | Anything about removing or circumventing a protection |

How the original's rules get into the engine: the original is studied (running it, reading its data, decompiling the parts that are needed) only to understand a format or a rule; the rule is described in prose in `docs/re/`; it is implemented independently in Rust; and the implementation is checked by tests written from the description and by observations of the running original (#227; from #448 on, made by Claude Code too), and against values recorded from the user's own executable by the oracle (PLAN.md §6.1, #363).
The oracle runs the original's machine code in an emulator: our code in it only sets the inputs and reads the outputs, and its vectors are written outside the repository, never committed.
The driver of the original (#448) runs the user's own copy and observes it (Art. 5(3)): its agent hooks functions in the running process alone, carries only addresses, offsets and the original's names, changes nothing of the original on disk, and its frames, logs and traces stay on the developer's machine, but for a frame of a problem attached to its issue or pull request (below).
Anything made locally while studying the original (Ghidra projects, notes with decompiled code, experiments) stays local and is never committed.

The frames of a problem (#1222): an issue whose problem shows on the screen carries a frame of it, and a pull request that changes what the screen shows carries its frames before and after (CLAUDE.md, "Issues and pull requests").
Each is the part of a frame of `opensacred-game`, or of the original beside it where the comparison tells, that shows the problem, cropped to it.
It is GitHub's attachment of the issue, the pull request or a comment (`gh issue create`, `gh issue comment`, `gh pr create` or `gh pr comment` with `--attach`), stored by GitHub apart from the repository: it is never committed, nor kept in the repository's git in any branch.
A frame shows the game's art, which is protected; a part of one that shows a fault of our own program, or the original's behavior beside it, quotes the game for the description of the problem (Directive 2001/29/EC, Art. 5(3)(d): a quotation for criticism or review, of a work lawfully made available, to the extent the purpose needs), as the reports of other engines' projects commonly do, and it carries none of the game's files.
The risk is low (**estimate**), and it stays apart from the repository: a complaint names the attachment, which is then deleted, and finds nothing of the game in the repository's history.

## 4. Repository and CI

* The tools read the user's own GOG Sacred Gold 2.28 installation from `OriginalGame/` (ignored by Git) or from `--game-dir`; `opensacred-convert verify` checks it against the manifest.
* The converter writes to `assets-src/` and `assets/`, which are ignored by Git as well.
* CI has no game files.
  The tests that need them are `#[ignore]` and run locally (`CLAUDE.md`, "Tests").
* The oracle (PLAN.md §6.1, #363) runs the user's own `Sacred.exe` under emulation on the developer's machine, never in CI, and checks it against the manifest first.
  It links Unicorn (GPL-2.0), so it is a separate development-only crate, a binary without a library, never linked into the converter or the game; Unicorn is its feature `emulator`, `deny.toml` allows GPL-2.0 for Unicorn's two crates alone and denies them anywhere else in the graph (docs/03 §3.4).
* The driver of the original (`tools/original/`, #448) runs the user's own `Sacred.exe` under Frida (wxWindows Library Licence 3.1) on the developer's machine, never in CI, and checks it against the manifest first.
  It is a Python tool of `tools/`, not a dependency of the converter or the game, and no release carries it.
  It runs the original in place in `OriginalGame/`, where the game writes its own files (`Settings.cfg`, `save/`, `Capture/`, its logs), never one of the manifest; what it observes reaches the repository and the issues as prose, and the issues and pull requests as the frames of a problem too (§3).
* A dependency comes only under a license that `deny.toml` allows (`CLAUDE.md`, "Prohibitions"): Apache-2.0, BSD-2-Clause, BSD-3-Clause, BSL-1.0, ISC, MIT, MIT-0, MPL-2.0, Unicode-3.0 and Zlib.
  `cargo deny check` checks the licenses on every pull request (docs/03 §3.4, §5).
* A binary release must ship with the executable the SIL Open Font License 1.1 of the Fira Mono font, which Bevy's `default_font` feature embeds, together with the licenses of the other dependencies (`cargo deny list` prints them; `THIRD-PARTY-NOTICES.md`, "Binary distributions").
* The release archive carries our own `ffmpeg.exe` (#1499, #1494), which the converter runs as a separate program: FFmpeg under the LGPL 2.1 or later, nothing GPL or non-free enabled, built statically with libopus (BSD-3-Clause) and SVT-AV1 (BSD-3-Clause-Clear with the AOMedia Patent License 1.0) from their unchanged sources, which `tools/ffmpeg/versions.toml` pins by their SHA-256, by `tools/ffmpeg/build.sh` (`.github/workflows/ffmpeg.yml`).
  Beside it go `FFMPEG.md` (the versions, the sources' places and hashes, the configuration) and the three licenses' texts; the release offers the three source archives as downloaded, as the LGPL's §4 asks of a program given in object form.
  `cargo deny` sees none of them, as they are no crates: a change of the build's sources or configuration that enables another library is checked against this paragraph by hand.
* The release archive (#1500, docs/03 §3.6) carries the licenses that the binaries owe: the crates' texts in `THIRD-PARTY-LICENSES.html`, which cargo-about writes from the licenses that `deny.toml` allows, the Fira Mono font's SIL Open Font License 1.1 (`release/FiraMono-LICENSE.txt`, Mozilla's text), our own `LICENSE-MIT` and `LICENSE-APACHE`, and FFmpeg's as above.

## 5. Community work

| Project | License | How we use it |
|---|---|---|
| open-grn (a granny.dll implementation) | MIT | a reference on the Granny 1.2b format and semantics; no code is used |
| grn2glb | MIT core, LGPL-2.1+ VTex codec | tag tables and the reading order of the MIT core, with the notice in `THIRD-PARTY-NOTICES.md`; the VTex codec is not used (FFmpeg runs as an external program) |
| openheilig | MIT code, CC BY-SA 4.0 research | layouts from the code, with the notice; facts from the research, in our own words |
| bssth/sacred-sdk | MIT | NetScript facts, the string hash, addresses; the notice is in `THIRD-PARTY-NOTICES.md` |
| kirides/sacred-bild | GPL-3.0 | function names only, in a developer's local Ghidra project; nothing is committed |
| kryotek, sacredbamboo | AGPL-3.0 and others | not used before multiplayer, after version 1.0; then facts about the protocol only, in our own words, no code |
| SacredEngineRemake | no license (all rights reserved) | not used |

Facts about formats are not protected by copyright; code is.
So from other projects we take knowledge and write our own code.
A constant table or formula taken from an MIT project carries the project's notice in `THIRD-PARTY-NOTICES.md` and a comment naming the source.

## 6. Name and trademark

* "Sacred", "Sacred Gold" and "Sacred Underworld" are trademarks of their owner; the rights to Sacred are held by THQ Nordic GmbH.
  OpenSacred is not affiliated with, endorsed by or sponsored by THQ Nordic, Ascaron Entertainment or GOG.
* The name states what the engine works with, as OpenMW and OpenGothic do for their games.
  The repository, the binaries and the icons use none of the game's logos, fonts or artwork.
* If the owner of the trademark objects to the name, the project will be renamed.

## 7. Risks

| # | Risk | Probability | Impact | Mitigation | Signal |
|---|---|---|---|---|---|
| 1 | Granny animations convert with errors | medium | high | three open readers as a reference; control poses; an early start in M1 | poses in Blender do not match the original's frames |
| 2 | Combat and loot rules are recovered incorrectly | medium | medium | oracle vectors (#363: the levels, the hit chance, the damage through a resistance and the most health so far); checks in the running original (#227); the deviation log | the balance "doesn't feel right" in play |
| 3 | NetScript semantics are harder than expected | medium | high | a disassembler early; coverage by type frequency | a quest cannot be completed |
| 4 | Bevy breaks its API more often than is worthwhile | high | medium | rules outside the engine (`opensacred-rules`, #364); scheduled migrations; the fallback is Godot + godot-rust | a migration > 10 agent-hours |
| 5 | Ecosystem crates lag behind Bevy | high | low | upgrade after the needed crates are out; keep critical parts in our own code | a crate does not build with the new Bevy |
| 6 | Token use exceeds the estimate | medium | medium | tracking from day one (docs/usage.md); a cheaper model for mechanical work | agent-hour cost above the model |
| 7 | Claude Code usage limits slow the work down | medium | medium | a task queue, scheduling heavy tasks, reviews in parallel | idle time due to limits |
| 8 | Claude Code misunderstands the original's behavior | medium | medium | vector tests, screenshot tests, review of key formulas | a test passes, but the in-game behavior is odd |
| 9 | Technical debt piles up from fast agent iterations | medium | medium | clippy, tests, milestone retrospectives, planned refactoring | task times grow |
| 10 | Maintainer burnout | medium | high | an early vertical slice; short milestones; visible progress | milestones drag on |
| 11 | HD pack size | high | low | selective upscaling; separate packs | tens of GB on disk |
| 12 | `sound.pak` in an unexpected format | retired 2026-10-16 | — | the format is confirmed by `snd_ls.py` (docs/02, §10.2) | — |
| 13 | Game content gets into the repository (a test fixture, an issue attachment, converter output) | low | high | no game bytes in the repository or CI; tests on synthetic data or on the user's local installation; the same rules for issues and pull requests, whose frames of a problem are cropped attachments, never in git (§3; PLAN.md §11) | a game file, a converted asset or a screenshot in a diff; a game file, a converted asset, a recording with sound or a frame of no problem in an issue or a pull request |
