---
name: original
description: Run the developer's own original Sacred.exe (GOG 2.28) in the background under Frida and play it through tools/original/host.py and tools/game/remote.py --port 15703 - a new game of a class, saves and loads, keys and clicks, its screen and frames, screenshots, the game's own log and the host's. Use to see how the original behaves (the checks of #227), to compare it with opensacred-game, or to take a screenshot of the original.
---

# Running and driving the original

The original is the developer's own `Sacred.exe` in `OriginalGame/` (a link to the GOG installation; in a worktree, link it first as for the ignored tests). `tools/original/host.py` starts it under Frida with `tools/original/agent.js` loaded, and serves the game's own remote-control methods on `127.0.0.1:15703` (docs/03 §3.8, #448). The maintainer allowed Claude Code to start it at any time (#448, decision 3): it runs in the background, never takes the focus, never blocks the maintainer's keys and never outlives its host.

## Start and stop

```bash
python -I tools/original/host.py --log-file <scratch>/original.log    # run_in_background; one original at a time
python -I tools/game/remote.py --port 15703 wait-state menu           # up, on its main menu
python -I tools/game/remote.py --port 15703 exec "new-game --class seraphim"
python -I tools/game/remote.py --port 15703 wait-ready                # in the world, the first windows closed
python -I tools/game/remote.py --port 15703 exec "save start-seraphim"   # later: exec "load start-seraphim"
python -I tools/game/remote.py --port 15703 click 720 480             # the hero walks there
python -I tools/game/remote.py --port 15703 screenshot <scratch>/menu.png
python -I tools/game/remote.py --port 15703 logs --after 0
python -I tools/game/remote.py --port 15703 exec quit                 # the game closes as its window is closed
```

The host checks `Sacred.exe` against the manifest and refuses to start while a `Sacred.exe` runs. Before each start it writes `FULLSCREEN : 0`, `SHOWMOVIE : 0` and the three volumes 0 into `Settings.cfg` (`--sound` keeps the volumes), and nothing else. It exits with the game's exit code: 0 after `quit`; 4 when its heartbeat stopped for 5 s; 5 when the agent was unloaded; 3 when the game asked DirectDraw for full screen.

Stopping the host's background task ends the game within a second (a job object). A run that hangs is ended by stopping that task, never by killing `Sacred.exe` by hand while the host lives.

| Option | What it does |
|---|---|
| `--place offscreen` | the window to the right of every monitor (by default): it draws there, and the screenshots are whole |
| `--place behind` | the window at the primary monitor's corner, behind every other window |
| `--sound` | keep `Settings.cfg`'s volumes |
| `--log-file FILE` | append the records to a file in the scratchpad as well |
| `--keep-captures` | keep the game's `Capture/shotNNNN.tga` and `.jpg` of each screenshot (removed by default once converted) |
| `--no-game-log` | leave out the game's own log (`logPrintf`) |
| `--arg WORD` | a word of `Sacred.exe`'s command line, as `--arg tabblocker` (docs/re/window.md) |
| `--port N` | another port (15703 by default) |

## What the client does here

| Call | What it does |
|---|---|
| `state` (`--json`) | the screen (`menu`, `loading`, `game`, `paused`: the game's time stands, under the menu over a game, the saves' list or a tutorial window), `ready` (a game, nothing scripted running); in a game the hero's cell, sector, floor, gait and whether it moves, its health, level and experience, the clock and its phase, the zoom, the creatures and objects counted, the nearest creature; the process, the frames drawn and the rate, the window's rectangle, whether the game believes it has the focus, the agent's cursor, the guards' refusals |
| `wait-state menu\|loading\|game\|paused`, `wait-ready` | poll until then |
| `key NAME` | press and release a key, as opensacred-game names them: `Escape` (the menu over a game), `Enter`, `Space`, `L` (the log book), `F1`…`F12`, `NumpadAdd`, `Control+…`; a key with a character sends it too, as a keyboard does |
| `type "TEXT"` | characters, as into a name's field |
| `click X Y` (`--button left\|right\|middle`, `--hold`) | a click on a pixel of the window, 1024 × 768: a menu's item, the ground to walk there |
| `move X Y`, `wheel N` | the cursor, the wheel |
| `screenshot FILE` | the next frame, as the original's own screenshot (`captureScreenshot`) converted to PNG |
| `logs --after N` | the host's records (`host`), the agent's (`agent`: each guard's refusal) and the game's own log (`game`) |
| `exec help` | the commands |
| `exec "new-game [--class C] [--difficulty bronze] [--name N] [--skip-tutorial]"` | from the main menu, the Ancaria campaign with a class of `seraphim` (by default), `gladiator`, `mage`, `wood-elf`, `dark-elf`, `daemon`, `dwarf`, `vampiress`; OK and No more Tips close the first windows unless `--skip-tutorial`; then `wait-ready` |
| `exec "save NAME"`, `exec "load NAME"`, `exec saves` | through the original's own menus: a save of that name is overwritten, else a new one is made in `save/`; a load from the main menu or a game; then `wait-ready` |
| `exec "creatures [RADIUS]"` | the creatures within RADIUS cells of the hero (20), the nearest first: id, type, cell (and floor inside), health, level, the AI's and the action's states (docs/re/ai.md), the target's id |
| `exec "cell X,Y [FLOOR]"`, `exec "sector X,Y"` | the hero onto the cell, or the nearest it may stand on, as the scripts' `Teleport` puts a player; floor 0 is the ground even within a building's walls, 1 a building's ground floor |
| `exec "go X,Y"`, `exec "go +DX,+DY"`, `exec "point X,Y"` | a walk by a click on the cell's pixel (on the screen only), or the pixel alone for `click` (a creature's cell: an attack) |
| `exec "time HH:MM"`, `exec "day-speed DAY [NIGHT]"`, `exec "zoom Z"` | the clock, its rates (0 stops it), the zoom (0.5 to 2; the game settles on 0.5, 1 or 2) |
| `exec quit` | the game closes as its window is closed, and the host with it |

The input goes to the game as the agent's own messages: the keys and cursor the game queries are the agent's, and the maintainer's keyboard and mouse never reach it, nor does input posted to the window from outside. The traces come with #452. A level-1 hero teleported among monsters dies within seconds (it rises again at its start): place it at a distance and read `creatures` first.

## Rules

- Frames, logs and traces of the original stay in the scratchpad: never in the repository, an issue or a pull request (docs/08 §3). Write what they show in words, the results into #227 and `docs/re/`.
- The game may overwrite the files it writes itself (`Settings.cfg`, `save/`, `Capture/`, its logs); never touch a file of the manifest, and add no DLL to `OriginalGame/` (dgVoodoo2 only as docs/03 §3.8 says).
- The agent hooks the running process alone and carries only addresses, offsets and the original's names (docs/re/window.md): no decompiler output, no code translated from it.
