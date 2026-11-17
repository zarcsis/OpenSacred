# Roadmap and estimates: Claude Code time and token cost

Dollar figures are the hypothetical cost of the same tokens at Anthropic API prices; development runs on a Claude subscription, so they are not money actually spent.

## Summary

* The work is done solo and almost entirely through Claude Code, so estimates are given in **agent-hours** (an hour of active agent work) and **maintainer hours** (task setting, review, manual and gameplay checks), and the cost is estimated through token usage at API prices.
* To a playable version 1.0 for Windows 11: **≈390–770 agent-hours and ≈230–460 maintainer hours** (estimate, uncertainty up to ×2).
* At API prices the same tokens would cost ≈$3.1k–13.9k on Claude Opus 5.5, ≈$1.6k–7.7k on Claude Sonnet 5.5, ≈$2.5k–11.4k with a mix of 60% Opus and 40% Sonnet.
  On a Claude Code subscription the payment is fixed, and the pace is limited by usage limits.
* Calendar: 2.5–4.5 months in an intensive mode, 6–12 months "evenings and weekends".
  The bottleneck is the maintainer's time for review and playtesting.
* The agent-hour cost model is an estimate of 2026-10-16, like every range of this document.
  The usage that every pull request records from the Claude Code transcripts (`docs/usage.md`) gives the actuals: by 2026-11-17, with M0 to M5 done and every issue of M6 closed, ≈432 agent-hours and ≈$11.5k at API prices, ≈$27 an agent-hour (§1.4).
  Version 1.0 itself waits for the maintainer's own play-through (PLAN.md G5).

## 1. Estimation model

### 1.1 Units

| Unit | Definition |
|---|---|
| Agent-hour | an hour during which Claude Code actively works on a task: reads code and decompilation, writes, runs builds and tests. Parallel sessions add up |
| Maintainer hour | setting a task, reviewing the result, manual checks, playtesting, decisions |
| Task | one finished result verified by a test, 1–4 agent-hours |

### 1.2 Productivity assumptions

| Kind of work | Assumption (estimate) |
|---|---|
| Studying game functions of average length (≈600 B of code), describing their behavior and implementing it anew | 8–15 functions per agent-hour, including the prose notes, oracle vectors and tests |
| Analysis scope | 2,000–3,000 of the ≈5,900 game functions (rules, items, AI, scripts); the rest is written from scratch without reading the code |
| Parsing a format and the converter | 2–30 agent-hours per format depending on complexity (TEX takes days, Granny weeks) |
| Engine systems on Bevy | 5–30 agent-hours per system (camera, streaming, animations, a UI screen) |
| Migrating to a new Bevy | 2–6 agent-hours per release, ≈3 releases a year |
| Share of rework after review | 20–30% on top of the net work, already included in the milestones |

### 1.3 Agent-hour cost

API prices as of 2026-10-16, in dollars per million tokens (the cost is hypothetical, see the note at the top):

| Model | Input | Output | Cache read | Cache write (5 min) |
|---|---|---|---|---|
| Claude Opus 5.5 | 4 | 20 | 0.20 | 5 |
| Claude Sonnet 5.5 | 2 | 10 | 0.20 | 2.50 |
| Claude Fable 5.1 | 10 | 50 | 0.25 | 12.50 |

Claude Code writes the cache for an hour, at $8 for Opus 5.5, which the recorded usage takes (docs/usage.md); the model below keeps the 5 minutes' price, as it was made on 2026-10-16.

Agent-hour profile (estimate): output including reasoning: 0.2–0.5M tokens; cache reads: 5–15M (the context is reread at every step); cache writes: 0.5–1M; uncached input is small.

```text
agent_hour_cost ≈ output × output_price + cache_read × read_price + cache_write × write_price
Opus 5.5:   0.2–0.5 × 20 + 5–15 × 0.20 + 0.5–1 × 5     ≈  $8–18
Sonnet 5.5: 0.2–0.5 × 10 + 5–15 × 0.20 + 0.5–1 × 2.50  ≈  $4–10
Fable 5.1:  0.2–0.5 × 50 + 5–15 × 0.25 + 0.5–1 × 12.50 ≈ $18–40
```

### 1.4 Calibration on this project

