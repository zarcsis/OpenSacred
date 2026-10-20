# Spawning: how the world is peopled

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra 12.1.4 on 2026-10-20 (#215).
The record formats are in [docs/02 §7.3](../02-data-formats.md); the record walker in [docs/re/netscript.md](netscript.md).
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
4. each creature a type at random by the weights of the record's `19:(type, weight, hostile)` entries (a bare `02:type` weighs 100 and is peaceful; `08` and `0E` make every entry hostile or peaceful), on a cell at random of the area, tried up to 50 times: one that may be walked on (`0x636C10`) with three more in a row along an axis (`0x45EF70`) and without bit 1 of its flags;
5. the thieves and the black magicians (class 14) and some listed types go in packs: a leader every three or four, the later ones of its class within two cells of it, joined to its group (`+0x249`);
6. a hostile entry sets bit 0 of the creature's flags word (`+0x1F4`, docs/re/movement.md, "The objects in the way"), a peaceful one clears it, and the farm and wild animals take bit `0x20` of `+0x200` and bit `0x40` or `0x10` of `+0x1F4`;
7. at the `DETAILLEVEL` 0 (`0x182EE78`), 30 types of farm and wild animals (513 to 593: crows, bats, rabbits, pigs, cows, deer, bulls, sheep, chickens) are left out unless the record has `08`.

`34:(x, y)` names the record's sector, always its own function's; `33:(lowest, highest)` a level range, which the sector's `SpawnValues` overrides (always, in the files); `36:n`, 100 for the animals' records and 310 for the monsters', goes to the creature's `+0x255`, which the AI reads of idle creatures (`0x46AE70`; not read further).

## `MakeGroup`

`MakeGroup` (type 115, `0x4AB940`) is not a group of creatures but a rule of their equipment: its record adds an entry of `0xA0` bytes, up to 20 creature types (`02`), three items each by a pool and a chance in percent (`87`, `88`, `89`), a byte for the item maker (`8B`), a name (`1D`, `1E` or `16`) and up to ten spells (`67`), of which a creature takes one at random (`+0x2B6`, `+0x261`).
Its one reader is `0x47F8E0`, which `FillSector` calls for each creature it makes of those types; the items come from the start script's `Waffenpool` pools (type 114: a pool and an item; its handler not read).

## The start of the Seraphim

The nine sectors about the start (49 to 51, 38 to 40) are of the levels 1 to 4; their neighbours to the west and the north (48,37 to 48,41, 49 to 51,37) of 25 to 38.

- Their `Enter` functions' `FillSector`s make rabbits alone (`RABBIT.GRN`, two records of `35:(6, 6, 4096)` a sector: about ten a sector for a hero of level 1, a quarter of them of the variants 592 and 593), and place the start sector's novices, a chicken and a rabbit, three pigs in 51,38, and farmers, citizens and animals in 50,40.
- Their `Init` functions place four goblin warriors (`gob_warrior.grn`) in 51,39 and two thieves with their weapons in 50,40, hostile; the ring about them 21 goblins and 11 thieves more.
- Further west and north, the monsters' `FillSector`s make bandits, black magicians, flight lizards and wolves, about 14 hostile creatures a sector.

## In `opensacred-game`

The game places the start script's NPCs and objects alone (`npcs`); the sectors' functions are not run (#221 for their placed creatures, objects and closed cells; #222 for `FillSector`).
