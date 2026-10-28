# The inventory: its windows, grid, slots, cursor's item, belt and tooltip

Read 2026-10-28 for #1251 by a static reading of the GOG `Sacred.exe` 2.28 (a subagent of the session through the Ghidra MCP server and byte scans of the PE), and seen the same day in the running original (the driver of #448, the Seraphim's new game).
Status: **confirmed** by code unless marked otherwise; places are pixels of the 1024 × 768 screen, as in docs/re/interface.md.
"Image N" is the entry N of the interface's images' table at `0x9ECEB0` (`tables/ui_images.ron`), its anchors relative as docs/re/interface.md has them.

## Summary

- The inventory's key and button open three windows together: the inventory itself (a backpack's grid of 20 × 8 cells under three tabs), the equipment beside it and the character's statistics above that.
- The items lie in the grid as their 3D models, each over its rectangle of cells, the sizes from `Weapon.pak` or by the category.
- The equipment's slots lie differently for each class; the hands are not among them: the weapons and the shields go into the taskbar's five weapon sets.
- A click takes an item onto the cursor, puts it down, swaps it, equips it or uses it; a click on the world drops it beside the hero.
- The belt has no slots of its own: it shows the first potion of each of five kinds found in the grid.
- The tooltip lists the item's name, its set, its price, its values and its requirements, those not met in red.

## Seen in the original

- The key `I` opened the three windows at once: the statistics at the top right (the name, the level, the skills, the health, the experience, the attack and defence, the weapon's and the arts' damage, the resistances and the gold), the equipment at the bottom right on its ornament, and the backpack's grid at the bottom left, translucent over the world, its cells' lines drawn over the scene, the tabs of the backpack, the combat arts and the combos above its left.
- The Seraphim's frames lay where the table of §4 puts them: the helmet at the top's middle, the shoulder at the top right, the armour at the left, the wings in the middle, the amulet and two rings at the right, the arms, the boots and the belt along the bottom.
- The template's potions lay in the grid's first column.
- A potion hovered showed its tooltip to its right: its name in white on the first line, then its price, a grey line of its effect and a grey hint of its use.
- A click of the driver on a potion did not take it onto the cursor (**hypothesis**: the agent's clicks do not reach the window's input); the clicks below are read from the code alone.

## 1. Three windows open together

