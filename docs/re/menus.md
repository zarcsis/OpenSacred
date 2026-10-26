# The menus: the main menu, its submenus and the choice of the hero

Seen in the GOG `Sacred.exe` 2.28 with the driver of #448 on 2026-10-26 (#1114), in its window of 1024 × 768, and read in its strings, its text keys and the menus' textures.
Status: **confirmed** by sight unless marked otherwise; the code that lays the screens out is not read.

The places below are the window's pixels, as the driver's clicks name them (`tools/original/host.py`).
The interface's classes are `cUI_MainMenu` (the main menu and its submenus), `cUI_Character` (the choice of the hero) and `cUI_Manager`, which loads their textures (the names of their RTTI and logs).
Each screen's textures come in two sets, the Underworld's (`gui_uw_*`, `ui_uw_*`) and the first game's (`gui_*`, `ui_*`), which `cUI_Manager` names in pairs; the GOG release, with the Underworld, shows the Underworld's set, and `texture.pak` lacks the first game's main menu (`gui_option1` to `12`).

## The main menu

The background is a picture of 1024 × 768 made of 12 textures of 256 × 256, `gui_uw_option1` to `gui_uw_option12`, four to a row from the top left: a stone portal in a mossy cave over water, its round doorway dark red with a red emblem.
The water at the bottom moves: from about the row 600 down, frames a second apart differ in ripples and in the reflections, which `cUI_MainMenu`'s textures `ui_uw_main_ref` (the portal reflected, its alpha a gradient) and `ui_uw_main_center` (the bed under the water, the game's name carved in it) make (**hypothesis** by their pictures and names: how they are drawn is not read).
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
`ui_uw_char_center` and `ui_uw_char_ref` are `cUI_Character`'s water as `ui_uw_main_*` are the main menu's (**hypothesis** by their names); the mist and the water move.

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
A figure is chosen by a click.

Under the figures, a panel spans the columns 285 to 740 and the rows 547 to 748, a dark frame with silver ornaments of the interface's atlases (the buttons, arrows and edges of `gui_main_12`).
At its foot are two buttons, Back (centre 415, 714) and Start Game (607, 714, `res:1029`), the latter greyed until a hero is chosen.
Above them the panel holds, before a choice, Import alone (510, 663, `UI_IMPORT`); after one, also the hero's name in a field (centre 510, 586), which starts as the class's own name with the cursor after it (`UI_DEFAULT_SERAPHIM`, `UI_DEFAULT_GLADIATOR`, `UI_DEFAULT_KAMPFMAGIER`, `UI_DEFAULT_DUNKELELF`, `UI_DEFAULT_WALDELFIN`, `UI_DEFAULT_VAMPIRIN`, `UI_DEFAULT_ZWERG`, `UI_DEFAULT_DAEMONIN`), and the difficulty between two arrows (510, 621; the right arrow at 593), Bronze first (`UI_DIFFICULTY_BRONZE`, `_SILVER`, `_GOLD`, `_PLATINUM`, `_NIOB`), which offers the levels opened alone (#1086).
While the cursor is on a figure, the panel's upper part shows that class's description in place of these, a few lines of white serif text: `res:1060` the Gladiator's, 1061 the Seraphim's, 1062 the Dark Elf's, 1063 the Wood Elf's, 1064 the Battle Mage's, 1065 the Vampiress's, 1066 the Daemon's and 1067 the Dwarf's (the Dwarf's seen; the Daemon's and the Dwarf's are also `UI_DEFAULT_DAEMONIN_INFO` and `UI_DEFAULT_ZWERG_INFO`, the Dwarf's the same string).
The descriptions mark words to stand out with `<colorw=FFFFFF80>`, the colour of the next word.
Start Game starts a game of the chosen class, name and difficulty (`cUI_Character::executeAction(SINGLE_GO) difficulty[%d] hardcore=%d`), from the class's template `Templates\Hero%.2d.ptx` (docs/02 §9), through the loading screen.
Back returns to the main menu.

The Underworld Campaign shows the same picture with no class's figures: on the pedestals stand the heroes saved in `save/` (`hero*.pax`, docs/02 §9), each labelled with its class and its level, and the free pedestals are labelled `<empty>`, a click on one doing nothing.
The GOG installation ships two of them, a Dwarf and a Daemon of level 29 (`save/hero06.pax`, `hero07.pax`, docs/02 §13), which stand on the two right pedestals.
Start Game is greyed until a hero is chosen; what a chosen hero's game starts with is not read.
