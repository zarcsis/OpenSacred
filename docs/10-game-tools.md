# Tools to test and drive the game

## Summary

* **Why:** before this work everything the game could be told was a command line option, read before Bevy started; the hero's class was fixed for the process, and a change meant a restart.
  The tools let the maintainer test the game as a game, and let Claude Code drive a running game by itself.
* **Settings:** `settings.toml` in the user's directory; borderless full screen by default, F11 for a window, both kept.
* **Menu:** a main menu (New Game, Quit) and the choice of the hero, which needs the game to start and end inside one process: states, and a loading in the background.
  Esc in the game pauses it under a small menu: Continue, Main menu, Quit.
* **On the screen:** the HUD's portrait, health bar and gait go; F3 shows a debug overlay (the frame rate and times, the hero's place), F4 the log, which a file keeps too.
* **Commands:** one set of commands, which the command line's scripts, the console on `~` and the remote protocol run alike; the command line's options become commands.
* **Remote control:** Bevy's Remote Protocol (JSON-RPC over HTTP, loopback only, off by default) with the game's own methods: run a command, read the state, send the player's input, take a screenshot, read the log; a client in `tools/game/` and the project skill `game` for Claude Code, no MCP server.
* **Work:** the issue #165 and its sub-issues #166–#181 and #189, in M2, then #196, #198 and #202; done 2026-10-19 and 2026-10-20.

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

