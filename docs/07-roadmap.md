# Roadmap: the milestones

## Summary

* The work is done solo and almost entirely through Claude Code: the queue is the issues, a session takes a task, and the maintainer sets the tasks, reviews and plays.
* The milestones M0 to M6 lead to version 1.0 for Windows 11; M7 to M9 come after it.
  By 2026-11-17 M0 to M5 are done and every issue of M6 was closed; M6 then took what the documents' audit found (#2787, #2845), and version 1.0 itself waits for the maintainer's own play-through after it (PLAN.md G5).
* No estimates of time or cost are kept.
  Those of 2026-10-16, in agent-hours, maintainer hours and dollars, were dropped on 2026-11-17 as out of date (#2846).
  What the work takes is recorded as it is done, in every pull request's usage and the README's badge (`docs/usage.md`).

## 1. Milestones

### M0. Foundation

* Cargo workspace (`opensacred-formats`, `opensacred-convert`, `opensacred-data`; `opensacred-rules`, `opensacred-scripts`, `opensacred-game`, `opensacred-oracle` and `opensacred-locate` are added in their milestones), `rust-toolchain.toml`, `CLAUDE.md`, Claude Code hooks, CI on Linux.
* The SHA-256 manifest of the GOG installation; installation check in the converter.
* **Exit criterion:** `cargo build` and the tests pass in CI; the converter checks the installation.
  Met on 2026-10-16; CI took its Windows job on 2026-10-17 with the pull requests' workflow (#1).

### M1. Asset converter

* The formats: the PAK containers, the textures and the tile atlases; the Granny models (meshes, skeletons, weights, materials) and animations → glTF; the world (sectors, heights, static objects, triggers); the tables (items, creatures, weapons, balance, sets, modifiers); the texts → Fluent; the sound (`sound.pak`, profiles, music); the video through FFmpeg; NetScript → IR and listings; the manifest, the tests and incrementality.
* **Exit criterion:** 100% of the installation's files are processed; models pass glTF-Validator; the NetScript disassembler gets through 20 directories with no unknown records; a rerun gives the same result.
  All four were met on 2026-10-17 (`opensacred-convert coverage`, `gltf_validator`, `opensacred_formats::netscript`, the run manifests of two full runs).
* The order within M1 is subordinate to the vertical slice: textures, models, animations and the world of the starting region come first.

### M2. Vertical slice

* Window, orthographic camera, sector streaming, terrain with an atlas, sprite objects, water; the hero with walk and run animations; footstep sounds and region music; a minimal HUD.
* **Exit criterion:** the hero walks through the starting region at 3840×2160 at ≥ 60 fps; screenshot tests of the scene in CI.
  On 2026-10-17: about 775 fps at 3840 × 2160 (docs/05 §3.6), and the screenshot test of a synthetic scene runs in CI on lavapipe: both criteria are met.

### M3. Rules

* A local Ghidra project (made in 12.1.4, opened in 12.2 since 2026-10-24) on the GOG `Sacred.exe` and `GameServer.exe`, RTTI, community names; the MCP server connected to Claude Code.
  Moved here from M0: M1 and M2 do not need the original's code; Ghidra is installed earlier if a format question does.
* Study in Ghidra, the behavior described in prose in `docs/re/`, and an independent implementation in `opensacred-rules` (PLAN.md §4.2): attributes and levels, skills and combat arts, combat and damage, resistances, items and loot generation, sets, creature AI, spawning.
* `opensacred-oracle`, run locally: emulation of the formula functions of the user's own `Sacred.exe`, vectors in JSON.
* The converter's two stages and the game's packed assets (#694, moved here from M7 on 2026-10-23): lossless masters in `assets-src/`, the game's KTX2 with BC7 and mipmaps, Opus and ZIP archives in `assets/`.
* **Exit criterion:** combat with the starting region's creatures, loot, experience and levels work; the formulas are covered by vectors; deviations are recorded in `docs/decisions/`.
* **Status:** closed 2026-10-26 (PLAN.md §10).
  The Ghidra project with the classes from RTTI, the community names and the library marks was done on 2026-10-18 and 2026-10-19 (#87, #88).
  On 2026-10-20 the spawning, the creatures' AI and the fight with experience and levels followed, in `opensacred-game` at first, and moved into `opensacred-rules` from 2026-10-21 (#328, #364).
  The hero's items, combat arts and spells, the loot and the oracle's vectors came between 2026-10-21 and 2026-10-25.

### M4. Quests and world

* NetScript translated into Luau and run (#720), dialogues, triggers, NPCs, day and night, the weather's choice; the trading and the blacksmith moved to M5 on 2026-10-26 (#1066, #1067).
* **Exit criterion:** the first chapter of the Sacred campaign is completable from start to finish; the 68 most frequent record types (99% of the campaign) are implemented and covered by tests.
* **Status:** closed 2026-10-26 (PLAN.md §10).
  Day and night came with M2 (#5), and the townsfolk's day (#255) and the doors that open and close (#268) by 2026-10-20.
  The scripts run in Luau through the play (#726), with the dialogues (#1028) and the weather's choice (#1072).
  The first chapter was played through with the whole of both campaigns in M6 (#1099, 2026-11-10).

### M5. UI and effects

* Full UI: main menu, hero creation, inventory, character, skills, trading and the blacksmith (from M4, 2026-10-26), map, journal; the original's 178 effect classes, planned as data and made as its particle systems in code (#1092; PLAN.md §6.2); the weather drawn; profile-driven sound; movies; the saves (from M6, 2026-11-03, #1095); the settings' menu is M6's (#1096).
* **Exit criterion:** all game screens are accessible from keyboard and mouse; the effects of all skills are displayed.
  The gamepad went to M8 on 2026-11-09 (#1097).
* **Status 2026-11-09:** closed with 892 issues done; the saves came from M6 (#1095), the settings' menu stays M6's (#1096).

### M6. Version 1.0 for Windows 11

* Settings, layouts, 4K and HiDPI, polish, fixing bugs found in playthroughs; the Underworld campaign's script forms and the difficulty's remaining rules (from M5, 2026-11-03); the hero's export (from M9, 2026-11-10, #2358).
  The saves and the autosave went to M5 on 2026-11-03 (#1095), the gamepad to M8 on 2026-11-09 (#1097).
* **Exit criterion:** both campaigns (Sacred and Underworld) completed by the maintainer without critical bugs.
* **Status 2026-11-18:** every issue of the plan closed by 2026-11-17; open are the defects that the documents' audit read (#2787) and what the notes list as not followed (#2845), 104 issues, which come before the play-through by the maintainer's decisions of that day.
  Among the closed are the player's release with the Steam installation (#1494, #1484), the keys in `controls.toml` (#1553), the options window (#1096), 4K and HiDPI (#1098), the horses (#1822), the Underworld campaign (#1117) and the hero's export (#2358); both campaigns were played through to their ends in Claude Code's driven runs on 2026-11-10 (#1099).
  The exit criterion, the maintainer's own play-through, is what version 1.0 waits for (PLAN.md G5, #2479).

### M7. HD assets

* The upscale pipeline and alpha processing into masters that stage B builds (#694), path-based substitution; HD packs for the ground, characters and UI.
* **Status 2026-11-17:** research begun, on the lighting, the shading and the sprites' depth (#2622, #1437, #2604, #2621; PLAN.md §10); the upscale pipeline has not started.

### M8. Platforms

* Linux; Android, by a separate decision; the browser version was dropped on 2026-10-23.
* The gamepad, moved from M6 on 2026-11-09 by the maintainer's decision: after version 1.0, beside the other platforms' input (#1097).

### M9. Multiplayer

* After version 1.0 (PLAN.md ADR-09): the Sacred and Underworld campaigns played together over the network; the protocol, a host or a server, joining a game, the multiplayer script trees.
* How it is made is to be decided first (PLAN.md §13): speaking the original's protocol, whose game messages nobody has described (docs/09 §3.2), costs far more than a protocol of our own.

## 2. Critical path

```text
M0 → M1 (starting region) → M2 → M3 → M4 → M6
                 └→ M1 (rest) ─────┘   M5 in parallel with M3–M4
M7, M8 and M9 come after M6
```

As it went: M1 was done whole on 2026-10-17, the day the vertical slice came; M3 and M4 closed on 2026-10-26; M5 followed them and closed on 2026-11-09, with much of M6's work done beside it; every issue of M6 was closed by 2026-11-17, the thirty-third day.

## 3. The first 30 and 90 days

The plan of 2026-10-16 with what came of it; its items on the estimates left with them.

### 3.1 The first 30 days

1. All of M0.
2. Textures and tile atlases → PNG; a test against the Python prototype.
3. A Granny 1.2b parser; one hero model with its skeleton in glTF, checked in Blender.
4. Reading one sector of the starting region.
5. A Bevy application: one sector and the hero model at 3840×2160.
6. Usage tracking in `docs/usage.md`.

All six were done by 2026-10-17.

### 3.2 The first 90 days

1. M1 for the starting region: models, animations, world, sound.
2. M2: the vertical slice with its exit criterion.
3. Start of M3: hero attributes, basic combat, the first oracle vectors.
4. Retrospective: how often Bevy breaks; a decision to continue on Bevy or switch to Godot with godot-rust (`docs/04-language-and-engine.md`, §5).

Items 1 to 3 were done by 2026-10-21, the sixth day.
Of item 4: Bevy has not broken yet, the game being on 0.20 since its first day, and it stays on Bevy, none of the triggers of PLAN.md §5.3 having come.

## 4. How to run the project with Claude Code

* **Task queue:** the issues on GitHub (PLAN.md §10.1, docs/03 §3.6); each task has a definition of done and a link to the notes.
* **Session = task:** Claude Code gets a task, does it, runs the checks and opens a pull request, which it merges with `--auto` once the checks are green, except those that wait for the maintainer (`needs-maintainer`, docs/03 §3.6).
  Contributors' pull requests are merged by the maintainer.
* **Definition of done:** formatting, clippy without warnings, tests, the mutants of the changed lines of the crates without Bevy (`CLAUDE.md`, "Before finishing a task"), updated `docs/re/` notes when the original's behavior was studied, an entry in `docs/decisions/` when deviating from the original.
* **Daily:** the dependency update, made by the Dependencies workflow without a session (#1550; `CLAUDE.md`, "Work order").
* **Weekly:** a usage review, the plan for the week.

## Open questions

1. The decision on Android: after version 1.0.
2. How multiplayer is made: when M9 starts (PLAN.md §13).
