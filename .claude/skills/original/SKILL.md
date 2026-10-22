---
name: original
description: Run the developer's own original Sacred.exe (GOG 2.28) in the background under Frida and drive it through tools/original/host.py and tools/game/remote.py --port 15703 - its state, frames, screenshots, the game's own log and the host's. Use to see how the original behaves (the checks of #227), to compare it with opensacred-game, or to take a screenshot of the original.
---

# Running and driving the original

The original is the developer's own `Sacred.exe` in `OriginalGame/` (a link to the GOG installation; in a worktree, link it first as for the ignored tests). `tools/original/host.py` starts it under Frida with `tools/original/agent.js` loaded, and serves the game's own remote-control methods on `127.0.0.1:15703` (docs/03 §3.8, #448). The maintainer allowed Claude Code to start it at any time (#448, decision 3): it runs in the background, never takes the focus, never blocks the maintainer's keys and never outlives its host.

## Start and stop

```bash
python -I tools/original/host.py --log-file <scratch>/original.log    # run_in_background; one original at a time
python -I tools/game/remote.py --port 15703 state                     # starting, then running once frames come
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
| `state` (`--json`) | the process, the frames drawn and the rate, the window's rectangle, whether the game believes it has the focus, the guards' refusals |
| `screenshot FILE` | the next frame, as the original's own screenshot (`captureScreenshot`) converted to PNG |
| `logs --after N` | the host's records (`host`), the agent's (`agent`: each guard's refusal) and the game's own log (`game`) |
| `exec help`, `exec quit` | the commands |

The player's input, the program's screen (`wait-state`), the world's state and commands and the traces come with #450–#452.

## Rules

- Frames, logs and traces of the original stay in the scratchpad: never in the repository, an issue or a pull request (docs/08 §3). Write what they show in words, the results into #227 and `docs/re/`.
- The game may overwrite the files it writes itself (`Settings.cfg`, `save/`, `Capture/`, its logs); never touch a file of the manifest, and add no DLL to `OriginalGame/` (dgVoodoo2 only as docs/03 §3.8 says).
- The agent hooks the running process alone and carries only addresses, offsets and the original's names (docs/re/window.md): no decompiler output, no code translated from it.
