# The combat arts

Status: **confirmed** by code where it gives an address, read 2026-10-22 (#500, Ghidra, in prose); the fields' meanings are **hypotheses** where marked.

## Two tables

The arts are the records of two tables compiled into `Sacred.exe`, the defaults of a balancing spreadsheet: `0x5E6550` can load them again from its sheets and write them out as `xls_magictypes.h` (the first table) and `xls_smovetypes.h` (the second), its sheets named by class and kind (`GladMo`, `SeraSp`, `SeraMo`, `MageFire`, `MageEarth`, `MageWater`, `MageAir`, `MageLost`, `DelfMo`, `DelfTr`, `VampSu`, `VampMo`, `DwarfMo`, `DwarfSp`, `DaemonMo`, `DaemonSp` and others).
The first table holds the spells, the second the weapon arts (its icons `GUI_MOVE_*`, its sheet `SMOVETYPES`): every melee class's own `Rundum`, `Hardhit`, `Attacke` and `Kick` (the icons' names), the Gladiator's and the Seraphim's special moves, the Wood Elf's bow arts, the Vampiress's arts, the Dwarf's guns and the Daemon's arts.
`opensacred-convert tables` writes both into `tables/arts.ron`, their layout in docs/02 §6.5 (#505).

The parsers of the sheet's words name three of the fields' values:

- the launch type (`0x43D190`): 1 `ECF_ONENEMY`, 2 `ECF_ONFRIENDLY`, 3 `ECF_ONWALKABLE`, 4 `ECF_ONDEAD`, 5 `ECF_ONMEMYSELFANDI`, 0 none;
- the element (`0x43D100`): 1 water, 2 air, 3 earth, 4 fire, 5 `LOST`;
- the classes (`0x43ED30`), a mask: 1 the Seraphim, 2 the Gladiator, 4 the Mage, 8 the Dark Elf, `0x10` the Wood Elf, `0x20` the Vampiress, `0x400000` the Dwarf, `0x800000` the Daemon (`ALLHEROES` `0xC0003F`).

A hero's creature type is its class's number: 1 the Seraphim, 2 the Gladiator, 3 the Mage, 4 the Dark Elf, 5 the Wood Elf, 6 and 7 the Vampiress by day and by night, 8 the Dwarf, 9 the Daemon; `0x4D1B20` and `0x4D24C0` give a number's bit of the mask.

The arts' texts are indices: an index `N`, as `0x672C90` takes it, names the `global.res` string whose name is `N` in decimal, whose id is that name's hash (docs/02 §8), as the weapons' records name theirs (docs/02 §6.2).
Among the labels of an art's description (`0x5893B0`), 1107 is the regeneration's and 1146 its unit, seconds; 1097 a duration, 1085 a damage, 1116 a damage in percent, 1074 and 1166 an attack, 1167 a defence, 1104 a radius, 1083 a range, 1105 a quantity, 1106 a size, 1070 a level, 1073 health, 1093 and 1094 the physical and the mental regeneration, 1100 the attack speed.

## The first table: the spells

The records are compiled from `0x952A38`: records of `0x94` bytes, the record 0 empty and the records 1 to 70 searched, to `0x955344`.
`0x4D2310` finds a record by an art's id, the `u16` at the record's start, and gives none for an id it does not hold; it has some 75 callers, the drops, the arrows' special projectiles, the stats' recomputation, the cursor and the interface among them.
The 70 records carry the ids 1 to 72 but 54 and 68, mostly in order: 13 and 14, and 47 and 48, are swapped, and from 54 on the ids run one or two ahead of the records.

| Offset | Type | What | Read by | Art 18 |
|---|---|---|---|---|
| `+0x00` | `u16` | the art's id | `0x4D2310` | 18 |
| `+0x04` | `u32` | the name's text index | `0x4D2690` | 816 |
| `+0x08` | `u32` | the long description's text index; 600 more, the short one's | `0x4D26A0`, `0x5893B0` | 9247 |
| `+0x0C` | 32 characters | the icon's texture name; `+0x2C` keeps its texture's handle at run time | `0x4D25F0`, `0x4D2B10` | `GUI_SPELL_IRRITATION.TGA` |
| `+0x30`, `+0x34`, `+0x38` | `u32` | three sprites of the interface's table (`0x9ECED8`) for the icon (`UI_SPELL_*` in the sheet), `+0x38` the one `0x4D25F0` draws (**hypothesis** for which state each is) | `0x4D2790`, `0x4D25F0` | 558, 559, 560 |
| `+0x3C` | `u16` | the launch type, which `0x51D840` tests | `0x4FE870`, `0x5481F0` | 1, on an enemy |
| `+0x3E`, `+0x40` | `u16` | the regeneration time in seconds at the speed 1, `L·[+0x40] + [+0x3E]` (`0x4D25C0`) | `0x571080` | 15 + 9·L, 24 at level 1 |
| `+0x42 + 4k`, `+0x44 + 4k` | `u16` | thirteen effects, each a base and a step by the level, `base + step·L`: the physical, fire, magic and poison damage (`k` 0 to 3), the attack (4), the defence in percent (5), the physical, fire, magic and poison resistances (6 to 9), the speed (10), the mental and the physical regeneration (11, 12) | `0x4D27B0`, `0x6A6550`, `0x5893B0` by their labels (the magic resistance by its place, **hypothesis**) | none |
| `+0x76 + 4j`, `+0x78 + 4j` | `u16` | three values by the level, `j` 0 to 2, a base and a step (`0x4D26E0`) | `0x4D26B0`, `0x4D26E0` | pair 0: 14 + L |
| `+0x82 + 2j` | `u16` | the cap of the value `j`, 0 for none | `0x4D26E0` | 30 |
| `+0x88` | `u16` | the element | the loader (no reader in the play found) | 0 |
| `+0x8C` | `u32` | the classes that learn and use the art | `0x4D24C0` | 1, the Seraphim |
| `+0x90` | byte | flags: bit 0 not on horseback, bit 1 on horseback only with a bow, bit 2 with a two-handed weapon only (the texts `UI_HORSE_CANTCAST`, `UI_HORSE_ONLYWITHBOW`, `UI_STAT_2H_ONLY`) | `0x4D25A0`, `0x4D2590` | 0 |
| `+0x91`…`+0x93` | bytes | loaded from the sheet, 0 in every record | the loader | |

The value 0 is the art's duration in seconds for most: `0x539E70` gives the target the effect `0x22`, keyed by the art, for the value 0 under its cap 0 (for an art of the second table that casts it, 0.8 times its regeneration time instead).
For others, by its label in `0x5893B0`, it is a range (`GUI_SPELL_PHASEN`, the art 1: 80 + 10·L, at most 200), a summoned creature's level (`GUI_SPELL_SUMMONBEAR`, 3), the health healed (`GUI_SPELL_HEILKRAFT`, 7: 10 + 50·L) or a quantity; the values 1 and 2 are a radius, a speed or a range (the arts 8, 12, 20 and 29).
An art's FX objects (`0x4D2E10`) are as many as its value 0 at its level under its cap 2, at most 20: objects of type 848 that `sdk_CREATE` makes about the target, each within 100 of it at random, every fifth posting an engine's event of type `0x80D` (`0x6770E0`; not read).
So `GUI_SPELL_STEINHAUT`, the art 2, regenerates in 40 + 24·L seconds, lasts 44 + L seconds, at most 120, and gives 15 + 5·L percent of defence, 25 + 40·L of the physical resistance and 10 + 15·L of the poison's, of the element earth, for the Mage.

## The second table: the weapon arts

The records are compiled from `0x94FDB0`: records of `0x78` bytes, the record 0 empty and 94 searched, found by the `u16` at their start (`0x4D1AF0`), the ids 1000 to 1103, mostly in order; an art that the first table does not hold is looked for here.

| Offset | Type | What | Read by | 1071 | 1059 | 1057 |
|---|---|---|---|---|---|---|
| `+0x00` | `u16` | the art's id | `0x4D1AF0` | | | |
| `+0x04`, `+0x08` | `u32` | the name's and the description's text indices, as the first table's | `0x4D1D00`, `0x4D1D10` | 8266, 9166 | 8262, 9162 | 8260, 9160 |
| `+0x0C`, `+0x2C`, `+0x30`…`+0x38` | | the icon, its texture's handle and three sprites, as the first table's | `0x4D1C20`, `0x4D1D40` | `GUI_MOVE_ATTACKE`, 618–620 | `GUI_MOVE_ATTACKE`, 618–620 | `GUI_MOVE_RUNDUM`, 624–626 |
| `+0x3C` | `u32` | a column of the sheet, no reader found | | 7 | 7 | 5 |
| `+0x40`, `+0x44` | `f32` | the regeneration time, `L·[+0x44] + [+0x40]` (`0x4D1C00`) | `0x571080` | 10 + 5·L | 10 + 5·L | 5 + 3·L |
| `+0x48` | `i32` | the distance the art is made from: 101 to 250 is the reach of the two creatures (`0x4DBC60`), within which the creature walks to ±10 (±120 in one case); 0 where it stands; below 0 for the jumps (1003: −290, 1017: −330) | `0x4FD0F0` | 130 | 130 | 130 |
| `+0x4C` | byte | the launch type | `0x4FD0F0`, `0x51D840` | 1 | 1 | 1 |
| `+0x50`, `+0x54` | `f32` | for most arts the damage's factor of each strike, `L·[+0x54] + [+0x50]`, shown as `100 · factor − 100` percent; for the jumps 1003 and 1032 the radius they strike within as they land, 190 + 10·L (below), and for 1083 another value | the executors, `0x4D1EB0`, `0x6A6550`, `0x51CC20` | 0.75 + 0.05·L | 0.75 + 0.05·L | 0.8 + 0.1·L |
| `+0x58`, `+0x5C` | `f32` | a second value by the level, of each art its own: the factor of the strikes of 1003 (1.8 + 0.2·L) and 1083 (0.9 + 0.1·L), the value that 1032 gives each creature it lands by (5 + 10·L; **hypothesis** for its sense), the kicks' 55 + 7·L and 40 + 5·L (**hypothesis** for their sense) | `0x4D1EB0`, `0x4FF0B0`, `0x51CC60` | 0 | 0 | 0 |
| `+0x60`, `+0x64` | `i32` | a whole value by the level: for the `Rundum` arts the radius in percent of the reach, `L·[+0x64] + [+0x60]`, shown 100 less | `0x4FF0B0`, `0x5893B0` | 0 | 0 | 160 + 20·L |
| `+0x68`, `+0x6C` | `i32` | another, of each art its own (1014's in `0x4D1EB0`, below) | | 0 | 0 | 0 |
| `+0x70` | `u32` | the classes | `0x4D1B20` | 1 | 2 | 2 |
| `+0x74` | byte | the flags of the first table's `+0x90` | `0x4D1BE0`, `0x4D1BD0` | 1 | 1 | 1 |
| `+0x75`…`+0x77` | bytes | from the sheet, 0 | the loader | | | |

Some of them cast a spell of the first table, whose regeneration and values they take (`0x4D1E10`): 1018 the art 8, 1019 51, 1044 47, 1049 45, 1050 6, 1051 48, 1052 12, 1053 46.
`0x4D1EB0` gives an art's factor at the level `L` for its description (`0x6A6550`), read again 2026-10-23 (#577): `L·[+0x54] + [+0x50]` for most of the melee arts (1000 to 1002, 1012, 1013, 1015 to 1017, 1020, 1031, 1033, 1038 to 1040, 1054, the classes' `Rundum`, `Hardhit` and `Attacke` from 1057 to 1080, 1084, 1090 to 1092), `L·[+0x5C] + [+0x58]` for 1003 and 1083, `1 + 0.1·L` for 1010, `√L / [+0x5C] + [+0x58]` for 1024 and 1025, `[+0x60] + [+0x68] − [+0x6C] + ([+0x64] + 1)·L` for 1014 (52 + 4·L), and 0 for the others.
The jumps take their values otherwise (read 2026-10-23, #577):

- the Gladiator's 1003 (`0x4FF0B0`), at the point of its clip where the executor tests 0.6 (0.7 for the `Rundum`), starts its regeneration and strikes each creature within `L·[+0x54] + [+0x50]` of it (190 + 10·L) that `0x548F60` admits, with the strike event `0x4EA3F0` and the factor `L·[+0x5C] + [+0x58]` (1.8 + 0.2·L);
- the Vampiress's 1032 (`0x50E6C0`), at the point of its clip where the executor tests 0.35, gives each creature within `L·[+0x54] + [+0x50]` (`0x51CC20`, 190 + 10·L) beyond the two creatures' reach that `0x548F60` admits an event of `0x51CB00` with the whole value `L·[+0x5C] + [+0x58]` (`0x51CC60`, 5 + 10·L, at most 65,534);
- the Seraphim's 1017 holds its factor at `+0x50` (1.8 + 0.2·L) and nothing at `+0x58`, as the other arts do.

`0x580A10` gives the arts 1061 and 1062 as the class's own `Rundum` and `Hardhit`, by the creature's type: 1069 and 1070 of the Seraphim, 1057 and 1058 of the Gladiator, 1065 and 1066 of the Dark Elf, 1073 and 1074 of the Vampiress, 1078 and 1079 of the Dwarf, 1090 and 1091 of the Daemon; the Mage and the Wood Elf keep 1061 and 1062, the Wood Elf's own (classes `0x10`).
The second table holds no 1090, and no rune names it (#577; **hypothesis**: the Daemon has no `Rundum`, and his level of 1061 is that of an art he cannot learn).
The Vampiress's arts by day and by night go in pairs (1022 and 1023, …, 1073 to 1076 and 1038 to 1041), which `0x552370` maps and which are learnt and levelled together; `0x4D1CC0`, `0x4D1D60` and `0x4D1DA0` hold the sets of the night and of the day, and `0x51D840` refuses an art in the other form.

## The runes

The runes are the 140 items of `items.pak` of category 28 (docs/02 §6.1): each names its art by the high word of its `u32` at `+0x68` (`+0x6A`, `0x43AF10`), and its model names the class it serves (`upgrade_` and the class: the Seraphim's 18 runes, the Gladiator's 11, the Mage's 24, the Wood Elf's 17, the Dark Elf's 16, the Vampiress's 17, the Daemon's 13, the Dwarf's 16; and 8 rune stones of another model), with a constant of its class at `+0x70` (1150 to 1159, 1357 for the stones).
By them the Gladiator's, the Dark Elf's and the Vampiress's arts are all of the second table, the Mage's all of the first, the Seraphim's, the Wood Elf's, the Daemon's and the Dwarf's of both; some runes name the art 0 (**hypothesis**: none, or a rune of another use).
Read off the files 2026-10-22 (a scratch script over `tables/items.ron`).

A hero reads a rune through `cItemUpgrade` (`0x5CD100`):

1. it must meet the item's requirements (`0x54ABE0`: its level `+0x14`, strength `+0x15`, dexterity `+0x16`, charisma `+0x17`, endurance `+0x18`, and the skill `+0x1A` of at least `+0x1B`; whether a rune's are computed is not read);
2. its art is of the second table when that holds it with the hero's class in `+0x70`, else of the first when that holds it with the class in `+0x8C`, else the rune is refused (an engine's event 7, 2 and a line of the log) and kept;
3. `0x580750` adds the art to the hero's list at level 0 if it lacks it, with the Vampiress's or the Daemon's linked art, and `0x581F80` raises its level by 1 for good, the linked ones too, and recomputes the stats, so that the first rune teaches the art at level 1;
   `0x580750` takes the stats and the entry of 22 bytes on the stack (the table, the id, the rest 0), `0x581D40` gives the entry's index of an id, and `0x581F80` takes the index, the levels (1), for good (1) and whether to leave the linked arts (0) (read 2026-10-23, #569).
4. the rune takes the flag `0x8000000`, used.

A debugging path (`0x557D60`, under flag `0x1000` of `0x182EE3C`) destroys the rune even when refused.

## A creature's arts

A creature's stats (docs/re/fight.md) hold its arts as a list of entries of 22 bytes, from `+0x102` to `+0x106` of the stats (the creature's `+0x4AA`):

