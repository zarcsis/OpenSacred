# The items' values

How `Sacred.exe` 2.28 makes an item's values, and so the damage of a creature that holds a weapon (#294).
Status: **confirmed** by code (2026-10-20, Ghidra; the disassembly read where the decompiler drops the x87 arithmetic) unless marked otherwise.
The difficulty is the int `0x182EE44`, 0 bronze to 4 niobium; "truncated" is the runtime's `_ftol`, toward zero.
`R(a, b)` is the game's roll of a whole number between `a` and `b` (`0x443B20` and the same code inline): a negative least becomes 0, a negative most 1, the two swapped when reversed.
Then, but for equal bounds, which take no roll, it is `low + rand() % (high − low + 1)`, the division signed (**confirmed** by code and by the oracle's vectors, #1433).
`rand()` is the C library's, linked into `Sacred.exe` at `0x849B40`: the LCG of docs/01 §7 on the state at `+0x14` of the thread's data of the runtime (`0x84EC63`), 0 to 32,767 (**confirmed** by code, #1433).
`srand` (`0x849B33`) sets that state, and the game seeds it with `time()` (`0x84B68D`) as it starts (`initApp`, `0x815FC7`) and as the engine is made (`cEngine`'s constructor, `0x60ACA9`), so that every run rolls otherwise (**confirmed** by code, 2026-10-31, #1644).
  It seeds it with the time again in a quest's reward (`0x4AC984`), the scripts' drop of a creature's type (`0x4B073D`), `CreateObj` (`0x486401`), at the end of `FillChest` (`0x4A1440`) and at `0x60AB66`, `0x756A69` and `0x812D64`.
  It seeds it with a value of its own only where a roll is to repeat: `FillChest` by the hero and the container (`0x4A0705`, docs/re/containers.md), a creature's line of chatter by the creature's id `+0x0C` (`0x549A84`), and `0x425F64` and `0x426128` (not read further).
The damage kinds 0–3 are physical, fire, magic and poison, as in docs/re/fight.md.

## The template (`wpmod.bin`)

A template is the file's 53 words and its `k` entries of six words (docs/02 §6.3; `tables/weapon_modifiers.ron`).
The lookup `0x42CA70` takes the first template, in the file's order, whose words 0–4 (word 5 of them counting) hold the item's type; the item maker `0x5D5080` first follows the type's parent (`Weapon.pak +0x24`, through the items table's `+0x18`) while the type lies within 2000–2999 or 6500–7999, at most six times.
The words:

- 6, the scale `S` of the damage and the resistances, per cent of the level's factor `f`; 7, the spread `w` in per cent that splits a damage into a least and a most;
- 8–10, 11–13, 14–16, 17–19: each damage kind's chance in per cent, least and most; 20–31 the four resistances' likewise;
- 32–34 and 35–37: the chance, least and most of `A` and of `D`;
- 38, a byte for the item's flags (bits 8–15 of `+0x04`), 29 meaning "make nothing"; 39, the item's least level;
- 41–49, three triples of a chance, a level's divisor and a word not read, and 50–52 their ids: an id 10–17 gives the item a hero class's bit, an id above 100 writes `id − 100` and the level over the divisor to `+0x1A` and `+0x1B` (**hypothesis**: a skill or combat art and its level);
- 40 is not read by the maker; the drops' choice of a type (`0x5C7FA0`, "Drops" below) takes a type only when its template's word 40 is at most the drop's quality (read 2026-10-21, #404).

The entries are the magic bonuses (below).

## The item maker `0x5D5080`

It takes the level, a key of the quality, the type and the item's 130-byte record (the item's `+0x118`, docs/re/fight.md).

1. The level is held at or below `216/MinLevelFactor`, 270.
2. Without a template, for the items table's byte `+0x2E` of `0x10` or `0x1C` (the books and scrolls, and the combat arts' runes, docs/02 §6.1), or a word 38 of 29, it leaves the record as it is.
3. Else it clears the record and writes the type and the flags, keeping the record's class bits (`+0x04` bits 0–5, 22 and 23) but for a weapon, a shield or a ring (the categories 5, 13 and 8, `0x427A40`, `0x427CD0`, `0x4284C0`).
4. The level's factor `f = SpawnWeaponMin + (SpawnWeaponMax − SpawnWeaponMin)·level/100`, 25 + 2.5 a level.
5. A level below word 39 times `MinLevelFactor` is raised to word 39 over it, for the record's level `+0x19` and the bonuses; `f` keeps the level given.
6. The damage and the resistances by `f` (below).
7. `A` (`+0x2C`) with its chance `⌊R(least, most)·f/100⌋`, else 0; `D` (`+0x2E`) likewise: they grow with `f`, not with `S`.
8. The triples, one magic bonus, the class bits, the level, a quality pass, and the price.

## The damage (`0x5D0280`, `0x5CFCC0`)

Without a chance for any kind the item has no damage.
The budget is `p = max(S·f/100 + 0.5, 1)`, in single precision.
The difficulty's term `d` is 0 on bronze, then 0.34, 0.67, 1.0 and 1.34.
In order, each kind by its own chance:

- fire `a1 = ⌊max(R·p/100, 1)⌋`;
- magic `a2 = ⌊max((1 + d)·R·p/100, 1)⌋`;
- poison likewise, unless both fire and magic were taken;
- each element adds `⌊a·d⌋` to a sum `s`;
- physical last: `⌊p⌋` when no element was taken, else `⌊max(R·p/100 − s, 0)⌋`.

The four are then normalised to the budget: each `n = ⌊a·p/T⌋` of their total `T`, and the remainder `⌊p − Σn + 0.5⌋` goes to the first kind that has any, so the sum is always `⌊p + 0.5⌋`.
The rolls give only the shares, and the difficulty moves the share toward magic and poison.
Each kind is split by `q = w/100`: the least `⌊max(n − n·q, 1)⌋`, the most `⌊(1 + q)·n⌋`.
A weapon of physical damage alone has `m = ⌊p + 0.5⌋`, the least `⌊max(m(1 − w/100), 1)⌋` and the most `⌊m(1 + w/100)⌋`.
The constant 0.01 is a single-precision float a little below it, so at an exact edge the result depends on the precision of the x87 unit (`n` 10 with `w` 10 gives a most of 10 at 53 bits, 11 at 24; **hypothesis**: the scripts run at 53 bits).

The resistances (`0x5D0600`, `0x5D0030`) take the same budget, chances and normalisation, without the difficulty's term and without a least and a most.

## The magic bonus (`0x5D4CE0`)

An entry is a chance, a bonus word, an effect, a least, a most, a list and two divisors; the maker takes the first entry whose chance passes, one bonus at most.
Its value is `R(least, most)` times 1.0, 1.34, 1.67, 2.0 or 2.34 by the difficulty, held below that factor times the first float of the effect's record (effects 800–869, 24 bytes each in the balance's image from `+0x9DC`); the effects up to 200 and 600–798 take a value by the level and their divisors instead.
At the fight (`0x568C40`) the effect's byte 2 scales the value by one of the six attributes over 100; the effects `0x321`…`0x324` add it to a kind's least and most, `0x325`…`0x328` to its multiplier, in the attack record for a bonus word with a condition, and **hypothesis**: in the recomputation `0x5796A0` for one without.
The quality pass `0x5D2E40` keeps the slot 0 and clears the others when its map gives 0 (the fills' key 0), and rolls the sockets; it leaves the damage, `A` and `D` alone.

## The bonus slots in the values (`0x5796A0`)

Read 2026-10-22 (#490), the stats' recomputation (docs/re/fight.md, "An NPC's attack, defence and damage") over the equipment slots 0 to 18 (`0x556F50`), each item's own record and those of its sockets: it passes over an item whose `items.pak` record has bit 25 of its first word, bit 1 of its byte `+0x03` (`0x426550`): the seven of `items.pak` with it are cosmetic models, the heroes' hair, the Dwarf's goggles, the Mage's cowl and the Gladiator's belt among them, so that the belt's `D` 5, its resistances and its attributes count for nothing (seen in the original's new game: the Gladiator's `D` 22.6, his attributes his type's).
After an item's damage, resistances, `A` and `D`, each of its bonus slots counts whose condition's low byte is 0 or 13 (not an opponent's class) and whose condition passes `0x568B30` (its bits 8 to 11 0 or 7, else the holder's type, 6 for the types 6 and 7), its value times, by the effect's byte 2, one of the attributes over 100 (`0x568C40`: 1 `a0`, 2 `a2`, 3 `a1`, 4 `a3`, 5 `a4`, 6 `a5`; the product whole, then times 0.01; the attributes before the items' bonuses), else its value:

- 801 to 804: added to the kind's least and most damage, or with the condition's bit 15 to its resistance (the condition's bits 13 and 14 cleared for them);
- 805 to 808: a hundredth of it to the kind's damage multiplier (the stats' `+0x96`…), or with the condition's bit 13, 14 or 15 to the multipliers at `+0xB6`, `+0xA6` or `+0xD6` (not read further);
- 809 and 810: a hundredth to the multipliers of `A` and `D` (`+0xE6`, `+0xEA`); 813 and 814 to those at `+0xEE` and `+0xF2`;
- 811 and 812: to the stats' `+0x3C`, the attack speed, and `+0x40`, the pace, which scales the walk and the run and their clips (docs/re/movement.md, "The speed"; #599), each added and truncated;
- 815 to 820: to the attributes `a0`, `a2`, `a1`, `a3`, `a4` and `a5`, after every item, held at 0 and above, before the attributes' parts of `A`, `D` and the resistances;
- the others are not followed here: the default case hands the effects below 200 and from 1000 to the effect list, and 600 to 798 to a skill slot, and 840 and more have cases of their own.

Seen in the original's new games (#448's driver, the stats read): the Seraphim's attack speed 154, 140 and her weapon's 811 of 14, her `+0x40` 132; the Gladiator's physical ends 25.63 and 31.63, his sword's 22 and 28 and its 801 of 11 times his `a0` of 33 over 100.

## The level an item asks (`0x5D3AB0`)

