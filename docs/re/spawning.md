# Spawning: how the world is peopled

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra 12.1.4 on 2026-10-20 (#215).
The record formats are in [docs/02 §7.3](../02-data-formats.md#73-the-opcode-space-141-and-113); the record walker in [docs/re/netscript.md](netscript.md).
Status: **confirmed** on the code and on the trees' listings unless marked otherwise.

## The sectors' functions

Every NetScript tree has a function `Sector<x><yyy>Init` and a function `Sector<x><yyy>Enter` for nearly every sector of the world (5,694 and 5,720 of them, the same in every tree; the engine builds the names with `Sector%d%3.3dInit` and `…Enter`, `…Exit`, strings at `0x94F088`, `0x94F0C0`, `0x94F0F8`; no tree has an `Exit` function).
48 of them are named another way, `Sector28Enter` for sector 0,28, which the engine's format names `Sector0028Enter`; **hypothesis**: they never run.

When the sector at the centre of the view changes (`0x6288F0`, `0x62A5C0`, both calling `0x6263B0`), the engine walks that sector and its eight neighbours:

- a sector whose record (`+0x30`) lacks bit `0x400` gets it and runs its `Init` function, once (`0x626130`, `0x493680`);
- a sector that comes into the 3 × 3 runs its `Enter` function (`0x6261A0`, `0x4939F0`, which sets bit `0x800` and posts a weather event);
- a sector that leaves it runs the exit handler (`0x6262A0`, `0x4954C0`).

While they run, the script's current sector is in the globals `0x94E08C` and `0x94E090`, the interpreter's byte `+0x13` is set during an `Init` and `+0x12` during an `Enter`, the ten values of `SpawnValues` (`0xAB785C`) are cleared before each, and the `MakeGroup` rules (`0xAB7850`) before each `Enter`.

What they hold:

- `Init`: the sector's `SpawnValues`, its placed monsters (1,053 `CreateNPC` records, 909 of them hostile with tag `0x08`), which persist, and closed triggers with their cells (`CreateTrigger`, `TriggerPatch`, docs/re/movement.md, "The chests' cells");
- `Enter`: the sector's `SpawnValues`, its villagers and animals (1,764 `CreateNPC` records and `CreateObj` records), made again at every `Enter`, its `MakeGroup` rules and its `FillSector` records.

The creature maker `0x480D40`, which `CreateNPC` and `FillSector` share, marks a creature made during an `Enter` with `+0x2B8` bit `0x80` and `+0x14` bit `0x80000000`.
The exit handler destroys them, keeping the living ones first as records of `0x30` bytes in a list at the sector record's `+0x5A0` (the time at `+0x59C`); at the next `Enter`, `0x4939F0` puts them back and `FillSector` returns at once, while the region last filled (`0x94E080`) is the same; a change of region drops the other regions' lists (**hypothesis**: so that they are filled anew).
A sector entered without such a list gets crabs and fish (the types 163 to 165) in its waters (below, "The waters' crabs and fish").

`FillSector` has a gate for its respawn (at `0x47CC1E`): the balance variable `Respawn` (300) or `DungeonRespawn` (1,000) against the clock in seconds, keeping the clock in milliseconds at `+0x9A` of the sector's information; it compares the clock with the constant alone, not with the time kept (**hypothesis**: a slip in the original's expression), so it passes and `FillSector` fills at every `Enter`.

## The sector's level

`SpawnValues` (type 100, `0x4A9670`), the first record of every sector function, sets three words of the sector's information (`0x63D010`, the world record's `+0x17C`): `+0xD1` a density in percent (50 in every record), `+0xD3` and `+0xD5` the lowest and the highest level of the sector's creatures (1 to 45, 4 to 80).
`CreateNPC`, `CreateObj`, `0x564D60` and the message of an area's danger (`0x622D70`) read them.
The difficulty adds to them from the balance table (its image's `+0x788` and `+0x98C`, indexed 0 to 3 for the difficulties 1 to 4, **hypothesis** for the order): 0, 35, 70 and 128 to the lowest, 50, 120, 190 and 250 to the highest.
A creature's level is the hero's (`+0x3FE`) held within those bounds.
`SpawnValues` writes the information of the sector whose function runs (the globals `0x94E08C`, `0x94E090`); a sector's `Init` and `Enter` give the same values, in all 5,666 sectors of the Seraphim's tree that have them, whether the `Enter` has a `FillSector` or not.

### `CreateNPC`'s level

`CreateNPC` (`0x482510`) gives its creature a level by the record's level tag `0x1F`, the last of its tags, `T` (`0x4843C2` to `0x484761`, read 2026-10-22, #530, #540):

- `T` of 1000 (`0x4843C9`) makes the creature a boss (`+0x1F4` bit `0x80000`, docs/re/fight.md, "Bosses and champions") of the marker level 1, set by the level's setter without its scaling (`0x549500`), which its first fight levels (`0x550E50`);
- any other `T` above 0 (`0x4843F1`, compared without its sign; 1 to 165 in the scripts) is given to the level's setter with its scaling (below), without the hero's level, the sector's lowest or the time played;
- `T` of 0, or no tag, gives it its level so:

1. `L` is the local hero's level, in a network game the highest of the party's;
2. where the sector of the creature's position (its `+0x18`, `0x63CFC0`) has its information with both `+0xD3` and `+0xD5` above 0, `L` is held within them, each raised by the difficulty's addition: their lowest below them, their highest above, and between them `L + rand() % 2`, the hero's level or one more (so a hero at the highest makes one of one more, which the setter holds at the highest);
3. a horse (`Creature.pak` class 4, `0x4266F0`) takes that plus an eighth of the level of the hero's skill 22 (`0x582450`; **hypothesis**: riding);
4. any other creature takes it through the difficulty's scaling of the levels (`0x552230`): `⌊L·(1 + (P + x)/100) + B + 0.5⌋`, at least 1, with `P` and `B` the difficulty's `OffLevelProz` and `BaseOffLevel` (0 and 0 on bronze, docs/re/fight.md) and `x` the tenth of the curve `0x565D70` of the minutes played `m` (the statistics' counter `+0x570C` over 1,000 and over 60, a **hypothesis** as in docs/re/items.md), `100·(1 − 1/(1 + (m − 1)/120))` from a minute on, truncated: 0 below 15 minutes, 1 from 15, 5 from 121, never 10.
   A hero whose count of `+0x4F8` at `0x1C` is 6 or more (**hypothesis**: the pieces of a set worn) adds 1 to 2.34 to `x` by the difficulty before it is truncated;
5. the level's setter `0x564D60` takes it without its scaling.

The level's setter `0x564D60` (`0x565326` to `0x565523`, read 2026-10-22, #540) takes a level, and two flags that turn off its scaling and its hold; for an NPC (an id above 16):

1. a level of 0 becomes 1 unless the creature is a boss or a champion;
2. its scaling, unless the creature is a boss or a champion set to 1 or 1000: the level plus the difficulty's `OffLevel` (the balance image's `+0x788`, 0 in bronze's column) through the difficulty's scaling `0x552230` with no time played, plus the global `0xAB4680`, which nothing writes, for one neither a boss nor a champion; on bronze the level itself;
3. its hold: at most the difficulty's addition to the highest (`+0x98C`, `LevelKap`, 0 in bronze's column) plus the highest of the sector's information at the creature's position (`+0xD5`, `0x63D0C0`) where that is above 0, else that addition alone; 300 for the type 701; for a boss or a champion that plus `LevelUpBoss` or `LevelUpChamp`, and no hold for one of a big type (`Creature.pak` flag `0x02`);
4. a horse instead keeps the level it was given, at most 250.

The creature maker `0x480D40`, which `CreateNPC` and `FillSector` share, sets the level it is given through `0x549500` (at `0x4812F8`), the setter without its scaling; `FillSector` sets its bosses' and champions' marker level 1 likewise (`0x47E6FE`…`0x47EA52`).
So on bronze a tag is its creature's level up to its sector's highest, and a creature of the rule above or of a fill is at most at its sector's highest; where the sector's information has no highest the hold is 0 on bronze, which no tagged record's sector of the scripts meets, and 16 untagged records of the Seraphim's tree stand in such sectors (8 of the start script's at 4877,2905 in 76,45, 2 of a quest's in 25,95, 6 of the `Enter` of 83,54, which has no `SpawnValues`).
**Confirmed** in the original (2026-10-23, a fresh start of the Seraphim on bronze): the start script's garrison of 52,39 (1 to 4) with the tag 5, the four 288 and the 295 of 3383 to 3388, 2524 to 2529, the captain 297 of 3349,2534 and the 295 of 3347,2535, is at level 4; by the code the 692 of 60,75 (6 to 10) with the tag 6 is at 6.
**Confirmed** too, the hold of the rule's level: the `Enter`'s untagged 297 of 3378,2517 in that sector is at level 2 with the hero at level 1, and made again at 4 with the hero at level 4 and at 4, held, with the hero at level 6.

The record's tag `0x90` (the stables' horses', 1 to 9) raises the creature's level to its value earlier in the handler and is kept in its byte `+0x400`; the rule above sets the level again after it.
The start script's creatures take the bounds of their sectors however far from the hero's start, so their sectors' information holds the values by then (which code writes it first is not read).
**Confirmed** in the original's fresh start of the Seraphim, the hero at level 1 (#530): the blacksmith 688 and the 692 of the start script in 50,44 (4 to 7), a sector without a `FillSector`, are at level 4, and the trader 656 of the start script in 64,41 (29 to 45) at 29; in the start's sector 50,39 (1 to 4), where a teleport away and back left two of each of an `Enter`'s novices 677 and chicken 560 on the same cells, one of each pair is at level 1 and the other at 2.

### `SetNPCState`'s level

`SetNPCState` (type 3, the handler `0x48BB40`, which applies its tags through `0x461540`; read 2026-10-23, #541) sets the level of the creature its `01` names by its level tag `0x1F` as `CreateNPC`'s tag does: 1000 makes it a boss of the marker level 1 (`+0x1F4 |= 0x80000`, its bit `0x200` cleared, `0x549500(1, 0)`), any other level goes to the level's setter with its scaling and its hold (`(L, 0, 0)`, at `0x46213D`): on bronze a horse takes it as it is, at most 250, and any other creature at most its sector's highest.
The class trees and `netscriptcamp` have 156 `SetNPCState` records with a level tag, 89 of them of 1000, the others of 1 to 35 (`netscript` 156 and 86, the add-on's trees 79 and 11, up to 40); in the Seraphim's tree the start script has 62, the levels 1 to 25 of the stables' horses and a few others', each after the horse's `CreateNPC` (its tag `0x90` and the rule) and `EquipNPC`, and the quests' `OnEnter` functions 2 of 1000.
So a stable's horse takes its level last from its `SetNPCState`, and one whose `SetNPCState` has no level keeps the rule's.
**Confirmed** in the original (2026-10-23, a fresh start of the Seraphim on bronze): the horses of the stable of 64,41, 551, 550, 558, 552, 555 and 554, are at 13, 16, 19, 25, 10 and 22, their `SetNPCState`s' levels, by the trader 656 at the rule's 29.

## `FillSector`

`FillSector` (type 51, `0x47C610`) makes creatures at random over a sector when its record runs:

1. the density: the record's `35:(a, b, area)` is `a` to `b` creatures per `area` cells (4,096, a sector, in every record), each raised by the sector's density (50 %, 25 % for the lowest when `a = b`); a hero at or below the sector's lowest level gets the lowest, at or above its highest the highest, between them a random one;
2. times the balance variable `SpawnAnimalsSP` (0.8) when the record's first type's `Creature.pak` class is 3, 4 or 6, else `SpawnEnemysSP` (1.2) (the network game's `…MP` by the players' count);
3. the count: the area's cells times the density over `area`, at least 1;
4. each creature a type at random by the weights of the record's `19:(type, weight, hostile)` entries (a bare `02:type` weighs 100 and is peaceful; `08` and `0E` make every entry hostile or peaceful): `r` at random below their sum, then the first entry whose weights so far reach `r`, which gives the first one more chance than its weight;
5. whether it leads a pack (read 2026-10-20, #229, `0x47D755` to `0x47DD6B`): three counters of the run, at 0 when it starts, each count down the creatures of their kinds, and the one that finds its counter spent leads and sets it to its slots: the counter of the classes 9 and 14 (the thieves, the black magicians, the priests, the prince's guards, the pirates, some demons and spiders) with three or four slots, that of the dark and the ice elves (319 to 321, 327, 140, 141; four to eight) and of the goblin warriors (76, 118; four or five), and that of the wolves (512, 568 to 570; two to four); the elves' champions (326, 146; four to eight), the wolf 567 (three or four) and the types 37, 43, 48, 67, 258, 342, 343, 349 and 212 (three to six) always lead; in a dungeon (`0x63D1C0`, the dungeon test of the area's corner) the slots and the counter are halved;
6. a cell at random of the area, tried up to 50 times: one that may be walked on (`0x636C10`) with three more in a row after it along `+x`, `-x`, `+y` or `-y`, tried in that order (`0x45EF70`), whose flags' bit 1 is clear unless the creature's class has a low nibble of 0; a leader's also no nearer than 2,000 to an earlier leader of the run, a distance in cells (`0x47DEE5`), which no two cells of a sector are, so that a run places its first leader alone and drops the later ones; a creature that finds no cell is dropped;
7. then each creature that does not lead goes by the first leader of its class (`0x43ADC0`) with slots left, on one of 50 cells tried within two cells of it that may be walked on and that its flags allow, taking a slot and joining its group (`+0x249`, a group's number at random from 5,000 to 10,000, `0x4A9EF0`); else on one of 50 cells of the area that may be walked on; else it stays (`0x47E120` to `0x47E4C0`);
8. last (`0x47E551`), a quarter of the rabbits (516) become one of their variants 592 and 593, a sixth of those another of the three, and a ninth of the crows (513) their variants 589 and 590; the creature maker turns a fifth of the type 344 into 359, for `CreateNPC` too;
9. a hostile entry sets bit 0 of the creature's flags word (`+0x1F4`, docs/re/movement.md, "The objects in the way"), a peaceful one clears it, and the farm and wild animals take bit `0x20` of `+0x200` and bit `0x40` or `0x10` of `+0x1F4`; each faces a heading at random (`rand() % 361`);
10. at the `DETAILLEVEL` 0 (`0x182EE78`), 30 types of farm and wild animals (513 to 593: crows, bats, rabbits, pigs, cows, deer, bulls, sheep, chickens) are left out unless the record has `08`.

`34:(x, y)` names the record's sector, always its own function's; `33:(lowest, highest)` a level range, which the sector's `SpawnValues` overrides (always, in the files); `36:n`, 100 for the animals' records and 310 for the monsters', goes to the creature's `+0x255`, whose one reader, a branch of the group's alarm (`0x46AE70`), none of its callers takes: it serves nothing in 2.28.

A group's alarm (`0x46AE70`, called at `0x54EB19`, `0x5337CE`, `0x535DD3` and `0x5442DC`; **hypothesis**: as a member is hit or alerted) sends the group's other idle members to the alerter's target (the creature event `0x100`) or wakes them.
A run's creatures share one level, which `FillSector` takes before its loop over them (from `0x47D187`, read 2026-10-23, #582): the hero's, raised to the sector's lowest when below it (to the highest where the lowest is not below it), but not held at the highest; moved once by a roll of its band, `rand() % 2` below 6, then two rolls averaged, `(r % 4 + r % 4)/2 − 1` below 30 (0 in 7 cases of 16), `(r % 6 + r % 6)/2 − 2` below 90 and `(r % 10 + r % 10)/2 − 3` below 250, none from 250; then raised by the time played through the difficulty's scaling `0x552230`, as `CreateNPC`'s rule, and at most 300, the level of the run's items too (docs/re/items.md).
Each creature that its rank's roll leaves common takes that level through the level's setter (`0x47E7FE`, `(L, 1, 0)`), which holds it at the sector's highest ("`CreateNPC`'s level"); on a dungeon's sector (`0x63D260`, the sector of the creature's position, as `0x63D1C0`'s) it takes one or two more by a roll of its own after its rank's (`0x47E7C0`…`0x47E7F7`; #570), held so too.
**Confirmed** in the original (2026-10-23, the setter's calls from `FillSector` probed, #582): one level a run, 14 bandits 279 at 24, the thieves 307 of two runs at 24 and 25, each run of rabbits at one of 24 to 27 in sectors of 25 to 38, the ranks' creatures at the marker level 1.
Some types and about 2 % of the creatures hostile to a living hero are made champions (`HarteSau`, 20 in 1,001), the types 37, 48, 55 and 84 a boss or a champion by a coin, 127 and 49 so in one case of two; they are made at the level 1, which their first fight raises (docs/re/fight.md, "Bosses and champions").
A black rider's horse (261) brings its rider (256), hostile, a boss, with a short bow or a tribal axe.

## `MakeGroup`

`MakeGroup` (type 115, `0x4AB940`) is not a group of creatures but a rule of their equipment: its record adds an entry of `0xA0` bytes, up to 20 creature types (`02`), three items each by a pool and a chance in percent (`87`, `88`, `89`), a byte for the item maker (`8B`), a name (`1D`, `1E` or `16`) and up to ten spells (`67`), of which a creature takes one at random (`+0x2B6`, `+0x261`).
Its one reader is `0x47F8E0`, which `FillSector` calls for each creature it makes of those types; the items come from the start script's `Waffenpool` pools.

Read 2026-10-20 (#228):

- `Waffenpool` (type 114, inline in the record walker at `0x475F1A`) adds its record's last `02` item to its `0B` pool (0 to 254) of the interpreter (`+0x448 + pool · 0xC`), duplicates kept, so a repeated item weighs more; the Seraphim's start script makes 114 pools of 1 to 32 items (706 weapons, 158 pieces of armour, 38 shields, 2 helmets, 2 arm pieces) by 906 records, the same in every tree.
- A rule's slots default to the pool −1 for the first and the third, 0 for the second, all of chance 0.
- For a creature of the first rule in the sector's order that names its type (the rules of the `Enter` that runs; a later rule for the same type gives nothing), `0x47F8E0` rolls each of the three slots in order, `rand() % 100` below its chance, and takes an item of its pool at random among its entries; after a matching rule, the creature takes no item of the general maker even with all its chances 0.
- The first item: a crossbow or a musket (kinds 12 and 14, `0x427E80`) whose wielder's model lacks the motion slot `0x8B` is replaced by a random weapon of kind 1, or 7 where the model has the slot `0x72` (`0x5C7FA0`); some types have lists of their own (`0xBB`, `0xCC` to `0xD0`, `0xFD` to `0xFF`, `0x14A` to `0x14C`, 39, 40, 49, 51, 202, 344, 359), and every other type's items 1800 to 1804 become 1723, 1720, 1759, 1729 and 1752.
- The second is made only after a first item that is a weapon of a kind other than 2, 3, 4, 7 and 12.
- The third, a worn piece, goes through the armour switch (`0x43CA20`, the lists of `Rust.bin`, `tables/armour_switches.ron`), by the creature's type folded (304 to 306 as 314, 307 and 308 as 313, and others): the type's piece where the item's list names it, nothing where the list lacks the type, the item itself where it has no list.
- The creature maker equips them in order as `CreateNPC`'s items (`0x54B130`, the hand by `0x54ACC0`), and an item left without a place is destroyed; `8B`, passed to the items' maker (`0x5D5080`), is never read (**hypothesis**: it does nothing in 2.28).
- `FillSector` gives each creature one spell at random of every rule that names its type, the last rule's counting, and the rule's name (`0x47EFE7` to `0x47F0E2`).
- `CreateNPC` takes items by the rules too when its record has an operand `54` of 0 or more (23 records of the quest functions), by the rules the last `Enter` left.

About the start, the thieves of 48,37, 48,38, 49,37 and 50,37 take a bow, half of them a shirt; those of 48,39 to 48,41 a sword or a bow, the swordsmen sometimes a round shield, and the black magicians a staff.

## The waters' crabs and fish

Status: **confirmed** by code (`0x4939F0`, read 2026-10-23, #575) unless marked otherwise.
After the sector's `Enter` function, the runner looks at the sector's list of kept creatures: with none, or more than 4,999, or a flag at `0x182EBEC` set (not read), it puts crabs and fish in the sector's waters; else it puts the kept ones back.
A sector with no `Enter` function does the same.

- None in a sector one of whose two liquid materials (keyx `+0x2E0`, `+0x2E1`, docs/02 §5.1) is 4, 5 or 6, `A_LAVA`, `B_LAVA` or `C_LAVA`.
- It walks every other row of the sector's 64, from the row 0, 1 or 2 at random (`rand() % 3`), and in each row every other cell, from the cell 0, 1 or 2 drawn anew.
- A cell fits when its nature is a liquid's (`0x90` or `0xA0`) and its four first corner heights (`+0x10`, its bed) all lie between −14 and −2, exclusive: −5 or −10, the heights being multiples of 5 (`0x47C5B0`, which takes the two bounds).
- A fitting cell takes a creature in 11 (`rand() % 11`) when its eight neighbours are liquid cells whose beds lie between −25 and −2 and fewer than 14 of the 16 cells two away are: a shallow cell near a shore, none in open water.
  The cells about it may lie in the next sectors: the position names the global cell, and the world loads their sectors if it must (`0x634340`).
- The area of the sector (keyx `+0x2C0`, through the sector information's `+0xD7`) and the dungeon test of the cell's sector (`0x63D260`, `0x622E20`) decide the type.
  In a dungeon out of the areas 13, 16, 17 and 55 it makes none.
  Otherwise it makes a 164 in three (`rand() % 3`), else a 165, after one more `rand()` whose number is not used.
  In those four areas it makes a crab, the type 163, in their dungeons always and elsewhere in three.
- The object maker `0x5FBA40` makes it on the ground at the cell's centre, `(cell + 0.5) · 53.66563` truncated, with the bits `0x90000000` of `+0x14`, bit 31 the mark of a creature made during an `Enter`, which the exit handler keeps.
  It faces `((s − 50) · 0.02, (r − 50) · 0.02)` on the grid, `r` and `s` two rolls of 0 to 100 (`0x44DD90`).
- The crab's type 163 is `items.pak`'s `krabbe.grn`, which no record of `Creature.pak` describes (#603, "A type without a record" below).
  The fish 164 and 165 are `fisc_01.grn` and `fisc_02.grn`, of class 6, the base 5, 10, 20, 10, 0, 3 and the skills 8 and 9; the 166 of `fisc_03.grn` is never made.

Seen in the original on 2026-10-23, at the Seraphim's start with its world paused, five fish:

- the 165s at 3182,2526 and 3170,2550 (49,39), at 3298,2465 (51,38) and at 3321,2526 (51,39);
- the 164 at 3325,2469 (51,38).

Each is at level 1 with 100/100 health, peaceful, its AI's state and action 0, and its attributes are its base.
Its `A` is 12.5 times 1.07 and its `D` 17 times 1.28: its two skill slots at level 1, as `0x586C00` makes the stats with no level's setter after it.
Its most health is the stats' own 100 (**hypothesis**: `0x5658F0` never runs for it).
The rule above fits their cells on the sector files, and also the 165 at 3398,2515 in 53,39 and the 164 and 165 at 3214,2827 and 3234,2843 in 50,44 seen earlier (#575).
Over the world, 27,618 cells of 700 sectors fit it (counted on the sector files 2026-10-23).
A quarter of the cells is visited, and one in 11 of those takes a creature, so the original makes about 630 crabs and fish when every sector comes in once with an empty list.
Whether they move is not read: the world stood paused when they were seen.

### A type without a record

Read 2026-10-23 (#603); **confirmed** by code and in the original unless marked otherwise.
The client finds a type's `Creature.pak` record through its items table: the word `+0x1A` of the type's 128-byte entry is the record's place in the table of `Creature.pak`'s records, as the file holds them (`+0x563008` of the items table, 86 bytes each), which the loader `0x4371C0` writes for the id of every record; `0x43AD80` gives the record, or none where the word is 0.
`items.pak` holds 0 there for every type (its 17,409 entries), so the type 163, which no record names, keeps 0 and the crab has none:

- its class is 0 (`0x43ADC0`), the side its constructor `0x52DE40` gives it, a friend of every side but 13 (docs/re/fight.md, "Sides"), so it is peaceful;
- the stats' maker `0x586C00` gives it the level 1, the base attributes 25, 15, 20, 20, 10 and 5 and no skill slot (its defaults for a type without a record, as a record gives a fish its base and its two skills);
- its speed is `0x4F73C0`'s, the default of `0x4F75F0` for a type whose record gives none: 100 walking and 200 running for the type 163, as for any type out of that function's list;
- the functions that take the record's place without `0x43AD80`'s test, as the shadow's `0x426D30` does, read the table's first record, of the id 0, whose flags are 0: the crab casts its shadow;
- its model `krabbe.grn` has no motion slot (its `ModelDesc`, docs/02 §4.5), so it plays no motion (**hypothesis** by the data: it stands as its model is made); the `KRAB_*`, `KRA1_*` and `KRA2_*` motions are the killer crabs' (`Killerkrabbe*.grn`, the types 209, 347 and 348, 20 of which stood about the hero in 50,9).
  Like a fish it takes its flags at its first update (docs/re/ai.md, by code): 8, the townsfolk's, being made by the waters' runner as by an `Enter` function, of class 0 and not hostile.

Seen in the original on 2026-10-23 under the driver of #448, the Seraphim at level 1 on bronze:

- in 50,9 (area 55, a dungeon by its keyx, docs/02 §5.1), 21 crabs and no fish, all crabs as in a coast's dungeon; in 44,67 (area 13, no dungeon) 5 crabs and 5 fish;
- each crab of the type 163, peaceful, at level 1 with 100/100 health, its attributes its base 25, 15, 20, 20, 10 and 5, `A` 22.5 and `D` 21, each times 1 (no skill), its attack speed 140, then 120 and 130, its least and most physical damage 8.375 and 11, its resistances 1.59, 1.18, 0.59 and 0.8 before their multipliers;
- with the world running, none of the five crabs within 40 cells of the hero in 44,67 left its cell over 40 s of play, the cells about them water that no creature may walk on.

Of the 163 sectors of the four areas whose waters fit the rule, 31 are dungeons, whose waters take crabs alone (counted on the sector files 2026-10-23).

## The start of the Seraphim

The nine sectors about the start (49 to 51, 38 to 40) are of the levels 1 to 4; their neighbours to the west and the north (48,37 to 48,41, 49 to 51,37) of 25 to 38.

- Their `Enter` functions' `FillSector`s make rabbits alone (`RABBIT.GRN`, two records of `35:(6, 6, 4096)` a sector: about ten a sector for a hero of level 1, a quarter of them of the variants 592 and 593), and place the start sector's novices, a chicken and a rabbit, three pigs in 51,38, and farmers, citizens and animals in 50,40.
- Their `Init` functions place four goblin warriors (`gob_warrior.grn`) in 51,39 and two thieves with their weapons in 50,40, hostile; the ring about them 21 goblins and 11 thieves more.
- Further west and north, the monsters' `FillSector`s make bandits, black magicians, flight lizards and wolves, about 14 hostile creatures a sector.

## In `opensacred-game`

The game takes the `CreateNPC`, `CreateObj`, `CreateTrigger` and `TriggerPatch` records of the sectors' `Init` and `Enter` functions with the start script's (`npcs`, #221), and with them those of the `OnSetUp` function of each quest that the start script sets up, after its `SetUpQuest`, as the client runs it there (#624, docs/re/netscript.md, "Quests"), and of the `OnEnter` function of each quest it enters: each NPC or object is placed as the sector of its cell comes in; as its sector goes, a creature of the start script or of an `Init` is kept where it stands with its health and its level, and comes back so, a killed one never, while an `Enter`'s villagers, animals and objects are made again at each coming; a fill's living creatures are kept with their sector and put back at its next coming, which fills anew only when none of them lives (`npcs::Kept`, #329; the region's rule is not followed, docs/decisions/2026-10-20-spawning.md); the placed monsters, hostile, stop the hero's steps (docs/re/movement.md).
In the Seraphim's tree that makes 3,281 NPCs and 3,123 objects over the world, against the start script's 320 and 1,990: the start sector's novices, chicken and rabbit, and the four goblins of 51,39 among them; the quests' `OnSetUp` functions give 144 of the NPCs and an object, their givers among them, such as quest 15004's by the garrison of 52,39, peaceful, staying (`0x74`) and with its dialogue (until #624 the game ran none of them).
Each `CreateNPC`'s creature takes its level by the rule above (#530, `opensacred_rules::fight::scripted_level`): the `SpawnValues` of its cell's sector, kept for every sector with a fill or without, its roll of one more by the game's generator seeded by its cell, so the same at each coming, a horse's eighth of the hero's riding none, as the hero's skills stay at level 1 (docs/decisions/2026-10-20-levels.md), and the time played that the drops count; a creature of the start script or of a quest it enters takes its level as the game starts, at the hero's first level and no time played, though the game makes it as its sector first comes in; the setter then holds it at its sector's highest, a horse's at 250 (#562, `fight::held_level`), and at 1 where the original's is 0 (docs/decisions/2026-10-23-level-zero.md).
A creature of a level tag takes it by the setter (#540, `fight::tagged_level`): held at its sector's highest, a horse's at 250; one of the tag 1000 is a boss, levelled as it is made at the hero's level then, as a fill's boss (docs/decisions/2026-10-20-bosses.md).
A creature that a `SetNPCState` of the start script or of a quest it enters names after its `CreateNPC` takes that `SetNPCState`'s level tag in place of its own (#541, `npcs::set_states`), by the same rule, and is a boss for 1000; of its other tags the game follows the dialogue, the flags word, the hostility, the trades and the home (#578, docs/re/netscript.md, "`SetNPCState`").
The `Enter` functions' `FillSector`s make their creatures as their sectors come in (`spawning`, #222, #229): the count by the sector's `SpawnValues` for the hero's level and the balance's `SpawnAnimalsSP` and `SpawnEnemysSP`, the types by their weights as the client picks them, the leaders by the run's counters, the cells by the steps 6 and 7 above (the first leader of a run alone, its kind gathered about it), the rabbits' and the crows' variants, the hostile ones with bit 0 of their flags word, the run's one level as above (#582, `fight::fill_level`, rolled after the count and before the creatures are made), each common creature's one or two more on a dungeon's sector by a roll after its rank's (#570) and held at the sector's highest (#562, `fight::filled_level`), and their bosses' and champions' by their ranks (docs/re/fight.md, "Bosses and champions").
The waters' crabs and fish (#575, `spawning::swimmers`) come in a sector that comes in anew, as its fills do, by the rule above with the game's generator seeded by the sector.
They are made once no sector is building, on the cells of the sectors loaded then; a cell whose ring reaches a sector not loaded takes none (docs/decisions/2026-10-23-waters-edges.md).
Each fish is made at level 1 with its two skill slots at level 1, 100 health, peaceful and with no behaviour, facing its way, and its sector keeps it as it keeps a fill's creatures.
A crab is made so too, its type a record the game makes as the original makes a type without one (#664, `opensacred_rules::fight::recordless`, "A type without a record" above): of class 0, the base attributes 25, 15, 20, 20, 10 and 5 with no skill, the speeds 100 and 200 and a shadow, standing with no clip; in 44,67 a crab of the game shows the original's level, health, `A`, `D`, damage, resistances and attack speed (`stats`).
They hold the first two items of the first `MakeGroup` rule of the sector that names their type (#228), rolled by the rule's chances from the start script's pools, in the hands `npcs` gives the scripts' NPCs, with the clips of what they hold; the third, a worn piece, the crossbows' replacement for the models without their slot, the types' own lists and the spells are not followed.
About the start that makes some ten rabbits a sector, and in 48,39 the bandits and black magicians of the monsters' records, in packs.
The generator is the game's own, seeded by the sector and the record, so that a sector that comes back has the same creatures; the worn items and the black riders are not followed (docs/decisions/2026-10-20-spawning.md), and the creatures' levels, the fills' bosses and champions and the groups' alarm are in docs/re/fight.md, "In `opensacred-game`".