The planning phase was carried out by multi-agent Claude Code runs, mostly on Claude Fable 5.1:

| Run | Agents | Tool calls | Duration | Tokens by the orchestrator's counter |
|---|---|---|---|---|
| Research (9 topics + 6 follow-ups) | 16 | 1,502 | ≈4 h | 5.1M |
| Writing the documents and two rounds of critique (the last run) | 35 | 1,124 | ≈1.4 h | 6.5M |
| A later revision of the documents | 32 | 1,494 | ≈1.5 h | 6.6M |

What exactly the orchestrator's counter includes (only output with reasoning, or all input) has not been verified.
So these numbers indicate the order of magnitude, and the §1.3 model is measured against the usage that `tools/usage/pr_usage.py` records in every pull request (`docs/usage.md`).

The recorded usage: the pull requests' blocks merged by the end of each day, with the sessions before them (12.7 agent-hours, `docs/usage.md`), summed by `tools/usage/usage_badge.py` on 2026-11-17 over every pull request of the maintainer's account, 1,465 of them with a block.
A pull request counts on the day it merged (UTC); the subagents' time is in no agent-hour, their tokens in every sum.

| By the end of | Closed since the row above | Agent-hours | Tokens, B | At API prices, $ | Estimate for all the milestones closed by then, agent-hours |
|---|---|---|---|---|---|
| 2026-10-17 | M0, M1; G2 met | 12.7 | 1.03 | 338 | 86–172 |
| 2026-10-23 | M2 | 87.3 | 8.99 | 2,679 | 116–232 |
| 2026-10-26 | M3, M4 | 136.4 | 14.28 | 4,113 | 296–592 |
| 2026-11-09 | M5 | 358.6 | 37.09 | 10,150 | 346–692 |
| 2026-11-17 | every issue of M6 | 432.4 | 42.29 | 11,498 | 386–772 (≈390–770) |

The milestones overlapped (M3's work began before M2 closed, M6's before M5 did), so a row is the sum to that day, not one milestone's cost.
To the last issue of M6 the work took agent-hours near the low end of the estimate and a cost at API prices near its high end (≈$3.1k–13.9k, §3.1).
An agent-hour cost ≈$27 against the model's $8–18: its output (0.17M tokens) and its cache writes (0.59M) are at or under the model's 0.2–0.5M and 0.5–1M, while its cache reads are 97M against 5–15M and make about three quarters of the cost, the context of long sessions reread at every step.
All but a little of the work ran on Claude Opus 5.5.
The maintainer's hours were not recorded.
The estimates below were not recomputed 30 days after 2026-10-16, as §4.1 planned: those of M0 to M6 are overtaken by this table, and those of M7 and M8 stand as of 2026-10-16.

## 2. Milestones

### M0. Foundation — 6–12 agent-hours, 4–8 maintainer hours

* Cargo workspace (`opensacred-formats`, `opensacred-convert`, `opensacred-data`; `opensacred-rules`, `opensacred-scripts`, `opensacred-game`, `opensacred-oracle` and `opensacred-locate` are added in their milestones), `rust-toolchain.toml`, `CLAUDE.md`, Claude Code hooks, CI on Linux.
* The SHA-256 manifest of the GOG installation; installation check in the converter.
* **Exit criterion:** `cargo build` and the tests pass in CI; the converter checks the installation.
  Met on 2026-10-16; CI took its Windows job on 2026-10-17 with the pull requests' workflow (#1).

### M1. Asset converter — 80–160 agent-hours, 25–50 maintainer hours

| Format | Agent-hours |
|---|---|
| PAK containers, textures, tile atlases | 6–12 |
| Granny models → glTF (meshes, skeletons, weights, materials) | 15–30 |
| Granny animations → glTF clips | 10–20 |
| World: sectors, heights, static objects, triggers | 12–25 |
| Tables: items, creatures, weapons, balance, sets, modifiers | 10–20 |
| Texts → Fluent | 2–4 |
| Sound: `sound.pak`, profiles, music | 4–10 |
| Video through FFmpeg | 2–4 |
| NetScript → IR and listings | 15–30 |
| Manifest, tests, incrementality | 4–8 |

