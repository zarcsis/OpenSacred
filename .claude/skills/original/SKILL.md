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

The host checks `Sacred.exe` against the manifest and refuses to start while a `Sacred.exe` runs. Before each start it writes `FULLSCREEN : 0`, `SHOWMOVIE : 0` and the three volumes 0 into `Settings.cfg` (`--sound` keeps the volumes), and the window's keys of dgVoodoo2's `dgVoodoo.conf` back to their defaults, and nothing else. It exits with the game's exit code: 0 after `quit`; 4 when its heartbeat stopped for 5 s; 5 when the agent was unloaded; 3 when the game asked DirectDraw for full screen; 6 when `--monitor` names no monitor.

Stopping the host's background task ends the game within a second (a job object). A run that hangs is ended by stopping that task, never by killing `Sacred.exe` by hand while the host lives.

| Option | What it does |
|---|---|
| `--place offscreen` | the window to the right of every monitor (by default): it draws there, and the screenshots are whole |
| `--place behind` | the window at the primary monitor's corner, behind every other window |
| `--sound` | keep `Settings.cfg`'s volumes |
| `--log-file FILE` | append the records to a file in the scratchpad as well |
| `--keep-captures` | keep the game's `Capture/shotNNNN.tga` and `.jpg` of each screenshot (removed by default once converted) |
| `--no-game-log` | leave out the game's own log (`logPrintf`) |
| `--muted` | the game's volumes 64, so that it plays its sounds (for the sound traces), while the agent sets every volume of mss32.dll to 0; never `--sound` while the maintainer may hear it |
| `--arg WORD` | a word of `Sacred.exe`'s command line, as `--arg tabblocker` (docs/re/window.md) |
| `--port N` | another port (15703 by default) |
| `--interactive` (`--monitor N`) | the maintainer's play (#551, docs/03 §3.8): the original full screen through dgVoodoo2 on the primary monitor (or the n-th the log lists), scaled with its proportions kept, the maintainer's mouse and keyboard reaching it while it has the foreground; it takes the screen and the focus, so only when the maintainer starts it or asks for it, never for a check of Claude Code's own |

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
| `exec "creatures [RADIUS]"` | the creatures within RADIUS cells of the hero (20), the nearest first: id, type, cell (and floor inside), health, level, `hostile` or `peaceful` to the hero, `champion` or `boss`, the AI's and the action's states (docs/re/ai.md), the target's id, its dialogue's index (`+0x245`) where it has one |
| `exec "stats [ID]"` | a creature's stats, the hero's by default (`+0x3A8`): its attributes and base ones, `A` and `D` with their multipliers, each kind's least and most damage with the items' and the attributes' multipliers, the resistances with theirs, the attack speed and the health; the raw values, which the attack record multiplies (docs/re/fight.md), each float exactly (the shortest decimal that reads back as it) |
| `exec fpu` | the x87 control word and its precision (24, 53 or 64 bits) on the render thread's frame, where the commands run, and in the latest stats' recomputation (`0x5796A0`) and strike (`0x56B130`), with their threads (#589) |
| `exec "cell X,Y [FLOOR]"`, `exec "sector X,Y"` | the hero onto the cell, or the nearest it may stand on, as the scripts' `Teleport` puts a player; floor 0 is the ground even within a building's walls, 1 a building's ground floor |
| `exec "cell-info [X,Y\|+DX,+DY]"` | what a cell is, the hero's by default, through the original's own functions (#568, docs/re/movement.md, "Positions and layers"): its sector, its building and floors; for the ground, each floor of the building and the hero's own layer, the cell's nature, ground byte and flags, what its low nibble makes it (a mass, a wall or water, an entrance, an exit, a stair and its offsets, a teleport), a door's trigger and state, whether it may be walked on, whether the hero may stand there (as `cell` places it), its corner heights (drawn and standing, the cell's raw bytes) and the height it stands at (units of the models' height); the objects of each level's cell, the hero among them; a sector not in memory is refused |
| `exec "go X,Y"`, `exec "go +DX,+DY"`, `exec "point X,Y"` | a walk by a click on the cell's pixel (on the screen only), or the pixel alone for `click` (a creature's cell: an attack) |
| `exec "god [on\|off]"` | the hero takes no damage (toggles without a switch): a strike at it leaves its health as it was, any other loss comes back at the next frame, and the original's own `lord` flag spares it the knock-downs (docs/re/objects.md, "The console's cheats") |
| `exec "heal [ID] [--to N]"` | a creature's health to its most, or to N (at most its most), the hero's by default |
| `exec "effect EFFECT VALUE [--whole N] [--seconds S] [--art A] [--id ID]"` | an effect on a creature's stats, the hero's by default, through the original's own `0x582D50`, for S seconds (60), then its `stats` (docs/re/arts.md, "The effects", #651) |
| `exec "arts [ID]"`, `exec "arts learn ART [LEVELS]"`, `exec "arts ready"`, `exec "arts use ART"` | a creature's combat arts (the hero's by default): each entry's table (`spell` or `art`), id, level and bonus, the bit "learnt", the shortcut's slot, the time, the factor and the regeneration still to run; the hero learns an art or rises in it as that many runes teach it (below 1000 a spell); every art of the hero ready at once; an art's regeneration started as a use of it starts it, through the original's `0x562470`, its time added to every art of its kind (docs/re/arts.md, #623) |
| `exec "pause [on\|off]"` | the world's time stands without the menu over a game (the timer's first byte, the screen `paused`), the frames and screenshots going on; toggles without a switch |
| `exec "level N"`, `exec "experience N"`, `exec "experience +N"` | the hero's level, by adding the experience the level starts at as kills add it (each level-up as play gives it), or its experience set or raised, its level following; the original's levels and experience only rise, so a lower one needs a new game |
| `exec "spawn TYPE [--at X,Y\|+DX,+DY] [--count N] [--level L] [--rank champion\|boss] [--hostile\|--peaceful] [--immortal]"` | creatures made by `FillSector`'s maker on the cell (the hero's by default) and the rings about it: at `L` exactly (1 to 250), else by `CreateNPC`'s rule held at the sector's highest, a rank at the marker level 1 (levelled at its first fight); hostile or peaceful by the type unless told; an immortal one takes each strike's damage, which the log tells (`agent: immortal …`), and gets its health back at once (docs/re/objects.md, "Making creatures, kills and drops") |
| `exec "kill [RADIUS]"` | the hostile creatures within RADIUS cells (5) die as the hero's kills, their drops and the hero's experience the game's own: the hero's unarmed strike sent at each at a health of 1, again while it misses (`by N strikes`; within about 27 cells, a strike's reach) |
| `exec "drop [TIMES] [--id ID]"`, `exec "item TYPE"` | the nearest hostile creature (or ID) drops as its death makes it drop, the hero its killer, TIMES times, each object made listed; an item of a type, as the hero's drop forced to it, at the hero's place and level |
| `exec "time HH:MM"`, `exec "day-speed DAY [NIGHT]"`, `exec "zoom Z"` | the clock, its rates (0 stops it), the zoom (0.5 to 2; the game settles on 0.5, 1 or 2) |
| `exec "trace PRESET\|GROUP\|ADDRESS …"`, `exec "untrace NAME\|GROUP\|all"`, `exec traces` | log every call of a function: the presets of docs/re/traces.md (`strike`, `chance`, `death`, `experience`, `drop`, `item`, `regen`, `wake`, `look`, `idle`, `door-search`, `door`, `trigger-set`, `trigger-reset`, `sound-pick`, `sound-event`, the samples and streams), the groups `fight`, `drops`, `ai`, `doors`, `sound`, or an address with `--abi`, `--args` (kinds `int uint hex u16 i16 float ptr object id cstr out-float`), `--ret`, `--ids`, `--limit`; `call original/trace '{"target": …}'` does the same |
| `wait-log PATTERN [--after N] [--timeout S]` | wait for a record whose `target: message` matches, as `wait-log "trace: drop: .*gold"` |
| `exec quit` | the game closes as its window is closed, and the host with it |
| `tools/game/compare.py X,Y …` | with the game (`--remote`) running beside it, both heroes onto each cell and their creatures compared: the pairs of a type whose level, most health, hostility or rank differ, and the creatures of one game alone (docs/03 §3.7, #574) |
| `tools/game/compare.py stats --types T,… --levels L,…` | each type made at each level in both games and their fight values compared, the original's taken through its multipliers as its attack record does (#587, #589) |
| `tools/game/conformance.py [--suites experience,hero,stats,drops,places,regeneration,strikes,arts,movement]` | a conformance run with both games: PASS or FAIL per check, a non-zero exit on a failure; it reloads both (`--save`, the Seraphim's new game) before each suite (docs/03 §3.7, #600) |
| `shell` | the maintainer's, by hand in a terminal (#550): a line a command of the original (`cell 3296,2528`, `creatures 20`), `:CALL` one of these (`:state`, `:click X Y`, `:screenshot FILE`, `:logs`, `:follow on`, `:help`), `:quit` leaves the game running |

The input goes to the game as the agent's own messages: the keys and cursor the game queries are the agent's, and the maintainer's keyboard and mouse never reach it, nor does input posted to the window from outside. A level-1 hero teleported among monsters dies within seconds (it rises again at its start): `god on` first, or place it at a distance and read `creatures` first.
The traces see what the commands make the game do (a `kill`'s drops and experience among them), as the agent runs them from its replacement of the frame's function, outside its hooks. After one click the hero strikes once: click again for each strike, about every 1.3 s. A right-click on a creature makes the hero use its selected combat art on it (the Seraphim's 1071 at the start: `arts ready` first, then `click X Y --button right` about 45 pixels above the creature's cell). A click at a creature's feet (`point` of its cell) hits the ground; click about 45 pixels higher, on its body.

## Rules

- Frames, logs and traces of the original stay in the scratchpad: never in the repository, an issue or a pull request (docs/08 §3). Write what they show in words, the results into #227 and `docs/re/`.
- The game may overwrite the files it writes itself (`Settings.cfg`, `save/`, `Capture/`, its logs); never touch a file of the manifest, and add nothing to `OriginalGame/` beyond dgVoodoo2's two DLLs and its `dgVoodoo.conf` (docs/03 §3.8).
- The agent hooks the running process alone and carries only addresses, offsets and the original's names (docs/re/window.md): no decompiler output, no code translated from it.
