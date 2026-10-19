# Tools to test and drive the game

## Summary

* **Why:** everything the game can be told today is a command line option, read before Bevy starts; the hero's class is fixed for the process, and a change means a restart.
  The tools let the maintainer test the game as a game, and let Claude Code drive a running game by itself.
* **Settings:** `settings.toml` in the user's directory; borderless full screen by default, F11 for a window, both kept.
* **Menu:** a main menu (New Game, Quit) and the choice of the hero, which needs the game to start and end inside one process: states, and a loading in the background.
  Esc in the game pauses it under a small menu: Continue, Main menu, Quit.
* **On the screen:** the HUD's portrait, health bar and gait go; F3 shows a debug overlay (the frame rate and times, the hero's place), F4 the log, which a file keeps too.
* **Commands:** one set of commands, which the command line's scripts, the console on `~` and the remote protocol run alike; the command line's options become commands.
* **Remote control:** Bevy's Remote Protocol (JSON-RPC over HTTP, loopback only, off by default) with the game's own methods: run a command, read the state, send the player's input, take a screenshot, read the log; a client in `tools/game/`, then a project skill or an MCP server for Claude Code.
* **Work:** the issue #165 and its sub-issues #166–#179, in M2; ≈10–20 agent-hours (estimate).

## 1. Goals

1. The maintainer starts the game as a player would: full screen, a menu, a hero to choose.
2. The maintainer sees what the game is doing without a debugger: the frame rate and times, the hero's place, the log.
3. What the command line can set now, the console can change while the game runs, without a restart.
4. Claude Code starts the game, plays it as a player does (the menu, the clicks, the keys), reads its state and log, takes screenshots, and repeats a run from a script, without the maintainer.

None of this changes the game's rules or look; the menu and the HUD are stopgaps until the interface of M5.

## 2. The parts

```text
command line ── --exec, --script ─┐
console (~) ──────────────────────┼──► commands (clap) ──► the game: states, resources, messages
remote protocol: opensacred/exec ─┘                            ▲
remote protocol: opensacred/input ──► keyboard and mouse ──────┘
settings.toml ◄──► settings: the display mode, the window's size, the overlays (F11, F3, F4)
tracing ──► the log's ring ──► F4, the console, opensacred/logs, the log file
```

Each part is a module of `opensacred-game` with its own plugin (CLAUDE.md, "Code conventions").
The commands are the one way in: the console, the scripts and the remote protocol add no behavior of their own.

## 3. Settings (#167)

* The file is `settings.toml` in the user's configuration directory: `%APPDATA%\OpenSacred\` on Windows, `$XDG_CONFIG_HOME/OpenSacred/` on Linux, through the `directories` crate (docs/06 §6, ADR-08).
* The first section is the display:

  ```toml
  [display]
  mode = "borderless"   # or "windowed"
  width = 1920          # the window's size in physical pixels, for "windowed" only
  height = 1080
  ```

* Borderless full screen is the default, on the monitor the window opens on, at its resolution; the size applies only to the window.
* F11 switches between borderless and the window and writes the mode at once.
* No file means the defaults, and the file is written the first time a setting changes.
  A file that does not parse is reported and left alone, never overwritten; an unknown key is reported.
