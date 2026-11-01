# The window, the focus and the frames

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra 12.1.4 on 2026-10-21, for the original's driver (#448, #449, #450; docs/03 §3.8).
Status: **confirmed** on the code unless marked otherwise; "observed" marks what the driver saw in the running original on 2026-10-21.

## The start

`WinMain` (`0x817B30`) first takes the named mutex `SACRED_INSTANCE`: if it exists, a message box says that the game runs already and the program ends.
It then reads `Settings.cfg` and the text table, and the words of its command line through `0x813A90`, which finds a word whole and logs it as `cmd: <word>` (observed).

Two words concern the window:

- **`win`**: the window is created with a size of zero instead of the screen's, and the keyboard hook below is not installed.
- **`tabblocker`**: without `win`, `0x810C70` installs a low-level keyboard hook (`WH_KEYBOARD_LL`, the procedure `0x810BC0`, the handle kept at `0x182ED68`), and `0x810CB0` removes it again at the end of the same block of `WinMain`, as a local object's destructor: the hook lives only between two calls, before the window exists (observed with the word: the call came, and the driver refused it).
  Without the word, as the GOG shortcut starts the game, no hook is installed at all.
  The "tabblocker" that swallows Win and Alt-Tab (docs/09 §2.1) therefore does not outlast its block in 2.28; whether another build keeps it is not known.

`0x813B90` registers the class `Sacred` (the styles `CS_VREDRAW | CS_HREDRAW | CS_DBLCLKS`, the window procedure `0x812BF0`, the arrow cursor, a black brush) and creates its one window: `WS_POPUP | WS_VISIBLE`, no extended style, at 0,0 with the size of the primary screen (`GetSystemMetrics` 0 and 1), then `ShowWindow` with `WinMain`'s `nCmdShow`.
With `FULLSCREEN : 0` the window ends up 1024 × 768 at its corner, and `dxDriver7::init` logs `windowed` (observed).

## The wait for the foreground

`sdk_FORCE_FG_WAIT` (`0x811440`) makes the window the foreground's, and `WinMain` calls it twice, after the window is created and after `initApp` (`0x815F70`): unless the window is minimized, it shows it (`SW_SHOW`), calls `SetForegroundWindow` until `GetForegroundWindow` returns the window, then `SetFocus` until `GetFocus` does, and updates it.
`WinMain` passes the timeout that the function takes for "none", so the wait has no end: a game that Windows does not let into the foreground, as one started from the background, spins there at a full core (observed: "Not Responding", 30 s of the processor in 40 s, while the driver's focus answers were wrong).

## The window procedure and the focus

The window procedure (`0x812BF0`; Ghidra has it as a label, not a function) first gives the message to the input method's object (`0x80C140` makes it), then handles:

| Message | What it does |
|---|---|
| `WM_CREATE` | (not read) |
| `WM_DESTROY` | logs `message: WM_DESTROY` (observed) and posts the quit with 0 |
| `WM_CLOSE` | logs, then `terminateApp` (`0x817580`): stops the threads and subsystems, destroys the display (`dxDriver7::~dxDriver7`), and, in full screen, restores the display mode (`ChangeDisplaySettingsA(NULL, 0)`); the window is then destroyed |
| `WM_ACTIVATEAPP` | logs `WindowProc_engine(%d, %d) WM_ACTIVATEAPP`; active: clears the flag `0x20000` of the engine's flags word `0x182EE38` and calls `SetFocus` on its window; inactive: sets the flag; either way posts a sound event, and in full screen (the display's `+0x90` is 1) sets the display mode again |
| `WM_SETCURSOR` | hides the system's cursor (`SetCursor(NULL)`) and answers 1 |

The keys and the mouse are in "The input" below.
The message loop of `WinMain` waits with `WaitMessage`, takes the window's messages with `GetMessageA` and ends when it fails; `WinMain` then returns, and with it the program (observed: 0 after `WM_CLOSE`).

