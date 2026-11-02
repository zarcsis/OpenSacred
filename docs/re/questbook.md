# The quest book (`UI_WND_QUESTBOOK`, `cUI_Diary`)

Read 2026-10-30 (#1560) in the GOG `Sacred.exe` 2.28 and measured on frames of the running original at 1024 × 768 (a Seraphim at the start; the frames stay in the scratchpad).
Status: **confirmed** by code unless marked otherwise; "measured" means the game's pieces at the places below were compared with a frame.
All places are pixels of the 1024 × 768 screen; "image N" is the entry N of the interface's images' table at `0x9ECEB0` (docs/02 §11, docs/re/interface.md).

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
- The fonts (the table `0xA1CDB4`): 1 AntiquaSSK 13, 5 Carolingia 16, 6 Carolingia 18; the titles and the counts in 6, a main quest's row in 5, a side quest's row and every text in 1.
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
- An ornament row (`GUI_main_08`, images `0xEE` and `0xEF` side by side, 25 high) is first on every page and comes before a main quest whose log entry follows one of another kind.
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
  Its point (`0x6B0C33`): the cell's middle on the iso plane (`0x622620`), less 2,248 across and plus 420 down (the floats at `0x89647C` and `0x896478`), truncated; the lens shows it at the screen's middle, so that the place stands about the picture's middle (**measured**).
- The text: its lines with a blank line between (`0x9E086C`), in `0xFF000066`, justified, a paragraph's last line set left (the flags `0x2500`, **seen**), poured page after page, each page under the title; the pages counted by pouring them unseen until one takes nothing.
- The page's count centred in (548, 576) 300 wide.
- The arrows, 32 × 32: the list's back and on at (156, 581) and (444, 581), the text's at (540, 581) and (828, 581); back `0x493` and on `0x495`, dark green, lit `0x492` and `0x494`, yellow, under the pointer (**seen**: the on arrow dark green with the pointer away); back shows above the first page, on below the last; their clicks are silent.

## The lens's ground (`0x6CCBD0`)

The world map's lens draws the minimap about a point of the iso plane; the quest book calls it for its picture with its fog, its icons, its creatures and its pin off (the arguments 0 and `0xFF`).

- The scale: a twelfth of the iso plane (`0x891070`), a cell 4 × 2 pixels; the point stands at the screen's middle (512, 384), whatever view the call clips to (**measured**: the book's picture matches the pictures laid so, to the pixel).
- The ground: the 25 sectors within three steps (|dx| + |dy| ≤ 3) of the point's sector, each its picture `MINIMAP<x><y>` (`texture.pak`, three digits each), 256 × 256, its corner at ((x − y) · 256 − 256, (x + y) · 128) of the lens's plane (**measured**); a sector without its picture black.
- Not read for the book: the corners faded where a sector lacks a neighbour.

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

### The information's book (type 2, `0x6B04B0`)

