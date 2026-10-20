# The fight

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra 12.1.4 on 2026-10-20 (#276), by a subagent of the session with its key points checked.
Distances are in world units, 53.66563 a cell; "straight" is the distance of `0x5370B0` and `0x4B2350`, √(dx² + dy²), 32,000 beyond ±32,000 along an axis.
Status: **confirmed** on the code unless marked otherwise.
The creature's fields are those of docs/re/ai.md: the AI's state `+0xFC` (sub-state `+0xFE`), the action's `+0x150` (`+0x152`), the target `+0x104`, the gait `+0x1F8` (1 walk, 2 run), the level `+0x3FE`.
The balance's values are named by their keys and read from the image of `bin/Balance.bin` (docs/02 §6.3, `tables/balance.ron`); the difficulty is the int `0x182EE44`, 0 to 4 (**hypothesis**: bronze to niobium), and a balance row of six takes the sixth value for 0, else the value `0x182EE44 − 1`.

## Who fights: the test of the levels

An awake hostile creature (`0x542B20`, docs/re/ai.md, "Dormant and awake") weighs its level against its target's, the local player's hero, or the owner (`+0x1EC`) of an NPC it chose that is no rider.
Its level `L` is first brought back to the bronze scale, the inverse of the difficulty's scaling of the levels (`0x5522D0`; forward `0x552230`): 1 up to `BaseOffLevel`, else `(L − BaseOffLevel) / (1 + OffLevelProz/100)`, rounded, at least 1 (the image's `+0x11D0` and `+0x11B8`; 0 and 0 on bronze, where it keeps `L`).
With `E` that level and `H` the target's, `r = (E + 9)/(H + 9)` and the threshold `t` is `(H + 6)/(H + 9)` up to level 10, `(H − 6)/(H + 9)` from 92, else `UnterExpAnteil` (0.85); the test passes when `r > t` and `(r − t)/(UnterExpVoll − t)` is above 0.04: exactly when the hero would earn more than the least experience from the creature (below).
A failed test sends the creature dormant again, and in one case of three to its idle action, only when its target is also beyond its reach, and it is no boss (`+0x1F4` bit `0x80000`) nor champion (`+0x2B8` bit `0x40`) outside a group, nor a creature of a script on sight: a weak monster lets the hero pass and fights only when the hero comes within its reach, and a lone boss or champion always fights.
Otherwise it attacks a target within 800, alive and hostile to it (`0x422820`), when a straight line of walkable cells leads to it (`0x4FCC80`); without one it goes for the nearest closed door of its sector (`0x4D4800`), and when no way leads to it at all (`0x4F7710`) it takes its idle action.
It takes the AI's state 8 with a ranged weapon in a hand (item category 5, weapon kinds 7, 12, 13, 14), else 4; in a group, in one case of two, it raises the group's alarm (`0x46AE70`, docs/re/ai.md, "Groups").
The reach `0x4DBC60(a, b)` is the mean of two sizes by type: 130 by default, the heroes 70 to 130, the monsters 70 to 480.

## Melee: the AI's state 4