**The focus's effect.**
The render thread (`cEngine::renderThread::run`, `0x60E350`) tests the flag `0x20000` at the top of every loop: while it is set, it skips its whole frame, the world's updates it runs as well as the drawing and the flip, and sleeps 100 ms instead.
The simulation thread (`cEngine::simThread::run`, `0x60F060`), which only measures its own time between sleeps, does not read it.
So the game stands still without the focus rather than spinning, and it draws and moves again when `WM_ACTIVATEAPP` comes back active.
A window that is never activated never receives the inactive message, and the flag stays clear (observed: the driver's window, opened without activation and to the right of every monitor, drew 47 frames a second for a minute while another program had the foreground).
Full screen, each `WM_ACTIVATEAPP`, active or not, costs more (observed 2026-10-22 through dgVoodoo2, #551): the log's `- restoreSurfaces`, the display mode set again (`SetDisplayMode` 1024 × 768), `- flushTextures` with `cResourceMgr::disposeAll()` and `- flushModels` with `cGrannyModelManager::flushCache()`; a run that turned each deactivation into an activation ended by itself after the second such round.
Full screen, the process holds the cursor in the game's screen, `ClipCursor(0,0,1024,768)` as the game is activated (and `ClipCursor(NULL)` now and then while it is not, by whom is not read), and the game moves it with `SetCursorPos` in its own pixels (observed, #551).

## Two window procedures

The procedure above serves the menus.
A game is started or loaded by `cCommand_armaPlay::execute` (`0x7561E0`), which posts the message `0x8002` to the window (its `wParam` 1 or 4 for a load, else a new game); the menus' procedure, on `0x8002`, puts a second procedure in its place (`SetWindowLongA` with `GWL_WNDPROC`, `0x811A20`) and makes the engine, `cEngine` at `0xAD6EF8` (`0x60D6C0`).
The second procedure serves the game: it handles `WM_ACTIVATEAPP` as the first does (the same log, the same flag, `SetFocus`), and the keys and the mouse of its own (below); it puts the first back (`0x81290C`) when the game ends.
Observed: every message of a game, the clicks of the menu over it among them, comes to `0x811A20`.

## The input

The game reads its input in three ways (observed with the driver's own messages, and in the code):

- **The key's state**, polled: `GetAsyncKeyState` (39 calls, docs/01 §4.5) for the keys the game holds, the zoom's keypad `+` and `-` without Shift among them (`cEngine_handleZoomInput`, `0x60DD20`), the left button in the render thread, and Escape, Space and the buttons that end a video (`0x6A0EA0`); `GetKeyState` and `GetKeyboardState` are imported as well.
- **The keys' messages.**
  In the menus' procedure, `WM_KEYDOWN` only keeps the modifiers (`0x182EE70`: Shift `0x3`, Control `0xC`, Alt `0x30`) and gives Escape to the menu (`0x6A1660`, `0x6A1FB0`), but for one language (the language index 5) that types through `ToAscii`; `WM_KEYUP` clears the modifiers, takes a screenshot on Print Screen (`dxDriver7_startCapture`, `0x648660`), and posts every other key but Escape as an event of the game's kernel (`sdk_SACRED_KERNEL_GET`, `0x808E50`, and `sdk_SACRED_KERNEL_RECV`, `0x8092F0`).
  In the game's procedure, `WM_KEYDOWN` translates the key into the game's own code (`0x65DD10`: the arrows, the function keys, the keypad and the modifiers to codes from `0x100` on, letters and digits as they are, the rest 0), keeps the modifiers (`0x182EE6C`), and posts a few keys of its own (Tab, the arrows, the digits with Control); `WM_KEYUP` posts every key with its character (`0x65E130`, Shift and Control taken into account).
  The game's codes of the function keys are F1 `0x10F` to F12 `0x11A`; the keypad's `*` is `0x12E`.
- **The game's keys** (`cEngine_receiveEvent`, `0x618090`, read 2026-10-30, #1536).
  On a key's release (the event of its code, its flag's bit 0 clear; **hypothesis**: the release's, as the latch of Space that this event clears and the character's sets tells): F1 to F12 go to the interface's windows, the hidden ones too (the spells page's F1 to F6, docs/re/arts.md); F9 saves the quick save where the interface allows it, F8 loads it outside a network game, and Space clears its latch.
  On a character (its flag's bit 0 set), once the interface's windows have not taken it, by the character in upper case: Space, Q, W, E, R and B the potions (docs/re/inventory.md, §7); then without Control the key under Esc the console (where the build allows it), Esc, A the collect of the items about the hero (refused in a network game within 100 of its last), P the pause outside a network game, and the keypad's `*` the zoom back to 1; with Control, B a screenshot, F the toggle at `0x182EE38`, X the quick load and Z the quick save.
  The game has no key that toggles walking and running: its moving action (`0x4F4770`, at `0x4F5193` and `0x4F6155`) makes a hero's run (gait 2) a walk (1) at every step while Shift is held, and its walk a run once Shift is let go, a horse its player rides too.
  Since #1536 the game follows it (`hero::pace`), its R no longer toggling the gait; **seen** in the game: the hero walked while Shift was held and ran again once it was let go.
- **The characters**: `WM_CHAR` is posted as an event in both procedures; the menus, the text fields and the game's keys take their letters from it, Escape too (its character 27): with `WM_KEYDOWN` and `WM_KEYUP` alone, Escape opens nothing in the game (observed), with its `WM_CHAR` it opens the menu over the game.

The mouse's messages read the cursor themselves: `getClientCursorPos` (`0x66E280`) takes `GetCursorPos` less the corner of the window's client area (`ClientToScreen` of 0,0), into `0x182EE10`/`0x182EE14` (the previous position at `0x182EDB0`); `lParam` is not read.
`WM_LBUTTONDOWN`, `WM_LBUTTONUP` and the right button's, their double clicks, and `WM_MOUSEWHEEL` (its turn, `wParam`'s high word) are posted as mouse events of the kernel; the middle button is not handled.
The UI thread (`cUI_Manager::runThread`, `0x757060`), which draws the menus and flips for them, also skips its frames while the application is inactive, as the render thread does.

**The mouse's two points** (**confirmed** by code and by probes of the maintainer's real clicks and the driver's, 2026-10-28, #1340).
The mouse object (`cMouse`, `0x654F60`, the instance at `0xCDDB5C`) keeps the cursor's point at `+0x04` and `+0x08` and the point before it at `+0x0C` and `+0x10`.
`cMouse_setPosition` (`0x6554E0`) copies the point into the point before, then takes the new one; in a game only the play's procedure calls it, on `WM_MOUSEMOVE` (`0x8123E0`), and the buttons' messages read the cursor for their event without moving the mouse object (`0x81230F`).
A press reaches the interface as one mouse event (kind 2) that carries the press's point, its sub-kind 0 for the left button and 2 for the right; no event of a release reaches the items' page.
The items' page (`0x6C0150`) does not test the event's point: it takes the point before, less the held item's offset (`0x655810`), and tests the grid's cell there (`0x5D9D60`); a tooltip's links, below, test the event's own point.
A real mouse sends many small moves before a press, so the point before lies within a pixel of the press (seen); a single move before a press leaves it where the cursor was before, and the click lands there.
What the cursor holds is at `+0x68`: its kind (1 an item) and, at `+4`, the item's id.

## The interface's controls

The interface's manager (`cUI_Manager_instance`, `0x756F40`, the instance at `0x17EEBBC`) holds the game's windows in its fields from `+0x80` to `+0xD8`, those that `cUI_Manager_isCursorOverUi` (`0x75AA90`) tests: the taskbar `+0x80`, the inventory `+0x84`, the equipment `+0x88`, the portraits' column `+0x9C`, the character's statistics `+0xA0` (seen); the blacksmith `+0x8C` (seen), the merchant `+0x90`, the town stash `+0xC0` (`UI_WND_CHEST`) and the players' trade `+0xD8` (`UI_WND_TRADE`), as `cUI_Manager_createGameWindows` stores them (`0x75A07F`, `0x75A220`); the main menu at `+0x130`; others in a vector at `+0x124`; the control that takes the keyboard's text at `+0x148` (`0x75AF80`).
A control (`cUI_Control2`, `0x731B00`) has its flags at `+0x10` (bit 0 shown), its place against its parent's at `+0x24` and `+0x28`, its width and height at `+0x2C` and `+0x2E` (16 bits each), its name at `+0x30` (32 bytes) and its parent at `+0x50`; `cUI_Control2_getAbsoluteRect` (`0x732350`) adds its parents' places.
A window (`cUI_Window2`) keeps its children in a vector at `+0x78` (`cUI_Window2_addChild`, `0x727890`).
The main menu's items and the inventory's pages are not children of their windows (seen: none listed).

A tooltip's window (`cUI_Tooltip`, `UI_WND_TOOLTIP`), the first window of a new game and the tutorial's tips among them, draws its buttons as green words of its text, not as controls: up to four links of 20 bytes from `+0x2180`, each the index of the scripts' function it runs, its place against the window's (`+0x04`, `+0x08`) and its width and height (`+0x0C`, `+0x0E`).
A press inside a link (`0x6E74E0`, at the event's own point) runs its function by its index (`0x499DA0`, through the runner `0x46BA90`, after the creature event `0x10C` to the objects 1 to 16) and plays `(9, 0x5D)`; a press elsewhere inside the window is taken and goes no further.
The functions' names are the records of `0x54` bytes from `sdk_TRIGGER_NAME_BEGIN` (`0xAAB708`), a name each, by the index.
Seen on the Daemon's first window (2026-10-28): its OK runs `Tutorial_Trigger` and its Skip Tutorial `btn_Deamonstart`, which then runs `TT_KEINEMEHR`.
A tooltip's window stays shown under the menu over a game, and takes the presses inside its rectangle: with the Daemon's first window open, the menu's Save Game, which lies inside it, does nothing (seen).

## The program's screen

What the driver reports as the screen (observed on the main menu, a new game, the menu over it, the tutorial's windows, saving and loading):

- **the menus** before a game: no engine (`0xAD6EF8` is 0);
- **loading**: the engine's state (`+0x54`) has `0x10000`, while the render thread draws the loading screen (`dxDriver7_drawLoadingScreen`); on this machine a new game or a load comes in less than a second, and the state is rarely seen;
- **the game's time standing**: the first byte of the game's timer (`0xCD7A4C`, made by `0x6413E0`; the render thread updates the hero only while it is 0, `cEngine_handleZoomInput` zooms only then) is set while the menu over a game is open, the saves' list too, and while a window of the tutorial waits (observed); the first window of a new game does not stop it;
- **the game** otherwise.

## The menus that the driver clicks

Observed on 2026-10-21 (the window's pixels, 1024 × 768; the host's `new-game`, `save` and `load`):

- The main menu: Ancaria Campaign, Underworld Campaign, Multiplayer, Options, Load Game, Extras and Quit, centred, one under another; it takes clicks a few seconds after it first appears.
- The Ancaria campaign's hero choice: eight figures, left to right the Gladiator, the Seraphim, the Daemon, the Dwarf, the Wood Elf, the Battle Mage, the Dark Elf and the Vampiress; the one clicked lights up and gives its name to the name's field below (the name is the class's until typed); the difficulty's arrows offer the levels the hero has unlocked (Bronze alone on a fresh installation); Import, Back and Start Game.
- A new game opens on a window with OK and Skip Tutorial, centred, of a height that depends on the class's text (its buttons 573 pixels down for the Dwarf, 624 for the Seraphim, 497 for the Daemon); Enter is its OK; after OK the tutorial's first tip has OK and No more Tips, its buttons 462 pixels down.
  These are a tooltip's links (above): the host's `ui` lists them by their functions, and `ui press` presses one.
- Escape in a game opens the menu over it: Options, Save Game, Export, Quit, Continue.
  Save Game opens the saves' list, which also loads (the main menu's Load Game opens the same list with Save greyed): the saves in the order of their files' numbers, then one empty row, three rows shown and arrows to scroll a row at a time (the list keeps where it was scrolled); a row has a picture, the name's field (`<empty>` until a name is typed, and a typed name goes after what the field holds), the file's name and the date.
  Load and Save ask "OK" or "Cancel" when a save would be loaded or overwritten.

## The frame and the screenshot

`dxDriver7::flip` (`0x645E40`, the driver in `ECX`; the one driver is also at `sdk_DXDISPLAY`, `0xCDCA1C`) ends every frame of the render thread.
In a window it copies the back buffer (`+0xBC`) to the primary surface (`+0xB8`) within the window's rectangle (`Blt`), restoring the surfaces when they are lost; in full screen it flips them.
The copy takes the window's client rectangle in the screen's pixels for its source as well as its destination, so it holds only for a window of 1024 × 768, the back buffer's size (observed 2026-10-22, #551: a window that dgVoodoo2 made 3840 × 2160 gave both rectangles `0,0,3840,2160`, and every copy failed with `DDERR_INVALIDRECT`, the window black).
Full screen, the game takes DirectDraw's cooperative level with the flags `0xC11` (`DDSCL_FULLSCREEN`, `DDSCL_EXCLUSIVE`, `DDSCL_MULTITHREADED`, `DDSCL_FPUSETUP`) and sets the display mode to 1024 × 768 (observed, #551).
In a window it takes `0xC08` (`DDSCL_NORMAL`, `DDSCL_MULTITHREADED`, `DDSCL_FPUSETUP`): the two calls of `SetCooperativeLevel` side by side at `0x6451FF` and `0x64522A` (read 2026-10-23, #589).
With `DDSCL_FPUSETUP` Direct3D leaves the x87 unit's precision as it finds it; `WinMain` sets 24 bits on the main thread alone, and the render thread, which runs the world's frame, keeps 53 (measured, #589; docs/04, "Open questions").
A window outside every monitor still draws its back buffer (observed: the copy to the screen goes nowhere, and the flip reports no failure).
A locked desktop stops the frames (observed 2026-10-22 through the system's D3D7, #546): the surfaces are lost at the lock, the flip is not reached and the frames counted stand until the lock ends.
About 15 s after the unlock the scene's begin and end restore the surfaces (the log's `dxDriver7::BeginScene() surface restored`, then `EndScene()`'s complaint that the surface is gone), and the frames go on; the day's clock, which follows the wall clock, has moved on by the lock's length.
Through dgVoodoo2 (D3D7 → D3D11) the surfaces are not lost and the frames go on through a lock (docs/03 §3.8).

Before the copy, the flip takes screenshots: the driver's `+0xA8` counts the frames still to capture and `+0xAC` the frames captured.
When `+0xA8` is set, the flip locks the back buffer and calls `captureScreenshot` (`0x6487D0`) with its pixels and pitch:

- with `+0xA8` 1 and `+0xAC` 0, a single shot: the first name `.\Capture\shot%.4d.tga` that does not exist yet (formatted into `0xCDC918`), 1024 × 768 at 32 bits, 3,145,746 bytes, written at once; a thread of its own (`0x648430`) then writes a JPEG of it beside it (observed: `shotNNNN.jpg`);
- with a larger count, a run of frames into a mapped file (`vidobj`), written out afterwards as `.\Capture\cap%.4d.tga`; Escape held stops the run.

The flip then adds 1 to `+0xAC` and takes 1 from `+0xA8`.
The key that asks for a shot is not read here; the driver sets `+0xA8` and `+0xAC` itself.

## The game's log

`logPrintf` (`0x66EF40`, `printf`'s arguments) writes the game's log: when the log is on (`0x17E7F60`), into `DEBUG.LOG` (and `ExpLog.LOG`) and to the debugger's output.
With the GOG installation's settings the files stay empty but for `log closed`; the driver formats the calls itself (observed: about 330 records from the start to the main menu, the display modes of the two Direct3D devices among them).
