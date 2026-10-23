---
name: game
description: Run opensacred-game and drive it yourself - start it windowed with --remote and play it through tools/game/remote.py (its commands, its state, keys, text, clicks, screenshots, the log), or run a one-shot script with --exec. Use to see a change working in the game, to reproduce a bug, to take a screenshot of a scene, or to measure the frame rate.
---

# Running and driving the game

The game is `opensacred-game` (docs/10). It reads the converted assets of `assets/`; in a worktree, point it at the main checkout's with `--assets ../../../assets` (or link `assets` there with a junction).

## Choose the way

- **One shot, nothing to wait on** (a screenshot, a benchmark, a scripted tour): a script on the command line, no server.

  ```bash
  cargo run --release -p opensacred-game -- --assets <assets> --windowed --no-music \
    --settings <scratch>/settings.toml --log-file <scratch>/game.log \
    --exec "new-game --class mage; wait ready; go +6,+2; wait arrived; screenshot <scratch>/shot.png; quit"
  ```

  `--script FILE` takes the same commands from a file, a line each, `#` for comments. The process ends with `quit`; read the log file for each command's output (`> line`, then its lines, `ERROR` for a failure).
- **Interactive** (look, decide, act again): start the game with `--remote` in the background, then call `tools/game/remote.py` as often as needed.

  ```bash
  cargo run --release -p opensacred-game -- --assets <assets> --remote --hidden --no-music \
    --settings <scratch>/settings.toml --log-file <scratch>/game.log      # run_in_background
  python -I tools/game/remote.py wait-state menu        # up, on its main menu
  python -I tools/game/remote.py exec "new-game --class dwarf"
  python -I tools/game/remote.py wait-ready             # loaded, the hero animating, the sectors in place
  ```

Always pass `--hidden` (or `--windowed` for a one-shot script), `--no-music`, and a `--settings` and `--log-file` in the scratchpad: the default is borderless full screen over the maintainer's desktop, with the maintainer's settings (F11's mode), console history and log. `--hidden` keeps the window off the screen, the taskbar and Alt-Tab, while the game draws on and its screenshots are whole, also while the computer is locked (#556); it cannot be shown again, as the game hangs when it is. With `--remote` the window opens without taking the focus.

## What the client does

| Call | What it does |
|---|---|
| `exec "LINE"` | a command of the game; `exec help` lists them, `exec "help go"` explains one; `wait` is for scripts: from the client, use `wait-ready` or `wait-state` |
| `state` (`--json`) | the overlay's values: the state (menu, loading, game, paused), frames, the hero's cell, sector, gait, target, the clock, the zoom, the sectors, the NPCs, objects and entities, the nearest creature's health and level, the hero's level, health and experience, the drops nearest the hero, the version, GPU and window, whether the scene is ready (`--json` has them all; `exec state` prints the game's own lines) |
| `wait-state menu\|loading\|game\|paused`, `wait-ready` | poll until then, through the game's start too (`--timeout`, 120 s) |
| `key NAME` | press and release a key by its place: `Escape`, `Enter`, `F1`, `F3`, `F4`, `F11`, `R`, `Backquote` (the console), `ArrowUp`, …, or keys held together: `Control+V` (the console pastes the system's clipboard; it is the maintainer's: keep what it held and put it back) |
| `type "TEXT"` | type text, as into the console |
| `click X Y` (`--button`, `--hold`) | click a pixel of the window: a menu's item, or the ground for the hero to walk there |
| `move X Y`, `wheel N` | the cursor, the wheel (the zoom) |
| `screenshot FILE` | the next frame, waiting until the PNG is written; look at it with Read |
| `logs --after N` | the log's records from number N on |
| `wait-log PATTERN` (`--after N`, `--timeout S`) | wait for a record whose `target: message` matches the regular expression, and print it |
| `call METHOD [JSON]` | any method of the protocol, Bevy's too, with JSON parameters |
| `tools/game/compare.py X,Y …` | with the original running beside it under its driver (the skill `original`), both heroes onto each cell and their creatures compared: the pairs of a type whose level, most health, hostility or rank differ, and the creatures of one game alone (docs/03 §3.7, #574); `compare.py stats --types T,… --levels L,…` makes each type at each level in both and compares their fight values (#587); `tools/game/conformance.py` runs the suites `experience`, `hero`, `stats` and `drops` with PASS or FAIL each (#600) |
| `shell` | the maintainer's, by hand in a terminal (#550): a line a command of the game, `:CALL` one of these (`:state`, `:screenshot FILE`, `:logs` from the last shown, `:follow on`, `:help`), `:quit`; lines piped in run as a script, each echoed as `> line` |

`--port N` before the call talks to a game on another port (15702 by default).

The commands most used: `new-game [--class …] [--sector X,Y] [--time HH:MM]`, `menu`, `quit`, `sector X,Y` (lands near the centre where the hero can walk on), `cell X,Y [FLOOR]` or `cell +DX,+DY [FLOOR]` (the cell itself, whatever it is: a pocket, a building's mass, a floor), `cell-info [X,Y]` (a cell's ground, floors, pocket and occupants), `nearest [X,Y]` (where `sector` would land), `go X,Y` or `go +DX,+DY`, `gait walk|run`, `zoom Z`, `time HH:MM`, `pause on|off` (a still frame without the pause's menu), `creatures [RADIUS]` and `stats [ID]` (as the original's driver prints them, for side-by-side checks), `spawn TYPE [--at X,Y] [--count N] [--level L] [--rank boss] [--hostile|--peaceful] [--immortal]` (creatures of a type, `creatures` prints the types), `kill [RADIUS]` (the hostile creatures about the hero fall as if it killed them), `item ITEM [--at X,Y] [--level L] [--key K] [--amount N]` (an item of an `items.pak` slot thrown by the hero, `state` lists it), `level L`, `experience N|+N`, `heal [ID] [--to N]` (a creature's health, the hero's by default, to its most or to N), `god [on|off]` (no damage), `fly [on|off]` (straight through walls and over water), `drop [TIMES]` (the nearest hostile creature drops as if killed), `arts`, `arts learn ART [LEVELS]`, `arts ready`, `arts use ART` (the hero's combat arts, as the original's driver lists them; `use` starts their regeneration as a use would), `keys on|off` (the keys' help), `overlay on|off`, `log on|off`, `console on|off`, `window windowed W H`, `state`, `record DIR N`, `benchmark SECONDS`. Prefer a command to a click where one exists: `go` names a cell, a click a pixel.

## Keys of the game

The left button walks the hero, or attacks a foe under the cursor, Ctrl with it strikes at once, R walks or runs, the wheel zooms; Esc pauses (Continue, Main menu, Quit) or goes back in the menus; F1 the keys' help (a frame of it lists them all), F3 the overlay, F4 the log, F11 borderless or windowed; the key under Esc the console.

## Rules

- Screenshots, recordings and logs show the converted assets: keep them in the scratchpad, look at them, never commit them and never attach them to an issue or a pull request (docs/08 §3). Describe what they show in words.
- End the game you started (`exec quit`), or it keeps its port: a second `--remote` on the same port fails to listen; pass another port (`--remote 15703`, `--port 15703`).
- A click posted to the window from outside (`PostMessage`, `SendKeys`) does not walk the hero: the system reports the cursor leaving a window it is not over. Use the client.
