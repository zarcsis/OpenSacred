# The quest book (`UI_WND_QUESTBOOK`, `cUI_Diary`)

Read 2026-10-30 (#1560) in the GOG `Sacred.exe` 2.28 and measured on frames of the running original at 1024 × 768 (a Seraphim at the start; the frames stay in the scratchpad).
Status: **confirmed** by code unless marked otherwise; "measured" means the game's pieces at the places below were compared with a frame.
All places are pixels of the 1024 × 768 screen; "image N" is the entry N of the interface's images' table at `0x9ECEB0` (docs/02 §11, docs/re/interface.md).
Implemented in `opensacred-game` (`quest_book`, `minimap`, `statistics`) and `opensacred-rules` (`quests`).

## The window

- `cUI_Manager_createGameWindows` makes the diary (`0x6AE880`, made at `0x759E98`) at (64, 64), 832 × 576; a control's place adds its parents' (`0x732350`).
- Its methods (vtable `0x8963E4`): events `0x6AED40`, the test of a point `0x6AF1B0`, show and hide `0x6AEAC0`, the draw `0x6AEEC0`; the init `0x6AF440`, the main tabs' layout `0x6AF250`, a main tab's selection `0x6AF0E0`.
- The picture of the book is six statics of 256 × 256 (`GUI_BOOK_BASE1` to `6`) from (128, 128), three across and two down: 768 × 512 from (128, 128) to (896, 640).
- A point is the window's from x 128 to 896 and y 64 to 640, or on a visible bookmark of the selected tab's book (`0x6B35F0`), which stick out left of x 128.
- The close button (image `0x167`, `GUI_main_15`, 34 × 34) stands at (861, 129).
- Each page's parchment is four statics over the picture: for the left page from x 128 and the right one from x 512, (0, 0) and (0, 256) of 256 × 256 and (256, 0) and (256, 256) of 128 × 256 of the page, from y 128.
  Each page takes a set of four images (`0x9E05D4`, the sets `0x5A7` to `0x5B6`, `GUI_BOOK_SHEET1A` …, the C picture split in two halves), the set rolled once as the window is made, ⌊`rand()` / 32767 · 3⌋, so the fourth only where `rand()` answers 32767; the pages roll apart.
- The draw (`0x6AEEC0`): the six pieces, the main tabs (the one under the pointer with its tooltip at its corner plus (32, 0), the style `0x20B`), the selected book's bookmarks (`0x6B2B10`, its pass 1), the parchment, the close button, then the selected book's pages and arrows (its pass 0); so the tabs and the bookmarks show only where they stand out of the parchment.

## The main tabs

- Four tabs of 64 × 102 (`GUI_BOOK_TABS`, `GUI_BOOK_TABS2`) at x 164 + 72 · i, at y 96, the selected one at y 72 (`0x6AF0E0`): the quests' (image `0x5BD`), the hero's, the information's (`0x5BC`) and the statistics' (`0x5BF`), their tooltips `UI_TOOLTIP_QBOOK_QUESTS`, `_HERO`, `_INFO` and `_STATS`.
- The hero's tab's image by the hero's type (`0x603E30`'s `+0x10`): 1 `0x5B7`, 2 `0x5B8`, 3 `0x5BB`, 4 `0x5BE`, 5 `0x5B9`, 6 and 7 `0x5BA`, 8 `0x5C1`, 9 `0x5C0`, any other `0x5B7`; the draw lays the tabs out again whenever the type changes.
- The selected tab is kept as the book hides and shows, the quests' as the game begins.
- A click on a tab selects it with the sound event `(14, 2, 9)`, and the books but the statistics' are built again (below), with their own sound.

## The books

- Each tab has its book (`cUI_Book`, `0x6AFD50`, its type the tab): seven chapters, each a list of pages and a list of items, a page its rows (`+0` flags, `+4` the title's text, `+8` the item, `+0xC` the log's index, `+0x10` the icon's image) and its selected row, an item its title, up to ten lines, and a picture.
- A book is built again (`0x6B1990`) as the book shows, as its tab is selected, and as a side quest is tracked: the list's page and its selected row kept where they still stand, the chapter selected again with the sound event `(14, 2, 10)`, the other pages' selections lost.
  So the book shows the log as it stood at its build: an entry made while it shows comes at the next show.
- The fonts (the table `0xA1CDB4`, entries of `0x44` bytes, the face's name then its size): 1 AntiquaSSK 13, 5 Carolingia 16, 6 Carolingia 18, 7 AntiquaSSK 8 (read 2026-10-31, #1611); the titles and the counts in 6, a main quest's row in 5, a side quest's row and every text in 1, the statistics' legend in 7.
  **Measured**: the text's lines 20 apart, a line of the title 28 high (the text under one line of title starts 8 lower than under none).

## The quests' book (type 0)

### Its chapters and their bookmarks

- Its builder (`sdk_JOURNAL_BUILD`, `0x6B07E0`) goes through the quest log in its order (`0xAAD3A4`, entries of `0x174` bytes) and takes the entries with a title (`+0x24`): an entry of the chapter c + 1 (`+0x16C`) into the chapter c, the chapter 0 into the first.
- The chapters' titles (`0x9E07CC`): the texts 9000 to 9004, none for the last two.
- The bookmarks (`0x6B3F90`, made once): the chapters 0 to 4 at (86 − j, 184 + 80 · c), the chapters 5 and 6 at (866 − j, 184 + 80 · (c − 5)), 64 × 64, each j a roll of ⌊`rand()` / 32767 · 4⌋.
  Their images and lit images (`0x9E075C`): `0x49D`/`0x4A2`, `0x49E`/`0x4A3`, `0x49F`/`0x4A4`, `0x4A1`/`0x4A6`, `0x4A0`/`0x4A5`, `0x4CD`/`0x4D1`, `0x4CE`/`0x4D2`.
- A bookmark shows while its chapter holds an entry; the selected one is lit, and one under the pointer for the frame.
- A click on one selects its chapter (`0x6B1880`): the list's first page where the chapter changed, the right page's first page, the sound event `(14, 2, 10)`.

### The left page

- The chapter's title in red (`0xFF990000`), centred in (152, 160) 300 wide.
- The list from (160, 196), 284 wide; a row's text from x 184, a side quest's 16 further, wrapped in 284; each row takes its height and 1 more.
- An ornament row (`GUI_main_08`, images `0xEE` and `0xEF` side by side, 25 high) is first on every page, pushed as the page takes its first entry, and comes before a main quest whose log entry follows one of another kind; a chapter without entries has no row at all, its page empty under the title (**seen** 2026-11-09 at the Underworld campaign's start, #2367).
- A page ends before an entry once its rows reach 334 below the list's top (`0x6B1D90`), a row of text measured as its lines at its font's line height.
- An entry's row: its icon (16 × 16, `GUI_book_x3`) at (160, top + 2); a main quest's running `0x496`, lost `0x497`, won `0x498`; a side quest's running `0x49A`, tracked `0x499`, lost `0x49B`, won `0x49C`.
  Its colour, each rule over the one before: `0xFF000033`; ended `0xFF3F3F3F`; entered since the game began (`+0xC` bit 0, which no save keeps) `0xFF660000`; the page's selected row `0xFF000066`; under the pointer `0xFF000099`, or `0xFFFFFF80` for one entered since the game began.
- The page's count, "page/pages", centred in (160, 576) 284 wide in `0xFF000066`.
- A row is under the pointer within its first line, across the list (`0x6B3640`).
  A left click beside its icon (from x 176) selects it; on a side quest that runs it also makes it the one the compass's inner pin follows (`0x49DAB0`, the manager's `+0x3A8` by the player) and builds the book again; a click on the icon, or another button's, only tracks, and selects the row where it does.
- An `ExitQuest` or a `LoseQuest` of the tracked quest drops it.

### The right page

- The item shown is the selected row's, else, while the page's ornament is selected as at its first show, the chapter's first.
- An entry's item: its title, its first line, then its other lines without repeats among them; only nine lines show.
- The title in red, centred and wrapped in (540, 160) 316 wide.
- The text's rectangle from x 548, 300 wide, from y 196 + T − 20 to 548, T the title's height.
- The picture on the text's first page, for an entry whose compass has a place (`0x497540`: its `+0x18` and `+0x1C`, the cell the compass resolved, **seen**: the novice's cell): the lens's ground (below) in a view of 256 × 256 at (556, the text's top), the text 264 lower.
  The cell is written by the markers' resolve (`0x4A5980`, read 2026-11-22 for #3110, **confirmed** by code): for the newest main and side quests' entries as the compass, the overview or the map asks them, an object's target its object's cell at each ask (`0x4A5DDD`, `0x4A5E4D`), a target in a hidden sector, the hero outside, the way's cell (`0x4A61A1`; docs/re/interface.md, "The markers (`sdk_MARKER_RESOLVE`, `0x4A5980`)"); a new entry copies the waiting compass's cell, which its writers set to 0 alone (`0x46C475`; `0x46CBBB`, `0x48F82C`).
  It is written too by `sdk_KOMPASS_RESOLVE` (`0x4A6450`), which `QuestKompassPos` calls on the entry as it sets its compass: a cell's target its cell, an object's its object's cell (read 2026-11-23 for #3480, **confirmed** by code; docs/re/interface.md, "The markers (`sdk_MARKER_RESOLVE`, `0x4A5980`)").
  So an entry of a cell's target has that cell from its compass's record on, and one asked no longer keeps its last (**hypothesis** for the latter).
  **Seen** 2026-11-23 for #3480: the quest 9 of the Seraphim's tree, entered by the driver's `script SelfTriggerQuest9`, its compass the cell (3201, 2473), showed its picture of the river bank by that cell and its icon there on the world map; the quest 1111, its compass a cell inside a dungeon, showed the dungeon's mouth outside, the way's cell (docs/re/interface.md, "The markers (`sdk_MARKER_RESOLVE`, `0x4A5980`)").
  Its point (`0x6B0C33`): the cell's middle on the iso plane (`0x622620`), less 2,248 across and plus 420 down (the floats at `0x89647C` and `0x896478`), truncated; the lens shows it at the screen's middle, so that the place stands about the picture's middle (**measured**).