* **Exit criterion:** 100% of the installation's files are processed; models pass glTF-Validator; the NetScript disassembler gets through 20 directories with no unknown records; a rerun gives the same result.
  All four were met on 2026-10-17 (`opensacred-convert coverage`, `gltf_validator`, `opensacred_formats::netscript`, the run manifests of two full runs).
* The order within M1 is subordinate to the vertical slice: textures, models, animations and the world of the starting region come first.

### M2. Vertical slice — 30–60 agent-hours, 15–30 maintainer hours

* Window, orthographic camera, sector streaming, terrain with an atlas, sprite objects, water; the hero with walk and run animations; footstep sounds and region music; a minimal HUD.
* **Exit criterion:** the hero walks through the starting region at 3840×2160 at ≥ 60 fps; screenshot tests of the scene in CI.
  On 2026-10-17: about 775 fps at 3840 × 2160 (docs/05 §3.6), and the screenshot test of a synthetic scene runs in CI on lavapipe: both criteria are met.

### M3. Rules — 120–240 agent-hours, 40–80 maintainer hours

* A local Ghidra project (made in 12.1.4, opened in 12.2 since 2026-10-24) on the GOG `Sacred.exe` and `GameServer.exe`, RTTI, community names; the MCP server connected to Claude Code.
  Moved here from M0: M1 and M2 do not need the original's code; Ghidra is installed earlier if a format question does.