`0x4E0300` keeps a creature within a band about its reach `R`: `R − 15` to `R + 15` (`R + 45` for a player's).
Deciding (sub-state 0):

- At a third of its health or less, within 199 of its target, not a boss nor of the classes 1 and 5, once a fight (`+0x1F4` bit `0x200`), in one case of eight a frame while its target does not attack, it flees 700 from the target along the line.
- Within the band it strikes (sub-state 2); outside it, it comes on (sub-state 1), at a run in 7 cases of 11.
- The hero's second to fourth nearest hostiles within the band take places beside the target (sub-state 5; partly read).

Coming on (sub-state 1), it gives up farther than 2,000 (the AI's state 0); a creature of `+0x2B8` bit `0x10` takes a bow (`BOW_SHORT` or `BOW_LONG`, `0x4D4A80`) for a target beyond its reach + 100 and shoots; nearer than the band it steps back; else it walks or runs to the target, stopping within the band.
In the band (sub-state 2), a creature at full health away from home, farther than 2,500 or 5,000 from it, walks home (the leash; partly read); else it strikes: the attack action 3 (`0x4E4CF0`) with its target, and the AI back to state 0, from which the idle handler, once the action ends, takes it into state 4 again.

The attack action plays one of the model's attack slots (`0x5467A0`, action 4: 79 + 5 · the class of what it holds, docs/re/motions.md) at its attack speed (`0x584FE0`, at most 220) times `+0x210`, 1.0, raised by 0.1 up to 1.7 for each strike that a hero or an archer chains while the last one runs.
A strike hits (`0x4EA1F0`) at a fraction of its clip given by the clip's `MotionDesc +0xA0` (0.5 where that is 0), and a second time at `+0xA4` when that is above 0 or the creature holds two weapons: `floats_62[1]` and `[2]` of `tables/motions.ron` (docs/02 §4.1; 827 of the 989 attack clips carry a first fraction, 173 a second).
It lands when the target is then within three times the reach (1.5 times for a player's; **hypothesis** that the base is `R`), sending it the event 5 with the attacker, the weapon in hand and a factor of 1, or 0.5 for each of two weapons.
In one case of four an NPC whose target attacks plays another clip instead (**hypothesis**: a parry).

## At range: the AI's state 8

`0x4DBFC0` keeps an archer 300 to 800 from its target: within 300, up to ten times a fight, it backs off; beyond 800 it comes on, stopping within 450; between, without a straight line, it moves to a point near the target, turned −30° to +30°; else it shoots (the action 5, the arrow not read), its `+0x210` ramping between 250 and 650.
It gives up farther than 2,000, and goes back to melee without a ranged weapon.

## The hero's attack

A left click on a creature (`0x6172C0`) sends the hero the event 7: the AI's state 1 (`0x53A170`), which for a creature alive and hostile to the hero, without a dialogue, takes state 4, or 8 with a ranged weapon; a creature with a dialogue or a trader takes the talk (state 10) unless a hostile is within reach.
With Ctrl held the hero attacks at once, and a click on the ground strikes the air there.
In state 4 the hero runs the creatures' machine above, without fleeing or the leash, its band to `R + 45`, pushing a target nearer than the band, its chained strikes up to 1.7 times as fast; its hits resolve as the creatures'.

## A strike's resolution

The event 5 (`0x531DC8`) goes to `0x56B130`; a target dead or dying, talking, or farther than 1,500 from the attacker ignores it.
The attack record (`0x568D70`) gives, for each of the four kinds of damage (physical, fire, magic, poison), a damage rolled between a least and a most, the four resistances, the attack `A` and the defence `D`, from the weapons in hand, the items' bonuses and `Creature.pak`'s bonuses; the stats' recomputation (`0x5796A0`) scales them by the level and the attributes (the balance's `BalStatOff`, `BalChar*`).
An NPC's own come from its `Creature.pak` record and its level (below).

- At range, `A` keeps its value to `FernAWMin` and falls to `FernAWProz` of it at `FernAWMax`.
- Bosses and champions: the attacker's `A` times `AWFakBoss` or `AWFakChamp`, the defender's `D` times `VWFakBoss` or `VWFakChamp`; a boss or champion of level 1 or 1000 takes, when first fighting, the attacker's level + `LevelUpBoss` or `LevelUpChamp`, within its sector's levels raised by `OffLevel` and `LevelKap` (`0x550E50`).
- The difficulty: an NPC attacking a player has `A` times `ProzAW` and its damages times `ProzDmg`; a player attacking an NPC meets `D` times `ProzAW` and resistances times `ProzRes`.
- The hit chance (`0x43EEE0`) is `p = 1 − 1/(1 + A/D)^k` with `k = −log₂(1 − 0.5) = 1`, that is `A/(A + D)` (`A` raised for a combat art); the strike misses when a roll of 0 to 1000, over 1000, exceeds `p`.
- Each kind's damage, through the same function, becomes `damage²/(damage + resistance)` (a larger `k` for an NPC attacker above the defender's level), times `BalanceDmg` and `BalanceRes`, then times `(100 − damping)/100`, the defender's `Creature.pak +0x42` for the kind and the difficulty; criticals, life leech and the skills' specials follow (partly read).
- The health falls by the sum, not below 0, and the handler shows the number over the target.

A creature at health 0 takes the AI's state 9 (`0x546060`, `0x53E7C0`): it plays its dying action 6, runs its death's scripts and leaves its group; the kill (the event 9) sends its killer the experience `0x57D860`: `Creature.pak`'s experience `a + E · b` (`+0x08`, `+0x0A`), times `ProzExp`, times `(LevelKap/E)²` above `LevelKap` on the first three difficulties, times the killer's bonuses, times the factor `0x551530`: 0.04 up to `t`, `(r − t)/(1 − t)` up to `r = 1`, 1 up to `ObereEXPVoll`, then `ObereEXPVoll/r`, at least 0.04.

## An NPC's attack, defence and damage

Status: **confirmed** by code (#281) unless marked otherwise.
A creature's stats (`+0x3A8`) are made by `0x586C00` with its type: the six base attributes `b0`…`b5` (`+0x4A`…, `u16`) are `Creature.pak +0x0C`…`+0x11`, the extra points (`+0x1C`…) 0, the skill slots `Creature.pak +0x14`, `+0x15` and, for a type not of class 1, `+0x16`…`+0x1B`, the row of the table of `A` and `D` (`+0x126`) 6 (a hero's loader `0x585C60` sets 0–5).
The level's setter `0x564D60` gives an NPC (an id above 16) only its first `n` skill slots, each at level `⌊L/3⌋`: `n` is 0 below level 3, 1 for 3–6, 2 for 7–20, 3 for 21–35 and 8 above; then it recomputes the stats.
The recomputation `0x5796A0` (also from `CreateNPC` and some forty other places: the equipment, the effects, the level-ups), with `L` the level:

- The attributes `a_i = e_i + b_i + ⌊b_i·(L − 1)/10⌋` (`+0x10`…), `e_i` the extra points, 0 for an NPC, plus the items' attribute bonuses (the effects `0x32F`…`0x334`), not below 0.
- The level's factor `F = (156 − BalStatOff)·L/156 + BalStatOff + 9`: 29.87 at level 1, 37.72 at level 10.
- The attack `A = (a0·T0 + a2·T1)/100` and the defence `D = (a0·T3 + a2·T4)/100`, plus each item's `A` and `D` times `T2/100` and `T5/100`, `T` the row `+0x126` of a table of floats compiled in at `0x956C80`, whose rows are all `50, 50, 100, 20, 80, 100`: `A = (a0 + a2)/2`, `D = 0.2·a0 + 0.8·a2`.
  The items' effects, the effect list and the skills raise their multipliers (`+0xE6`, `+0xEA`, 1 at first).
- The damage of each kind (physical, fire, magic, poison), a most and a least: each item's adds its own; a creature **unarmed**, holding a thing of class 0 (`0x546F80`, the class of docs/re/motions.md) and nothing in its right hand, takes `Creature.pak`'s bonuses, and if its physical most is still 0, the most `0.15·(2·a0 + a2) + L/2` and the least `0.175·(a0 + a2) + L/2`.
- `Creature.pak`'s bonuses: six words at `+0x2A` (the low byte a kind, the high a target), their values at `+0x36` and classes at `+0x3C`; those of class 0 apply here, others against an opponent of that class in the attack record (no record has one).
  The kinds 1 to 7 give `x = value · s/100` with `s` the attribute `a0`, `a1`, `a3`, `a4`, `a2`, `a5` or the level `L`; the targets 1–4 add `x` to that kind's most and `x/2 + 1` to its least (unarmed only), 5–8 add `x` to that kind's resistance.
  The kinds 8–14 and 16–22 raise the multipliers of the damages and the resistances, others do other things; the data uses only the kinds 1, 2 and 5.
- Each kind's damage multiplier is 1 plus the bonuses' and the items' plus a part of the **base** attributes: physical `(F·b0/10 + e0)/BalCharPD` (with a ranged weapon `b2`, `e2`), fire `(F·(b0 + b2)/10 + 2(e0 + e2))/BalCharFD`, magic `(F·b4/10 + e4)/BalCharMD`, poison `(F·(b1 + b5)/10 + 2(e1 + e5))/BalCharGD`.
- The resistances: physical `(a3 + a1)/BalanceResPh`, fire `a2/BalanceResFe`, magic `a4/BalanceResMa`, poison `(a5 + a1)/BalanceResGi`, plus the items', the effects' and the bonuses'; each times `1 + (F·b1/10 + e1)/BalChar{PR,FR,MR,GR}` and the skills'.
  A creature with `+0x14` bit `0x200000` has the physical, fire and poison multipliers doubled, the magic one halved.
- All are held at 0 and above.

The attack record (`0x568D70`) multiplies each kind's least and most by its multiplier, the resistances by theirs, and `A` and `D` by theirs, truncated when not 1.
It rolls each kind's damage as a whole number between its ends in hundredths when the physical most is below 100, in tenths below 1,000, in units below 10,000; a negative most counts as 1 and a negative least as 0.
If every kind's damage is below 1, the physical one becomes 15.
Only the local player's hero then has a factor of its own.

The skills (`0x566070`) give, for a slot of level `s`, `v(s) = off + 2·(w − off)·(1 − 1/(1 + (s − 1)/S))` per cent from a triple of balance keys: the skill 2 raises every damage multiplier (`WKoff`…), 8 `A`'s and `D`'s, 9 `D`'s, more with a shield, 10 the regeneration and the health, 11 each resistance's; the weapon skills raise `A`'s and the attack speed while the creature holds the matching weapon.

The regeneration value (`+0x138`, the creature's `+0x4E0`) is `2·a3`, without the items, times the skill 10's; on the difficulties 3 and 4 a creature hostile to the player has `b3` and `a3` raised by half or doubled, and its speeds 170 and 160 or 200 and 190.
The attack speed (`+0x3C`) is 140 for every creature but the type 5's 100, raised by items, the weapon skills and effects, held within 100 and 220.

For example, a wolf (512) of level 1, base 8, 10, 18, 13, 0, 1, has `A` 13, `D` 16, a physical damage of 3.33 to 4.43 (the bonus (1, 1) of 50, then 1.109) and a physical resistance of 4.48 (the bonus (2, 5) of 30); a goblin warrior (76) of level 1, unarmed, `A` 18.5, `D` 20.6, a physical damage of 12.04 to 21.67 and a physical resistance of 6.62.

## Health

A creature's health is its stats' (`+0x3A8`) `+0x130` (the creature's `+0x4D8`), its most `+0x12C` (`+0x4D4`) and the value shown `+0x128` (`+0x4D0`), which follows the health over time.
The most (`0x5658F0`) comes from `Creature.pak`'s base bytes `b0`, `b2`, `b3` (`+0x0C`, `+0x0E`, `+0x0F`) and the level `L`: `S = b0 + b3 + ⌊(L − 1)·b0/10⌋ + ⌊(L − 1)·b3/10⌋`, `g = 1.5 + 0.004 · max(0, 60 − b0 − b3)`, the most `⌊(b0 + b2) · S^(g − 1) · 10/31⌋`, raised by the skill 10, by `HPFakBoss` or `HPFakChamp`, and for a creature not a hero by a factor of its level between the balance's `HP_s1`…`HP_w5` points and by `ProzHP`; 0 becomes 100, and a new creature starts full.
It regenerates (`0x562730`) by its regeneration value (`+0x4E0`, above) times `RegInFight` for an NPC, and for a player `RegInFight` fighting, `RegEnemyNear` with a hostile within 1,000, else `RegNoEnemy`.
The value times the rate times the frame's time (**hypothesis**: in seconds) adds to an accumulator, each 4,096 of which gives 1 health; the remainder kept is the accumulator's low ten bits, not twelve (a quirk).

## Open

The items' generation (`wpmod.bin`, `0x5D5080`) and so an armed NPC's damage; the weapon skills' predicates and the combat arts; the special behaviours of `+0x25F` and the bosses (`0x4D4E50`); the AI's state 4 from sub-state 5 on; the arrow (the action 5) and its range; the criticals and the skills' specials; whether a hit makes a creature that failed the test of the levels fight back (**hypothesis**: yes, by the event 5).

## In `opensacred-game`

The creatures do not fight yet (docs/decisions/2026-10-20-idle.md); the features follow in #285 and its sub-issues (#277–#284).
The balance's rows of six by the difficulty are named in `tables/balance.ron` (#277; `Balance::row_f32`).
Every NPC has a health (#278, `fight::Health`), full when it is made: its level the hero's, 1, held within its sector's (`fight::level`), jittered for a fill's creature; its most by `0x5658F0` from its base and its level, with the level's factor and bronze's `ProzHP` (`fight::most_health`; docs/decisions/2026-10-20-health.md); the overlay and the `state` command show the nearest creature's.
The hostile creatures come at the hero (#279, `ai::pursue`): awake, one that takes on the hero (`fight::takes_on`, every one at the hero's level 1) or has it within its reach comes at a hero within 800 to whom a straight line of walkable cells leads, its group's members with it in one case of two; it keeps the band of a reach of 130 about the hero, coming on at a run in 7 cases of 11 and after the hero as it moves, stepping back when nearer, turned to the hero in the band, and gives up farther than 2,000 (docs/decisions/2026-10-20-pursuit.md).
In the band it strikes (#280, `npcs::Strikes`): one of its model's attack clips (`motions::Slots::attacks`) at random, once, at the game's speed, which hits at the clip's fractions from `tables/motions.ron` (0.5 for a first of 0; a second where there is one) when the hero is then within three times the reach, sending a `fight::Hit` of a share of 1, or 0.5 for a creature with two weapons (#295); it decides again when the clip ends.
A hit resolves (#282, `fight::resolve`): every NPC has its attack, defence, damage and resistances by its base attributes, its level and its bonuses (`fight::combat`, above; an armed one's damage the default's), and a hit lands by the chance `A/(A + D)`; each kind's damage rolled, then `d²/(d + r)` less the damping of bronze, the sum times the share, whole, takes the health down, and the number rises over the target for a second.
At 0 the NPC plays one of its dying clips (the slots 173 and 174; **hypothesis** for the action 6) once and lies there, without its mind, its walk, its strike or its place in the way (`npcs::Dies`).
No skill, criticals or death scripts follow (docs/decisions/2026-10-20-hits.md).
The hero attacks (#283, `attack`): a left click on a hostile creature alive, within 45 of the cursor's line of sight up to 110 over its feet, makes it the hero's target, and the press does not steer the hero while it is held; the hero runs at it until within the band `R + 45`, 175, then stops, turns to it and strikes with one of its model's attack clips for bare hands at random, at the game's speed, hitting at the clip's fractions when the target is within 1.5 times the reach, 195, with a share of 1; it strikes again until the target dies, and a click elsewhere ends the attack with its strike.
With Ctrl held it strikes once, at once, where it stands, at the creature or in the air.
Its attack, defence, damage and resistances are its type's record's (`game::Class::creature`) by the same recomputation as an NPC's, unarmed, at level 1, and a kill earns it the creature's experience by `0x57D860` and `0x551530` (`fight::experience_of`), which the overlay shows; it gains no level yet (#300; docs/decisions/2026-10-20-hero-attack.md).
The hero takes the creatures' hits (#292): its health is its type's record's by `0x5658F0` without a creature's factors (`fight::hero_health`), and between the hero and a creature bronze's factors weigh a strike (`fight::weighed`): the creature's attack times `ProzAW` and its damages times `ProzDmg` when it strikes the hero, with the exponent `k` of a creature above the hero's level, and the creature's defence times `ProzAW` and its resistances times `ProzRes` when the hero strikes it.
The hero regenerates by its value `2·a3` times `RegInFight` while it attacks, `RegEnemyNear` with a hostile within 1,000, else `RegNoEnemy`, each 4,096 one health (`fight::Regenerates`); the overlay shows its health.
At 0 it plays one of its dying clips, the creatures leave it, and five seconds later it rises at the game's start with its health full (`hero::fall`; docs/decisions/2026-10-20-hero-health.md).
