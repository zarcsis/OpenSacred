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
- A creature with a champion's levelling (`+0xB0` bit `0x400`, docs/re/fight.md, "Bosses and champions") has `B` = `ITBonusChamp` (35) and, unless too weak, `N` + `R(1, ITThrowCountBonusChamp)` (3).
- A boss (`+0x1F4` bit `0x80000`) adds `ITBonusBoss` (50) to `B` and, unless too weak, `R(1, ITThrowCountBonusBoss)` (4) to `N`; then a switch over its type (`0x533E2E`, its jump tables at `0x536EBC` and `0x536F48`) gives some twenty types more drops or particular items (below).
- For a player's kill, `B` becomes `⌊B + LongivityFactor·v(t)⌋` (`LongivityFactor` 0.5), `v(t) = 100·(1 − 1/(1 + (t − 1)/120))` for `t` at least 1, else 0, `t` the statistics' counter `+0x570C` over 1,000 and over 60 (`0x425290`, `0x424650`; **hypothesis**: the minutes played), and then `⌊B + √(100·(ℓ − 100))⌋`, `ℓ` the hero's luck of `0x564A60`: 100 plus the value of its effect 840, raised when some of its counts at `+0x4F8` are high (**hypothesis**: the worn pieces of sets, below).
  So the time played adds up to 50 to the bonus and a luck of 25 adds 50.
- `s` is 0, but for a killer with the effect 42 in its list of effects (`+0x11A`, `0x5539D0`) the first value of the combat art record 55 (`0x4D2310`, a table compiled in at `0x952A38`) at that effect's level (`0x4D26B0`; **hypothesis**: an art that finds treasure).
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
5. While `P` is above 0, a pass: with `T` 0 it makes nothing unless `rand() % 1000` is below `P`; a type whose template has the word 38 −1, or whose record is missing or of flag `0x40` (five times at most), is drawn again with the pass's roll, up to six tries in all; then `P` falls by 1,000.
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
2. Else, unless `C` is 6 (the animals) or the creature made the last rune, a rune (category 28) when `rand() % 1000` is below `UpgradeDrop·r` (50), `r` of `0x5598B0`: `x = H·RunenProLevel_*/(n/RuneInventoryFactor + a)`, the key by the local hero's class (1.2 to 2.2), `n` the runes it carries (the items of category 28 in two of its containers, `0x5DEBC0`; **hypothesis**: the inventory and the storage), `RuneInventoryFactor` 3.0, `a` a sum over its list of 22-byte entries at `+0x4AA` (**hypothesis**: the runes read into its combat arts); `r = x⁵` when `x` is above 1 or the creature a boss or champion, else the weighing times `x⁴`, held within 0.01 and `RuneChanceFactorMax` (2.0), times `RunesPerBoss` (1.5) for a boss or champion.
   The rune is one of 123 types at `0x9565A0` (from 5375, `upgrade_*.grn`), a potion instead for the types 516, 592 and 593 or a rune of `items.pak +0x6A` 0.
3. Else gold in `DropGold` (300) ‰, always for `C` 6, and for `C` 11 (the ghosts) in 600 ‰ of the rest.
4. Else, unless `C` is 11, a weapon (category 5) in `DropWaffe` (330) ‰.
5. Else, unless `C` is 11, armour in `DropRuestung` (880) ‰: one of eleven categories (6, 13, 17, 18, 19, 21, 22, 23, 24, 25 and 29, the bridles drawn once again).
6. Else, unless the same creature made the last jewel, a ring (in two cases of three) or an amulet (categories 8 and 20).
7. Else, and wherever no type is found, the dagger 1712.

`DropSonst` (1000) is stored by the balance's parser and read by no code.
For potions, runes and gold `k` becomes at least 16.
The animals thus drop potions and gold only, the ghosts no weapon or armour.
For an ordinary creature the kinds share out, without the runes, as 18 % potions, 25 % gold, 19 % weapons, 34 % armour and 5 % jewels.

The type of a weapon, armour or jewel is `0x5C7FA0`'s (the client's `TypeManager::getRandomItem`), from the types of the category, at the level `L` with the quality `q`, any weapon kind, the local hero's class in one case of two (else any), and set pieces left out; partly read:

- it takes a type of the category whose template's word 38 is not −1, word 39 at most `L` and word 40 at most `q`, not of `items.pak` flag `0x2000000`, not 5120–5131, 5632 or 5633, a rune only with `items.pak +0x6A`, no unique of word 38 29, and whose record's levels (`Weapon.pak +0x94` and `+0x99`) lie below the level plus a jitter (the level held at 28; the jitter 1, or above level 5 the product of two rolls of 1 to a third of the level, at most 3), that check left out in two cases of three;
- of the types with class bits, only those of the class asked;
- then one at random: with a class asked, in one case of two among those with its bit; else, in one case of two, the first of up to three draws that the local hero can wear (`0x54ABE0`).

