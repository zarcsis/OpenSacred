# Roadmap and estimates: Claude Code time and token cost

Dollar figures are the hypothetical cost of the same tokens at Anthropic API prices; development runs on a Claude subscription, so they are not money actually spent.

## Summary

* The work is done solo and almost entirely through Claude Code, so estimates are given in **agent-hours** (an hour of active agent work) and **maintainer hours** (task setting, review, manual and gameplay checks), and the cost is estimated through token usage at API prices.
* To a playable version 1.0 for Windows 11: **≈390–770 agent-hours and ≈230–460 maintainer hours** (estimate, uncertainty up to ×2).
* At API prices the same tokens would cost ≈$3.1k–13.9k on Claude Opus 5.5, ≈$1.6k–7.7k on Claude Sonnet 5.5, ≈$2.5k–11.4k with a mix of 60% Opus and 40% Sonnet.
  On a Claude Code subscription the payment is fixed, and the pace is limited by usage limits.
* Calendar: 2.5–4.5 months in an intensive mode, 6–12 months "evenings and weekends".
  The bottleneck is the maintainer's time for review and playtesting.
* The agent-hour cost model is an estimate.
  It needs to be calibrated against actuals over the first 30 days: the usage that every pull request records from the Claude Code transcripts (`docs/usage.md`).

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
So these numbers indicate the order of magnitude, and the §1.3 model needs to be refined with the usage that `tools/usage/pr_usage.py` records in every pull request (`docs/usage.md`).

## 2. Milestones

### M0. Foundation — 6–12 agent-hours, 4–8 maintainer hours

* Cargo workspace (`opensacred-formats`, `opensacred-convert`, `opensacred-data`; `opensacred-rules`, `opensacred-game` and `opensacred-oracle` are added in their milestones), `rust-toolchain.toml`, `CLAUDE.md`, Claude Code hooks, CI on Linux.
* The SHA-256 manifest of the GOG installation; installation check in the converter.
* **Exit criterion:** `cargo build` and the tests pass in CI; the converter checks the installation.

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

* A local Ghidra 12.1.4 project on the GOG `Sacred.exe` and `GameServer.exe`, RTTI, community names; the MCP server connected to Claude Code.
  Moved here from M0: M1 and M2 do not need the original's code; Ghidra is installed earlier if a format question does.