* Writing changes only the keys it sets and keeps the user's comments (`toml_edit`).
* The command line overrides the file for one run without writing it: `--settings FILE` (another file, for the tests and Claude Code's runs), `--windowed` or `--borderless`, `--width` and `--height`.
* `--screenshot`, `--record` and `--benchmark` run in a window of the given size, as now, so that their frames do not depend on the monitor.
* The overlays add `[debug] overlay` and `[debug] log` (§5); the rest of docs/06 §6 (sound, controls, gameplay) comes with M6.

## 4. States, the menus and the choice of the hero (#171, #172, #178)

* `main.rs` now reads the world, the hero, the start script and the first sector before Bevy starts.
  For a menu, the game must start and end inside one process.
* States `AppState { Menu, Loading, InGame }`.
  A resource `NewGame` holds what a game starts from: the class and the command line's options that belong to a game (`--sector`, `--hero`, the clips, `--creature`, `--tree`, `--footsteps`, `--walking`, `--target`, `--time`, `--zoom`, `--no-music`), as a `clap::Args` struct that the command `new-game` reuses (§6).
* What every game shares (the world's index, the sprites, the tables) is read once; `Loading` does the rest on Bevy's async compute pool behind a loading screen, and an error goes to the log and back to the menu.
* The modules' systems run only in `InGame`: the steps of `frame::Step` take the condition once, and the plugins' `Startup` systems move to `OnEnter(InGame)`.
  Leaving `InGame` despawns the game's entities (`DespawnOnExit`) and removes its resources.
* The menu: New Game and Quit, in the original's interface font.
  New Game shows the seven classes of `--class` with their portraits (`gui_qbp_*`) and names; a choice starts the game.
  The mouse, or the arrows, Enter and Esc.
* The pause menu (#178): Esc in the game pauses it under a small menu over the frame dimmed, Continue, Main menu and Quit; Continue or Esc again goes on.
  The pause is a state `Paused` under `InGame` with Bevy's virtual time paused, so that the hero, the NPCs, their animations, the clock and the liquids stop and the world takes no input; the music and the ambient loops go on.
  With the console open, Esc closes the console instead.
* A game named on the command line (`--class` or another of the game's options, or `--screenshot`, `--record`, `--benchmark`, which need a scene) starts as if `new-game` with those options ran at the start (§6), not by a mechanism of its own.
  The remote protocol can drive the menu too (§8), but a one-shot run, such as a screenshot, a benchmark or a comparison with an older build, then needs no server, client or waiting.
* The menus depart from the original's screens (the hero's name, difficulty, a turning model; saving and the options in the game's menu): a decision in `docs/decisions/`, a stopgap that does not change the play.

## 5. The HUD, the overlay and the log (#168, #170, #169)

* The HUD's portrait, health bar and Walk/Run label go (#168): the health is always full until M3, and the gait is the overlay's now.
  The cursor and the interface's scale stay; `docs/decisions/2026-10-17-hud.md` changes with it.
* **The overlay (F3)**, in the upper left, refreshed four times a second in a monospaced font.
  One function gathers its values, which the remote protocol returns too (§8):
  * the frames a second, the mean and slowest frame time of the last second;
  * the hero's cell, sector, level, target and gait;
  * the clock and its phase, the zoom;
  * the sectors spawned and being built, the NPCs and objects placed, the entities;
  * the version, the GPU adapter and backend, the window's size and mode.
* **The log (F4)**: a `tracing` layer keeps the last 1,000 records in a ring with sequence numbers; F4 shows the last 15 at the lower left, warnings yellow and errors red.
  The same records go to `%LOCALAPPDATA%\OpenSacred\logs\opensacred.log` (`$XDG_STATE_HOME` on Linux), a new file each start with the two before kept, so that a crash's log can be read after it; `--log-file` writes another one.

## 6. Commands and scripts (#173)

Each command gives lines of output or an error.
They are parsed by clap, as a REPL (`multicall`, the line split as a shell does), so that a command takes the same options as the command line and has `help`, and they run between frames with exclusive access to the world.

| Command | What it does | Command line now |
|---|---|---|
| `new-game [--class C] [--sector X,Y] […]` | ends the game, if any, and starts a new one | the same options |
| `menu`, `quit` | back to the main menu; end the process | |
| `pause on\|off` | pauses the game without the pause menu, for a still frame | |
| `sector X,Y`, `cell X,Y` | puts the hero at a sector's centre or at a cell; the sectors stream around it | `--sector` |
| `go X,Y`, `go +DX,+DY` | sends the hero to a cell, or by an offset in cells, as a click does | `--target` |
| `gait walk\|run` | as R | `--walking` |
| `zoom Z` | the camera's zoom, 0.5 to 2 | `--zoom` |
| `time HH:MM` | sets the clock | `--time` |
| `music on\|off` | the music and the ambient loops | `--no-music` |
| `screenshot FILE [--frames N]` | saves a frame and goes on | `--screenshot`, `--frames` |
| `record DIR N` | saves N frames | `--record`, `--record-frames` |
| `benchmark SECONDS` | measures and prints | `--benchmark` |
| `window borderless\|windowed [W H]` | the display mode and the window's size, kept | F11 and the settings |
| `overlay on\|off`, `log on\|off` | as F3 and F4 | |
| `state` | the app's state, the hero, the clock, the frames (the overlay's values) | |
| `wait ready\|arrived`, `wait frames N`, `wait seconds S` | in a script, the next command waits for that | |
| `exec FILE` | runs a script: a command a line, `#` comments | |
| `help [COMMAND]` | | |

* `--exec "CMD; CMD"` and `--script FILE` run commands from the start, for example `--class mage --exec "go +10,+4; wait arrived; screenshot a.png; quit"`.
  `--screenshot`, `--record`, `--benchmark` and `--target` stay as shortcuts.
* `--assets` and `--settings` stay the command line's alone.
* `sector` and `cell` jump: the streamer drops the sectors beyond two of the new place and builds the 3 × 3 around it, and the hero waits for its ground.

## 7. The console (#174)

* The key under Esc opens and closes it, by its place rather than its character (`KeyCode::Backquote`: `~` in an English layout, `Ё` in a Russian one); Esc closes it too.
* The top half of the screen: one scrolling view of the log and the commands' output, and a line of input (Bevy 0.20's `TextInput`, so no `bevy_egui`).
* Enter runs the line; Up and Down walk the history, kept between runs; Tab completes the commands' names and the options' values; Ctrl+V pastes.
* While it is open the game takes no keys or clicks (no walking, no R, the wheel scrolls the console); F3, F4 and F11 still work, and the game goes on under it.

## 8. Remote control (#175, #176)

* `--remote [PORT]` turns on Bevy's Remote Protocol: JSON-RPC 2.0 over HTTP on `127.0.0.1:15702` by default, on the loopback only, off by default and never from the settings.
* The game's own methods:

  | Method | What it does |
  |---|---|
  | `opensacred/exec {line}` | runs a command (§6) and returns its output or its error |
  | `opensacred/state` | the overlay's values (§5) and whether the scene is ready |
  | `opensacred/input {…}` | a key, the cursor's move to a pixel, a mouse button or the wheel, sent into the window as if from the system, so that the menu, the clicks on the ground and the keys run through their own code |
  | `opensacred/screenshot {path}` | schedules a capture; the client waits for the file |
  | `opensacred/logs {after}` | the log's records after a sequence number |

* Bevy's own methods (`rpc.discover`, `world.query`, `world.get_resources`, `world.mutate_resources`, …) see what is registered for reflection: the hero, the clock and the zoom are.
* The client, `tools/game/remote.py`, uses the standard library alone: `exec "zoom 1.5"`, `state`, `wait-ready`, `key F11`, `click X Y`, `screenshot FILE`, `logs`.
* Claude Code runs the game so that it neither covers the maintainer's screen nor touches the maintainer's settings: `--remote --windowed --no-music --settings <scratch>/settings.toml --log-file <scratch>/game.log`, the window opening unfocused.
* Screenshots and logs taken this way are pictures of the converted assets: they stay local, never in issues or pull requests (docs/08 §3).
* How Claude Code learns all this in a new session (#176): a project skill that describes the run and the client, needing nothing installed, or the MCP server `bevy_brp_mcp` (0.23 for Bevy 0.20), whose tools are discoverable but generic and which has to be installed and to follow Bevy's releases.
  The skill comes first, and MCP if it falls short.
* `bevy_brp_extras` 0.23 has ready methods for screenshots and keys; the game's own methods are a few small systems that do not wait for its releases after Bevy's.

## 9. Keys

| Key | Where | What it does |
|---|---|---|
| Left mouse button | the game | the hero walks or runs to the cursor (as now) |
| R | the game | walk or run (as now) |
| Wheel | the game | zoom (as now) |
| Esc | the menus, the game, the console | back a screen; in the game, the pause menu and back; closes the console |
| `~` (the key under Esc) | everywhere | the console |
| F3 | everywhere | the overlay |
| F4 | everywhere | the log |
| F11 | everywhere | borderless or windowed |

The original's own keys come with remapping in M6 (docs/06 §3); F3, F4 and F11 move then if they clash.

## 10. Order and estimates

| Issue | Part | Needs | Agent-hours (estimate) |
|---|---|---|---|
| #166, #179 | this plan and its refinements | | 0.3–0.5 |
| #167 | settings, borderless, F11 | | 0.5–1.5 |
| #168 | the HUD trimmed | | 0.2–0.5 |
| #169 | the log on F4 and in a file | | 0.5–1.5 |
| #170 | the overlay on F3 | #168 | 0.5–1.5 |
| #171 | the game's states | | 1.5–3 |
| #172 | the menu and the choice of the hero | #171 | 1–2 |
| #178 | the pause menu on Esc | #172 | 0.5–1 |
| #173 | commands and scripts | #171 | 1.5–3 |
| #174 | the console | #173, #169 | 1–2.5 |
| #175 | remote control | #173, #170 | 1.5–3 |
| #176 | a skill or MCP for Claude Code | #175 | 0.5–1.5 |

The estimates follow the game's recent pull requests, ≈0.7 agent-hours each; the total, ≈10–20 agent-hours, is within M2's budget (PLAN.md §9.2).
#167, #168, #169 and #171 can go in parallel; #172 and #173 after #171.

## 11. Open questions

1. Whether F3 and F4 clash with the original's keys: settled with remapping in M6.
2. Whether the remote protocol stays in release builds: on at runtime only with `--remote`; a Cargo feature can leave it out of a player's build later.