Read 2026-10-24 (#844; **confirmed** by code unless marked otherwise).
The byte `+0x19` of an item's record is the item's own level, and the byte `+0x14` the level it asks of its holder, worked out from it; neither comes from the file as it is.
The use's test (`0x5BEB30`) takes `+0x14` as the level asked and the tooltip shows `+0x19` as the item's level (docs/re/inventory.md, "Who may use an item"), **confirmed** in the original (#1303): a sword of the level 30 asked 22 of the Daemon, red in the grid to her level 21.
As the client loads the item types (`0x425AEB`, which logs `makeSpawnInfo`, gated by the byte `0x949909`, 1 in the image with no store to it; `0x42C370` from `0x433E72` makes the same pass), it clears bit 31 of each record's `+0x04` and calls `0x5D3AB0(record, 0, 0)`, which, the bit clear, works both out anew from the record's values and sets the bit:

- the damage's worth `D`, the mean of the four kinds' least and most summed (`+0x1C` to `+0x2A`), or their most summed where every least is 0 (`0x5D40C8`);
- `T = max(D + Σ resistances (+0x32 to +0x38), 1)`, and with a template (`0x42CA70`, after the chain of parents) `T = 100·T/S`, `S` its word 6;
- `x = max(T − SpawnWeaponMin, 1)·100/(SpawnWeaponMax − SpawnWeaponMin)` (25 and 275), and the level `⌊x + 0.5⌋`, at most 255 (`0x890908`), 0 becoming 1 (`0x5D47E2` to `0x5D4815`);
- with sockets, at least the highest of its sockets' items' levels (a call of itself at `0x5D409A`);
- then `+0x14 = ⌊max((level + 9)·MinLevelFactor − 8.5, 1)⌋` (0.8 at `0xAD55AC`, 8.5 at `0x890CA0`, `0x5D4973` to `0x5D49AB`), and it answers the item's worth (below), which the loader writes to the record's price (`+0x88` of the `Weapon.pak` record, `0x425B3B`) and the item maker to the made item's (`+0x08`, `0x5D5702`).

The item maker writes the level it makes an item at into the item (`0x5D55FE`), but no flags it gives carry bit 31, so that its call of `0x5D3AB0` (`0x5D56FA`) replaces it by the estimate from the damage and the resistances it made; the level it was made at reaches only the bonuses and the quality pass (`0x42BF40` at `0x5D5601`), which run before (**hypothesis**: so a ring without damage or resistances asks the level 1, as 7511 below).
The sockets' code (`0x5C0430`) takes the level back from `+0x14` the other way, `⌊(+0x14 + 9)/0.8 − 8.5⌋`.
For example the Seraphim's staff 3809 of 1018 (35 to 42 physical and 8 to 15 magic, its template's scale 113) is worth 50, `100·50/113` 44.25, `(44.25 − 25)·100/250` 7.7, the level 8, and `+0x14` `⌊17·0.8 − 8.5⌋` 5, as #448's driver read in the running original where the file has 10 and 6, which follow the same rule for `+0x14`.
`opensacred_rules::fight::item_level` works it out so, its sockets left out (#844).

## The worth of an item (`0x5D3AB0`)

Read 2026-10-31 for #1621 (**confirmed** by code, and on the files: for 4,700 of the 4,708 `Weapon.pak` records of bit 31 the worth below is the file's price, `opensacred_rules::trade::worth` run over `tables/weapons.ron`).
The trade's price of an item is its record's `+0x08` plus its sockets' items' (`0x5BEC70`), which `0x43C350` turns into what it costs and fetches (docs/re/inventory.md, §9 "The price"); the worth is what `0x5D3AB0` writes there, in the balance's `Price…` keys (docs/02-data-formats.md, the balance's image):

1. A potion (category 9) is worth its record's price; a book (category 16) its record's price, else its type's record's, else 600; the gold piles 5132 to 5135 their amount or a roll (not followed).
2. With the item's object given, each socket's item takes its damage and resistances, scaled by the socket's kind, off the item's sums, and the item's level is at least the highest of its sockets' (the game has no sockets).
3. The damage `d`: half the sum of each kind's least and most (`+0x1C`…`+0x2A`) times its weight (`PriceDPh`, `PriceDFe`, `PriceDMa`, `PriceDGi`: 1, 2, 2 and 1.7, physical, fire, magic and poison); where above 0, the worth `⌊c2·d² + c1·d + c0⌋` by the row `PriceDmg:` (300, 10, 0.4), the polynomial stored as a single float first.
4. The resistances `r`: each (`+0x32`…`+0x38`) times its weight (`PriceRPh`, `PriceRFe`, `PriceRMa`, `PriceRGi`: 2, 1, 2 and 1.5); where above 0, the worth `⌊worth + (c2·r² + c1·r + c0)⌋` by the row `PriceRes:` (450, 21, 0.9), the polynomial a single float.
5. `A` (`+0x2C`) and `D` (`+0x2E`) where not 0, each `⌊c2·x² + c1·x + worth + c0⌋` by its three of the row `PriceAwVw:` (`A` 0, 3, 0.4; `D` 0, 4, 0.4).
6. Each of the eight bonuses (`+0x4A`, its value at `+0x6A`) by its effect: its value times the row `PriceBoni:`'s 100 for an effect below 200, 90 above 999, 40 for 600 to 798, 20 for 815 to 819 and 7 for the others; the effects 840 to 867 by their keys (`840_Glckskind` … `867_Tollwut`), some times the value (840, 844 to 845, 847 to 849, 852 and 866), the others fixed, 858 and 860 to 863 nothing, and each of them sets the factor below back to 1.
   Where the effect has no attribute (its high word 0) and its record's fourth float (`0x5E22B0`, the balance's image from `+0x9DC`) is 0, the bonus's worth, at least 0, is added; else `⌊min(v·f/r₄, r₃)⌋` adds a tenth to a factor from 1, a single float, `r₃` and `r₄` its record's fourth and fifth floats, `f` its sixth for a ring or an amulet (else 1) times 1/1.33, 1/1.66, 1/2 or 1/2.33 for the record's difficulty (`+0x04` bits 24 to 27 of 2 to 5).
7. The worth, at least the last polynomial's single float, times that factor where it is above 1; times `1 + q/8` of its quality `q` (`+0x04` bits 16 to 19), `1 + q/2` for a ring or an amulet; times one plus `RareSlotBronze`, `RareSlotSilber` or `RareSlotGold` (0.1 each) for each socket of the kinds 1, 2 and 3 (`+0x0C`…`+0x13`); each step cut.
8. A rune (category 28) a fiftieth of it, its levels set to 1; a worth of 0 its record's price; then times `PricePromille` (1.0).