- The text: its lines with a blank line between (`0x9E086C`), in `0xFF000066`, justified, a paragraph's last line set left (the flags `0x2500`, **seen**), poured page after page, each page under the title; the pages counted by pouring them unseen until one takes nothing.
- The page's count centred in (548, 576) 300 wide.
- The arrows, 32 × 32: the list's back and on at (156, 581) and (444, 581), the text's at (540, 581) and (828, 581); back `0x493` and on `0x495`, dark green, lit `0x492` and `0x494`, yellow, under the pointer (**seen**: the on arrow dark green with the pointer away); back shows above the first page, on below the last; their clicks are silent.

## The lens's ground (`0x6CCBD0`)

The world map's lens draws the minimap about a point of the iso plane; the quest book calls it for its picture with its fog, its icons, its creatures and its pin off (the arguments 0 and `0xFF`).

- The scale: a twelfth of the iso plane (`0x891070`), a cell 4 × 2 pixels; the point stands at the screen's middle (512, 384), whatever view the call clips to (**measured**: the book's picture matches the pictures laid so, to the pixel).
- The ground: the 25 sectors within three steps (|dx| + |dy| ≤ 3) of the point's sector, each its picture `MINIMAP<x><y>` (`texture.pak`, three digits each), 256 × 256, its corner at ((x − y) · 256 − 256, (x + y) · 128) of the lens's plane (**measured**); a sector without its picture black.
- The sectors' looks (read 2026-11-22 for #3418 and 2026-11-23 for #3479 in the code, its disassembly and its jump tables, **confirmed** by code; the pictures' fades **seen** in the running original 2026-11-25 for #3609, "The fades seen" below, the rest a **hypothesis** wherever the data does not bear it out, as marked): the 25 sectors are found by the cells 64 apart about the point's sector (`0x634970`), through the table of the grid's places that `cWorld::precalcSectorLookup` (`0x638730`) fills as the world loads, and drawn a column of one x − y at a time, from the point's x − y less 3 to its x − y plus 3, each column from the top down, each sector by one of two ways.
  - Every record of `sectors.keyx` becomes a sector as the world loads (`0x638160`), and they are kept until a game's load makes them all again (`0x63B580`), whatever the sector cache reads of their cells (docs/02 §5.1); so the lookup finds no sector only at a place of the grid that no record takes, or off the grid.
  - A place without a sector, or a hidden sector (`0x6238C0`, docs/re/interface.md, "The markers (`sdk_MARKER_RESOLVE`, `0x4A5980`)") where the call's flag for it is set, is drawn black at the call's alpha (its last argument, 255 for the map and the book, raised to 96 where it is lower), with no texture (`0x6CDADA`): first the square of its picture, then the diamond of its own place, from the square's left middle through its top right and bottom right corners to the point 256 right of its right middle.
    The next column's pictures are drawn after it, over that diamond's right half.
    A hidden sector is struck from the list before it is drawn, so the fog after the ground and the quests' icons of the listed sectors (`0x497620`: the log's entries whose compass's cell lies in one of them) leave it out too.
    The world map's lens sets that flag (`0x6CA81A`); the quest book's picture (`0x6B26E6`) and the overview (`0x6C6D04`, a register that its draw clears at `0x6C6C91`) do not, so only the world map's lens shows the hidden sectors, the dungeons' but those of the two blocks, black, the book's picture and the overview their pictures.
  - A sector is its picture (`0x622DA0`: `MINIMAP` with its x and y in three digits each, loaded at its first draw and kept at its `+0x178`, its `+0x30` bit `0x4000` set where there is none) drawn as one triangle strip of six vertices (`0x6CD6B5`): the square's top left, top right, left middle, right middle, bottom left and bottom right in that order, each at the same point of the texture, so that the strip splits the upper half from the top right to the left middle and the lower half from the right middle to the bottom left (**seen**: the other diagonal of the lower half does not fit the frames, "The fades seen" below).
    Each vertex is white at the call's alpha, or black at it where the links and the switch below say ("Per vertex" below); the colours blend linearly across each of the strip's four triangles, Gouraud's shading, the alpha the same at every vertex, so that the picture darkens toward each black vertex (**seen** for #3609).

