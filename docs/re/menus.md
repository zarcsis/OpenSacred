# The menus: the main menu, its submenus and the choice of the hero

Seen in the GOG `Sacred.exe` 2.28 with the driver of #448 on 2026-10-26 (#1114), in its window of 1024 × 768, and read in its strings, its text keys and the menus' textures.
Status: **confirmed** by sight unless marked otherwise; the code that lays the screens out is not read.

The places below are the window's pixels, as the driver's clicks name them (`tools/original/host.py`).
The interface's classes are `cUI_MainMenu` (the main menu and its submenus), `cUI_Character` (the choice of the hero) and `cUI_Manager`, which loads their textures (the names of their RTTI and logs).
Each screen's textures come in two sets, the Underworld's (`gui_uw_*`, `ui_uw_*`) and the first game's (`gui_*`, `ui_*`), which `cUI_Manager` names in pairs; the GOG release, with the Underworld, shows the Underworld's set, and `texture.pak` lacks the first game's main menu (`gui_option1` to `12`).

## The main menu

The background is a picture of 1024 × 768 made of 12 textures of 256 × 256, `gui_uw_option1` to `gui_uw_option12`, four to a row from the top left: a stone portal in a mossy cave over water, its round doorway dark red with a red emblem.
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
- Extras: a submenu of Credits (`UI_MAIN_CREDITS`), Intro (`UI_MAIN_INTRO`), Underworld Intro (`UI_MAIN_INTRO_UW`) and, after an empty row, Back, its rows' tops at 276, 309, 340 and 405; the keys `UI_MAIN_EXTRO` and `UI_MAIN_EXTRO_UW` name the two campaigns' last movies, which this installation's Extras did not show (**hypothesis**: shown once a campaign is won).
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

The Underworld Campaign shows the same picture with no class's figures: on the pedestals stand the heroes saved in `save/` (`hero*.pax`, docs/02 §9), each labelled with its class and its level, and the free pedestals are labelled `<empty>`, a click on one doing nothing.
The GOG installation ships two of them, a Dwarf and a Daemon of level 29 (`save/hero06.pax`, `hero07.pax`, docs/02 §13), which stand on the two right pedestals.
Start Game is greyed until a hero is chosen; what a chosen hero's game starts with is not read.

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
- Options opens `UI_WND_OPTIONS`, Save Game `UI_WND_SAVEGAME` (both through the window's events, `0x6BBBD0`; not followed: the options #1096 and the saves #1095 are not there), Export the character's window (`UI_CHARACTER`, not read), Continue closes the menu, and Quit asks first in `DEFAULT_BUSYDLG` (`cUI_BusyDlg`, the text `UI_BUSY_2MAINMENU`).
- That dialog darkens the whole screen (`UI_STATIC_BG`, 1024 × 768), lays its dark box (288, 288)–(736, 480) with the same frame about (320, 320)–(704, 448), its question in white in the middle, and two buttons of the choice of the hero's kind under it, OK (the text 1024) and Cancel (1025): OK leaves the game for the main menu, and Cancel closes the dialog alone, the menu staying (**seen**).

## In `opensacred-game`

The main menu, its submenus and the choices of the hero follow the sections above (`menu`, `choice`, #1118, #1119).
Since #1410 the menu over a game is the original's (`esc_menu`): Esc and the taskbar's Options button pause the game under the window at its place, its fill and the standard frame drawn with the choice of the hero's pieces (`choice::frame_about`), its items in Carolingia at the main menu's size, pale and lit white under the cursor (seen: the letters (228, 212, 169), the lit (249, 249, 249)); Options, Save Game and Export stand greyed while their screens are not there (#1096, #1095, M9), Continue goes on, and Quit opens the dialog, the screen darkened by half (**estimate**), its box, frame, question and the buttons OK and Cancel at their places (**estimate** to a few pixels), OK leaving for the main menu and Cancel, or Esc, closing it alone.