* Study in Ghidra, the behavior described in prose in `docs/re/`, and an independent implementation in `opensacred-rules` (PLAN.md §4.2): attributes and levels, skills and combat arts, combat and damage, resistances, items and loot generation, sets, creature AI, spawning.
* `opensacred-oracle`, run locally: emulation of the formula functions of the user's own `Sacred.exe`, vectors in JSON.
* **Exit criterion:** combat with the starting region's creatures, loot, experience and levels work; the formulas are covered by vectors; deviations are recorded in `docs/decisions/`.
* **Status 2026-10-20:** the Ghidra project with the classes from RTTI, the community names and the library marks was done on 2026-10-18 and 2026-10-19 (#87, #88).
  On 2026-10-20 the spawning, the creatures' AI and the fight with experience and levels followed, in `opensacred-game` rather than `opensacred-rules` (PLAN.md §10, items 11–13; #328).
  Items, loot, skills and combat arts, and the oracle's vectors remain.

### M4. Quests and world — 60–120 agent-hours, 25–50 maintainer hours

* The NetScript interpreter in `opensacred-rules`, dialogues, triggers, NPCs, trading, the blacksmith, horses, day and night, weather.
* **Exit criterion:** the first chapter of the Sacred campaign is completable from start to finish; the 68 most frequent record types (99% of the campaign) are implemented and covered by tests.
* **Status 2026-10-20:** day and night came with M2 (#5); the townsfolk's day (#255) and the doors that open and close (#268) are done; the NetScript interpreter, dialogues, trading, the blacksmith and weather have not started.

### M5. UI and effects — 50–100 agent-hours, 20–40 maintainer hours

* Full UI: main menu, hero creation, inventory, character, skills, trading, map, journal, settings; 178 effects as data; profile-driven sound; movies.
* **Exit criterion:** all game screens are accessible from keyboard, mouse and gamepad; the effects of all skills are displayed.

### M6. Version 1.0 for Windows 11 — 40–80 agent-hours, 100–200 maintainer hours

* Saves and autosave, settings, layouts, 4K and HiDPI, polish, fixing bugs found in playthroughs.
* **Exit criterion:** both campaigns (Sacred and Underworld) completed by the maintainer without critical bugs.

### M7. HD assets — 15–30 agent-hours, 10–30 maintainer hours

* The upscale pipeline, alpha processing, KTX2 with mipmaps, path-based substitution; HD packs for the ground, characters and UI.

### M8. Platforms — 55–110 agent-hours, 20–40 maintainer hours

* Linux 5–10; Android 25–50; Chrome (WebAssembly) 25–50, by a separate decision.

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
| M8 | 55–110 | 20–40 |
| **Everything** | **≈460–910** | **≈260–530** |

### 3.1 Money at API prices (estimate)

The hypothetical cost of the same tokens at API prices, not money spent (see the note at the top):

| Mode | To version 1.0 | Everything |
|---|---|---|
| Claude Opus 5.5 only ($8–18/h) | ≈$3.1k–13.9k, center ≈$6k | ≈$3.7k–16.4k |
| Claude Sonnet 5.5 only ($4–10/h) | ≈$1.6k–7.7k | ≈$1.8k–9.1k |
| 60% Opus 5.5 + 40% Sonnet 5.5 ($6.4–14.8/h) | ≈$2.5k–11.4k | ≈$2.9k–13.5k |
| Claude Fable 5.1 only ($18–40/h) | ≈$7k–31k | ≈$8.3k–36.4k |

Which model fits where is up to the maintainer.
A sensible split: the strongest model for binary analysis, architecture and hard debugging; a cheaper one for mechanical work (converter tables, UI layout, migrations).

### 3.2 Calendar

| Mode | Agent-hours per week | Maintainer hours per week | Vertical slice | Version 1.0 |
|---|---|---|---|---|
| Intensive | 40 | 25 | ≈1 month | ≈2.5–4.5 months |
| Evenings and weekends | 15 | 10 | ≈2–3 months | ≈6–12 months |

The duration is determined by the larger of the two: agent-hours ÷ agent pace, and maintainer hours ÷ maintainer pace.

### 3.3 Critical path

```text
M0 → M1 (starting region) → M2 → M3 → M4 → M6
                 └→ M1 (rest) ─────┘   M5 in parallel with M3–M4
M7 and M8 come after M6
```

## 4. The first 30 and 90 days

### 4.1 The first 30 days

1. All of M0.
2. Textures and tile atlases → PNG; a test against the Python prototype.
3. A Granny 1.2b parser; one hero model with its skeleton in glTF, checked in Blender.
4. Reading one sector of the starting region.
5. A Bevy application: one sector and the hero model at 3840×2160.
6. Usage tracking in `docs/usage.md`; recomputing the estimates in this document.

### 4.2 The first 90 days

1. M1 for the starting region: models, animations, world, sound.
2. M2: the vertical slice with its exit criterion.
3. Start of M3: hero attributes, basic combat, the first oracle vectors.
4. Retrospective: the actual pace in functions per agent-hour, the agent-hour cost, how often Bevy breaks; a decision to continue on Bevy or switch to Godot with godot-rust (`docs/04-language-and-engine.md`, §5).

## 5. How to run the project with Claude Code

* **Task queue:** the issues on GitHub (PLAN.md §10.1, docs/03 §3.6); each task has a definition of done and a link to the notes.
* **Session = task:** Claude Code gets a task, does it, runs the checks and opens a pull request, which it merges with `--auto` once the checks are green, except those that wait for the maintainer (`needs-maintainer`); the `review-after-merge` ones the maintainer reviews after the merge (docs/03 §3.6).
  Contributors' pull requests are merged by the maintainer.
* **Definition of done:** formatting, clippy without warnings, tests, updated `docs/re/` notes when the original's behavior was studied, an entry in `docs/decisions/` when deviating from the original.
* **Daily:** the dependency update (`CLAUDE.md`, "Work order").
* **Weekly:** a usage review, the plan for the week.
* **After each milestone:** a retrospective and re-estimation of the remaining milestones from the actual pace.

## 6. What affects the estimates most

| Factor | Direction | How to find out earlier |
|---|---|---|
| Complexity of the combat and loot rules | up to ×2 on M3 | the first 50 functions of M3 |
| NetScript semantics | up to ×2 on M1 and M4 | the disassembler in the first weeks of M1 |
| Quality of the Granny animation conversion | up to ×2 on M1 and M2 | one hero animation in the first 30 days |
| Bevy API breakage | +5–20 agent-hours a year | the migration log |
| Share of rework after review | ±30% | tracking in `docs/usage.md` |

## Open questions

1. The actual cost of an agent-hour on the project's tasks (in tokens and at API prices): after 30 days of tracking.
2. The actual pace of function analysis: after the first 50 functions of M3.
3. The decision on Android and the browser: after version 1.0.

## Sources

* Claude prices: the Claude API reference (cached as of 2026-10-06).
* Data from the planning-phase runs: the orchestrator's workflow completion reports.