* The file is `settings.toml` in the user's configuration directory: `%APPDATA%\OpenSacred\` on Windows, `$XDG_CONFIG_HOME/OpenSacred/` on Linux, taken from the environment (docs/06 §6, ADR-08).
  Done 2026-10-19 (#167).
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
* The command line overrides the file for one run without writing it: `--settings FILE` (another file, for the tests and Claude Code's runs), `--windowed` or `--borderless`, `--width` and `--height`, and `--hidden`, a window that is never shown, for the runs driven from afar (#556).
* `--screenshot`, `--record` and `--benchmark` run in a window of the given size, as now, so that their frames do not depend on the monitor.
* The overlays add `[debug] overlay` and `[debug] log` (§5); the rest of docs/06 §6 (sound, controls, gameplay) comes with M6.

## 4. States, the menus and the choice of the hero (#171, #172, #178)

* Before #171, `main.rs` read the world, the hero, the start script and the first sector before Bevy started.
  For a menu, the game must start and end inside one process.
* States `AppState { Menu, Loading, InGame }`.
  A resource `Request` holds a `NewGame`, what a game starts from: the class and the command line's options that belong to a game (`--sector`, `--hero`, the clips, `--creature`, `--tree`, `--footsteps`, `--walking`, `--target`, `--time`, `--zoom`, `--no-music`, the hero's `--name` and the `--difficulty`, #1086), as a `clap::Args` struct that the command `new-game` reuses (§6).
  The command line's `--zoom` and `--no-music`, which say how a game shows, hold for every game of the run: a `new-game` of a script, the console or remote control takes them from a resource `Shows` unless its own line sets them (#831).
* What every game shares (the world's index, the sprites, the tables) is read once; the tables that the start and the loads read (`items`, `weapons`, `creatures`, `motions`, `balance`) are parsed when first asked for and kept in `sectors::Shared` (`tables`, #830).
  `Loading` does the rest on Bevy's async compute pool behind a loading screen, and an error goes to the log and back to the menu.
* The modules' systems run only in `InGame`: the steps of `frame::Step` take the condition once, and the plugins' `Startup` systems move to `OnEnter(InGame)`.
  Leaving `InGame` despawns the game's entities (`DespawnOnExit`) and removes its resources.
* Done 2026-10-19 (#171), in a module `game`: the systems of a game are in a set `frame::Playing`, which runs only in `InGame`; the camera and the light are spawned once and stay, so that the menus draw through the same camera and its sRGB frame; each module takes back what it made on `OnExit(InGame)`, the sectors with what they adopted, the hero, the building it was in, the music and the night, and `game` the resources it loaded, and warns of anything left in the world when the menu comes.
  Until the menu (#172), the process goes straight into the game that the command line gives and ends, failing, when it does not load.
* The menu: New Game and Quit, in the original's interface font.
  New Game shows the eight classes of `--class` with their portraits (`gui_qbp_*`) and names; a choice starts the game (the Vampiress since #790).
  The mouse, or the arrows, Enter and Esc.
  Done 2026-10-19 (#172), in a module `menu` (docs/decisions/2026-10-19-menus.md): the game's logo over the items, a sub-state `Screen { Main, Heroes }`, the items chosen by pointing (the picking of the interface's nodes, a feature of Bevy's now) or the arrows, and a hero starting a game with the command line's other options; while a game loads, the original's first loading screen shows (`ui/screens/loading0.png`).
  A game that the command line names starts at once, and a screenshot's, a recording's or a benchmark's ends the process when it does not load; any other goes back to the menu.
* The original's main menu (#1118, docs/re/menus.md): its picture of 12 tiles with the water at its foot, the column of its items in Carolingia, gold, lit under the cursor with a glow and fading in, and the Multiplayer and Extras submenus, a sub-state `Screen { Main, Multiplayer, Extras, Heroes }`; the items' words come from the game's texts (`menu::Words`), and those whose screens the game does not have yet stand greyed and are passed over by the arrows.
  The picture keeps the original's 1024 × 768, scaled to the window's height, black at its sides in a wider window.
* The original's choice of the hero (#1119, docs/re/menus.md), a module `choice`: the eight classes as idling 3D figures on the pedestals, in a scene of their own that their camera draws into a texture between the picture and the panel (`figures.wesl`), dark until pointed at or chosen; the panel of the name, the difficulty, Import, Back and Start Game, and a class's description while the cursor is on its figure.
  The arrows choose a figure, the keys type the hero's name, Enter starts the game with the class and the name (`--name` of `new-game`).
  The Underworld campaign's choice (#1129), a sub-state `Screen::Underworld`: the same picture, its pedestals labelled free until the heroes can be saved and imported, Back, and Start Game greyed.
* The pause menu (#178): Esc in the game pauses it under a small menu over the frame dimmed, Continue, Main menu and Quit; Continue or Esc again goes on.
  The pause is a state `Paused` under `InGame` with Bevy's virtual time paused, so that the hero, the NPCs, their animations, the clock and the liquids stop and the world takes no input.
  The sound pauses too, the music, the ambient loops and the footsteps fading out and, on Continue, in again from where they stopped: paused rather than muted, so that a track does not run on unheard.
  With the console open, Esc closes the console instead.
  Done 2026-10-19 (#178): a module `pause` with a sub-state `Play { Running, Paused }` of `InGame`, Bevy's virtual time paused, the three channels of the sound paused and resumed with a fade of 0.2 s, and a set `frame::Unpaused` of what stops with the pause, the hero's keys and clicks, the zoom, and the systems that start a sound; the menu is the third screen of `menu`.
  The rest of a game's systems go on, standing still on the paused time, so that a model that comes in while the game is paused still takes its materials.
* A game named on the command line (`--class` or another of the game's options but `--zoom` and `--no-music`, which only say how it shows, or `--screenshot`, `--record`, `--benchmark`, which need a scene) starts as if `new-game` with those options ran at the start (§6), not by a mechanism of its own.
  The remote protocol can drive the menu too (§8), but a one-shot run, such as a screenshot, a benchmark or a comparison with an older build, then needs no server, client or waiting.
* The menus depart from the original's screens (the hero's name, difficulty, a turning model; saving and the options in the game's menu): a decision in `docs/decisions/`, a stopgap that does not change the play.

## 5. The HUD, the overlay and the log (#168, #170, #169)

* The HUD's portrait, health bar and Walk/Run label go (#168): the health was always full until M3, and the gait is the overlay's now.
  Since #292 the overlay shows the hero's health too; the bar comes back with the interface of M5.
  The cursor and the interface's scale stay; `docs/decisions/2026-10-17-hud.md` changes with it.
  Done 2026-10-19.
* **The overlay (F3)**, in the upper left, refreshed four times a second in a monospaced font.
  One function gathers its values, which the remote protocol returns too (§8):
  * the frames a second, the mean and slowest frame time of the last second;
  * the hero's cell, sector, level, target and gait;
  * the clock and its phase, the zoom;
  * the sectors spawned and being built, the NPCs and objects placed, the entities;
  * the health of the creature nearest the hero, its most and its level (#278);
  * the hero's level, its health and its most (#292) and its experience (#283);
  * the version, the GPU adapter and backend, the window's size and mode.

  Done 2026-10-19 (#170), in a module `overlay`: the frame times are those of the last second by the real time, kept by the module itself rather than Bevy's diagnostics; a `Snapshot` holds the values, which the command `state` and the remote protocol give too; whether the overlay shows is the setting `[debug] overlay`.
  At 3840 × 2160, running across sectors, the overlay shown took 2.3 % of the frame rate (580 frames a second without it, 566 with it, about 0.04 ms a frame).
* **The log (F4)**: a `tracing` layer keeps the last 1,000 records in a ring with sequence numbers; F4 shows the last 15 at the lower left, warnings yellow and errors red.
  The same records go to `%LOCALAPPDATA%\OpenSacred\logs\opensacred.log` (`$XDG_STATE_HOME` on Linux), a new file each start with the two before kept, so that a crash's log can be read after it; `--log-file` writes another one.
  Done 2026-10-19 (#169): the records are those that Bevy's filter lets through (INFO and above), each line of the file `seconds-since-the-start LEVEL target: message`; the panel takes one line a record, its targets in the game without `opensacred_game::`, and whether it shows is the setting `[debug] log`.

## 6. Commands and scripts (#173)

Each command gives lines of output or an error.
They are parsed by clap, as a REPL (`multicall`), so that a command takes the same options as the command line and has `help`, and they run between frames with exclusive access to the world.
The line is split at blanks, single or double quotes keeping blanks within a word; a backslash is a plain character, so a Windows path may be written with either separator (#368).

| Command | What it does | Command line |
|---|---|---|
| `new-game [--class C] [--sector X,Y] […]` | ends the game, if any, and starts a new one | the same options |
| `menu`, `quit` | back to the main menu; end the process | |
| `pause on\|off` | pauses the game without the pause menu, for a still frame | |
| `sector X,Y` | puts the hero at a sector's centre, on the ground or in a room of a building, else on the ground nearest it (#458); the sectors stream around it | `--sector` |
| `cell X,Y [FLOOR]`, `cell +DX,+DY [FLOOR]` | puts the hero on the cell itself, whatever it is (#522): on the ground, or on a building's floor `FLOOR` there, 0 the ground even within its walls | |
| `cell-info [X,Y]` | what a cell of the loaded sectors is, the hero's by default: its ground, the inside layers over it, whether it is a pocket, what stands in it (#522) | |
| `nearest [X,Y]` | where `sector` would land the hero for a cell: the cell itself, a room, or the nearest cell of the ground it walks on from (#522) | |
| `go X,Y`, `go +DX,+DY` | sends the hero to a cell, or by an offset in cells, as a click does | `--target` |
| `gait walk\|run` | as R | `--walking` |
| `zoom Z` | the camera's zoom, 0.5 to 2 as in the original, 2 the widest view | `--zoom` |
| `time HH:MM` | sets the clock | `--time` |
| `setrain N`, `setfog N`, `setsnow N` | starts the rain, the fog or the snow of the intensity N (1 to 100), or stops it with 0, as the original's console (docs/re/weather.md) | |
| `music on\|off` | the music and the ambient loops | `--no-music` |
| `screenshot FILE` | saves the next frame and goes on (a script waits before it with `wait frames N`) | `--screenshot`, `--frames` |
| `record DIR N` | saves N frames | `--record`, `--record-frames` |
| `benchmark SECONDS` | measures and prints | `--benchmark` |
| `window borderless\|windowed [W H]` | the display mode and the window's size, kept | F11 and the settings |
| `overlay [on\|off]`, `log [on\|off]`, `keys [on\|off]`, `console [on\|off]` | as F3, F4, F1 and the key under Esc; without a switch, toggles | |
| `state` | the app's state, the hero, the clock, the frames, the drops nearest the hero (the overlay's values) | |
| `creatures [RADIUS]` | the creatures within RADIUS cells of the hero (20), the nearest first: an id, the type, the cell (and the floor inside), the health, the level, hostile or peaceful, awake, fighting, a champion or a boss, dying, a dialogue; as the original's driver lists them (#523) | |
| `stats [ID]` | a creature's fight values, the hero's by default (`ID` as `creatures` prints it, or its index): the type, the level, the health, the hero's base attributes, A and D, each kind's least and most damage, the resistances, the attack speed, already multiplied where the original's driver prints them raw with their multipliers (#523) | |
| `spawn TYPE [--at X,Y\|+DX,+DY] [--count N] [--level L] [--rank champion\|boss] [--hostile\|--peaceful] [--immortal]` | makes N creatures of type TYPE on a cell and the rings about it, the hero's by default (on its floor), each row by row and whatever the cells are, a liquid's or a wall's too, but never the hero's own, on the cells that the original's driver takes for the same command (#984), as `CreateNPC` makes them: awake when hostile, at the level `CreateNPC` gives them by their sector's values about the hero's (docs/re/spawning.md, "`CreateNPC`'s level"), or L; hostile by default where their class's side is hostile to the heroes and peaceful otherwise, as the creature maker makes them for neither (`0x480D40`, docs/re/fight.md, "Sides"); `--immortal` takes no damage, a target for the hero's hits; kept by their sectors as a fill's creatures (#524) | |
| `kill [RADIUS]` | fells the hostile creatures within RADIUS cells of the hero (5): they die, drop and give the hero their experience, as its kills do (#524) | |
| `item ITEM [--at X,Y\|+DX,+DY] [--level L] [--key K] [--amount N]` | throws an item of type ITEM (an `items.pak` slot) about a cell, the hero's by default, as a death's drop is thrown: gold of N pieces (a pile's by the level by default), a potion as it is, a weapon, armour or a jewel made by the item maker at level L (the hero's) with the pass's quality key K (16 plain, 2 to 15 the maker's key 16 − K, 0 its 14); a type of no drop's kind (a rune, an upgrade) is refused (#525) | |
| `level L`, `experience N\|+N` | the hero's level, its experience at the level's start; or its experience set or added to: its level, its attribute and skill points follow as its kills would give them, and its health (refilled), values and regeneration as at a level-up (#526) | |
| `heal` | the hero's health to its most (#526) | |
| `god [on\|off]` | the hero takes no damage (`fight::Immortal`): a hit lands and shows, the health stays (#526) | |
| `fly [on\|off]` | the hero goes straight where it is sent, through walls and buildings and over water, on the ground's level, through no door and up no stair (`hero::Flies`, #526) | |
| `take` | sends the hero to take the item lying nearest it, as a left click on it does: it walks to the item and takes it once it stands within its reach, gold into its purse, which `state` shows, and the rest into its pack (#911) | |
| `talk [ID]` | sends the hero to speak to the creature ID, or to the creature of a dialogue nearest it, as a left click on it does: within its reach the creature's dialogue runs and its window shows (#1028) | |
| `answer [N]` | the dialogue's window shown: its button N (from 1) chosen, as a click on it does, which closes it and runs the button's function; without N, the window's name, texts and buttons | |
| `drop [TIMES]` | makes the hostile creature nearest the hero drop as if the hero killed it, TIMES deaths' worth, and lists what fell (#415) | |
| `script FUNCTION` | runs the function FUNCTION of the hero's tree (`QIS_OnEnter1202`, `OMO-1tprein2.3`) as the scripts run one, a function of a quest only while the quest is set up, the hero put where it teleports it; then lists the scripts' timers, each one's id, function and minutes still to run (#933); it says where the function waits for its quest | |
| `quest set-up\|trigger QUEST` | sets the quest up, or triggers it and enters it where its trigger runs through, as the scripts' `SetUpQuest` and `TriggerQuest` do, its sections run in turn and the hero put where they teleport it; then the quest's flags and the timers (#952) | |
| `arts`, `arts learn ART [LEVELS]`, `arts ready`, `arts use ART`, `arts select ART`, `arts make [ART]` | the hero's combat arts as the original's driver lists them: each one's table (`spell` or `art`), id, level and bonus, `learnt` where a rune taught it, the shortcut's slot, the time, the factor and the regeneration still to run, which runs down in steps of the world's tick of about 0.1 s, as the original's (#507, #627); `learn` teaches ART (below 1000 a spell) or raises it by LEVELS as that many runes do, whatever the class; `ready` makes every art ready; `use` starts the regeneration as a use of ART does, every art of its kind taking its time to run (#506); `select` sets the art the right button makes, `make` makes ART, the selected by default, on the hostile creature nearest the hero, as a right click on it does: the hero comes on, plays the art's clip and strikes (#509); an art not ready is refused here, where a right click walks the hero to the creature (#654) | |
| `wait ready\|arrived`, `wait frames N`, `wait seconds S` | in a script, the next command waits for that | |
| `exec FILE` | runs a script: a command a line, `#` comments | |
| `help [COMMAND]` | | |

* `--exec "CMD; CMD"` and `--script FILE` run commands from the start, for example `--class mage --exec "wait ready; go +10,+4; wait arrived; screenshot a.png; quit"`.
  `--screenshot`, `--record`, `--benchmark` and `--target` stay as shortcuts.
* `--assets` and `--settings` stay the command line's alone.
* `sector` and `cell` jump: the streamer drops the sectors beyond two of the new place and builds the 3 × 3 around it, and the hero lands once they are in (`hero::land`, `terrain::Heights::landing`, #458), and `wait ready` waits for it.
  After `sector` it lands in the room of a building's floor 1 where the cell is one, a cell that a hero who comes in by the building's entrances reaches, whatever doors are closed; else on the cell of the ground nearest it, the cell itself first, within a sector's side, that a hero may step into and walk on from, reaching 1,024 places through the doors and the floors (an estimate: by the building of 4,37 at (268, 2378) the nearest walkable cell is a pocket of one cell amid the masses), and the log says where when it is not the cell.
  The original has no such command: a hero that starts on a building's mass goes onto floor 1 wherever the floor's grid holds the cell walkable, and so do the empty cells of the grid about its rooms, behind its walls, where a hero put there by the command used to stand stranded.
* `cell` puts the hero on the cell itself since #522: the commands are debugging tools, and a check must reach what the play cannot, a pocket of the ground or a building's mass (the blacksmith's plateau of (3253, 2867) is a pocket of 707 places in the game's data, where #458's landing put the hero 21 cells away).
  Without a floor it stands on the ground, where the original's rule for a hero on a building's mass then takes it onto floor 1; with one it keeps that level while it stays on the cell, whatever the cell's mass, entrance or exit would make of it, and a stair it stands on takes it nowhere until it steps off.
  A floor that no building has at a cell of the loaded sectors is an error; farther, the log says so once the sectors are in, and the hero stands on the ground.
  `cell-info` and `nearest` answer what the landing decided: whether the hero can walk on from a cell (a pocket is fewer than 1,024 places reached through the doors and floors, whatever triggers are closed), and where `sector` would put it.
  `cell`, `cell-info` and `nearest` take `+DX,+DY` from the hero's cell, as the original's driver does.
* Done 2026-10-19 (#173), in a module `command`: `run` parses a line and does it with the whole world at hand, giving an `Outcome` of lines (and for `wait` what a script waits for) or an error; a script runs up to 256 lines a frame, logs each line and its output, and starts a wait with the next frame, so that `wait frames 3` lets three frames render.
  `screenshot FILE` saves the next frame; a script waits the frames it wants before it, instead of `--frames`.
  A jumping hero stands on the ground as soon as its sector's ground is in; the streamer needed nothing more.
  `--exec` splits at every semicolon, so a command in it cannot hold one.
* Fixed 2026-10-20 (#202): a script goes on after a `screenshot` only once its file is written, a frame or two later, so that a `quit` or a jump right after it cannot come first (the last frame of a script was lost, and a frame asked for before a jump showed the new place before its sectors); and `wait ready` (and the state's `ready`) holds only once the sectors around the hero's place of now are in, not merely when the streamer has nothing to build, which it had right after a jump.
* Fixed 2026-10-22 (#538): a hero whose model has nothing to play, a `--hero` model without animations or without the hero's clips, is ready once its scene has spawned (`hero::still`), where it waited for an animation player that Bevy's glTF loader gives no such scene, so that `wait ready`, `--screenshot` and `--benchmark` never ended.

## 7. The console (#174)

* The key under Esc opens and closes it, by its place rather than its character (`KeyCode::Backquote`: `~` in an English layout, `Ё` in a Russian one); Esc closes it too.
* The top half of the screen: one scrolling view of the log and the commands' output, and a line of input of its own (no `bevy_egui`).
* Enter runs the line; Up and Down walk the history, kept between runs; Tab completes the commands' names and the options' values; Ctrl+V pastes.
* While it is open the game takes no keys or clicks (no walking, no R, the wheel scrolls the console); F3, F4 and F11 still work, and the game goes on under it.
* Done 2026-10-19 (#174), in a module `console`, with a line of input of its own rather than Bevy's `TextInput`: a console needs a line, a cursor and the history, which take a few dozen lines.
  The view is the log itself: the commands' lines and their output are logged under the target `console`, so F4 and the log file have them too.
  A set `frame::Control` holds the player's keys and mouse on the world, the pause and the menus, which the open console holds back; the console reads the keys right after Bevy does and takes Esc and its own key from the game, so that Esc closes it without pausing the game.
  The history lies beside the settings file (`history.txt`), so that a run with `--settings` keeps its own; the command `console on|off` opens and closes it from a script.
  Ctrl+V did not paste at first: Bevy reads the system's clipboard through `arboard`, whose Windows backend `clipboard-win` (and its `error-code`) is under BSL-1.0, which `deny.toml` did not allow.
  On 2026-10-19 the maintainer allowed BSL-1.0 (#189): permissive like MIT, it asks nothing of compiled code, only that copies of the sources keep their notice, and the repository copies none.
  Ctrl+V, by its place, pastes the clipboard's text at the cursor, its lines as one; Ctrl+C copies the line; the remote protocol sends them as `Control+V` and `Control+C`.

## 8. Remote control (#175, #176)

* `--remote [PORT]` turns on Bevy's Remote Protocol: JSON-RPC 2.0 over HTTP on `127.0.0.1:15702` by default, on the loopback only, off by default and never from the settings.
* The game's own methods:

  | Method | What it does |
  |---|---|
  | `opensacred/exec {line}` | runs a command (§6) and returns its output or its error |
  | `opensacred/state` | the overlay's values (§5) and whether the scene is ready |
  | `opensacred/input {…}` | a key, text typed, the cursor's move to a pixel, a mouse button or the wheel, sent into the window as if from the system, so that the menu, the clicks on the ground and the keys run through their own code |
  | `opensacred/screenshot {path}` | schedules a capture; the client waits for the file |
  | `opensacred/logs {after}` | the log's records numbered `after` and later |

* Bevy's own methods (`rpc.discover`, `world.query`, `world.get_resources`, `world.mutate_resources`, …) see what is registered for reflection, which is none of the game's types yet.
* `world.mutate_components` of `bevy_remote` 0.20.0 panics on an entity that is gone (`World::entity_mut`), so that a call on an object of a sector that has unloaded ended the game (#456, 2026-10-21).
  The game registers its own method under that name after Bevy's, which takes its place: it looks the entity up and answers `ENTITY_NOT_FOUND` (-23401) for one that is gone, as Bevy's other methods on an entity do, and hands the rest to Bevy's.
* The client, `tools/game/remote.py`, uses the standard library alone: `exec "zoom 1.5"`, `state`, `wait-state`, `wait-ready`, `key F11`, `type`, `click X Y`, `move`, `wheel`, `screenshot FILE`, `logs`, `call METHOD`, `shell`, and `--port`.
  `shell` (#550) reads lines until `:quit`, for driving a game by hand: a line is a command of the game, one starting with `:` a call of the client with the same arguments (`:state`, `:key Escape`, `:screenshot FILE`), and `:logs` goes on from the last record shown; `:follow on` prints the new records after each line.
* Claude Code runs the game so that it neither covers the maintainer's screen nor touches the maintainer's settings: `--remote --hidden --no-music --settings <scratch>/settings.toml --log-file <scratch>/game.log`, the window hidden (#556).
* Screenshots and logs taken this way are pictures of the converted assets: they stay local, never in issues or pull requests (docs/08 §3).
* Done 2026-10-19 (#175), in a module `remote`, with the client `tools/game/remote.py` and the recipe in docs/03 §3.7.
  The input waits in a queue that sends each message in its frame before Bevy reads the input: a key's release a frame after its press, a click's press a frame after the cursor's move and its release `hold` frames later (2 by default), so that the game's systems see the key and the button held.
  The cursor the game steers by is now the one its input saw last (`hud::Pointer`), the system's moves and the protocol's alike, so that a click sent from afar moves the hero and the system's cursor stays where it is.
  Bevy's own methods see no type of the game for reflection yet: `opensacred/state` and the commands cover what the plan named.
* Done 2026-10-19 (#176): the project skill `game` (`.claude/skills/game/SKILL.md`), which says when to take a script and when the remote protocol, how to start the game without covering the desktop or touching the maintainer's settings, what the client does, the commands and the keys, and the rules for screenshots; CLAUDE.md's commands name it.
  No MCP server for now: the skill and `remote.py` need nothing installed.
* How Claude Code learns all this in a new session (#176): a project skill that describes the run and the client, needing nothing installed, or the MCP server `bevy_brp_mcp` (0.23 for Bevy 0.20), whose tools are discoverable but generic and which has to be installed and to follow Bevy's releases.
  The skill comes first, and MCP if it falls short.
* `bevy_brp_extras` 0.23 has ready methods for screenshots and keys; the game's own methods are a few small systems that do not wait for its releases after Bevy's.

## 9. Keys

| Key | Where | What it does |
|---|---|---|
| Left mouse button | the game | the hero walks or runs to the cursor |
| Left mouse button on a foe | the game | attack it |
| Ctrl + left mouse button | the game | strike at once |
| R | the game | walk or run |
| Wheel | the game | zoom |
| Esc | the menus, the game, the console | back a screen; in the game, the pause menu and back; closes the console |
| `~` (the key under Esc) | everywhere | the console |
| F1 | everywhere | the keys' help (#196); Esc hides it too |
| F3 | everywhere | the overlay |
| F4 | everywhere | the log |
| F11 | everywhere | borderless or windowed |

The original's own keys come with remapping in M6 (docs/06 §3); F1, F3, F4 and F11 move then if they clash.
F1 shows these keys in the game, with the console's, by where the keys work (a module `help`, 2026-10-19, #196); the command `keys on|off` shows and hides it.

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