So the sword 7901 (21 to 23 physical, `A` 5, `D` 2, the effects 811 of 14 and 812 of 2, whose records' fourth and fifth floats are 25 and 2.5) is worth `d` 22, `⌊713.6⌋` 713, with `A` 738, with `D` 747, its 811 raising the factor by `⌊14/2.5⌋/10` to 1.5: 1120, the file's price, by which the original sells it (#1273).
The 8 records whose prices are not their worths (1799, 2006, 2302, 3122, 3851, 4065, 7579 and 7741, each with an effect 856 or 866) are worth 5 to 7 % less; the client works their prices out anew as it loads the types, so it trades them by the worths (**hypothesis**: the files' prices of an earlier balance).

Since #1621 the game prices every type by its record's worth as it loads the tables, and a made item by its values' (`drops::Tables::trade_price`), its quality and its difficulty those of the passes it does not follow (#1367), so 0; the tooltip shows a made item's price.

## The made item's name (`0x5C3130`)

Read 2026-10-29 (#1424; **confirmed** by code and in the original unless marked otherwise).
An object's name is the text of its key `+0x3C` (`0x44A250`, the hand's name beside the cursor and the label on Alt alike), which its constructor sets to its type's key, the `items.pak` record's `+0x20` (`0x428830`, at `0x449883`), and the setter `0x44A1B0` replaces.
The item maker calls `0x5C3130` on the object last, after the quality pass, where it makes the item anew and has an object (`0x5D5697`); an item it leaves as it is, without a template or of the word 38 29, keeps its type's name.
A key the namer composes is resolved by its name (`sdk_QB_RESOLVE`), 0 where `global.res` has no such name.

1. Its type is taken through its parents while it lies within 2000–2999 or 6500–7999, at most six times; a type of the chain without a parent leaves the name as it is.
2. It takes its family's generic name, `EWT_` and the word of its record's byte `+0x05` (`0x43B030`, the table of 33 at `0x949930`, the byte 33 and above the first's): `Schwert`, `Dolch`, `Degen`, `Saebel`, `2HSchwert`, `Axt`, `2HAxt`, `Schild`, `Bogen`, `Armbrust`, `Klingenwaffe`, `Kettenwaffe`, `Peitsche`, `Ruestung`, `Ring`, `Amulett`, `Helm`, `Armschiene`, `Beinschiene`, `Guertel`, `Schulter`, `Speer`, `Keule`, `Stab`, `Magierstab`, `Zaumzeug`, `Schuhe`, `Handschuhe`, `Fluegel`, `Item`, `Pistole`, `Muskete`, `Rucksack`; the maker writes its template's word 38 to that byte.
3. Its template found (`0x42CA70`, else the generic name stays), it composes `Gen_`, the band of its quality (`+0x04` bits 16–19: `st_` below 5, `kl_` below 9, `mi_` below 13, else `gr_`), the word of its element and the word of its class.
4. The element (`0x5C2BF0`): of its most damages (`+0x24` to `+0x2A`), then of its resistances (`+0x32` to `+0x38`), the fire, the magic or the poison where it alone is the highest of the three and not 0, `0x400`, `0x800` or `0x1000` plus its share of the four in per cent; else the first of its bonuses of the effects 802 to 804 or 806 to 808, the same bits by the kind plus twice its value.
   With no such bonus it returns what its x87 stack holds (**hypothesis**: taken as 0).
   The word is `fe_` for the fire of 51 per cent or more (the low 9 bits), else `ma_` for the magic of 41, else `gi_` for the poison of 31, else `ph_`.
5. The class is the template's word 38; at −1, of the type's weapon kind (`Weapon.pak +0x1E`): 1 and 2 a sword (0) or an axe (5) by the substrings `SCHWERT`, `SWORD`, `AXT` and `AXE` of its `items.pak` name (`0x5C3080`, `0x43CED0`), the two-handed ones (4 and 6) for the kind 2; 3 the class 6, 4 23, 5 1, 7 8, 9 11, 10 10, 11 12, 12 9, 13 30, 14 31; another kind, or a class of 33 or more, puts no word in.
   The class's word is the family's (the jump table at `0x5C4194`), the class 4's the one-handed `Schwert` for a weapon of the kind 1, and the class 32's `Kanone`.
   The gloves (27) of the weapons' category (5) and the mage's staffs (24) keep their type's own key instead where its text is not empty, of a type not of a set (`Weapon.pak +0x23`) and of a quality below 4; another mage's staff takes a key of the numbers 2650 to 2689 by `rand() % 40` and `0x5C2E50`, else `EWT_MAGIERSTAB` (not read further).
6. The count `n` of its bonuses, at most 5, over the eight slots: an effect of the kind 0 or 1 of `0x5D2D70` (801 to 808, and every effect outside the other kinds' ranges: 809 to 820, below 201, 1000 to 1200, 600 to 798 and 840 to 869) weighs `max(0.04·p + 0.5, 1)`, the count truncated after each, with `p = ⌊v·f·100/r + 0.5⌋` of its value `v` by its effect's record in the balance's image (`0x5E22B0`: 24 bytes from `+0x9DC` for 800 to 869, then the records of the effects below 200, above 999, of 600 to 799 and of the rest): `r` the record's first float where the bonus scales by an attribute (byte 2 of its effect), else its fourth where its third is below 0.00001, else `p` 0; `f` the record's sixth float for a ring or an amulet (the categories 8 and 20, `0x428540`), else 1; and `p` an eighth (`shr 3`) for a bonus of an opponent's class (its condition's low byte not 0 or 1).
   The effects 840 to 869 count one each.
   The difficulty's factor of the weight is never taken: the code compares the quality bits (`& 0xF0000`) with the difficulty's (`0x2000000` to `0x5000000`).
7. With `n` above 0 it takes the composed name with the highest digit from `n` down that resolves; with `n` 0, the digit 1 where `rand()` is odd and that resolves; else the composed name without a digit where it resolves; else the generic name stays.

`global.res` holds the generic names but those of the pistol, the musket and the backpack, and 496 composed ones, most with each digit 1 to 5 and without one, the rapier's, the chain weapons' and the whips' without digits.
**Seen** 2026-10-29 in the original (#448's driver, `trace` of `0x44A1B0` and the record at `0x5C3130`): three gloves 1221 made by `item 1221` took `EWT_Handschuhe`, then `Gen_st_gi_Handschuhe`, `Gen_st_gi_Handschuhe1` and `Gen_st_gi_Handschuhe`; a boss's 40 drops took the names of their records' families and qualities, as `Gen_kl_fe_Guertel1` of a belt of the quality 5, `Gen_gr_ph_Ring1` of a ring of 14 and `Gen_st_ph_Dolch3` of a dagger with bonuses.
The quality pass (`0x5D2E40`) writes the quality bits last, the sum of its map's value by the maker's key, its sockets' and its bonuses' worth, at most 14.

## A creature's items

A fill (`FillSector`, `0x47C610`) makes all its creatures' items at one level: the hero's within the sector's bounds and jittered, as the creatures' (docs/re/spawning.md), raised by a tenth of a curve of the minutes of a counter (**hypothesis**: the time played) through `0x552230`, which is the level itself on bronze, at most 300; every item of its `MakeGroup` rule is made at it with the quality key 0.
`CreateNPC` makes a record's items at the creature's level with a key at random of 0–3.
A fixed item keeps its record when the maker stops (**hypothesis**: the copy of `Weapon.pak +0x80…+0x101`, whose words fit the record's layout).

## Example: a goblin's dagger

The goblin warriors of 51,39 hold nothing, and fight with `Creature.pak`'s bonuses; the armed goblins are the fills' of the rule `MakeGroup 02:76 87:(71,100)…`, whose pool 71 holds daggers and a short bow.
The dagger 1712 (`DAGGER.GRN`, template 29: `S` 50, `w` 10, physical always, fire and magic 8 % each, `A` 20 to 80 always) at level 1 on bronze: `f` 27.5, `p` 14.25; with physical damage alone (84.6 %) 12 to 15, else 9 to 10 with an element of 3 to 4; `A` 5 to 21, `D` 0; and a bonus of effect 801 of 28 to 40, which adds 6 to 9 physical by its attribute.
At the levels 4 and 5 of the goblins' sectors it gives 16 to 19 and 17 to 20.

## Drops

What a killed creature drops and how the drops lie in the world, read 2026-10-21 (#404) in Ghidra and, where Ghidra leaves part of the creature's events' handler undisassembled (about `0x533BA8`–`0x534763`), in a disassembly of the same bytes.
Status: **confirmed** by code unless marked otherwise.
The balance's values are named by their keys (`tables/balance.ron`; the tables of the tail of `bin/Balance.bin` are in docs/02 §6.3).
`L` is the dead creature's level (`+0x3FE`), `E` that level brought back to the bronze scale (`0x5522D0`, docs/re/fight.md), `H` the local hero's level; "the weighing" is the experience factor `0x551530` of `E` against `H`, 0.04 to 1 (docs/re/fight.md, "The hero's levels"), and a creature is "too weak" when `(E + 9)/(H + 9)` is below `ObereEXPVoll` (1.1) and its weighing at the floor 0.04.

### Who drops

A creature drops in its events' handler as it dies (`0x533BA8`), where it takes the AI's state 9, before the kill's experience is given: the drops leave its position at once, not when its dying clip ends.
A player's hero (class 1, `0x426650`, with an id up to 16) takes another path and drops nothing there.
A creature drops nothing when:

- it is a player's companion (`+0x251` 1 to 16);
- the local hero is dead, dying or at health 0, or not hostile to it (`0x549000`): a creature that is not the hero's enemy drops nothing;
- its `+0x1F0` is 7, or its type is 71 or 75 (`small_spider.grn`, `med_spider.grn`; **hypothesis**: the spiders that a giant spider brings);
- it is of class 5 (`0x426A80`) with a most health (`+0x4D4`) other than 0 (**hypothesis**: an undead that rises again; not followed);
- there is no killer, or the killer is no player (an id above 16) and is not in the local hero's list at `+0x39C` (`0x4FC900`; **hypothesis**: the hero's summoned creatures and companions).

A killer of type 71 with `+0x1F0` 7, or of `+0x1F4` bit `0x40000`, counts as the local hero.

### How many drops, and the bonus

The death calls the drop `0x559BF0` `N` times, each with a bonus `B` and a value `s`:

- `N` is `WurfAnzahl` (1) and `B` 0.
- A creature with a champion's levelling (`+0xB0` bit `0x400`, docs/re/fight.md, "Bosses and champions"; a champion, and a fill's 343 or 212 without the mark, #621) has `B` = `ITBonusChamp` (35) and, unless too weak, `N` + `R(1, ITThrowCountBonusChamp)` (3).
- A boss (`+0x1F4` bit `0x80000`) adds `ITBonusBoss` (50) to `B` and, unless too weak, `R(1, ITThrowCountBonusBoss)` (4) to `N`; then a switch over its type (`0x533E2E`, its jump tables at `0x536EBC` and `0x536F48`) gives some twenty types more drops or particular items (below).
- For a player's kill, `B` becomes `⌊B + LongivityFactor·v(t)⌋` (`LongivityFactor` 0.5), `v(t) = 100·(1 − 1/(1 + (t − 1)/120))` for `t` at least 1, else 0, `t` the statistics' counter `+0x570C` over 1,000 and over 60 (`0x425290`, `0x424650`; **hypothesis**: the minutes played), and then `⌊B + √(100·(ℓ − 100))⌋`, `ℓ` the hero's luck of `0x564A60`: 100 plus the value of its effect 840, raised when some of its counts at `+0x4F8` are high (**hypothesis**: the worn pieces of sets, below).
  So the time played adds up to 50 to the bonus and a luck of 25 adds 50.
- `s` is 0, but for a killer with the effect 42 in its list of effects (`+0x11A`, `0x5539D0`) the value 1 of the combat art record 55 (`0x4D2310`, a table compiled in at `0x952A38`) at that effect's level (`0x4D26B0`), the Dwarf's greed (docs/re/arts.md, "The Dwarf's spells").
- `DropAll` is 0 on file: in `R(0, 1000)` below it the creature would throw up to three of the items it holds (`0x4E9900`, which throws one from the slots 13, 12, 1 or 0); so a creature's own weapons, shield and armour never drop.

The boss switch is a table of cases compiled into the client; here is its shape, its values left for the maintainer (docs/08 §3), the types named by their models:

- 53 and 94 (`BIG_DEMON.GRN`), the dragons 52, 74, 88 and 100–103 (`DRAGON*.GRN`, `dragon_*.grn`), 144 (`WOODGOD.GRN`), 348 (`Killerkrabbe02.grn`) and 185, 250–252 (`daemonenfuerst*.grn`) each add a type's `ITBonus_*` key to `B` and 2 to 4 to `N`, and make a treasure (`0x5516B0`, "Other drops" below) of a chance, a quality and a jitter of their own;
- 55 (`GIGANT_SPIDER.GRN`) adds its key, and 247, 336, 338 and 354 a key and 1 to 3 to `N`;
- 136 (`Medusa.grn`, with its key) and 137 (`TROLL.GRN`) make the doll 3140 (`V_Doll.grn`) in one case of 26 when none is in the world;
- 145 (`WOODGOD_CHILD.GRN`) takes the boss's additions back;
- 182 (`Anducar.grn`) makes no ordinary drop but one item of a random set, or for the sets 33 to 38 the item of 7920–7927 for the killer's class;
- 183 (`Anducar_Demon.grn`) makes six treasures and, in one case of three, one more item (not followed).

After the calls, when the killer has the effect 868 (`0x570ED0`), the creature drops a healing potion (`0x43B1A0` of the row 5, below) when `R(0, 99)` is below `(1 − a/(b + v))·100 + 0.5`, held to `c`, `v` the effect's value and `a`, `b`, `c` the effect's parameters from the balance (`0x5F23A0`; 500, 502 and 25 on file, 100, 100 and 90 for an effect not listed; docs/02 §6.3).

### One call of the drop (`0x559BF0`)

It takes a type `T`, 0 for any, the bonus `B`, the value `s` and the killer.

1. With `T` 0, the scripts' drop of the creature's type (`0x4B06D0`): the first entry for its type in the list that `SetDrop` fills (below) gives its item when `rand() % 100 + 1` is at most the entry's chance; `T` is then that item.
2. Still 0, when the local hero has more than four at the entry 22 of its counts at `+0x4F8` (**hypothesis**: the worn pieces of a set, as the set bonuses read the same counts), `T` is a potion in one case of ten: 5137, 5138, 5144 or 5146 in a case of ten each, else 5140.
3. A creature of class 1 (the heroes) with `T` 0 drops nothing.
4. The chance `P`, per mille, truncated at each step: `WurfWarsch` (350); in a network game for a player's kill, times `1 + n·MPThrowfactor` (0.25), `n` the other members of the killer's group within 1,500 of it (`0x422820`, `0x4228F0`; **hypothesis**: its party), at most 4; times `WurfBoss` (2.0) for a boss or a champion (`+0x2B8` bit `0x40`); times `(100 + v(s))/100`, `v(s) = 10 + 180·(1 − 1/(1 + (s − 1)/100))` for `s` at least 1, else 0 (`0x565D70`).
5. While `P` is above 0, a pass: with `T` 0 it makes nothing unless `rand() % 1000` is below `P`; a type whose template has the word 38 −1, or whose record is missing or of flag `0x40` (five times a call at most), is drawn again, up to six tries in all; then `P` falls by 1,000.
   A try draws again from its start (read again 2026-10-21, #414): it rolls the chance again, takes the bonus on (below) and draws a key, a kind and a type anew; the potions, the gold piles and the types of the choice below have their records and no template of the word 38 −1 on file, so that only a forced `T` can be drawn again.
   So a call makes `⌊P/1000⌋` items and one more in `(P mod 1000)/1000` of the cases: an ordinary creature one item in 35 % of its deaths, a boss or champion one in 70 % at each of its calls, and with a forced `T` every pass makes it.

The console's `droplog` (`0x614230`, bit `0x2000` of `0x182EE3C`) logs every drop to `LogDrop.txt`, and `0x4D1030` makes 50 drops of a creature into `DropTest.txt` (**hypothesis**: tools of the developers, which can check the rules in the original).

### A pass: the quality key

- For a creature with an id above 16, with `DropQuality` (40) above 0 and its `Creature.pak` experience by level `b` (`+0x0A`) not 0, `B` becomes `⌊B·√(40·b)/40⌋`, that is `B·√(b/40)`: at each pass, so that it compounds over a call's passes (a quirk).
- With `T` 0, a key `k` from 0 to 17: `u = R(1, 10000)`; the weights `w_k` of the keys, the second table of the tail of `Balance.bin` (docs/02 §6.3), add up in the order of the keys, each as `⌊w_k·m·g + 0.5⌋` (the key 16 without `m`), and `k` is the first key whose sum passes `u`, 17 when none does.
  `m = 1 + min(B, 700)/100`; `g = 10·((1 − ItemThrowFactor)·f + ItemThrowFactor)`, `f` the weighing, or 1 when `(E + 9)/(H + 9)` is at least 1.1 (`ItemThrowFactor` 0.15).
  On file the weights are 7, 9, 0, 3, 5, 6, 8, 10, 12, 13, 14, five times 15, 300 and 544: for a creature that weighs 1, without a bonus, the key 0 in 0.7 % of the passes, 1 in 0.9 %, 3 to 15 in 0.3 to 1.5 % each, 16 in 30 % and 17 in the rest; a weak creature, `g` down to 1.84, gives mostly 17.
- The type's quality `q` is `16 − k` for the keys 2 to 15, else 0.

### A pass: what it makes

With `T` 0, in order, `C` the creature's class (`0x43ADC0`):

1. In `DropSaft` (180) ‰ a potion (category 9) of `0x43B1A0`: of six rows of three potions (small, medium, large; `0x9499B4`), the row 5 (the healing potions 5171, 5145, 5140) in 601 cases of 1,000, else a row at random of 0 to 4 (the rows 0 and 1 are the same); the column by the difficulty: small on bronze, small or medium on silver, medium or large on gold, large above.
   The rows 0 and 1 hold 5169, 5143 and 5138, the row 2 5168, 5142 and 5137, the row 3 5170, 5144 and 5139, the row 4 5172, 5146 and 5141 (read 2026-10-21, #414).
2. Else, unless `C` is 6 (the animals) or the creature made the last rune, a rune (category 28) when `rand() % 1000` is below `UpgradeDrop·r` (50), `r` of `0x5598B0`: `x = H·RunenProLevel_*/(n/RuneInventoryFactor + a)`, the key by the local hero's class (1.2 to 2.2), `n` the runes it carries (the items of category 28 in two of its containers, `0x5DEBC0`; **hypothesis**: the inventory and the storage), `RuneInventoryFactor` 3.0, `a` the sum of the levels, without their bonuses, of the combat arts in its list of 22-byte entries at `+0x4AA` (**confirmed** by code 2026-10-22, docs/re/arts.md, "A creature's arts"); `r = x⁵` when `x` is above 1 or the creature a boss or champion, else the weighing times `x⁴`, held within 0.01 and `RuneChanceFactorMax` (2.0), times `RunesPerBoss` (1.5) for a boss or champion.
   The rune is one of 123 types at `0x9565A0` (from 5375, `upgrade_*.grn`), a potion instead for the types 516, 592 and 593 or a rune of `items.pak +0x6A` 0.
   Read again 2026-10-28 (#1183): the roll is `(rand() % 1000)·30` against `0x5598B0`'s value with 30, which is `r·UpgradeDrop·30`, so a rune comes when the roll below 1,000 is below `r·UpgradeDrop`; the creature becomes the last to make a rune as the roll holds, before the type is drawn, so that a potion in its place counts too; the type is the table's entry `rand() % 123`, uniform.
   The class's key of `RunenProLevel_*` goes by the hero's type: 1 `_S`, 2 `_G`, 3 `_M`, 4 `_D`, 5 `_E`, 6 and 7 `_V`, 8 `_Z`, 9 `_T`; `H` is the hero's level, at least 1; with neither runes nor arts `x` is infinite and `r` the most; the weighing is `0x551530`'s of the levels (**hypothesis**: the dying creature's against the hero's, the code reading a register the decompiler leaves unnamed); a global at `0xAD3FF8` set to a constant on the first call makes `r` 0 while negative (not read further).
3. Else gold in `DropGold` (300) ‰, always for `C` 6, and for `C` 11 (the ghosts) in 600 ‰ of the rest.
4. Else, unless `C` is 11, a weapon (category 5) in `DropWaffe` (330) ‰.
5. Else, unless `C` is 11, armour in `DropRuestung` (880) ‰: one of eleven categories (6, 13, 17, 18, 19, 21, 22, 23, 24, 25 and 29, the bridles drawn once again).
6. Else, unless the same creature made the last jewel, a ring (in two cases of three) or an amulet (categories 8 and 20).
   The last is a global, the id (`+0x0C`) of the creature that made the last jewel (`0xAD4000`, read again 2026-10-23, #610), which a jewel made sets and nothing clears.
7. Else, and wherever no type is found, the dagger 1712: so a creature makes one jewel at most until another creature makes one, and a later pass that comes to the jewel makes the dagger.
   The same holds over its deaths: the driver's `drop --id`, which makes one creature drop again and again, made one jewel in 1,000 drops of a goblin warrior of level 10 and the dagger 24 times (2026-10-23, #610), where the game, which kept the last jewel within a death alone, made 14 jewels and the dagger 16 times.
   The runes have their own such global (`0xAD3FFC`).

`DropSonst` (1000) is stored by the balance's parser and read by no code.
For potions, runes and gold `k` becomes at least 16.
The animals thus drop potions and gold only, the ghosts no weapon or armour.
For an ordinary creature the kinds share out, without the runes, as 18 % potions, 25 % gold, 19 % weapons, 34 % armour and 5 % jewels.

The type of a weapon, armour or jewel is `0x5C7FA0`'s (the client's `TypeManager::getRandomItem`), from the types of the category, at the level `L` with the quality `q`, any weapon kind, the local hero's class in one case of two (else any), and set pieces left out; read in full 2026-10-21 (#414):

- the category's list, made once: the types below 8191 of the category that the items table lists (below), not of its flags `0x2000000` or `0x8000000` (`+0x00`), not 5120–5131, 5632 or 5633, a rune only with `items.pak +0x6A`, with a `Weapon.pak` record not of the flag `0x40` (its `+0x84`), and no template of the word 38 −1 (by the lookup `0x42CA70`, which takes a type's parent once for the types within 2001–2998 and 6501–7998 and fails for a parent within those ranges; on file it finds the same template as the maker's chain of parents for every listed type);
- of the list, a type with a template whose word 39 is at most the level held at 28 plus a jitter (1, or above level 5 the product of two rolls of 1 to a third of the held level, at most 3, rolled for each type) and at most `L`, whose word 40 is at most `q`, and whose record's levels (`Weapon.pak +0x94` and `+0x99`) lie below the held level plus the jitter, that check left out for the categories 30 and 31 and, for a weapon or armour, in two cases of three, never for a jewel; the record's level `+0x99` must also be at least `⌊h/2 − 15⌋`, `h` the held level, at least 0, which is 0 at every held level, so there is no least level;
- of the types with class bits (`+0x84` bits 0–5, 22 and 23), only those of the class asked: the bit 1 of the Seraphim (type 1), 2 of the Gladiator, 4 of the Battle Mage, 8 of the Dark Elf, `0x10` of the Wood Elf, `0x20` of the Vampiress (6 and 7), `0x400000` of the Dwarf and `0x800000` of the Daemon, and none of a record of the flag `0x40`;
- not of the record's bit `0x100000`, no set piece (the record's `+0x23`, which the sets' loader writes), and no unique (the quality bits 16–19 of `+0x84` all set) of the word 38 29;
- then one at random: with a class asked, in one case of two among those of its bit or of none, which the filter left all of (their indices kept as bytes, a quirk past 256 types, not followed); else, in one case of two, the first of up to three draws that the local hero can wear (`0x54ABE0`: the item's own test of the hero's type, not followed, then the record's least level `+0x94`, four attributes `+0x95`–`+0x98`, 0 in every record of the lists on file, and a skill `+0x9A` at the level `+0x9B`), the third where none is.

The items table (`0x4257A0`, read 2026-10-23, #596) holds an entry of 128 bytes for each type below 32,352: each slot's `items.pak` record, its `+0x18` the index of the type's `Weapon.pak` record (`0x434100`).
The loader of `Weapon.pak` also sets the category of the type 4053, `gladarmor01.grn`, to 6, where `items.pak` has 21.
Then, in `Weapon.pak`'s order, the type of each record with a parent (`+0x24`) takes its parent's entry as it stands, keeping its own `+0x18` and its own `+0x20`, or its id there where it had none.
So a derived type has its root's category, flags (`+0x00`), rune (`+0x6A`) and model, and its own record; a type whose parent has no entry takes an empty one.
On file 2,374 types are derived so, 997 of them within 2000–2999 and 1,377 within 6500–7999, each from a slot of `items.pak` by one parent but 7908, whose parent 7803 is derived from 1773.
By category they are 1,021 weapons, 449 armour of the category 6, 309 rings, 242 helmets, 128 shields and fewer of the other armour, and 33 amulets; 2,337 of them pass the list's flags.
The lookup `0x42CA70` finds a template for all but 15 of them, so that the pass never takes those 15:
- 2000, 2999, 6500 and 7999, on the ranges' bounds, which it takes as they are;
- 7908, whose parent is derived;
- 7968–7971 and 7974–7979, whose parents 1833 and 1834 have none.

Seen in the original (#596), they make about a tenth of the drops, for example 2029 of the knife 1727, 2114 of the hand axe 1741, 6501 of 3853 and 7804 of 4071.

On file the Seraphim's wings (category 25) are all uniques of the word 38 29, so that the armour of that category is always the dagger; the Seraphim has no belts, leg plates or gloves of its bit, nor the Dwarf arm or leg pieces, which give the dagger when its class is asked (the choice applied to the data, #414).

### A pass: making the item

The object of the type is made at the creature's position (`sdk_CREATE`).
Gold takes its amount, `g = a + ⌊(L − 1)·(b − a)/100⌋`, `a` and `b` the two values of `GoldProLevel` (20 and 2,000, docs/02 §6.3), then `g − ⌊g/6⌋ + rand() % (⌊g/3⌋ + 1)`, at least 1, into its `+0x120` (its record's price, `0x5C7D10`), and `0x4AEBD0` is called (not followed; with the code `0x7E60` it makes and throws a gold pile, with the others it only rolls `rand()`, docs/re/character.md): 17 to 23 at level 1, 165 to 231 at level 10, with the creature's own level, which is higher on the higher difficulties.
Its model is one of four piles, 5132 to 5135 (`GoldCoins_01`, `_02`, `_05`, `_10.grn`), chosen before the amount and whatever it is: 5132, but in a case of 13 above level 4 5133, of those in a case of 7 above level 9 5134, and of those in a case of 4 above level 19 5135.
Gold, potions, runes and the categories 26 (the blacksmith's upgrades, slots 8000 on), 30 and 31 are not made further.
Another item is switched to the local hero's class's variant of its type (`0x5C7370`, by `0x43CA20`; **hypothesis**: through the armour switch table `bin/Rust.bin`) and made at the levels `lo = ⌊(L − BaseOffLevel)/(1 + OffLevelProz/100)⌋`, the creature's level brought back to bronze (`L` itself on bronze), and `hi = ⌊1.05·lo + 0.5⌋`, by its key.
The switch gives no variant for a creature type below 32 (`0x43CA20` returns none where it finds one for such a type), so the local hero's type, 1 to 9, keeps the drop's type (**confirmed** by code 2026-10-21, #414).
The factor 1.05 is a single-precision float a little below it, so that `hi` is one less where `1.05·lo + 0.5` would be whole: 10 at `lo` 10, 31 at 30, 52 at 50.
By the key:

- 0, a unique: with `T` 0, the uniques of the item's family (its template's word 38; a map at `0xAD4428` built by `0x5D4AC0`) of quality 15, no set piece, of a least level (`Weapon.pak +0x94`) up to `H`, and not 7911–7930 (reserved for the scripts' drops); in 30 % of the cases, if there is one, one of them at random, raised to `lo` (below); else the item with the key 14 at `R(lo, hi)`.
- 1, a set piece, chosen before the object is made: in 60 % of the cases a set whose first piece is for the local hero's class, has a template and a least level up to `H` and is no icon (5101–5106), else any set with a piece that has a template; of its pieces with a template one at random, the pieces the hero wears kept in one case of two; taken when its level (`Weapon.pak +0x99`) times `MinLevelFactor` is below `H`, and raised to `lo`; failing that, the key 2.
- 2 to 15: the maker (`0x5D5080`) at `R(lo, hi)`, at least 1, with the key `16 − k`.
- 16: the maker at `lo` with the key 0; 17: at `R(⌊lo/2⌋, lo)`, at least 1, with the key 0.

"At least 1" rolls twice (read 2026-10-21, #414): 1 where a first roll is below 1, else a second roll, which is not held (**hypothesis**: a macro's maximum that evaluates its roll twice); so a least of 0 can still give 0, as the key 17's `R(0, 1)` at level 1 does in one case of four.

A forced `T` that is a unique takes the key 0 and a set piece the key 1, both only raised; any other forced item keeps the key 16.
A unique or set piece of level `ℓ` (its record's `+0x19`) is raised (`0x5C29A0`, `0x5C2210`, not followed further) to `ℓ + d·⌊(lo − ℓ)/d + 0.5⌋`, `d = max(ℓ/2, 15)`, when `lo` is above `ℓ`.
The key given to the maker passes through the third table of the tail of `Balance.bin` to the quality pass `0x5D2E40` (0 to 7 kept, then 9, 11, 13, 15, 18, 21, 25 and 25 for 8 to 15).
The maker's fourth argument, rolled 0 in three cases of four, else 1, 2 or 3, is not read by it.

### On the ground

A drop is a `cObject3D` (a `cItemBase`), drawn as its model by its type (`items.pak +0x70`, with the texture of `+0x08`, docs/02 §6.1), not a sprite.
The model has its own scale (docs/re/drawing.md, "The models' scale"): the gold piles are drawn at 0.33, a coin 4.9 units across against the Seraphim's 74 units of height, and the combat arts' `UPGRADE_*` runes at 0.5 (#443).
It leaves the creature's position 60 units up (`0x44D7B0`, `0x44D520`): a height in the world, not over the ground, which lies within 15 of 0 almost everywhere (the creatures' heights, docs/re/movement.md) and 28 a floor higher in a building; the hero's own drop (`0x611620`) starts 60 over the ground.
It flies (`0x44DED0`) with the velocity `(10·R(−75, 75), 10·R(−75, 75), 190)` and spins about each axis at `R(−6, 6)`.
Each frame (`0x44DFE0`, from the objects' update `0x44B230`, `dt` the frame's time) its angles grow by the spin times `dt`, its `x` and `y` by a tenth of its velocity times `dt` and its height by its vertical velocity times `dt`; the velocity takes `−0.1·v` along `x` and `y` and −290.81 along the height, times `dt`.
Where its next cell may not be walked on (`0x636890`) its `x` and `y` velocities turn back, a bounce; below the ground (`0x449950`) it lies on it with no velocity; it rests once each velocity is below 1 and it lies within 1 of the ground.
Computed from the above with the ground at 0, a drop would land about 1.6 s later within about 110 units, two cells, of the creature along each axis; but seen in the original (2026-10-22, #478, the flight hooked frame by frame in two flights, five drops read 4 s after their kills), the velocity along the ground turns back at every frame, the test of the next cell failing at every one (**hypothesis**: it takes the flight's float offset `+0xCC`, `+0xD0` for the place, a point near the world's corner, where the object's own world point `+0x1C`, `+0x20` stays), so that the offset swings between its first step and 0 and the drop rises to about 64 and falls back within a unit of the body in about 1.3 to 1.6 s, at the angles its spin left it; the types 5120 to 5135 (the gold piles among them) and 5375 to 5631 (the runes) are held upright at every frame.
The treasure's items fly the same way twice as far, and the held items that `0x4E9900` throws leave the hand slower, at `(R(−100, 100), R(−100, 100), 150)`, spinning at `R(−15, 15)`.

Holding Alt (`GetAsyncKeyState` of `VK_MENU`, the world view's flag `0x4000`) shows a label over each item in view (`0x627180`): its name, or for gold the amount and a word, laid out by a label layout so that labels do not overlap (`cLabelLayout`, up to 1,024), on a dark translucent panel (`0xCF303030`), in a colour that the item gives (a virtual call, not followed), and the label under the cursor highlighted.

A creature takes an item off the ground as "Picking up" below tells (`cCreature::pickupItem`, `0x558080`).

### The glitter

Read 2026-10-29 (#1382, **confirmed** by code) and seen in the running original (the Seraphim's start, a sword, a healing potion and a pile of gold made at her feet by the driver's `item`, framed at the zoom 0.5).

- The objects' draw (`0x44B230`) calls the glitter, `sdk_GLOW` (`0x408B70`), for an object of `+0x14` bit `0x80000`, or for one whose type has the bit 19 of its `items.pak` record's first word (`0x428460`; the byte `+0x02` bit 3) that no creature holds or wears (the draw's holder none).
  The bit 19 is set for every type of the item categories (counted in `tables/items.ron`: all of the weapons 5, the armour 6, the rings 8, the potions 9, the shields 13, the books 16, the helmets to the arms 17 to 24 and the cannons 33, all but 8 of the category 15, the gold and the other items, and all but one of the runes 28), and for none of the statics, the creatures, the containers, the doors and the effects: so every item lying on the ground glitters (the maintainer's report of 2026-10-29, #1382).
  What sets an object's `+0x14` bit `0x80000` is not read (**hypothesis**: the scripts' quest items, as chapter 2's water bottle glitters in turns of 2.5 s, #960).
- The glitter stands about the object's point and its offset (`+0xD8` to `+0xE0` plus `+0xE4` to `+0xEC`), halved into the models' space (`0x890670`, 0.5), and its time runs from the game's clock in milliseconds plus a hundred times the object's id, at 0.0005 a millisecond, so that no two objects glitter in step.
- It draws 3, 5 or 8 sparks by `DETAILLEVEL` 0, 1 or 2 (2 in the installation's `Settings.cfg`).
  Each spark `i` from 1 moves about the point by the sines of a table of sixteen numbers `a` (`0x8E808C`: 1.3452, 6.6458, 4.6646, 2.7472, 6.4337, 4.5351, 1.7371, 0.9962, 2.5993, 3.9645, 2.3992, 1.7379, 2.8925, 0.0085, 6.7786, 5.5555, taken by index modulo 16), each axis 5 times a sine `sin(t·a[p] + a[q])` times one more than a second `sin(t·a[r] + a[s])`, so within 10 of the point in the models' space: across, `(p, q, r, s)` is `(i − 6, i − 7, i − 5, i)`; along, `(i + 7, i − 8, i + 5, i + 1)`; up, `(i + 4, i + 6, i + 2, i + 3)`.
- Each spark is two camera-facing squares, added to the frame: `PARTICLE_SPARK04` 8 across, in a full hue of the value 250 at the fraction of `a[i + 4] − t` round the wheel (a turn in two seconds; `0x642420`: from blue through cyan, green, yellow, red and magenta), and over it `PARTICLE_SPARK07` of the grey `0xBBBBBB`, its half side 2 plus the first sine of its height, from 1 to 3.
  **Seen**: the sparks of several colours at once, white at their hearts, wandering about the items and fading as they shrink.

### The scripts' drops

`SetDrop` (record type 137, the walker's handler `0x4AFFF0`) sets for a creature type the drop of an item type with a chance in per cent, into a list of `{creature, item, chance}` at the quests' manager `+0x762C`: the two types in either order, a chance 0 removing the creature's entry and another replacing its chance (and its item, both types given).
The trees hold 1,204 such records (docs/02 §7.3): mostly quest items of category 15 and potions or bottles of category 9, at 50 to 100 %, and 602 of chance 0.
A creature of a listed type drops the item in that per cent of the calls of its drop, in place of the call's random drops.
Picking up an item counts it for the quests that collect it (`0x4AAAE0`: a list at `+0x7620`, whose creature types are read through the drops' list as their items), finishing the quest at its count.

### Other drops

- The treasure `0x5516B0` (a chance `p` per mille, a quality range, a jitter): eight gold piles (each a forced drop of 5133) and two rolls of an item, each in `p` ‰ (halved after an item, doubled after none, the quality range halved each roll, at least 1): a category of a pool of 19 (weapons five of them, rings and amulets two each, the armour pieces, shields and wings one each), the type by `0x5C7FA0`, made at `L` plus `R(0, jitter)` with a key of the range, thrown with `10·R(−150, 150)` along `x` and `y`.
- A hit by a creature with the effect 856 (in the strike's resolution `0x56B130`, at `0x56F679`): in `rand() % 101` below its value (10 times the difficulty's factor more when its counts at `+0x4F8` hold more than four at the entry 30) the struck creature drops a gold pile (5132 in a case of two, else 5133 in a case of two, else 5134 or 5135), its amount held to the strike's damage and to a value of `0x549460` (not followed).
- A quest's reward (`0x4AC940`) gives gold, experience and items by its flags, and after a count of finished quests at the hero's `+0x2CD` passes `R(1, 56)` a piece of a set not 33 to 38 through the drop (not followed further).

## Picking up

Read 2026-10-25 (#911) by the static reading of `Sacred.exe`; **confirmed** by code unless marked otherwise.

### The click

- The world view's mouse handler `0x6172C0` takes a press of the left button and picks the object under the cursor with `0x626C50`.
  It looks among the rectangles the view drew this frame, entries of 28 bytes at the view's `+0x96B24`, and the pick gives up beyond 1,000 of them.
- Each object in a rectangle that holds the cursor answers its pick class, its virtual query 1:
  - an item (`cItemBase`, `0x5C70C0`) answers 2;
  - a living creature hostile to the local hero answers 0, any other creature 3 (`0x55D260`);
  - another 3D object answers 3 (`0x450930`).
- The classes are tried in the order 0, 1, 2, 3, or 2, 1, 0, 3 while Alt is held (the two tables at `0x95F934`).
  Within the first class that has a rectangle under the cursor, the rectangle whose centre lies nearest the cursor wins.
  So a hostile creature under the cursor wins over an item unless Alt is held, the key that also shows the items' labels.
- With Alt held and nothing picked, the last object's rectangle under the cursor is taken (`0x6270A0`).
- With an object under the cursor, the handler sends the sound manager the event `(9, 0x5C)` with the object's type (`0x6770E0`, `0x67A7A0`), by which a hireling or a named companion clicked says one of its click lines (read 2026-10-27, docs/re/sound.md, "The sources").
- When the cursor holds nothing (`0x655560`) and Shift is not held, an object that is not a creature, an item among them, gives the hero the event 7, with the object's id and the click's world point; with Shift held, a click on an item sends nothing.
- While the cursor holds an item, a click on the ground with nothing under the cursor calls `0x611E20` with the hero's position and the point (**hypothesis**: it drops the held item there).
- Holding the button (`0x60F130`, at every tick) repeats only a walk to the point under the cursor, every 0.4 s, and sends no event 7.
- The event 7 takes the AI's state 1 (`0x53A170`) with the object as its target `+0x104` (docs/re/fight.md, "Doors in the way").
- An item still in flight is picked where it is drawn, as are the labels (`0x627180` walks the same rectangles), and nothing on the way asks after the flight (`+0x14` bit `0x100000`).
  **Seen** 2026-10-31 in the running original (#1647; the Seraphim's start, gold piles thrown at her feet by the driver's `item`): with Alt held, a pile's label stood about 100 pixels over her feet 0.8 s after the throw and on the ground at 3.2 s; with the world's time stood 0.3 s after a throw, the cursor over the pile in the air, 80 pixels above its point on the ground, showed the hand and the pile's name, and a click there sent her to it and took it (`cCreature::pickupItem` with that pile).

### The walk

- In the state 1's sub-state 0, the target's category (`items.pak +0x2E`, `0x426220`) chooses the branch: every category but 2 and 3 (the creatures), 4 (the chests) and 10 (the doors) takes the items' branch.
- An item already out of the world (`+0x14` bit 1), or lying at a world point whose x or y is 0, ends the order (`0x54E6D0`, the idle).
- The signposts 4993 to 4999 (`0x43F6C0`) take the sub-state 8 instead (not read).
- A type that cannot be picked up is only walked to.
  A type may be picked up (`0x427920`) of the categories 5 to 9, 13 to 25, 28, 29 or 33, its record's first word without bit 22; the table's eight types with bit 22 are the signposts and `windstrauch.grn`, all of the category 15 (counted in `items.ron`).
- For an item it may pick up, the creature walks (`+0x150` 1) to the item's world point plus 25 along both x and y (`+0x158`, `+0x15C`), stopping within 50 (`+0x176`), and takes the sub-state 3.
  It walks on the item's layer, and where the item's cell lies under a building (`0x635240`), on a layer taken from that building's cell (**hypothesis**: the building's floor).
- A creature of `+0x200` bit 2 does not walk to it while the pick-up's clip plays (the bit not read).

### The take

- In the sub-state 3, once the creature stands (`+0x150` 0, `+0x241` 0), it takes the item if the item lies within its reach, else the order ends (the idle) without it:
  - the reach is 100 for an id up to 16 (the heroes) and 200 for a higher one;
  - the distance is the straight distance between the two world points, its integer square root rounded up;
  - a hero also needs a straight line of walkable cells to the item (`0x4FCC80`).
  The walk stops at most 50 + 25·√2, about 85, from the item, so a walk that reaches its point ends within a hero's reach.
- A creature without `+0x200` bit 6 takes the item at once, with no clip (**hypothesis**: the bit marks a player's hero): it leaves the world (`0x5FC8B0`) for the slot its type gives (`0x54ACC0`, `sdk_EQUIP`).
- A hero turns to the item (`0x44DD90`) and orders the motion 172, which the motions' names call `MOTION_PICKUP`, with the sub-mode `+0x152` `0x65` and the target `+0x184` (`0x53CC74`), and `+0x221` 0.3 over the product of the attack speed (`0x584FE0`) and `+0x2C9` in hundredths (**hypothesis**: as for the strikes, docs/re/fight.md, the time before the clip's end at which a next clip starts).
  The standing action plays the ordered slot `+0x154` at the attack speed times `+0x2C9` (1.0 but for the effect 15) times 0.01, times 1.8 where the slot is 172 and 1.0 for any other (`0x544BCC`; read 2026-10-27, #1255).
  **Seen** 2026-10-27 in the running original (the Seraphim's new game, quest 15001's plant picked up, a trace of `0x593320`): the slot 172 played at the blend 0.25, one loop and the rate 2.772, her attack speed 154 times 1.8, and the item was taken 0.40 s after the clip began.
  It plays it only while the setting `PICKUPANIM` holds (`0x81A690`, read from the configuration, on by default); else there is no clip.
- `cCreature::pickupItem` (`0x558080`, called at `0x53CEB0`) takes the item once half the clip has played, or at once without a clip or where the clip's times are not sane (longer than 10 s, or negative); an item of `+0x14` bit `0x10`, `0x1000` or 1 is not taken.
- The order ends (the AI's state 0) as the clip ends; a target kept at `+0x138`, with `+0x13C` 1234, then becomes the state 1's next target.
- In a network game (`0x182EBEC` set; **hypothesis** for the meaning) the hero checks for room before the clip, as below, and without room the item jumps (`0x5592B0`); the local player sends the network event `0x141` with the item, and at the clip's half the item only leaves the world, waiting for the server's answer.

### What becomes of the item (`0x558080`)

- It refuses an item that is gone, of `+0x14` bit `0x10`, or whose type has bit 22 (`0x43F650`) or is 0; else it clears the item's bit `0x20` of `+0x14` ("vanishing") and logs the pick-up.
- An item object first takes the picking creature's type's variant of its type (`0x5C7370`, "switchItem", `0x43CA20`; **hypothesis**: the class's own model of a piece), which is none for a hero, a type below 32 (above, and docs/re/equipment.md, "The item's model").
- The creatures of an id below 32 each have an inventory entry of `0x1180` bytes (`0x5DEBC0`, `0x5DEC90`; a log calls it `cInventoryEntry`), whose grid is `+0x115C` cells wide and `+0x115E` high.
- Every branch that takes the item for a hero runs a script named by the item where that name begins with `Take:`: the rest of the name is the function, called with the taker (`0x4A71F0`, `0x46BF40`).
- The branches, in this order:
  1. **A quest's item** (`0x4AAAE0` non-zero, unless `+0x14` bit `0x4000` is set) goes into the entry's list of the main quest's items (`0x5D8280`, pairs of a type and a key); a type the list holds already is only logged, and the item is destroyed either way (`sdk_DESTROY`), with the sound event `(9, 0x81)`; a creature without an entry does not take it.
  2. Else the item takes `+0x14` bit `0x4000`, so that it counts once, and **the element stones** go the same way into that list (`0x428680`: the first word's bit 24 with `+0x61`, `+0x62` or `+0x1C` set, the ten `Elementstein01.grn` types 6008 to 6017).
  3. Unless the item has `+0x14` bit `0x800` (not read), the rest follow.
  4. **Gold**, the four piles 5132 to 5135 (`0x43AF50`, the table at `0x949920`): the pile's amount (the item's query 3) is added to the creature's gold (`0x57FD80`) and the pile is destroyed, with no room needed; a kernel message of kind 6 with `0x3EC` and the amount follows at the creature's place (**hypothesis**: the amount shown over the hero).
     The gold is the word `+0x46` of the stats (the creature's `+0x3EE`), held below `0x7FFFFFFF`; the handler `0x57FD80` (a `__thiscall` on the stats with the amount, less than 0 too) first runs the integrity test `0x590730` for the local hero and, when it fails, kills the hero for good instead (its health 0, the action 6, the global `0x182EE50` set): a gold written into the word directly fails it at its next change (**seen** 2026-10-25, #973: the hero left at 1 gold, never rising).
  5. **Into a hand or a slot**, for a creature of a hero's type (`0x426650`) that can use the item (`0x54ABE0`) and whose `+0x200` lacks `0x600000`: a weapon (category 5) into the hand its type gives (`0x54ACC0`) where both hands (`+0x1D4`, `+0x1D8`) are empty; any other item into the first of the slots 0 to 18 that its class has (`0x43F3B0`), that holds nothing or a cosmetic model (`0x426550`; the slots at `+0x1A4`), and that it fits (`0x43EFC0`; the slots' categories in docs/re/equipment.md, the torches 5632 and 5633 only the slot 12); it is equipped there (`sdk_EQUIP`), with the sound event `(9, 5)` and the item's category.
  6. **Onto the cursor**, for the local player whose entry has `+0x1162` bit 3 (`0x5D9060`; **hypothesis**: the inventory's window open) while the cursor holds nothing (`cMouse +0x68`, `0x655550`): the item goes onto the cursor (`0x655580`, drawn 32 pixels up and to the left), with the sound event `(9, 0x80)`.
  7. With bit `0x1000` of the global `0x182EE3C`, `0x557D60` uses or equips the item at once (a mode not followed).
  8. **The category 14** goes into the entry's list of 256 types at `+0xD5C` (`0x5DB030`) and is destroyed, with the sound event `(9, 0x82)`; no type of the table is of the category 14.
  9. **Into the inventory's grid**: the item's size in cells (`0x428D60`) is looked for in the grid (`0x5DABA0`), column by column from the left, each from the top, at the first place where it fits; the item clears `+0x14` bit `0x100000`, the scripts hear of it (`0x471790` with the code 2), and it leaves the world for that place (`0x5D9F00`), with the sound event `(9, 0x80)`.
  10. **Without room**: the sound event `(8, 3)` with the item's type, the hero's `CANTCARRYMORE` line (docs/re/sound.md, "About items"; read 2026-10-28, #1324), then `0x5592B0`.
- So in a game of one player the hero walks to the item and plays the clip even with the inventory full: the room is looked for only at the take.

### Without room (`0x5592B0`)

- The item stays in the world and jumps where it lies: it is put on the ground, then raised 40 to 70 above it (`rand()/32767·30 + 40`, cut to an integer), and flies by `0x44DED0` with a velocity of 0 along the ground and 100 upward, spinning at `R(−6, 6)` about each axis, with bit `0x8000000` at `+0xB0`; in a network game the network event `0x147` goes to the server too.

### The automatic pick-up (the AI's state 12)

- `cCreature::pickupItem` is also called from the AI's state 12 (`0x4EC560`, at `0x4ECF69` and `0x4ED095`) and from `0x536A3B` (not read).
- Without a target, the state 12 searches square rings of cells about the creature, out to 19 cells, for the first item that may be picked up, passes the filter `0x4EC330`, and fits the inventory, is gold, or is of a type of the list at `0xAB45A0` (entries of 76 bytes, not read); the creature then walks to the item's point, stopping within 20.
  At the clip's half it takes every such item on the 3 × 3 cells about itself, and as the clip ends it searches again; finding nothing it goes idle.
- Read 2026-10-28 (#1408, **confirmed** by code): the taskbar's Collect button is the hero's way into the state 12.
  Its command (`0x6E5BB7`, built with the bar) sends the local hero (`0x7D84A0`'s `+0x14`) the creature event `0x113`.
  The creature's receive (`0x52E590`, the seventh entry of `cCreature`'s table) takes the events `0x110` to `0x201` through the tables `0x536CF0` and `0x536CCC`, and sends `0x113` to `0x530C9F`: nothing in a network game (`0x182EBEC`), nor while the AI's state (`+0xFC`, read at `0x52E62B`) is 4, 6, 8, 10 or 11, or the action (`+0x150`) is 7; else the button is let up (the UI event `0x12` of `UI_TB_COLLECT` through the kernel, `0x553210`, `0x808E50`, `0x8092F0`, with 1 and 150; **hypothesis**: 150 ms on), and the hero takes the state 12 (`0x4EC560`), sub-state 0, with no target (`+0x104` 0).
  The sub-state 0 walks the rings of the radius 0 to 19 about the hero's cell, each ring by its rows from its least y to its greatest and in a row from the least x, on the hero's layer (its horse's while it rides), and takes the first object of a cell's chain that may be picked up (`0x427920`), passes the filter (`PICKUPAUTO`, 0 in the installation's `Settings.cfg`: every item) and fits the backpack (`0x5DABA0` with its size `0x428D60`), or is gold (`0x43AF50`), or of the list at `0xAB45A0`; the hero walks to its point, stopping within 20 (`+0x176`), then takes the sub-state 3; nothing found, it goes idle (`0x54E6D0(1, 0)`).
  The sub-state 3, standing, plays the pick-up (the action `0x65`), and at the clip's half (or at once without `PICKUPANIM`) takes every item that may be picked up and passes the filter on the 3 × 3 cells about the hero (`cCreature::pickupItem`, `0x558080`), once a clip (`+0x134`); nothing there, it goes idle; 0.2 s before the clip's end it goes back to the sub-state 0 and searches again.
- Read 2026-10-31 (#1646, **confirmed** by code): neither sub-state asks whether an item still flies, nor does `0x558080`, which refuses only an item of `+0x14` bit `0x10`, a type of bit 22 or the type 0.
  The flight (`0x44DED0`) sets the item's `+0x14` bit `0x100000` and clears `0x800000` and `0x200`, and the backpack's branch of `0x558080` clears `0x100000` as it takes the item.
  So the collecting finds the items that a death, a chest or a stash has just thrown, the hero starting its pick-up while they are in the air (the maintainer's report of the running original), and takes those still in flight at the clip's half out of the air.
- The filter `0x4EC330` reads the setting `PICKUPAUTO` (`0x81A4E0`): the element stones, the items of `+0x14` bit `0x800` and the types of the list at `0xAB45A0` always pass; with 1, gold and the runes (category 28) pass, of the potions (category 9) only the bottles 3999 and 5058 and the large potions 5137 to 5141, and the other items by a bar of quality that rises with the creature's `+0x3FE` at 10, 25 and 35, read from the item's magic record (`0x5BEB30`: the bits 16 to 19 of its `+0x04`, its bytes `+0x0C` to `+0x0E`); with 2 only gold; with any other value everything.
- The state 12 is taken at `0x548010` and at `0x4EAB07` (found by a scan of the stores to `+0xFC`): at `0x548010` an idle creature takes it without a target in one case of four, with `0x182EE3C` bit `0x1000` set and `0xAD3CE4` above 250; at `0x4EAB07` the state 13 hands its target on to it; the hero's own way into it is the Collect button's event, below.

## Open

The words 43, 46 and 49; what the quality pass makes of its map's value, and `0x5D1750`'s bonuses; the price `0x5D3AB0`; the bonus's other branch; whether `0x5796A0` adds the unconditional effects; the object's creation and the copy of a fixed item; `CreateNPC`'s second making of a rule's items; the thread that runs the scripts.
Of the drops: the boss switch's cases (`0x533E2E`) and the type 183's last item; the raising of a fixed item (`0x5C2210`); the counts at `+0x4F8`, the effect 42 and the art 55; the colour of an item's label; how long an item lies on the ground and whether it stays when its sector is left; what the network events of a drop carry; the receivers of the effects 856 and 868.
Of the picking up: who else takes the AI's state 12 (`0x548010`, the state 13 `0x4EA890` at `0x4EAB07`, the caller at `0x536A3B`), the options' screen `0x718660`, which reads `PICKUPANIM` and `PICKUPAUTO`, the list at `0xAB45A0`, the order of a cell's chain, and what the state 12 does where its walk cannot reach the item; `0x43F3B0`'s classes by type, `0x4AAAE0` as a test; the item's `+0x14` bits `0x800`, `0x1000` and `0x100000`, the creature's `+0x200` bits 2 and 6 and `+0x241`; the signposts' sub-state 8; the kernel message of kind 6; the network events `0x141`, `0x145` and `0x147` and the server's answer; the clip's rate (`0x584FE0`, `+0x2C9`); `0x5C7370`'s variants of the classes (`0x43CA20`).

## In `opensacred-game`

An NPC's items in its hands are made at its level by their templates (#309, `opensacred_rules::fight::Templates::make`): their damage, resistances, `A` and `D`, without the magic bonus, the triples, the quality pass, the difficulty's term (bronze's 0) or a fixed item's own values (#1367).

The drops' rules are `opensacred_rules::drops` (#414), which the game calls as a creature dies (#415, below): a death's calls and their bonus (`Rules::calls`), a call's chance and its passes with their tries, the bonus compounded by the creature's worth, the quality key by the weights of the tail of `Balance.bin` (`tables/balance.ron`, #413), the kind, the type by `0x5C7FA0`'s choice over the types of the items table, `tables/items.ron`'s slots and the types that `weapons.ron` derives from them with their roots' entries and the category of 4053 (`Catalogue`, #596), the item's levels `lo` and `hi`, gold's pile and amount, and the item's values by the maker at its level, by its key; the result is a list of drops (`drops::drops`), each with its kind, its type, its level, its key, the maker's key, its amount and its values.
The game keeps the creature that made the last jewel over the deaths (#610), so that a creature's drops made again by `drop` make one jewel, as the original's do.
The runes drop since #1183 (`tables/runes.ron`, the hero's runes those of its backpack, a rune's drop sounding `DROPRUNE` at the creature); the uniques, the set pieces, the bosses' drops by their type, the set's potions, the party's factor and the effects 856 and 868 are left out, the keys 0 and 1 falling back as the original's do where it finds no unique or set piece (#1367, #1262).
The scripts' drops are followed since #930 (`opensacred_rules::quests::Drops`, `drops::drops`): kept by the creature type and the item (docs/decisions/netscript.md, "Scripts' drops per creature and item"), each entry of the dead's type rolls its chance at each call, and the items that come are the call's type at each of its passes in place of its random ones, a quest item or another of no drop's kind lying as it is (`Kind::Other`).

The game makes them (#415, `drops`): as an NPC dies (the component `npcs::Dies` added), a creature hostile to the living hero, not of the types 71 or 75, drops on bronze, the hero's type, level and skills taken for its class and what it can wear, the time played that of the game since it started, the luck 100; each drop is its item's model with its texture (`items.pak +0x70`, `+0x08`), a derived type's its root's, thrown from the creature's place at the height 60 and flown by `opensacred_rules::drops::Flight` (`0x44DFE0`) over the heights a creature stands at (`terrain::Heights::standing`), its velocity along the ground turning back at every frame as the original's (#478), so that it rises and falls within a unit of the creature's place, the gold piles and the runes upright, until it rests, where it lies.
The command `state` lists the five drops nearest the hero, and `drop [TIMES]` makes the hostile creature nearest the hero drop as if killed (docs/10).
How long the drops lie is not followed (docs/decisions/items.md, "Drops lie until the game ends"); a rune's drop sounds since #1183, and an item's landing is silent in the original too.
The labels on Alt since #1409 (`labels`): while Alt is held, each item in view, lying or since #1647 still in flight, shows its name of the game's texts (as the hand's name beside the cursor, `cursors`), gold its amount and its pile's name, white on the panel `0xCF303030`, its left edge on the item's point and centred on it upright, in the game's font at 18 on a line of 16; where two would overlap the later, by the points from the top down, goes under the other, 4 apart (**estimate** of `cLabelLayout`, which is not read).
**Seen** 2026-10-29 in the original (the Seraphim's start, five items made by the driver's `item` at her feet, Alt held by the driver): the items' labels stacked downward 22 pixels apart, their left edges together, the gold's beside them as "17" and its word, the panels 18 high.
A left click on a label takes its item where no item lies near the cursor's line (`0x6270A0`), and with Alt held an item near the line or a label under the cursor comes before a foe (`attack::aim`).
Since #1647 an item still in flight is picked by a click, shows the hand and its name under the cursor, and is taken by the command `take`, where it flies, as the original's.
Not followed yet: the labels over the creatures and the objects, which the original shows too (the hero, the townsfolk, the foes in their colour, a chest), the items' colours (#1273), and the highlight of the label under the cursor.
The items that the maker makes go by the names it gives them since #1424 (`opensacred_rules::drops::name`, `drops::Tables::made_name`), as above: the generic name, then the composed one, of the texts that `global.ftl` holds, the roll of one in two drawn as the item is made (`Dropped::named`) and the effects' records read from the balance's image; the hand's name and the labels show it, a script's text name before it.
Every made item is of the quality 0, `st_`, as the game follows neither the magic bonus nor the quality pass that would raise it (#1367), nor the numbered names of the mage's staffs.
The glitter since #1382 (`glitter`, `opensacred_rules::particles::glitter`): every item of a type of the bit 19 (`drops::Tables::glitters`), lying or flying as it is thrown (#1477; the original's, **seen** by the maintainer, glitters through its flight), draws the 8 sparks of `DETAILLEVEL` 2 as above, the glitter's space two of the world's units to one (the inverse of the draw's halving; **seen** beside the original's at the zoom 0.5, the sparks of the same size and spread), its phase a hundred milliseconds a step of the item's entity's index (**hypothesis** for the object's id), not tinted by the night; the objects of `+0x14` bit `0x80000` that are no items are not followed.
The hero picks the drops up since #911 (`pickup`): a press of the left button that goes to no attack, on an item lying under the cursor, before a door, sends the hero to the item's point and 25 along both axes; standing within 100 of the item, with a straight line of walkable cells to it, the hero takes it, gold into its purse and every other item into its pack, else it gives up; the jump without room (`0x5592B0`) is not followed (#1088).
Since #1601 an item that the hero may use goes on in the first empty slot of its class that takes it before the cursor and the grid, as the branch 5 above, with its category's sound (`opensacred_rules::equipment::slot_on_pickup`, `pickup::Pack::wear_picked`); the hero's values and look made anew with it; a weapon into the empty hands is not followed (#1608), nor the bits `0x600000` of `+0x200`.
**Seen** in the game: the Seraphim collected three rings 4876 and the helmet 3005 thrown beside her, the rings into her slots 8 and 9 and the helmet into 0, the third ring into her backpack.
The bar's Collect button and the command `collect` send the hero collecting since #1408 (`pickup::Collects`), and the key A since #1591 (`controls`' `collect`, the open console taking it), unless it fights, shoots, makes an art or dies: it looks for the first item it may carry (gold, a quest's item the scripts laid, the weapon it hurled, or one for which the backpack has room) by the rings 0 to 19 about its cell in the original's order, the one nearest it where a cell holds more (**hypothesis** for the cell's chain), on its layer (an item whose height lies within 20 of the height its level gives at the item's point, as the items at rest keep no layer of their own; an item in flight by the height at which its flight's level lets it land); it walks there on its level, stopping within 20, plays its pick-up at its rate, takes every item of the 3 × 3 cells about it at the clip's half, and looks again 0.2 s of the game before the clip's end; nothing found, or nothing about it at the half, it goes idle, and a click, a fight or an art ends it.
Since #1646 the items still in flight count as those at rest, as the original's do: the key A pressed as a creature's drops fly sends the hero to them at once, and those still in the air at the clip's half are taken out of it.
**Seen** in the game: the Seraphim in the start's garden collected a pile of gold, a healing potion, a sword and a heavy crossbow thrown within 19 cells, one after another, and left a pile 30 cells away.
The button's pressed image and its 150 ms are not followed.

## Defects the community reports

Checked for #357 on the GOG 2.28 files and the client on 2026-10-20 (Ghidra, read in prose); the reports themselves are **community-reported** (the changelog of Sacred ReBorn, SacredWiki), and the items are named by their records and the sets by their numbers, which the issue's names map to in its order.

The item record, the item's `+0x118`, as far as these checks need it, **confirmed** by code (`0x5D5080`, `0x5BEB30`, `0x5D0600`, `0x568D70`, `0x42BF40`); a `Weapon.pak` record's `+0x80…+0x101` holds one in the same layout, and every fixed item's bonus slots fit it:

- `+0x00` the type; `+0x04` the flags: bits 0–5, 22 and 23 the hero classes, 8–15 the template's word 38, 16–19 the quality (15 on every unique, which `0x42BF40` keeps), 24–27 the difficulty plus one and bit 28, both written by the maker; `+0x08` the price;
- `+0x0C…+0x13` eight sockets; `+0x19` the level; `+0x1A`, `+0x1B` a skill and its level; `+0x1C…+0x23` each kind's least damage, `+0x24…+0x2B` its most; `+0x2C` `A`, `+0x2E` `D`; `+0x32…+0x39` the four resistances;
- eight bonus slots: a condition word from `+0x3A` (its low byte an opponent's class, bits 13–15 what the bonus adds to: the damage, the resistance or another multiplier), an effect from `+0x4A` (the low 16 bits the effect, byte 2 the attribute that scales it) and a value from `+0x6A`.

The reports:

- **Six sets whose last bonus doubles all damage**: **confirmed** by code, and not in the data.
  `bin/sets.bin` holds a set's pieces, the key of its name, its count and its number, and no bonus; the pieces' records hold their own bonuses, none of them on all damage.
  The set bonuses are compiled into the client: `0x5711F0` counts the worn pieces of each set by the number that the loader `0x42C3E0` writes to each piece's `Weapon.pak +0x23`, and `0x571710` applies them in a switch over the set's number.
  The last bonus of each of the six sets sets one bit of the stats' flags `+0x148` (the creature's `+0x4F0`): set 39's (at six pieces) `0x2`, 41's (six) `0x400`, 44's (five) `0x40`, 45's (five) `0x200`, 47's (five of its six) `0x100`, 49's (six) `0x80`; no other set sets these bits, and set 39's also starts a weather event in the recomputation `0x5796A0`.
  The strike's resolution `0x56B130` multiplies the whole damage of a strike by 1.0, 1.34, 1.67, 2.0 or 2.34 by the difficulty `0x182EE44` when the attacker's flags hold any bit of `0x7C2`, which are exactly these six: 2.0 on the fourth difficulty and 2.34 on the fifth, where the community reports up to 2.
  There is nothing to repair in the tables: when the game implements the set bonuses, keeping or dropping this factor is a decision that changes the balance, so it waits for the maintainer.
- **Eleven rings that lose their level, and a twelfth always of level 1**: **not found in the data**.
  The records are 7694, 4930, 7983, 7463, 4915, 7290, 7472, 7497, 7320, 7357 and 7358 (a second record of that ring's name, 7397, is not of quality 15), in the issue's order, and 7511, the twelfth.
  A ring carries no level of its own: all 400 ring records (those whose root by the parents is an item of category 8) hold 1 at `+0x14` and at `+0x19`, and the item maker writes the level it makes an item at to `+0x19`.
  The twelve are uniques of quality 15, as are 72 other rings; comparing their 258-byte records byte by byte, their bonus effects and conditions, their parents and the templates that the lookup `0x42CA70` finds for them (each finds one: by its parent for the ids within 6501–7998, by its own id for 4915 and 4930), finds nothing that the twelve share and the other unique rings lack.
  The template of 7511, 571, raises a level made below 26 to 26 (its word 39, 21, over `MinLevelFactor`, 0.8), so a 7511 of level 1 did not come from the maker but keeps the record as `Weapon.pak` holds it (**hypothesis**: a path that makes a fixed item without the maker, or the reload of a saved one).
  ReBorn's change to the table is not known; without it there is no repair to describe.
- **Potions of concentration whose amounts do not differ by size**: **not in the data**.
  The records 5172 (lesser), 5146 and 5141 (strong), like every potion's, hold only the type, the flags and the price (400, 800 and 1,200): no bonus slot and no amount; the balance has durations by size for two other kinds of potion (`BluePotion…Dur` and `BlackPotion…Dur`, rows of five values) and nothing for this one.
  The amounts are the client's: drinking one (`cPotion3D`, `0x5CAA60`) sends the drinker a creature event of the kind 10 with the value 25, 50 or 100 by the type, as the viper's antidotes send the kind 8 with the same three (**confirmed** by code).
  Why tests find no difference lies in the receiver of the event 10 (**hypothesis**; not read); there is nothing to repair in the tables.
- **The chance of open wounds that works only as a set bonus**: **not in the data**; the item's bonus and the set's are both well formed and show the same text, the resource 1151.
  On an item it is the effect 857 (`0x359`) in a bonus slot, whose line the bonus text `0x55E5E0` writes with that resource; 76 `Weapon.pak` records carry it (75 without a condition), the balance image has its record like any effect's, and no template rolls it, so only fixed items have it.
  A set's is compiled into `0x571710`: the sets 31 (at five pieces), 40 (two), 48 (four) and 60 (three) show it.
  The recomputation `0x5796A0` only marks a creature that wears the effect 857, with nineteen other effects, by bit `0x40` of its `+0x2B7`, and neither the attack record `0x568D70` nor the strike's resolution `0x56B130` names `0x359` (**hypothesis**: the strike reads only the sets' chance, so the item's bonus does nothing, as the community reports).
  A matter of the engine; a converter repair would have nothing to change.
- **The wood elf's plain body armours of one mesh** (the comment of 2026-10-20 on the issue): **not a defect**, **confirmed** on the data.
  Items 3764–3771 name the models `elve_krotenhaut`, `elve_KurzHemd`, `elve_leather_01`–`03`, `elve_metal_01`, `elve_metal_02` and `elve_Tuch` (`items.pak +0x70` 157–163 and 188), and each its own texture at `items.pak +0x08`: the `texture.pak` records 6556, 6559, 6562, 6565, 6580, 6573, 6574 and 6569, eight different images named after the Amazons' armours.
  Eight more items of the same models (3511, 3513, 3532, 3539, 3541, 3598, 3602, 3606) name eight more textures (6576, 6578, 6560, 6582, 6570, 6558, 6564 and 6567), and most items of the `AMAZONE_*` models (3080–3088, 3514–3516, 3531, 3540) textures of the same family; the sixteen images differ, by the converted files' hashes.
  Her arm greaves, belts, boots and leg plates are told apart the same way: six items share each of the models `elve_s_arms`, `elve_s_belt`, `elve_s_boots` and `elve_s_legs`, with six textures.
  So her body armours share a mesh and differ by the item's texture, which the game draws (docs/re/equipment.md); **confirmed** 2026-10-25 in the original (#227, check 23): the eight put on the Wood Elf in turn by the driver's `wear` each looked their own.
