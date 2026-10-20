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
A sector entered without such a list gets crabs and fish (the types 163 to 165) on fitting cells.

`FillSector` has a gate for its respawn (at `0x47CC1E`): the balance variable `Respawn` (300) or `DungeonRespawn` (1,000) against the clock in seconds, keeping the clock in milliseconds at `+0x9A` of the sector's information; it compares the clock with the constant alone, not with the time kept (**hypothesis**: a slip in the original's expression), so it passes and `FillSector` fills at every `Enter`.

## The sector's level

`SpawnValues` (type 100, `0x4A9670`), the first record of every sector function, sets three words of the sector's information (`0x63D010`, the world record's `+0x17C`): `+0xD1` a density in percent (50 in every record), `+0xD3` and `+0xD5` the lowest and the highest level of the sector's creatures (1 to 45, 4 to 80).
`CreateNPC`, `CreateObj`, `0x564D60` and the message of an area's danger (`0x622D70`) read them.
The difficulty adds to them from the balance table (its image's `+0x788` and `+0x98C`, indexed 0 to 3 for the difficulties 1 to 4, **hypothesis** for the order): 0, 35, 70 and 128 to the lowest, 50, 120, 190 and 250 to the highest.
A creature's level is the hero's (`+0x3FE`) held within those bounds.

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
The creature's level is the hero's held within the sector's bounds and jittered (by 0 to 1 below 6, −1 to 2 below 30, −2 to 3 below 90, −3 to 6 below 250), one or two more in a dungeon's cell; some types and about 2 % of the creatures hostile to a living hero are made champions (`HarteSau`, 20 in 1,001), the types 37, 48, 55 and 84 a boss or a champion by a coin, 127 and 49 so in one case of two; they are made at the level 1, which their first fight raises (docs/re/fight.md, "Bosses and champions").
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

## The start of the Seraphim

The nine sectors about the start (49 to 51, 38 to 40) are of the levels 1 to 4; their neighbours to the west and the north (48,37 to 48,41, 49 to 51,37) of 25 to 38.

- Their `Enter` functions' `FillSector`s make rabbits alone (`RABBIT.GRN`, two records of `35:(6, 6, 4096)` a sector: about ten a sector for a hero of level 1, a quarter of them of the variants 592 and 593), and place the start sector's novices, a chicken and a rabbit, three pigs in 51,38, and farmers, citizens and animals in 50,40.
- Their `Init` functions place four goblin warriors (`gob_warrior.grn`) in 51,39 and two thieves with their weapons in 50,40, hostile; the ring about them 21 goblins and 11 thieves more.
- Further west and north, the monsters' `FillSector`s make bandits, black magicians, flight lizards and wolves, about 14 hostile creatures a sector.

## In `opensacred-game`

The game takes the `CreateNPC`, `CreateObj`, `CreateTrigger` and `TriggerPatch` records of the sectors' `Init` and `Enter` functions with the start script's (`npcs`, #221): each NPC or object is placed as the sector of its cell comes in and placed again when it comes back, which the original's `Init` creatures, made once, and its `Enter` creatures, made again at each coming, both looked like while nothing changed them; since the AI and the fight, a creature killed or moved comes back whole at its place (docs/decisions/2026-10-20-spawning.md, #329); the placed monsters, hostile, stop the hero's steps (docs/re/movement.md).
In the Seraphim's tree that makes 3,137 NPCs and 3,122 objects over the world, against the start script's 320 and 1,990: the start sector's novices, chicken and rabbit, and the four goblins of 51,39 among them.
The `Enter` functions' `FillSector`s make their creatures as their sectors come in (`spawning`, #222, #229): the count by the sector's `SpawnValues` for the hero's level and the balance's `SpawnAnimalsSP` and `SpawnEnemysSP`, the types by their weights as the client picks them, the leaders by the run's counters, the cells by the steps 6 and 7 above (the first leader of a run alone, its kind gathered about it), the rabbits' and the crows' variants, the hostile ones with bit 0 of their flags word.
They hold the first two items of the first `MakeGroup` rule of the sector that names their type (#228), rolled by the rule's chances from the start script's pools, in the hands `npcs` gives the scripts' NPCs, with the clips of what they hold; the third, a worn piece, the crossbows' replacement for the models without their slot, the types' own lists and the spells are not followed.
About the start that makes some ten rabbits a sector, and in 48,39 the bandits and black magicians of the monsters' records, in packs.
The generator is the game's own, seeded by the sector and the record, so that a sector that comes back has the same creatures; the worn items and the black riders are not followed (docs/decisions/2026-10-20-spawning.md), and the creatures' levels, the fills' bosses and champions and the groups' alarm are in docs/re/fight.md, "In `opensacred-game`".
