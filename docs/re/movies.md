# Movies

Where the original plays its movies, and how (read 2026-11-03 for #1094, **confirmed** by code unless marked otherwise).
The files and their conversion are in docs/02 §10.3.

## The player

The movies' player is one object (`0x6A1660` makes it the first time it is asked for and keeps it at `0x17E9FC8`), whose `0x6A16E0` plays a movie by its number, 1 to 13.
It plays nothing while its flags' bit `0x100` is set, and otherwise sets their bit 2 and the game's flag `0x20000` (`0x182EE38`) while it plays.
Before a movie it waits half a second and posts the sound manager's event `(0xC, 8, 1)`, and after it the same event with 0 (**hypothesis**: the music stopped and again on); after it, a quarter of a second, the keys and the mouse's messages pressed meanwhile thrown away.

`initApp` sets the bit `0x100` where `Settings.cfg`'s `SHOWMOVIE` is 0 and clears it otherwise (`0x816BAB`); `SHOWMOVIE` is read by `0x81C6C0`, which writes it as 1 where the file lacks it (#1946).
So with `SHOWMOVIE : 0` no movie plays but the Extras': the start's logos, the scripts' and `GameEnd`'s are all skipped, and the extros' marks below are not written, as they are written after the bit's test.

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
- **The scripts' `GameEnd`** (120, `0x475C3A`): the extro (2), and nothing else; the function goes on.
- **The main menu's Extras** (`0x7125B0`): their rows 1 to 4 play the intro (1), the extro (2), the Underworld's intro (12) and its extro (11); the menu (`0x713670`) adds the extros' rows where `SHOWEXTRO` and `SHOWEXTRO_UW` read set (`0x81A970`, `0x81ABC0`), so a campaign's extro is offered there once it has been seen at its end.
  The rows stand 32 pixels apart in this order: Credits, Intro, the extro where it is offered, Underworld Intro, the Underworld's extro where it is offered, an empty row and Back (`0x713670`).
  The Extras play their movie whatever `SHOWMOVIE` says: where the player's bit `0x100` is set, they clear its bits `0x80` and `0x100` for the play and set them again after it.

## Its play

`0x6A0EA0` plays the file through DirectShow's multimedia stream into a DirectDraw surface, frame by frame, until the stream ends or a key or a button stops it.
At each frame it tests, by `GetAsyncKeyState`, Escape, Space, the left button and the right one: any held stops the movie; then it waits while Escape or Space is still held.
It reads `COMPAT_VIDEO` (the GOG release's 1) and draws by one of two paths (**hypothesis** for which the key picks): each frame as a quad over the whole screen, 1024 × 768, from the 640 × 480 frame in a 1024 × 512 surface, or copied to the window's rectangle; either way the 4:3 movie fills the 4:3 screen.
The keys' and the mouse's messages are thrown away at each frame.

## In `opensacred-game`

The player (`movies`, #1945) follows the one above: a movie asked for by its name waits half a second, then plays over everything, its frames filling the interface's 4:3 screen and the rest of a wider window black, until it ends or Escape, Space or either mouse button held stops it; a quarter of a second after it, and while Escape or Space is still held, it still takes the keys and the clicks, which reach neither the game nor the menus.
A game stands under a movie as under the pause without its menu, its time and its sound; in the menus their music and sounds pause.
A movie whose file is not there is skipped.
The console's `movie NAME` plays one.

They play where the original's do (#1946): the start's logos before the main menu, in one request whose movies play in turn (the publisher's skipped, as neither of its files is shipped), each stopped alone by a key; the scripts' `InfoPlayer`s of the modes above and `GameEnd`'s extro; and the Extras' rows, the extros' among them once played.
The settings' `[movies] show` is `SHOWMOVIE`, and `[movies] extro` and `extro_underworld` are `SHOWEXTRO` and `SHOWEXTRO_UW`, written as an extro's play is asked for.
A run driven by a script, from afar or for a frame plays none by itself, as it would stand in the run's way (docs/10 §3); the console's `movies on|off` sets that for the run.
The script runner's bit 8 of `+0xA45C`, which the modes 23 to 25 set, is not read.
