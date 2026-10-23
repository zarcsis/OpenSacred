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
| `+0x3C` | `u32` | a column of the sheet that no code of the play reads (#510: neither the callers of `0x4D1AF0` nor a reference into the table): 5 for most, 6 and 7 for most classes' `Hardhit` and `Attacke`, 15 for their kicks, 10, 30, 48 and 120 for some special moves | | 7 | 7 | 5 |
| `+0x40`, `+0x44` | `f32` | the regeneration time, `L·[+0x44] + [+0x40]` (`0x4D1C00`) | `0x571080` | 10 + 5·L | 10 + 5·L | 5 + 3·L |
| `+0x48` | `i32` | the distance the art is made from: 101 to 250 is the reach of the two creatures (`0x4DBC60`), within which the creature walks to ±10 (±120 in one case); 0 where it stands; below 0 for the jumps (1003: −290, 1017: −330) | `0x4FD0F0` | 130 | 130 | 130 |
| `+0x4C` | byte | the launch type | `0x4FD0F0`, `0x51D840` | 1 | 1 | 1 |
| `+0x50`, `+0x54` | `f32` | for most arts the damage's factor of each strike, `L·[+0x54] + [+0x50]`, shown as `100 · factor − 100` percent; for the jumps 1003 and 1032 the radius they strike within as they land, 190 + 10·L (below), and for 1083 another value | the executors, `0x4D1EB0`, `0x6A6550`, `0x51CC20` | 0.75 + 0.05·L | 0.75 + 0.05·L | 0.8 + 0.1·L |
| `+0x58`, `+0x5C` | `f32` | a second value by the level, of each art its own: the factor of the strikes of 1003 (1.8 + 0.2·L) and 1083 (0.9 + 0.1·L), the value that 1032 gives each creature it lands by (5 + 10·L; **hypothesis** for its sense), the Wood Elf's 1008 its arrows' second value (`0x5BD2D0`, the first its factor), the kicks' 40 to 55 + 5 to 7·L, which the Gladiator's kick does not read (#510) | `0x4D1EB0`, `0x4FF0B0`, `0x51CC60`, `0x5BD2D0` | 0 | 0 | 0 |
| `+0x60`, `+0x64` | `i32` | a whole value by the level: for the `Rundum` arts the radius in percent of the reach, `L·[+0x64] + [+0x60]`, shown 100 less; for the kicks 1,000, the base of the knockdown's time (below); for the Wood Elf's 1006, over `L·[+0x6C] + [+0x68]`, the ratio its arrow's hit takes (`0x5BB010`, not followed) | `0x4FF0B0`, `0x5893B0`, `0x5BB010` | 0 | 0 | 160 + 20·L |
| `+0x68`, `+0x6C` | `i32` | another, of each art its own (1014's in `0x4D1EB0`, below; 1006's, above) | `0x4D1EB0`, `0x5BB010` | 0 | 0 | 0 |
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
4. the rune takes the flag `0x8000000`, used, unless `0x581F80` refused (a level past 255), when it is kept.

The three functions, read 2026-10-23 (#506):

- `0x580750` does nothing for an art the list holds (by its id alone); else it appends the entry with bit 0 of its `+0x08` set, and for the Vampiress (the types 6 and 7) the night art of a day art's pair (1022 and 1023, 1024 and 1025, 1026 and 1027, 1028 and 1029, 1073 to 1076 and 1038 to 1041), and for any creature the art's counterpart by `0x552370` where that is another, each a copy of the entry, marked too, whether the list holds it or not; then it recomputes the stats (`0x5796A0`).
  `0x552370`, with its flag set as here, gives the Daemon's (type 9) other art of a pair of the table at `0x891154` (1094 and 1091, 1097 and 1092, 1099 and 1101, 1098 and 1102, 1100 and 1103), either way; the Vampiress's other form's, the day art of a night art by day (6) and the night art of a day art by night (7); else the art itself.
  So a rune that the Vampiress reads by night adds the night art twice (read, not seen).
- `0x581D40` gives the index of the first entry of an id, the count where there is none; for the generic `Rundum`, `Hardhit`, `Attacke` and `Kick` (1000, 1001, 1002, 1009) the first entry of any class's own (1000: 1078, 1090, 1057, 1069, 1065, 1061, 1073, 1038; 1001: 1079, 1091, 1058, 1062, 1066, 1070, 1074, 1039; 1002: 1080, 1092, 1059, 1063, 1067, 1071, 1075, 1040; 1009: 1093, 1060, 1064, 1068, 1072, 1076, 1041).
- `0x581F80(index, n, for good, leave the linked)` raises for good the entry's level by `n`, refused where it passes 255, sets its bit 0 and recomputes the stats; else its bonus, refused alike, without the stats.
  Where the time then grew, the growth is added to the regeneration still to run: an art a rune adds goes from its time at level 0 to its time at level 1, so that it waits that growth, the record's step, before its first use (the spell 18, 15 to 24 s: 9 s; **hypothesis** as to what the play shows).
  Unless told to leave them, it raises by as much the art raised with this one where the list holds it (1022, 1024, 1026 and 1028 their night arts 1023 to 1029, and 1025, 1027 and 1029 theirs back but not 1023; 1000 to 1002 and 1009 the night's 1038 to 1041; 1073 to 1076 and 1038 to 1041 each other), else the counterpart of `0x552370`, each with the linked ones left; and it marks the creature's stats changed (`+0x127`).

`opensacred_rules::arts` follows the three (`Arts::add`, `Arts::index`, `Arts::raise`) and the rune's reading, its requirements left to the caller (`Arts::read_rune`, #506).

A debugging path (`0x557D60`, under flag `0x1000` of `0x182EE3C`) destroys the rune even when refused.

## A creature's arts

A creature's stats (docs/re/fight.md) hold its arts as a list of entries of 22 bytes, from `+0x102` to `+0x106` of the stats (the creature's `+0x4AA`):

| Offset | What |
|---|---|
| `+0x00` | `u32`, the table: 1 the first, 2 the second |
| `+0x04` | `u16`, the art's id |
| `+0x06` | byte, its level |
| `+0x07` | byte, a bonus to the level, of the items and the sets |
| `+0x08` | `u16`: bit 0 set on an art a rune taught (`0x580750` sets it on every art it adds, `0x581F80` on every art it raises for good), the high byte a slot of the shortcuts (`0x580C80`, `0x580D00`) |
| `+0x0A` | `f32`, its regeneration time |
| `+0x0E` | `f32`, its regeneration's factor, 1.0 |
| `+0x12` | `f32`, the regeneration still to run, in seconds |

The level in play is the level and the bonus (`0x580A10`); a rune raises the level by 1, to 255 at most, and the items' and sets' bonuses the bonus (`0x571710` through `0x581F80`, not for good).
Seen in the original (2026-10-23, #569, the driver's `arts`): the new Seraphim's 1071 at level 1 has bit 0 of `+0x08` clear, as the template's entry has it, and the rune's functions set it; raised to level 3 its time went from 15 to 25 s, `10 + 5·L`, and its regeneration still to run rose by the same 10 s; the spell 18 learnt at level 1 took 24 s, `15 + 9·L`.
A creature that is no player's and lacks an art uses it at the level `⌊creature's level / 5⌋ + 1` (`0x580A10`).
`0x580A10` takes the generic 1061 and 1062 as the class's own (above), finds the entry by its id alone, and gives its level with its bonus, or without it on its flag; for a creature that is no player's and lacks the art, a boss (`+0x1F4` bit `0x80000`) takes that level times `SpellFakBoss` (1.4) and a champion (`+0x2B8` bit `0x40`) times `SpellFakChamp` (1.2), truncated (the balance's image at `0xAD56A0` and `0xAD56A4`; read 2026-10-23, #510).
`0x571080` makes every entry's time: the record's value at the level, from the first table or the second, and with the bonus half of what the bonus adds to it when that is more, `v(L) + (v(L + b) − v(L))/2`; a time that grows adds as much to the regeneration still to run.
It looks an art up in the first table, then in the second, whatever the entry's table; a spell's value is whole seconds, `L·[+0x40] + [+0x3E]` in integers (`0x4D25C0`), a weapon art's a float (`0x4D1C00`); every entry's factor becomes 1.0, and an art that neither table holds keeps its time (read 2026-10-23, #506).
The growth is added in `0x581F80`, not here.
The skills' function `0x566070`, which the stats' recomputation runs after it, then shortens the times of some arts by some skills (#507).
`0x582A20(art, level, bonus)` gives the same for one art.
The drops' `a` is the sum of the levels of a hero's arts without their bonuses (`0x5598B0`, docs/re/items.md).
`0x588990` (its log calls it `FixSpellVector`), for a hero of a player (an id of 16 or below) whose list holds more entries than it is given, marks each entry's table by its id (1 below 1,000, else 2), takes out an entry of the id 0 or 1,104 and above, of the level 0, of neither table, or of an art not of the hero's class (`0x4D1B20`, `0x4D24C0`), then every later entry of an id already held, and logs the old and the new counts (read 2026-10-23, #510).
No code calls it (no call, no address of it in the program); among the code that writes the list's end itself it is the only one that shortens it, and a vector's own functions, reached with the list's address, were not searched, so what leaves a template's rune-taught arts out of a new game is still open.

## The regeneration

An art is ready when its regeneration still to run is at most 0.01 (`0x580DF0`, which the cursor and the input ask).
Its executor calls `0x562470` once, at its first strike or its cast: it adds the art's time to the regeneration still to run of **every** art of its kind, the spells or the weapon arts, each held to its own time, so the two kinds regenerate apart (the arts 1019 and 1085 to 1087 count as spells); two flags of the stats' `+0x148`, 8 for the spells and 4 for the weapon arts, set by the items' and sets' effects in `0x571710`, take the added time down to 0.8 of it.
`0x562390` keeps the Dark Elf's traps 1047 to 1053 and the Vampiress's arts 1030 to 1037 to their own groups.

`0x562730` runs the regeneration for the local player's hero alone, not while its `+0x200` has bit `0x1000000`: each art's regeneration still to run goes down by the time of the world's tick times its factor and its kind's speed, held within 0 and its time.
It runs on the world's tick, not every frame: the render loop `0x60E350` gathers the frames' time in the float `0xAD6EFC` and, once it is above 0.1 s, calls the creatures' updates `0x5375C0`, which call `0x562730` (`0x5395D2`) with that time, and sets it back to 0; the hero's came every 100 or 117 ms with 0.1 or 0.117 s, 6 or 7 frames at 60 per second (measured with a probe 2026-10-23, #616, #617), so that what is still to run goes down in steps of about 0.1 s and an art is ready at the first tick after it reaches 0.

- The spells' speed (the stats' `+0xEE`) is `(1 + Σ items' bonus 0x32D / 100 + the sets' bonuses + the effect 0xB) · (1 + MagicRegen / 100)`.
- The weapon arts' speed (`+0xF2`) is `(1 + Σ items' bonus 0x32E / 100 + the effect 0xB) · (1 + PhysicalRegen / 100)`.
- `MagicRegen` and `PhysicalRegen` are the attributes at the stats' `+0x18` and `+0x16` with the items (`0x5796A0`); the Dwarf's spells take the weapon arts' speed.
- The time shown is the time over the speed (`0x5828E0`, `0x5893B0`).
- No skill is read on this path; the skills shorten the times themselves (below).

A handler at `0x52B6BB` (after an effect `0x1E`) takes the regeneration of both kinds away at once (−1000); what sets it off is not read.

Read again 2026-10-23 (#507):

- `0x562470(time, kind, group)` takes the kind as a table's number, 1 the spells and 2 the weapon arts; with a combo running (the creature's `+0x2B4`, the stats' `+0x59`) or the kind 3 it runs the combos' instead (not read here).
  For each entry it takes the kind of the entry's table, the spells' for 1019 and 1085 to 1087, and the time, 0.8 of it with the flag of that kind; an entry of the kind given takes it, held within 0 and its time, unless `group` is an art and `0x562390` refuses the pair.
  `0x562390(art, group)` admits the art itself, and else counts how many of the two are among the traps 1047 to 1053 and the bites and bats 1030 to 1037: it admits 0 or 2, whatever the groups, and refuses 1.
- The executors pass the time of `0x582850(art)`, the entry's time, else `0x582A20(art, 0, 0)`, the record's at level 0: the Gladiator's `(time, 2, 0)`, the Dark Elf's `Attacke` its own id 1067 as the group, one call of the Dark Elf's the time times a constant first; which executor passes what is #508's.
- `0x562730` takes the spells' speed for an entry of the first table and 1019, the weapon arts' for any other: 1085 to 1087, which start as spells, run at the weapon arts' speed.
  An entry with time still to run loses `factor · speed · seconds`, `seconds` the tick's, held within 0 and its time; it runs for the creature whose `+0x39` is the local player's number alone.
- `0x5796A0` sets both speeds to 1.0, adds a hundredth of each item's bonus 813 (`0x32D`) to the spells' and 814 (`0x32E`) to the weapon arts', adds the value of the effect `0xB` to both, multiplies them by `1 + MagicRegen/100` and `1 + PhysicalRegen/100` after the skills, holds them at 0 and above, and gives the Dwarf (type 8) the weapon arts' speed for his spells.
- `0x5796A0` writes the times in `0x571080` and then calls the skills' function `0x566070` (`0x57C658`), whose `0x565DC0` shortens the times of a list of arts by a skill: each art of the list that the creature holds has its time divided by `1 + v/100`, `v` the skill's curve at its level from a triple of balance keys (`off + 2(w − off)(1 − 1/(1 + (level − 1)/S))`), held at 1 s at least, its regeneration still to run held to it.

| Skill | Keys | `off`, `S`, `w` | Arts |
|---|---|---|---|
| 1 | `HM` | 6, 50, 100 | the spells 13, 15, 17 to 22, and 1019 |
| 15 | `FM` | 6, 50, 100 | 5, 11, 23, 24 |
| 16 | `WM` | 6, 50, 100 | 4, 27, 34, 35 |
| 17 | `EM` | 6, 50, 100 | 2, 16, 25, 26 |
| 18 | `LM` | 6, 50, 100 | 1, 28, 29, 33 |
| 19 | `MM` | 6, 50, 100 | 3, 7, 36 to 42 |
| 20 | `KO` | 12, 50, 300 | 43, 1023, 1025, 1027, 1029 to 1041 |
| 25 | `KO` | 12, 50, 300 | 1000 to 1018, 1020, 1022, 1024, 1026, 1028, 1042 to 1046, 1054, 1057 to 1076, 1078 to 1080, 1082 to 1084, 1089, 1091, 1092, 1094, 1097 to 1103 |
| 26 | `BA` | 6, 50, 100 | 1047 to 1053 |
| 27 | `KO` | 12, 50, 300 | 1047 to 1053 |
| 28 | `BL` | 6, 50, 100 | 43, 1022, 1030 to 1037 |
| 29 | `WT` | 6, 50, 100 | 1085 to 1087, 55 to 59 |
| 31 | `KO` | 12, 50, 300 | 1085 to 1087, 55 to 59 |
| 32 | `HOM` | 6, 50, 100 | 60 to 67 |

The heroes' second starting skills are 25 for the Gladiator and the Dark Elf and 20 for the Vampiress, each at level 1 so that `v` is 12: their times are the records' over 1.12 (the Gladiator's 1059 13.39 s and 1057 7.14 s, seen 2026-10-23 with the driver, #507), as the templates hold them; the other classes' skills are none of these.
The Gladiator's 1057 taught by a rune then rose from 7.14 to 9.82 s at level 2, the growth still to run (seen).
`opensacred_rules::arts` follows them (`Arts::recompute`, `Arts::shorten`, `Arts::start`, `Arts::regenerate`, `Arts::ready`, `Speeds`), `opensacred_rules::fight::arts_speeds` gives a creature's speeds, and the game runs the hero's regeneration on the world's tick, as the original does (`frame::WorldTick`, #616, #627); every entry of the 8 templates, taught or not, recomputes to the very time the template holds.

## A hero's combos

A hero's stats (only for a creature of the heroes' class, `0x426650`) hold its combos as a list of entries of 38 bytes, from `+0x10E` to `+0x112` of the stats (`0x5813D0`): an id at `+0`, four arts' references from `+2` (four bytes each, the art's id in the low word) and their levels from `+0x12` (a `u16` each), the combo's time at `+0x1A`, its regeneration's speed at `+0x1E`, and the time regenerated at `+0x22`, counting up.
Setting a combo writes its four arts and levels and makes `+0x1A` the sum of their values at their levels, from either table, 1.0 where it comes below 0.001, `+0x1E` 1.0 and `+0x22` 0.
The speed is then `1 +` the mean over its arts of their attribute over 100, `MagicRegen` for an id below 1000 and `PhysicalRegen` above; the combo is ready when `+0x22` reaches `+0x1A` (`0x581A60`).
While a combo runs, a use of an art takes from every combo's `+0x22` and its own goes back to 0 (read in part).
A combo is ordered as an art of `0x200` plus its index (the right button, below); its arts' executors take the level of the combo's step (`0x56ABE0(combo, step)`: the step's level for the steps 0 to 3, else the level of the last step that has an art and a level) where the creature's `+0x2B3` and `+0x2B4` name a combo and its step, or `+0x401` a combo, instead of the art's own (read 2026-10-23, #510).
Its description (`0x581550`) lists each art's name and level, then `+0x1A` over `+0x1E` (`+0x1A` alone where `+0x1E` is 0) to one decimal between the regeneration's label and its unit, the texts 1107 and 1146.

## An art in a strike

The order to make an art (`0x4FD0F0`, `KiSpMove`) tests its launch type (`0x51D840`), walks to within its distance `+0x48` of the target and takes the class's action of the arts from `0x95583C` (the same at `0x956C48`): the Seraphim 9, the Gladiator 8, the Mage 12, the Dark Elf 10, the Wood Elf 11, the Vampiress 13, the Dwarf 14, the Daemon 15; the art in use is the creature's `+0x120`.
The classes' executors: the Gladiator's `0x4FF0B0`, the Seraphim's `0x5038F0`, the Dark Elf's `0x506770`, the Wood Elf's `0x50AC00`, the arts 1000 to 1002 `0x50CD10`, the Vampiress's `0x50E6C0`, the Dwarf's `0x514790`, the Daemon's `0x5192E0`, the spells' `0x522480`; a shot, an art's with a bow among them, is the action 5's `0x4DCB80` (docs/re/fight.md).
`0x5481F0`, a creature's handler of an event whose sender is not read, picks an art of the class of the event's `+0x10` from a table of 20 rows of 8 columns by their weights, a roll out of 301, and orders it (`0x105`) on the creature for a launch type 2, else on the local player's hero (`0xAD3CE0`); in `w` cases of `w + 1` (`w` 2, 4 or 8 by the class) it posts the event `0x100` instead (read 2026-10-23, #510).

- `Attacke` (1059 in `0x4FF0B0`, 1071 in `0x5038F0`): at every hit point of the clip (`0x4115D0`, `0x4135D0`, as the plain strike), it strikes the target within its reach + 45, and when the target is gone, the next hostile creature within it; it ends when the clip has no more hit points.
  Each strike is the creature's strike event (`0x4EA3F0`) of the hit's kind 1 and the factor `0.75 + 0.05·L`, and the first starts the regeneration; the strikes are as many as the clip's hit points, not by the level (**hypothesis**).
  Seen in the original (2026-10-23, #569, an immortal goblin warrior of level 1 beside the Seraphim): one use of her 1071 made 7 strikes in about 2.8 s at level 1 and at level 3 alike, each with the attack `28.5 · (1.3 + 0.125·L)` in its chance (40.61 and 47.74) and its damage times 0.8 and 0.9.
- `Rundum` (1057): from 30 % of its clip (read again, #510: while what is left of it is under 70 %), it strikes the creature nearest each bone of a weapon or a hand (`0x44CA30`) within `160 + 20·L` percent of the two creatures' reach, unless it is the one it struck last (`+0x190`), with the factor `0.8 + 0.1·L`; the Seraphim's 1069 from the clip's start ("A `Rundum`'s run" below).
- `Hardhit` (1058 in `0x4FF0B0`, 1070 in `0x5038F0`): at its clip's hit point, half the clip where the clip has none, one strike on its target with the factor `1.8 + 0.2·L`, whatever the distance, which starts the regeneration and ends the art.

Read again 2026-10-23 (#508):

- An executor sends a strike with the byte 15 as its kind and the factor `L·[+0x54] + [+0x50]`, `L` the level in play (`0x580A10`); `0x4EA3F0` makes it the hit's kind 1 with that factor, and a plain strike its kind with the factor 1, and halves the factor where the creature holds a weapon of kind 8, or an item in each hand, the one of `+0x1D4` not of the shields' category 13 (`0x427CD0`): 0.5 for a factor of 0 or below.
- `Attacke`, the Gladiator's and the Seraphim's alike: each time its executor runs, while its target lives (`0x549080`) within the two creatures' reach and 45 (`0x5370B0` against `0x4DBC60`), it waits for the clip's next hit point and strikes then, the first strike starting the regeneration (`+0x18C` counts the strikes); its target lost, it takes the first creature of the world's list (`0xAD3CE0`, `0xAD3BC8` of them) other than it that is hostile (`0x548F60`) within the reach and 45, and with none it ends.
  The same test of the reach and 45 stands in the Wood Elf's 1063, the Vampiress's 1040 and 1075, the Dwarf's 1080 and the Daemon's 1092, and in `0x50CD10` for 1000 to 1002.
- `Rundum`: its executor's first run starts the regeneration; each run from 0.3 of its clip (the Gladiator's; read again, #510) it takes a point of a bone of a weapon or a hand, twice (`0x44CA30`, `0x623AD0`), and finds the creature nearest it (`0x5FEAF0`'s, but itself), within 1,000 and the radius, `[+0x60] + [+0x64]·L` per cent of the reach both times; that creature, unless it is the one struck last (`+0x190`) or it may not be struck (`0x549000`), takes a strike and becomes the last, so that a creature is struck again after another.
- The strike's resolution takes the art's attack only for a hit's kind other than 0 from a creature in its class's action of the arts (`0x956C48`) or the action 5, with an art of the second table (`+0x120`) whose level is the entry's level and bonus (`0x580A10`), or a combo's (`0x56ABE0`); a hit's kind other than 0 of no such art skips the hit's test, and a creature of type 71 takes the plain test as well.
- Every strike that lands deals 1 at least, after the factor (`0x56B130`), a plain strike's too.

`opensacred_rules::fight::art_strike` and `art_attack` resolve an art's strike, `fight::strike` scales any strike by its factor between 0 and 500 and deals 1 at least, and `opensacred_rules::arts` gives the arts' styles, factors and targets (`style`, `factor`, `held_factor`, `attacke_target`, `rundum_radius`, `rundum_target`, #508).
- With a bow the Seraphim's 1069, 1070 and 1071 are shots: the action 5, `+0x184` 1, the special projectiles `0x3D`, `0x3E` and `0x3F`, and `min(L + 1, 5)` arrows for 1069 and 1071.

The strike's resolution (`0x56B130`, docs/re/fight.md) takes, for a strike of a hit's kind other than 0 by a creature in its action of the arts (or the action 5) with an art of the second table, the attack `A' = A · (1.3 + 0.125·L)` (times the usual factor) into the hit's chance `A' / (A' + D)`, the level the entry's or the combo's slot's; the description shows it as `30 + 12.5·L` percent of attack (`0x5893B0`).
Other kinds other than 0 skip the hit's test, so that they always land (read again, #508, above).
The damage, summed after the resistances, is multiplied by the hit's factor when it lies between 0 and 500 and is not 1, at least 1; `0x4EA3F0` halves the factor with two weapons or a weapon of kind 8.
So at level 1 the Seraphim's 1071 and the Gladiator's 1059 strike with 1.425 times the attack for 80 % of the damage.

## The arts' clips

Read 2026-10-23 (#509) in the Gladiator's and the Seraphim's executors, on foot.
An art's first run plays its clip with `0x593320(slot, 0.5, 1.0, rate)`: a slot of the model's motions (docs/re/motions.md), the blend 0.5 over the rate at most 0.2 s, and the rate the creature's attack speed over 100 (`0x584FE0`, the stats' `+0x3C` at most 220, times the float 0.01), the speed its plain attack plays at.
The slot is the class's by the art and the class of what the creature holds (`0x546F80`); with a bow or a crossbow (7, 12) the arts are shots instead (above):

| Art | Class 0 | 1 and the others | 2 | 3 | 4 | 8 |
|---|---|---|---|---|---|---|
| the Gladiator's `Attacke` 1059 | 210 | 211 | 212 | 213 | 211 | 216 |
| the Gladiator's `Rundum` 1057 | 220 | 220 | 221 | 222 | 220 | 220 |
| the Gladiator's `Hardhit` 1058 | 201 | 202 | 203 | 204 | 202 | 207 |
| the Seraphim's `Attacke` 1071 | 211 at 0.73 | 211 at 0.73 | 212 | 211 at 0.73 | 214 | 216 |
| the Seraphim's `Rundum` 1069 | 153 | 153 | 153 | 153 | 153 | 153 |
| the Seraphim's `Hardhit` 1070 | an attack's | an attack's | an attack's | an attack's | an attack's | an attack's |

- The Seraphim's `Attacke` of the slot 211 plays at 0.73 of the rate (the float at `0x891080`), and her `Hardhit` plays the slot of the action 4, an attack's clip (`0x5467A0`, one of five at random).
- The Gladiator's `Attacke` plays other slots, some at random, for six types of NPCs (`+0x10` 192, 204, 329, 342, 343 and 350; not followed); with a flag of its order set a `Hardhit` plays 194 (**hypothesis**: on horseback).
- The models' clips in those slots are named for them: `GLAD_SPECIAL_MULTI_*` (210 to 216), `GLAD_SPECIAL_MGEG_*` (220 to 222), `GLAD_SPECIAL_HHIT_*` (201 to 207), `SERA_SPECIAL_MULTI_*` (211 to 216) and `SERA_ATTACK_SPECIAL10` (153).

An art's hit points are its clip's floats from `MotionDesc +0xA0`: an `Attacke` reads its `n`-th as `0x4135D0(model, slot, 8 + n)`, `+0x80` and `4·(8 + n)`, which is `floats_62[1 + n]` of `tables/motions.ron` (docs/02 §4.1), and ends at the first that is not above 0; a `Hardhit` reads the first, 0.5 where it is not within (0, 1].
So the Seraphim's 1071 with her blade, `SERA_SPECIAL_MULTI_1H`, has seven, from 0.137 to 0.747 of its 6.38 s, and the Gladiator's 1059, `GLAD_SPECIAL_MULTI_1H`, four, from 0.152 to 0.702 of its 5.18 s: at the game's speed 1.25 times her attack speed 1.54 times 0.73 the first and the last are 2.77 s apart, and at 1.25 times his 1.40 1.63 s apart, as the original's 7 strikes in about 2.8 s and 4 in about 1.65 s (#508's comments).
A strike is due once what is left of the clip is less than `1 − point` of it.

The order (`0x4FD0F0`) makes the art, taking the class's action of the arts (`0x95583C`), once the creature stands within 10 of the distance the art is made from (120 for a rider), the target where it was: for a record's distance of 100 to 250 the reach of the two creatures (`0x4DBC60`), for 0 where it stands; the jumps of a distance below 0 and the arts 1003, 1017 and 1032 test the way first (`0x4FCC80`).

A `Rundum`'s bones (`0x44CA30`, `0x623AD0`): first `weapon_fx01` or `Weapon_fx01` of the item in its right hand, else its own `Bip01 R Hand`, within a radius of the reach times the per cent truncated to a whole number; then `weapon_fx01` or `Weapon_fx01` of its other item, within the radius as a float.

In `opensacred-game` (#509, `making`): a right click on a hostile creature, or the console's `arts make`, makes the selected art (at a new game the first whose action is read; `arts select`) if the hero holds it and it is ready, where the original's hero walks to the target with an art not ready (#654, "The right button" below); the hero comes on to within 10 of its reach with the target (by their sizes, #642), stops, turns to the target and plays the art's clip at `GameSpeed` times its attack's clip speed times the art's rate (`opensacred_rules::arts::action`, `hit_points`, `made_from`), and sends its strikes as `fight::Hit`s of the art's level and factor (`fight::art_strike`).
Its `Rundum` looks for the creatures about its bones on the world's tick of about 0.1 s (`frame::WorldTick`; **hypothesis**, #617), about the first weapon's effects' bone of the hero's scene, else its right hand, the Gladiator's from 0.3 of its clip and the Seraphim's throughout (`opensacred_rules::arts::rundum_looks`, #644); the second bone is left out; each creature's radius is by its reach with the hero (#642).
Only the Gladiator's and the Seraphim's arts have actions; the spells, the shots with a bow, the jumps and the kicks wait, and the other classes' clips are read below (#510).

### The other classes' clips

Read 2026-10-23 (#510) in the other executors' first runs, which play an art's clip with `0x593320(slot, blend, 1.0, rate)` as the Gladiator's and the Seraphim's do.
With a bow or a crossbow (7, 12) a class's `Rundum`, `Hardhit` and `Attacke` are shots, the action 5 with `+0x184` 1 and the special projectiles `0x3D`, `0x3E` and `0x3F`, `min(L + 1, 5)` arrows for the first and the last (docs/re/fight.md); on horseback a `Hardhit` plays 194 at the blend 0.2, turned to its target first.
The slots by the class of what the creature holds (`0x546F80`, docs/re/motions.md):

| Executor | Rate, blend | `Attacke` | `Hardhit` | `Rundum` | Kick |
|---|---|---|---|---|---|
| the Gladiator's `0x4FF0B0` | the attack speed, 0.5 | 1059 (above) | 1058 (above) | 1057 (above) | 1060: 148 at the rate 1.0 |
| the Seraphim's `0x5038F0` | the attack speed, 0.5 | 1071 (above) | 1070 (above) | 1069 (above) | 1072: 81 at the rate 1.0 |
| the Dark Elf's `0x506770` | the attack speed, 0.5 | 1067: 0 210, 2 212, 8 216, else 211 | 1066: 0 145, 1 202, 2 203, 4 205, 8 207, else the action 4's clip | 1065: 0 219, 2 221, 4 223, 8 225, else 220 | 1068: 144 at the rate 1.0 |
| the Wood Elf's `0x50AC00` | the attack speed, 0.25 | 1063: 0 210, 4 214, 5 215, else 211 | 1062: 0 201, 4 205, 5 206, else 202 | 1061: 0 219, 4 223, 5 224, else 220 | 1064: 147 for 2 and 3, else 146, at 1.0 and 0.5 |
| `0x50CD10`, the generic arts | 1.0, 0.5 (`Attacke` 0.25) | 1002: 0 210, 4 214, 5 215, else 211 | 1001: the action 4's clip | 1000: 0 219, 2 221, 4 223, 5 224, 8 225, else 220 | 1009: 147 for 2 and 3, else 146 |
| the Vampiress's `0x50E6C0` | the attack speed, 0.5 | 1040 and 1075: the claws 210, 2 212, 8 216, else 211 | 1039 and 1074: the claws 201, else the action 4's clip | 1038 and 1073: the claws 219, 8 225, else 221 | 1041 and 1076: 151 at the rate 1.0 |
| the Dwarf's `0x514790` | 1.0, 0.5 | 1080: 0 210, 2 212, 3 213, 8 216, 13 and 14 248, else 211 | 1079: 0 201, 2 203, 3 204, 8 207, 13 247, 14 148, else 202 | 1078: 0 219, 2 221, 3 222, 13 and 14 248, else 220 | |
| the Daemon's `0x5192E0` | 1.0, 0.5 | 1092: 0 210, 2 212, 3 213, 8 216, else 211 | 1091: a charge (below) | none | |

- The Vampiress's claws are her night form's, the flag `+0x14` bit `0x2000` (the class 0 whatever she holds, docs/re/motions.md); her day arts 1073 to 1075 shoot with a bow, the night's do not test for one.
- The Dwarf's slot falls back to 211, 202 or 220 where his model lacks it (`0x4115D0`, `0x4117B0`); his 248 and 247 are his guns' (**hypothesis** for the classes 13 and 14), and his `Rundum` with a gun first gathers the hostile creatures within 1,000 that a straight way reaches (`0x4FCC80`; not followed).
- The Daemon's executor looks for no bone: he has no `Rundum`, as the second table has no 1090 (above); his 1091 (`_hardhit_demon`, the distance 0) tests the way to its target (`0x4FCC80`) and runs to it on the action 2's clip (**hypothesis**: a charge; not followed).
- The kicks' and the generic arts' clips, and the Dwarf's and the Daemon's, play at the rate 1.0, not at the attack speed.

### A `Rundum`'s run

Read again 2026-10-23 (#510) in the Gladiator's (`0x500CCC`) and the Seraphim's (`0x504F37`, her 1069 and 1020) executors, the others' alike at their starts: the first run starts the regeneration; each run takes the point of the bone `weapon_fx01` or `Weapon_fx01` of the item in the right hand (`+0x1D8`), else of the creature's own `Bip01 R Hand` (`0x44CA30`, `0x623AD0`), and among the creatures of `0x5FEAF0`'s list but itself the nearest to that point, its whole distance below 1,000 and below `⌊reach · ([+0x60] + [+0x64]·L) · 0.01⌋`; unless that creature is the one struck last (`+0x190`) or one it may not strike (`0x549000`), it strikes it with the art's factor and makes it the last.
Then it does the same about the bone of the item in the other hand (`+0x1D4`), without a hand's bone.
The second nearest is never taken: by one creature a `Rundum` strikes once, by two it strikes each in turn as the blade comes nearer the other.
The Gladiator's goes to the bones only while what is left of its clip is less than 0.7 of it, so from 0.3 of the clip to its end; the Seraphim's, the Dark Elf's and the Wood Elf's test no part of their clips (#644).

Measured in the original (2026-10-23, by the coordinator with #448's driver: immortal goblin warriors of level 1 beside the hero, the art of the right button set, a probe of the strikes at `0x56B130`), from the clip's start:

- the Seraphim's 1069 by one goblin struck once, at +216 ms, by two 7 times in turn, at +215, +332, +448, +548, +898, +1015 and +1249 ms of a clip of about 1.45 s;
- the Gladiator's 1057 by one goblin struck once, at +780 ms, by two 3 times in turn, at +766, +883 and +1083 ms (+764, +882 and +1116 in a second run) of a clip of about 1.85 s (**hypothesis**: his blade's bone stays beyond the radius of 225 until then);
- the strikes fell on the world's ticks (**hypothesis**: the executors run in the creatures' updates on the tick, #617), the Gladiator's 1059 at +449, +1098, +1649 and +2082 ms (the clip's points give 0.45, 1.08, 1.63 and 2.08 s), the Seraphim's 1070 at +415 to +465 ms and the Gladiator's 1058 at +650 to +681 ms, once each with the factor 2.0;
- a right click with 1059 on a goblin 118 to 120 away made the art where the hero stood, at the first tick, within 10 of the reach of 125 (docs/re/fight.md).

### The kicks

Read 2026-10-23 (#510) in the Gladiator's 1060 (`0x5019D6`; the other classes' kicks not read, #650): once what is left of the clip is less than 0.4 of it (0.45 with a weapon of class 2 or 3), the target takes a strike with the factor and, by a roll of 0 to 99 below `0x4FC9D0(L, target)`, is knocked down.
The chance is `100 · 5L/M` per cent, `M` the target's level (`+0x3FE`, through `0x552230` for a player's), 0 for the types 52, 53, 55, 74, 88, 100 to 103, 136, 137, 141, 144, 182, 183, 185, 237, 250 to 252, 336, 348, 360, 363, 365, 367, 701 and 1249, times 0.769 for a boss (`+0x1F4` bit `0x80000`) and 0.833 for the types of `0x426CB0`.
A knocked target's `+0x24D` takes `[+0x60] · min(1, 5L/M)`, 1,000 times for every kick, and above 10 it plays its slot 175 (`0x593770`, at 3.0).
The kicks' `+0x58` and `+0x5C` (40 to 55 and 5 to 7 by the level) are not read on this path.

Measured in the original (2026-10-23, by the coordinator with #448's driver, two runs: the Gladiator of level 1 in a new game, 1060 learnt and set as the right button's art, an immortal goblin warrior of level 1, a probe at about 1 ms):

- the clip 148 ran about 1.07 and 1.10 s, and the kick struck once, +633 and +665 ms after its start, the factor 1.0 (the clip's 1.22 s at the rate 1.0 and `GameSpeed` 1.25 make 0.98 s, whose 0.6 is 0.59 s, then the next tick);
- at the strike's frame the goblin's `+0x24D` became 1,000 and its action went from 3 to 0;
- `+0x24D` then fell at every frame, by 6 or 7 at 60 frames a second, about 400 a second, to 0 about 2.49 s after the strike and to −1 a frame later, and the goblin took the action 1 (it stood and walked) about 0.7 s after that: so a knockdown of 1,000 keeps its target down about 2.5 s and up again some 3.2 s after the kick (**hypothesis**: the fall is the frame's time times 400; not read).

### The right button

Read 2026-10-23 (#510): the art of the right button is the engine's `u16` at `+0x5A` (`cEngine` at `0xAD6EF8`).
The engine's handler of the mouse (`0x6172C0`), on the right button over a creature with an art below 1,200 there, posts the hero the event `0x105` with the target, its place and the art `0x552370(art, 0)` (the other form's for the Vampiress and the Daemon); the cursor (`0x611E70`) shows by the same art whether its launch type admits the creature under it (`0x51D840`) and whether it is ready (`0x580DF0`).
Written by the driver, the field made the right click on a goblin make that art, and the interface did not write it back (#510).
The event carries the target at `+0x10`, its place at `+0x18` and `+0x1C`, 100 at `+0x28`, the art's kind at `+0x2C` (0 a spell, 1 a weapon art, 2 a combo) and the art at `+0x30`.
The creature's handler of the event (about `0x52FE40`) takes an art that is not ready as an order to walk to the target's place instead (`+0xFC` 1, with the target and its place), or with Ctrl held posts the creature an event of kind 4 (not followed); a combo's (its art `0x200` plus the combo's index) waits for `0x581A60` alike.

## The effects

Read 2026-10-23 (#510): a creature's stats hold its effects as a list of entries of 32 bytes from `+0x11A` to `+0x11E`: the effect's id at `+0x00`, a whole value at `+0x04`, a value as a float at `+0x14` and an art at `+0x1C`, added by `0x582D50(id, …, value, …)`.
The recomputation (`0x5796A0`, after the items) adds each to the stats by its id (#651):

| Effect | What it does |
|---|---|
| 1 | adds its value to `A`'s multiplier (`+0xE6`) |
| 2 | adds it to `D`'s (`+0xEA`), held at 0.01 at least |
| 3 to 6 | to the physical, fire, magic and poison damage multipliers (`+0x96`, `+0x9A`, `+0x9E`, `+0xA2`) |
| 7 to 10 | to the multipliers at `+0x66`, `+0x6A`, `+0x6E`, `+0x72` (**hypothesis**: the four resistances') |
| 11 (`0xB`) | to both regeneration speeds, the spells' (`+0xEE`) and the weapon arts' (`+0xF2`), before the attributes' factors (above) |
| 12 | the regeneration value (`+0x138`) times 1 plus it |
| 28 | the attack speed (`+0x3C`), `+0x3E` and the pace (`+0x40`) times it, truncated |
| 30 | its whole value taken from the three |
| 33 | its value to the damage multiplier of the kind its whole value names (0 to 3) |
| 40 | with its art 1015 and the stats' `+0x148` bit `0x20000`, the record's `[+0x50] + [+0x54]·L` of 1015 at its level (1 to 100) to `A`'s and `D`'s multipliers |
| 42 | `D`'s multiplier times 100 over its value |
| 52 | the attack speed and the pace times it |
| 59 | its whole value to `+0x7A` and `+0x8A` (not identified) |

The other ids up to 59 do nothing there; among them the effect `0x22` (34), which `0x539E70` gives a spell's target keyed by the spell, its value the spell's duration (its value 0 at its level, or 0.8 of its time for an art of the second table), and the effects `0x15` and `0x19` of the shots (docs/re/fight.md).

## The spells' executor

Read 2026-10-23 (#510): the spells' executor (`0x522480`) switches on the spell and makes the spell's object with `sdk_CREATE` (`0x5FBA40`), an object of the client's types whose names its table keeps (`TYPE_FX_…`, records of 68 bytes with the number before the name); the effects then come from those objects (not read).
By the spell, its object types, and whether the executor itself gives the target the effect `0x22` (`0x539E70`):

| Spell | Objects | `0x22` |
|---|---|---|
| 1 (`PHASEN`) | 775 `FX_TELEPORT_IN`, 774 `FX_TELEPORT_OUT` | |
| 2 (`STEINHAUT`) | 769 `FX_SHIELDED` | yes |
| 3 (`SUMMONBEAR`) | 777 `FX_BIGSUMMON` | |
| 4 (`WASSERFORM`) | 783 `FX_SUPERFLUID` | |
| 5 (`FEUERBALL`) | 778 `FX_FIRESTRIKE` | |
| 6 | 782 `FX_PENTAGRAMMKILLER` | |
| 7 (`HEILKRAFT`) | 784 `FX_HEALING` | |
| 8, 13 (`BLITZ_SERA`) | 798 `FX_LIGHTNING_STRIKE` | |
| 11 (`FEUERSPIRALE`) | 790 `FX_FIRE_CIRCLE` | |
| 12 (`ACIDBOLT`) | 792 `FX_GROUND_POISON` | |
| 15 (`HIMMELSLEUCHTEN`) | 804 `FX_HOLYLIGHT` | |
| 16 (`BANNKREIS`) | 805 `FX_GROUNDSYMBOL` | |
| 17 (`LICHTKLINGEN`) | 814 `FX_HOLYWALL` | yes |
| 18 (`IRRITATION`) | 816 `FX_BURNINGLIGHT` | |
| 19 (`BEKEHRUNG`) | 806 `FX_HOLYCONVERT` | |
| 20 (`LICHT`) | 817 `FX_MAGLITE` | |
| 21 (`GLAUBENSSTAERKE`) | 818 `FX_CRUSADERSTRENGTH` | yes |
| 22 (`LICHTSCHILD`) | 819 `FX_HOLYSHIELD` | yes |
| 23 (`FEGEFEUER`) | 820 `FX_HELLFIRE` | |
| 24 (`FLAMMENHAUT`) | 821 `FX_FIRESKIN` | yes |
| 25 (`FLEISCHSTEIN`) | 822 `FX_STONED` | |
| 26 (`METEORITENHAGEL`) | 823 `FX_METEOR` | |
| 27 (`KATARAKT`) | 825 `FX_SPEEDUP` | yes |
| 28 (`WINDBOE`) | 826 `FX_TWISTER` | yes |
| 29 (`WIRBELWIND`) | 831 `FX_WINDSTRIKE` | |
| 30 (`SCHILDWALL`) | 834 `FX_MAGICSHIELD` | yes |
| 31 (`AUE`) | 835 `FX_MANAREGEN` | yes |
| 32 (`REIKI`) | 836 `FX_HPREGEN` | yes |
| 33 (`BLITZSCHLAG`) | 799 `FX_FREEZE`, 872 `FX_FREEZE_IMPACT` | |
| 34 (`EISSPLITTER`) | 839 `FX_ICEBULLETS` | |
| 35 (`FROSTRING`) | 840 `FX_CIRCLEOFICE` | |
| 36 (`MOUSE`) | 845 `FX_CHANGELING` | |
| 37 (`SPEED`) | 846 `FX_HIGHSPEED` | yes |
| 38 (`CAMOUFLAGE`) | 847 `FX_BUBBLETRAP` | |
| 39 (`THORNS`) | 848 `FX_KILLERPLANTS` | |
| 40 (`PLANTCAGE`) | 849 `FX_PLANTCAGE` | |
| 41 (`LOSTSOULS`) | 850 `FX_LOSTSOULS`, 779 `FX_TEMPTEX` | yes |
| 42 (`PLANTHEAL`) | 851 `FX_NATUREHEALING` | |
| 43 | 924 `FX_REANIMATE` | |
| 49, 50, 52, 53 | 909 `FX_LASERBEAM_IMPACT`; 910 `FX_DAEMONFIRE` and 878 `FX_SMALLFIRE`; 922 `FX_SHADDAR_MAGIC`; 867 `FX_BALL` | |
| 55 to 59 (the Dwarf's) | 1155 `FX_DWARF_GIER`, 1154 `FX_DWARF_MINE_SIGN`, 1152 `FX_DWARF_SHIELD`, 1021 `FX_PANZER`, 1022 `FX_STAHL` | |
| 60 to 67 (the Daemon's) | 1005 `FX_HELLDISK`, 1004 `FX_TRANSDIM`, none for 62 (the effect 46 itself), 1173 `FX_FORCEOFHELL`, 1015 `FX_DEMONFIRE`, 1013 `FX_DAYOFTHETENTACLE`, 1011 `FX_SONGSFROMHELL` (and the effect 35), 1012 `FX_HELLSCREAM` | |
| 69 to 72 | 1163 `FX_ANDUCAR_DEMON`, 1164 `FX_ANDUCAR_GARDE`, 1166 `FX_DAEMONENFUERST`, 1167 `FX_TERRORSPRINTER` | |

The spells 44 to 48, 51, 54 and 68 fall to the switch's default, and 9, 10 and 14 to another case, and make no object there.
The spells that give the effect `0x22`, each of the launch type 2 or 5, are those that last on their target (**hypothesis**: the effect marks a running spell for its duration); what each object gives its target, and for how long, is the objects' code.

## The heroes' starting arts

Each hero template's creature section `0xC7` (docs/02 §9) holds the hero's arts as the stats' list holds them (above): from its byte 1,227 a `u16` count, then the entries of 22 bytes from 1,229 (read off the 8 templates 2026-10-22 with a scratch script; the counts 2, 3 for the Daemon's and 4 for the Vampiress's, so that their sections are 22 and 44 bytes longer, 1,903 and 1,925 for 1,881):

| Template | Class | Arts (table, id, level 1), bit 0 clear | Arts with bit 0 set |
|---|---|---|---|
| `Hero00.ptx` | Gladiator | 2: 1059 | 2: 1057 |
| `Hero01.ptx` | Seraphim | 2: 1071 | 1: 18 |
| `Hero02.ptx` | Daemon | 2: 1097, 1092 | 1: 66 |
| `Hero03.ptx` | Dwarf | 2: 1080 | 1: 55 |
| `Hero04.ptx` | Wood Elf | 2: 1062 | 1: 42 |
| `Hero05.ptx` | Mage | 1: 5 | 1: 32 |
| `Hero06.ptx` | Dark Elf | 2: 1066 | 2: 1051 |
| `Hero07.ptx` | Vampiress | 2: 1022, 1023 | 2: 1074, 1039 |

Each entry carries its time at level 1 as the stats' recomputation left it: the record's, shortened by the class's skill where one shortens it (#507; the Seraphim's 1071 15.0 and her 18 24.0, the records'; the Gladiator's 1059 13.39 and 1057 7.14, the records' 15 and 8 over 1.12).
`opensacred_formats::amh` reads them (`Amh::arts`, matching `amh_dump.py --arts`), and `opensacred-convert heroes` writes them into `heroes.ron` (#506).

In the original's new game the Seraphim has the first alone, 1071 at level 1 of the time 15.0, its entry as the template's (seen 2026-10-22 with #448's driver), and the Gladiator 1059 alone (seen 2026-10-23, #507); the hero's loader `0x585C60` reads every entry, and what drops the others is not read (not `0x588990`, "A creature's arts" above).
The arts dropped are those whose bit 0 is set, the bit a rune sets; so a new game keeps the arts of the template that no rune taught: the Daemon's 1097 and 1092 and the Vampiress's 1022 and 1023, pairs of their forms, and the first of the others (**confirmed** in the original 2026-10-23 with the driver's `arts`: the Daemon's 1097 of 32.0 s and 1092 of 12.0 s, the Vampiress's 1022 of 30.0 s and 1023 of 2.7 s, each at level 1).
`opensacred_rules::arts::Arts::new_game` keeps them, their times by their records shortened by the hero's skills (#507), and the game gives them to the hero (#506).

## Open

What the spells' objects give their targets (`FX_…`, above; #652); the special weapon arts' executors (#653: the jumps 1003, 1017 and 1032 in part, 1004 to 1008, 1010 to 1016, 1018 to 1021, the Vampiress's 1022 to 1037, the Dark Elf's 1042 to 1053, the Dwarf's 1081 to 1089, the Daemon's 1091 and 1093 to 1103) and the other classes' kicks (#650); the `+0x58…+0x6C` that they read; a rune's requirements; the combos' run beyond their levels; the sender of `0x5481F0`'s event; what leaves a template's arts that a rune taught out of a new game (not `0x588990`, above); the effects 59's fields and 7 to 10's (above).
