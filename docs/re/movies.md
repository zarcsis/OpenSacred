# Movies

Where the original plays its movies, and how (read 2026-11-03 for #1094, **confirmed** by code unless marked otherwise).
The files and their conversion are in docs/02 §10.3.

## The player

The movies' player is one object (`0x6A1660` makes it the first time it is asked for and keeps it at `0x17E9FC8`), whose `0x6A16E0` plays a movie by its number, 1 to 13.
It plays nothing while its flags' bit `0x100` is set, and otherwise sets their bit 2 and the game's flag `0x20000` (`0x182EE38`) while it plays.
Before a movie it waits half a second and posts the sound manager's event `(0xC, 8, 1)` (**hypothesis**: the music stopped); after it, a quarter of a second, the keys and the mouse's messages pressed meanwhile thrown away.

Each name is a path under `.\MOVIE\`, from the table at `0x9DFED0`, and a movie whose file is not there (`_access` of the mode 4) is skipped silently:

| Number | Movie | Notes |
|---|---|---|
| 1 | `INTRO.WMV` | |
| 2 | `EXTRO.WMV` | first writes `SHOWEXTRO : 1` into `Settings.cfg` (`0x81A840`) |
| 3 to 6 | `act1.WMV` to `act4.WMV` | |
| 7 to 10 | `act5.WMV` to `act8.WMV` | not shipped |
| 11 | `EXTROUW.WMV` | first writes `SHOWEXTRO_UW : 1` (`0x81AA90`) |
| 12 | `INTROUW.WMV` | |
| 13 | `ASCARON.WMV`, then the publisher's, then `OEM.WMV` | the publisher's is `Take2.WMV` where that file is there, else `PUBLISHER.WMV` (neither shipped) |

## Where each plays

- **The start** (`initApp` at `0x816D6C`): the number 13, the three logos one after another, before the main menu.
- **The scripts' `InfoPlayer`** (132, `0x4AE350`; docs/re/netscript.md), not in a network game (`0x182EBEC`): its mode 23 plays the intro (1), 24 the Underworld's intro (12), 25 the Underworld's extro (11), each also setting the bit 8 of the script runner's `+0xA45C`, and 100 to 107 the chapters' movies `act1` to `act8` (3 to 10).
- **The scripts' `GameEnd`** (120, `0x475C3A`): the extro (2).
- **The main menu's Extras** (`0x7125B0`): their rows 1 to 4 play the intro (1), the extro (2), the Underworld's intro (12) and its extro (11); the menu (`0x713670`) adds the extros' rows where `SHOWEXTRO` and `SHOWEXTRO_UW` read set (`0x81A970`, `0x81ABC0`), so a campaign's extro is offered there once it has been seen at its end.

## Its play

`0x6A0EA0` plays the file through DirectShow's multimedia stream into a DirectDraw surface, frame by frame, until the stream ends or a key or a button stops it.
At each frame it tests, by `GetAsyncKeyState`, Escape, Space, the left button and the right one: any held stops the movie; then it waits while Escape or Space is still held.
It reads `COMPAT_VIDEO` (the GOG release's 1) and draws by one of two paths (**hypothesis** for which the key picks): each frame as a quad over the whole screen, 1024 × 768, from the 640 × 480 frame in a 1024 × 512 surface, or copied to the window's rectangle; either way the 4:3 movie fills the 4:3 screen.
The keys' and the mouse's messages are thrown away at each frame.

## In `opensacred-game`

Not yet played (#1094).