**The picture's square and its sector.**
A picture's square is not its sector's diamond (**confirmed** on the pictures): the sector's diamond of 512 × 256 has its left corner at the square's left middle, its top and bottom corners at the square's top right and bottom right corners and its middle at the square's right middle, where the lens lays the fog's diamond; so the square shows the sector's left half, above it the lower right quarter of the sector (x − 1, y), whose middle is the square's top left, and below it the upper right quarter of (x, y + 1), whose middle is the square's bottom left.
On the masters' 6,046 pictures (2026-11-23), the top left triangle is black (its mean below 8 of 255) in 757 of the 818 pictures whose (x − 1, y) has no sector and the bottom left one in all 827 whose (x, y + 1) has none, against a mean of 63 where those sectors are.
So the six vertices lie on these sectors: the top left is the middle of (x − 1, y); the top right the corner of (x, y), (x − 1, y), (x − 1, y − 1) and (x, y − 1); the left middle the corner of (x, y), (x − 1, y), (x − 1, y + 1) and (x, y + 1); the right middle the middle of (x, y); the bottom left the middle of (x, y + 1); the bottom right the corner of (x, y), (x, y + 1), (x + 1, y + 1) and (x + 1, y).

**The links.**
A sector's `+0x04` to `+0x20` are its eight neighbours' sector ids, the eight `u16` from its record's `+0x2C` (docs/02 §5.1), which the loaders copy (`0x638160`, and `0x637D30` for a `sectors.key`) and nothing else writes (the constructor `0x623720` clears them; a sector's part of a save, `0x623240`, holds none of them).
On the file each names the neighbour of the table below and is 0 exactly where the grid has no sector there (**confirmed** on all 8 × 6,050 words, 2026-11-23); so a link is empty only toward a place of the grid without a sector, never toward a sector, read into memory or not, a dungeon's or not.
The lens reads seven of them (`0x6CD327`–`0x6CD3C2`), each empty one setting a bit of the sector's outcode (as the client's debug line names it) and turning black the vertices that lie on the missing sector:

| Link | Neighbour | Its square by the sector's | Bit | Black |
|---|---|---|---|---|
| `+0x04` | (x − 1, y − 1) | above | `0x01` | top right |
| `+0x08` | (x, y − 1) | the next column right, half up | `0x40` | top right |
| `+0x0C` | (x + 1, y − 1) | two columns right | — | not read: its diamond touches no vertex of the square |
| `+0x10` | (x + 1, y) | the next column right, half down | `0x20` | bottom right |
| `+0x14` | (x + 1, y + 1) | below | `0x10` | bottom right |
| `+0x18` | (x, y + 1) | the next column left, half down | `0x08` | left middle, bottom left, bottom right |
| `+0x1C` | (x − 1, y + 1) | two columns left | `0x04` | left middle |
| `+0x20` | (x − 1, y) | the next column left, half up | `0x02` | top left, top right, left middle |

**The outcode's switch.**
Then a switch over the whole outcode (`0x6CD3E1`: its index table at `0x6CFEF0`, its jump table at `0x6CFEC4`) turns more vertices black for 18 of its values; any other value adds nothing.

| Outcode | Its missing neighbours | Black beyond the links' | All black |
|---|---|---|---|
| `0x01` | above | top left | top left, top right |
| `0x07` | above, up left, two left | none | top left, top right, left middle |
| `0x10` | below | bottom left | bottom left, bottom right |
| `0x20` | down right | right middle | right middle, bottom right |
| `0x21` | above, down right | top left, right middle | top left, top right, right middle, bottom right |
| `0x30` | below, down right | right middle, bottom left | right middle, bottom left, bottom right |
| `0x38` | down left, below, down right | right middle | the lower four |
| `0x3C` | two left, down left, below, down right | right middle | the lower four |
| `0x40` | up right | right middle | top right, right middle |
| `0x41` | above, up right | top left, right middle | top left, top right, right middle |
| `0x43` | above, up left, up right | right middle | the upper four |
| `0x47` | above, up left, two left, up right | right middle | the upper four |
| `0x60` | down right, up right | right middle | top right, right middle, bottom right |
| `0x61` | above, down right, up right | top left, right middle | top left, top right, right middle, bottom right |
| `0x63` | above, up left, down right, up right | right middle | all but the bottom left |
| `0x70` | below, down right, up right | right middle, bottom left | top right, right middle, bottom left, bottom right |
| `0x71` | above, below, down right, up right | top left, right middle, bottom left | all but the left middle |
| `0x73` | above, up left, below, down right, up right | right middle, bottom left | all six |

The upper four are the top left, the top right, the left middle and the right middle, the lower four the left middle, the right middle, the bottom left and the bottom right; "up left" is (x − 1, y), "down left" (x, y + 1), "up right" (x, y − 1), "down right" (x + 1, y) and "two left" (x − 1, y + 1).
So the right middle, the sector's own middle, turns black only by the switch, and stays white in another combination with a missing neighbour on the right (16 values of 33 sectors on the file, such as `0x2C` and `0x7F`); a picture with no empty link is white all over.
On the file 2,546 sectors have an empty link among the seven, in 53 values of the outcode, 1,814 of them in the 17 of the switch's values that occur (`0x73` does not).

**Per vertex.**
The links and the switch together black each vertex of a sector's square so (**confirmed** by code, **seen** for #3609 as "The fades seen" below tells):
- the top left, the middle of (x − 1, y): where (x − 1, y) has no sector (`0x02`), or where the outcode is `0x01`, `0x21`, `0x41`, `0x61` or `0x71`;
- the top right, the sector's top corner: where any of (x − 1, y − 1), (x, y − 1) and (x − 1, y) has none (`0x01`, `0x40`, `0x02`);
- the left middle, its left corner: where any of (x, y + 1), (x − 1, y + 1) and (x − 1, y) has none (`0x08`, `0x04`, `0x02`);
- the right middle, the sector's own middle: by the switch alone, for each of its 18 values but `0x01`, `0x07` and `0x10`;
- the bottom left, the middle of (x, y + 1): where (x, y + 1) has none (`0x08`), or where the outcode is `0x10`, `0x30`, `0x70`, `0x71` or `0x73`;
- the bottom right, its bottom corner: where any of (x + 1, y), (x + 1, y + 1) and (x, y + 1) has none (`0x20`, `0x10`, `0x08`).

