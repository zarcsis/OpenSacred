# The combat arts' tables

Status: **confirmed** by code where it gives an address, read 2026-10-22 (#500, Ghidra, in prose); the fields' meanings are **hypotheses** where marked, and most of the records' fields are not read yet.

## The arts' table

The combat arts' records are compiled into `Sacred.exe` from `0x952A38`: records of `0x94` bytes, the record 0 empty and the records 1 to 70 searched, to `0x955344`.
`0x4D2310` finds a record by an art's id, the `u16` at the record's start, and gives none for an id it does not hold; it has some 75 callers, the drops, the arrows' special projectiles, the stats' recomputation, the cursor and the interface among them.
The 70 records carry the ids 1 to 72 but 54 and 68, mostly in order: 13 and 14, and 47 and 48, are swapped, and from 54 on the ids run one or two ahead of the records.

The record's fields, as far as read:

| Offset | Type | What |
|---|---|---|
| `+0x00` | `u16` | the art's id |
| `+0x04` | `u32` | 769 to 867: the art's name, a text's index (`0x4D2690`, which the combos' description gives to `0x672C90`; not an id of `global.res`) |
| `+0x08` | `u16` | up to 9,295, 0 in some (**hypothesis**: the name's or the description's text) |
| `+0x0C` | characters | an icon's texture name, inline |
| `+0x30`, `+0x34`, `+0x38` | `u32` | three numbers in a row, from 462 to 1,589 (**hypothesis**: motion slots or sounds) |
| `+0x3C` | `u16` | 0, 1, 2, 3 or 5 (**hypothesis**: a kind or a class) |
| `+0x3E`, `+0x40` | `u16` | the art's value at a level `L`: `L·[+0x40] + [+0x3E]` (`0x4D25C0`), such as 2·L + 10 or 24·L + 40 (**hypothesis**: its regeneration time, which a combo sums, below) |
| `+0x76 + 4j`, `+0x78 + 4j` | `u16` | the `j`-th pair of a base and a step by the level, `L·[+0x78 + 4j] + [+0x76 + 4j]` (`0x4D26E0`) |
| `+0x82 + 2i` | `u16` | the `i`-th cap of such a value, 0 for none (`0x4D26E0`) |

An art's FX objects (`0x4D2E10`) are as many as its pair 0 at its level, under its cap 2 and at most 20: objects of type 848 that `sdk_CREATE` makes about the target, each within 100 of it at random, every fifth posting an engine's event of type `0x80D` (`0x6770E0`; not read).

## The second table

An art that the first table does not hold is looked for in a second one (`0x4D1AF0`): records of `0x78` bytes from `0x94FDB0` to `0x952A38`, the record 0 empty and 94 searched, found by the `u16` at their start, the ids 1000 to 1103 (mostly in order).
Its value at a level `L` is `L·[+0x44] + [+0x40]`, both floats (`0x4D1C00`), such as 3·L + 5 or 6·L + 10; its name a text's index at `+0x04` as the first table's (`0x4D1D00`).
So the arts have two kinds by their ids, 1 to 72 in the first table and 1000 to 1103 in the second; by the runes (below), the melee classes' arts are mostly of the second (**hypothesis**: the weapon arts against the spells).

## The runes

The runes are the 140 items of `items.pak` of category 28 (docs/02 §6.1): each names its art by the high word of its `u32` at `+0x68` (`+0x6A`), and its model names the class it serves (`upgrade_` and the class: the Seraphim's 18 runes, the Gladiator's 11, the Mage's 24, the Wood Elf's 17, the Dark Elf's 16, the Vampiress's 17, the Daemon's 13, the Dwarf's 16; and 8 rune stones of another model), with a constant of its class at `+0x70` (1150 to 1159, 1357 for the stones).
By them the Gladiator's, the Dark Elf's and the Vampiress's arts are all of the second table, the Mage's all of the first, the Seraphim's, the Wood Elf's, the Daemon's and the Dwarf's of both; some runes name the art 0 (**hypothesis**: none, or a rune of another use).
Read off the files 2026-10-22 (a scratch script over `tables/items.ron`); how the client uses a rune is not read.

## The heroes' starting arts

Each hero template's creature section `0xC7` (docs/02 §9) holds the hero's arts as the stats' list holds them (below): from its byte 1,227 a `u16` count, then the entries of 22 bytes from 1,229, the first `u32` of an entry 1 for an art of the first table and 2 for one of the second (read off the 8 templates 2026-10-22 with a scratch script; the counts 2, 3 for the Daemon's and 4 for the Vampiress's, so that their sections are 22 and 44 bytes longer, 1,903 and 1,925 for 1,881):

| Template | Class | Arts (table, id, level 1) |
|---|---|---|
| `Hero00.ptx` | Gladiator | 2: 1059, 1057 |
| `Hero01.ptx` | Seraphim | 2: 1071; 1: 18 |
| `Hero02.ptx` | Daemon | 2: 1097, 1092; 1: 66 |
| `Hero03.ptx` | Dwarf | 2: 1080; 1: 55 |
| `Hero04.ptx` | Wood Elf | 2: 1062; 1: 42 |
| `Hero05.ptx` | Mage | 1: 5, 32 |
| `Hero06.ptx` | Dark Elf | 2: 1066, 1051 |
| `Hero07.ptx` | Vampiress | 2: 1022, 1023, 1074, 1039 |

Each entry carries its value at level 1 already, the art's record's at that level (the Seraphim's 1071 15.0, her 18 24.0).
In the original's new game the Seraphim has the first alone, 1071 at level 1 of the value 15.0, its entry as the template's (seen 2026-10-22 with #448's driver); why the second is not there is not read.

## A creature's arts

A creature's stats (docs/re/fight.md) hold its arts as a list of entries of 22 bytes, from `+0x102` to `+0x106` of the stats: the art's id at `+4`, its level at `+6`, a bonus to its level at `+7`, and the value it takes at `+0xA`, a float, with 1.0 at `+0xE`.
`0x571080` makes every entry's value: the record's value at the level, from the first table or the second, and with the bonus half of what the bonus adds to it when that is more, `v(L) + (v(L + b) − v(L))/2`.
`0x582A20(art, level, bonus)` gives the same for one art; the record that `0x4D1E10` points at may stand in for it (not read).

## A hero's combos

A hero's stats (only for a creature of the heroes' class, `0x426650`) hold its combos as a list of entries of 38 bytes, from `+0x10E` to `+0x112` of the stats (`0x5813D0`): an id at `+0`, four arts' references from `+2` (four bytes each, the art's id in the low word) and their levels from `+0x12` (a `u16` each), a float at `+0x1A`, another at `+0x1E` set to 1.0, and a word at `+0x22` cleared.
Setting a combo writes its four arts and levels and makes `+0x1A` the sum of their values at their levels, from either table, 1.0 where it comes below 0.001.
Its description (`0x581550`) lists each art's name and level, and then `+0x1A` divided by `+0x1E` (`+0x1A` alone where `+0x1E` is 0) to one decimal between two texts (`0x453`, `0x47A`; **hypothesis**: the combo's regeneration time and its unit).

## Open

The texts `0x453` and `0x47A`, and what `+0x1E` of a combo is; the records' other fields and their uses (`0x571710`, `0x5813D0`, `0x4E0300`, `0x582D50`, the interface's); how an art is learnt from its rune, levelled and regenerated; the second table's other fields.
