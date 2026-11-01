# The blacksmith: his window, the sockets he fills and empties, and his price

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra on 2026-11-01 (#1067).
Status: **confirmed** by code unless marked otherwise; the window **seen** the same day in the running original (the driver of #448) at the smith of 50,44 (the creature 464, type 688, level 4), the Seraphim at level 1: at (0, 0), 640 × 352, its grid at the left, the smith's portrait and the three round buttons at the right, the sockets and the work's button not shown while the grid is empty.

The items' sockets are docs/re/items.md: a record's `+0x0C…+0x13` hold eight sockets' kinds (0 none, 1 bronze, 2 silver, 3 gold), the item maker rolls them ("The sockets"), and the worth counts their kinds (`RareSlotBronze`, `RareSlotSilber`, `RareSlotGold`) and their items (`0x5D3AB0`).
The talk that opens the traders' windows is docs/re/trade.md and docs/re/arts.md, "The combat master" (`0x52AB70`).

## Who opens it

- A creature of `+0x200` bit `0x1000` (the tag `0x23`, docs/re/ai.md; the cursor's hammer, docs/re/cursor.md) that the hero talks to and that has no dialogue of its own shows the inventory (the UI event `0x11` of `UI_WND_INVENTORY`) and `UI_WND_BLACKSMITH`, and sends the window the UI event `0x30` of the sub-kind 8 with its level (`+0x3FE`) and its id (`0x52AB70`), as the merchants' `0x2000` and the masters' `0x4000` do with theirs.
- The script's `PlayAnim` of the name `schmiede` opens it too (docs/re/netscript.md).
- The Dwarf's skill 33 has a button of the inventory that opens the same window with the hero as the smith (docs/re/inventory.md; the button not read): the window tells the two apart by its smith's id, a hero's below 17, and its smith's class (`+0x10`), 8 the Dwarf.
- Its show (`0x6A41F0`) sends the event `0x32` of `UI_TABSHEET_INVENTORY`, plays the smith's window's sound (`0x6770E0(0xE, 2, 0x1E, …)`, docs/re/sound.md: the window 30, `7108`) and passes the smith and the hero to the inventory entries (`0x5DEBC0`, `0x5DEE10`), as the merchant's does.
- Its hide (`0x6A3080`) puts the items of its grid back into the hero's backpack (the inventory entry 18), then those that wait for the sockets, an upgrade among them lost, each that finds no room dropped at the smith's feet (`0x5FCCB0`), and empties a cursor that holds an upgrade (the cursor's kind 5, below).
- The inventory's hide hides it (docs/re/inventory.md, §2).

## The window (`cUI_Blacksmith`)

- Its constructor `0x6A1FD0`: at (0, 0), 640 × 352, the window `UI_WND_BLACKSMITH`; an inventory entry of its own (`0x5DEC90`) of 3 × 5 cells at (16, 64), the grid of the item it works on; a 3D view of an item (`cBsItem3D`, `0x5C9510`) for the tooltips; its layout `0x6A4F30`, its draw `0x6A33A0`, its input `0x6A4380`, its show `0x6A41F0`.
- Its layout: a panel `UI_STATIC_BG` from y 16 to 28 above the window's bottom, tinted `0x9F000000`, with pieces of a frame; the smith's portrait at (524, 42) by his type (`0x4345C0`, the table of 84 bytes at `0x9ECED8`, as the merchant's), else the default; three buttons at (512, 192), (576, 192) and (544, 256); the button that does the work at (432, 232); four sockets' cells from x 16, 88 apart, at y 232 with their frames (**hypothesis** for the cells' order: the sockets 0 to 3 from left to right).
- The event `0x30` 8 keeps the smith's id (`+0x158`) and works out his tier (`+0x15C`, `0x6A2230`, below); a smith of class 8 takes another picture of the panel (`0xA0DDA8` instead of `0x9F2454`).
- Its draw: the text `UI_BLACKSMITH_MAKE` at (16, 16) in the font 1; the three buttons with the pictures of the upgrades they give, and under the pointer the upgrade's tooltip (the 3D view's, at the pointer's right); the item's sockets, up to four, each empty one's frame (`0x9F208C`) or a filled one's (`0x9F20E0`), the item in it (a socket's item, or the upgrade waiting for it) drawn at its icon's size, an upgrade at 1.9 times; under the pointer an empty socket's hint (the text `0x26DD`), a filled one's its item's tooltip; the price (below) at (429, 148) in `0xFFFFFF80`, or in `0xFFFF0020` where the hero cannot pay it; the work's button lit (`0x9F22B0`) where the item has a socket, something waits to go in and the hero can pay, else dark (`0x9F2358`), and its hint (`0x26D8`) under the pointer; over the empty grid the hint `0x26DC`.

## The work

The grid takes one item, through the inventory's rules of a fit (`0x5DB0D0`):

- of the categories 5, 6, 13, 17, 18, 19, 21, 22, 23, 24, 25 or 33 (the weapons, the shields and the armour's pieces), for which the item's check `0xE` (its virtual `+0x24`) answers other than 0 (**hypothesis**: one that may be worked);
- an item put in takes the place of the one there, which goes onto the cursor; a press on the grid with nothing on the cursor takes the item onto it, and anything waiting for its sockets is dropped.

Its sockets, the first four at most (`0x5BE9C0` counts those of a kind other than 0), each either filled already (the item's list of its sockets' items, `0x5BEAE0`) or empty:

- A press on an empty socket with an item on the cursor puts it there to wait (the cursor emptied, the socket keeping its id, its type and its picture), where the item may go into a socket (`0x428780`): the rings (category 8), the amulets (20), the blacksmith's upgrades (26), the runes (28), and six quest items (the dragon orbs 6075 and 6076, the dragon scale 6089, the sacred symbols 6112 and 6113, a skeleton warrior's head 5100); a press with the cursor empty takes a waiting item back onto it.
- A press on a filled socket with the cursor empty takes its item out (`0x6A27B0`), for nothing: an item of another category than 26 is made anew of the socket's record and put onto the cursor, an upgrade is lost; the socket's kind keeps, its record's mark of a filled socket (`+0x04` bit 31) cleared, and the item's worth is made again (`0x5D3AB0`); whatever else waited for the sockets is dropped.

The three buttons, with the cursor empty, put onto it an upgrade of the smith's tier `t` (0 to 5), not an object yet but its type alone (the cursor's kind 5), made only as it goes into a socket:

| Button | Types | Name |
|---|---|---|
| 0 | 8000 + `t` | `SCHLEIFEN` (grinding) |
| 1 | 8020 + `t` | `HAERTEN` (hardening) |
| 2 | 8030 + `t`, the Dwarf's 8042 + `t` | `VEREDELN` (refining), the Dwarf's `VEREDELN_R` |

The types 8010 to 8015 (`KERBEN`, notching) and 8036 to 8041 (`HAERTEN_R`) are of the category 26 too, but no button gives them.

The work's button, where something waits and the hero has the price (the hero's gold, `+0x3EE`), takes the price from it (`0x57FD80`) and puts every waiting item into its socket in order (`0x5BFF60`, below, with the smith's tier), plays the smith's sound again, and shows the item's sockets as they now are (`0x6A2470`).
The Dwarf as the smith may work only an item and socket items that his check allows (their virtual `+0x3C` with his level and his skill 33; **hypothesis**: their requirements against them), else the work's button does nothing and the item's tooltip shows in red.

## The smith's tier (`0x6A2230`)

- A creature as the smith: the difficulty (`0x182EE44`, 0 bronze to 4 niobium), one more for the smith of the type 367 (`TYPE_NPC_NEOPHYSTIS`); at most 5.
- The Dwarf as the smith: `⌊min(0.025·s + 0.5, 5)⌋` of his skill 33's value `s` (`0x582450(0x21)`), so 1 from 20, 2 from 60, 3 from 100, 4 from 140 and 5 from 180.

## The price (`0x6A2B20`)

`⌊(0.05·P + 0.1·S + 500) · n · f⌋`, in single floats:

- `P` the item's worth (its record's `+0x08`);
- `S` the sum of what waits for its empty sockets: a rune 2,000, any other item its record's worth, an upgrade its type's record's worth (`0x4340B0`); a socket filled already counts nothing;
- `n` the item's sockets (all eight counted), at least 1;
- `f` 1, or `100 / (s + 79)` for the Dwarf as the smith of skill 33's value `s`: 1.27 at 0, 1 at 21, 0.5 at 121.

The products by the hundredths are of whole numbers (`5·P` and `10·S` as 64-bit integers, times the single float 0.01, a little below a hundredth), then added to 500 and multiplied by `n` and `f`.
At the x87 unit's 53 bits a price that comes to a whole number in real numbers falls just below it and is one less: an item worth 1,000 with a socket costs 549 (**hypothesis**: the window's draw runs in the render thread's frame, measured at 53 bits, #589, and its input there too; at the main thread's 24 bits it would cost 550; not yet measured in the running original).

## A socket filled (`0x5BFF60`)

- Refused where every socket of the item is filled, or the item may not go into a socket (`0x428780`, above); the item put in is destroyed either way.
- The first item put into the item keeps the item's requirements as they were (`+0x192…+0x197`, the record's level `+0x14`, its `+0x15…+0x17`, `+0x18`, `+0x1A` and `+0x1B`), to take them back when its sockets empty; then each requirement of the item is raised to the socket's item's where that is higher (the level, the three bytes after it and `+0x18`; the skill `+0x1A` with its level `+0x1B` where the level is higher).
- The socket's item's record (130 bytes) joins the item's list of its sockets' items, the next socket in order filled.
- Its values are added to the item's record (`0x5BFB60`), each times `1 + (k + t)/100` and truncated, `k` 1, 3 or 9 by the socket's kind (bronze, silver, gold; the kind 4 as gold, 0 none) and `t` the smith's tier: the four kinds' least and most damage (`+0x1C…+0x2A`), the four resistances (`+0x32…+0x38`), `A` (`+0x2C`), `D` (`+0x2E`) and `+0x30`.
  The hundredth is the float 0.01 again, so at 53 bits a product that comes to a whole number falls just below it: `A` 100 in a gold socket by a smith of tier 2 adds 110, not 111 (the same **hypothesis** as the price's).
- The item's values are worked out again (`0x5C0810`), and in a network game the change is sent (the event `0x16C`).
- Taking an item out (`0x5C0430`) subtracts the same, each held at 0 or more.

## Not read

- The Dwarf's button of the skill 33 and his checks (`+0x3C`), and the item's check `0xE` (`+0x24`).
- What the upgrades' records give an item (their values and bonuses, read with the items' table, docs/02 §6.1), and how the sockets' items' bonuses count where the item is worn.
- The window's pictures, fonts and hints' texts by their numbers, measured against the running original.