The outcode is the seven bits of the links' table above, the link to (x + 1, y − 1) left out; a white vertex is (255, 255, 255) and a black one (0, 0, 0), both at the call's alpha.
A pixel of the square is its picture's texel times the colour interpolated across its triangle of the strip, (top left, top right, left middle), (top right, left middle, right middle), (left middle, right middle, bottom left) or (right middle, bottom left, bottom right), on the texel's stored value, as the fixed pipeline modulates a texture by the diffuse colour, with no gamma (**seen**: the frames fit a blend linear in the stored values to their rounding, where a blend in linear light would leave a black vertex's midpoint at about 0.73 of the texel, not 0.5).
So a black vertex darkens every triangle of the strip that holds it, from black at the vertex to none at the triangle's opposite edge: a ramp 256 pixels across and 128 down at the lens's one scale, in the overview, the world map's lens and the book's picture alike.

**The fades in play.**
The links follow the grid, not what the sector cache reads, so the pictures fade only where the grid lacks a neighbour (**confirmed** on the file for the links, and **seen** for #3609, "The fades seen" below): along the outer ring of the continent and of the few small islands of sectors that are no dungeon's, 691 of the 3,622 such sectors; around the four holes of the grid inside it, 49 places within x 11 to 18 and y 30 to 43, 35 within x 55 to 66 and y 37 to 46, 15 within x 46 to 52 and y 47 to 51, and the place (54, 46); and around every dungeon's island, 1,855 of the 2,428 dungeon sectors.
The places beyond them are black, as are the hidden sectors' places in the world map's lens, toward which their neighbours do not fade, their links being whole (**confirmed** by code; not seen, the 43 shown sectors beside a hidden one lying on dungeons' islands and on the island of the sectors 0 to 3, 96 to 99).
**Seen** 2026-11-23 for #3479 (#3601; the driver of #448, the Dwarf of a new game, frames kept):
- the sector 56,39 has no cell to stand on within 24 of its middle, nor 56,40 and 57,40 (the hole of x 55 to 66), so the overview was taken from 55,39 and from its east side at x 3,555 to 3,560, where the cells nearer the hole may not be stood on: the fog of war covered every place but the discs about the cells the hero stood on, and none of them reached the hole's side, so no fade toward the missing neighbours showed;
- on the island of the dungeon's sectors 92 to 94, 16 to 19 (the Upper Underworld, `dungeon` in the world's index), the overview showed its ground in the discs the hero had explored, while the world map's lens held there over the hero's place was black all over, its pointer alone drawn: the island's sectors hidden, as the code has it;
- so the fades themselves were not seen there, the fog of a new game lying over the places where they fall (seen since, below).
About the start (50, 39) no picture of the 25 fades, the nearest sector with an empty link being (56, 39), six steps away at the hole east of it; the lens about 1,913 of the 3,622 sectors that are no dungeon's shows a faded picture, and about 1,430 of them a black place.
So in a game the overview fades the pictures within about four sectors of the coast, of the holes or, inside a dungeon, of its island's edge, the quest book's pictures likewise about their places, and the world map's lens likewise about the pointer, with the dungeons black.

**The fades seen.**
**Seen** 2026-11-25 for #3609 (the driver of #448, the Seraphim of a new game in god mode, the frames kept in the scratchpad):
- The island of the nine sectors 20 to 22, 94 to 96, no dungeon's and with no sector about it, was explored whole by the driver's teleports 15 cells apart, each stamping the day's disc, and the overview framed at each of its sectors' middles with the world's time stopped, beside a frame of the same scene without the overview.
- The lens's ground is such a frame less the world under it at the overview's alpha (128 of 255); over each square it was fitted as the converted picture times six vertex colours blended across the strip's triangles, by least squares where the fog of war was clear.
- Every vertex of the nine squares came out 0 or 1 within 0.05, as "Per vertex" above gives, for the outcodes `0x00`, `0x07`, `0x1C`, `0x1F`, `0x30`, `0x3C`, `0x41`, `0x47` and `0x71`, but the top left of the three western squares, where their pictures are black; `0x1C` and `0x1F`, no values of the switch, black only what their links black, the right middle white.
- The fit's residual stayed at the frames' rounding, 1.5 to 1.8 a pixel over its three channels, so the blend is linear across each triangle and the alpha unchanged, a black vertex showing the world under the overview at 1 − alpha; with the lower half split from the left middle to the bottom right instead, the residual rose to 8 to 15 on the squares whose lower half fades, and a blend in linear light missed the ramps by 15 a channel against 0.7 to 3.
- The switch's other values were seen where the hero could stand on the vertex they black, the overview about it: the top left of `0x01` (4,18 and 63,46), the bottom left of `0x10` (10,23, 13,54, 34,23 and 74,45), the right middle of `0x20` (60,46), `0x38` (35,23), `0x40` (55,18), `0x43` (56,45 and 62,46), `0x61` (50,15, and 68,26 with its top left), `0x63` (58,17) and `0x70` (9,27, with its bottom left), each black within 0.08; the other vertices of those squares that the fog let be seen came out as the rule gives, the white ones at 0.65 to 1.15 where the fog's soft edge lay near; `0x21` and `0x60` (the sectors 8,26, 10,30 and 11,31, with no cell to stand on near their vertices) and `0x73`, which no sector has, were not seen.
- An unexplored neighbour is not a missing one: at the cell 3840,3840 in the middle of the continent, the corner of the sectors 59 and 60 by 59 and 60, explored only by the hero's disc there, the ground under the disc was its pictures' own (1.00 to 1.03 of them), where the three squares' vertices that meet there would be black if the fog counted; the lens reads the links alone, as the code has it.
- The lens kept its one scale, a twelfth of the iso plane, at the game's zooms 0.5, 1 and 2 (the same ground and fit at each), so the ramps of "Per vertex" are 256 pixels across and 128 down on the screen whatever the zoom.
- The overview's point is the engine's world view's, which near the grid's rim stood off the hero (47 pixels of the lens aside of it, 564 of the iso plane, at the island's side sectors), so the overview there is not about the hero (the view's stop at the grid's rim, docs/re/drawing.md, "The view's point", #3730).
- At the overview's alpha a place without a sector showed the world at a quarter of its light where two blacks lie over each other (a place's square and its own diamond, or its diamond's right half under the next column's square, black there by its links and its picture) and at a half under one (a picture's corner that its vertices black whole), a step along the island's edge, as the order above has it.

`opensacred-game` follows the lens's order and its black since #3479 (`minimap::spawn`, `minimap::spawn_fog`, `world_map::quests_in_lens`, `daynight::Dungeons::hides`, `opensacred_rules::explored::hidden`): the 25 sectors drawn a column at a time from the left, each from the top down; a place without a picture black, its square and then its own diamond, which the next column's pictures cover; and on the world map's lens the hidden sectors so too, struck from its list, no fog and no quest's icon over them.
Since #3609 each picture fades as "Per vertex" above tells (`minimap::blacks`, `minimap::shade`, `minimap::fade`): its sector's outcode taken from the world's grid, a picture with a black vertex shown as a copy whose texels' stored values are multiplied by the colours blended across the strip's triangles, its first level's BC7 blocks decoded for it, made as it first shows and kept for the sector's next draws.
**Seen** 2026-11-25 in the game beside the original's frame of the island of the sectors 20 to 22, 94 to 96 at its middle: the pictures at the island's edges darkening toward the black places in ramps where they had met them in hard edges, the corners of the overview's window as dark as the original's.

## Sounds and keys

- Every show and hide plays `UI_QUESTBOOK_CLOSE01` (`(14, 2, 8)`, 7104); a tab's or a chapter's selection `UI_QUESTBOOK_TURN01` (`(14, 2, 9)` and `(14, 2, 10)`, 7106), so a show plays both, and a tab's click `TURN01` twice.
- L (`0x757840`) shows and hides it, unless the world map or another of the manager's windows shows; the taskbar's `UI_TB_BOOK` (594, 705) too.
- Esc closes the world map first, else the book, else the inventory, else the other windows, else opens the menu over the game (`0x7572C0`).
- The book's show hides the inventory; the inventory's show hides the book (docs/re/inventory.md).

## The other books

Read in the code for #1565; **seen** in the game where said.

### What the hero's and the information's books share

- Their rows have the flags `0x11`: drawn in font 1 as a side quest's row without its indent and icon, `0xFF000033`, the selected row `0xFF000066`, the one under the pointer `0xFF000099`; a left click from x 176 selects one (`0x6B3940`), and the quests' book's tracking does nothing (the handler returns before it for a type other than 0).
- A new page starts where the next row would pass the page's end (`0x6B1D90`), but the hero's skills, which all go on the chapter's first page (at most eight).
- The left page's title is the chapter's for every page of it; the right page shows the selected row's item, the first row's until another is clicked.
- A page may have a text of its own (`+8`, a "non-list" page): that text is drawn in the list's rectangle, (160, 196) 284 × 352, in font 1, `0xFF000066`, wrapped (the flags `0x500`, **hypothesis** for their sense), in one piece; its right page shows the item whose index is the right page's current page number (`book + 0x160`, `0x6B2E3A`; a quirk, 0 while the right page is not turned).
- The bookmarks stand as the quests' book's, each shown while its chapter holds something.
- The chapters' titles (`0x9E07CC`, seven `u16` per type): the hero's 0, 9030, 9031, 9032, and 9033 that its builder writes itself (`0x6B15E1`) for the fifth; the information's 9021, 9020, 9022, 9023, 9024; the statistics' none.
  The title 0 is the empty string (`0x672C90`), so the class's page has no title on either page.
- The bookmarks' images (`0x9E075C`, a pair of the image and its lit look per chapter): the hero's `0x4BD`/`0x4C1` (replaced by the class's below), `0x4B6`/`0x4BB`, `0x4C8`/`0x4CC`, `0x4C0`/`0x4C4`, `0x4BC`/`0x4AC`; the information's `0x4B2`/`0x4B7`, `0x4B3`/`0x4B8`, `0x4B4`/`0x4B9`, `0x4B5`/`0x4BA`, `0x4AB`/`0x4B1`; 64 × 64 cells of `gui_book_x4` to `gui_book_x7`.
- The class's bookmark (`0x6B3E20`, the hero's book alone): the chapter 0's pair by the hero's type, 1 `0x4BD`/`0x4C1`, 2 `0x4BF`/`0x4C3`, 3 `0x4BE`/`0x4C2`, 4 `0x4C6`/`0x4CA`, 5 `0x4C7`/`0x4CB`, 6 and 7 `0x4C5`/`0x4C9`, 8 `0x4D0`/`0x4D4`, 9 `0x4CF`/`0x4D3`, any other `0x4BD`/`0x4C1`.
  It runs at every draw, and a type other than the one remembered (`0x17EA060`) changes the bookmark and builds the book again, so a change of class or of the Vampiress's form shows at once.

### The hero's book (type 1, `0x6B0E00`)

- Chapter 0, the class (made only for a hero's type, `0x426650`: the type's `Creature.pak` byte `+4` equal to 1): one non-list page, its text the class's description and its item the class's picture, by the type: 1 the text 1061 and the image `0x5C7`, 2 1060 and `0x5C8`, 3 1064 and `0x5CB`, 4 1062 and `0x5C9`, 5 1063 and `0x5CA`, 8 1067 and `0x5CE`, 9 1066 and `0x5CD`, any other (the Vampiress's 6 and 7) 1065 and `0x5CC` (`gui_qbp_*`).
  The item has neither title nor lines: the right page shows the picture, 256 × 256 from x 556 (centred on 684), from the right page's text top.
- Chapter 1, the skills: for each of the stats' eight skill slots in order (`0x5823C0`, the byte `stats + 0x24 + i`), a skill `v` not 0 makes a row titled with the text 9399 + `v` (its name) and an item of that title whose line is the text 9449 + `v` (its long description; the statistics window takes 9499 + `v`, the short one, docs/re/character.md); no picture, no level.
- Chapters 2 and 3, the combat arts and the magic: two passes over the stats' arts list (docs/re/arts.md), first the weapon arts (kind 2, `0x581CE0`), then the spells (kind 1, `0x581C30`), each in the list's order.
  The weapon arts' pass leaves out the Vampiress's night arts (`0x4D1CC0`: 1023, 1025, 1027, 1029, 1038 to 1041), so each pair of hers shows once, by its day art.
  The chapter (`0x4D2AE0`): the magic for the spells but 51 and 55 to 59 and for the weapon art 1019; the combat arts for the others.
  An art's item: its record's name as the title, its long description as the line, its first sprite (a 64 × 64 `gui_spell*` icon) as the picture; its level is not shown.
  The icon is drawn on the text's first page at its own size, centred on x 684, its top at the text's top, the text 8 below it (`0x6B23BF…0x6B2480`).
- Chapter 4, the main quest's items: the inventory entry's list of pairs `{item type, key}` (`cInventoryEntry::addMainQuestItem`, `0x5D8280`; `opensacred_rules::quests::QuestItems`), a row and an item each in the list's order: the title the key, else, where the key is 0 or −1, the type's name with its description as the line; the item shows its type turning in the circle.
  So an entry with a key shows its key alone, with no text.
- The circle (`0x6B1FF0`), on the item's first text page: `GUI_BOOK_CIRCLE.TGA` stretched to 290 × 290, white, from (556, 246), whatever the title; the type's model centred at (701, 391), scaled 2.5 at the depth −100, rocking `0.2·sin(1.03541·t)` radians about x and `0.2·sin(0.9142·t)` about z, `t` the seconds the book has been drawn; the text from `32·h + 16` below the text's top, `h` the item's height in the backpack's cells (docs/re/inventory.md).
  The model's draw (read 2026-11-23 for #3547, **confirmed** by code, `0x6B2586` to `0x6B2632` and the item draw `0x40E9C0`; not seen):
  - It is the backpack's own orthographic item draw (`0x40E9C0`, docs/re/inventory.md, "Drawing it"), called with the type, the point (the window's `+145`, `+195`), the scale 2.5 where the grid passes 1.0, the value −100 where the grid passes 0, the flags 0 where the grid passes the item's `+0xB0` bits 20 to 23, and the matrix `R = Rx(0.2·sin(1.03541·t))·Rz(0.2·sin(0.9142·t))` (`0x653910`, `0x653790`, `0x653B30`), where the grid passes none.
  - The pose: with the flags' bit 1 clear the draw builds the type's inventory transform at the scale given (`0x434270(type, M, 2.5)`: its `Weapon.pak` floats' scale times 2.5, its three turns and its offsets, as in the backpack), then multiplies it by `R` (`M·R`, row vectors, `0x653B30`), so that the item rocks about the world's x and z after its pose; the flags' bit 2 (a mirror) is clear; and a shield (`0x427CD0`) takes the value 30 in place of the one passed, the book's −100 as the grid's 0, where the flags' bits 1 and 4 are clear (`0x40EC06`).
  - The projection and the units: orthographic over the 1,024 × 768 of the screen in its pixels (`0x642570(−512, 512, −384, 384, −400, 2000)`), the eye at `(512 − x, v, y − 384)` looking along −y (the direction `(0, −10, 0)` normalised, `0x404AA0`), so that the model's origin is drawn at the screen's point `(x, y)`, its x to the right and its z up (the up vector `(0, 0, 1)`; the signs not followed through the look's cross products), one unit of the model `2.5·s` pixels for its type's scale `s`; `v` is the value above, −100 here, the eye's place along its axis, which the orthographic projection draws at the same size: it moves only the model's depth within the near −400 and the far 2,000 (**hypothesis** as to its purpose: room before the near plane for the model 2.5 times larger).
  - The light: none; the draw turns the lighting on (`0x643430(0x20, …)`) only for flags other than 4 that are not 0, so that the book's model, of the flags 0, is drawn unlit, as the backpack's of no shine, with no second texture (the shine's, by the flags' bits 20 to 23, `0x426320`); the depth's test and write, the culling, the blending by the alpha (`SRCALPHA`, `INVSRCALPHA`) and the linear filter are on; at a detail other than 0 a type of the effects' flag goes by `0x40F270` instead (docs/re/drawing.md), as in the backpack.
  - `t` grows by the frame's time after each draw (`0x6413E0`'s `+0xC`, at `0x6B2652`), so it stands while the book is not drawn.
  - `opensacred-game` draws it so since #3547 (`quest_book::circle_model`, `rock`): the type's model in a picture of its own over the screen, as the backpack's items (`backpack::spawn_picture`, `backpack::posed`), its pose at 2.5, its origin at (701, 391), unlit (`backpack::Matte`), rocking about the screen's x and its up axis after its pose by the seconds it has been drawn, of the real time, standing while no circle shows.

### The information's book (type 2, `0x6B04B0`)

- The books and scrolls the hero has read (the inventory entry's list at `+0xC`, each once in the order read; `pickup::Pack::books`, #1358), each an item of its `items.pak` record (`0x43AEB0`): the text `+0x1C` as the title, `+0x1C` + 1 as the line, the byte `+0x62` as the chapter; a type whose `+0x1C` is 0 is logged and passed over.
- Then, chapter by chapter, a row per item; no picture: the right page shows the title and the text.
- On the data (`tables/items.ron` with the client's changes of docs/02 §6.1): 139 types of the category 16 have the book `+0x61` 2 and a title; by `+0x62`: 0 62 types from 4100 to 4639, 1 13 from 4125 to 4137, 2 13 from 4150 to 4162, 3 8 from 4250 to 4257, 4 43 from 4175 to 4243.
- The diary passes a UI event (`0x10`, message `0x20`) to the book `items.pak +0x61` of a type (`0x6AED40`), which builds the book again; the build starts again from the hero's list, so it matters only where the list holds the type; its sender was not found.

### The statistics' book (type 3, `cUI_StatsTab`)

- No pages, bookmarks or arrows: each draw (`0x6B2B10`, under and over the parchment) makes a `cUI_StatsTab` (`0x6DDB00`, `UI_TAB_STATS`) on the stack, calls its four panels with places of the screen and destroys it; every label and value takes its colour `0xFF000033`.
- The player's statistics object (`0x425290`): 768 records of 16 bytes by creature type (`0x424250`: the type, its kills, the highest level killed, the two hit chances at that kill); `+0x3000` the ten types of the most kills (`0x424560`, the earlier type first among equals); `+0x30A4` the 16 latest kills, records of `0x24` bytes, the newest last (the type, its name's text, the two hit chances, its level, its health, damage, resistances' sum, attack and defence); `+0x32E4` a record per level from 1 and `+0x56E8` their count, kept at each level-up (`0x424CA0`: the gold, each damage kind's value, each resistance times its factor); `+0x56EC` the sectors visited (as a sector first takes its flag `0x1000`), `+0x56F0` the kills, `+0x56F4` the resurrections, `+0x56F8…+0x5700` the hours, minutes and milliseconds played.
- **The survival counter** `+0x570C` (read 2026-11-25 for #3749, **confirmed** by code and **seen** where marked): the milliseconds that the local hero has stood in its fight stance since its last death, which the survival bonus reads in whole minutes (below); no line of the book shows it, and the statistics window shows its curve (docs/re/character.md, "The window").
  - Its tick (`0x424A60`, called by the render thread `cEngine_renderThreadRun` at `0x60E692` once a frame): while the game's time runs (the first byte of the timer `0x6413E0` clear) and the local hero exists, the time played's milliseconds (`+0x5700`, carried into the minutes `+0x56FC` and the hours `+0x56F8`) take the frame's time times 1,000, and the counter takes the same where the hero's `+0x200` has the bit `0x8000`, its fight stance, which its look sets while a hostile is within 350, or 1,000 with a bow (docs/re/motions.md, "The fight stance"); the counter stays below 2,147,483,646.
    Each frame's sum is truncated to a whole millisecond, so that both lose the fraction of a millisecond of every frame, about 4 % at 60 frames a second; the frame's time is the real time between two of the render thread's frames (`0xCD7A58`), 1/24 s where `0x66EAF0` holds (not read), so that the time of a save or of a load leaks into the first frame after it.
  - Its clear (`0x4246D0`): the counter 0 and the resurrections `+0x56F4` one more, which nothing else raises; its twelve calls (a scan of the code, two more than Ghidra's references) are the death of a player's hero, an id up to 16, in the creatures' events' handler (`0x5335F1`, the block that moves `MULTI_START_WP`, docs/re/arts.md, "The effects"), the end of the drowning's effect 55 that bursts the local hero (`0x588813` in `0x586D80`, after `0x4E8DA0(1, 2)`), and ten copies of the test of cheats (`Cheater detected`: `0x465B93`, `0x538FA8`, `0x56518F`, `0x57E62D`, `0x57FE02`, `0x580192`, `0x5804C2`, `0x586195`, `0x61A0A3` in the save, `0x61CDD1`), each of which kills the hero it finds changed (docs/re/saves.md, "The file").
    So the counter starts again at the hero's death, and at no rise, load, teleport or level; the stats manager's making clears it with the rest of the object (`0x423E60`).
  - A save's header keeps its whole seconds at `+0x44` (`0x424650` at `0x61A5E1`), and the load sets the counter to them times 1,000 (`0x424670` at `0x61BB3D`); a hero's file keeps them so (`0x605280` in its save, `0x6066E8` in its load `0x6058A0`).
  - Its readers, each of its whole seconds (`0x424650`): the creatures' levels (`sdk_CREATENPC` at `0x484635`, `FillSector` at `0x47D277`, the bosses' and champions' first fight at `0x5510E2`; docs/re/spawning.md, "`CreateNPC`'s level"), a kill's drop bonus (`0x5345B6`, docs/re/items.md, "How many drops, and the bonus"), the item maker of the contents (`0x5D5AF8`, docs/re/containers.md), the hero's damages (`0x56A53A`, `0x56C1DA`, docs/re/character.md), the statistics window (`0x6A7D03`, `0x6A837C`, `0x6AA075`) and the creatures' update (`0x537471`, which drops it, docs/re/sets.md, M12).
  - **Seen** 2026-11-25 (the driver of #448, the Seraphim's new game, a probe of `0x424A60` reading `ECX` plus `0x570C`, `0x5700`, `0x56FC` and `0x56F4` once a second):
    - the time played rose 961 to 965 milliseconds a real second; the counter stood at 0 while no hostile came near, rose as fast with a goblin beside her, stood through the driver's pause (no tick in 4.4 s) and again once she was teleported 50 cells from the goblin, while the time played went on;
    - at her death by two goblins of the level 15, in the frame of the death's `changeDefPos(MULTI_START_WP, …)` line, the counter went from 34,106 to 0 and the resurrections from 0 to 1;
    - a quick save at 19,846 and F8 gave 19,000, the time played the save's 46,088, and the next probe 0.37 s later 21,239, the load's own time in its first frame;
    - a save through the menu at about 340,100, loaded from the main menu of a new run of the game, gave 340,525 at the first probe, with the time played at 49,700 against the save's 49,175: 340,000 and the 525 ms played since, foes beside her.
- `0x6DDCC0`, the latest foes: the title `UI_STATS_GEGNER_TITLE` in font 6 red at (550, 167); the heads `LOG_1070`, `LOG_1073`, `LOG_1078_2HIT`, `LOG_1076`, `LOG_1077` in font 1 centred in 80 × 16 at y 191, x 575, 630, 690, 740, 790; ten rows from the newest at y 210 + 24k, stopping at the first empty one: the level, the health, the two chances `%2d/%2d`, the damage and the resistances (the flags 4, **hypothesis**: right-aligned), the type's portrait in (550, y − 2) 24 × 24, its name's tooltip at (598, y + 6) while the pointer is on the portrait.
  The tooltip (read 2026-11-23 for #3109, **confirmed** by code, and **seen** the same day, below): while the pointer stands within x 550 to 574 and y − 2 to y + 14, the edges in, so over the portrait's upper 16 rows alone (`0x6DE1F4`–`0x6DE21A`), the panel shows the manager's tooltip by a text's key (`0x75B430`, the key's twin of `0x75B390`, which takes the text of the key by `0x6E6FB0`): the record's name key (`+4`, the type's name), at (598, y + 6) of the row, the style `0x1B` and shown at once.
  The style `0x1B` (docs/re/interface.md, "The tooltip (`cUI_Tooltip`)"): its bit 1, the tooltip lasting the frame it was shown in, so that it stands while each draw of the book shows it again and goes at the first draw that does not; its bit 2, `(598, y + 6)` its top left; its bit 8, kept 16 pixels within the screen; its bit `0x10`, the text's draw given the font's flag 1 (what it changes is not read); the interface's font 1 (the style without `0x8000`; with it `0x75B430` would take the font 3), white, on the tooltips' black panel and border.
  The most frequent foes' panel (`0x6DE2B0`) shows no tooltip.
  **Seen** 2026-11-23 in the running original (#3525, the driver of #448, the Seraphim's start, two goblin warriors of the level 3 spawned and killed by the driver, the book's statistics tab, the pointer moved a pixel at a time): the goblin warrior's name in white on the tooltips' dark panel with its border, the panel from about (585, 212) to (738, 242) for the first row (y 210), 13 pixels left of and 4 above the corner (598, 216) that the code gives (**hypothesis**: the window's border drawn out about its rectangle, not measured further), while the pointer stood within x 550 to 574 and y 208 to 224, the edges in (none at x 549 or 575, at y 207 or 225); the second row's portrait showed its own the same way, and the most frequent foes' portrait none.
- `0x6DE2B0`, the most frequent foes: the title `UI_STATS_GEGNER2_TITLE` at (550, 452); the heads `UI_STATS_MAX_LEVEL`, `LOG_1078_2HIT` and `UI_STATS_COUNT` centred in 50 × 16 at y 476, x 600, 690, 780; the first four of the ten at y 500 + 24k: the highest level killed, the chances at that kill, the kills and the portrait.
- `0x6DE710`, the game's history: the title `UI_STATS_HISTORY_TITLE` at (174, 318); white axes, x 180 from y 350 to 580 and y 570 from x 170 to 430; for the levels' records and a last live point, `w = ⌊250/(L + 1)⌋` apart, three polylines from y 570 (each segment from the point before, so the first is flat): the gold in `0xFFFFFF00` at `200/max`, the four damages' sum in `0xFFFF0000` at `200/max`, the four resistances' sum in `0xFF0000FF` at `220/max`; the legend `UI_STATS_GRAPH_BLUELINE`, `_REDLINE` and `_GOLDLINE` in font 7, `0xFFFFFF80`, at x 174, three lines up from 560; each line's highest in its colour at x 400 (y 350, 370, 390) and the levels' count in white at (400, 577).
- The record of a kill (`0x424250`, read 2026-10-31 for #1612) is made where the killer is the local player's hero or its master is (`0x5348BA`): the foe's `A` and `D` as their getters give them (`0x56AA10`, `0x56AA90`: times `ProzAW` for a creature of an id above 16, 0.65 on bronze; times `0xAD4D60` and `0xAD4D58` for its `+0x1F4` bit `0x80000`, or `0xAD4D64` and `0xAD4D5C` for its stats' bit `0x400`, by the balance's offsets `AWFakBoss` and `VWFakBoss`, `AWFakChamp` and `VWFakChamp`, docs/re/fight.md, "Bosses and champions"), cut to words, its level on bronze's scale (`0x5522D0`), its most health, its damage (`0x564800`) and resistances' sum (`0x553A00`), the kinds' shares, and the hero's two hit chances by `0x43EEE0` with 1.0 and 0.5 (`A/(A + D)`), times 100, at most 100.
- The latest foes' chances are made again with the hero as it is at each draw (`0x424700`); a type's count keeps those of its first kill at its highest level (**seen**: 66/32 among the latest, 62/36 among the most frequent, the hero risen two levels since).
- The game's history (`0x6DE710`, read 2026-10-31 for #1613): as the hero rises (`0x57E160`), a record for each level passed from the old one up to the new one less 1 (`0x424CA0`): the gold, each damage kind's mean `⌊(round(least·(m + b)) + round(most·(m + b)))/2⌋` (halved where the class of what the hero holds is 8 or 0 with an item in its right hand, docs/re/character.md, "The window") and each resistance times its multiplier, cut; `0x424870` gives the level `i + 1`'s for `i` below 256 and the values now above.
  The title `UI_STATS_HISTORY_TITLE` at (174, 318); white axes from (180, 350) to (180, 580) and from (170, 570) to (430, 570); for each point `i` from 0 to the highest level kept `L` (the records of the levels 1 to `L`, then the values now) a segment from the point `max(i − 1, 0)` at x `180 + i·s` to it at `180 + (i + 1)·s`, `s = ⌊250/(L + 1)⌋`: the gold in yellow `570 − 200·v/most`, the damages' sum in red `570 − ⌊200·v/most⌋` and the resistances' in blue `570 − ⌊220·v/most⌋`, each where its most is not 0; the maxima at x 400 (blue y 350, red 370, gold 390) and `L` in white at 577 in the system font (`0x182EE2C`, below); the legend `UI_STATS_GRAPH_BLUELINE`, `_REDLINE`, `_GOLDLINE`, each coloured by its text's tag, right in 250 from x 174 in the font 7, 13 apart from y 521.
- The system font (`0x182EE2C`, read 2026-11-22 for #3101, **confirmed** by code and by the data): the font manager's entry 0 (`0x8152D0`), a `cFsFont` (`0x648CD0`) of `FONT_SYSTEM.TGA` (`texture.pak`'s texture 19, 256 × 256), a bitmap font of the 256 characters of the code page 437 in cells of 8 × 10, 32 to a row, white with their coverage in the alpha.
  **Seen** 2026-11-22 for #3418 (the Seraphim raised to level 4 by the driver): the graph's three maxima at x 400 and the count of levels kept, 3, at (400, 577) in the bitmap's glyphs 8 pixels apart, two digits of the red maximum 8 pixels from each other, square-cut and unsmoothed.
  Each character takes its byte's cell, its quad 8 × 10 at the pen, the pen 8 further; a line's break goes 10 lower (its virtual `+0x28`, `0x64B0B0`); the colour codes are the other fonts'.
- `0x6DEE60`, the statistics: the title `UI_STATS_STATS_TITLE` at (170, 168); lines 24 apart from y 192, 250 × 24 at x 170 in font 1, the label left and its value right: `UI_STATS_CHARACTER` and the class's name (while the local hero exists, else the others move up), `UI_STATS_PLAYTIME` and `%d:%.2d:%.2d`, `UI_STATS_WELT` and `%.2f` of 100 times the sectors visited over the world's (the object manager's `+0x8284`) with `%` at x 420, `UI_STATS_VICTORY` and the kills, `UI_STATS_RESPAWN` and the resurrections.

## Open questions

- What clears an entry's `+0xC` bit 0 within a game, if anything.
- Where an ornament falls where a page break and a main quest after another kind meet.
- What plays `UI_QUESTBOOK_CLOSE02`, `TURN02` and `WRITING` (`WRITING`: an item of a quest picked up, docs/re/sound.md).
- The sender of the UI message `0x20` that adds an information item to a shown book.
- The `CreateObj` handler's condition for `addMainQuestItem` (`0x4868F6`), and when its entries' keys are 0.
- The exact terms of the levels' damage values; whether the world's sectors' count (`+0x8284`) counts the empty slots of the sectors' array (**hypothesis**: it is the sectors the world loads, 6,050, by which three sectors visited make the original's 0.05 %).

## In `opensacred-game`

Since #1560 `opensacred_rules::quests` keeps the log's entries (`Entry`: the title, the lines, the chapter, how it stands, whether entered since the game began) as `QuestBook` and `SetQuestInfo` write them, and `Quests::track` the side quest tracked; `quest_book` shows the book, its tabs, the quests' book's bookmarks, pages, rows, texts, arrows and counts as above, and `minimap` its picture's ground.
Since #1745 an entry's item shows every line it keeps, ten at most, and a side quest of 9000 to 9499, which the inner pin cannot follow, is neither tracked by a click nor shown as tracked (`quests::pinned`; docs/decisions/interface.md, "Quest book shows every kept line", "Quest book tracks what the pin follows"); a chapter's page of its own text shows its own picture, not the right page's quirk of an item by its page number.
Bevy lays the texts out, in the fonts at about the original's widths (AntiquaSSK at 19.5, Carolingia at 23 and 26 pixels), set 2 or 3 pixels lower than it would set them; a hidden node of each text measures its lines for the pages' breaks.
**Seen** in the game beside the original's frames: the Seraphim's first quest's row, its picture and its text's two pages within a few pixels; a side quest's click selecting and tracking it.
Since #1565 the hero's and the information's books are built from the hero's class, skills and arts (`arts::HeroArts`), its main quest's items (`QuestItems`) and the books it read (`pickup::Pack::books`), as above, each tab keeping its own chapter, page and selected row.
**Seen** in the game: the Seraphim's class page with its picture, its two skills, its art with its icon, and two books read in the information's first two chapters.
Since #1611 the statistics' book writes its statistics block (`quest_book::stats_block`): its title and five lines as `0x6DEE60` lays them, the class's name by the hero's type, the time played `h:mm:ss` of the game's time (`statistics::Tally`, standing while the game is paused, **hypothesis** for the render thread's), the sectors entered over the world's 6,050 sectors, the foes the hero killed (counted where its last foe is kept, and by the command `kill`) and the hero's rises.
**Seen** beside the original's frame after five kills and a level-up at 52,40: the same lines at the same places, `0.05%` in both.
Since #1612 it writes its latest and most frequent foes (`quest_book::foes_panel`, `statistics::Kills`): the kills of the hero and of its followers recorded where its last foe is kept, and by the command `kill`, their `A` and `D` times `ProzAW`; the ten latest, the newest first, their chances made again with the hero at each show; the four types killed most, the lower type first among equals; each row its portrait of 24 × 24 and its values at the right of their columns, the headers centred over them (**measured** on the original's frame).
**Seen** beside the original's frame after the same kills and level-up: the same rows, levels, health, damage, resistances and the hero's chances; the foes' own chances 2 apart where the creatures' attacks differ (the original's level-3 goblin `A 22 times 1.07`, the game's 23.0).
Since #1613 it draws the game's history (`quest_book::history_panel`, `statistics::History`): a record of the hero's gold, damages' means and resistances for each level it passes as it rises, and the graph of them and the values now as above, its axes and lines drawn into an image at the window's scale, its labels and legend in AntiquaSSK at 12 pixels.
**Seen** beside the original's frame at the level 3: the same axes, the red flat at 370, the blue at 413 rising to 350 in the last step, no gold line at 0 gold, the labels 7, 31, 0 and 2, and the legend in its colours.
The record's `A` and `D` take the creature's values as they stand, its effects' and its rank's factors in them (`opensacred_rules::fight::Rank::fight`, `statistics::Foe::of`), as the getters give the original's.
Since #3423 the graph's maxima and its count of levels are written in the system font (`0x182EE2C`, above; `quest_book`), its texture kept by the converter.
Since #3109 a latest foe's portrait shows its name's tooltip as above (`quest_book::foe_tip`).
Not followed: the halving of a hero's damage without a weapon in the history's records (#2935), the bar's button's pressed look (#2924); a compass's place is taken where its creature stands as the book is built (#3110).
