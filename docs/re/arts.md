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

In `opensacred-game` (#509, `making`): a right click on a hostile creature, or the console's `arts make`, makes the selected art (at a new game the first whose action is read; `arts select`) if the hero holds it and it is ready, and with the art not ready the right click walks the hero to the creature's place, as the original's does (#654, "The right button" below; its Ctrl's event left out); the hero comes on to within 10 of its reach with the target (by their sizes, #642), stops, turns to the target and plays the art's clip at `GameSpeed` times its attack's clip speed times the art's rate (`opensacred_rules::arts::action`, `hit_points`, `made_from`), and sends its strikes as `fight::Hit`s of the art's level and factor (`fight::art_strike`).
Its `Rundum` looks for the creatures about its bones on the world's tick of about 0.1 s (`frame::WorldTick`; **hypothesis**, #617), about the first weapon's effects' bone of the hero's scene, else its right hand, the Gladiator's from 0.3 of its clip and the Seraphim's throughout (`opensacred_rules::arts::rundum_looks`, #644); the second bone is left out; each creature's radius is by its reach with the hero (#642).
The Gladiator's, the Seraphim's, the Dark Elf's, the Wood Elf's, the Vampiress's, the Dwarf's and the Daemon's `Rundum`, `Hardhit` and `Attacke` have actions, by "The other classes' clips" below (#649; the Vampiress's claws by night a **hypothesis**, the Dwarf's slots falling back where his model lacks them, his and the Daemon's at the rate 1.0), and the Gladiator's and the Seraphim's kicks (#650); the spells, the shots with a bow, the jumps, the other classes' kicks, the Daemon's charge 1091, the Dwarf's guns' `Rundum` and the executors' blends wait.

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

Read 2026-10-23 (#510) in the Gladiator's 1060 (`0x5019D6`; the other classes' kicks not read, #650): once what is left of the clip is less than 0.4 of it (0.45 with a weapon of class 2 or 3), the target takes a strike with the factor and, by a roll of 0 to 99 below `0x4FC9D0(L, target)`, is knocked back.
The Seraphim's 1072 (`0x5038F0` at `0x50408E`) strikes once what is left is less than 0.6 of its clip, by the same rule (read 2026-10-23, #650).
The chance is `100 · 5L/M` per cent, `M` the target's level (`+0x3FE`, through `0x552230` for a player's), 0 for the types 52, 53, 55, 74, 88, 100 to 103, 136, 137, 141, 144, 182, 183, 185, 237, 250 to 252, 336, 348, 360, 363, 365, 367, 701 and 1249, times 0.769 for a boss (`+0x1F4` bit `0x80000`) and 0.833 for the big types of `0x426CB0` (`Creature.pak` flags bit `0x02`).
A knocked target's `+0x24D` takes `[+0x60] · min(1, 5L/M)`, 1,000 times for every kick; above 10 (and unless the target's `+0x200` has bit `0x40` and the kicker's not, not read) it plays its slot 175, its first hit clip (`HIT_A`), at 3.0 with the blend 0.25 (`0x593770`), and takes an event of kind 3 with the kicker and the distance; the kicker makes an object of type 842 at its place (not followed).
The target then slides away from its kicker (`0x594D12`, under `+0x200` bit `0x20000`, the kicker at `+0x259`, read 2026-10-23, #650): at every frame a step of 400 units a second times the frame's time, at most 0.3 s of it, with a carry in 16.16 at `+0x379`, along the line from the kicker's place to its own, moved where it may go (`0x4EBC40`), the step taken from `+0x24D` whether it moved or not; at 0 or below `+0x24D` is −1 and the bit cleared.
The kicks' `+0x58` and `+0x5C` (40 to 55 and 5 to 7 by the level) are not read on this path.

The event of kind 3, read 2026-10-24 (#773) in the creature's events' handler (`0x531AF7`): unless its `+0x24` method answers the query `0x50` (in the single player's game; not followed), the creature takes the event's value for `+0x24D`, sets `+0x200` bit `0x20000` and the sender at `+0x259`, ends what it does (`0x54E6D0(0, 1)`: the AI's state and sub-state 0, the action 0 and its sub-state 1, its idle clip, `+0x200` bit `0x10` cleared), sets its timer to fire in 3 s (`+0x208` = `+0x204` less the float 3.0 at `0x890768`) and `+0x200` bit `0x40000`, clears `+0x36D` and `+0x371`, and passes the event on to the creature that `0x552660` names where that one is of a type that `0x4266F0` admits (**hypothesis**: its mount; not followed).
While `+0x200` bit `0x40000` is set the AI's state 0 does nothing (`0x542B20` returns at once), so the creature neither looks nor wakes; the first firing of its timer that finds bit `0x20000` clear, its slide over, clears bit `0x40000` (`0x5375C0`), and the creature takes up its AI again.
**Seen** 2026-10-24 in the running original (the driver of #448, the Seraphim of level 1 at 3250,2511, a goblin warrior of level 5 made two cells off, struck and knocked back by her 1020): at the strike its AI's state went from 4.6 to 0.0 and its action to 0.1; it slid three cells and stood there, the state 0.0, until about 3.1 s after the strike, then took the state 4.1, walked back and was struck again 3.4 s after the first strike.
`opensacred-game` follows it (#773, `opensacred_rules::minds::Mind::knocked`, `making::held` at each insertion of `making::Knocked`): the creature leaves its fight and is held, its timer set to fire in 3 s, thinking nothing until a firing finds its slide over (`Senses::sliding`); on the same goblin the game's stood still to about 3.0 s after the strike and then came at the hero again.

Measured in the original (2026-10-23, by the coordinator with #448's driver, two runs: the Gladiator of level 1 in a new game, 1060 learnt and set as the right button's art, an immortal goblin warrior of level 1, a probe at about 1 ms):

- the clip 148 ran about 1.07 and 1.10 s, and the kick struck once, +633 and +665 ms after its start, the factor 1.0 (the clip's 1.22 s at the rate 1.0 and `GameSpeed` 1.25 make 0.98 s, whose 0.6 is 0.59 s, then the next tick);
- at the strike's frame the goblin's `+0x24D` became 1,000 and its action went from 3 to 0;
- `+0x24D` then fell at every frame, by 6 or 7 at 60 frames a second, about 400 a second, to 0 about 2.49 s after the strike and to −1 a frame later, and the goblin took the action 1 (it stood and walked) about 0.7 s after that: the slide of 1,000 at 400 a second (above; where the goblin went was not measured).

In `opensacred-game` (#650, `making`): the Gladiator's 1060 and the Seraphim's 1072 play 148 and 81 at the rate 1.0 times `GameSpeed`, strike once at their point (`opensacred_rules::arts::kick_point`), roll the knockback (`knockback_chance`, `knockback`) and leave the target `making::Knocked`, which slides it away from the hero at 400 a second, at most 0.3 s a frame, onto the cells it may stand on, its mind, walk and strikes held until the slide ends; the target's clip 175, the event and the object 842 are left out, and the other classes' kicks wait for their executors (#649).
In the game the Gladiator's 1060 on a goblin warrior struck 616 ms after its start; the goblin slid one cell, the walls of the start's room in its way, and struck again about 3.5 s after the kick.

### The right button

Read 2026-10-23 (#510): the art of the right button is the engine's `u16` at `+0x5A` (`cEngine` at `0xAD6EF8`).
The engine's handler of the mouse (`0x6172C0`), on the right button over a creature with an art below 1,200 there, posts the hero the event `0x105` with the target, its place and the art `0x552370(art, 0)` (the other form's for the Vampiress and the Daemon); the cursor (`0x611E70`) shows by the same art whether its launch type admits the creature under it (`0x51D840`) and whether it is ready (`0x580DF0`).
Read again 2026-10-24 (#760): with its flag clear `0x552370` gives the Vampiress the art of her form as with it set (a night art's day art by day, a day art's night art by night), and the Daemon, of a pair of `0x891154`, its second (1091, 1092, 1101, 1102, 1103) while an effect keyed by its first (1094, 1097, 1099, 1098, 1100) holds on him (`0x582B00`), its first otherwise.
Seen 2026-10-24 (the driver of #448, the Daemon's new game, 1091 learnt and selected, the right button on a goblin warrior): he made 1094.
`opensacred-game` takes the art so (#760, `opensacred_rules::arts::right_button`) for the right button and the console's `arts make`: the Daemon's 1091 selected makes 1094, and the Vampiress's night art by day her day art (1023 made 1022).
Written by the driver, the field made the right click on a goblin make that art, and the interface did not write it back (#510).
The event carries the target at `+0x10`, its place at `+0x18` and `+0x1C`, 100 at `+0x28`, the art's kind at `+0x2C` (0 a spell, 1 a weapon art, 2 a combo) and the art at `+0x30`.
The creature's handler of the event (about `0x52FE40`) takes an art that is not ready as an order to walk to the target's place instead (`+0xFC` 1, with the target and its place), or with Ctrl held posts the creature an event of kind 4 (not followed); a combo's (its art `0x200` plus the combo's index) waits for `0x581A60` alike.

## The special weapon arts

Read 2026-10-24 (#653) in the executors' disassembly, **confirmed** by code unless marked otherwise.
An executor's first run (the action's sub-state 0) plays the art's clip and passes to the sub-state 1; each later run (the sub-state 1, at the world's ticks) takes `left`, the share of the clip still to play, and acts on it; an art ends by `+0x16C` 1.
The arts of the second table whose classes (`+0x70`) are 0 belong to no hero: 1000 to 1002, 1009 and 1021.

The order's test of an art and its target (`0x51D840`, read 2026-10-24, #759), beyond what "An art in a strike" names:

- the launch types 5 and 0 make the maker its own target;
- the type 2 (`ECF_ONFRIENDLY`) takes the maker where there is no target, and else wants a target alive, on the maker's floor, and its friend (`0x5490B0`), or, for a maker that is no player's hero, one not hostile to it (`0x549000`); the Daemon's 1094 and 1097 to 1100 pass where the maker is of the type 9;
- 1031, 1042, 1044 to 1048, 1082 and 1089 are refused while an effect keyed by them holds on the maker (`0x582B00`), and 1018 and 1019 while either's does;
- 1083 and 1084 want a weapon of the classes 2 or 3 in hand, 1085 and 1087 an item at `+0x1DC`, 1086 that and a straight way to its point.

**Seen** 2026-10-24 in the running original (the driver of #448, the Gladiator's new game, `arts learn 1015` and `arts select 1015`): a right click on a peaceful goblin warrior beside him, and ones on the ground at five cells about him and at his feet, made nothing, while one on his body made it on himself (the effects 1 and 2 of 0.7 for 40 s keyed by 1015, then the effect 34): so an art of the launch type 2 is made on the hero by the right button on him, or on a friend under the cursor (a party's member, not followed); and the Dwarf's 1089, made again while its effects held, made nothing.

### The Seraphim's

The Seraphim's executor (`0x5038F0`) has cases for 1003 and 1017 together, 1013 and 1054 together, 1018, 1019, 1020 with 1069, 1021, and her `Hardhit`, `Attacke` and kick (above); of them her own are 1017, 1018, 1019, 1020 and 1054 (the classes 1), the jump 1003 and the hurled blades 1013 being the Gladiator's (2).

- **The jumps, 1017 (`GUI_MOVE_KAMPFSPRUNG`) and 1003 here.**
  The first run tests the way to the action's point (`0x4FCC80`) and that the creature may stand there (`0x4EBC40`), and without either ends the art (the sub-state 2); else it plays the clip and sets `+0x200` bit `0x10`.
  Once `left` is below 0.75, once, it takes its target's place where the target lives (else the action's point), brought nearer by half the two creatures' reach (`0x4DBC60`), and flies there (`0x44DED0`: a move through the air, `+0x14` bit `0x100000`, to the ground's height), and starts the regeneration.
  Once `left` is below 0.4 it lands: its target, where it may strike it (`0x549000`), takes one strike of the factor `L·[+0x54] + [+0x50]`, 1.8 + 0.2·L for 1017, with the effect `0x44` on it; then the art ends and the bit `0x10` is cleared.
  So the Seraphim's jump strikes its target alone, where the Gladiator's strikes about it ("The second table" above).
  Read again 2026-10-24 (#754): the first run plays the slot 147 at the blend 0.15 (`0x505C99`) and at a rate by her distance `d` from the point: 5 within 10, 1 beyond 500, else `5 − ⌊d/99⌋`, which is 0 from 495 to 500.
  The flight (`0x44DED0`) takes the landing point, the marker −1000 (from where she stands, on the ground), the height `d/12` and the time a quarter of the clip's (the float 0.25 at `0x8907C4`); its step (`0x44DFE0`, under `+0x14` bit `0x100000`) moves her along the line to the point in that time, at the part `t` of it raised `d/12 · (1 − 4·(t − 0.5)²)` units of the models' height, and tests the cells of the way, ending the flight at one she may not stand on.
  The order makes 1003, 1017 and 1032 where the creature stands, whatever the record's distance (−290, −330), once a straight way leads to the target (`0x4FD0F0`, `0x4FCC80`).
  **Seen** 2026-10-24 in the running original (the driver of #448, the Seraphim of level 1 at 3250,2511, `arts learn 1017`, a goblin warrior of level 5 made four cells off, which came on to two, and the right button on it): about half a second after the click she was a cell nearer it, and her one strike fell on it there.
  `opensacred-game` makes it (#754, `opensacred_rules::arts::Style::Jump`, `order_distance`, `jump_rate`, `jump_landing`, `jump_height`; `making::Leap`): where the hero stands, refused without a straight way of walkable cells to the target or with its cell not walkable, at the rate by the distance but at least 1 (docs/decisions/2026-10-24-jump-rate.md); below 0.75 left of the clip it flies to the target's place short of half the reach, or to the target's first place where it is gone, on the arc above the ground, in a quarter of the clip, and starts its regeneration; below 0.4 it strikes its target where it lives and ends; the cells under the flight are not tested, and the effect `0x44` and the bit `0x10` are left out.
  On the same setup the game's Seraphim flew a cell on to a creature beside her and struck it 0.63 s after the art began.
- **The hurled blades, 1054 (`GUI_MOVE_SCHLEUDERKLINGEN`) and 1013 here.**
  With a bow or a crossbow in hand (the classes 7 and 12, `0x546F80`) the art is refused (the AI's state 0); 1054 starts its regeneration at its first run, 1013 at none of this executor's.
  Each run takes the item in the right hand (`+0x1D8`), else the one in the left (`+0x1D4`), and does nothing without one; the point of the Seraphim's own socket of that hand, `Bone_weapon_01` or `Bone_weapon_02` (`0x44CA30`), brought down to the ground (`0x623AD0`); and the first object that `0x600340` finds there, read again 2026-10-24 (#770): of the point's sector's list and, where the point lies within 80 of their edges, its neighbours' lists, the first object of any kind but the Seraphim herself, not flagged at `+0x14` bit 4 and on her layer (`+0x24`), nearer the point than the sum of its type's size and the item's (`0x428CE0`: `items.pak +0x14`, 50 for 0).
  Unless that object is the one struck last (`+0x190`) or one she may not strike (`0x549000`), so that the run strikes nothing then, she strikes it with the factor `L·[+0x54] + [+0x50]`, 1.35 + 0.15·L for 1054 (2.75 + 0.25·L for 1013), with the effect `0x44`, and it becomes the last.
  So the blade strikes what it passes through as the clip swings it, each creature once until it strikes another.
  On foot 1054 plays the slot 148 at the blend 0.5 and the rate 1.6 (`0x505D44`).
  **Seen** 2026-10-24 in the running original (the driver of #448, the Seraphim of level 1 after `load start-seraphim`, `arts learn 1054`, `arts select 1054`, four immortal goblin warriors made about her and the right button on one, the trace `strike`): she struck three times, the target, a goblin beside it a tick later and the target again 1.4 s after.
  `opensacred-game` makes it (#754, #770, `opensacred_rules::arts::Style::Blades`, `blade_touches`; `making::Blade`): on each world's tick of its clip the point of the hero's socket of the item in its right hand, else its left's, and the living creature nearest it that it touches by the two sizes, the item's from its template, where that creature is hostile and not the last struck, takes a strike; the nearest creature stands for the first object of the lists, and the objects that are no creatures are left out; its regeneration starts as it starts.
  Her socket swings out from her and back twice in the clip: on four goblins about her, the game's Seraphim struck them mostly at about 1.0 s and 2.4 s into the clip, as the original's struck its goblins 1.4 s apart; how many each swing strikes depends on where the creatures stand.
- **1018 (`GUI_MOVE_BFG`, the launch type 5).**
  The first run plays the clip with the effect `0x43` on the Seraphim and sets `+0x200` bit `0x10`.
  Once `left` is below 0.5 it takes the items out of both hands (`0x556A30` of the slots 13 and 12, the interface's taskbar told for the local player), starts the regeneration, ends, and gives the Seraphim the effect 16 of the whole value `L` for `[+0x58] + [+0x5C]·L` seconds, 29.5 + 0.5·L, keyed by 1018.
  The effect's adder (`0x582D50`) sets `+0x200` bit `0x200000`, makes the item of type 3809 (`SeraBFG.GRN`, of class 5 and subclass 24) unless the one it made before lives (`+0x263`), and puts it in her right hand (`0x555E00`, the slot 13): a weapon of her own for the effect's time (what becomes of her items as it ends, not followed).
  **Seen** 2026-10-24 in the running original (the driver of #448, the Seraphim of level 1 after `load start-seraphim`, `arts learn 1018`, `arts select 1018` and the right button on an immortal goblin warrior beside her, a trace of `0x582D50`): the effect 16 of the whole value 1 for 30 s, the weapon's glow turned on, then her damage 35 to 42 physical and 8 to 15 magic, her attack speed 100 (154 before), and one click's strike took the goblin's 52 health whole.
- **1019 (`GUI_SPELL_ENERGIEBLITZE`, the launch type 5).**
  The first run plays the clip and sets `+0x200` bit `0x10`.
  Once `left` is below 0.35 it gives the Seraphim the effect 17 of the whole value `L` for 29.5 + 0.5·L seconds, ends, starts the regeneration, and makes an object of type 844 (`FX_SERA_SM_CAST`) at her place with its level, `1000 · (29.5 + 0.5·L)` (milliseconds) and her four damages' multipliers, each the sum of the stats' `+0x96` and `+0xB6`, `+0x9A` and `+0xBA`, `+0x9E` and `+0xBE`, `+0xA2` and `+0xC2`, with the effect `0x43` twice.
  The effect 17's adder sets `+0x200` bit `0x400000` and makes an object of type 856 (`FX_GENERIC`) with the effect `0x50` for each item in her hands (**hypothesis**: the weapons' glow); the bit `0x400000` is the one by which each of her shots makes 2 arrows (docs/re/fight.md, "At range"), and what the object 844 does is not followed.
  **Seen** as 1018: the effect 17 of the whole value 1 for 30 s.
- **1020 (`GUI_MOVE_WIRBELSCHLAG`)** runs as her `Rundum` 1069 ("A `Rundum`'s run" above), with the radius `[+0x60] + [+0x64]·L`, 160 + 5·L per cent of the reach, and the factor 0.7 + 0.05·L; refused with a bow or a crossbow, it sets `+0x1F4` bit `0x800000` while it runs (cleared at its end with `0x450630` and `0x44DDC0`, not followed).
  Each creature it strikes is knocked back as by a kick, by a roll of 0 to 99 below `0x4FC9D0(L, target)` ("The kicks" above): the event of kind 3 from the Seraphim with the whole value `[+0x58] + [+0x5C]·L`, 170 + 5·L.
  On foot it plays the slot 149 at the blend 0.5 and the rate 1.0 (`0x50619A`; the case of the art's switch at `0x505B67`, by the bytes at `0x5066D0`), not her `Rundum`'s 153 at the attack speed.
  **Seen** 2026-10-24 in the running original (the driver of #448, the Seraphim of level 1 after `load start-seraphim` moved to 3250,2511, `arts learn 1020`, four goblin warriors of level 5 made on the four cells beside her, which stepped back to two cells, and two creatures of type 313 come by; the right button on one, the trace `strike`): twelve strikes of 17 to 19 in two waves, six from 54.67 s to 55.31 s, a tick apart and each on another creature, and six from 58.06 s to 58.99 s, each creature struck again 2.9 to 3.8 s after its first strike, once it had come back from its knockback.
  `opensacred-game` makes it (#754, `opensacred_rules::arts::Style::Whirl`, `whirl_knockback`): as her `Rundum`, its regeneration started as it starts, and each creature struck knocked back (`making::Knocked`) by the kicks' roll the distance 170 + 5·L; the bit `0x800000` and the second clip of `0x5491F0` are left out.
  On the same goblins the game's Seraphim struck as often per wave, 17 or 18 each, but in four waves within 4.7 s, its knocked creatures coming back 0.8 to 1.2 s after their strike, until #773 held them for 3 s as the original does ("The kicks" above).
- **1021 (`GUI_MOVE_DOPPELGAENGER`)**, of no class, only ends and starts its regeneration here.

### The Gladiator's

The Gladiator's executor (`0x4FF0B0`) has cases for 1003, 1010, 1012, 1013 and 1054 together, 1014, 1015, 1016, and his `Rundum`, `Hardhit`, `Attacke` and kick (above); his own are 1003, 1010, 1012, 1013, 1014, 1015 and 1016 (the classes 2).

- **The jump 1003 (`GUI_MOVE_KAMPFSPRUNG`)** starts as the Seraphim's; once `left` is below 0.8 it flies to its target's place, and once it is below 0.6 it lands, ends and starts its regeneration: a weather event of kind `0x2D` (**hypothesis**: the ground shaking), and each creature of the hero's list (`0xAD3CE0`) within `L·[+0x54] + [+0x50]` of it (190 + 10·L) that `0x548F60` admits takes a strike of the factor `L·[+0x5C] + [+0x58]` (1.8 + 0.2·L, as "The second table" above has it).
  By a roll of 0 to 99 below the kicks' chance (`0x4FC9D0`), a creature that lives is stunned: `+0x200` bit `0x40000`, its timer (`+0x208`) set back by its level plus 3 seconds, at least 2 (**hypothesis**: the time it stands), an object made with an effect, and the event told to the network.
  Read again 2026-10-24 (#755): on foot the first run plays the slot 144 at the blend 0.15 and at the Seraphim's jump's rate by the distance (`0x502619`); the flight is hers (`0x44DED0` with the time a quarter of the clip's, at `0x5006FB`), so that the landing at 0.6 falls before the flight's end at 0.45 of the clip; the stun first ends what the creature does (`0x54E6D0`: the AI's state 0, the action 0) and holds it as a knockback does ("The kicks" above), without a slide; the strikes' distances are `0x5370B0`'s from the Gladiator as he lands, and the weather's event is of kind 7 with `0x2D`.
  **Seen** 2026-10-24 in the running original (the driver of #448, the Gladiator's new game at 3250,2511, `arts learn 1003`, four immortal goblin warriors of level 1 made four to six cells off, which came on, and the right button on the farthest): he flew three cells and struck five creatures as he landed, and the four goblins stood, their AI's state 0.0 and their action 0.1, until about 4 s after (their level 1 and 3), then took up the fight.
  `opensacred-game` makes it (#755, `opensacred_rules::arts::Style::Jump` with `jump_points`, `jump_quake` and `jump_stun`, `opensacred_rules::minds::Mind::stunned`): as the Seraphim's jump, its points 0.8 and 0.6, and as he lands each hostile of the hero's look (`crowd::Nearest`) nearer him than 190 + 10·L struck with 1.8 + 0.2·L and, by the kicks' roll, stunned for its level and 3 seconds, at least 2; the weather's event, the objects and the network's are left out.
  On the same goblins the game's Gladiator struck five creatures as he landed, and the goblins stood until about 4 s after.
- **1010 (`GUI_MOVE_RUECKENBRECHER`, the distance 80)** takes its target up.
  Its first run admits a target of the kinds 5, 7, 8, 12 or 14 (`0x43ADC0`) that is not of the big types (`0x426CB0`), else ends (the AI's state 0); it keeps the item of its right hand (`+0x1D8` at `+0x17C`), holds the target there instead, fastened to its own `Bip01 Neck`, else its `Bip01 R Hand`, the target's `+0x14` bit `0x1000` and `+0x200` bit `0x40000` set and its timer set back by `2·L' + 3 − M` seconds held from 5 to 10 (`L'` the Gladiator's level `+0x3FE`, `M` the target's).
  Once `left` is below 0.5 (0.7 with a weapon of the classes 2 and 3) it lets the target go: the right hand empty again, the target put down by its bone's point and thrown (`0x44DED0`), with a strike of the hit's kind 1 and the factor `1 + 0.1·L` (`0x4D1EB0` gives the same; the record's `+0x50` and `+0x54`, 70 and 6, are not read here); unless a roll of 0 to 99 falls below `100·L/M` (times 0.769 for a boss), the target's timer comes back within 0.5 s.
  Then the art ends and the interface's taskbar is told.
  **Seen** 2026-10-24 in the running original (the driver of #448, the Gladiator's new game at 3250,2511, `arts learn 1010` and the right button on a goblin warrior of level 1 two cells east, traces of `0x593320`, `0x44DED0` and `strike`): he came within a cell of it and played the slot 202 (`GLAD_SPECIAL_HHIT_1H`, 1.98 s) at the blend 0.5 and the rate 1.0, his one-handed weapon in hand; the goblin stood from about 0.5 s after the click to 5.9 s, its AI's state 0.0 and its action 0.1 (`2·L' + 3 − M` held at 5); 0.88 s into the clip, about 0.56 of it at 1.25, it was put down by `0x44DED0` at his cell and struck, and the regeneration of 14.3 s had started at the click.
  `opensacred-game` makes it (#755, `opensacred_rules::arts::Style::Backbreaker`, `backbreaker_hold`, `backbreaker_lets_go`, `backbreaker_factor`, `backbreaker_holds`, `LET_GO`): made from within 10 of the distance 80 (**hypothesis**: the record's distances of 1 to 99 are the distance itself, `made_from`), his `Hardhit`'s slot at the rate 1.0, the target stunned as the clip starts for `2·L' + 3 − M` seconds from 5 to 10 and the regeneration started; below 0.5 left (0.7 with a weapon of the classes 2 and 3) a strike of `1 + 0.1·L`, and unless the roll keeps it held its stun ends within 0.5 s; the carrying at his neck, the throw's flight and the admission of the target's kind are left out.
  In the game the Gladiator of level 1 held a goblin warrior from about 0.9 s to 5.7 s after the command and struck it for 38 as he let it go.
- **1012 (`GUI_MOVE_HANDGOTTES`)**, once `left` is below 0.35: a strike at its target of the factor `L·[+0x54] + [+0x50]`, 3.6 + 0.4·L, and the same strike at every other creature of the hero's list (`0xAD3CE0`) nearer the target than the reach of the target and the Gladiator (`0x5370B0` and `0x4DBC60` of the target, `0x4FF9EF`), each with an effect; then it ends.
  On foot it plays the slot 146 at the blend 0.5 and the rate 1.0 (`0x502C10`).
  **Seen** 2026-10-24 in the running original (the Gladiator's new game, 1012 learnt and made by the right button on one of six goblin warriors beside him, the trace `strike`): the hero struck five at once, the target and the four within about a cell and a half of it, not the one two cells off it along each axis; goblins made by the driver's `spawn --hostile` were not struck beside the target until they were in the hero's list.
  `opensacred-game` makes it (#755, `opensacred_rules::arts::Style::Smite`, `smitten`): at 0.65 of its clip its target and the hostiles of the hero's look (`crowd::Nearest`) within the reach of the target, its regeneration started then; the effects at each are left out.
- **The hurled blades 1013 (`GUI_MOVE_SCHLEUDERKLINGEN`) and 1054 here** need an item in a hand that `0x427A40` admits, else end at once.
  Once `left` is below 0.6 the item leaves the hand (`0x556A30`) and flies (`0x44DED0`, `+0x14` bit `0x1000000`) to the point 200 beyond its target along the line from the Gladiator (750 from him for a creature that is no player), at the speed 55 (80 for a creature that is no player), the throw told to the network (**hypothesis**: the flying item strikes and comes back by its own update, not followed).
- **1014 (`GUI_MOVE_EHRFURCHT`, the launch type 2)**, at its first run of the sub-state 1: every creature about it (`0x5FEAF0`) but itself nearer the Gladiator than `L·[+0x54] + [+0x50]` truncated, 150 + 10·L, that `0x548F60` admits and whose id is above 16 gets `+0x24D` = `[+0x60] + [+0x68] − [+0x6C] + ([+0x64] + 1)·L`, 52 + 4·L, at least 10, and plays its hit clip (`0x593770`); by a roll of 0 to 99 below the kicks' chance it takes the event of kind 3 from the Gladiator with that value and is held for `[+0x58] + [+0x5C]·L` seconds, 3 + 0.35·L, at least (its timer, `+0x200` bit `0x40000`); each with an effect.
  So the Gladiator's roar knocks back and stuns the hostile creatures about him.
  Read again 2026-10-24 (#755): on foot it plays the slot 149 at the blend 0.5 and the rate 1.0 (`0x502C2F`; 169 mounted); its run (`0x4FFB75`) comes once what is left of the clip is below all of it, at its first run, about its target where `0x5490B0` admits it as a friend, else about the Gladiator himself; the distance is `0x5370B0`'s from that centre; the hit clip is the slot 175 at 3.0 with the blend 0.25; a creature knocked back is held to `[+0x58] + [+0x5C]·L` seconds where its timer would fire sooner (`+0x208` set to `+0x204` less that), beyond the event's own 3 s ("The kicks" above); each creature roared at gets an object of type 842 (`0x34A`); then its regeneration starts and its clip plays on.
  **Seen** 2026-10-24 in the running original (the driver of #448, the Gladiator's new game moved to 3250,2511, `arts learn 1014`, four goblin warriors of level 5 made two cells about him, and the right button on his head, clear of them): within 0.4 s of the click each goblin was a cell farther off and stood, its AI's state 0.0 and its action 0.1, until about 3.6 s, then walked back at the state 4.2.
  `opensacred-game` makes it (#755, `opensacred_rules::arts::Style::Roar`, `roar_radius`, `roar_knockback`, `roar_hold`): the right button on the hero himself, or `arts make`, and at the first frame of its clip each hostile creature nearer him than the radius knocked back by the kicks' roll (`making::Knocked` with the hold, `opensacred_rules::minds::Mind::knocked`), its regeneration started then; the hit clips and the objects 842 are left out.
  On the same goblins the game's were a cell farther off within 0.1 s and stood until about 3.4 s, then came at the hero again.
- **1015 (`GUI_MOVE_HELDENMUT`, the launch type 2)**, once `left` is below 0.3: its target where `0x5490B0` admits it, else the Gladiator himself, takes the effects 1 and 2 of the value `[+0x50] + [+0x54]·L`, 0.5 + 0.2·L, for `[+0x58] + [+0x5C]·L` seconds, 39.5 + 0.5·L, keyed by 1015: `A`'s and `D`'s multipliers raised ("The effects" below); then it ends, starts its regeneration, makes an effect `0x42` and calls `0x539E70` (the effect 34 of the art's time, above).
  On foot it plays the slot 151 at the blend 0.5 and the rate 1.0 (`0x502C5A`; 169 mounted).
  `opensacred-game` makes it as a boon (#755, `Style::Boon`): the right button on the hero himself, or `arts make`, and at 0.7 of its clip the effects 1 and 2 on him; on a hostile creature it is refused, as the original's; the hero of level 1 then had `A` 48 and `D` 38 for 40 s.
- **1016 (`GUI_MOVE_KLINGENBLICK`, the launch type 2)**, once `left` is below 0.3: its target or himself, as 1015, takes the effect 18 of a whole value, `[+0x50] + [+0x54]·L` (3 + 7·L) times one plus a hundredth of a curve of a value of the Gladiator's (`0x582450` with 2, through `0x565D70` and the balance's values at `0xAD44D0` to `0xAD44D8`) plus `L` over the value at `0xAD5630` (partly read), for 39.5 + 0.5·L seconds, keyed by 1016; then as 1015.
  The effect 18's adder sets `+0x200` bit `0x800000` (what it does in a fight, not followed).

### The Wood Elf's

The Wood Elf's executor (`0x50AC00`) has cases for 1003, 1004 to 1008, and her `Rundum`, `Hardhit` and `Attacke` (above, her kick the default's clip); her own are 1004 to 1008 (the classes 16).

- **The special arrows, 1004 (`GUI_MOVE_DUMDUM`), 1006 (`GUI_MOVE_SPINNENPFEIL`), 1007 (`GUI_MOVE_KEULEN`) and 1008 (`GUI_MOVE_KLINGEN`)**, at their first run: with a bow or a crossbow (the classes 7 and 12) a shot, the action 5 with `+0x184` 1 (its sub-state 4 for a creature of another player), its target or the action's point, the special projectile at `+0x18E`, and the regeneration started; without one, nothing.
  The projectile's byte is the art's low byte plus `0x18`, kept in a byte (`0x50C7F8`), so the art less 1000: 4, 6, 7 and 8 (read 2026-10-24, #761; read before as `0x1C` to `0x20`).
  The shot (docs/re/fight.md, "The archers") makes for them the types 3078, 3074, 3075 and 3077, each carrying the art's level in play (the entry's level and its bonus, `0x580A10`) at `+0x249`, at most 255.
  Their starts (`0x5BD2D0`) and hits (`0x5BB010`), read 2026-10-24 (#761), each hit's strike sent by the shooter's client alone:
  - 1004's 3078 lives 1.5 s, flies at 600, leaves a trail (the effect `0x26` every 40 units) and ends at its first hit; its strike is of the hit's kind 1 (the record's `+0x14`, its `+0x228`) with the factor `[+0x50] + [+0x54]·L` of 1004 (2.35 + 0.35·L; the record's `+0x1C`) and the record's `+0x18` 15 (not read).
    A creature of an id above 16 that the strike leaves dead bursts (`0x4E8DA0`: objects of type 800 for its parts and blood, the creature destroyed or put to its action 9), unless `0x426A80` and `0x549460` keep it (not read); one alive shows the effect `0x55`.
  - 1006's 3074 ends at its first hit with no strike: at its target `⌊([+0x60] + [+0x64]·L) / ([+0x68] + [+0x6C]·L)⌋` creatures of type 71, `⌊(10 + 130·L) / (50 + 20·L)⌋` (2 at level 1, 4 at 5, 5 at 10, 6 from 25), each of the level `min(5·L, M)` (`M` the local player's hero's level, the setter `0x564D60`), its `+0x1F0` 7 and `+0x1F4` bit `0x4000` (the guards' flag, docs/re/ai.md), sent the event `0x100` with the target's id (**hypothesis**: to attack it), and given the effect 29 of the value 15 for 15 s, keyed by nothing (not read; **hypothesis**: its time); the sample `0x816` and the effect `0x4F` at the target.
  - 1007's 3075 ends at its first hit: a target whose level is below `0.4·L' + 5·L` (`L'` the level of the hero of the client that made the arrow, `+0x39`, the shooter for a hero) and whose type a kick may knock back (`0x4FC9D0` above 0, no roll) takes the event 3, the knockback, of the whole value `[+0x60] + [+0x64]·L`, 15; then the strike of the hit's kind 1 with the shot's record otherwise, its factor 0, which scales nothing; the effect `0x55` at it.
  - 1008's 3077 homes (`+0x1E4` bit `0x10`): it flies at 400, two thirds of 600, turning toward its target each frame, its direction turned back where its next cell may not be walked on and its rise where it would sink below the ground; it lives `[+0x50] + [+0x54]·L` seconds (5.66 + 0.35·L), and ends when its target is gone or dead.
    It passes through, striking a creature it meets at most every 0.5 s with the hit's kind 1 and the factor `[+0x58] + [+0x5C]·L` (0.7 + 0.05·L), and after each strike takes another target (`0x600570`, not read); a trail as 1004's.
    The shot fans its `+0x25D` arrows round the circle, `2π/n` apart.
  What the event 3's knockback of 15 does to its target is the kicks' (above).
  **Seen** 2026-10-24 in the running original (the driver of #448, the Wood Elf's new game at 3240,2520, `arts learn` at 9 and the right button on goblin warriors of level 8): 1004's shot took 92 of a goblin's 149, 1007's 16, as a plain shot, with `0x4FC9D0` called at its hit.
  `opensacred-game` makes 1004 and 1007 (#756, `opensacred_rules::arts::Style::Arrow`, `club_knockback`; `opensacred_rules::arrows::projectile`, `fanned_for`, `club_knocks`): with a bow or a crossbow, the hero's shot at once at its target (`attack::Attack::shot`), at the next tick, its regeneration started; its arrows of the art's model (`ARROW_EXPLOSION.GRN`, `ARROW_LONG.GRN`), fanned by 0.15 rad with the multi-shot, each hit a strike of the art at its level in play with the art's factor (none for 1007), and 1007's first the knockback of 15 where the bound holds and a kick may move the creature; 1004's burst, the trails and the effects are left out, and 1006 waits.
  Read 2026-10-24 (#756): a homing arrow turns toward its target by at most 5 radians a second, `min(a, 5·dt)` of its angle `a` to it (the float `0x890754`), about the axis square to both, its speed kept; after each strike its next target is the nearest creature to the one it struck within a squared distance of 100,000 (about 316) that the shooter may fight, but its current target, else the current one (`0x600570`, at `0x5BC7EB`).
  **Seen** 2026-10-24 in the running original (the Wood Elf's new game at 3250,2511, 1008 learnt at level 1 and the right button on one of three goblin warriors of level 8, a trace of `strike`): the blade struck two of the goblins in turn, six times over about 5 s, for 11 to 13 each, the strikes 0.5 to 1.4 s apart, and never the third.
  `opensacred-game` makes 1008 too (`opensacred_rules::arts::blades`; `opensacred_rules::arrows::Arrow::homing`, `Arrow::home`, `BLADE_STRIKES_EVERY`, `BLADE_NEXT_WITHIN`): its blades of `ARROW_BLADE.GRN` fanned round the circle with the multi-shot, homing on the target's neck at 400 for 5.66 + 0.35·L seconds, turned back off the cells that may not be walked on and off the ground, gone when the target is gone or dead; they pass through, striking a creature they come on at most every 0.5 s with the factor 0.7 + 0.05·L, each strike taking the next target as above.
  In the game at level 1 the same blade flew 6.0 s and struck two of three goblins of level 8 in turn for 12 and 13, its strikes 0.5 to 0.7 s apart; at level 9 it flew 8.8 s and struck all three for 18 to 20.
  In the game at the open place of 3250,2511 the same shots took 86 and 16 of a goblin warrior of level 8, the second knocking it back by 15.
- **1005 (`GUI_MOVE_MULTIPFEIL`, the launch type 0)**, at its first run: the effect `0x15`, the multi-shot (docs/re/fight.md, "At range"), of the whole value `n = ⌊([+0x50] + [+0x54]·L) / ([+0x58] + [+0x5C]·L)⌋`, `⌊(50 + 144·L) / (77 + 20·L)⌋` (2 at level 1, 5 at 10, 6 from 18), for `[+0x60] + [+0x64]·L` seconds, 20, keyed by 1005, and `+0x261` the same `n`; the regeneration started, and an object of type 861 (`FX_ELVE_SM_CAST`) at her place with her level, the time in milliseconds and her damages' multipliers (as 1019's), with the effect `0x54`; then the art ends.
  So for 20 s each of her shots looses `n` arrows, each of `0.83^n` of its damage.
  Read again 2026-10-24 (#756): it is the case of the first run's switch (`0x50C585`, by the bytes at `0x50CCC8`) and plays no clip: `0x582D50(0x15, n, 0.0, 20, 1005, 1, 0, 0)`.
  **Seen** 2026-10-24 in the running original (the driver of #448, the Wood Elf's new game at 3250,2511, `arts learn 1005` and the right button on a goblin warrior, a trace of `0x582D50`, then a left click on it): the effect 21 of the whole value 2 and the value 0 for 20 s, and her shot struck the goblin twice within 2 ms.
  `opensacred-game` makes it (#756, `opensacred_rules::arts::Style::Volley`, `volley`; `opensacred_rules::arrows::volley`, `fanned`, `Arrow::turned_by`; `opensacred_rules::fight::Rules::multi_shot`): at once, without a clip, the effect 21 on the hero for 20 s and its regeneration; while it holds, each of the hero's shots (#786) looses its whole value of arrows, the `i`-th after the first turned by `⌈i/2⌉ · 0.05` rad to either side in turn, each hitting with `Multipfeil^n` of its share; the object 861 is left out.
  In the game the Wood Elf of level 1 then loosed two arrows a shot at a goblin warrior, each striking for 13, her 19 times 0.83².
- 1003, the jump, runs here for the creatures of the mark `0x20E` (which it turns into `0x210`): a flight to the target, the regeneration, and no strike.

### The Dark Elf's

The Dark Elf's executor (`0x506770`) has cases for 1042 to 1053 and his `Rundum`, `Hardhit`, `Attacke` and kick (above); all of them are his (the classes 8).
His arts of the launch types 0 and 5 play the clip 194 at the blend 0.2 when he is mounted or so (a flag of the order), else their own.

- **1042 (`GUI_MOVE_NATTER`, the distance 130)**, once `left` is below 0.2: its target, and every other creature of the hero's list within `[+0x50] + [+0x54]·L` (148 + 2·L) of it, each by a roll of 0 to 99 below the kicks' chance (`0x4FC9D0`): stopped (`0x54E6D0`), held for `[+0x58] + [+0x5C]·L` seconds (19.5 + 0.5·L; its timer and `+0x200` bit `0x40000`), the effect 35 for that time keyed by 1042 (what it does, not read), and an object of type 864 (`FX_DARK_SM_HIT`) at it; then the art ends.
  Read again 2026-10-24 (#757): on foot it plays the slot 148 at the blend 0.5 and the rate 1.0 (`0x509DEE`, the case of the art's switch at `0x509DE7`); its run is at `0x508C3B`, the others' loop at `0x508E5B` over the hero's list, their distance `0x5370B0`'s from the target; each stunned one's timer is set to its period less the seconds, and the effect 35 has the whole value and the value 0, keyed by 1042; the others' timers take the target's period, not their own (`0x508F59`), so that one of another period is held that much longer or shorter.
  **Seen** 2026-10-24 in the running original (the driver of #448, the Dark Elf's new game at 3250,2511, `arts learn 1042`, four goblin warriors of level 1 made two cells about him, and the right button on the one at 3252,2511): about two seconds after the click that goblin stood, its AI's state 0.0 and its action 0.1, and still stood 11 s later, while the three others, farther than 150 from it, fought on.
  `opensacred-game` makes it (#757, `opensacred_rules::arts::Style::Viper`, `viper`, `VIPER_LEFT`; `opensacred_rules::minds::Mind::stunned`): at 0.8 of its clip its target and each hostile of the hero's look within the radius of the target, by the kicks' roll, stunned for 19.5 + 0.5·L seconds with the effect 35 (`effects::Effects`), each held by its own period; then it ends; the objects 864 are left out.
  On the same setup the game's Dark Elf, its target the creature beside the goblin at 3252,2511, stunned it and the two goblins within 150 of it for 20 s, the two farther fighting on.
- **1043 (`GUI_MOVE_PANTHER`, the launch type 3)**: its first run takes the action's point, brought to `[+0x50] + [+0x54]·L` (400 + 50·L, truncated) from him where it lies farther, or where the middle button is held, and refuses a point it may not stand on; once `left` is below 0.7 he leaps there (`0x44DED0`), and the art ends.
  Read again 2026-10-24 (#757): on foot it plays the slot 182 at the blend 0.5 (`0x509E7B`), at a rate by the reach `r` (`0x509FF5`): 5 below 10, `6 − 0.0101·r` up to 500, else 1 (1.45 at level 1); the point at the reach is the line's, moved by `(to − from)·r/d` in whole units; once `left` is below 0.7 (`0x507DAA`) the flight takes half the clip's time and rises to a twelfth of the distance (`0x44DED0(x, y, 0, −1000, d/12, t)`, its step `0x44DFE0` a straight line under the parabola `h·(1 − 4(s − 0.5)²)`), and the regeneration starts.
  **Seen** 2026-10-24 in the running original (the driver of #448, the Dark Elf's new game at 3250,2511, `arts learn 1043` and the right button on the ground eight cells east): he stood at 3252 at 0.62 s, 3255 at 0.89 s and 3258 at 1.14 s, his AI's state 11.1 and his action 10.1 throughout, then 0.0 by 1.65 s, and the art's regeneration of 5.4 s ran.
  `opensacred-game` makes it (#757, `opensacred_rules::arts::Style::Panther`, `panther_reach`, `panther_rate`, `panther_point`, `PANTHER_LEAPS`, `PANTHER_FLIGHT`): the right button on the ground takes the point where the cursor's line meets the hero's height, `arts make` the nearest hostile's place; refused where the hero may not stand; the clip 182 at that rate, and below 0.7 left a flight (the `Leap` of 1003) to the point in half the clip's time, the regeneration started, and the art ends.
  On the same setup the game's Dark Elf, `arts make` on a goblin warrior seven cells east, leapt from 3250 to 3257 between about 0.6 s and 1.3 s.
- **1044 (`GUI_MOVE_SOULCATCHER`, the launch type 5)**, at once: the regeneration, an object of type 869 (`FX_SOULCATCHER`) at his place with his level, his damages' multipliers, 1.0 and a value of his (`0x4D2310`), with an effect, and `0x539E70` with 1044; the art casts the spell 47 ("The second table" above).
- **1045 (`GUI_MOVE_ARROWPROOF`) and 1046 (`GUI_MOVE_SPELLPROOF`), the launch type 2**, once `left` is below 0.2: on its target where `0x426610` admits it, else on himself, an object of type 883 (`FX_FERNUMLEITUNG`) or 870 (`FX_MAGIEUMLEITUNG`) with the time `[+0x50] + [+0x54]·L` (44 + L), and the effect `0x19` or `0x1A` of the value `[+0x58] + [+0x5C]·L` (1 + 0.05·L) for 44 + L seconds, keyed by the art: the deflection of arrows (`0x19`, docs/re/fight.md, "At range") and, by its object's name, of spells (`0x1A`, not read); then `0x539E70`.
  Read again 2026-10-24 (#757): on foot 1045 plays the slot 146 and 1046 the slot 147, at the blend 0.5 and the rate 1.0 (`0x509E28`, `0x509E62`; 194 at the blend 0.2 at the attack's rate, mounted); 1045's run (`0x508561`) adds the effect with `0x582D50(0x19, the object's id, 1 + 0.05·L, 44 + L, 1045, 0, L, 0)`, its level the deflection's `v`.
  `opensacred-game` makes 1045 (#757, `opensacred_rules::arts::Style::Ward`, `ward`): the right button on the hero himself, or `arts make`, and at 0.8 of its clip the effect `0x19` of 1 + 0.05·L with the level `L` for 44 + L seconds on him (`effects::Effects`), its regeneration started then; the object 883, its whole value and the effect 34 are left out, and 1046, whose effect `0x1A` the game reads nowhere, waits.
- **1047 (`GUI_MOVE_TESTOSTERON`) and 1048 (`GUI_MOVE_ADRENALIN`), the launch type 5**, once `left` is below 0.3 (0.2 for 1048): on himself the effect 1 (`A`'s multiplier; 1048 the effect 2, `D`'s) of `[+0x50] + [+0x54]·L` (0.3 + 0.2·L) and the effect 6 (the poison damage's multiplier; 1048 the effect 12, the regeneration) of `[+0x58] + [+0x5C]·L` (0.2 + 0.05·L), each for `[+0x60] + [+0x64]·L` seconds (29 + L), keyed by the art; the regeneration, two objects of type 864 with the effect `0x59`, and `0x539E70`.
  On foot they play the slot 180 at the blend 0.5 and the rate 1.0 (`0x50A09F`).
  **Seen** 2026-10-24 in the running original (the driver of #448, the Dark Elf's new game, `arts learn 1047`, `arts select 1047` and the right button on a goblin beside him, a trace of `0x582D50`): the effects 1 of 0.5 and 6 of 0.25 for 30 s keyed by 1047, then the effect 34 for 30 s; his `A` 39 times 1.5 and his poison damage's multiplier 1.25, his weapon arts' regeneration 30 s.
  `opensacred-game` makes them (#757, `opensacred_rules::arts::Style::Boon` and `boon`): the right button on a hostile creature, or `arts make`, makes the selected boon on the hero where it stands, its clip the slot 180 at the rate 1.0, and at its point its effects (`effects::Effects`, #763) and its regeneration; the objects 864, the effect 34 and the mount's clip are left out.
  In the game the Dark Elf of level 1 then had `A` 58 and a poison damage of 15.4 to 23.4, as the original's, and the regeneration of 30 s; each is refused while its own effects hold (`0x51D840`, above; #759).
- **The traps 1049 (`GUI_MOVE_BOMB`), 1050 (`GUI_MOVE_HELL`), 1051 (`GUI_MOVE_CONFUSION`), 1052 (`GUI_MOVE_VENOM`) and 1053 (`GUI_MOVE_FOG`), the launch type 0**, once `left` is below 0.7: an object of type 867 (`FX_BALL`) at his place, carrying his level, his damages' multipliers, 1.0, a value of his (`0x4D2310`), the art and the action's point, which it flies to (**hypothesis**: the thrown trap, which casts the art's spell, 45, 6, 48, 12 and 46, "The second table" above; not followed); then the art ends.

### The Vampiress's

The Vampiress's executor (`0x50E6C0`) has cases for 1022 to 1041 and 1073 to 1076, all hers (the classes 32); her claws' arts (1038 to 1041) and her day's (1073 to 1076) share theirs (above).
It was read more coarsely than the others: where the arguments of a call were not taken from the disassembly, this says so.

- **1022 (`GUI_MOVE_CHANGELING_DAY`) and 1023 (`GUI_MOVE_CHANGELING`), the launch type 0**, once `left` is below 0.5: 1022 in her day's form (`+0x14` bit `0x2000` clear) sends herself the event of her change and, for the local player, gives her the effect 22 for the art's factor in seconds (`0x51CC20`, `[+0x50] + [+0x54]·L`, 24 + 6·L), doubled where `0x41BA60` says no and times 1.4 and a value of `0x51CEF0` where her `+0x4F0` has bit `0x400000`; 1023 in her night's form sends the event back and takes every effect 22 off her; each ends and starts its regeneration (1023 only by day).
  So the effect 22 is her night's form, which ends with it.
  Read again 2026-10-24 (#758): `0x41BA60` is the renderer's test for the day (05:20 to 20:20, docs/re/daynight.md, "The phases"), so the time is doubled by night.
  **Seen** 2026-10-24 in the running original (the driver of #448, the Vampiress's new game at 3250,2511 at 08:03, 1022 by the right button on the ground beside her, traces of `0x593320` and `0x582D50`, `stats` read every 0.4 s):
  - 1022 played the slot 144 (`VMPD_ATTACK_SPECIAL01`, 4.23 s) at the blend 0.25 and the rate 0.7, and 2.44 s in, at half the clip at the speed 1.25 · 0.7, she was the type 7: the effect 22 of the whole value 0 and the value 0 for 30 s, keyed by no art, and 1022's regeneration of 30 s started;
  - her attributes became the type 7's record's base, 33, 22, 21, 18, 0, 26, with `A` 36 and `D` 34.4, her damage's multipliers by them, the most health 135 with the current kept (129), and her model `VLADY_N.GRN`, her weapons not drawn, fighting with her claws;
  - her stats kept their type 6 (`+0x08`, the recomputation's probe at `0x5797F0`, #792), by which `0x5796A0` takes the record's bonuses (at `0x5797FD`), so the type 7's resistance bonuses did not count: her resistances 5.818, 1.235, 1 and 1.92 before their multipliers, her first by her fourth attribute alone;
  - her attack speed, `+0x3E` and pace were 15 lower than by day, 139, 105 and 115, and stayed so through a recomputation by night: the recomputation takes 15 off the three after the skills where the creature's `+0x14` has bit `0x2000`, her night form (`0x5796A0` at `0x57CA23`; probed 2026-10-24, #792: 154, 120 and 130 after the skills, 139, 105 and 115 at the end); the effect 22 itself has no case in its loop over the effects;
  - 1023, made then the same way, played the slot 144 of the night's model at the blend 0.25 and the rate 0.7, and about 2.4 s in she was the type 6 again with her day's values, the most health 129 and the current held within it, and 1022's regeneration of 30 s started anew;
  - left alone, she was the type 7 still 32 s after the click and the type 6 with her day's values 34 s after it: her night's form ends with its effect's 30 s.
  `opensacred-game` makes them (#758, `opensacred_rules::arts::Style::Changeling`, `changeling_seconds`, `in_form`, `CHANGES`, `CHANGELING_RATE`; the module `forms`): the right button anywhere, or `arts make`, makes 1022 by day and 1023 by night where she stands (each refused in the other form, `0x51D840`), the slot 144 at the rate 0.7, and at half the clip her change: the type 7's record's base attributes with the rest of her day's record (#792), her most health made anew with the current kept, her pace and speeds, her model `VLADY_N.GRN` and its clips for her claws (the class 0) without her weapons, her arts' clips by night, her light doubled (`0x54A7B0`), and the effect 22 for 24 + 6·L seconds, doubled by night; 1023, or the effect's end (seen), gives her day's form back and takes the effect 22 off; 1022's regeneration starts at each change, 1023's own by day; the factor of the `+0x4F0` bit `0x400000` is left out.
  In the game the Vampiress of level 1 then had the original's attributes, `A`, `D`, damage, resistances and health by night and back by day, her claws bare by night and her sword in hand again by day; her attack speed 139 by night, 154 by day (`opensacred_rules::fight::Base::night`, `NIGHT_SLOWS`, #792).
- **1024 (`GUI_MOVE_BEHERRSCHUNG_DAY`) and 1025 (`GUI_MOVE_BEHERRSCHUNG`)**, once `left` is below 0.6: a target neither a boss (`+0x1F4` bit `0x80000`) nor of `+0x2B8` bit `0x40` is stopped and takes the effect 13 for the art's factor in seconds (17 + 3·L and 21 + 4·L), times `5L/M` where its level `M` is above `5L`, with an object and an effect at it: the effect 13 sets its `+0x1F4` bit `0x20000` (**hypothesis**: it is dominated and fights on her side for that time; the effect's whole value not read).
  **Seen** 2026-10-24 in the running original (the driver of #448, the Vampiress's new game at 3250,2511 by day, `arts learn 1024`, two goblin warriors of level 1 beside her and the right button on one, traces of `0x593320`, `0x582D50` and `strike`): 1024 played the slot 149 at the blend 0.5 and the rate 1.0; 1.77 s into it (0.41 of its 4.29 s) the goblin took the effect 13 of the whole value 1 for 20 s, keyed by no art, and from then on never struck her, its AI's state 4.0 with its action 0.1, while the other goblin struck on; it struck the other goblin neither, and the regeneration of 32 s had started at the click.
  So the dominated creature leaves her alone for the effect's time (that it fights on her side was not seen).
  `opensacred-game` makes them (#758, `opensacred_rules::arts::Style::Dominate`, `domination_seconds`, `DOMINATES`, `DOMINATED`): the slot 149 at the rate 1.0 (by night a **hypothesis**), the regeneration started as it starts, and below 0.6 left its target, hostile and no boss nor champion, stopped and given the effect 13 for its seconds; a creature under the effect 13 does not see her for a foe (`ai`'s thought) nor pursue her; the object and its effect at it are left out.
  In the game a goblin warrior of level 1 dominated by the Vampiress of level 1 struck her no more, her health rising by her regeneration alone.
- **1026 (`GUI_MOVE_WOLFSRUF_DAY`) and 1027 (`GUI_MOVE_WOLFSRUF`), the launch type 3**, once `left` is below 0.6: the first point 20 to 200 from her, in steps of 20, that a straight way reaches (`0x4FCC80`) gets a creature made there with an effect, its level and values by hers (**hypothesis**: the wolves she calls, as many as the factor, 4 and 6; their type and number not read).
- **1028 (`GUI_MOVE_SPEED_DAY`) and 1029 (`GUI_MOVE_SPEED`), the launch type 0**, once `left` is below 0.8: she dashes along a straight line of cells to the action's point, an effect every 30 cells, as far as she may stand within `[+0x60] + [+0x64]·L` units of her line (400 + 25·L and 500 + 50·L), and is put there; then the regeneration, the ground's shaking and its sound, as at the jumps' landing.
  **Seen** 2026-10-24 in the running original (the driver of #448, the Vampiress's new game at 3250,2511, `arts learn 1028` and the right button on the ground east of her, a trace of `0x593320`): 1028 played the slot 144 at the blend 0.5 and the rate 5, her AI's state 11.1 and her action 13.1; a click 6 cells off put her there, one 12 cells off 8 cells off (her reach of 425 at level 1, about 429), each within 0.36 s of the click, and the regeneration of 7 s started.
  `opensacred-game` makes them (#758, `opensacred_rules::arts::Style::Dash`, `dash_reach`, `DASHES`, `DASH_RATE`): the right button on the ground, or `arts make` toward the nearest hostile, the point brought to the reach as the panther's (`panther_point`), and back along the line a cell at a time to the farthest where she may stand with a straight way; the slot 144 at the rate 5, and below 0.8 left she is put there (`hero::Body::teleport`) and the regeneration starts; the effects on her way and the ground's shaking are left out.
  In the game a goblin 12 cells off had her put 8 cells off within 0.35 s, as the original's.
- **1030 (`GUI_MOVE_BLUTBISS`)**, once `left` is below 0.9: where the points of three bones of hers touch the target (`0x44CA30`, `0x604030`), an effect, the effect 15 on it for 4.4 s keyed by 1030 (it is held, as the spells' action 7's), an event of her own with a value, and the event `0x406` to the target with a value held at 65,534 (**hypothesis**: the bite takes health and gives it to her by the factor, 10 + 20·L).
  **Seen** 2026-10-24 in the running original (the driver of #448, the Vampiress by night at 3250,2511, her health set to 60, `arts learn 1030` and the right button on a goblin warrior of level 1 beside her, traces of `0x593320`, `0x582D50` and `strike`): 1030 played the slot 144 at the blend 0.5 and the rate 1.0; 0.44 s into it the goblin took the effect 15 for 4.4 s keyed by 1030 and stood frozen in its strike for about 4.4 s, the trace `strike` had her strike it for 25 (52 to 27), and her health rose from 60 to 90.
  So the bite drains its factor: her health rises by it, and the target is struck with it as a fixed physical damage through its resistance (30 to 25, as the claw jump's 15 to 10).
  `opensacred-game` makes it (#758, `opensacred_rules::arts::Style::Bite`, `bite_drain`, `BITES`, `BITE_HOLD`; `fight::Hit::fixed`): by night, the slot 144 at the rate 1.0, and below 0.9 left its target stunned for 4.4 s with the effect 15, struck with the fixed physical damage `10 + 20·L`, and her health raised by as much; the touch of her bones, her own event's effect and the effect 30 that came with it are left out.
  In the game the Vampiress of level 1 at 60 health went to 91 and the goblin from 52 to 28, frozen about 4.4 s.
- **1031 (`GUI_MOVE_MEISTERBISS`)**, once `left` is below 0.6: a target of no type of `0x426B70`, `0x426B20` or `0x426CB0`, whose level (a boss's scaled) is below `0.4·L' + 5·L` (`L'` her level), is taken by `0x5FE870`, with three effects, and she takes the effect 14 for `[+0x58] + [+0x5C]·L` seconds (12 + 3·L) keyed by 1031 (**hypothesis**: the bite kills a weak target outright).
  `0x5FE870` is the object manager's lookup of an object by its id (`cObjectManager::getData`), which takes nothing by itself.
  **Seen** 2026-10-24 in the running original (the driver of #448, the Vampiress by night at 3250,2511, `arts learn 1031` and the right button on a goblin warrior of level 1 beside her, traces of `0x593320`, `0x582D50`, `strike` and `death`): 1031 played the slot 154 at the blend 0.5 and the rate 1.0; 0.85 s into it she took the effect 14 of the whole value 1 for 15 s keyed by 1031, and the goblin, alive and unhurt, was no longer hostile to her (`0x548F60`), its AI's state 2.0 (a behaviour), and stayed so for 6 s at least; no strike and no death came.
  So the bite turns a weak creature to her side rather than killing it (what side, and what it then does, not read); the game, which has no followers yet, leaves 1031 out.
- **The jump 1032 (`GUI_MOVE_KLAUENSPRUNG`)** flies to its target, brought nearer by half the two creatures' reach, and ends; its landing's events are "The second table"'s above.
  Read again 2026-10-24 (#758): each creature that the landing takes (`0x512F6F`) is, by the kicks' roll (`0x4FC9D0`), stopped (`0x54E6D0`) and held, its timer set back by the float 3.0 (`0x890768`) and its `+0x200` bit `0x40000` set, and sent the event 5 (`0x51CB00`, the strike's event, docs/re/objects.md) whose record has the value `5 + 10·L` as its first fixed damage, the physical, and nothing else of a weapon.
  **Seen** 2026-10-24 in the running original (the driver of #448, the Vampiress by night at 3250,2511, `arts learn 1032`, four goblin warriors of level 1 about 4 cells east and the right button on the nearest, traces of `0x593320` and `0x44DED0`): she played the slot 153 at the blend 0.5 and the rate 3, flew 0.2 s into it (`0x44DED0` toward the target's place, the height a twelfth of 121, the time 0.8 s), and as she landed each goblin about her lost 10 of its 52, without a strike of the trace `strike`, and stood about 3 s, its AI's state 0.0 and its action 0.1.
  So the landing strikes each of them for `5 + 10·L` physical through its resistance (15 to 10).
  `opensacred-game` makes it (#758, `Style::Jump` with `jump_points` of the night form, 0.8 and 0.35, `CLAW_JUMP_RATE`, `CLAW_HOLD`, `claw_quake`; `fight::Hit::fixed`): the slot 153 at the rate 3, the flight as the other jumps', and as she lands each hostile of her look nearer her than `190 + 10·L` beyond the two's reach stunned for 3 s by the kicks' roll and struck with the fixed physical damage `5 + 10·L`; why her rate is 3 where the others' rule by the distance gives 4 is not read.
  In the game four goblin warriors of level 1 each lost 10 and stood until about 3 s after her landing, as the original's.
- **1033 (`GUI_MOVE_TODESKUSS`)**, once `left` is below 0.6: an object at a bone of hers, with her level, her damages' multipliers and the values `[+0x50] + [+0x54]·L` (1 + 0.1·L) and `[+0x58] + [+0x5C]·L` (8 + 0.5·L), with an effect (**hypothesis**: a missile at the target).
  **Seen** 2026-10-24 in the running original (the driver of #448, the Vampiress by night at 3250,2511, `arts learn 1033` and the right button on a goblin warrior of level 1 come up beside her, traces of `0x593320`, `0x582D50` and `strike`): 1033 played the slot 148 at the blend 0.5 and the rate 1.0; 1.84 s into it the goblin took, each for 5 s keyed by 1033 with the level 1, the effects 2 and 1 of −0.909 (its `D` and `A` multiplied by 1 − 0.909, that is by `1/(1 + 0.1·L)`'s complement of 1) and the effect 23 of the value 11; then strikes of hers took 6 and 8 of its health about 0.9 s and 2 s later, while she stood, and its regeneration of 17.9 s had started.
  So the kiss curses its target: its attack and defence fall to a tenth for 5 s, and the effect 23 (`0x582D50` sets its creature's `+0x1F4` bit `0x4000000`) seems to strike it on her behalf (**hypothesis**; what the effect 23 does, and the missile's flight, not read).
  At the level 9, the same way: the effects 2 and 1 of −0.526 and 23 of the value 19, each for 5 s; her strikes then took 14, 13 and 13 of its health about 0.9, 1.9 and 2.85 s later.
  So the effects 1 and 2 have the value `−1/(L·[+0x54] + [+0x50])`, `−1/(1 + 0.1·L)` (−0.909 at 1, −0.526 at 9), and the effect 23 the value `10 + L` (11 and 19; whence, not read), with a strike of hers about each second while it holds.
  `opensacred-game` makes it (#758, `opensacred_rules::arts::Style::Kiss`, `curse`, `KISS_LANDS`, `CURSE_SECONDS`): by night, the slot 148 at the rate 1.0, the regeneration started as it starts, and at 0.58 of the clip (where it landed two cells off) its target's effects 1 and 2 of `−1/(1 + 0.1·L)` for 5 s; the missile, its flight and the effect 23's strikes are left out.
  In the game, at the level 9, a goblin warrior's `A` 18.5 and `D` 20.6 fell to 8 and 9 for 5 s, 1.85 s into the clip.
- **1034 (`GUI_MOVE_ERWECKUNG`, the launch type 4)**, once `left` is below 0.6, on a target of an id above 16: hired (`sdk_HIRE`) as her follower, its mark (`+0x25F`) cleared to its `0xF000` bits, `+0x1F4` bit `0x2000000` and `+0x1F0` 3, with an effect and the effect 29 (not read); then the regeneration.
- **The bats, 1035 (`GUI_MOVE_BAT_ATTACK`), 1036 (`GUI_MOVE_BAT_CONFUSE`) and 1037 (`GUI_MOVE_BAT_DEFEND`), the launch type 0**, once `left` is below 0.87, for the local player alone: her followers of the type 594 (1035), 595 (1036) and, by the pattern, 596 (1037) beyond the art's factor in number are dismissed, a creature of that type is made beside her, at most at the level `5·L`, hired as her follower with its flags `0x1200000` and its mark `0x100`, each with an effect, and the network told.

### The Dwarf's

The Dwarf's executor (`0x514790`) has cases for his `Rundum`, `Hardhit` and `Attacke` (1078 to 1080, above), 1082 to 1087 and 1089; the second table holds no 1081 nor 1088.
It was read as coarsely as the Vampiress's: what each case makes, and the arguments where they were taken.

- His `Rundum`, `Hardhit` and `Attacke` with a gun (the classes 13 and 14, **hypothesis**) shoot: each strike makes the objects 1172 (`FX_GUNFIRE`) at him and 1151 (`FX_DWARF_SHOOT_IMPACT`) at what it strikes, and 842 (`FX_GLAD_SM_HIT`) as the blades' strikes do.
- **1082 (`gui_moves_abschlag`, the launch type 5)**, once `left` is below 0.9: on himself the effect 47 for `[+0x50] + [+0x54]·L` seconds (39 + L) keyed by 1082, its value from `[+0x60]` (5; its whole value not taken), and an object of type 1019 (`FX_KRIEGSRAUSCH`) at him with the art's factor and the effect `0x98`; then it ends (what the effect 47 does, not read).
  **Seen** 2026-10-24 in the running original (the driver of #448, the Dwarf's new game at 3250,2511, `arts learn 1082 4` and the right button on an immortal goblin warrior beside him, a trace of `0x582D50`): the effect 47 of the whole value 36 and the value 5 for 43 s keyed by 1082, so its whole value is `[+0x58] + [+0x5C]·L` (28 + 2·L) and its time 39 + L; his attack speed `+0x3C` stayed 140.
  With it the attack speed's getter (below) adds the creature's float `+0x584` (probed at `0x585011` while he struck the goblin about every 0.6 s): 0 until his first strike, 12 after it, then falling between his strikes by about 2 to 4 a second and rising at each, from 10.5 to 18.7, 16.7 to 22.8, and so on, so that it hovered between 20 and 28 over 14 s of strikes; so his attack speed rose from 140 toward 165 as he fought.
  What raises and lowers the float, and whether the effect's whole value 36 holds it, is not read.
- **1083 (`gui_moves_wucht`)**, at its clip's hit point: a move through the air (`0x44DED0`), an object of type 816 (`FX_BURNINGLIGHT`) with the effect `0x2B`, an effect added, and a strike of the factor `L·[+0x5C] + [+0x58]` (0.9 + 0.1·L, "The second table" above; not followed further).
- **1084 (`gui_moves_kriegsrausch`)** strikes at its clip's hit points, each with an object 842 and the effect `0x42`, its target and the creatures about it (**hypothesis**, by `[+0x60] + [+0x64]·L`, 100 + 10·L; not followed).
- 1083 and 1084 are the only arts whose records have the flag 4, "with a two-handed weapon only" (`+0x74`, the first table's `+0x90` bit 2, `UI_STAT_2H_ONLY`).
  Seen 2026-10-24 in the running original (the driver of #448, the Dwarf's new game, 1084 learnt and the right button on a goblin warrior beside him, twice): it was not made with his starting weapon, its regeneration not started.
- **The guns 1085 (`gui_spells_flammenwerfer`), 1086 (`gui_spells_moerser`) and 1087 (`gui_spells_kanone`), the launch type 0**, make their objects as their clips run: 1085 the flames (1016, `FX_DWARF_FLAMETHROWER`, the effect `0x94`) about from 0.65 of the clip left to 0.2, 1086 a grenade (1018, `FX_DWARF_GRENADE`, the effect `0x96`) thrown to the point, 1087 a shot (1017, `FX_DWARF_SHOOT`, the effect `0x95`) at a point of the clip that comes earlier the farther the target (`0.5 − 0.0002·d` of it left, **hypothesis**: `d` the distance), and the impact 1151 with the effect `0x9C` at each creature it strikes; the casts' spells and their values are the first table's ("The second table" above; not followed).
- **1089 (`gui_moves_kriegsschrei`, the launch type 5)**, once `left` is below 0.2: its target, else himself, where of a hero's class (`0x426610`) and without an effect of 1089 (`0x582B00`), takes the effects 1 and 2 of `[+0x50] + [+0x54]·L` and `[+0x58] + [+0x5C]·L` (0.4 + 0.1·L each) for `([+0x60] + [+0x64]·L) / 100` seconds (29.5 + 0.5·L) keyed by 1089 (with one, `0x5904E0` instead), and the object 1150 (`FX_KRIEGSSCHREI_SIGN`) with the effect `0x9B`; then each follower of his (`+0x39C`) within 500 takes the object, and one of a hero's class the effects at half their values, and so does each other player's hero within 500.
  So his war cry raises his and his party's attack and defence.
  On foot it plays the slot 154 at the blend 0.5 and the rate 1.0, with an object 1023 and the effect `0x9A` at him as it starts.
  **Seen** 2026-10-24 in the running original (the Dwarf's new game, 1089 learnt and made by the right button on a goblin beside him): the effects 1 and 2 of 0.5 for 30 s keyed by 1089, his `A` 25.5 and `D` 25.2 times 1.5.
  `opensacred-game` makes it as the Dark Elf's boons (#759, `Style::Boon`): on the hero, the slot 154 at the rate 1.0, at 0.8 of its clip; the hero then had `A` 38 and `D` 37, as the original's, and it was refused while its effects held; the followers' and the other heroes' part, the object 1023 and the mount's clip are left out.

### The Daemon's

The Daemon's executor (`0x5192E0`) has cases for 1091 to 1094 and 1097 to 1103 (the classes `0x800000`).

- **1091 (`gui_moves_hardhit_demon`, the distance 0)**: the charge above ("The other classes' clips"), its strikes each with an object 842, and an object 1168 (`FX_KAMIKAZE`) made as it ends (not followed).
  The right button makes it only while an effect keyed by 1094 holds on him, 1094 otherwise ("The right button" above).
  **Seen** 2026-10-24 in the running original (the driver of #448, the Daemon's new game at 3250,2511, 1094 made by the right button on a goblin warrior, then 1091 selected and the right button on one of two immortal goblin warriors about 4 cells east, traces of `0x593320`, `0x44DED0` and `strike`): he played the slot 148 at the blend 0.5 and the rate 1.0; 0.22 s into it he flew to the goblin (`0x44DED0`, the height 13.4, a twelfth of about 161, the time 0.096 s) and was beside it at 3252,2511 by 0.33 s, when the slot 14 (his fight stance) followed and he struck it twice within 2 ms; 0.63 s later he struck the other goblin, 3 cells off; the goblins about him stood, their AI's state 0.0 and their action 0.1, for about 3.5 s.
  Read 2026-10-24 (`0x5192E0` at `0x519C40`): the flight goes to the target's place brought nearer by half the two creatures' reach (as the jumps', `0x4DBC60` halved), its time the distance `d` times the float `0x3A3B3EE7` (1/1400) and its height `d` times 0.1 (`0x890D9C`), so 0.096 s and 13.4 for the 134 seen; as it starts, every effect keyed by 1094 and by `0x4321` is taken off him (`0x5888F0`): the charge spends his 1094's rage; then his effects are searched for an effect 50 (not followed).
  How the strikes are chosen, their factor and the hold are not read; the game leaves 1091 out.
- **1092 (`gui_moves_attacke_demon`)**: his `Attacke`, with the objects 842.
- **1094 (`gui_moves_hardhit_demon`, the launch type 5)**, once `left` is below 0.3, for the local player: the time `[+0x50] + [+0x54]·L` (29 + L), at most 180, times `1 + 0.4·f` where his `+0x4F0` has bit `0x8000` (`f` 1.34, 1.67, 2.0 or 2.34 by the global `0x182EE44`, **hypothesis**: the difficulty); for that time the effect 50 keyed by 1094, the effect 2 (`D`'s multiplier) of `[+0x58] + [+0x5C]·L` (0.1 + 0.05·L) and the effect 52 (the attack speed and the pace) of `([+0x60] + [+0x64]·L) / 100` (1.08 + 0.02·L), both keyed by `0x4321`; then a clip of his.
  Read again 2026-10-24 (#760): its first run (`0x51BBC5`, the case of the art's switch at `0x51B46E`) takes off the effects keyed by 1100, 1099, 1098, 1097, `0x4321` and 1094 (`0x5888F0`), gives the effect 50 for 10 s keyed by 1094, and plays the slot 150 at the blend 0.5 and the rate 1.0.
  **Seen** 2026-10-24 in the running original (the driver of #448, the Daemon's new game, `arts learn 1094` and the right button on a goblin beside him, a trace of `0x582D50`): the effect 50 for 10 s at the click, then 0.9 s later the effect 50 for 30 s, the effect 2 of 0.15 and the effect 52 of 1.1 for 30 s; his `D` 26.6 times 1.15, his attack speed 154 and his pace 143 (140 and 130 before).
  `opensacred-game` makes it as a boon (#760, `opensacred_rules::arts::boon` with its `clears` and `starts`, `DAEMON_FORM`; `effects::Effects::take_off`): where the hero stands, the slot 150 at the rate 1.0, the forms' effects off and the effect 50 for 10 s as it starts, and at 0.7 of its clip the three effects for 29 + L seconds at most 180; the factor of the `+0x4F0` bit `0x8000` is left out.
  In the game the Daemon of level 1 then had the effects as the original's, his `D` 30 (the attack record's 26.6 times 1.15, truncated) and his attack speed 154.
- **His forms 1097 (`gui_moves_attacke_demon`), 1098 (`gui_moves_feuerhauch`), 1099 (`gui_moves_giftring`) and 1100 (`gui_moves_energieblitze`), the launch type 2**, once `left` is below 0.3: on its target of a hero's class (else himself) every effect keyed by 1094, 1097 to 1100 and `0x4321` is taken off (`0x5888F0`), and an object of type 1014 (`FX_DEMONFORM`) made at it with the effect `0x8F`; then for `[+0x50] + [+0x54]·L` seconds (44 + L, at most 180):
  - 1097: the effect 3 (the physical damage's multiplier) of `([+0x60] + [+0x64]·L) / 100` (0.15 + 0.05·L) and the effect 1 (`A`'s multiplier) of `([+0x68] + [+0x6C]·L) / 100` (0.15 + 0.05·L), with the effect 53 (its whole value not taken);
  - 1098, 1099 and 1100: the effect 53 of the whole value `0x100`, `0x300` and `0x200` and the value `[+0x6C] / 1000` (0.871), and the effect 8, 10 or 9 (the fire, poison or magic resistance) of `[+0x58] + [+0x5C]·L` (10 + 20·L);
  - each with the effect 60 keyed by `0x4321` (its values not taken).
  So he takes a form of fire, poison or energy (**hypothesis**: the effect 53's whole value names the kind of an aura's damage).
  Read again 2026-10-24 (#760): on himself the forms play the slot 160 at the blend 0.2 and the rate 1.0 (`0x51BEF5` and the like; 197 at the attack's rate otherwise, not followed).
  **Seen** 2026-10-24 in the running original (the driver of #448, the Daemon's new game at 3250,2511, 1097 then 1098 learnt and made by the right button on his head, a trace of `0x582D50`): 1097 gave the effects 3 and 1 of 0.2, 7 of 30 and 60 (a whole value of 1,464), each for 45 s, his `A`'s and his physical damage's multipliers 1.2 and his first resistance 1.77 + 30; 1098 then took them off and gave the effects 53 of the whole value 1 and the value 0.871, 8 of 30 and 60 (1,475), his second resistance 1.29 + 30, and his physical damage 16.84 to 20.84 became 14.84 to 17.84 with a fire damage of 2 to 3 beside it: so 1097 too gives a resistance, the effect 7, and the effect 53 turns about an eighth of the physical damage into fire; its whole value 1 is the power of its value, and the `0x100` to `0x300` read above are its kinds, the adder's eighth argument (read 2026-10-24, #761, "The effects" below).
  `opensacred-game` makes them (#760, `opensacred_rules::arts::Style::Form` and `form`): the right button on the hero himself, or `arts make`, the slot 160 at the rate 1.0, and at 0.7 of its clip his forms' and his 1094's effects taken off and the form's given (`effects::Effects`); the effect 60's whole value and the objects 1014 are left out.
  The effect 53 (the whole value 1, the value 0.871, the kinds `0x100`, `0x300` and `0x200` from the physical to the form's) turns 0.129 of his physical damage into the form's kind ("The effects" below; `opensacred_rules::fight::effects::TURNS`, #761): after 1098 the game's Daemon of level 1 had 23.7 to 28.5 physical and 3.0 to 4.5 fire, the original's 14.84 to 17.84 and 2 to 3 times his multipliers.
  In the game the Daemon of level 1 had `A` 39 after 1097, as the original's 32.5 times 1.2, and after 1098 his first resistance back and his second 38.3.
- **1101 (`gui_moves_giftring`), 1102 (`gui_moves_feuerhauch`) and 1103, the launch type 0**, as their clips run (1101 once `left` is below 0.3): an object at him of type 1008 (`FX_RINGOFPOISON`), 1006 (`FX_WALLOFFIRE`) or 1159 (`FX_EMPEROR`), with casts' objects 791 (`FX_CAST_SPELL`) as the clip runs (their spells' values not followed).

## The effects

Read 2026-10-23 (#510): a creature's stats hold its effects as a list of entries of 32 bytes from `+0x11A` to `+0x11E`: the effect's id at `+0x00`, a whole value at `+0x04`, a value as a float at `+0x14`, its seconds at `+0x18` and an art at `+0x1C`, added by `0x582D50(id, whole, value, seconds, art, …)`, which recomputes the stats.
The recomputation (`0x5796A0`) adds each to the stats by its id, in the list's order, after the items and the record's bonuses and before the skills (`0x566070`, called after the loop), the attack speed then held from 100 to 220 (#651):

| Effect | What it does |
|---|---|
| 1 | adds its value to `A`'s multiplier (`+0xE6`) |
| 2 | adds it to `D`'s (`+0xEA`), held at 0.01 at least |
| 3 to 6 | to the physical, fire, magic and poison damage multipliers (`+0x96`, `+0x9A`, `+0x9E`, `+0xA2`) |
| 7 to 10 | to the four resistances themselves (`+0x66`, `+0x6A`, `+0x6E`, `+0x72`), before their multipliers |
| 11 (`0xB`) | to both regeneration speeds, the spells' (`+0xEE`) and the weapon arts' (`+0xF2`), before the attributes' factors (above) |
| 12 | the regeneration value (`+0x138`) times 1 plus it, truncated |
| 28 | `+0x3E`, the attack speed (`+0x3C`) and the pace (`+0x40`) times it, each truncated |
| 30 | its whole value taken from the three |
| 33 | its value to the damage multiplier of the kind its whole value names (0 to 3) |
| 40 | with its art 1015 and the stats' `+0x148` bit `0x20000`, the record's `[+0x50] + [+0x54]·L` of 1015 at its level (1 to 100) to `A`'s and `D`'s multipliers |
| 42 | `D`'s multiplier times 100 over its value, a value above 2⁻²³ |
| 52 | the attack speed and the pace times it, truncated |
| 59 | its whole value to the fire damage's most and least (`+0x7A`, `+0x8A`) |

After the loop, the first effect 53 of the list turns a part of one kind's damage into another's (`0x57CA45`, read 2026-10-24, #761): the kinds are the low nibbles of the first and the second byte of the first word of the entry's list at `+0x08` (`0x582D50`'s eighth argument; the kinds 0 to 3 the physical, fire, magic and poison least at `+0x76` to `+0x82` and most at `+0x86` to `+0x92`), the part is `1 − value^whole` (`0x849FA0`) stored as a float, and of each of the least and the most `part · end + 0.5` truncated is taken from the one kind and added to the other.
The adder keeps one effect 53 alone: a second takes the first's place, its seconds, value and whole value, its value held from 0 to 1 (`0x582D50`).

**Confirmed** 2026-10-24 in the running original (the driver's `effect`, through `0x582D50`, on the Seraphim's new game): 1 of 0.2 on a multiplier of 1.25 gave 1.45; 2 of −5 gave `D`'s 0.01 and of 0.3 1.3; 42 of 50 doubled it; 3 of 0.5 and 33 of 0.5 with the whole value 1 took the physical and the fire multipliers to 1.5; 7 of 10 took the physical resistance from 1.86 to 11.86, its multiplier unchanged; 52 of 1.5 took the attack speed of 154 to 220 (231 held) and the pace of 132 to 198, 28 `+0x3E` too, from 120 to 180; 30 of the whole 20 took 20 from the three; 59 of the whole 5 gave the fire damage 5 and 5.
`opensacred_rules::fight` follows them (`Effect`, `combat_with`, `regeneration_with`, `arts_speeds_with`; #651) but for 40.
The adder appends each effect to the list's end, so that two of one id and art stack, and recomputes the stats but for the effects `0x11`, `0x25` and `0x28` (`0x584F66`, read 2026-10-24, #763).

Before that the adder has cases of its own by the effect's id (read 2026-10-24, #761; `0x582D50`), most of them a bit of the creature (its `+0x1F4`, `+0x200`, `+0xB0`, `+0x2B7` or `+0x2B8`), which other code then reads:

| Effect | What the adder does besides adding it |
|---|---|
| 13 (`0xD`) | `+0x1F4` bit `0x20000` (the Vampiress's domination, 1024 and 1025) |
| 14 (`0xE`) | `+0x1F4` bit `0x40000` (her master bite's on her, 1031) |
| 15 (`0xF`) | a creature of an id up to 16 (a player) takes `+0x1F4` bit `0x8000000` and stops where its action is 1; any other is stopped (`0x537160`); a second 15 renews the first's seconds instead (her blood bite's hold, 1030) |
| 16 (`0x10`) | `+0x200` bit `0x200000`, and the object of its `+0x263` equipped (made where it is none or flagged) |
| 17 (`0x11`) | `+0x200` bit `0x400000`, the interface's taskbar told for the local player, two objects at it, and two of its items re-equipped (`0x556A30`) |
| 18 (`0x12`) | `+0x200` bit `0x800000`, with 1,000 in the entry (the Gladiator's 1016) |
| 19 (`0x13`) | two calls of the renderer's (`0x422820` with `0x423460` and `0x423430`), not followed |
| 20 (`0x14`) | for a creature of an id above 16 alone: its items destroyed, its model made anew, its type set to the effect's whole value and its `+0x1F4` to `0x20` (a change of creature, not followed further) |
| 21 (`0x15`) | `+0x2B7` bit 4 (the multi-shot, docs/re/fight.md) |
| 23 (`0x17`) | `+0x1F4` bit `0x4000000` (the Vampiress's death's kiss, 1033) |
| 24 (`0x18`) | `+0x2B7` bit `0x10`, and on a horse (`0x4266F0`) an object of ten times its seconds kept in the entry |
| 25 (`0x19`) and 26 (`0x1A`) | `+0x2B7` bits 1 and 2 (the deflections of arrows and of spells, 1045 and 1046) |
| 27 (`0x1B`) | `+0x200` bit `0x40000` (the knockback's hold, "The kicks") |
| 28 (`0x1C`), 37 (`0x25`) | one alone: refused where one holds already |
| 40 (`0x28`) | one alone: where one holds its seconds are renewed instead; else refused with a value below 1, and an object made at it by its kind |
| 61 (`0x3D`) | one alone, refused where one holds; `+0x1F4` bit `0x4000000` and `+0xB0` bit `0x20000000`, with objects at about a third of fifty of its bones |
| 30 (`0x1E`) | on a creature whose stats' `+0x148` has bit `0x10`, refused unless its whole value and value are 0; else `+0x1F4` bit `0x4000000`, and one already in the list merged with it (its value and seconds; not followed in detail) |
| 31 (`0x1F`) | `+0x2B8` bit `0x20`; one alone, its seconds and value raised to the new one's, with an object at it |
| 32 (`0x20`) | one alone, its seconds renewed; else `+0x1F4` bit `0x40000000` |
| 33 (`0x21`) | by its whole value 0 to 5, the bits `0x8000`, `0x10000`, `0x20000`, `0x40000`, `0x80000` or `0x1000000` of `+0xB0`; one of the same whole value alone, its seconds the longer; with a value of 0, calls of `0x549460`, `0x549480`, `0x562470` and of the adder itself first (not followed) |
| 38 (`0x26`) | one per its level word, their values merged by their seconds; `+0x1F4` bit `0x4000000` and `+0xB0` bit `0x20000000`, with objects at fifty of its bones |
| 39 (`0x27`) | `+0x1F4` bit `0x200000` |
| 46 (`0x2E`) | `+0xB0` bit `0x40000000` |
| 50 (`0x32`) | one alone, its seconds renewed |
| 53 (`0x35`) | one alone ("The effects" above) |
| 54 (`0x36`) | `+0x200` bit `0x10000` |
| 57 (`0x39`) | `+0x200` bit `0x1000000` |
| 58 (`0x3A`) | the stats' pace (`+0x40`) set to 0 |
| 1 and 3 keyed by `0x4321` | one alone (the Daemon's forms), their seconds and values the larger |

What each bit then does is the code that reads it, not followed here.

The attack speed's getter (`0x584FE0`, which the fight and the clips read) has two cases of its own (read 2026-10-24, #761): while an effect 47 (`0x2F`, the Dwarf's 1082) holds, it gives the stats' `+0x3C` plus the creature's float `+0x584` (what keeps that float, not read), truncated; and for the Daemon (type 9) while an effect keyed by `0x449` holds, that times the float `0xAD5700` (not read); then at most 220.
`opensacred-game` keeps a creature's effects (`effects::Effects`, #763): added at the list's end, counted down by the world's tick, one gone once its time is no longer above 0, and the hero's attack, defence, damage, resistances, attack speed, regeneration and arts' speeds, or an NPC's values with its pace and its hits' share, made anew whenever the list changes and at an NPC's levelling; the console's `effect` gives one (as the original's driver does), and `stats` lists them.
**Seen** 2026-10-24 against the running original, the Seraphim's start in both, each effect given for 4 s in both: the effects 1, 2, 3, 7, 12, 28, 42, 52, 30 and 59 gave the hero the same values in both (`compare.py`'s parsers), and so did 1, 2, 4, 8 and 52 a goblin warrior of level 5, each worn off within 5 s in both.
The effects' own side effects at their adding (13, 14, 16 to 18 and the others above) are not followed yet (#761); the arts and the spells that give effects come with their features (#754 to #760, #672).

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
The spells that give the effect `0x22`, each of the launch type 2 or 5, are those that last on their target (**hypothesis**: the effect marks a running spell for its duration); what each object gives its target, and for how long, is the objects' code (below).

### The spells' objects

Read 2026-10-23 (#652), in part; **confirmed** by code unless marked otherwise.
Every `TYPE_FX_…` type is of the `items.pak` category 12, which the object manager's maker (`0x451300`, by the category) makes a `cObjectFX` (`0x5A0A10`; its virtual table at `0x89199C`): one class for all, which switches on its type (`+0x10`) where they differ.

- `0x5A0C50`, called by the constructor, sets an object up by its type (a switch of some 220 cases).
- The update `0x5A6680` runs the object at each frame and hands its targets to `0x5ABD80`, which switches on the type again: of the spells' objects, `FX_FIRESTRIKE` (778) sends each creature of its list an event through the creature's handler (`+0x18`), as a strike sends one (docs/re/fight.md), and `FX_BUBBLETRAP` (847), `FX_RAILGUN` (801), `FX_ICEBULLETS` (839), `FX_BFG` (875), `FX_HELLFIRE` (820), `FX_PENTAGRAMMKILLER` (782), `FX_HOLYLIGHT` (804), `FX_CIRCLEOFICE` (840), `FX_FIRE_CIRCLE` (790), `FX_HOLYWALL` (814), `FX_METEOR` (823), `FX_TWISTER` (826), `FX_WINDSTRIKE` (831), `FX_FIRESKIN` (821), `FX_FLASH` (838), `FX_CONFUSION` (871), `FX_MAGLITE` (817), `FX_KILLERPLANTS` (848), `FX_GROUND_POISON` (792), `FX_EXPLOSION` (794), `FX_LOSTSOULS` (850) and `FX_BALL` (867) have cases of their own there (not read further); the other types fall to its default.
  `0x5B2600`, which the update calls too, gives effects in fifteen places (not read).
- An object gives its targets their effects through the stats' `0x582D50` (the effect's id, a whole value, a value as a float, a duration in seconds at the entry's `+0x18`, the art that keys it, then a flag for the network, a word and a flag), as the executor does with the effect `0x22` ("The effects" above).
  `FX_CONFUSION` gives the effects 14 and 13, each with a whole value and a duration.
- The effect 13 sets the creature's flags word's bit `0x20000` (`+0x1F4`) and the effect 14 its bit `0x40000`, as the adder takes them, beside their entries.
  The bit `0x40000` turns the creature's sides' answer over (docs/re/fight.md, "Sides"), so that it takes its friends for foes and its foes for friends.
  The fight at range (`0x4DBFC0`) takes a branch only without the bit `0x20000`, and with `0x40000` only against a target of an id above 16 (not read further); what else reads the bit `0x20000` is not read.
- The objects' scripts: the game's log names a `cObjectFX::dopSystemAction` that makes one object out of another (`FX_BLEED` making `FX_BLOODPUDDLE`), so the objects run actions of their own, the effects' spawning among them (**hypothesis**: from the types' data, not read).

Seen in the original on 2026-10-23 under the driver of #448, the Seraphim at level 1 with the spells learnt by `arts learn` and an immortal goblin of type 76 at level 5 beside her:

- The right button makes the art at `+0x5A` of the engine (`0xAD6EF8`) on its target, by the event `0x105` (`0x6172C0`) with `+0x58` beside it: 1 for the weapon arts (the Seraphim's 1071 at the start), 0 for a spell; a spell put there with 1 does nothing.
- `IRRITATION` (18, its object `FX_BURNINGLIGHT`) gave the goblin it was made on the effect 13, of the whole value 1 and the duration 3 s, keyed by the art 18, once in the 4 s watched; its regeneration ran its 24 s.
- `BANNKREIS` (16) and `HIMMELSLEUCHTEN` (15) on the goblins gave no effect within 5 s, their regenerations running (**hypothesis**: they act on the undead and the demons alone, as their names say).
- `GLAUBENSSTAERKE` (21) did not run by a right click on the hero herself (its launch type, `+0x3C`, not followed).

So the first table's values (above) are not each object's own: `IRRITATION`'s value 0 is 14 + L seconds, at most 30, where its effect lasted 3 s (the action 7 below scales it by the target).
What each object gives beyond the actions and the hits below, for how long and to whom, is not read: which action each object's system runs, and the objects that neither has a case for (#672).

#### The objects' actions

Read 2026-10-23 (#672); **confirmed** by code unless marked otherwise.
`0x5B2600` is the objects' actions: the update (`0x5A6680`, at `0x5A9AAF`) calls it at each frame with the action that the object's system holds, asked of the systems' manager by the object's `+0x6E` (`0x7CBD90`), so that which action an object runs comes from its system's data, not from its type (not read); 0 does nothing, and 1 to 11 are switched on.
The object's art is its `+0x118`, the record of the first table ("The first table" above), and its level `+0xE4`; its caster is `+0x54` and its target `+0x60`, each checked to be the object it was when the action started.

- **The action 6, the art's effects on its target:** the record's thirteen pairs from the attack's on, each `base + step·L` that is not 0, given as an effect for the art's value 0 at its level in seconds (`0x4D26B0`, truncated; 1 without a record), keyed by the art:

  | Pair | Effect | Its value |
  |---|---|---|
  | 4, the attack (`+0x52`, `+0x54`) | 1, `A`'s multiplier | a hundredth of it |
  | 5, the defence (`+0x56`, `+0x58`) | 2, `D`'s multiplier | a hundredth |
  | 6 to 9, the resistances (`+0x5A` to `+0x68`) | 7 to 10, the resistances | the pair's value |
  | 10, the speed (`+0x6A`, `+0x6C`) | 28, the attack speed and the pace | 1 plus a hundredth |
  | 11, the mental regeneration (`+0x6E`, `+0x70`) | 11, the arts' regeneration speeds | a hundredth |
  | 12, the physical regeneration (`+0x72`, `+0x74`) | 12, the regeneration value | a hundredth |

  The damages' pairs 0 to 3 give no effect here.
  So `GUI_SPELL_STEINHAUT` at level 1 gives `D`'s multiplier 0.2 and the physical and poison resistances 65 and 25 for 45 s (**hypothesis**: that its object runs the action 6; not seen).
- **The action 7, an effect that holds its target**, of the id the action's data names (13, 14 or 15), the effect's whole value 1 for 13 and 14: for the art's value 0 in seconds times the share of five times its level in the target's `+0x3FE` where that is larger (**hypothesis**: the target's resistance to the spells, not read), none for a target of the flags' bit `0x80000` (`+0x1F4`) or of `+0x2B8` bit `0x40`; the effect 15 lasts the value 0, or 3 s from a caster of an id above 16, and goes to the creature that a ridden target carries (`+0x14` bit 2, `+0x1EC`), and an event follows it (`+0x18`, not read).
- **The action 9** gives the effect 15 with a value of the action's data, for the art's value 0 in seconds, as the action 7's.
- The actions 1 to 5, 8, 10 and 11 give no effect (not read further).

**Seen** in the running original (the driver of #448, the Seraphim at level 1 with an immortal goblin of type 76 at level 5, `arts select 18` and the right button on the goblin, traces of `0x5B2600` and `0x582D50`): `IRRITATION`'s object (type 816) ran the action 7 once, which gave the goblin the effect 13 of the whole value 1 for 3 s, keyed by the art 18; the objects of the types 772 and 773 about the hero ran the actions 1 and others, and the birds (type 886) the action 0 at each frame.

#### The objects' hits

Read 2026-10-24 (#672) in `0x5ABD80`, which the update hands each object's targets to; **confirmed** by code unless marked otherwise.
It first makes the spell's damage event: the four kinds' damages of the first table's record, the pairs `+0x42`/`+0x44` to `+0x4E`/`+0x50` as `base + step·L` at the object's level (`+0xE4`), each times the caster's factor of its spells (`0x56A800`) and held at 65,534, the factor 1.0, the caster (`+0x54`) and the object.
Most objects then send it once (their `+0x11C` set, so that a later run sends nothing) to each target that does not turn the spell aside (`0x561860`: a target of a hero's class with the skill 14 by the hit chance `0x43EEE0` between a value of the spell's and its level raised by the skill's curve, then a target with the effect `0x1A` by the arrows' rule of docs/re/fight.md, which for the Dark Elf's 1046 sends the spell back to its caster).
By the object (its spell, "The spells' executor" above):

| Object | Spell | What its targets take |
|---|---|---|
| 778 `FX_FIRESTRIKE` | 5 | the event, once |
| 782 `FX_PENTAGRAMMKILLER` | 6 | a target of a level above the object's value (`0x7CBEA0`), or a player's hero in a network game, the event; any other the event 9 from the caster (**hypothesis**: slain); the factor `1 + (s₂₆ + s₂)/100 + (b + h)/K`, `s₂₆` and `s₂` the caster's skills 26 and 2 through the balance's curves (`0x565D70` with `0xAD45D8` and `0xAD44D0`), `b` a value of `0x5A03B0`, `h` the caster's `+0x3BC`, `K` the value at `0xAD5630` |
| 790 `FX_FIRE_CIRCLE` | 11 | the event, once |
| 792 `FX_GROUND_POISON` | 12 | the event, once, at the factor of 782 |
| 794 `FX_EXPLOSION` | (the Dwarf's guns) | the event, once, at the factor of 782 but from a caster of the type 8 |
| 801 `FX_RAILGUN` | | the event at every run |
| 804 `FX_HOLYLIGHT` | 15 | the event, once |
| 813 `FX_FI_SMOKE_M`, 876 `FX_FI_SMOKE_L`, 879 `FX_GEYSIR` | | an event of `0x51CB00` from the object with the damages 10, 20 or 100 of the first two kinds alone, once |
| 814 `FX_HOLYWALL` | 17 | the event, once |
| 817 `FX_MAGLITE` | 20 | the event, once, to the targets of the types that `0x426A80`, `0x426BC0` or `0x426B20` name (**hypothesis**: the undead and the demons) |
| 820 `FX_HELLFIRE` | 23 | the event, once |
| 821 `FX_FIRESKIN` | 24 | the event, once, and an object 788 (`FX_BURNING_BONE`) at each target |
| 823 `FX_METEOR` | 26 | the event at every run |
| 826 `FX_TWISTER` | 28 | from a caster of the local player, the event `0x112` from the caster to a target of a level 3 or more below the spell's, or by a roll of one in `3·(M − L + 6) + 1` for one nearer its level (**hypothesis**: thrown about) |
| 831 `FX_WINDSTRIKE` | 29 | the event to every target, and to one whose level (a boss's scaled) is below `0.4·L' + 5·L` and of no big type the event of kind 3, the knockback, of the spell's value 0 at its level (`0x4D26B0`) |
| 838 `FX_FLASH` | | the event, once |
| 839 `FX_ICEBULLETS` | 34 | the event, once, from a caster of the local player |
| 840 `FX_CIRCLEOFICE` | 35 | the event, once |
| 847 `FX_BUBBLETRAP` | 38 | the event, once |
| 848 `FX_KILLERPLANTS` | 39 | the event, once |
| 850 `FX_LOSTSOULS` | 41 | an event of `0x51CB00` to its target, then a new target within 100,000 (`0x600570`; **hypothesis**: the souls fly on) |
| 855 `FX_ARACATTACK` | | the event to its owner's target, once |
| 867 `FX_BALL` | 53, the Dark Elf's traps | to each hostile target (`0x548FB0`) an event of `0x51CB00` from the caster and an object 786 (`FX_GOBLINIMPACT`) at it, whose spell it casts (`0x5B7590`) |
| 871 `FX_CONFUSION` | | to a target neither a boss nor of `+0x2B8` bit `0x40`, without the effects 13 and 14, by a coin the effect 14 or 13 for `3·min(1, 5L/M)` seconds, its whole value the caster's level, and it stops ("The spells' objects" above) |
| 875 `FX_BFG` | the Seraphim's 1018 | the event, once |
| 878 `FX_SMALLFIRE` | 50 | an event of `0x51CB00` with the damages 5 and 5, or the caster's (`0x5B79F0`, `0x5B7A00`), to every target from a caster of no hero's class, else to the hostile ones |
| 880 `FX_SPIDERPUKE` | | an event of `0x51CB00` with the damages of the event that made it, once |
| 895 `FX_TRAP03_LASER` | | an event of `0x51CB00` with 200 of the first kind and 100 of the third, once |
| 905 `FX_FLYINGSKULL` | the Vampiress's 1033 | the effects 2, 1 and 23 for 5 s keyed by 1033, their values from the event's (`0x4EB620`; not followed), and an object 862 at each target |
| 906 `FX_BUBBLETRAP_EXPLO` | | the event at the factor 2.0, once |
| 910 `FX_DAEMONFIRE`, 922 `FX_SHADDAR_MAGIC` | 50, 52 | an event of `0x51CB00` from the caster at every run |
| 921 `FX_SHADDAR_SLAVE` | | the effect 32 for 5 s to a target of an id up to 16, or it follows the creature of the type 700 |
| 923 `FX_DRAGON_GROUNDFIRE` | | the event of damages by the caster's skill 2 and the object's level, once |
| 1005 `FX_HELLDISK` | the Daemon's 60 | an event of `0x51CB00` to its target, then a new target within 100,000 |
| 1006 `FX_WALLOFFIRE`, 1007 `FX_WALLOFFIRE_LINE`, 1008 `FX_RINGOFPOISON` | the Daemon's forms (1102, 1101) | from a caster of a hero's class, to each hostile target the effect 62 of the event's whole and float values (`0x4EB630`, `0x4EB6F0`), for 1 s (**hypothesis** for its arguments' order; 1007 not to a target of `0x5539D0`) |
| 1011 `FX_SONGSFROMHELL` | 67 | an event of `0x51CB00` to its owner's target |
| 1012 `FX_HELLSCREAM` | 66 | to each target without an effect of the spell: the effect 1 of `−(1 − 100/p)`, `p` of `0x590C80`, and the effect 2 of a value of `0x590CA0`, keyed by the spell (their times not taken) |
| 1013 `FX_DAYOFTHETENTACLE` | 64 | an event of `0x51CB00` from the caster, once |
| 1020 `FX_GRENADEIMPACT` | the Dwarf's 1086 | the event of its damages (`0x5B79F0`, `0x5B7A00`), once |
| 1154 `FX_DWARF_MINE_SIGN` | 56 | an object 1153 (`FX_DWARF_MINE`) made, once; the mine goes when its value reaches 100 (`0x5B7950`) |
| 1159 `FX_EMPEROR`, 1162 `FX_DRYAD_GHOST`, 1163 to 1167, 1170 | 69 to 72 and the scripts' | the event of their own damages (`FX_DAEMONENFUERST` by its state, `+0x9C` 185 or 250 to 252, one kind each), 1170 `FX_EITERBLUME` 500 of the first kind and 100 of the third |
| 1156 to 1158 `FX_PONG_…` | | a game of the scripts', not of the fight |

**Seen** 2026-10-24 in the running original (the driver of #448, the Seraphim of level 1, `arts learn 15`, `arts select 15` and the right button at the feet of a hostile goblin warrior of level 30): its health went from 757 to 715 within a second and to 571 within four, so that `HIMMELSLEUCHTEN` hits again and again over its time (**hypothesis**: it makes an object 804 at each of its beats, each hitting once); the immortal goblins' hook of the driver, which tells the strikes of `0x56B130`, told none of these hits.
The other objects (the shields, the healings, the summons and the changes of form among them) have no case here and log their type: what they give comes from their actions ("The objects' actions" above) and from their own updates (not read).

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

What the spells' objects give their targets beyond what "The spells' objects" reads (which action each object's system runs, the objects with no case in their hits and actions; #672); what the special weapon arts' effects and objects do (#761: the jump 1032 in part, what the Wood Elf's 1006 makes, the record's `+0x18` of 1004, what the effects 16, 17, 18, 35 and `0x1A` do, the hurled blades' flight, the Dark Elf's traps' objects, what the Vampiress's 1026 to 1029 and 1033 make, the effects 13, 22 and 29's whole values, what the Dwarf's guns and the Daemon's rings and breaths make, what raises and lowers the creature's float `+0x584` that the effect 47 adds to the attack speed, the effects 50, 53 and 60) and the other classes' kicks (#650); the `+0x58…+0x6C` that they read; a rune's requirements; the combos' run beyond their levels; the sender of `0x5481F0`'s event; what leaves a template's arts that a rune taught out of a new game (not `0x588990`, above); the effects 59's fields and 7 to 10's (above).
