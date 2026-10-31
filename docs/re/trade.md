# The trade: the merchant's window, its stock, buying and selling

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra and byte scans on 2026-10-31 (#1066, #1617).
Status: **confirmed** by code unless marked otherwise; the window, its pages and the purchase window **seen** the same day in the running original (the driver of #448) at the merchant of 50,40 (the creature 1165, type 692, level 2), the Seraphim at level 1.

The prices are docs/re/inventory.md, §9 "The price" (`0x43C350`, `opensacred_rules::trade::shown`, #1273).
The talk that opens the traders' windows is docs/re/arts.md, "The combat master" (`0x52AB70`): a creature of `+0x200` bit `0x2000` without a dialogue opens the inventory and `UI_WND_MERCHANT` and sends the window the UI event `0x2E` of the sub-kind 8 with its level (`+0x3FE`) and its id.

## The merchant's window (`cUI_Merchant`)

- Its constructor `0x6DB9B0`: at (0, 0), 640 × 336, the window `UI_WND_MERCHANT`; its layout `0x6DC010`, its draw `0x6DBAC0`, its input `0x6DBF50`, its show `0x6DBB70`.
- A panel `UI_STATIC_BG` at (512, 16), 128 wide and 12 less than the window high, tinted `0x9F000000`, with pieces of a frame along its edges and the window's top.
- The merchant's portrait (`+0x158`) at the panel's top, by its type's portrait (`0x4345C0`, the table of 84 bytes at `0x9ECED8`), else the default; the text 1197 in the font 1 in `0xFF0050FF` below it (**seen** at (540, 212)).
- The tab sheet `UI_TABSHEET_MERCHANT` at (0, 52), 640 × 256, of three pages (`cUI_Merchandise`, below) `UI_TABPAGE_WEAPONS`, `UI_TABPAGE_ARMORS` and `UI_TABPAGE_VARIOUS`, their tabs the texts 1057, 1058 and 1059 (**seen** in a row at the window's top).
- Its input: a press within it goes to the tab sheet; the event `0x2E` 8 of another merchant than its own (`+0x1D8`) keeps the merchant's id and level (`+0x1D8`, `+0x1D4`), takes its portrait and sends the tab sheet the event 8 again with the fifth word 1 (`0x6DBD20`); any other event goes to the tab sheet as it is.
- Its show passes the merchant and the hero to the inventory entries (`0x5DEBC0`, `0x5DEE10`; **hypothesis**: who trades, for the prices); its hide sends the UI event 7 and clears them.
- The inventory's hide hides it (docs/re/inventory.md, §2).

## The pages (`cUI_Merchandise`)

- Each page (`0x6D9580`) is 640 × 288 at the sheet's (0, 0), an inventory entry of its own (`0x5DEC90` of the entries 20, 21 and 22), its grid 16 × 9 cells of 32 pixels from the page's top left (`0x6DB850`: the entry's `+0x115C` 16 and `+0x115E` 9); the statics of 20 × 8 cells that it also makes are the cells' backgrounds.
- Each page holds a list of up to 16 categories, each with a weight from the table of `0x43B060` (`0x890B60`, the same as the drops' random category's, docs/re/items.md):
  - the weapons: 5;
  - the armour: 6, 13, 17, 19, 21, 25, 23, 18, 22, 24, 29 and 33;
  - the rest: 8, 16, 28, 20, 9, 9, 9, 8 and 30 (the potions three times and the rings twice, each with its weight).
- Its draw (`0x6DAA70`) draws the grid as the backpack's (`cInventoryEntry_render`, docs/re/inventory.md, §3: the items' blue, red for an item the hero cannot use) with `0xFFFF00` on the items that cost more gold than the hero has, and the hovered item's tooltip with the flags `0x5F` and the price as bought (`0x5DEE60(item, 1)`) at the cell's right.
  **Seen**: with no gold every item yellow but the red ones; with 5,000 gold the items blue but a few armours above it.
- Its show plays the sound event (14, 2, `0x1F`) and switches each item to the local hero's class's variant of its type (`0x5C7370`, docs/re/items.md, "A pass: making the item").

## The stock

One stock serves every merchant: the pages' items are the last merchant's, and a merchant spoken to again finds them as they were left, or thinned and refilled.
A page receives the event `0x2E` 8 (`0x6DAD90`) with the merchant's level `ℓ` (kept at `+0x3E4`) and its id:

- another merchant than the page's last (`+0x3E8`): the page is emptied (`0x6D9B90(1)`), the merchant's own items given by the scripts are added (`0x6DA1A0`, not read further), it is filled (`0x6DA320(0)`), and its potions topped up;
- the same merchant: in one case of two (`rand() & 1`), up to `⌊5·r⌋` cells are emptied at random (`0x6D9B90(0)`: each a column `⌊r·16⌋` and a row `⌊r·9⌋`, the item over it destroyed), and where `k` items left, the page is filled with `R(1, 2k)` tries; then its potions are topped up.

Here `r` is `rand()` times the float `0x890718`, a little below 1/32767, and `R(a, b)` the game's roll between two bounds.

### A fill (`0x6DA320`)

1. The tries: `⌊10 + r·(⌊10 + 10·r'⌋ + 10·t)⌋`, `t = min(⌊ℓ/20⌋, 5)`, 10 to 28 below the merchant's level 20 and up to 78 from its level 100; both rolls made where a count is given too.
2. The hero's level `L`, the top level `⌊L/3⌋ + 6 + L`, and the quality keys by its trading skill `s` (21): `x = 10.5·s/(L + 1)`, the least key `⌊(x + 2)/4⌋` and the most `⌊min(x, 10) + 2⌋`, the least one below the most where it would reach it; untrained 0 to 2, at the first level of trading at the hero's level 1 from 1 to 7.
3. A category by the page's weights (`⌊r·W⌋` against the weights summed in the list's order, none where it reaches `W`): for the weapons, one weapon of the hero's best weapon skill (below).
4. Each try, while fewer than five items were not made:
   - a category by the weights; the runes (28) make nothing;
   - a key `R(least, most)`, the hero's class in one case of two (`rand() & 1` of 0), else any, and the level `⌊L + r·(⌊L/3⌋ + 6)⌋`;
   - the type of the drops' choice (`0x5C7FA0(category, level, 2L + 100, 15, class, 0, 1, key)`, docs/re/items.md, "Drops"): its third argument is not read, the sixth 0 tests the records' levels, and the key is the quality that the templates' word 40 must not pass;
   - gold (`0x43AF50`) and a type the page holds already (`0x5DA780`) are passed over;
   - the item is added (below); one that was not made counts: no type, no room, or one of the kinds placed as they are.

The best weapon skill's weapon: the balance's lists of the weapon skills' types (the sixth table of the tail of `Balance.bin`, `0xAD57EC`, docs/02-data-formats.md; the skills 3, 4, 5, 7, 13 and 24 with 6 to 8 types each), in their skills' order, the skill of the hero's highest level among them, a skill of the same level taking its place in one case of two (so with every skill at 0 the last that wins its roll, none where none does); of its list a type at random, which for the Dwarf (type 8) and the skill 7 must be a gun (`Weapon.pak +0x1E` 13 or 14, `0x428000`), the others drawn out in turn; its key `R(least, most)`.

### An item added (`0x6D9DE0`)

- None for no type; its size in cells (`0x428D60`) looked for at its first fit column by column (`0x5DABA0`'s mode 0), none where it fits nowhere.
- The object is made (`0x5FB530`) and placed; gold, a potion (`0x428110`) and the categories 28, 26, 30 and 31 stay as they are, and the add answers that it made nothing.
- Another item is made by the item maker (`0x5D5080`, docs/re/items.md) at the level `R(⌊0.98·L⌋, ⌊1.1·L⌋)` in one case of two, else `L`, by the floats a little above 0.98 and 1.1, rolled again where the first roll is below 1 (1 then), with the key, the raw key of the maker (not `16 − k` as a drop's); before the level, the maker's fourth argument is rolled (0 in three cases of four, else 1, 3 or 2), which it does not read.
- While it is made, the balance's `SlotFeasibility` is raised by `SlotProbBonusMerchant` (`0xAD56F0`), the sockets' chance (**hypothesis**: of the quality pass `0x5D2E40`).

### The potions

A page whose list holds the potions and that holds fewer than 8 of them (`0x5DA810`) takes `⌊4 + 12·r⌋` more, each the drops' potion (`0x43B1A0(difficulty, 1, −1)`): a healing one in 601 cases of 1,000, else of the rows 2 to 4 (`R(2, 4)` for its second argument 1, where a drop's is `R(0, 4)`), its size by the difficulty.

**Seen** at the merchant of 50,40 for the Seraphim at level 1: about thirty weapons, eight armours and shields, and on the third page one jewel and seven potions; most of the third page's tries find no ring, book or amulet of the level, and five such end the fill.

## Buying and selling

The page's input (`0x6DAD90`):

- A left press on an item with nothing on the cursor: where it does not fit into the hero's backpack (`0x5DADB0`), the purchase window of the kind 4; where the hero cannot pay its price as bought or it has no room (`0x6D9840(…, 0)`), the kind 2; else, the left button, the kind 1 with the price, the item kept (`+0x3DC`); the right button buys it at once.
- With an item on the cursor: the item is kept (`+0x3E0`), the cursor emptied, and the purchase window of the kind 3 asks with the price as sold (`0x5DEE60(item, 0)`).
- The answer (the event `0x2E` of the window's kind with 1 or 0): to the kind 1, Yes buys the kept item; to the kind 3, No puts the item back onto the cursor, Yes adds its price to the hero's gold (`0x57FD80`) and sends the merchant the event `0x2E` 7 with it.
- The event 7: the item goes onto the page of its category (the rings' page also takes the runes and the categories 7 and 15), at its first fit, and is destroyed where it fits nowhere (the log `cUI_Merchandise::receive_event …`); so a sold item can be bought back, at its price as bought.
- Shift with the left button on the backpack's item sells it at once (docs/re/inventory.md, §3).

The purchase (`0x6D9840`): with the hero's gold at least the price as bought and a fit in its backpack, the item leaves the page, the price leaves the gold, the item's magic record clears its bit `0x200000`, and it is placed into the backpack at the fit.

### The purchase window (`cUI_Purchase`)

- `UI_WND_PURCHASE` (`0x6DCDD0`), shown by the UI event `0x11` with its owner's kind (1 the merchant, 2 the blacksmith, 3 the master), its own kind and a price; it takes the interface's input to itself while it shows (`0x6DD450`).
- Its draw (`0x6DCED0`): the kinds 1, 3, 5 and 6 two lines, the text 1000, 1001, 1003 or 1005, then the text 1004, the price and the text 1055 (`%s\n%s %d %s`), and the buttons 1034 and 1035 (Yes, No) at its left and right; the kinds 2 and 4 the text 1002 or 1190 alone and the button 1028 in the middle; at the screen's `(320 − w/2, 114)`, as wide as the text `w` plus 32 and 92 high, with a frame of `UI_STATIC_BORDER` pieces (**seen**: 147 × 92 at (263, 114) for a price of 810).
- A press on the first button answers 1, on the second (or the one) 0, each with the click `0x7325C0(5)`, to the owner's window (`0x6DD4A0`, `0x6DD700`).

## In OpenSacred

Since #1617 the stock is `opensacred_rules::trade::stock`: the pages and their weights (`Page`), a page's items and grid (`Shelf`), the visit of a merchant (`Stock::visit`) with the fill, the thinning, the best weapon skill's weapon and the potions, the items made by the drops' choice and `drops::keyed`.
Since #1618 a merchant without a dialogue shows, as the hero speaks to it, the inventory and its window (`merchant`, `talk::Speech::Merchant`): the panel tinted with its edges, the portrait at (524, 42) and the text 1197 centred at 208, the three tabs over the page that shows, each page's grid of 16 × 9 from (0, 52) with its items' models, tinted blue, red where the hero may not use an item and yellow where it costs more than the hero's gold (`merchant::cost`, the price of `drops::Tables::trade_price` as bought), and a hovered item's tooltip with its price as bought; the stock is made for the merchant's creature and level as the window opens, and kept for the game.
**Seen** beside the original's frame at the merchant of 50,40 for the Seraphim at level 1: the same tabs, grid, portrait, title and tints; the pages of about 15, 14 and 12 items for 30, 8 and 8 in the original's, by their rolls.
Not followed: the merchant's own items that the scripts give it (`0x6DA1A0`), the sockets' chance, the frame's pieces (the panel's edges a grey line), the page's show's sound and its switch of the items to the hero's class's variants, the stock in the saves, and the trade itself (#1619).