- `cUI_Manager_createGameWindows` (`0x759AF0`) makes `UI_WND_INVENTORY` as a `cUI_Inventory3` (`0x6C5930`), `UI_WND_EQUIPMENT` as a `cUI_Equipment` (`0x6B6410`) and `UI_WND_STATS_CHAR` as a `cUI_StatisticsChar` (`0x6A5D70`); each constructor then sets its own rectangle.
- The inventory window lies at (0, 388), 640 × 256 (`+0x24`/`+0x28` its x and y, `+0x2C`/`+0x2E` its width and height); the equipment window at (656, 388), 256 × 256; the statistics window at (656, 0), 256 × 388.
- Showing the inventory (`0x6C5A50`, its virtual 9) presses `UI_TB_INVENTORY`, hides `UI_WND_QUESTBOOK`, and shows `UI_WND_STATS_CHAR` and `UI_WND_EQUIPMENT`.
- Hiding it releases `UI_TB_INVENTORY` and hides the merchant's, the blacksmith's, the purchase's, the master's, the chest's, the trade's, the statistics' and the equipment's windows; a cursor that holds an art, a spell or a combo (the cursor's kinds 2 to 4) is emptied, an item on it stays.
- The equipment window sends the engine's event `0xD` with (100, 40) as it shows and (0, −30) as it hides (`0x6B6690`; **hypothesis**: the camera shifted so that the hero stays in sight beside the windows).

## 2. The inventory window (`cUI_Inventory3`)

Its layout (`0x6C6490`), from the window's corner:

- A tab sheet (`cUI_TabSheet`, `0x729640`) of 640 × 256 with three pages: the items (`cUI_InventoryTab`, `0x6BFFE0`, `UI_TABPAGE_ITEMS`, the text 1155), the spells (`cUI_SpellTab`, `0x6C1720`, `UI_TABPAGE_SPELLS`, 1156) and the combos (`cUI_ComboTab`, `0x6BEFB0`, `UI_TABPAGE_COMBOS`, 1157), the items selected at first.
- The tabs' buttons (`0x729980`) above the window, at y −20 less the height of the font 2, each as wide as its text and 24, at least 128, on the image 108 (`GUI_main_01` (0, 126)–(94, 168)); their advance is a **hypothesis**.
- The UI event 5 of the sub-kind `0x11` with 1 to 3 selects the page 0 to 2 (`0x6C61C0`; **hypothesis**: keys).
- The frame of `UI_STATIC` children, all of `GUI_main_03` (read again for #1267): ten steps along, each a piece of the top edge at its step raised by its first anchor's y, so that it stands on the window's top, the first the image 5, the last 12, the others `rand()` / 32767 · 5 + 22; and a piece of the bottom edge at its step and 256 less its first anchor's y, the first 3, the last 13, the others · 5 + 15; each step as wide as its bottom piece; then nine pieces of the right edge from the height 4 down at 641 less their second anchor's x, each as high as it is, the first 32, the last 33, the others · 3 + 28.
- The sort's button (`cUI_Button2` at `+0x158`) at (−2, 254), 34 × 34, the image 73 (`GUI_main_02` (222, 124)) and 71 ((188, 124)) for its other state, its tooltip `UI_TOOLTIP_AUTOSORT`, drawn and clicked on the items' page alone.
- Its click runs `cInventoryEntry::autoSortItems` (`0x5DB6B0`) in the mode of the button's `+0x168` bit 4; in a game of the network a second click within 2 s waits 5 s.
- The sort takes every item out, groups them by category, sorts each group by its area (w · h), the largest first, and puts the groups back in the order of the categories 28, 30, 31, 8, 20, 9, 15, 14, 16, 5, 17, 18, 19, 6, 13, 21, 22, 23, 24, 25, 29, 33, 0, 10, 11, 12, 27, 1, 26, 7, 2, 3, 4, each by the fit's search in the mode given; when an item finds no place it puts the grid back as it was and tries `0x5DB280` (not read).

## 3. The backpack's grid (`cInventoryEntry`)

### The entry

- 32 entries of `0x1180` bytes at `0xAD4448` (`0x5DEBC0` makes them, `0x5DEC90` gives the entry of an index below 32); `+0x1160` is the entry's index, the creature's id for a hero.
- `+0x18`: up to 256 cells of 12 bytes: `+0` the item's id (on its top left cell alone), `+4` flags (bit 0 taken; the marks `0x20` and `0x40` below), `+5` and `+6` the item's width and height on its top left cell, the top left cell's column and row on its other cells (`0x5D9BE0`), `+8` the item's type, tested against the item at each draw.
- `+0xC18` ten records of 12 bytes (**hypothesis**: the five weapon sets' halves, docs/02 §9's five records of 24 bytes; `0x5D8700`, `0x5D8730`); `+0xC90` ten of 20 bytes (docs/02 §9's 200 bytes, not read); `+0xD5C` the category 14's list; `+0x115C` and `+0x115E` the width and the height in cells; `+0x1162` flags; `+0x1164` and `+0x1166` the grid's top left on the screen; `+0x116C` to `+0x1174` a list of sparkles of 20 bytes; `+0x117C` set by a click.
- `+0x1162` bit 3 is the items' page shown: the page's show (`0x6C00F0`) calls `0x5D8FE0`, which sets it as it shows and clears it (and the sparkles) as it hides; as it shows the page plays `(14, 2, 1)`, `UI_INV_OPEN01`; `0x5D9060` reads it (the **hypothesis** of docs/re/items.md, "What becomes of the item", **confirmed**).
- `+0x1162` bit 4 is set at each place (**hypothesis**: changed since saved; docs/02 §9 finds 16 in every saved backpack).

### Its size

- Every entry is made 20 × 8 (`0x5DF570`, `0x5D7A50`), a load takes 20 × 8 before it reads the saved size (`0x5DDBA4`), docs/02 §9 finds 20 × 8 in every template and save, and no code gives a hero another: every class has 20 columns by 8 rows of cells of 32 pixels, 160 cells.
- The other entries: 18 is 3 × 5 at (16, 64), 20 to 23 are 20 × 8 (`0x5DF0C0`); the trade's windows set their own (the merchant 16 × 9, a chest a column of 10, the trade 7 × 5, the blacksmith and the cube 3 × 5; not followed here).
- The items' page puts the hero's grid at (0, 392) at every draw (`0x6C1480`; the saves hold 392 too): the grid covers x 0 to 640 and y 392 to 648, 4 pixels under the window's top.
- A point lies in the grid where x0 < x < x0 + 32 · W and y0 < y < y0 + 32 · H, its cell the row · W + the column (`0x5D9D60`).

### An item's size in cells (`0x428D60`)

- The type 4007 (the Seraphim's hair of her template) is 2 × 2.
- Else the bytes `+0x1C` (the width) and `+0x1D` (the height) of the type's `Weapon.pak` record, through the items table's `+0x18`, where both are above 0 (docs/02 §6.2 has them as `u8 +0x1C (1..4)`, `+0x1D (1..5)`).
- Else by the category (`items.pak +0x2E`): 5 the weapons 1 × 3, but 1 × 4 for the types 1715 to 1717, 1736, 1763 and 1766; 6 the armour 2 × 3; 13 the shields 2 × 3; 16 the books 1 × 2; 17 the helmets, 18 the boots, 21 the shoulders, 22 the arms and 23 the greaves 2 × 2; 19 the belts and 24 the gloves 2 × 1; 7, 9, 14, 20 and any other 1 × 1.

### Finding, testing and placing

- An item fits at a cell (`0x5DACB0`) where its rectangle from the cell lies in the grid and covers no taken cell.
- The search for a fit (`0x5DABA0`): in the mode 0 column by column from the left, each from the top (the pick-up and the inventory's message `0x2C`); in the mode 1 cell by cell in the rows' order.
- The swap's test (`0x5DB0D0`): the rectangle in the grid covers no item (a free place) or exactly one (a swap, which it gives back); two or more refuse (`0x5DC100` gathers them).
- The place (`0x5D9F00`) refuses with the log `inventory slot %d wxh …` where the item does not fit; else it tells the item its container (its query `0xB` with `+0x1160`), takes it out of the world (`0x5FC8B0`) unless its `+0x14` bit 2 says it is out already, clears its position, writes the cells, and marks them `0x20` for an item of `+0x14` bit `0x800` (a quest's item, docs/re/netscript.md, "The log's quest items") or `0x40` for one that had bit `0x40000`, cleared then (**hypothesis**: new).
- For an entry below 18 that is the local hero's the place computes the hero's stats again (`0x5796A0`); in a game of the network it sends the event `0x13F`.
- The removal (`0x5DA630` by the item's id, `0x5DA3D0` by the cell) clears its cells.

### Drawing it (`cInventoryEntry_render`, `0x5DC430`)

- Each cell's ground is the image 165 (`GUI_main_04` (64, 192)–(95, 223), 32 × 32) at (x0 + 32 · c, y0 + 32 · r); grids of 1 × 1, 3 × 5 and 3 × 2 and two other modes draw one picture instead (171, 169, 172, 352, 353).
- Each cell is then tinted, at an alpha of `0x40`: an item's cell blue (`0x3D59AB`); under the held item a free cell where it fits green (`0x3D9140`), an item's cell where it would swap purple (`0x91219E`), and every cell red (`0xFF0000`) where it does not fit (all of them where it is larger than the grid); red on every cell of an item that the hero cannot use (`0x54ABE0`); `0xFFF000` on the cells marked `0x20` and `0xFFFF80` on those marked `0x40`; in the shops' entries 20 to 23, `0xFFFF00` on the items that cost more gold (`0x5BECF0`) than the hero has.
- Each item is its 3D model, not an icon: `0x40E9C0` draws the type's model under an orthographic projection of the screen, centred in its rectangle of w × h cells, at the scale 1.0, turned by the type's inventory's transform of its `Weapon.pak` record's floats (`0x434270`: `+0x00` a scale, `+0x04`, `+0x08` and `+0x0C` three turns, `+0x10` and `+0x18` offsets), a shield turned 30° more, a second texture chosen by the object's `+0xB0` bits 20 to 23 (`0x426320`; **hypothesis**: the quality's shine).
- The transform, in D3D's rows (`0x434270`): the scale, then `D3DXMatrixRotationX` of `+0x04`, `D3DXMatrixRotationY` of `+0x08` and the rotation about z of `−(+0x0C)` (`0x653790`), then `+0x10` added to the translation's x and `+0x18` to its z; a record whose scale is 0 leaves the model as it is.
  The turns are radians (the torch 5633: 1.65, −1.52, 0.25, 0; the potions: 2.5 and no turn).
- Seen in the grid (#1268), the turns about x and z run the other way in the models as `opensacred-convert` writes them (Z up taken to Y up by a turn): the original's frames of the large torch 5633, the Daemonia's arms 1218 and the staff 2666 match only through a mirror of the files' y about the transform (**hypothesis**: the game's frame of the models mirrors their files' y).
  The kite shield 1200 matches the original's frame without the 30°, so what that 30 changes is open.
- The sockets: up to four icons of 8 × 8 along the bottom of the item's rectangle from its left, 2 pixels over its bottom edge, the images 393 to 395 (`GUI_main_icons`) for an empty socket of the kinds 1 to 3 and 396 to 398 for a filled one (`0x5BE9C0` the count, `0x5BE9E0` the kind, `0x5BEAE0` filled).
- An item whose cell's type disagrees, or that another holds (`+0x30`), is logged and left out of the grid.

### The clicks on the items' page (`0x6C0150`)

- The cell is the one under the cursor less the held item's offset (§6); a click outside the grid does nothing.
- The left button with the cursor empty: the cell's item leaves the grid (`0x5DA3D0`) onto the cursor, with no sound.
- The left button with something other than an item on the cursor (an art, a spell): `(9, 2)`, `UI_INV_PUTFAILED01`.
- The left button with an item: where it fits, its top left goes to the cell, the cursor is emptied and `(9, 1, its category)` plays `UI_INV_PUT*`; else, over exactly one item, that item leaves the grid, the held one is placed and the other goes onto the cursor (a swap); else nothing, with no sound.
- The right button: an item whose name begins with `Use:` runs that function of the scripts (`0x4A71F0`, `0x46BF40`), as `Take:` does as it is picked up; else the page sends `UI_WND_EQUIPMENT` the UI event `0x2C` of the sub-kind 1 (equip it, §4); a type for which `0x43F710` holds opens `UI_WND_CUBE` (**hypothesis**: the cube).
- Where neither took it: a bridle (29) goes onto the slot 0 of the hero's horse, the old bridle back to the cell; the weapons (5) and the shields (13) go to the taskbar as its message 5 (into the selected weapon set, §5); any other category sends the hero the object's event `0x10` with the item (its use; **hypothesis** that the item's handler removes it); an item taken so (equipped, sent to a set, or used by a script) leaves the grid.
- Shift and the left button with a shop open: to the merchant (`0x75A9B0`) the item is sold, leaving the grid, the merchant sent the UI event `0x2E` 7 and its price (`0x5DEE60`) added to the gold (`0x57FD80`); with `0x75AA10`, or `0x75AA30` and `0x6EBC40`, it moves to the reference `0x11` or `0x1A` (`0x5DECC0`; not read).
- Shift and the left button with no shop: where the hero answers its query `0x51`, the item is destroyed for its value (its query 3) in gold; else the engine's event `0x14` is sent with it (**hypothesis**: dropped).
- Shift and the right button on a potion (category 9): of the followers in the hero's list `+0x39C` below 99 % of their health, the one of the lowest most health drinks it.
- The UI event `0x2C` of the sub-kind 2 (from the equipment window) puts the item at the first fit by columns; of the sub-kind 3 (a swap from it) it puts the old item into the cell that the new one left.
- With the hero dead for good (`0x182EE50`) the clicks go to `0x4246F0` (not read).

## 4. The equipment window (`cUI_Equipment`)

- Its ground is the image 376, `GUI_main_ornament.TGA` 256 × 256 at (0, 0), drawn at an alpha of `0x9F` (`0x6BAC80`).
- Its frame (`0x6BAC80`, read for #1267): eight pieces of the right edge from the height 10 down at 256 less their second anchor's x, each as high as it is, the image 34 where `rand()` / 32767 · 100 is above 40, else · 2 + 34; and four steps from −1 along, a piece of the top edge at the step raised by its first anchor's y (50, 7, 7 and 51, the last 3 further along) and one of the bottom edge at 256 less its first anchor's y (60, two of · 5 + 15 and 61, the last 3 back), each step as wide as its bottom piece.
- At each draw it makes its slots again where the hero's type has changed (`0x6B7700`): the type 1 by `0x6B97D0`, 2 by `0x6B8290`, 3 and 5 by `0x6B88E0`, 4, 6 and 7 by `0x6B9050`, 8 by `0x6B9DA0`, 9 by `0x6BA510`.
- Each slot is a `UI_STATIC` at `+0x15C + 4 · i`, `i` the creature's slot of equipment (docs/re/equipment.md), with the category it takes at `+0x326 + 20 · i`; the slots 12 and 13 (the hands) and 18 (the horse) have none.
- Their frames, of `GUI_main_04` unless named: the helmet 146 (64 × 64), the armour 147 (64 × 96), the boots 150, the arms 152, the shoulder 162 and the greaves 163 (64 × 64 each), the belt 148 (64 × 64, the Seraphim's and the Gladiator's) or 149 (64 × 32), the gloves 151 (64 × 32), the amulets 153 and 154 and the rings 155 to 158 (32 × 32), the wings 164 (96 × 96), and the Dwarf's cannon 1611 (`GUI_main_17`, 64 × 96).

The slots' places in the window, (x, y) of their frames' corners:

| Slot (category) | Seraphim | Gladiator | Mage, Wood Elf | Dark Elf, Vampiress, Daemon | Dwarf |
|---|---|---|---|---|---|
| 0 helmet (17) | 96, 6 | 96, 6 | 96, 6 | 96, 6 | 96, 6 |
| 1 armour (6) | 16, 80 | 96, 80 | 96, 80 | 96, 80 | 96, 80 |
| 2 belt (19) | 176, 186 | 176, 186 | 176, 218 | 176, 218 | 96, 186 |
| 3 boots (18) | 96, 186 | 16, 186 | 16, 186 | 16, 186 | 16, 186 |
| 4 gloves (24) | – | 16, 146 | 16, 146 | 16, 146 | 16, 146 |
| 5 arms (22) | 16, 186 | 16, 76 | 16, 76 | 16, 76 | – |
| 6, 7 amulets (20) | 208, 74 | 192, 82 | 176, 82; 208, 82 | 192, 82 | 176, 138; 208, 138 |
| 8–11 rings (8) | 208, 112; 208, 148 | 176, 120; 208, 120 | 176, 120; 208, 120; 176, 158; 208, 158 | as the Mage's | 176, 172; 208, 172; 176, 206; 208, 206 |
| 14 cannon (33) | – | – | – | – | 176, 6 |
| 15 shoulder (21) | 176, 6 | 176, 6 | – | 176, 6 | 16, 76 |
| 16 greaves (23) | – | 96, 186 | 96, 186 | 96, 186 | – |
| 17 wings (25) | 96, 80 | – | – | – | – |

- The slot's test (`0x43EFC0`): the slot 0 takes the category 17, 1 takes 6, 2 takes 19, 3 takes 18, 4 takes 24, 5 takes 22, 6 and 7 take 20, 8 to 11 take 8, 14 takes 33, 15 takes 21, 16 takes 23, 17 takes 25 (for 1, 5, 14, 16 and 17 also `0x4263A0`, not read); the slot 12 takes a shield, the types of `0x427E30` and `0x427E80` (not read) and the weapons of the kinds 12 and 14, and is the torches' (5632, 5633) only slot; the slot 13 takes any other weapon.
- A slot glows green (`0xAF00FF00`) while the cursor holds an item of its category that the hero may use, red (`0xAFFF0000`) for one it may not, and is otherwise white (`0xAFFFFFFF`).
  **Seen** (#1288): with a healing potion on the cursor every slot glows red, so an item that a slot does not take makes it red too, and the white is only that of a cursor that holds nothing.
- A worn item is its 3D model centred in its slot (`0x6B75A0` keeps a slot's record), its sockets' icons as in the grid.
- A slot hovered shows its item's description in the manager's tooltip (`0x75B1E0`) at its right edge and 8, at its top; an empty slot its own text (the style `0x209`).
- While the hero wears a set's bonus (`+0x1F4` bit 31) the window shows the image 167 (168 hovered; `GUI_main_04`, 45 × 45) at the screen's (678, 400), whose hover lists the set's bonuses (`0x5711F0`) at (512, 256).
- The Dwarf of the skill 33 has a button (the images 1607 and 1608, or 1609 and 1610) that opens a window like the blacksmith's (not read).

### Who may use an item (`0x54ABE0`, `0x5BF810`)

- First its classes (the item's virtual `0x38`, `0x5BF810`): the record's bits (`+0x04` bits 0 to 5, 22 and 23; the item's `+0x11C`) of the hero's types 1 to 9, bit 0 the Seraphim, 1 the Gladiator, 2 the Mage, 3 the Dark Elf, 4 the Wood Elf, 5 the Vampiress (the types 6 and 7), 22 the Dwarf, 23 the Daemon; no bit, or all of them, any class; bit 6 none; a class's variant (`0x43CA20`) writes the bits of the taker's class.
  **Confirmed** on the data (#1303): each class's own item of the templates carries its class's bit alone in its record's `+0x04` (the Seraphim's 4007 bit 0, the Gladiator's 4054 bit 1, the Mage's 3219 and 7904 bit 2, the Dark Elf's 7903 bit 3, the Vampiress's 4028 bit 5, the Dwarf's 3018 bit 22), and the other templates' weapons all seven bits.
- The weapons' kinds (`Weapon.pak +0x1E`): the Dwarf may not use the kinds 4, 7 and 12, the Daemon not 7, and the kind 3 is the Gladiator's and the Dwarf's alone; the Dwarf may not use the bridles (29), and the types 1800 to 1804 are for no hero.
- Then the requirements of the item's record (`0x5BEB30`): its byte `+0x14` at most the hero's level (`+0x3FE`); `+0x15` the strength (the stats' `+0x10`, the creature's `+0x3B8`), `+0x16` the dexterity (`+0x3BC`), `+0x17` the charisma (`+0x3C2`) and `+0x18` the endurance (`+0x3BA`) at most the hero's; the skill `+0x1A` at least at the level `+0x1B` (`0x582450`); the tooltip names them by the keys `INVENTAR_MIN_*`.
- `+0x14` is the level an item asks of its holder and `+0x19` the item's own level, **confirmed** in the original (#1303): a sword 1712 made by the driver's `item` showed the item's level 30 and a least level of 22 in red in its tooltip (`⌊(30 + 9)·0.8 − 8.5⌋`, docs/re/items.md), and its cells in the grid stayed red for the Daemon up to her level 21 and turned blue at 22.

### The clicks on the equipment window (`0x6B6690`)

- Each needs the hero alive and acting (`0x549080`: not in the AI's state 9, not the action 6, its health above 0).
- The left button with the cursor empty: the slot's item is taken off (`0x556A30`) onto the cursor, with no sound.
- With an item the hero may not use: `(9, 6)`, `NOTMYITEM`, where the class refuses it, else `(9, 8)`, `NOTMYITEMLVL`; nothing moves.
- With an item it may use that the slot takes: the old item is taken off, the new one put on (`sdk_EQUIP`) with `(9, 5, its category)`, and the old one goes onto the cursor; an item that the slot does not take does nothing.
- The right button on a worn item: the inventory is asked (the event `0x2C` of the sub-kind 2) to put it at its first fit, and then it is taken off.
- An item sent from the grid (the event `0x2C` of the sub-kind 1) goes into the first slot 0 to 18 of the window that takes it and is empty, else the first that takes it, the old item back into the cell that the new one left (the sub-kind 3); the refusals are as above.

## 5. The weapon sets (the taskbar's left slots)

- The weapons and the shields are worn in five weapon sets of two halves (0 the left hand, 1 the right), kept in the inventory's entry (`0x5D8700` reads, `0x5D8730` writes; **confirmed** by code 2026-10-28): the set `s`'s half `h` is the entry of 12 bytes at the index `258 + 2s + h` (from `+0xC18`), its `+0` the item's id and its `+4` and `+8` the model that the taskbar draws in the slot, loaded as the item is put there by its type's model and texture (`0x426250`, `0x4262B0`); a half above 1 is taken as 1, and in a network game the write sends the event 299.
- The held item dropped on a set (`0x6E14C0`) tries the half under the cursor first (the slot's left half the half 0), then the other: success plays `(9, 3)` and selects the set; failure plays `(9, 4)` where the class refuses and `(9, 7)` where the requirements do.
- The rules (`0x6E09C0`): the hero must be able to use the item; a shield goes into the half 0 alone, and replaces a two-handed weapon; a two-handed weapon (`0x5CECD0`) takes the set alone, refused where the set holds two items, else swapped with the one; a one-handed weapon goes by its hands (`0x5CEC00`: 1 left, 2 right, 3 either), the half 0 needing a weapon of the hands 3 in the half 1 and the hero's skill 6, the two weapons' (`0x582450`, docs/re/fight.md); an item replaced goes onto the cursor or back into the grid.
- Read 2026-10-28 (**confirmed** by code): a weapon's hands (`0x5CEC00`) go by the weapon kind of its type's record, or its parent's (`Weapon.pak +0x1E`, docs/02 §6.2): 1 for the kinds 3, 7 (the bows) and 12 (the crossbows), 3 for 0, 1 (the swords and axes), 5 (the daggers), 6 and 10 (the dark elves' blades), and 2 for 2, 4 (the staves and polearms), any other kind and the fist weapons 4067 to 4072; a shield's are 1.
  A weapon is two-handed (`0x5CECD0`) of the kinds 2, 3, 4, 7 and 12 and the fist weapons 4067 to 4072.
  The rule answers 0 (refused), 1 (put into an empty place), 2 (put, the item it answers replaced) or 4 (put by trying the other half); it refuses everything while `0x5D8FD0` holds (not read), an item of no category 5 or 13, and an item the hero may not use (`0x54ABE0`).
  A shield, into the half 0 alone: beside a one-handed weapon or none it takes the half 0's place, the old shield replaced; beside a two-handed weapon in the half 1 it replaces that weapon where the half 0 is empty, else it is refused.
  A one-handed weapon into the half 1 needs the hands 2 or 3 and replaces a two-handed weapon of either half, else the half 1's item; into the half 0 it needs the hands 3, an item of the hands 3 in the half 1 and the hero's skill 6, and replaces a two-handed weapon of either half, else the half 0's item; so an empty set takes a weapon of the hands 3 into the half 1 alone, and the drop's second try puts it there.
  A two-handed weapon dropped on a half other than the one its hands name (the half 0 for the hands 1, the half 1 for the hands 2) is tried on the named half, the rule answering 4 where that takes it; on the named half it takes the set alone: into an empty set, or in place of its one item, refused where both halves hold one.
- A weapon put into the half 0 of an empty set moves to the half 1 unless its hands are 1; the half 1's weapon taken, the half 0's moves to the half 1.

## 6. The cursor's item

- `cMouse +0x68` holds a record of 20 bytes: `+0` the kind (1 an item, 2 to 4 others), `+2` the item's category, `+4` its id, `+8` its type, `+0x10` its picture (the items table's `+0x04`).
- `0x655580` sets it with the offsets (−32, −32); with no picture (`+0x04` 0, as for the weapon 1724 in `tables/items.ron`; **hypothesis** for every item) the cursor draws the item's 3D model at the cursor (`0x6546E0` with the flag 2, `0x6548A0`) rather than a quad of 64 × 64 at (−32, −32).
- For the model the offset is 8 · w for an even width and 16 · w − 16 for an odd one, the height likewise; the cell of a click or a highlight is the one under the cursor less that offset (`0x655810`, `0x655870`): the item's top left cell.
- **Seen** 2026-10-28 (#1269): a healing potion taken onto the cursor by a click on its cell is drawn centred on the cursor, and the system's arrow is not drawn; the cell it was taken from showed the green of a free cell under it while the driver's cursor (which moves `cMouse` alone) stood elsewhere, so the grid's highlight follows the interface's own cursor.
- An item picked up while the items' page shows and the cursor is empty goes onto the cursor (docs/re/items.md, "What becomes of the item", its branch 6, **confirmed**).
- A click on the world's ground with an item held calls `0x611E20`, `cEngine::dropItem` (`0x611620`, **confirmed**): the item drops beside the hero, not at the point; it looks from the hero's cell toward the point (one of 8 directions, the table at `0x95DCB0`) for a walkable neighbour (`0x636C10`), trying the next directions to each side in turn, up to 8; it enters the world there 60 over the ground and flies (`0x44DED0`) with the velocity (`rand() % 21 − 10`, `rand() % 21 − 10`, 50) and a spin of `rand() % 13 − 6` about each axis; in a game of the network the event `0x140` is sent.

## 7. The potions and the belt

- The belt has no slots of its own: its slot k shows the first potion of the belt's kind k in the grid, row by row (`cInventoryEntry::getPotionRef`, `0x5DA8C0`).
- The belt's kinds by type (`0x43F760`, the category 9): 1 (red) 3999, 5056 to 5058, 5140, 5145 and 5171; 2 (purple) 5137, 5142 and 5168; 3 (blue) 5059 to 5061, 5138, 5143 and 5169; 4 (green) 5139, 5144 and 5170; 5 (yellow) 5062 to 5064, 5141, 5146 and 5172.
- The kind 1 chooses: with `m` the share in per cent of the health missing, each of 5171, 5145, 5140 and 3999 has a threshold, a balance's value (`0xAD5680`, `0xAD5688`, `0xAD5684`) over the difficulty's divisor (`0xAD568C`), at most 100, and the potion of the smallest threshold above `m` is drunk.
- A click on a belt's slot (the engine's event `0x17`, `0x612FA0`): the left button drinks the potion (`0x54AA80`), which leaves the grid; the right button, for the kinds 1, 2 and 4, gives it to the follower of the lowest most health among those below 99 % of their health; a key calls it with the kind 6, the kind 1, or 5 with Shift (Space, as the slot's tooltip names it).
  Read 2026-10-28 for #1313: the right button's potion is the slot's choice by the hero's own share missing, as the left's; the followers weighed are the creatures of the hero's list (`+0x39C`, entries of 9 bytes, at most 100) whose health over their most is below 0.99, the lowest most health taken from 99,999 down, the first of equal ones; with none found nothing is drunk.
- A use (`0x54AA80`): a living hero sends a potion (9), a book (16) or a rune (28) the object's event `0x10`; a rune's handler (`cItemUpgrade`, `0x5CD100`, read 2026-10-28 for #1315) takes its art from `items.pak +0x6A` (`0x43AF10`), does nothing where that is 0 or the hero may not use the rune (`0x54ABE0`), looks the art up in the spells' table and then the weapon arts' (`0x4D1AF0`, `0x4D2310`), posts the event (7, 2) and reads nothing where neither holds it, and else teaches it or raises it (`0x580750`, `0x581D40`, `0x581F80`), the rune used up where that holds, the spells' page told (`UI_TABPAGE_SPELLS`); an item that then has `+0x14` bit `0x8000000` leaves the grid and is destroyed; the potion's handler (`0x5CAA60`) plays its sounds (docs/re/sound.md).
- The choice of a slot's potion (`0x5DA8C0`, read 2026-10-28 for #1311) walks the grid's cells row by row, and the first potion of the kind is the slot's; for the kind 1 a drink weighs the share missing `m`, 100 less the whole per cent of its health left, held to 0 to 100, against each healing potion's share, and takes the one whose share is the smallest above `m`, the first of equal shares, else the first of the kind; 5171 is weighed by the small potions' share, 5145 by the large ones' and 5140 and 3999 by the medium ones', the medium and the large crossed against what they heal; the other potions of the kind 1 (5056 to 5058) are taken only as the first of it; the belt's look passes no health, `m` 100, so that a slot shows the first potion of its kind.
- The potion's handler (`0x5CAA60`, read 2026-10-28 for #1311) plays one of its three drinking sounds at every use, whatever follows; the share of its drinker's health left is its health over its most.
  A healing potion (`0x5CA9E0`: 5056 and 5171 small, 5057 and 5145 medium, 5058, 5140 and 3999 large) gives its share, `HealingPotion`'s row of 120, 200 and 300 over `PotionDiv`'s value of the difficulty, 1 on bronze to 5 on niobium, at most 100, only where that health is below its most: it sends the drinker the creature event 5 with the share and is used up, and sends the same to an object whose `+0x1EC` is the drinker's id (**hypothesis**: its horse); at full health nothing is used up.
  The others, used up at once: the green potions the creature event 8 with 100, 50 or 25 (5139, 5144, 5170; **hypothesis**: the poison cured by that share), the yellow ones the event 10 with 100, 50 or 25 (5141, 5146, 5172), the purple ones the event 6 and the blue ones the event 7 (5137, 5142, 5168; 5138, 5143, 5169) with a value converted from a float (**hypothesis**: `BlackPotion…Dur` and `BluePotion…Dur`); the types 6081 and 6094 give the drinker the effect 31 of the value 10 for 400 to 1000 (`rand() % 601 + 400`; units not read).
  An item used up sends the taskbar its message `0x10` with the potion's belt kind (the slot's sparkle), leaves the grid (`0x5DEBC0`), and `0x5D8000` adds a sparkle at its cell to the list at `+0x1170` and plays `UI_INV_USEITEM01` (7006).
- **Seen** 2026-10-28 (#1311): the Seraphim's small healing potion drunk from the belt's first slot at 40 of 119 brought her health to 119 on bronze; a click at full health used none; the three potions of her start used up, the slot showed the empty flask with a sparkle; the slot's tooltip names Space as its key.

## 8. The gold

- The inventory window draws no gold: the gold, the stats' `+0x46` (the creature's `+0x3EE`), is the statistics window's (seen on its last line, not read).
- In that window's input (`0x6A62C0`) a click in a region tested against 723 to 1567 and 368 to 753 (their units not read) drops gold, all of it below 1000, else a tenth, taken by `0x57FD80` and laid at the hero's place by `0x4AEBD0` (**hypothesis**: the gold's line clicked).

## 9. The item's tooltip (the item's virtual `0x44`, `0x5C5360`)

- The grid's hover asks for it with the flags `0x5F` and shows it in the manager's tooltip, the style `0xB`, at the item's right edge and 16 and its top and 16 (`0x6C1480`, `0x75B390`); the tooltip's window (`cUI_Tooltip`, `0x6E76C0`) adds the item's sockets at its bottom left (`0x5C0A70`).
- The record shown is the item's with its socketed items' merged in (its virtual `0x40`, `0x5C4220`: the highest requirements, equal bonuses summed).
- Its lines in order, `\c<argb>` a colour and `\g` an image inline:
  1. the set's name, `SetName<n>` of the `Weapon.pak +0x23` set, in `0xFFFFD070`;
  2. `UI_TT_QUESTITEM` in `0xFFFFFF80` for an item of `+0x14` bit `0x800` (the flag `0x40`);
  3. `UI_ITEM_CANTUSE` in `0xFFFF0000` where the hero's class may not use it;
  4. with the flag 4 the price, the text 1055: the value and, where the hero has the skill 21, its value by the skill (`0x43C350`; **hypothesis**: trading);
  5. unless of the category 26, two lines of the damage and the resistances, the texts 1176 and 1177 in `0xFF30FF30`, with the kinds' icons (205 to 208) of the kinds above 5 %;
  6. two lines, the texts 1168 and 1169, "− x.xx %" (`0x5C8B00`: a weapon or a shield against the hero's; not read);
  7. the item's level `+0x19`, the text 1070, red with the caller's flag `0x100`;
  8. `A` (`+0x2C`) with the text 1074 and `D` (`+0x2E`) with 1075, where not 0;
  9. each kind of damage that has a most: the physical (1078) in `0xFFFFFFA0`, the fire (1079) in `0xFFFFA0A0`, the magic (1080) in `0xFFA0A0FF`, the poison (1081) in `0xFFA0FFA0`, each "least-most" or one number, followed by its bonuses' lines of the effects 801 to 808 (`0x5C4450`);
  10. the requirements not 0, `INVENTAR_MIN_LEVEL`, `_FORCE`, `_DEX`, `_CHARISMA` and `_END`, white where met and `0xFFFF0000` where not, then the skill's (the text 1108);
  11. the resistances, the text 1195 in `0xFF007700`, the four values with the icons 209 to 212 in their kinds' colours and their sum in brackets;
  12. the other bonuses' lines, then each socketed item's own description;
  13. with the flag 8 a grey (`0xFFAFAFAF`) hint of its use: a potion's text (`0x43F860`), a rune's (1010), a book's (1011), a bridle's (`UI_HORSE_BRIDDLE_USE`).
- The item's name in its quality's colour is not among these lines in the code read; seen, it stands first, in white for a potion.

## In `opensacred-game`

The hero picks the items up into the backpack's grid at their first fit by columns (`pickup::Pack`, docs/decisions/2026-10-26-pickup.md), and a new game's hero starts with its template's backpack (#1285).
The grid shows since #1268 (`backpack`): its cells, the blue tint under an item and the items as their models by their transforms; its red cells and the sockets' icons not yet.
The equipment window's frames since #1288 (`equipment`, `opensacred_rules::equipment`): each class's slots at their places, white, and while the cursor holds an item green where the slot takes it and the hero may use it, else red, by the use's test of its classes, its weapon's kind and the level it asks (its attributes' and skill's requirements and a class's variants not followed); putting items on and taking them off since #1289: the left button on a worn item takes it off onto the cursor, with an item held puts it on where the slot takes it with its category's sound, the old one onto the cursor, or the hero refuses it by its line (`NOTMYITEM` for its class, `NOTMYITEMLVL` for the rest); the right button on a worn item puts it into the grid at its first fit, and on an item of the grid puts it on in the first empty slot that takes it, else the first, the old one into the cell it left; the worn items drawn as their models centred in their frames; the classes' default items, which the window does not show and a slot emptied takes anew, not followed, and since #1290 the hero's fight takes the worn items' values (its maker's, else its type's record's): its attack, defence, damage, resistances and regeneration made anew with its effects at its level as an item is put on or taken off; since #1307 the hero wears them on its skeleton, the wings on its back, each taking off its class's default in its slot until the slot empties (docs/re/equipment.md), the amulets and the rings not drawn; the items in its hands, by whose class it picks its clips, are the weapon sets' (#1271).
The belt since #1311 (`belt`, `opensacred_rules::potions`): its slots lit by the potions of their kinds in the grid, the left button on a slot and Space for the kind 1 drinking the slot's potion as above, the right button on a potion of the grid drinking that one, with the drinking sounds; a healing potion gives back its share of the most health, added (**hypothesis**), and is used up with its sound where the hero's health is below its most; since #1313 Shift with Space drinks the yellow potion, and the right button on a slot of the kinds 1, 2 or 4 gives its potion to the hero's weakest follower below 99 % of its health; the other kinds' effects, the horse, the sparkles and the tooltips not yet (#1272).
The runes read since #1315 (`runes`): the right button on a rune of the grid, the cursor empty, teaches the hero its art (`items.pak +0x6A`) or raises it a level where the hero may use the rune by the use's test of #1288, the rune used up with `UI_INV_USEITEM01`; the spells' page, the sparkle and the event (7, 2) not yet.
The cursor's item since #1269 (`held`): the left button on the grid takes an item onto the cursor, puts it where it fits with its category's sound, or swaps it with the one item under it (with no sound: whether the swap plays one is not read); the cells under it are tinted green, purple or red; it is drawn centred on the cursor, which hides; an item picked up while the page shows goes onto the empty cursor with `PICKUP01`; a left click on the world drops it beside the hero as `cEngine::dropItem` does, the click doing nothing else; the right button, Shift's clicks and the equipment's slots not yet.
The windows show since #1267 (`inventory`): the key `I` and the bar's inventory button open and close the inventory and the equipment beside it with their sounds, on the original's screen centred along the window's width as the bar is; the inventory's frame, its three tabs and its sort's button, the equipment's ornament and frame, as above; their clicks kept from the world; the slots, the tabs' pages and the statistics' window not yet.
The weapon sets' rule since #1319 (`opensacred_rules::sets`): a weapon's hands and hold, the put of a shield or a weapon into a half, the drop's two tries and the right half's item taken; the taskbar's sets themselves not yet (#1271).
#1088 follows this page in its sub-issues: the windows and their layout (#1267), the grid (#1268), the cursor's item (#1269), the equipment's slots (#1270), the weapon sets (#1271), the potions and the belt (#1272), the tooltip (#1273) and the extras (#1274).

## Open questions

- Where the item's name and its quality's colour are drawn in the tooltip.
- The tabs' advance and font; the equipment window's event to the camera.
- `0x5D8FD0`, which refuses every weapon set's put; `0x4263A0`, `0x427E30` and `0x427E80` of the slot's test.
- `0x5DB280` (the sort's fallback) and the sort button's mode.
- `0x43F710` (the cube), the references `0x11` and `0x1A` of the Shift's clicks, the hero's query `0x51`, the engine's event `0x14`.
- The statistics window's gold and its region for the drop; the sparkles' list at `+0x116C`; the cells' mark `0x40`.
- What the creature events 5 to 8 and 10 of the potions do to their drinker: whether the event 5 adds its share of the most health or sets it (seen only at 100 %, on bronze, where the original's new heroes start).
- The tooltip's lines 5 and 6, the price's skill and `0x5C4450`'s bonuses' texts.
- Which of `+0x14` and `+0x19` is the level an item asks (§4).
- The clicks seen in the running original, which the driver's input did not reach.