| Offset | What |
|---|---|
| `+0x00` | `u32`, the table: 1 the first, 2 the second |
| `+0x04` | `u16`, the art's id |
| `+0x06` | byte, its level |
| `+0x07` | byte, a bonus to the level, of the items and the sets |
| `+0x08` | `u16`: bit 0 set when learnt, the high byte a slot of the shortcuts (`0x580C80`, `0x580D00`) |
| `+0x0A` | `f32`, its regeneration time |
| `+0x0E` | `f32`, its regeneration's factor, 1.0 |
| `+0x12` | `f32`, the regeneration still to run, in seconds |

The level in play is the level and the bonus (`0x580A10`); a rune raises the level by 1, to 255 at most, and the items' and sets' bonuses the bonus (`0x571710` through `0x581F80`, not for good).
Seen in the original (2026-10-23, #569, the driver's `arts`): the new Seraphim's 1071 at level 1 has bit 0 of `+0x08` clear, as the template's entry has it, and the rune's functions set it; raised to level 3 its time went from 15 to 25 s, `10 + 5·L`, and its regeneration still to run rose by the same 10 s; the spell 18 learnt at level 1 took 24 s, `15 + 9·L`.
A creature that is no player's and lacks an art uses it at the level `⌊creature's level / 5⌋ + 1` (`0x580A10`).
`0x571080` makes every entry's time: the record's value at the level, from the first table or the second, and with the bonus half of what the bonus adds to it when that is more, `v(L) + (v(L + b) − v(L))/2`; a time that grows adds as much to the regeneration still to run.
`0x582A20(art, level, bonus)` gives the same for one art.
The drops' `a` is the sum of the levels of a hero's arts without their bonuses (`0x5598B0`, docs/re/items.md).

## The regeneration

An art is ready when its regeneration still to run is at most 0.01 (`0x580DF0`, which the cursor and the input ask).
Its executor calls `0x562470` once, at its first strike or its cast: it adds the art's time to the regeneration still to run of **every** art of its kind, the spells or the weapon arts, each held to its own time, so the two kinds regenerate apart (the arts 1019 and 1085 to 1087 count as spells); two flags of the stats' `+0x148`, 8 for the spells and 4 for the weapon arts, set by the items' and sets' effects in `0x571710`, take the added time down to 0.8 of it.
`0x562390` keeps the Dark Elf's traps 1047 to 1053 and the Vampiress's arts 1030 to 1037 to their own groups.

`0x562730` runs the regeneration for the local player's hero alone, not while its `+0x200` has bit `0x1000000`: each art's regeneration still to run goes down by the frame's time times its factor and its kind's speed, held within 0 and its time.

- The spells' speed (the stats' `+0xEE`) is `(1 + Σ items' bonus 0x32D / 100 + the sets' bonuses + the effect 0xB) · (1 + MagicRegen / 100)`.
- The weapon arts' speed (`+0xF2`) is `(1 + Σ items' bonus 0x32E / 100 + the effect 0xB) · (1 + PhysicalRegen / 100)`.
- `MagicRegen` and `PhysicalRegen` are the attributes at the stats' `+0x18` and `+0x16` with the items (`0x5796A0`); the Dwarf's spells take the weapon arts' speed.
- The time shown is the time over the speed (`0x5828E0`, `0x5893B0`).
- No skill is read on this path (**hypothesis**: a skill that shortens the regeneration acts through the attributes or the effect `0xB`, not found).

A handler at `0x52B6BB` (after an effect `0x1E`) takes the regeneration of both kinds away at once (−1000); what sets it off is not read.

## A hero's combos

A hero's stats (only for a creature of the heroes' class, `0x426650`) hold its combos as a list of entries of 38 bytes, from `+0x10E` to `+0x112` of the stats (`0x5813D0`): an id at `+0`, four arts' references from `+2` (four bytes each, the art's id in the low word) and their levels from `+0x12` (a `u16` each), the combo's time at `+0x1A`, its regeneration's speed at `+0x1E`, and the time regenerated at `+0x22`, counting up.
Setting a combo writes its four arts and levels and makes `+0x1A` the sum of their values at their levels, from either table, 1.0 where it comes below 0.001, `+0x1E` 1.0 and `+0x22` 0.
The speed is then `1 +` the mean over its arts of their attribute over 100, `MagicRegen` for an id below 1000 and `PhysicalRegen` above; the combo is ready when `+0x22` reaches `+0x1A` (`0x581A60`).
While a combo runs, a use of an art takes from every combo's `+0x22` and its own goes back to 0 (read in part).
Its description (`0x581550`) lists each art's name and level, then `+0x1A` over `+0x1E` (`+0x1A` alone where `+0x1E` is 0) to one decimal between the regeneration's label and its unit, the texts 1107 and 1146.

