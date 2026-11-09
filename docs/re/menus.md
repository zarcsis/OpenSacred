# The menus: the main menu, its submenus and the choice of the hero

Seen in the GOG `Sacred.exe` 2.28 with the driver of #448 on 2026-10-26 (#1114), in its window of 1024 × 768, and read in its strings, its text keys and the menus' textures.
Status: **confirmed** by sight unless marked otherwise; the code that lays the screens out is not read.

The places below are the window's pixels, as the driver's clicks name them (`tools/original/host.py`).
The interface's classes are `cUI_MainMenu` (the main menu and its submenus), `cUI_Character` (the choice of the hero) and `cUI_Manager`, which loads their textures (the names of their RTTI and logs).
Each screen's textures come in two sets, the Underworld's (`gui_uw_*`, `ui_uw_*`) and the first game's (`gui_*`, `ui_*`), which `cUI_Manager` names in pairs; the GOG release, with the Underworld, shows the Underworld's set, and `texture.pak` lacks the first game's main menu (`gui_option1` to `12`).

## The main menu

The background is a picture of 1024 × 768 made of 12 textures of 256 × 256, `gui_uw_option1` to `gui_uw_option12`, four to a row from the top left: a stone portal in a mossy cave over water, its round doorway dark red with a red emblem.
`opensacred-game` draws it as the one picture that the converter assembles from them since #1824 (`menu::Images`, docs/02 §11), else, with older assets, as the twelve tiles.
The water at the bottom moves: drops fall into it and rings spread over its bed and the portal's reflection ("The water" below).
The version, `2.28 ASE`, stands small at the bottom right corner.

The items are a column centred on the column 512, their rows 32 pixels apart:

| Row's centre | Item | Text |
|---|---|---|
| 287 | Ancaria Campaign | `UI_MAINMENU_CAMPAIGN_SACRED` |
| 319 | Underworld Campaign | `UI_MAINMENU_CAMPAIGN_ADDON` |
| 351 | Multiplayer | `res:1042` |
| 383 | Options | `res:1043` |
| 415 | Load Game | `res:1045` |
| 447 | Extras | `res:1044` |
| 511 | Quit | `res:1046` |

The row of 479 stays empty.
The text keys are the names whose hash is the string's id in `global.res` (docs/02 §8); `res:N` is the number's hash.

The items are written in Carolingia (`font/CAROLING.TTF`) about 29 pixels high: "Ancaria Campaign" spans the columns 395 to 628, as the font draws it at 29 pixels within a pixel.
Their letters are gold, light yellow at their tops (`(245, 192, 71)`) darkening to brown at their feet (`(138, 98, 30)`), with a dark outline.
The item under the cursor is lit: its letters near white (`(255, 255, 255)` to `(255, 247, 197)`), and a soft pale glow, a rectangle blurred at its edges, stands behind the word.
As a menu replaces another, its items fade in over a fraction of a second (two frames a second apart differ in the items alone after a change).

What each item does:

- Ancaria Campaign: the choice of the hero (below).
- Underworld Campaign: the Underworld's choice of the hero (below).
- Multiplayer: a submenu of Local Network (`UI_MAIN_LAN2`), Closed Internet (`UI_MAIN_INET_CLOSED`), Open Internet (`UI_NET_OPENNET`) and, after an empty row, Back, its rows' tops at 309, 341, 372 and 437.
- Options: the settings' screen (not looked at here; #1096).
- Load Game: the list of the saves (`tools/original/host.py` loads through it; #1095).
- Extras: a submenu of Credits (`UI_MAIN_CREDITS`), Intro (`UI_MAIN_INTRO`), Underworld Intro (`UI_MAIN_INTRO_UW`) and, after an empty row, Back, its rows' tops at 276, 309, 340 and 405; the keys `UI_MAIN_EXTRO` and `UI_MAIN_EXTRO_UW` name the two campaigns' last movies, which this installation's Extras did not show: the menu adds each where `Settings.cfg`'s `SHOWEXTRO` or `SHOWEXTRO_UW` is set, which the extro's own play sets, each after its campaign's intro (docs/re/movies.md).
  Each movie's row plays it (#1946).
- Quit: the game ends (`UI_BUSY_REALLY_QUIT%d` names a question, not seen: the driver's `quit` closes the window).

The main menu takes no click for a few seconds after it first shows (the driver waits 200 frames and 2 seconds).

## The choice of the hero

The Ancaria Campaign shows a picture of 1024 × 768 of 12 textures, `gui_uw_char_01` to `gui_uw_char_12`, four to a row: a canyon with a waterfall at its far end, light falling from the top left, mist over water, and eight stone pedestals in it.
Its water is the first game's `ui_char_center` and `ui_char_ref` ("The water" below): no code names `ui_uw_char_center` or `ui_uw_char_ref`, and the latter is transparent throughout (**confirmed**).
The mist is painted into the picture, half transparent over the moving water.

On the pedestals stand the eight classes, as 3D figures that move as they idle, left to right with the driver's points on them:

| Class | Point |
|---|---|
| Gladiator | 95, 480 |
| Seraphim | 215, 380 |
| Daemon | 365, 380 |
| Dwarf | 530, 430 |
| Wood Elf | 620, 330 |
| Battle Mage | 735, 400 |
| Dark Elf | 805, 330 |
| Vampiress | 890, 480 |

The figures stand dark, all but shapes; the one under the cursor and the one chosen are lit in their colours.
They stand where a table compiled into the client puts them, under a camera of their own, and wear its showcase, not their templates' items: the Daemon a sword and shield of her own, the Seraphim her wings, armour and hair, among them (docs/re/equipment.md, "The choice of the hero's figures").
A figure is lit while the cursor is in its window of that table, and chosen by a click.

Under the figures, a panel's ink spans the columns 285 to 738 and the rows 541 to 752: a dark fill and a silver frame of the interface's images, each drawn a texel to a pixel unless stretched (**confirmed** 2026-10-27 in the code and against the frames, #1221).
- The fill is black at an alpha of 159 (`0x9F`) over the columns 300 to 723 and the rows 563 to 730.
- The frame stands about the box of the columns 320 to 703 and the rows 583 to 710, outside it, drawn in this order: the halves of a crest at the middle of its top and of its foot (the interface's images 314 to 317), its corners (318 to 321), twelve edges along its top (311) and its foot (313, from 12 rows under the box), and four sides down each side, the left ones (310) as wide as the corners, the right ones (312) from 15 columns out of the box.
  The edges are drawn over the crest.
- A rule crosses the panel over the buttons, on the rows 683 to 699 from the column 300: its left end (296), twelve middles (294) and its right end (297) last, over the last middle.
- The interface's table keeps an image's corners as floats, and one whose right stands left of its left, or whose bottom above its top, draws its texels turned about (#1236): eight of the frame's images are, the right side, the foot's edge, three of the crest's halves and three corners.
- A button is the image 281 stretched over its rectangle, 37 rows tall, with its ends outside it, 280 at its left (45 columns) and 282 at its right (46): Back over the columns 368 to 463 and Start Game over 560 to 655 from the row 698, Import over 448 to 575 from the row 646.
- The name's field is the image 333 of `gui_main_13` stretched over the columns 380 to 643 and the rows 575 to 598, an emblem at its left (331, 31 columns) and its end at its right (334) as wide as the middle, both squeezed into its 24 rows (the edit control at `0x7382F0`).
- The difficulty's bar is a button's middle (281) over the columns 448 to 575 and the rows 606 to 642, its two round buttons whose arrows point down at its ends, two images, 324 at the left and 325 at the right, neither turned.
- The panel's words are of AntiquaSSK (`antqs__.ttf`), the font table's size 13, which GDI makes 17 pixels to the em, hinted and smoothed, their alpha cut to 16 levels; a line is 20 pixels, the letters' foot 16 rows under a line's top.
  Each letter's cell is the font's hinted advance at 17 pixels (`hdmx`) and 2 more, a word's letters a pixel closer, which leaves them a pixel further apart than the font's own advances, or the `kern` table's amount where it has the pair; a space is 5 pixels, and spaces in a row are one.
- Back's and Start Game's words stand on the row 720 and Import's on 668 (their letters' last row), centred on their middles, white, (255, 255, 128) under the cursor and (127, 127, 127) greyed out; the name from the column 385 on the row 591, its caret an underscore shown 0.3 s and hidden 0.3 s; the difficulty centred on the column 512 on the row 627.

At the panel's foot are Back and Start Game (`res:1029`), the latter greyed until a hero is chosen.
Above them the panel holds, before a choice, Import alone (`UI_IMPORT`); after one, also the hero's name in its field, which starts as the class's own name with the cursor after it (`UI_DEFAULT_SERAPHIM`, `UI_DEFAULT_GLADIATOR`, `UI_DEFAULT_KAMPFMAGIER`, `UI_DEFAULT_DUNKELELF`, `UI_DEFAULT_WALDELFIN`, `UI_DEFAULT_VAMPIRIN`, `UI_DEFAULT_ZWERG`, `UI_DEFAULT_DAEMONIN`), and the difficulty on its bar, Bronze first (`UI_DIFFICULTY_BRONZE`, `_SILVER`, `_GOLD`, `_PLATINUM`, `_NIOB`), which offers the levels opened alone (#1086), bronze and silver to a new hero.
The difficulty is a drop-down list (#1145): a click on either of the bar's buttons opens the list under the bar, the difficulties offered one under another, centred on the column 512, their words on the rows 652, 674 and on, 22 apart, over a box of (31, 31, 31) at an alpha of 175 from the column 440 to 583, from the row 634 down 8 rows and 20 more for each difficulty; the difficulty under the cursor turns pure yellow, a click on it chooses it and closes the list, and a click elsewhere closes it (**seen** 2026-10-27; the box and the rows **confirmed**, #1221).
While the cursor is on a figure, the panel's upper part shows that class's description in place of these, a few lines of white serif text: `res:1060` the Gladiator's, 1061 the Seraphim's, 1062 the Dark Elf's, 1063 the Wood Elf's, 1064 the Battle Mage's, 1065 the Vampiress's, 1066 the Daemon's and 1067 the Dwarf's (the Dwarf's seen; the Daemon's and the Dwarf's are also `UI_DEFAULT_DAEMONIN_INFO` and `UI_DEFAULT_ZWERG_INFO`, the Dwarf's the same string).
The descriptions mark words to stand out with `<colorw=FFFFFF80>`, the colour of the next word.
A description stands in a box of 390 × 150 pixels from the column 316 and the row 562, in the panel's letters, its first line's foot on the row 580 and a line every 20 pixels, of the colour (198, 202, 187), its marked words (255, 255, 128), the tag's `FFFF80` (**seen** 2026-10-27, #1146; **hypothesis**: the tag's first byte is the alpha).
A word stays on its line while it ends short of the box's width, and each line but a paragraph's last spreads its gaps evenly to the width, the last keeping spaces of 5 pixels (**confirmed**, #1221: the Gladiator's and the Dwarf's line breaks reproduced by these rules).
The chosen hero's own figure shows no description under the cursor; the others' do (**seen**).
Start Game starts a game of the chosen class, name and difficulty (`cUI_Character::executeAction(SINGLE_GO) difficulty[%d] hardcore=%d`), from the class's template `Templates\Hero%.2d.ptx` (docs/02 §9), through the loading screen.
Back returns to the main menu.

The Underworld Campaign shows the same picture with no class's figures: on the pedestals stand the heroes saved in `save/` (`hero*.pax`, docs/02 §9), each labelled with its name and its level, and the free pedestals are labelled `<empty>`, a click on one doing nothing; the labels are in docs/re/underworld.md, "The pedestals".
The GOG installation ships two of them, a Dwarf and a Daemon of level 29 (`save/hero06.pax`, `hero07.pax`, docs/02 §13), which stand on the two right pedestals.
Start Game is greyed until a hero of level 25 or more is chosen; what a chosen hero's game starts with, the export that writes a hero and the deletion are in docs/re/underworld.md (#1675).

## The water

Read 2026-10-26 in the code (#1127) and seen with the driver of #448: 109 drops traced, the frames of the three screens.
The menus' water is one effect, `cConsoleFX` (built at `0x75E6E0`), which each screen gives its textures and its camera and then updates (`0x75EF40`) and draws (`0x75ED40`) every frame before its picture, which covers it but where the picture is clear (**confirmed**).

| Screen | Bed | Reflection | Cave | Camera (A, B, C, D) |
|---|---|---|---|---|
| Main menu | `ui_uw_main_center` | `ui_uw_main_ref`, at its first showing `ui_main_ref` | no | 45, 1.43, 77.5, 14.4 |
| Ancaria Campaign's choice of the hero | `ui_char_center` | `ui_char_ref` | yes | 69.7, 1.63, 47.9, 12.4 |
| Underworld Campaign's choice | `ui_char_center` | none | yes | 69.7, 1.63, 47.9, 12.4 |

- The menus' start (`0x7598E0`) sets the main menu's reflection to the first game's `ui_main_ref` after the menu has set its own, so its first showing reflects the first game's pale portal; the menu's showing again (`0x711EE0`) sets `ui_uw_main_ref`, the red emblem's, back (**confirmed**, **seen**).
- The water is a grid of 64 × 64 points, x and z from −32 to 32 in steps of 64/63, its row 0 the far edge, with a height y each.
  Its world turns it about X by B radians and moves it D along z; the camera looks from (0, −C, 0) at the origin, Z down the screen, with a horizontal angle of A degrees and a vertical one of 0.75 A, its near plane at 10 and its far one at 200, over the whole screen (**confirmed**).
  So the main menu's grid spans the rows from 499 down, its middle at the row 619, and the picture leaves it clear from about the row 650: the visible water is z from about 5 to 20, the bed's carved name at the rows 735 to 765 (**seen**).
- Only the points within a disc move, (column − 32)² + (row − 32)² below 825; the others keep a height and a speed of 0.
- Each update steps the heights once, every frame of the menus, which run at 30 a second (**seen**): each point's speed falls by a hundredth while it is above 0.003, then gains a quarter of the difference between its neighbours' mean, the four beside it a sixth each and the four across a twelfth each, and its height; then each height gains its speed less the mean of the heights and speeds over the grid (**confirmed**).
  A ring spreads about 0.29 points a step.
- A drop falls when a timer of real time runs out, first at the first update, then 1.5 + 2.5 r seconds later, r even from 0 to 1 (**confirmed**; **seen** 1.53 to 3.98 seconds apart, 2.80 on the mean).
  It falls on the row 57 and the column 64 (0.3 + 0.4 r) cut to a whole number (19 to 44), and takes 0.15 from the speed of its point, 0.075 from the four beside it and 0.0325 from the four across (`0x75F7D0`).
  The row 57 lies below the screen on the main menu (the row 824) and at its edge on the choice of the hero, so a drop's point is not seen: its rings come up into sight from the bottom.
- The draw has no depth test and no lighting and blends by the source's alpha, in three passes over the grid (**confirmed**):
  1. The bed: the grid at its heights, its texture at (x + 20 Tx) / 64 + 0.5, (z + 20 Tz) / 64 + 0.5, where T = −0.7502 (V + 0.8817 N), V the unit vector from the point to (0, −60, 0) and N the point's normal, (gx, 1, gz) normalized, gx and gz the sums of three height differences across it, its row's and the two diagonals' (the sign of the differences is not read, **hypothesis**: the downhill side's).
     The client builds a table of the refraction by Snell's law at 1/1.333 whose lookup always lands on its first entry, which gives those two numbers.
  2. The reflection, where the screen has one: the grid flat, its texture over it whole, colour and alpha the texture's, each point moved by (Rx, Rz) t, where R = −0.7502 (0.8817 N + (0, 1, 0)) and t is where R from the point meets a sphere of radius 35 about (0, 20, 0), 0 where it does not; so the reflection wobbles in the middle, by up to several points, and stays at the rim.
  3. The highlights: the moved grid again, untextured and added, each point grey of 196 / (k + 1) of 255, k four times the square of the area of its moved cell cut to a whole number, 0 from k = 16: a still surface takes 39, a pale veil; a ring brightens where it squeezes the reflection and darkens where it stretches it.
- The detail of the settings (`DETAILLEVEL`, 2 in the installation) takes the steps, the drops and the highlights away at 0, and the reflection below 2.
- A second timer of 30 to 60 seconds is read by nothing.

## The sound

Read 2026-10-26 in the code (#1128) and in the original's traces of `playSFX` (`0x693510`) and the drops, about five minutes over the three screens.

- The menus' first start plays `SOUND_FX_MENU_ADDON` (6700, `mp3/MENU_ADDON.mp3`) as music, looping, after it closes every stream (the sound event (12, 2, 0), `0x67A7A0`); it plays on through the submenus and the choices of the hero, and no other music plays (**confirmed**, **seen**).
  `MUSIC_MENU` (6506) is played by no code.
- Each drop plays its sound in the same call (the sound event (9, 0x85, cave)): `SOUND_FX_WATERDRIPPLE01` or `02` (7015, 7016) on the main menu, `SOUND_FX_WATERDRIPPLE_CAVE01` or `02` (7017, 7018) on the choices, one or the other at even odds, at the effects' volume, panned 0.1 to 0.9 at random apart from the drop's column, at a rate of 0.8 to 1.2 at random (**confirmed**, **seen**: all 109 drops).
  The drop's point lies below the screen, so its sound comes a little before its ring is seen.
- While the menus show, every half a second one chance in five plays an idle sound: on the main menu one in four `SOUND_FX_IDLEFX_WOOD01` to `20` (16160 to 16179), else `SOUND_FX_IDLEFX_GLOBAL01` to `10` (16120 to 16129); on the choices one in three `SOUND_FX_IDLEFX_DUNGEON01` to `16` (16220 to 16235), else the global ones; panned 0.3 to 0.7 and at a rate of 0.8 to 1.2 at random, at the effects' volume (`0x69B770`; **confirmed**, **seen**: 122 in five minutes, 2.44 seconds apart on the mean).
  There is no loop of ambience.
- On the choice of the hero, the cursor resting on a figure for half a second plays its class's introduction, stopped as it leaves the figures or the screen: `SOUND_FX_1060` (17000) the Gladiator's, `1061` (18120) the Seraphim's, `1062` (16600) the Dark Elf's, `1063` (17370) the Wood Elf's, `1064` (17750) the Battle Mage's, `1065` (18510) the Vampiress's, `SOUND_FX_STARTUP_DEMON` (21219) and `SOUND_FX_STARTUP_DWARF` (21545), at the middle and the sample's rate (**confirmed**; **seen** for the Gladiator).
- The items make no sound under the cursor or as they are clicked (**seen**); the buttons hold `SOUND_FX_UI_CLICK01` (7000) for a press but none played.

## The menu over a game (Esc)

Seen 2026-10-29 with the driver of #448 (#1410: the Seraphim's new game, its window's controls listed by `ui`, the cursor moved over each item) and read in the code where marked.

- In a game of one player, Esc and the taskbar's Options button (`UI_TB_OPTIONS`) open the window `UI_WND_ESCMENU` (`cUI_EscMenu`) over the world, and Esc again closes it; under it the game's time stands (the driver's state `paused`).
- The window is the rectangle (380, 284)–(644, 484), 264 × 200, dark and translucent, with the frame of the choice of the hero's panel about the box (400, 304)–(624, 464): the crest's halves at the middle of its top and its foot, the corners, the edges along the top and the foot (the foot's 12 below the box) and the sides (the right's 15 out of the box), the same pieces at the same sizes (`UI_STATIC_FRAME`, its 36 controls listed).
- Its five items are texts of the class `cUI_StaticText64`, not buttons: `UI_ESC_OPTIONS`, `UI_ESC_SAVE`, `UI_ESC_EXPORT`, `UI_ESC_QUIT` and `UI_ESC_CONTINUE`, each 32 high and the window's width, at the rows 8, 44, 80, 116 and 152 of the window, their words centred (`0x6BBBD0`, **confirmed**); their words are the texts 1050, 1051, 1049, 1052 and 1053 of `global.res` (the hashes of those numbers find them in the order the window shows them).
  They stand in a golden letter, the one under the cursor lit, as the main menu's.
- Options opens `UI_WND_OPTIONS`, Save Game `UI_WND_SAVEGAME` (both through the window's events, `0x6BBBD0`; the options' window below), Export the character's window (`UI_CHARACTER`, not read), Continue closes the menu, and Quit asks first in `DEFAULT_BUSYDLG` (`cUI_BusyDlg`, the text `UI_BUSY_2MAINMENU`).
- That dialog darkens the whole screen (`UI_STATIC_BG`, 1024 × 768), lays its dark box (288, 288)–(736, 480) with the same frame about (320, 320)–(704, 448), its question in white in the middle, and two buttons of the choice of the hero's kind under it, OK (the text 1024) and Cancel (1025): OK leaves the game for the main menu, and Cancel closes the dialog alone, the menu staying (**seen**).

## The busy dialog (`DEFAULT_BUSYDLG`)

Read 2026-11-01 (#1687), **confirmed** by code unless marked otherwise, and **seen** with the driver of #448 at the Seraphim's smith of 50,44.

- `cUI_BusyDlg` (its vtable `0x897134`, its input `0x721380`) takes the UI event `0x39`, whose kind 2 sets its mode (`0x7208F0`), its line (a text of the game by its number or name) and the reactions of its two buttons (`+0x2F8` the first's, `+0x2F4` the second's), and shows it; the mode 4 hides it.
- The modes lay the buttons out against the window's rectangle `(x, y, w, h)` (`0x7208F0`), at the row `y + h − 24`: 1 and 7 the first button (`+0x238`) alone, centred along the window; 2, 5 and 6 the second (`+0x17C`) alone, centred; 3 and 8 the first from `x + 64` and the second ending at `x + w − 64`.
- A click on the first button hides the dialog and runs its reaction, on the second the second's (`0x7210E0`, the reactions `0x720A80`): 1 the main menu, 2 the game goes on (`0x6413E0(0)`, `0x641720(0)`), 3 leaves the game, 4 sends the engine its event `0x16`, 5 closes the window (`WM_CLOSE`), and the others the network's.
- The key P outside a network game (`cEngine_receiveEvent`, `0x618090`) pauses a running game (`0x6413E0`, `0x641720`) and shows the dialog in the mode 1 with the line `UI_BUSY_PAUSE` and the reaction 2; over a paused game it hides it in the mode 4.
  While the first button's reaction is 2, the dialog takes P itself (`0x50` or `0x70`): it hides, logs `busydialog: UNPAUSE` and goes on (`0x721380`).
- **Seen**: P shows the screen darkened, the box and frame of the quit's question, the line in white about the row 367 and an OK alone under it, centred at x 512 about the row 440; Esc does nothing under it; the OK and P again go on.

## The saves' window (`UI_WND_SAVEGAME`)

Read 2026-11-04 for #1990 in the code (**confirmed** unless marked) and seen with the driver of #448: the main menu's Load Game, the menu over a game's Save Game and the key S, saves made, overwritten, loaded and erased through it (docs/re/saves.md for the files and the picture).

- **The class**: `cUI_Savegame`, a `cUI_Window2` (its vtable `0x8970E4`; its constructor `0x71BD40`, its layout `0x71EFE0`, its input `0x71D8A0`, its draw `0x71E4F0`, its show and hide `0x71D720`, Escape's action 12 at `0x71D880`, Enter's action 11 at `0x71D890`). Its scroll bar, four buttons and fields are not its children: it draws and tests them itself.
- **Two windows**: the game's (`cUI_Manager_createGameWindows`, the manager's `+0xAC`) and the menus' own (`0x75A480`, `+0x13C`), each the whole screen, each keeping its list's place between its openings (seen).
- **Its show**: from the menus (no UI manager, or the manager in its menus' mode) it is for loading (its mode 2), Save not enabled; in a game for saving and loading (mode 1); outside a network game the time stands; the list is read anew and its first row selected.
- **Its openings**: the menu over a game's Save Game (`0x6BBBD0`) shows it and hides the menu, which took the save's picture as it opened; the key S in a game (`cUI_Manager_receiveEvent`, `0x757E95`) opens it at once, its picture first, where no window blocks it and the interface allows a save (`0x75ADB0`); the main menu's Load Game opens the menus' window.
- **Its layout** (`0x71EFE0`), on the 1024 × 768 screen:
  - the background, twelve tiles of 256 × 256, `UW_option_01.tga` to `UW_option_12.tga`, four to a row;
  - two fills `0x9F000000`, (44, 44) 680 × 136 and (44, 300) 680 × 392, and the choice of the hero's frame with its crest about the quick save's box (64, 64) 640 × 96 and the list's (64, 320) 640 × 352;
  - two separators `UI_STATIC_TEXTBAR` across the list (the images 296, 294 and 297, 32 × 17) at the rows 419 and 555, from x 44, 680 wide;
  - two labels on bars of the images 342 to 344 at (64, 32) and (64, 288), 137 wide: `UI_SAVE_QUICKSAVE` and `UI_SAVE_SAVEGAMES` in the font 7, white, centred;
  - four groups, each a rectangle that a click selects and a picture of 128 × 96: the quick save's (48, 48) 672 × 128, its picture at (72, 64), and the list's three rows (48, 304), (48, 439) and (48, 575), 672 × 112, their pictures at (72, 312), (72, 447) and (72, 583);
  - in each group three fields of 384 × 26 from x 264, at its picture's top, 34 and 68 below it: the save's name, its file's name and its date, in the font 1 from 4 right of and 1 below the field's corner, white, the selected group's `0xFFFFFF80`; a field's left end stands against its left edge, its middle stretched over it and its right end at its right edge, as wide as the middle (`0x7382F0`); the read-only fields take the images 332, 333 and 334, the name's field of a list's row in a game 331 (an emblem), 333 and 334;
  - the scroll bar (`0x752670`) from (720, 336), 336 high: its up arrow 287 at the top, its down arrow 289 at (720, 635), its track 288 between them and its thumb 286, which runs from the row 357 down to above the down arrow (seen);
  - the buttons Load (`UI_SAVE_LOAD`), Save (`UI_SAVE_SAVE`), Delete (`UI_SAVE_ERASE`) and Back (`UI_NET_BACK`) at x 820 and the rows 288, 352, 416 and 672, 92 × 32: the image 281 stretched over the rectangle, 280 left of it and 282 right of it; their words in the font 1, centred 4 rows up, white, `0xFFFFFF80` under the cursor and `0xFF7F7F7F` not enabled; no picture of their own for a press;
  - without a quick save, the plate of the images 1511, 1512 and 1513 side by side from (48, 48) in place of the quick save's group.
- **The list** (`0x71EAA0`): the files `GAMEnn.PAK` of `SAVE\` whose header holds the game's network flag, sorted by their names as text, the slot 0 apart as the quick save's; a save's date line is `UI_DATE_DATE`, the computer's date of the save in the user's short form (`GetDateFormatA`), ` - `, `UI_DATE_TIME` and `hh:mm` (`0x71E600`).
  Three rows show, the first held at most at the list's length less 2 in a game and less 3 from the menus (`0x71C2B0`), so that a game's list ends with one empty row, its fields `UI_SAVE_EMPTY` and its picture a translucent grey, and the menus' ends on the last save.
  The arrows and the wheel over the list scroll a row an event; the bar's place runs to the list's length less 1 while the view stops short of it, so that at the end one or two rows up change nothing (seen); after a scroll the selection stays on its place on the screen.
- **The selection** (`0x71C360`): its group's words `0xFFFFFF80` and four corners of 32 × 32 in its rectangle's corners (the images 278, 276, 279 and 277); Load and Delete only where it holds a save, Save in a game for the list's rows alone; in a game the selected row's name's field takes the keys.
- **The name's field** (`0x737AC0`): each character from `0x20` up added at its end, up to 40, Backspace taking the last, no cursor's moving, an underscore blinking 0.3 s; it holds the row's name, or `UI_SAVE_EMPTY`, until typed into, so that the typing goes after what it showed (seen), and a new save never typed into is named by the placeholder.
- **Its actions** (`0x71CC90`): Save over a save asks `UI_BUSY_SAVE_OVERWRITE` and into the empty row saves at once, into the first free slot from 1 (`0x619B80`), named by the field; Load asks `UI_BUSY_LOAD_SURE`; Delete asks `UI_BUSY_SAVE_ERASE`, erases the file and reads the list anew; the questions are the busy dialog's mode 3, OK and Cancel; Enter saves a row whose name was typed and else loads; a double press on a save loads, asking; Back and Escape close the window, which from the menus shows the main menu and in a game goes on with no menu over it.
  A save hides the window and saves behind `PAK\SAVEGAME.BMP`, the game going on; a load hides it for the loading screen.
  A save of another version asks nothing and shows `UI_BUSY_SAVEGAME_VER` with a Cancel alone.
- **Its sounds**: none (seen, `playSFX` traced): its buttons hold `UI_CLICK01`, which their own input plays, but the window never gives them its events.

## The options window (`UI_WND_OPTIONS`, `cUI_Options`)

Read 2026-11-08 for #1096 in the code, with a frame of the running original's window (opened from the main menu) to check places by sight.
Status: **confirmed** means read in code; **hypothesis** and **estimate** as marked.
All places are pixels of the 1024 × 768 screen; "image N" is the entry N of the interface's images' table at `0x9ECEB0` (docs/02 §11), its atlas and texel rectangle given where it first appears.
The fonts are the font table's entries counted from 1 (`0xA1CDB4`, entries of `0x44` bytes): font 1 is AntiquaSSK 13, font 3 AntiquaSSK 16, font 7 AntiquaSSK 8 (**confirmed** by the table's bytes; their pixel sizes as GDI makes them are not read here).

### The window, its openings and closings

- The class is `cUI_Options` (RTTI `.?AVcUI_Options@@`), a `cUI_Window2`, its vtable `0x897078` (**confirmed**):
  the constructor `0x716D30` (an object of `0x1B8` bytes), which calls the layout `0x718CA0` and hides the window (its flags 5 cleared);
  the input `0x7172F0` (slot 4); the show and hide `0x717040` (slot 9); Escape's action `0x717180` (slot 13); Enter's the base window's `0x724B50` (slot 14, not read);
  the draw `0x716E00` (slot 17, called by the base window's draw); the settings read into the controls `0x718660` (at every show); the settings written `0x717C20` (at OK).
- Its rectangle is the whole screen, (0, 0)–(1023, 767), 1024 × 768 (`0x6A57C0` with 0, 0, `0x3FF`, `0x2FF`; **confirmed**).
- Two windows are made, each with the same constructor (**confirmed**):
  the game's, by `cUI_Manager_createGameWindows` (the call at `0x75A014`), kept at the manager's `+0xBC`;
  the menus' own, by `0x75A480` (the call at `0x75A5CE`), kept at `+0x138`.
- The UI events that show and hide a window by its name are the kinds `0x11` (show) and `0x12` (hide), sent through the kernel as `cEvent` with the name, then `cEventUI2` kind 5 (**confirmed** by their uses below: the key O shows the window with `0x11` and, while it shows, hides it with `0x12`).
- The openings (**confirmed**):
  - the main menu's Options item (its label `UI_LABEL_OPTIONS`, made by the main menu's layout `0x713670` at `0x7141C1`) carries two events: hide `UI_MAINMENU` (`0x71404F`) and show `UI_WND_OPTIONS` (`0x7140D4`), which reach the menus' window;
  - the menu over a game's Options (`0x6BBBD0`) carries three: show `UI_WND_OPTIONS`, the event `0x12` of the taskbar's `UI_TB_OPTIONS` (**hypothesis**: the button let up) and hide `UI_WND_ESCMENU`;
  - the key O in a game: `cUI_Manager_receiveEvent`'s key switch (`0x757E46`: the key upper-cased less `'C'`, its bytes at `0x758DAC`, its jumps at `0x758D80`) sends O to `0x758024`, which shows the game's window where it was made; the switch is reached only while neither the saves' window (`+0xAC`) nor the options (`+0xBC`) shows and the byte `0x17EA344` is 0 (not read; **hypothesis**: a text field or the chat takes the keys);
  - the key O again while the window shows hides it (`0x758798`: the window at `+0xBC` shown and the upper-cased key `0x4F`), writing nothing, as Cancel.
- Its show (`0x717040` with 1, **confirmed**): `0x718660` sets every control from the settings (section 3); outside a network game (the byte `0x182EBEC`) the game's time stands (`0x6413E0(1)`, `0x641720(1)`, as the busy dialog's pause, docs/re/menus.md); where the dword `0xAD6EF8` is set, the engine event `0x12` with the value 0 is sent (not followed).
  Its hide (`0x717040` with 0): the time goes on (`0x6413E0(0)`, `0x641720(0)`) and the engine event `0x12` with 1.
  Setting or clearing a control's flag 1 calls its show and hide (`0x732550`, `0x7325C0`), so every way of hiding the window unpauses the game.
- The closings (**confirmed**):
  - OK (`UI_BTN_OK`): `0x717C20` writes the settings (section 3), then the window sends show `UI_MAINMENU` and hide `UI_WND_OPTIONS` (`0x717713`–`0x717965`);
  - Cancel (`UI_BTN_CANCEL`): the same two events, nothing written (`0x71796C`–`0x717BC1`);
  - Escape (`0x717180`): the window clears its own flags 5 (hides) and, the byte `+0x154` being 0, sends show `UI_MAINMENU`; nothing is written.
    The constructor sets `+0x154` to 0 and no code writes it again (a scan of the byte stores at `+0x154`), unlike the saves' window, whose game instance gets 1.
  - Out of a game the main menu comes back.
    In a game no window is named `UI_MAINMENU` (only `0x75A480`, the menus' manager, makes one), so the game goes on with no menu over it (**seen** 2026-11-08 with the driver of #448: OK, Cancel, Escape and the key O again each left the game going on with no menu, for the window that the key O showed and for the one that the menu over a game's Options showed).

### The controls

#### How the controls work

- A control's flags (`+0x10`, **confirmed**): 1 shown, 4 takes input, `0x10` checked or pressed, `0x8000000` the hover glow (below); a static text's `0x80000` centres its words, `0x100000` puts them against the right edge, neither against the left (`0x754140` turns them into the font's draw flags 1 and 4).
- The labels are `cUI_StaticText64` (vtable `0x897700`, draw `0x754140`), made by the window's `0x7254C0` with a name, a rectangle, a text of `global.res` by its key, a font, flags and a colour; every label of this window is white (`0xFFFFFFFF`) and has no colour of its own under the cursor (its flag `0x400` unset); the words start at the rectangle's top row and are not clipped (**hypothesis** for the clipping: the detail's label is 64 wide and its words run past it in the frame).
- The ticks and the round choices are `cUI_Button2` (the constructor `0x72A8F0`, vtable `0x897308`, the input `0x72B560`, the draw `0x72B700`), made with the flags `0x8000005`, each the size of image 323, 35 × 36:
  - drawn as image 323 (`GUI_main_12.TGA` (220, 58)–(254, 93), an orb with a yellow tick) while its flag `0x10` is set, and as image 272 (`GUI_main_12.TGA` (1, 77)–(35, 112), a plain blue orb) while it is clear (the draw takes the texture `+0x80` and the coordinates `+0x54`–`+0x60` for a set flag, `+0x84` and `+0x64`–`+0x70` for a clear one; the layout puts image 323's in the first pair and image 272's in the second; **confirmed**, and the frame's ticked orbs, the third detail's and the sound's among them, agree with the installation's `DETAILLEVEL` 2 and `SOUND` 1);
  - a left press inside it plays its sound (`+0x18`, `UI_CLICK01` 7000, docs/re/sound.md) and turns its flag `0x10` over (`0x72B560`);
  - the hover glow (`0x731F00`, `0x732220`, flag `0x8000000`): while the cursor is over it, the control is drawn again 49 times, moved −6 to 6 in steps of 2 along each axis, its colour's alpha the byte ⌊0.3 · 12.75⌋ = 3, with the engine's state `0x10` on (**hypothesis**: the destination blend ONE, a soft additive halo); on a display of fewer than 32 bits the alpha is 16 and the steps 4 (**confirmed**).
- The window's input (`0x7172F0`, **confirmed**) gives the event to the four sliders first, then to the choices; each control's own input turns its tick over, and for the three groups of round choices the window then sets the pressed one's flag `0x10` and clears the others', so one of a group always holds (a press on the ticked one leaves it ticked).
  The order: the base window's own input (`0x726FF0`), the four sliders (the first that takes the event stops the others), the detail's, the quality's, the collect's and the data flow's groups; then, only where neither the base window nor a slider took the event, the ticks of the atmospheric animation, sound on, follow, fog of war and autosave; then OK and Cancel; FSAA and violence (this one only while it takes input, its flag 4) last, again only where nothing took it before.
- The sliders are `cUI_Scrollbar` (the constructor `0x752210`, vtable `0x897664`, the input `0x752A60`, the draw `0x752EE0` not read; **confirmed** unless marked):
  - `0x752670` lays a horizontal slider (wider than high) out of four images: image 263 (`GUI_main_12.TGA` (1, 1)–(37, 37), a round "−" button) at its left edge, 37 × 37; image 265 ((80, 1)–(119, 37), a round "+" button) against its right edge, 40 × 37; image 264 ((40, 1)–(77, 37), the bar) over (x + 37, y), (w − 77) × 37 (kept at `+0x1B0`; **hypothesis** that the draw stretches it over that); image 262 ((121, 1)–(150, 37), the knob), 30 × 37, its top at y + h/2 − 18;
  - the knob runs over the inner track from x + 52, w − 107 wide (the bar less the knob's width, its half at each end);
  - `0x753070` sets the number of positions `n` (126 for the volumes, 255 for the transparency); the place is a fraction `f` from 0 to 0.9999 (`+0x8C`), the position `⌊f · n⌋` held below `n` (`0x753430`), set as `p / n` (`0x7533B0`);
  - a left press on "−" takes one position off (none below 0), on "+" adds one (up to 0.9999); a press anywhere else on the slider puts the knob at the point, `f = (x − inner x) / inner width` held to 0 … 0.9999 (`0x7530C0`);
  - the wheel over it moves one position, its direction by the bit `0x80` of the event's byte `+0x11`; the keys Page Up and Page Down move 5, Home to 0 and End to the last;
  - while the left button is held (`0x655920`, bit 8 of the mouse's state; **hypothesis**: the left button), the window's draw `0x716E00` sends each of the four sliders a press at the cursor every drawn frame, so a held "−" or "+" moves one position a frame and a held press on the bar drags the knob after the cursor;
    **seen**: a trace of `0x752A60` while "+" was held gave the four sliders a press at each of the frames that the button stayed down, one after the input's own, and a click held for one frame moved the volume two positions, for two frames three;
  - the sliders play no sound (no call of `playSFX` in `0x752A60`).

#### The panels' controls

Every rectangle is (x, y) w × h; "right-aligned" and "centred" are the label flags above; the labels' font is 3 unless marked.

**Graphic settings** (the box (64, 64) 384 × 128):

| Control (name) | Class | Rectangle | Look, text |
|---|---|---|---|
| `UI_LABEL_GFXDETAIL` | label | (64, 116) 64 × 16, left | `UI_CFG_GFX_DETAIL` |
| `UI_BTN_DETAILSETTING_1` (`+0x160`) | round choice | (416, 56) 35 × 36 | images 323 / 272 |
| `UI_LABEL_DETAILLOW` | label | (272, 60) 128 × 16, right-aligned | `UI_CFG_LOW` |
| `UI_BTN_DETAILSETTING_2` (`+0x164`) | round choice | (416, 93) | |
| `UI_LABEL_DETAILMID` | label | (272, 97) 128 × 16, right-aligned | `UI_CFG_MEDIUM` |
| `UI_BTN_DETAILSETTING_3` (`+0x168`) | round choice | (416, 130) | |
| `UI_LABEL_DETAILHIGH` | label | (272, 134) 128 × 16, right-aligned | `UI_CFG_HIGH` |
| `UI_SLIDER_TRANSPARENCY` (`+0x1B4`) | slider, 255 positions | (260, 167) 192 × 32 | images 263, 265, 264, 262 |
| `UI_LABEL_MAPTRANS` | label | (66, 167) 192 × 32, left | `UI_CFG_MAPTRANS` |

**Sound volume** (the box (544, 64) 384 × 128); the three sliders have no name of their own, their labels are named after them:

| Control | Class | Rectangle | Look, text |
|---|---|---|---|
| SFX (`+0x1A8`) | slider, 126 positions | (648, 64) 288 × 32 | images 263, 265, 264, 262 |
| `UI_SLIDER_SFX` | label | (504, 68) 128 × 16, right-aligned | `UI_CFG_SFX` |
| speech (`+0x1AC`) | slider, 126 positions | (648, 112) 288 × 32 | |
| `UI_SLIDER_VOICE` | label | (504, 116) 128 × 16, right-aligned | `UI_CFG_SPEECH` |
| music (`+0x1B0`) | slider, 126 positions | (648, 160) 288 × 32 | |
| `UI_SLIDER_MUSIC` | label | (504, 164) 128 × 16, right-aligned | `UI_CFG_SOUND` |

**Auto-collect** (the box (64, 288) 384 × 128; its separator at the row 310):

| Control | Class | Rectangle | Look, text |
|---|---|---|---|
| `UI_LABEL_ANIM` | label | (80, 278) 320 × 16, left | `UI_CFG_ANIM` |
| `UI_BTN_COLLECTANIM` (`+0x178`) | tick | (416, 276) 35 × 36 | images 323 / 272 |
| `UI_LABEL_COLLECT` | label | (80, 368) 320 × 16, left | `UI_CFG_COLLECT` |
| `UI_BTN_COLLECT1` (`+0x16C`) | round choice | (416, 330) | |
| `UI_LABEL_COLLECT1` | label | (272, 334) 128 × 16, right-aligned | `UI_CFG_AUTO_ALL` |
| `UI_BTN_COLLECT2` (`+0x170`) | round choice | (416, 367) | |
| `UI_LABEL_COLLECT2` | label | (272, 371) 128 × 16, right-aligned | `UI_CFG_AUTO_UNIQUE` |
| `UI_BTN_COLLECT3` (`+0x174`) | round choice | (416, 404) | |
| `UI_LABEL_COLLECT3` | label | (272, 408) 128 × 16, right-aligned | `UI_CFG_AUTO_GOLD` |

**Sound options** (the box (544, 288) 384 × 128; its separator at the row 310):

| Control | Class | Rectangle | Look, text |
|---|---|---|---|
| `UI_LABEL_SNDPLAYBACK` | label | (560, 278) 320 × 16, left | `UI_CFG_SOUNDPLAYBACK` |
| `UI_BTN_SOUNDONOFF` (`+0x18C`) | tick | (896, 276) | images 323 / 272 |
| `UI_LABEL_SNDQUALITY` | label | (560, 368) 320 × 16, left | `UI_CFG_SOUND_QUALITY` |
| `UI_BTN_SNDQ_LOW` (`+0x190`) | round choice | (896, 330) | |
| `UI_LABEL_SNDQ_LOW` | label | (752, 334) 128 × 16, right-aligned | the text `UI_BTN_SNDQ_LOW` |
| `UI_BTN_SNDQ_MID` (`+0x194`) | round choice | (896, 367) | |
| `UI_LABEL_SNDQ_MID` | label | (752, 371) 128 × 16, right-aligned | the text `UI_BTN_SNDQ_MID` |
| `UI_BTN_SNDQ_HIGH` (`+0x198`) | round choice | (896, 404) | shown only where `0x692B60` answers yes |
| `UI_LABEL_SNDQ_HIGH` or `UI_LABEL_SNDQ_NO3D` | label | (752, 408) 128 × 16, right-aligned | the text `UI_BTN_SNDQ_HIGH`, or, where `0x692B60` answers no, `UI_LABEL_SNDQ_NO3D` and the choice hidden (its flags `0x8000005` cleared) |

`0x692B60` is not read (**hypothesis**: whether the sound library found a 3D provider; the frame, with no 3D card, shows the line `UI_LABEL_SNDQ_NO3D` and no orb).

**Other options** (the box (64, 512) 384 × 160):

| Control | Class | Rectangle | Look, text |
|---|---|---|---|
| `UI_BTN_FOLLOW` (`+0x184`) | tick | (416, 507) | images 323 / 272 |
| `UI_LABEL_FOLLOW` | label | (80, 511) 320 × 16, left | `UI_CFG_AUTOFOLLOW` |
| `UI_BTN_AUTOSAVE` (`+0x1A0`) | tick | (416, 544) | |
| `UI_LABEL_AUTOSAVE` | label | (80, 548) 320 × 16, left | `UI_CFG_AUTOSAVE` |
| `UI_BTN_EXPLOREMAP` (`+0x19C`) | tick | (416, 581) | |
| `UI_LABEL_EXPLORE` | label | (80, 585) 320 × 16, left | `UI_CFG_EXPLORE` |
| `UI_BTN_VIOLENCE` (`+0x188`) | tick | (416, 618) | hidden (flags 5 cleared) where `VIOLENCE` is 2 |
| `UI_LABEL_VIOLENCE` | label | (80, 622) 320 × 16, left | `UI_CFG_VIOLENCE`, made only where `VIOLENCE` is present and not 2 (`0x81A180`, which, where the key is missing, writes 0 and answers no, so a file without the key hides the option for the window's life) |
| `UI_BTN_FSAAFILTER` (`+0x1A4`) | tick | (416, 655) | |
| `UI_LABEL_FSAAFILTER` | label | (80, 659) 320 × 16, left | `UI_CFG_FSAA_FILTER` |

**Multiplayer** (the box (544, 512) 384 × 64):

| Control | Class | Rectangle | Look, text |
|---|---|---|---|
| `UI_LABEL_LANSPEED` | label | (560, 535) 320 × 16, left | `UI_CFG_LANSPEED` |
| `UI_BTN_NETSPEED_LOW` (`+0x17C`) | round choice | (896, 512) | images 323 / 272 |
| `UI_LABEL_NETSPEED_LOW` | label | (752, 516) 128 × 16, right-aligned | `UI_CFG_AB_DSL` |
| `UI_BTN_NETSPEED_HIGH` (`+0x180`) | round choice | (896, 549) | |
| `UI_LABEL_NETSPEED_HIGH` | label | (752, 553) 128 × 16, right-aligned | `UI_CFG_AB_ISDN` |

**The foot**: `UI_BTN_CANCEL` (`+0x158`) at (604, 644) and `UI_BTN_OK` (`+0x15C`) at (804, 644), each `cUI_Button2` of the flags 5 (no glow), 95 × 37 (image 281's width 38 times 2.5, its height 37), their words `UI_CANCEL` and `UI_OK` (section 5).

Every place above is **confirmed** in `0x718CA0` and agrees with the frame (the ticks' orbs, the right-aligned words ending at the columns 400 and 880, the sliders' "−" and "+").

### What each control reads and writes

- The settings are `Settings.cfg`'s keys in a store in memory (the object `0x182EE78`, its store at `0x182EE88`): each getter (those named below, `0x819C00` to `0x81F0D0`) reads its key from the store every call (`0x662F60`) and, where the key is missing, writes its default (`0x662DF0`) and answers it (**confirmed**).
- At OK, `0x717C20` writes each key into the store (`0x662DF0`) and then writes the whole file at once (`0x662AD0`: one line `KEY : value` a key; **confirmed**).
- At every show, `0x718660` sets the controls from the store; so whatever was not confirmed by OK is gone at the next opening (**confirmed**).

| Control | `Settings.cfg` key (getter, default where missing) | Values | Written at OK | When it takes effect |
|---|---|---|---|---|
| detail low / medium / high | `DETAILLEVEL` (the byte `0x182EE78`, read at the start by `loadSettings`, `0x814152`) | 0, 1, 2 | the key and the byte (`0x717DB3`–`0x717EF9`) | at OK: every reader reads the byte when it runs (section 4) |
| map transparency | `MINIMAP_ALPHA` (`0x81B8A0`, 127) | 0–254 | the slider's position (`0x81BA50`) | at OK: the overview map's render reads it every frame (`0x6C6CF2` in `0x6C6C50`), its show too (`0x6C7330`) |
| SFX, speech, music | `SFXVOLUME`, `VOICEVOLUME`, `MUSICVOLUME` (`0x81AEC0`, `0x81ACE0`, `0x81B0A0`, 64) | 0–125 | the sliders' positions | at once while dragged: `0x716E00` gives the sound system `p / 126` (`0x677A60` the effects', `0x677FB0` the speech's, `0x677750` the music's) at every step; at the start `initApp` gives it the key's value times 1/128 (`0x8906F8`), 0.5 where missing |
| atmospheric animation | `PICKUPANIM` (`0x81A690`, 1) | 0, 1 | the tick | at OK |
| collect everything / gold and uniques / gold | `PICKUPAUTO` (`0x81A4E0`, 0) | 0, 1, 2 | the choice | at OK |
| sound | `SOUND` (`0x819C00`, 1; it first passes the key `SOUND3D` to `0x6634C0`, **hypothesis**: dropping an old key) | 0, 1 | the tick | at once at the click: the window gives the sound system `0x6902F0(not on)`, `0x678180(on)` and `0x6781A0(on)` (`0x717696`); at the start `initApp` gives `0x6902F0(not SOUND)` |
| sound quality low / all effects / 3D | `SOUNDQUALITY` (`0x819E20`, 2) | 0, 1, 2 | the choice | at OK, where it changed: `0x6780C0(quality)` (`0x7182A7`); at the start `initApp` (`0x81727B`) |
| follow opponents | `AUTOTRACKENEMY` (`0x81A330`, 0) | 0, 1 | the tick | at OK |
| autosave | `AUTOSAVE` (`0x81B6F0`, 0; it answers yes the once it writes the default, docs/re/saves.md) | 0, 1 | the tick | at OK |
| fog of war on the map | `EXPLOREMAP` (`0x81B540`, 1) | 0, 1 | the tick | at OK |
| violence | `VIOLENCE` (`0x819FD0`, 0) | 0, 1; 2 hides the option | the tick, only where `VIOLENCE` is not 2 (`0x81A180`) | at OK |
| FSAA filter | `FSAA_FILTER` (`0x81C870`, 0) | 0, 1 | the tick, then `0x81C870` refreshes the settings object's cached byte `+2` | at OK |
| data flow DSL / modem | `NETWORK_SPEEDSETTINGS` (`0x81F0D0`, 2) | 2 DSL (`+0x17C`), 1 modem (`+0x180`); another value ticks neither | the choice and `setNetSpeedMode` with the same number | at OK |

- The radio groups are set at the show from the values 0, 1 and 2 in order (detail, collect, quality); the transparency's and the volumes' positions from their numbers (`0x7533B0`), the volumes' read as text and turned to numbers (`0x84B24A`) (**confirmed**).
- What Cancel (and Escape, and O) restores: the store and the file keep their values, and the next show sets the controls from them; but the volumes dragged and the sound's tick act on the sound system at once, and neither Cancel, Escape, O nor the hide sets them back, so the running game keeps the dragged volumes and the sound's state until the next start, when the file's old values are read again (**confirmed** by the absence of any call in those paths; **hypothesis** that the difference is heard).
- The volume's scale differs between the two paths: the window gives `p / 126`, the start `p / 128`, so the loudest position, 125, is 0.992 while the window is open and 0.977 after a restart (**confirmed**).
- No control writes `SCREEN_QUAKE`, `SHOW_ENEMYINFO`, `WARNING_LEVEL` or the other keys of the file: `0x717C20` writes the fifteen above alone (**confirmed**).
- The window's own sounds: every `cUI_Button2` (the ticks, the choices, OK and Cancel) plays `UI_CLICK01` as pressed; (**seen**: a sample started at a tick's press and at Cancel's, none at a slider's);
  the sound quality's choices play a second `UI_CLICK01` from the window (`0x7172F0`): low and medium where the sound system's byte `+0x80` is 1, 3D where it is 0 (**confirmed**; **hypothesis**: `+0x80` is the 3D provider's use, docs/re/sound.md, so the second click marks a change of the mode).

### What the settings change in play

- **`DETAILLEVEL`**: the byte `0x182EE78` is read 118 times in the code (a scan of `mov al, byte ptr [0x182EE78]`); the readers already described:
  - the liquids' animation at 1 or more, the clear waters' second layer at 2 (docs/re/liquids.md);
  - the statics' shadow list at 1 or more (`0x62BA03`), the models' shadows at 1 or more, else a few dots (`0x407960`), and the flagged items' `cGranny::renderComplex` while not 0 (`0x44A8C0`) (docs/re/drawing.md);
  - the weather drawn only at 1 or more (docs/re/weather.md);
  - 30 farm and wild animals' types left out of the sectors at 0 (docs/re/spawning.md);
  - the hidden corpses destroyed 25, 10 or 7 s after the death at 2, 1 or 0 (`0x553DC0`, docs/re/fight.md);
  - the effects' emission intervals times 1.0, 1.5 or 2.2 at 2, 1 or 0 (docs/re/effects.md), which the hundred-odd readers from `0x76B896` to `0x7C71C0` are (**hypothesis**: the effects' classes);
  - the items' glitter, 3, 5 or 8 sparks (docs/re/items.md);
  - the menus' water's steps, drops and highlights at 0 and its reflection below 2 (docs/re/menus.md).
  Not followed: the readers at `0x408E06`, `0x409F96`, `0x40EA06`, `0x4191CB`, `0x419325`, `0x419690`, `0x5BAF79`, `0x5C97B0`, `0x62DD56` and `0x62E138` (#2352).
  The 30 animals (read 2026-11-08 for #2342, **confirmed**): the types 513 to 516, 520 to 528, 530 to 533, 540, 541, 544, 560 to 563, 565, 566, 589, 590, 592 and 593, by the table of a byte a type from 513 at `0x47F774` (0 leaves the type out), the record's entries removed before the creatures are chosen, unless the record has `08`, in a game of one player.
- **`MINIMAP_ALPHA`** (the map's transparency): the overview map (Tab) reads it at its show (`0x6C7330`, keeping 255 − the value at `+0x15C`) and in its render (`0x6C6C50`, the call at `0x6C6CF2`), which draws the sectors at the alpha 255 − the value, the lens raising an alpha below 96 to 96 (docs/re/maps.md); so the slider's left end is opaque and its right end the most transparent (**confirmed**).
- **`PICKUPAUTO`** (the collect modes): the Collect's filter `0x4EC330`: 0 every item, 1 gold, runes, certain potions and the items above a bar of quality by the hero's level, 2 gold alone (docs/re/items.md).
- **`PICKUPANIM`** (the atmospheric animation): the hero's pick-up clip (`0x4EC560`, `0x53A170`, `0x5368E6`…; without it the items are taken at once, docs/re/items.md) and the mount's and the dismount's clips (docs/re/horses.md).
- **`AUTOTRACKENEMY`** (follow opponents with the mouse): `cEngine_scrollViewAndWarpCursor` (`0x6105B0`) reads it as the view scrolls by the hero; with it on, no window of the three `0x75A8B0`, `0x75A8D0`, `0x75A890` open, and the local hero (`0x603E30`) walking (its action `+0x150` 1) toward a target (`+0x104` in the AI states 4 or 8, `+0x158` in the states 6 or 11), it moves the system cursor by the view's scroll on the screen (`GetCursorPos`, `SetCursorPos`), so that the cursor stays on the foe (**confirmed** that it moves the cursor by the scroll; **hypothesis** the meaning of the states); the three windows are the manager's `+0xAC` (the saves'), `+0xBC` (the options) and `+0x94` (**confirmed** read 2026-11-08).
- **`EXPLOREMAP`** (the minimap's fog of war): `0x638F30`; the overview's unexplored ground opaque black with 1, half black with 0 (docs/re/maps.md).
- **`FSAA_FILTER`**: cached at the settings object's `+2` (`0x81C870`, read by `0x81CA30`), read by two draws of the client (`0x62E410` at `0x62EC75`, `0x62FF60` at `0x6308AD`); when on, they move a quad's texture coordinates in by 1/256 at each edge (`0x89208C`) (**confirmed**; **hypothesis**: the ground's tiles, against the seams that the anti-aliasing shows, docs/09-ecosystem.md).
- **`SOUND`**: read at the start (`loadSettings` `0x81520B`, `initApp` `0x817285`) and given to the sound system as a mute (`0x6902F0`); the tick acts at once (section 3).
- **`SOUNDQUALITY`**: given to the sound system at the start and at OK (`0x6780C0`) and read by its start (`0x676200`, which logs it); 2 with the DirectSound3D hardware provider places the samples in 3D (docs/re/sound.md); what tells 0 from 1 is not read.
- **`NETWORK_SPEEDSETTINGS`**: read by the network's `0x7EDEA0`, set by `setNetSpeedMode` at OK (not followed).
- **`AUTOSAVE`** and **`VIOLENCE`**: docs/re/saves.md and docs/re/effects.md, "The blood fountain".

### The panels, the background and the buttons' looks

- **The background** (**confirmed**): twelve statics `UI_STATIC_BG` of 256 × 256, the textures `UW_option_1_01.tga` to `UW_option_1_12.tga` (`texture.pak`'s `uw_option_1_01` …) whole, four to a row from (0, 0), rows 256 apart, each a static of the mode 5; the window covers the whole screen, so in a game the world is hidden under it (**seen**: the key O's window in a game).
- **The fills** (`0x725640`, **confirmed**): six rectangles of black at the alpha 159 (`0x9F000000`), each its box grown by 20 on every side: (44, 44) 424 × 168, (524, 44) 424 × 168, (44, 268) 424 × 168, (524, 268) 424 × 168, (44, 492) 424 × 200 and (524, 492) 424 × 104.
- **The panels' frames** (`cUI_Frame`, the constructor `0x73A7A0` with a name, a kind, a rectangle and flags; the images chosen by `0x73D860`, laid out by `0x73ADC0`):
  - the six boxes, kind 1 with the flags 1, `UI_FRAME_GFXSETTINGS` (64, 64) 384 × 128, `UI_FRAME_SNDVOLUME` (544, 64) 384 × 128, `UI_FRAME_AUTOCOLLECT` (64, 288) 384 × 128, `UI_FRAME_3DSOUND` (544, 288) 384 × 128, `UI_FRAME_GENERALSETTINGS` (64, 512) 384 × 160 and `UI_FRAME_INTERNET` (544, 512) 384 × 64: the images 310 to 321 of `GUI_main_12.TGA`, the frame with its crest of the choice of the hero's panel, the menu over a game and the saves' window, drawn outside the box (**confirmed** the images; the pieces' places by `0x73ADC0`'s kind 1 are not read again here and are taken from docs/re/menus.md, **hypothesis** that they are the same code's);
  - the six titles' bars, kind 2 with the flags 1, `UI_FRAME_LABEL_GFXSETTINGS` (152, 32), `UI_FRAME_LABEL_SNDVOLUME` (632, 32), `UI_FRAME_LABEL_AUTOCOLLECT` (152, 256), `UI_FRAME_LABEL_3DSOUND` (632, 256), `UI_FRAME_LABEL_GENERAL` (152, 480) and `UI_FRAME_LABEL_INTERNET` (632, 480), each 200 wide (their box's x + 88 and y − 32, so centred over the crest): image 342 (`GUI_main_13.TGA` (165, 63)–(196, 94)) at the left, image 343 ((66, 63)–(97, 94)) repeated from x + 32 every 32 columns ⌊(w − 64)/32⌋ + 1 times (5 for 200), image 344 ((198, 63)–(229, 94)) last against the right end, each 32 × 32 at its own size (**confirmed**);
  - the two separators, kind 2 with the flags 4, `UI_FRAME_SEP_AUTOCOLLECT` (44, 310) and `UI_FRAME_SEP_SNDQUALITY` (524, 310), 424 wide: image 296 (`GUI_main_12.TGA` (152, 19)–(183, 35)), twelve of image 294 ((173, 1)–(204, 17)) and image 297 ((185, 19)–(216, 35)), 32 × 17 each, the pieces of `UI_STATIC_TEXTBAR` (**confirmed**);
  - the kind 2's other flags take 345, 343, 346 (flags 2) and 290, 291, 292 (flags 8), unused here.
- **The titles** (`0x7254C0`, **confirmed**): `UI_LABEL_GFX` (`UI_CFG_GFX_SETTINGS`), `UI_LABEL_VOLUME` (`UI_CFG_SOUND_VOLUME`), `UI_LABEL_PICKUP` (`UI_CFG_AUTOPICKUP`), `UI_LABEL_SND_Q` (`UI_CFG_SOUND_QUALITY_TITLE`), `UI_LABEL_VARIOUS` (`UI_CFG_VARIOUS`) and `UI_LABEL_WAN` (`UI_CFG_WAN`), each in its bar's rectangle (200 wide, as high as its box, its words from the bar's top row), font 7, white, centred.
- **OK and Cancel** (**confirmed**): image 281 (`GUI_main_12.TGA` (47, 39)–(84, 75)) stretched over the rectangle, image 280 ((1, 39)–(45, 75), 45 wide) left of it and image 282 ((88, 39)–(133, 75), 46 wide) right of it, the same three for the pressed state (`0x72B460(280, 282, 280, 282)`, both textures image 281's), so there is no pressed look; the words in the font 1, centred along the button, set 3 rows up (`0x72B4B0(0, −3)`) from the middle of the button less half the font's height of `A`, white (`+0xA8`), `0xFFFFFF80` under the cursor (`+0xAC`), `0xFF7F7F7F` when not enabled; no glow (the flags 5).
  So Cancel's ends span the columns 559 to 744 and OK's 759 to 944, the rows 644 to 680, as the frame shows.
- The ticks' and the choices' looks: images 323 and 272 and the hover glow (section 2).

### Open

- The kind 1 frame's pieces' places in `0x73ADC0` (taken from the choice of the hero's panel; not re-read).
- The slider's draw (`0x752EE0`): where exactly the knob is drawn for a fraction (**estimate**: its middle at the inner track's x + f · width, which the frame's knobs at the volumes 0 agree with to a few pixels).
- The hover glow's blend (the engine's state `0x10`, **hypothesis** additive) and its alpha on a 32-bit display (3 per copy).
- What the engine event `0x12` sent at the show and hide where `0xAD6EF8` is set does.
- The byte `0x17EA344` that keeps the key O (and the others of the switch) from the manager.
- `0x692B60`'s test for the 3D choice, and what `SOUNDQUALITY` 0 changes against 1.
- The readers of `DETAILLEVEL` listed as not followed, and the effects' range `0x76B896`–`0x7C71C0` taken as the effects' intervals.
- `NETWORK_SPEEDSETTINGS`' reader `0x7EDEA0` and `setNetSpeedMode`.
- The fonts 1, 3 and 7's pixel sizes as GDI makes them (font 1 as the choice of the hero's buttons, docs/re/menus.md).
- Enter's action (the base window's `0x724B50`).
- Seen in the running original: the window, its places and its sounds, the held sliders, the key O and the closings in a game; the live volume after Cancel is #2341's to check.

## In `opensacred-game`

The main menu, its submenus and the choices of the hero follow the sections above (`menu`, `choice`, #1118, #1119).
Since #1676 the Underworld campaign's choice stands the installation's two heroes on their pedestals in their summaries' items, labelled as the original labels them, chooses one of level 25 or more by a click, its name in the field and the difficulties it is offered in the list, and lights Start Game, whose game is #1677's; the choice is of a pedestal in both campaigns (docs/re/underworld.md).
Since #1990 the main menu's Load Game opens the saves' window for loading (`saves_window`, the menu's screen `Load`): its layout above, the slots' saves of `saves/` (`GAMEnn.sav`, the slot 0 the quick save's; `saves::slots`) with their pictures (#1991), the list's place kept between its openings, its first row selected, a click selecting a group, the arrows and the wheel scrolling a row, Load and a double press asking before they load, Delete asking before it erases, Enter loading, Back and Esc going back to the main menu, no sound; the date in the user's short form, the pattern `sShortDate` that `GetDateFormatA` takes, in the computer's time zone.
In a game the menu over it's Save Game, greyed while the interface allows no save, and the key S (`controls.toml`'s `saves`, its picture taken before the window opens) show the game's own window for saving, the game paused under it with no menu over it: the list ends with an empty row, the list's rows' names stand in their editable fields (the emblem 331), the selected one's takes the characters typed after what it shows and Backspace, its caret an underscore blinking 0.3 s, the other keys taken from the game; Save over a save asks `UI_BUSY_SAVE_OVERWRITE`, into the empty row saves at once into the first free slot from 1, named by the field, the placeholder too; Enter saves a row whose name was typed and else loads; a save, Back and Esc go on with the game.
Since #1410 the menu over a game is the original's (`esc_menu`): Esc and the taskbar's Options button pause the game under the window at its place, its fill and the standard frame drawn with the choice of the hero's pieces (`choice::frame_about`), its items in Carolingia at the main menu's size, pale and lit white under the cursor (seen: the letters (228, 212, 169), the lit (249, 249, 249)); Options, Save Game and Export stand greyed while their screens are not there (#1096, #1095, M9), Continue goes on, and Quit opens the dialog, the screen darkened by half (**estimate**), its box, frame, question and the buttons OK and Cancel at their places (**estimate** to a few pixels), OK leaving for the main menu and Cancel, or Esc, closing it alone.
Since #1687 the dialog is a module of its own (`busy_dialog`), which the quit's question takes, and the key P (`controls.toml`'s `pause`) pauses a running game under it with the line `UI_BUSY_PAUSE` and an OK alone, the OK or P going on, Esc and the Options button doing nothing under it; P under the menu over a game does nothing.
Not followed: the dialog's other modes and reactions, which the network's and the loading's waits take.
Since #2340 the main menu's Options (the menu's screen `Options`), the menu over a game's Options and the key O (`controls.toml`'s `options`) show the options window above (`options`): its background, fills, frames, titles, separators, labels, ticks and round choices with their glow under the cursor (`control_glow`), sliders and buttons at the original's places, its words the game's texts by their names (`menu::Words`, which the menus and a game share); each show sets the controls from the settings, OK writes their fifteen keys into `settings.toml` (docs/10 §3) and Cancel, Esc and O again write nothing, each going back to the main menu, or on with the game paused under the window.
A slider takes a press as the original's: the input's and its frame's draw's, then one every sixtieth of a second while the button is held (docs/decisions/interface.md); the wheel over it moves a position, up raising it (**estimate**: the bit of the original's event is not read).
The 3D sound's choice is hidden and its line says that no 3D sound card is found, as the original's on a computer without a 3D provider (docs/decisions/interface.md).
What the settings change in play is, since #2342, the detail's readers of the list above (the liquids' animation and the clear waters' second layer, the statics' and the models' shadows, the weather, the animals left out of the fills at 0, the glitter's sparks and the menus' water's steps, drops, highlights and reflection; the effects' intervals, the models' effects, the shadows' dots and the corpses' times #2352's) and the autosave's and the violence's, which act since #1989 and #1964, the volumes and the sound's switch since #2341 (docs/re/sound.md, "The volumes"), and since #2343 the map's transparency, the fog of war, the atmospheric animation, the collect modes and the cursor that follows the foe (docs/re/maps.md, docs/re/items.md, docs/re/horses.md; `follow_foe`: while the hero walks to the foe it attacks, the system's cursor moves by the view's scroll on the screen, only while the window has the focus); the FSAA filter and the data flow change nothing (docs/decisions/interface.md).