* Study in Ghidra, the behavior described in prose in `docs/re/`, and an independent implementation in `opensacred-rules` (PLAN.md §4.2): attributes and levels, skills and combat arts, combat and damage, resistances, items and loot generation, sets, creature AI, spawning.
* `opensacred-oracle`, run locally: emulation of the formula functions of the user's own `Sacred.exe`, vectors in JSON.
* The converter's two stages and the game's packed assets (#694, moved here from M7 on 2026-10-23): lossless masters in `assets-src/`, the game's KTX2 with BC7 and mipmaps, Opus and ZIP archives in `assets/`; the milestone's estimate was left as it stood.
* **Exit criterion:** combat with the starting region's creatures, loot, experience and levels work; the formulas are covered by vectors; deviations are recorded in `docs/decisions/`.
* **Status:** closed 2026-10-26 (PLAN.md §10).
  The Ghidra project with the classes from RTTI, the community names and the library marks was done on 2026-10-18 and 2026-10-19 (#87, #88).
  On 2026-10-20 the spawning, the creatures' AI and the fight with experience and levels followed, in `opensacred-game` at first, and moved into `opensacred-rules` from 2026-10-21 (#328, #364).
  The hero's items, combat arts and spells, the loot and the oracle's vectors came between 2026-10-21 and 2026-10-25.

### M4. Quests and world — 60–120 agent-hours, 25–50 maintainer hours

* NetScript translated into Luau and run (#720), dialogues, triggers, NPCs, day and night, the weather's choice; the trading and the blacksmith moved to M5 on 2026-10-26 (#1066, #1067).
* **Exit criterion:** the first chapter of the Sacred campaign is completable from start to finish; the 68 most frequent record types (99% of the campaign) are implemented and covered by tests.
* **Status:** closed 2026-10-26 (PLAN.md §10).
  Day and night came with M2 (#5), and the townsfolk's day (#255) and the doors that open and close (#268) by 2026-10-20.
  The scripts run in Luau through the play (#726), with the dialogues (#1028) and the weather's choice (#1072).
  The first chapter was played through with the whole of both campaigns in M6 (#1099, 2026-11-10).

### M5. UI and effects — 50–100 agent-hours, 20–40 maintainer hours

* Full UI: main menu, hero creation, inventory, character, skills, trading and the blacksmith (from M4, 2026-10-26), map, journal; the original's 178 effect classes, planned as data and made as its particle systems in code (#1092; PLAN.md §6.2); the weather drawn; profile-driven sound; movies; the saves (from M6, 2026-11-03, #1095); the settings' menu is M6's (#1096).
* **Exit criterion:** all game screens are accessible from keyboard and mouse; the effects of all skills are displayed.
  The gamepad went to M8 on 2026-11-09 (#1097).
* **Status 2026-11-09:** closed with 892 issues done; the saves came from M6 (#1095), the settings' menu stays M6's (#1096).

### M6. Version 1.0 for Windows 11 — 40–80 agent-hours, 100–200 maintainer hours

* Settings, layouts, 4K and HiDPI, polish, fixing bugs found in playthroughs; the Underworld campaign's script forms and the difficulty's remaining rules (from M5, 2026-11-03); the hero's export (from M9, 2026-11-10, #2358).
  The saves and the autosave went to M5 on 2026-11-03 (#1095), the gamepad to M8 on 2026-11-09 (#1097).
* **Exit criterion:** both campaigns (Sacred and Underworld) completed by the maintainer without critical bugs.
* **Status 2026-11-17:** every issue closed, the milestone open.
  Among them the player's release with the Steam installation (#1494, #1484), the keys in `controls.toml` (#1553), the options window (#1096), 4K and HiDPI (#1098), the horses (#1822), the Underworld campaign (#1117) and the hero's export (#2358); both campaigns were played through to their ends in Claude Code's driven runs on 2026-11-10 (#1099).
  The exit criterion, the maintainer's own play-through, is what version 1.0 waits for (PLAN.md G5, #2479).

### M7. HD assets — 15–30 agent-hours, 10–30 maintainer hours

* The upscale pipeline and alpha processing into masters that stage B builds (#694), path-based substitution; HD packs for the ground, characters and UI.
* **Status 2026-11-17:** research begun, on the lighting, the shading and the sprites' depth (#2622, #1437, #2604, #2621; PLAN.md §10); the upscale pipeline has not started.

### M8. Platforms — 30–60 agent-hours, 10–20 maintainer hours

* Linux 5–10; Android 25–50, by a separate decision; the browser version was dropped on 2026-10-23.
* The gamepad, moved from M6 on 2026-11-09 by the maintainer's decision: after version 1.0, beside the other platforms' input (#1097).

### M9. Multiplayer — not estimated yet

* After version 1.0 (PLAN.md ADR-09): the Sacred and Underworld campaigns played together over the network; the protocol, a host or a server, joining a game, the multiplayer script trees.
* Estimated once it is decided how it is made (PLAN.md §13): speaking the original's protocol, whose game messages nobody has described (docs/09 §3.2), costs far more than a protocol of our own.

## 3. Totals

| Milestone | Agent-hours | Maintainer hours |
|---|---|---|
| M0 | 6–12 | 4–8 |
| M1 | 80–160 | 25–50 |
| M2 | 30–60 | 15–30 |
| M3 | 120–240 | 40–80 |
| M4 | 60–120 | 25–50 |
| M5 | 50–100 | 20–40 |
| M6 | 40–80 | 100–200 |
| **To version 1.0** | **≈390–770** | **≈230–460** |
| M7 | 15–30 | 10–30 |
| M8 | 30–60 | 10–20 |
| M9 | not estimated | not estimated |
| **Everything but M9** | **≈435–860** | **≈250–510** |

### 3.1 Money at API prices (estimate)

The hypothetical cost of the same tokens at API prices, not money spent (see the note at the top):

| Mode | To version 1.0 | Everything but M9 |
|---|---|---|
| Claude Opus 5.5 only ($8–18/h) | ≈$3.1k–13.9k, center ≈$6k | ≈$3.5k–15.5k |
| Claude Sonnet 5.5 only ($4–10/h) | ≈$1.6k–7.7k | ≈$1.7k–8.6k |
| 60% Opus 5.5 + 40% Sonnet 5.5 ($6.4–14.8/h) | ≈$2.5k–11.4k | ≈$2.8k–12.7k |
| Claude Fable 5.1 only ($18–40/h) | ≈$7k–31k | ≈$7.8k–34.4k |

Which model fits where is up to the maintainer.
A sensible split: the strongest model for binary analysis, architecture and hard debugging; a cheaper one for mechanical work (converter tables, UI layout, migrations).

### 3.2 Calendar

| Mode | Agent-hours per week | Maintainer hours per week | Vertical slice | Version 1.0 |
|---|---|---|---|---|
| Intensive | 40 | 25 | ≈1 month | ≈2.5–4.5 months |
| Evenings and weekends | 15 | 10 | ≈2–3 months | ≈6–12 months |

The duration is determined by the larger of the two: agent-hours ÷ agent pace, and maintainer hours ÷ maintainer pace.

As it went: the vertical slice came on the second day, 2026-10-17, and M0 to M5 were done and every issue of M6 closed by 2026-11-17, the thirty-third, at ≈90 agent-hours a week (§1.4); version 1.0 waits for the maintainer's play-through.

### 3.3 Critical path

```text
M0 → M1 (starting region) → M2 → M3 → M4 → M6
                 └→ M1 (rest) ─────┘   M5 in parallel with M3–M4
M7, M8 and M9 come after M6
```

As it went: M1 was done whole on 2026-10-17, the day the vertical slice came; M3 and M4 closed on 2026-10-26; M5 followed them and closed on 2026-11-09, with much of M6's work done beside it.

## 4. The first 30 and 90 days

The plan of 2026-10-16, kept as it was made, with what came of it.

### 4.1 The first 30 days

1. All of M0.
2. Textures and tile atlases → PNG; a test against the Python prototype.
3. A Granny 1.2b parser; one hero model with its skeleton in glTF, checked in Blender.
4. Reading one sector of the starting region.
5. A Bevy application: one sector and the hero model at 3840×2160.
6. Usage tracking in `docs/usage.md`; recomputing the estimates in this document.

Items 1 to 5 and the usage tracking were done by 2026-10-17.
The estimates were not recomputed: the recorded usage beside them is in §1.4.

### 4.2 The first 90 days

1. M1 for the starting region: models, animations, world, sound.
2. M2: the vertical slice with its exit criterion.
3. Start of M3: hero attributes, basic combat, the first oracle vectors.
4. Retrospective: the actual pace in functions per agent-hour, the agent-hour cost, how often Bevy breaks; a decision to continue on Bevy or switch to Godot with godot-rust (`docs/04-language-and-engine.md`, §5).

Items 1 to 3 were done by 2026-10-21, the sixth day.
Of item 4: the agent-hour cost is in §1.4; the pace in functions per agent-hour was not measured; Bevy has not broken yet, the game being on 0.20 since its first day, and it stays on Bevy, none of the triggers of PLAN.md §5.3 having come.

## 5. How to run the project with Claude Code

* **Task queue:** the issues on GitHub (PLAN.md §10.1, docs/03 §3.6); each task has a definition of done and a link to the notes.
* **Session = task:** Claude Code gets a task, does it, runs the checks and opens a pull request, which it merges with `--auto` once the checks are green, except those that wait for the maintainer (`needs-maintainer`, docs/03 §3.6).
  Contributors' pull requests are merged by the maintainer.
* **Definition of done:** formatting, clippy without warnings, tests, the mutants of the changed lines of the crates without Bevy (`CLAUDE.md`, "Before finishing a task"), updated `docs/re/` notes when the original's behavior was studied, an entry in `docs/decisions/` when deviating from the original.
* **Daily:** the dependency update, made by the Dependencies workflow without a session (#1550; `CLAUDE.md`, "Work order").
* **Weekly:** a usage review, the plan for the week.
* **After each milestone:** a retrospective and re-estimation of the remaining milestones from the actual pace, as planned on 2026-10-16; none was written for M0 to M6, whose recorded usage beside the estimates is in §1.4.

## 6. What affects the estimates most

| Factor | Direction | How to find out earlier |
|---|---|---|
| Complexity of the combat and loot rules | up to ×2 on M3 | the first 50 functions of M3 |
| NetScript semantics | up to ×2 on M1 and M4 | the disassembler in the first weeks of M1 |
| Quality of the Granny animation conversion | up to ×2 on M1 and M2 | one hero animation in the first 30 days |
| Bevy API breakage | +5–20 agent-hours a year | the migration log |
| Share of rework after review | ±30% | tracking in `docs/usage.md` |

## Open questions

1. The actual pace of function analysis, in functions per agent-hour: not measured; M3's issues and pull requests hold what it would be counted from.
2. The decision on Android: after version 1.0.
3. How multiplayer is made, and its estimate: when M9 starts (PLAN.md §13).

Closed on 2026-11-17: the actual cost of an agent-hour on the project's tasks, ≈98M tokens and ≈$27 at API prices (§1.4).

## Sources

* Claude prices: the Claude API reference (cached as of 2026-10-06).
* Data from the planning-phase runs: the orchestrator's workflow completion reports.