- The books and scrolls the hero has read (the inventory entry's list at `+0xC`, each once in the order read; `pickup::Pack::books`, #1358), each an item of its `items.pak` record (`0x43AEB0`): the text `+0x1C` as the title, `+0x1C` + 1 as the line, the byte `+0x62` as the chapter; a type whose `+0x1C` is 0 is logged and passed over.
- Then, chapter by chapter, a row per item; no picture: the right page shows the title and the text.
- On the data (`tables/items.ron` with the client's changes of docs/02 §6.1): 139 types of the category 16 have the book `+0x61` 2 and a title; by `+0x62`: 0 62 types from 4100 to 4639, 1 13 from 4125 to 4137, 2 13 from 4150 to 4162, 3 8 from 4250 to 4257, 4 43 from 4175 to 4243.
- The diary passes a UI event (`0x10`, message `0x20`) to the book `items.pak +0x61` of a type (`0x6AED40`), which builds the book again; the build starts again from the hero's list, so it matters only where the list holds the type; its sender was not found.

### The statistics' book (type 3, `cUI_StatsTab`)

- No pages, bookmarks or arrows: each draw (`0x6B2B10`, under and over the parchment) makes a `cUI_StatsTab` (`0x6DDB00`, `UI_TAB_STATS`) on the stack, calls its four panels with places of the screen and destroys it; every label and value takes its colour `0xFF000033`.
- The player's statistics object (`0x425290`): 768 records of 16 bytes by creature type (`0x424250`: the type, its kills, the highest level killed, the two hit chances at that kill); `+0x3000` the ten types of the most kills (`0x424560`, the earlier type first among equals); `+0x30A4` the 16 latest kills, records of `0x24` bytes, the newest last (the type, its name's text, the two hit chances, its level, its health, damage, resistances' sum, attack and defence); `+0x32E4` a record per level from 1 and `+0x56E8` their count, kept at each level-up (`0x424CA0`: the gold, each damage kind's value, each resistance times its factor); `+0x56EC` the sectors visited (as a sector first takes its flag `0x1000`), `+0x56F0` the kills, `+0x56F4` the resurrections, `+0x56F8…+0x5700` the hours, minutes and milliseconds played.
- `0x6DDCC0`, the latest foes: the title `UI_STATS_GEGNER_TITLE` in font 6 red at (550, 167); the heads `LOG_1070`, `LOG_1073`, `LOG_1078_2HIT`, `LOG_1076`, `LOG_1077` in font 1 centred in 80 × 16 at y 191, x 575, 630, 690, 740, 790; ten rows from the newest at y 210 + 24k, stopping at the first empty one: the level, the health, the two chances `%2d/%2d`, the damage and the resistances (the flags 4, **hypothesis**: right-aligned), the type's portrait in (550, y − 2) 24 × 24, its name's tooltip at (598, y + 6) while the pointer is on the portrait.
- `0x6DE2B0`, the most frequent foes: the title `UI_STATS_GEGNER2_TITLE` at (550, 452); the heads `UI_STATS_MAX_LEVEL`, `LOG_1078_2HIT` and `UI_STATS_COUNT` centred in 50 × 16 at y 476, x 600, 690, 780; the first four of the ten at y 500 + 24k: the highest level killed, the chances at that kill, the kills and the portrait.
- `0x6DE710`, the game's history: the title `UI_STATS_HISTORY_TITLE` at (174, 318); white axes, x 180 from y 350 to 580 and y 570 from x 170 to 430; for the levels' records and a last live point, `w = ⌊250/(L + 1)⌋` apart, three polylines from y 570 (each segment from the point before, so the first is flat): the gold in `0xFFFFFF00` at `200/max`, the four damages' sum in `0xFFFF0000` at `200/max`, the four resistances' sum in `0xFF0000FF` at `220/max`; the legend `UI_STATS_GRAPH_BLUELINE`, `_REDLINE` and `_GOLDLINE` in font 7, `0xFFFFFF80`, at x 174, three lines up from 560; each line's highest in its colour at x 400 (y 350, 370, 390) and the levels' count in white at (400, 577).
- The record of a kill (`0x424250`, read 2026-10-31 for #1612) is made where the killer is the local player's hero or its master is (`0x5348BA`): the foe's `A` and `D` as their getters give them (`0x56AA10`, `0x56AA90`: times `ProzAW` for a creature of an id above 16, 0.65 on bronze; times `0xAD4D60` and `0xAD4D58` for its `+0x1F4` bit `0x80000`, or `0xAD4D64` and `0xAD4D5C` for its stats' bit `0x400`), cut to words, its level on bronze's scale (`0x5522D0`), its most health, its damage (`0x564800`) and resistances' sum (`0x553A00`), the kinds' shares, and the hero's two hit chances by `0x43EEE0` with 1.0 and 0.5 (`A/(A + D)`), times 100, at most 100.
- The latest foes' chances are made again with the hero as it is at each draw (`0x424700`); a type's count keeps those of its first kill at its highest level (**seen**: 66/32 among the latest, 62/36 among the most frequent, the hero risen two levels since).
- The game's history (`0x6DE710`, read 2026-10-31 for #1613): as the hero rises (`0x57E160`), a record for each level passed from the old one up to the new one less 1 (`0x424CA0`): the gold, each damage kind's mean `⌊(round(least·(m + b)) + round(most·(m + b)))/2⌋` (halved again for a hero without a weapon, `0x54A470`) and each resistance times its multiplier, cut; `0x424870` gives the level `i + 1`'s for `i` below 256 and the values now above.
  The title `UI_STATS_HISTORY_TITLE` at (174, 318); white axes from (180, 350) to (180, 580) and from (170, 570) to (430, 570); for each point `i` from 0 to the highest level kept `L` (the records of the levels 1 to `L`, then the values now) a segment from the point `max(i − 1, 0)` at x `180 + i·s` to it at `180 + (i + 1)·s`, `s = ⌊250/(L + 1)⌋`: the gold in yellow `570 − 200·v/most`, the damages' sum in red `570 − ⌊200·v/most⌋` and the resistances' in blue `570 − ⌊220·v/most⌋`, each where its most is not 0; the maxima at x 400 (blue y 350, red 370, gold 390) and `L` in white at 577 in the font of `0x182EE2C`; the legend `UI_STATS_GRAPH_BLUELINE`, `_REDLINE`, `_GOLDLINE`, each coloured by its text's tag, right in 250 from x 174 in the font 7, 13 apart from y 521.
- `0x6DEE60`, the statistics: the title `UI_STATS_STATS_TITLE` at (170, 168); lines 24 apart from y 192, 250 × 24 at x 170 in font 1, the label left and its value right: `UI_STATS_CHARACTER` and the class's name (while the local hero exists, else the others move up), `UI_STATS_PLAYTIME` and `%d:%.2d:%.2d`, `UI_STATS_WELT` and `%.2f` of 100 times the sectors visited over the world's (the object manager's `+0x8284`) with `%` at x 420, `UI_STATS_VICTORY` and the kills, `UI_STATS_RESPAWN` and the resurrections.

## Open questions

- What clears an entry's `+0xC` bit 0 within a game, if anything.
- Where an ornament falls where a page break and a main quest after another kind meet.
- What plays `UI_QUESTBOOK_CLOSE02`, `TURN02` and `WRITING` (`WRITING`: an item of a quest picked up, docs/re/sound.md).
- The sender of the UI message `0x20` that adds an information item to a shown book.
- The `CreateObj` handler's condition for `addMainQuestItem` (`0x4868F6`), and when its entries' keys are 0.
- The font at `0x182EE2C`; the exact terms of the levels' damage values; whether the world's sectors' count (`+0x8284`) counts the empty slots of the sectors' array (**hypothesis**: it is the sectors the world loads, 6,050, by which three sectors visited make the original's 0.05 %).
- The fonts' table (`0xA1CDB4`, entries of `0x44` bytes, the face's name then its size) makes the font 7 AntiquaSSK 8, the statistics' legend's (read 2026-10-31, #1611).

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
Not followed: the effects' factors of `A` and `D` in the record, the portraits' tooltip of the name, the font of the graph's labels (`0x182EE2C`), and the halving of a hero's damage without a weapon in the history's records, the item's model turning in the circle (the circle is drawn, the model not), the bar's button's pressed look, the corners of the picture's ground; a compass's place is taken where its creature stands as the book is built.
