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

- The hero's (type 1, `0x6B0E00`): the class's page with its picture, the hero's skills, its combat arts in two chapters, the creatures shown turning in `GUI_BOOK_CIRCLE`; the information's (type 2, `0x6B04B0`): the information the hero collected, each in its chapter; the statistics' (type 3, `cUI_StatsTab`): four panels, no bookmarks and no list.
  Their research is #1565.

## Open questions

- What clears an entry's `+0xC` bit 0 within a game, if anything.
- Where an ornament falls where a page break and a main quest after another kind meet.
- What plays `UI_QUESTBOOK_CLOSE02`, `TURN02` and `WRITING` (`WRITING`: an item of a quest picked up, docs/re/sound.md).

## In `opensacred-game`

Since #1560 `opensacred_rules::quests` keeps the log's entries (`Entry`: the title, the lines, the chapter, how it stands, whether entered since the game began) as `QuestBook` and `SetQuestInfo` write them, and `Quests::track` the side quest tracked; `quest_book` shows the book, its tabs, the quests' book's bookmarks, pages, rows, texts, arrows and counts as above, and `minimap` its picture's ground.
Bevy lays the texts out, in the fonts at about the original's widths (AntiquaSSK at 19.5, Carolingia at 23 and 26 pixels), set 2 or 3 pixels lower than it would set them; a hidden node of each text measures its lines for the pages' breaks.
**Seen** in the game beside the original's frames: the Seraphim's first quest's row, its picture and its text's two pages within a few pixels; a side quest's click selecting and tracking it.
Not followed: the other three books (#1565), the bar's button's pressed look, the corners of the picture's ground; a compass's place is taken where its creature stands as the book is built.