## An art in a strike

The order to make an art (`0x4FD0F0`, `KiSpMove`) tests its launch type (`0x51D840`), walks to within its distance `+0x48` of the target and takes the class's action of the arts from `0x95583C` (the same at `0x956C48`): the Seraphim 9, the Gladiator 8, the Mage 12, the Dark Elf 10, the Wood Elf 11, the Vampiress 13, the Dwarf 14, the Daemon 15; the art in use is the creature's `+0x120`.
The classes' executors: the Gladiator's `0x4FF0B0`, the Seraphim's `0x5038F0`, the Dark Elf's `0x506770`, the Wood Elf's `0x50AC00`, the arts 1000 to 1002 `0x50CD10`, the Vampiress's `0x50E6C0`, the Dwarf's `0x514790`, the Daemon's `0x5192E0`, the spells' `0x522480`, and the NPCs' special arts `0x4DCB80`.

- `Attacke` (1059 in `0x4FF0B0`, 1071 in `0x5038F0`): at every hit point of the clip (`0x4115D0`, `0x4135D0`, as the plain strike), it strikes the target within its reach + 45, and when the target is gone, the next hostile creature within it; it ends when the clip has no more hit points.
  Each strike is the creature's strike event (`0x4EA3F0`) of the hit's kind 1 and the factor `0.75 + 0.05·L`, and the first starts the regeneration; the strikes are as many as the clip's hit points, not by the level (**hypothesis**).
  Seen in the original (2026-10-23, #569, an immortal goblin warrior of level 1 beside the Seraphim): one use of her 1071 made 7 strikes in about 2.8 s at level 1 and at level 3 alike, each with the attack `28.5 · (1.3 + 0.125·L)` in its chance (40.61 and 47.74) and its damage times 0.8 and 0.9.
- `Rundum` (1057): before 70 % of its clip, it strikes the creature nearest each bone of a weapon or a hand (`0x44CA30`) within `160 + 20·L` percent of the two creatures' reach, each creature once (`+0x190`), with the factor `0.8 + 0.1·L`.
- With a bow the Seraphim's 1069, 1070 and 1071 are shots: the action 5, `+0x184` 1, the special projectiles `0x3D`, `0x3E` and `0x3F`, and `min(L + 1, 5)` arrows for 1069 and 1071.

The strike's resolution (`0x56B130`, docs/re/fight.md) takes, for a strike of a hit's kind other than 0 by a creature in its action of the arts (or the action 5) with an art of the second table, the attack `A' = A · (1.3 + 0.125·L)` (times the usual factor) into the hit's chance `A' / (A' + D)`, the level the entry's or the combo's slot's; the description shows it as `30 + 12.5·L` percent of attack (`0x5893B0`).
Other kinds other than 0 skip the hit's test (**hypothesis**: they always land).
The damage, summed after the resistances, is multiplied by the hit's factor when it lies between 0 and 500 and is not 1, at least 1; `0x4EA3F0` halves the factor with two weapons or a weapon of kind 8.
So at level 1 the Seraphim's 1071 and the Gladiator's 1059 strike with 1.425 times the attack for 80 % of the damage.

## The heroes' starting arts

Each hero template's creature section `0xC7` (docs/02 §9) holds the hero's arts as the stats' list holds them (above): from its byte 1,227 a `u16` count, then the entries of 22 bytes from 1,229 (read off the 8 templates 2026-10-22 with a scratch script; the counts 2, 3 for the Daemon's and 4 for the Vampiress's, so that their sections are 22 and 44 bytes longer, 1,903 and 1,925 for 1,881):

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

Each entry carries its time at level 1 already, the art's record's at that level (the Seraphim's 1071 15.0, her 18 24.0).
In the original's new game the Seraphim has the first alone, 1071 at level 1 of the time 15.0, its entry as the template's (seen 2026-10-22 with #448's driver); why the second is not there is not read.

## Open

The second table's `+0x3C`, and its `+0x58…+0x6C` for each art; which clip each art plays, and so how many strikes an `Attacke` makes; the spells' effects (`0x522480` and the objects it makes), the effects `0x22` and `0xB`; a rune's requirements; the combos' use in full; the NPCs' choice of their arts (`0x5481F0`, `0x4DCB80`); why the Seraphim's art 18 is missing in a new game.
