# The world map (`UI_WND_MEGAMAP`, `cUI_Megamap`)

Read 2026-10-30 (#1561) in the GOG `Sacred.exe` 2.28 and measured on frames of the running original at 1024 × 768 (a Seraphim at the start; the frames stay in the scratchpad).
Status: **confirmed** by code unless marked otherwise; "measured" means the game's pieces at the places below were compared with a frame pixel by pixel.
All places are pixels of the 1024 × 768 screen; "image N" is the entry N of the interface's images' table at `0x9ECEB0` (docs/02 §11, docs/re/interface.md); a "map pixel" is the map's own (0 to 2,047 across, 0 to 2,815 down) before its scroll is taken off.

## The window

- `cUI_Manager_createGameWindows` (`0x759CCF`) makes it, its class's vtable `0x896714`, its constructor `0x6C7540`, over the whole screen; its methods: the show and hide `0x6C7BB0`, the render `0x6C9F60`, the input `0x6C8140`, the init `0x6CB530`; `0x6C7800` and `0x6CBBB0` destroy it.
- The map is 2,048 × 2,816 (`+0xC0160`, `+0xC0162`), 88 pictures of 256 × 256 in 11 rows of 8 (`0x6CB530`): rows 0 to 2 `ui_worldMapUW<row><column>.TGA`, the Underworld, and rows 3 to 10 `ui_worldMapX<row − 3><column>.TGA`, Ancaria.
- Its children (`0x6CB530`): the ornament under the buttons, images 228 at (432, 685) and 229 at (512, 685), each 80 × 65 (`GUI_main_07`, the left half with the pin's button and the right with the close button); the pin's button (230, its other image 231) at (432, 685) with the tooltip `UI_TOOLTIP_MAP1` and the close button (232 and 233) at (558, 685) with `UI_TOOLTIP_MAP2`, 34 × 34; the bar where the edges do not scroll, from 32 left of the ornament to 32 right of it, (400, 685) 224 × 65.
- Its frame (`0x725870`, the style 1, about (48, 48) 928 × 672, images 310 to 321 of `GUI_main_12`): the corners 318 to 321 outside the rectangle's corners; the top edge 311 along it from its left, the edge's width apart, its count the rectangle's width over it, 44 above it; the bottom edge 313 at 12 below it; the left edge 310 down its left, 41 left of it, its count the height and the corner's over the edge's height, less 1; the right edge 312 at 15 right of it (**measured** to the pixel).
- The show (`0x6C7BB0`): the cursor's item put aside, the inventory and the quest book hidden, the taskbar's `UI_TB_MAP` pressed, the hero's world point projected into the scroll so that it stands at the screen's middle (**measured**: the scroll whole pixels, the map's pictures 1 pixel left at its fraction), and a single player's game paused (`0x6413E0(1)`, `0x641720(1)`); the hide gives the item back, ends the pin's mode and goes on.
- The keys (`cUI_Manager_receiveEvent`, `0x757840`): M shows it, unless another of the manager's windows or a dialogue shows, and hides it; the other letters do nothing while it shows; Esc closes it before the quest book (`cUI_Manager::handleEsc`, `0x7572C0`).

## Its projection (`0x6CB3B0`)

- A world point goes to the iso plane (`0x622620`: sx = (x − y) · 48 / 53.66563, sy = (x + y) · 24 / 53.66563, the globals `0xAD7790` to `0xAD77A4` written once by the C runtime's static initialisers; `0x622660` the inverse), then to the map pixel (sx · 0.007446 + 1018.74994, sy · 0.014914 − 186.94739).
- In cells (cx, cy) the map pixel is ((cx − cy) · 0.35741 + 1018.75, (cx + cy) · 0.35794 − 186.95): a view from above turned by 45°, about the same scale on both diagonals.

## The render (`0x6C9F60`)

1. The scroll: outside the pin's mode, the pointer within a pixel of the screen's edge (x below 1 or above 1022, y below 1 or above 766) and off the bar scrolls the map 4 pixels a frame toward each edge it is at, with the edge's cursor (`0x6C7920`: `MOUSE_DIRNW` …, docs/re/cursor.md); the right button held drags it by the pointer's motion; the scroll held within (0, 0) and (1,024, 2,048).
2. The 88 pictures (`0x6C9C60`), each at its place less the scroll; `ui_worldMapShadow.tga` is loaded but never bound: the map has no fog.
3. The places' names, the people's icons and the 13 portals (`0x6C8A40`), the frontier of the land walked (`0x6C8800`) and the pacified areas (`0x6C9330`): below, #1567.
4. The hero (`0x6C9520`): the active player's hero projected each frame, drawn as a white cross, the diagonals of a 22-pixel square about its map pixel less the scroll (a line list of two lines); in a network game the party's heroes as the image 121 too.
5. The pin (the marker 3's first value, the player's pin of the marker 2): #1569.
6. The quests' icons (`0x6C9A60`), each entry of the log at its compass's cell (`0x497540`: its `+0x18` and `+0x1C`, the cell the compass resolved, (cell + 0.5) · 53.66563 the world point): the image 138 (22 × 18) for a main quest, 139 (14 × 11) for another, each a square of 2 · ⌊w / 3⌋ (14 and 8) about its map pixel less the scroll, coloured by the entry's state (`0x4972E0`): running `0xFFFFFF80` for a main quest and `0xFF006CFF` for another, 100 `0xFF00FF00`, 101 and any other `0xFFFF0000`.
   The titles (`+0x24`) of the icons whose 16 × 16 box holds the pointer are gathered into a tooltip, in the font 2 at the pointer's right, 34 along (or at its left where it would pass the screen's edge), over grey `0x7F7F7F7F` (**seen**: the first quest's title so at the start).
   An entry without a compass has the cell (0, 0), above the map.
7. The lens about the pointer (`0x6CCBD0`): #1568.
8. The black edge: quads from black at the screen's edge to clear 70 pixels in.
9. The ornament, the buttons and their tooltips.

## The input (`0x6C8140`)

- The window's two buttons take an event first; a left press on the close button hides it as M does.
- A left press off the bar, outside the pin's mode, shows the lens (#1568), and its release hides it; the pin's button, the pin's mode and the pin's place and removal are #1569.
- No wheel and no zoom: the map's scale is fixed, the lens its only magnification.
- Alt and a left press, under the debug toggle `0x182EE38` bit `0x2000000`, teleport the hero to the point's cell and close the map (a developer's).

## The places, the people, the portals, the frontier and the pacified areas (#1567)

- The places' names (`0x6C8A40`): the list of `0x49A150`, records of 0x18 bytes (a cell, offsets as signed bytes at `+0x10` and `+0x11`, a text at `+0x14`), each whose sector has been seen (`+0x30` bit `0x400`), written in the font 3 over `ui_worldMapFontBg.tga` scrolled by the map's scroll over 512.
- The people's icons: the list of `0x49A350` (a cell and a kind), the kinds 0 to 3 `MOUSE_TRADER`, `MOUSE_BLACKSMITH`, `MOUSE_COMBO` and `mouse_horsetrader` (**seen** about the monastery at the start).
- The 13 portals (the cells of `EE_USEPORTAL`'s cases 0 to 12) whose sector has been visited (`0x1000`), the icon `0x9EF7DC`.
- The frontier (`0x6C8800`): a grid of 100 × 100 bytes at the world's `+0xB73C`, a byte a sector, rebuilt at each sector entered (`0x6331D0`, `0x6333F0`) and as a game loads (`0x633130`): each not entered sector's bit k set where its k-th neighbour has been entered (`0x20000`), each set bit a red 2 × 2 dot at the sector's corner offset by (0, −20), (10, −10), (20, 0), (−10, −10), (10, 10), (−20, 0), (−10, 10), (0, 20) and 20 down (**seen**: red dots about the monastery at the start).
- The pacified areas (`0x6C9330`): each area pacified (`0x61DB10`) with a place (`0x61DBA0`, the cells at `0xAD6F38`) the image 364 (`GUI_main_15`, turned) 32 × 32 about its cell's map pixel.
- The sectors' flags (`+0x30`): `0x400` seen, set on the sector entered and its eight neighbours at each sector change (`0x6263B0`), the first time running the sector's scripts' init; `0x1000` visited, set on the sector entered (`0x622AE0`), counted in the statistics; `0x20000` entered, kept by the saves.
- Not read: what fills the lists of `0x49A150` and `0x49A350`.

## The lens and the fog of war (#1568)

- The left button held off the bar shows over 0.2 s a view of 700 × 525 at the screen's middle of the minimap about the map's point under the pointer, the point at the screen's middle (docs/re/questbook.md, "The lens's ground"), with its fog, its quests' icons and titles laid out not to overlap, the creatures within the hero's sight by their side, the party, the places and the pin.
- The lens's view is (512 − 350a, 384 − 262.5a) 700a × 525a, its alpha a rising and falling at 5 a second as the left button holds it or not; the pointer is kept while it holds it, and the edges do not scroll while a button is held.
- The fog (`0x638F30`): each sector's explored bitmap (`+0x180`, 32 rows of 4 bytes, the bit `(dx >> 1) & 7` of the byte `(dy >> 1) · 4 + (dx >> 4)` for the cell (dx, dy) of the sector) as `FOGOFWAR01.TGA` of 32 × 32, a set bit clear and a clear bit opaque black (half black where the call's flag and `0x81B540` on `0x182EE78` are off), drawn white over the sector's own diamond: (0, 128), (256, 0), (512, 128) and (256, 256) of its picture's corner, which is the sector's area at the lens's scale, the picture's square being another tile of the same lattice.
- The stamps (`0x5997C0`), about the local hero's cell each time it changes (`0xAD4404`, `0xAD4408`), the bits of a cell below 8192 on each axis set (`0x638E00`): by day a rasterised disc 29 cells across (616 cells, the mask at `0x8915CC`), at night, 22:20 to 03:20 (`0x41BAB0`), one 15 across (145 cells, `0x891328`), in a dungeon (the renderer's `+0x9E0C`) an uneven patch of 67 cells of a 15 × 15 mask (`0x891409`); a third mask of 24 cells (`0x8914EA`) is not used there.
  The masks' rows are not those of a disc's formula: the day's runs 9, 13, 15, 17, 19, 21, 21, 23, 23, 25, 25, 27, 27, 29, 29, 29, 27, 27, 25, 25, 23, 22, 22, 21, 19, 17, 14, 13, 9 cells down its rows, the last few shifted by one; the night's 1, 7, 9, 11, 11, 13, 13, 15, 13, 13, 11, 11, 9, 7, 1.
- **Seen** in the game: the fog's edge soft (the 32 × 32 texture smoothed over the 512 × 256 diamond), as in the original's frames.
- Not read: the creatures' colours' predicates.

## The pin (#1569)

- The pin's button starts the pin's mode with the cursor `MOUSE_PUSHPIN` (−16, −16); a left release off the bar places the pin, its world point kept as the script manager's marker 2 (`0x4A63B0`, `+0x7674`), the compass's outer pin (docs/re/interface.md, "The markers"); a right press within the pin's 64 × 64 takes it away; a right press elsewhere ends the mode.
- **hypothesis**: a defect, where no pin is placed the render and the right press project the marker 3's direction as a place.

## In `opensacred-game`

Since #1561 `world_map` shows the map as above: M, the taskbar's map button and Esc, the close button, the pause, its show hiding the inventory and the book, the pictures from the scroll that centres the hero, the edges' scroll and the right button's drag, the hero's cross, the quests' icons at their compasses' cells with the titles' tooltip, the black edge, the frame, the ornament and the buttons.
**Seen** in the game beside the original's frames: the pictures, the frame, the ornament and the buttons at the Seraphim's start to the pixel; the cross and the first quest's icon at the screen's middle with its title under the pointer.
Since #1568 `opensacred_rules::explored` keeps the sectors' explored bitmaps and the stamps, `explored` stamps them about the hero at each cell it steps into, and the left button held on the map shows the lens (`world_map`): the minimap's ground about the point under the pointer, its fog over the sectors' diamonds (`minimap`), and the quests' icons at their own sizes with their titles beside them over grey (**hypothesis**: the label layout that keeps the titles apart is not followed); the map's tooltip hides under it.
Not followed yet: the layers of #1567, the lens's creatures and party (#1571), the pin (#1569), the buttons' pressed looks and tooltips, the edges' cursors.