### A pass: making the item

The object of the type is made at the creature's position (`sdk_CREATE`).
Gold takes its amount, `g = a + ⌊(L − 1)·(b − a)/100⌋`, `a` and `b` the two values of `GoldProLevel` (20 and 2,000, docs/02 §6.3), then `g − ⌊g/6⌋ + rand() % (⌊g/3⌋ + 1)`, at least 1, into its `+0x120` (its record's price, `0x5C7D10`), and the quests' manager hears of it (`0x4AEBD0`, not followed): 17 to 23 at level 1, 165 to 231 at level 10, with the creature's own level, which is higher on the higher difficulties.
Its model is one of four piles, 5132 to 5135 (`GoldCoins_01`, `_02`, `_05`, `_10.grn`), chosen before the amount and whatever it is: 5132, but in a case of 13 above level 4 5133, of those in a case of 7 above level 9 5134, and of those in a case of 4 above level 19 5135.
Gold, potions, runes and the categories 26 (the blacksmith's upgrades, slots 8000 on), 30 and 31 are not made further.
Another item is switched to the local hero's class's variant of its type (`0x5C7370`, by `0x43CA20`; **hypothesis**: through the armour switch table `bin/Rust.bin`) and made at the levels `lo = ⌊(L − BaseOffLevel)/(1 + OffLevelProz/100)⌋`, the creature's level brought back to bronze (`L` itself on bronze), and `hi = ⌊1.05·lo + 0.5⌋`, by its key:

- 0, a unique: with `T` 0, the uniques of the item's family (its template's word 38; a map at `0xAD4428` built by `0x5D4AC0`) of quality 15, no set piece, of a least level (`Weapon.pak +0x94`) up to `H`, and not 7911–7930 (reserved for the scripts' drops); in 30 % of the cases, if there is one, one of them at random, raised to `lo` (below); else the item with the key 14 at `R(lo, hi)`.
- 1, a set piece, chosen before the object is made: in 60 % of the cases a set whose first piece is for the local hero's class, has a template and a least level up to `H` and is no icon (5101–5106), else any set with a piece that has a template; of its pieces with a template one at random, the pieces the hero wears kept in one case of two; taken when its level (`Weapon.pak +0x99`) times `MinLevelFactor` is below `H`, and raised to `lo`; failing that, the key 2.
- 2 to 15: the maker (`0x5D5080`) at `R(lo, hi)`, at least 1, with the key `16 − k`.
- 16: the maker at `lo` with the key 0; 17: at `R(⌊lo/2⌋, lo)`, at least 1, with the key 0.

A forced `T` that is a unique takes the key 0 and a set piece the key 1, both only raised; any other forced item keeps the key 16.
A unique or set piece of level `ℓ` (its record's `+0x19`) is raised (`0x5C29A0`, `0x5C2210`, not followed further) to `ℓ + d·⌊(lo − ℓ)/d + 0.5⌋`, `d = max(ℓ/2, 15)`, when `lo` is above `ℓ`.
The key given to the maker passes through the third table of the tail of `Balance.bin` to the quality pass `0x5D2E40` (0 to 7 kept, then 9, 11, 13, 15, 18, 21, 25 and 25 for 8 to 15).
The maker's fourth argument, rolled 0 in three cases of four, else 1, 2 or 3, is not read by it.

### On the ground

A drop is a `cObject3D` (a `cItemBase`), drawn as its model by its type (`items.pak +0x70`, with the texture of `+0x08`, docs/02 §6.1), not a sprite.
It leaves the creature's position 60 units up (`0x44D7B0`, `0x44D520`): a height in the world, not over the ground, which lies within 15 of 0 almost everywhere (the creatures' heights, docs/re/movement.md) and 28 a floor higher in a building; the hero's own drop (`0x611620`) starts 60 over the ground.
It flies (`0x44DED0`) with the velocity `(10·R(−75, 75), 10·R(−75, 75), 190)` and spins about each axis at `R(−6, 6)`.
Each frame (`0x44DFE0`, from the objects' update `0x44B230`, `dt` the frame's time) its angles grow by the spin times `dt`, its `x` and `y` by a tenth of its velocity times `dt` and its height by its vertical velocity times `dt`; the velocity takes `−0.1·v` along `x` and `y` and −290.81 along the height, times `dt`.
Where its next cell may not be walked on (`0x636890`) its `x` and `y` velocities turn back, a bounce; below the ground (`0x449950`) it lies on it with no velocity; it rests once each velocity is below 1 and it lies within 1 of the ground.
So a drop lands about 1.6 s later within about 110 units, two cells, of the creature along each axis (computed from the above with the ground at 0), at the angles its spin left it; the types 5120 to 5135 (the gold piles among them) and 5375 to 5631 (the runes) are held upright at every frame.
The treasure's items fly the same way twice as far, and the held items that `0x4E9900` throws leave the hand slower, at `(R(−100, 100), R(−100, 100), 150)`, spinning at `R(−15, 15)`.

Holding Alt (`GetAsyncKeyState` of `VK_MENU`, the world view's flag `0x4000`) shows a label over each item in view (`0x627180`): its name, or for gold the amount and a word, laid out by a label layout so that labels do not overlap (`cLabelLayout`, up to 1,024), on a dark translucent panel (`0xCF303030`), in a colour that the item gives (a virtual call, not followed), and the label under the cursor highlighted.

The pick-up is `cCreature::pickupItem` (`0x558080`), which the AI's state 1 calls (`0x53A170`, at `0x53CEB0`; **hypothesis**: as the hero arrives at an item it was sent to, as at a door, docs/re/fight.md): a quest's item is counted (`0x4AAAE0`, below) and taken; gold adds its amount to the creature's (`0x57FD80`) and vanishes; a hero that can use the item (`0x54ABE0`) takes a weapon into its hands when both are empty, and another item into an empty slot that fits it (`0x43F3B0`, `0x43EFC0`); else for the local player onto the cursor when it holds nothing and `0x5D9060` allows (**hypothesis**: while the inventory is open), or into its inventory where its size finds room (`0x428D60`, `0x5DABA0`, `0x5D9F00`), the scripts hearing of it (`0x471790`); without room it is handed to `0x5592B0` (not followed).

### The scripts' drops

`SetDrop` (record type 137, the walker's handler `0x4AFFF0`) sets for a creature type the drop of an item type with a chance in per cent, into a list of `{creature, item, chance}` at the quests' manager `+0x762C`: the two types in either order, a chance 0 removing the creature's entry and another replacing its chance (and its item, both types given).
The trees hold 1,204 such records (docs/02 §7.3): mostly quest items of category 15 and potions or bottles of category 9, at 50 to 100 %, and 602 of chance 0.
A creature of a listed type drops the item in that per cent of the calls of its drop, in place of the call's random drops.
Picking up an item counts it for the quests that collect it (`0x4AAAE0`: a list at `+0x7620`, whose creature types are read through the drops' list as their items), finishing the quest at its count.

### Other drops

- The treasure `0x5516B0` (a chance `p` per mille, a quality range, a jitter): eight gold piles (each a forced drop of 5133) and two rolls of an item, each in `p` ‰ (halved after an item, doubled after none, the quality range halved each roll, at least 1): a category of a pool of 19 (weapons five of them, rings and amulets two each, the armour pieces, shields and wings one each), the type by `0x5C7FA0`, made at `L` plus `R(0, jitter)` with a key of the range, thrown with `10·R(−150, 150)` along `x` and `y`.
- A hit by a creature with the effect 856 (in the strike's resolution `0x56B130`, at `0x56F679`): in `rand() % 101` below its value (10 times the difficulty's factor more when its counts at `+0x4F8` hold more than four at the entry 30) the struck creature drops a gold pile (5132 in a case of two, else 5133 in a case of two, else 5134 or 5135), its amount held to the strike's damage and to a value of `0x549460` (not followed).
- A quest's reward (`0x4AC940`) gives gold, experience and items by its flags, and after a count of finished quests at the hero's `+0x2CD` passes `R(1, 56)` a piece of a set not 33 to 38 through the drop (not followed further).

## Open

The words 43, 46 and 49; what the quality pass makes of its map's value, and `0x5D1750`'s bonuses; the price `0x5D3AB0`; the bonus's other branch; whether `0x5796A0` adds the unconditional effects; the object's creation and the copy of a fixed item; `CreateNPC`'s second making of a rule's items; the thread that runs the scripts.
Of the drops: the boss switch's cases (`0x533E2E`) and the type 183's last item; the type choice's least level and preferences (`0x5C7FA0`); the raising of a fixed item (`0x5C2210`); the counts at `+0x4F8`, the effect 42 and the art 55, the list at `+0x39C`; the colour of an item's label; how long an item lies on the ground and whether it stays when its sector is left; what the network events of a drop carry; the receivers of the effects 856 and 868.

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
