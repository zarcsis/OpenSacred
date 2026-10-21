# The items' values

How `Sacred.exe` 2.28 makes an item's values, and so the damage of a creature that holds a weapon (#294).
Status: **confirmed** by code (2026-10-20, Ghidra; the disassembly read where the decompiler drops the x87 arithmetic) unless marked otherwise.
The difficulty is the int `0x182EE44`, 0 bronze to 4 niobium; "truncated" is the runtime's `_ftol`, toward zero.
`R(a, b)` is the game's roll of a whole number between `a` and `b` (`0x443B20` and the same code inline): a negative least becomes 0, a negative most 1, the two swapped when reversed.
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
- 40 is not read by the maker.

The entries are the magic bonuses (below).

## The item maker `0x5D5080`

It takes the level, a key of the quality, the type and the item's 130-byte record (the item's `+0x118`, docs/re/fight.md).

1. The level is held at or below `216/MinLevelFactor`, 270.
2. Without a template, for the items table's byte `+0x2E` of `0x10` or `0x1C`, or a word 38 of 29, it leaves the record as it is.
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

## A creature's items

A fill (`FillSector`, `0x47C610`) makes all its creatures' items at one level: the hero's within the sector's bounds and jittered, as the creatures' (docs/re/spawning.md), raised by a tenth of a curve of the minutes of a counter (**hypothesis**: the time played) through `0x552230`, which is the level itself on bronze, at most 300; every item of its `MakeGroup` rule is made at it with the quality key 0.
`CreateNPC` makes a record's items at the creature's level with a key at random of 0–3.
A fixed item keeps its record when the maker stops (**hypothesis**: the copy of `Weapon.pak +0x80…+0x101`, whose words fit the record's layout).

## Example: a goblin's dagger

The goblin warriors of 51,39 hold nothing, and fight with `Creature.pak`'s bonuses; the armed goblins are the fills' of the rule `MakeGroup 02:76 87:(71,100)…`, whose pool 71 holds daggers and a short bow.
The dagger 1712 (`DAGGER.GRN`, template 29: `S` 50, `w` 10, physical always, fire and magic 8 % each, `A` 20 to 80 always) at level 1 on bronze: `f` 27.5, `p` 14.25; with physical damage alone (84.6 %) 12 to 15, else 9 to 10 with an element of 3 to 4; `A` 5 to 21, `D` 0; and a bonus of effect 801 of 28 to 40, which adds 6 to 9 physical by its attribute.
At the levels 4 and 5 of the goblins' sectors it gives 16 to 19 and 17 to 20.

## Open

The words 40, 43, 46 and 49; the category `0x1C` (`items.pak +0x2E`) that the maker leaves as it is; the quality map and `0x5D1750`'s bonuses; the price `0x5D3AB0`; the bonus's other branch; whether `0x5796A0` adds the unconditional effects; the object's creation and the copy of a fixed item; `CreateNPC`'s second making of a rule's items; the thread that runs the scripts.

## In `opensacred-game`

An NPC's items in its hands are made at its level by their templates (#309, `opensacred_rules::fight::Templates::make`): their damage, resistances, `A` and `D`, without the magic bonus, the triples, the quality pass, the difficulty's term (bronze's 0) or a fixed item's own values (docs/decisions/2026-10-20-weapons.md).

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
  So her body armours share a mesh and differ by the item's texture, which the game draws (docs/re/equipment.md; **hypothesis** that the client draws it so, checked in the original in #227).
