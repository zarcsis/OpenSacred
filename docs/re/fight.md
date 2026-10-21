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
Otherwise it attacks a target within 800, alive and hostile to it (`0x422820`), when a straight line of walkable cells leads to it (`0x4FCC80`); without one it goes for the nearest closed door of its sector (`0x4D4800`, "Doors in the way" below), staying idle when there is none, and when no way leads to it at all (`0x4F7710`) it takes its idle action.
It takes the AI's state 8 with a ranged weapon in a hand (item category 5, weapon kinds 7, 12, 13, 14), else 4; in a group, in one case of two, it raises the group's alarm (`0x46AE70`, docs/re/ai.md, "Groups").
The reach `0x4DBC60(a, b)` is the mean of two sizes by type: 130 by default, the heroes 70 to 130, the monsters 70 to 480.

## Melee: the AI's state 4

`0x4E0300` keeps a creature within a band about its reach `R`: `R − 15` to `R + 15` (`R + 45` for a player's).
Deciding (sub-state 0):

- At a third of its health or less, within 199 of its target, not a boss nor of the classes 1 and 5, once a fight (`+0x1F4` bit `0x200`), in one case of eight a frame while its target does not attack, it flees 700 from the target along the line.
- Within the band it strikes (sub-state 2); outside it, it comes on (sub-state 1), at a run in 7 cases of 11.
- Then a creature that is the second, third or fourth of the hero's list of hostiles (`0xAD3CE0`, docs/re/motions.md, "The fight stance"; the list's count above 1) and stands nearer than `R − 15` to the first of that list takes sub-state 5 instead (`0x4E1404`).

Coming on (sub-state 1), it gives up farther than 2,000 (the AI's state 0); a creature of `+0x2B8` bit `0x10` takes a bow (`BOW_SHORT` or `BOW_LONG`, `0x4D4A80`) for a target beyond its reach + 100 and shoots; nearer than the band it steps back; else it walks or runs to the target, stopping within the band.
In the band (sub-state 2), a creature at full health away from home, farther than 2,500 or 5,000 from it, walks home (the leash; partly read); else it strikes: the attack action 3 (`0x4E4CF0`) with its target, and the AI back to state 0, from which the idle handler, once the action ends, takes it into state 4 again.
A creature standing in sub-state 2 strikes even beyond the band when a straight line leads to the target; without one it looks for a way to the target (`0x4F7710`) and goes on (sub-state 0) when the way ends within `R + 80` of the target along each axis, else goes for a door ("Doors in the way"), and without a door gives up (the AI's state 0; `0x4E4931`).
A creature still moving (the action 1) in sub-state 2 is steered by its target alone: within the band it strikes at once, beyond it it walks straight to the target, nearer it steps back.

Places (sub-state 5, one frame, read 2026-10-21, #383; `0x4E224E` to `0x4E23A4`):

- By default the creature's point is its target's position, its move to stop at `R` (`+0x174` 15, `+0x176` `R`, `+0x17E` the target), and it walks there (the action 1) when farther than `R + 15` from the target.
- In place of that, the second of the list takes the point `R − 15` from its target along the world's `+x`, the third along `−y` and the fourth along `+y` (while the list's count is above 1, 2 or 3), its move to stop within 40 of the point (`+0x174` 1, `+0x176` 40: in the AI's state 4 the move `0x4F4770` ends within `+0x176` of its point when `+0x174` is 1), walking there when the point is farther than `R + 15` from it.
- Then a creature not of class 5 runs in one case of two (`+0x1F8` 2), and it takes sub-state 2 with its target.
- The AI runs at every frame, and at the next one sub-state 2 steers a moving creature by its target alone (above): a place holds for a frame, and the creature then strikes, comes on to the target or steps back as the others do (**hypothesis**: the spreading hardly shows in play).

The attack action plays one of the model's attack slots (`0x5467A0`, action 4: 79 + 5 · the class of what it holds, docs/re/motions.md) at its attack speed (`0x584FE0`, at most 220) times `+0x210`, 1.0, raised by 0.1 up to 1.7 for each strike that a hero or an archer chains while the last one runs.
A strike hits (`0x4EA1F0`) at a fraction of its clip given by the clip's `MotionDesc +0xA0` (0.5 where that is 0), and a second time at `+0xA4` when that is above 0 or the creature holds two weapons: `floats_62[1]` and `[2]` of `tables/motions.ron` (docs/02 §4.1; 827 of the 989 attack clips carry a first fraction, 173 a second).
It lands when the target is then within three times the reach (1.5 times for a player's; **hypothesis** that the base is `R`), sending it the event 5 with the attacker, the weapon in hand and a factor of 1, or 0.5 for each of two weapons.
That is `0x4EA3F0`, a function of its own, which `0x4EA1F0` calls near its end, after the checks of the fraction and of the distance (`0x4EA3BF`): it reads both hands, sets the factor and posts the event (read 2026-10-20, #322, #333).
It has 58 calls in 14 functions, 5 of them in the melee state's `0x4E0300`; the others are not followed.
In one case of four an NPC whose target attacks plays another clip instead (**hypothesis**: a parry).

## At range: the AI's state 8

`0x4DBFC0` keeps an archer 300 to 800 from its target: within 300, up to ten times a fight, it backs off; beyond 800 it comes on, stopping within 450; between, without a straight line, it moves to a point near the target, turned −30° to +30°; else it shoots (the action 5, "The archers" below), its `+0x210` ramping between 250 and 650.
It gives up farther than 2,000, and goes back to melee without a ranged weapon.

Its sub-states, read 2026-10-21 (#391), **confirmed** by code unless marked otherwise:

- Before them, a creature without a ranged weapon in a hand (`0x54A2A0`) and without `+0x200` bits `0x600000`, or one of `+0x14` bit `0x2000`, goes back to melee (the AI's state 4); within 150 of its target a creature of class 5 (`0x426A80`) whose `+0x25F` is not 10 does so in one case of four, its bow put away.
- Deciding (sub-state 0), it clears its count of back-offs (`+0x12B`); standing or moving (the action 0 or 1), it shoots when its target is from 250 to 650, and else, unless `0x4D4B70` sends it to melee ("The archers" below), takes the sub-state 1.
  In another action it waits: a shot that has left (the action 5, its sub-state not 0) raises `+0x210` by 0.1 up to 1.7, and any other action resets it to 1.
- Moving to its band (sub-state 1), against a target hostile to it (else its idle action), the distances integers of `0x5370B0`:
  - farther than 2,000 it gives up (the AI's state 0);
  - nearer than 300, not marked (`+0x1F4` bit `0x200`, which a boss's or a champion's levelling sets too) and with fewer than ten back-offs counted, it waits while it moves, and standing, unless `0x4D4B70` sends it to melee, backs off to the point as far beyond itself as its target is on the other side, at a walk with `+0x200` bit `0x800` in one case of two (**hypothesis**: walking backwards, facing the target), else at a run, a walk for class 5, and counts the back-off;
  - farther than 800 it comes on to its target's place, stopping within 450 (`+0x174` 1, `+0x176` 450), at its gait as it is, whether it moves or not;
  - from 300 to 800 without a straight line (`0x4FCC80`) it waits while it moves, and standing walks to its place turned about its target by `rand() % 61 − 30` degrees, its distance kept, stopping within 25;
  - else it shoots, whether it moves or not: the action 5 with its target, the AI's state 0 and the count of back-offs 0, from which the idle handler takes it into the state 8 again.
- A creature with an owner (`+0x14` bit 2 and `+0x1EC`) within 800 of its target skips the moves and shoots.

So an archer shoots from 300 to 800 along a straight line, from 250 when it has just decided, and nearer only after ten back-offs in a row; a back-off ends in a shot as soon as it is 300 away, and "ten times a fight" is ten times between two shots.

## Doors in the way

Status: **confirmed** by code (#383, read 2026-10-21) unless marked otherwise.
`0x4D4800` serves a creature not of the classes 2, 5 (unless its `+0x25F` is set) and 6 (`Creature.pak +0x04`, `0x426AD0`, `0x426A80`, `0x426A30`; by their models the beasts, the undead and the animals).
It takes the objects of the creature's sector that the script manager keeps (`0xAB4490`; a list of more than 1,000 is emptied instead, a guard) and of them the nearest door, straight from the creature: an object of category 10 with a trigger (`+0x34`) whose state (`+0x0A`) is neither barred (bit 15, which keeps a door from opening, `0x415F00`) nor open (bit 0) and whose flags word (`+0x04`) has bit `0x2000`, on the creature's layer or on the ground and the first floor either way.
That bit is a hero's mark: the door's event handler (`0x44BCD0`) sets it when the event `0x21`, 1 or 2 comes from an id of 16 or below, so a creature goes only for a closed door that a hero has opened or closed before.
With such a door it sends itself the event 7 with the door and its position, as the hero's click does (`0x6172C0`), and returns true; without, false.
The event 7 takes the AI's state 1 (`0x53A170`) with the door as its target:

- it walks to the door's outer point when it stands on the ground, its inner point when on a building's floor (`0x43B290`, docs/re/ai.md, "Houses and doors"), by a way (`0x4F7CE0`, `0x4F8AF0`), stopping within 20; with no way there, to a nearby cell of the ground nibble 9 or 10 (not read); already within 25 of the point along each axis, it stands;
- a creature of `+0x200` bit 2 (not read) does so only for the 19 gates, palisades, fences and cages among the door types (5674, 5676 to 5679, 5740 to 5750, 5756 to 5758), and for another door takes its `+0x1EC` as its target instead;
- standing (sub-state 2) within 300 of the door, on its layer or the ground and the first floor either way, it calls the quests' hook (`0x471790`, not followed) and, unless the door's object has flag `0x4000`, sends the door the event `0x21`; then the AI's state 0.

The door turns (`0x44BCD0`): one not barred opens when closed (its clip `0xAA`, the trigger's bit 0 set) and closes when open (`0xAB`); a barred one only closes; and the event from a creature that is no hero leaves open a door that a hero marked.
So the creature opens the door, and its idle state (`0x542B20`) finds its target again, the door's cells now walkable (docs/re/movement.md: a door's cell may be walked on while its trigger's bit 0 is set).
`0x4D4800` is called in three places: the idle state's choice when no straight line leads to the target (`0x5442E8`; without a door the creature stays idle); the melee's sub-state 2 when the target is beyond the band, no straight line leads to it and the way ends farther than `R + 80` from it (`0x4E4931`; without a door the AI's state 0); and the move (`0x4F4770`, at `0x4F5652`) in the AI's state 4 when it finds no way to its point twice, the second time after a jitter of up to 50 along each axis, while the local hero is alive and hostile to it (`0x549000`).
**Hypothesis**: the hero's left click on a door takes the same way, since `0x6172C0` sends the hero the event 7 with the object under the cursor (`0x626C50`) and the state 1 handles a door for any creature; that would answer #372, which found no door opening on the hero's side.

## Bosses and champions

Status: **confirmed** by code (#316) unless marked otherwise.
A boss is a creature of `+0x1F4` bit `0x80000`, a champion one of `+0x2B8` bit `0x40`.
`FillSector` makes them after the creature maker (`0x47E6B7`…`0x47E863`): the types 203, 342 and 349 always champions; 37, 48, 55 and 84 a boss or a champion by a coin; 127 and 49 the same in one case of two, else as the others; any other creature hostile to a living hero a champion when `rand() % 1001` is below `HarteSau` (20); 343 and 212 only take a champion's levelling (`+0xB0` bit `0x400`, which a champion has too).
They are made at the marker level 1; the black rider (256) is a boss at its fill's level.
`CreateNPC` makes a boss with the level tag `0x1F` of 1000 (at the marker level) or the tag `0x72`, never a champion; the script command `0x49E760` with the level 1000 makes a group's creatures bosses (**hypothesis**: `SetNPCState`).
At its first strike or hit (`0x56B130` for both sides, `0x550E50`), a boss or champion of level 1 or 1000 takes its opponent's level plus `LevelUpBoss` or `LevelUpChamp` (2 and 2), at least `OffLevel` plus its sector's lowest level, and, unless its type is "big" (`Creature.pak` flag `0x02`), at most `LevelKap` plus its sector's highest plus the same bonus; it is then marked (`+0x1F4` bit `0x200`), its health refilled and its worn items made again at the new level.
An archer makes this levelling in its state 8 against the local hero rather than its target.
Its most health is times `HPFakBoss` (2.5) or `HPFakChamp` (2.0), three times that for the types 701, 137 and 53, and 1.8, 2.8 or 4 times for a dragon or flyer (class 10) by the difficulty; a boss attacks with `A` times `AWFakBoss` (1.5) and defends with `D` times `VWFakBoss` (2.0), a champion with `AWFakChamp` (1.3) and `VWFakChamp` (1.5); its kill's experience is twice or 1.5 times.

## The archers

A creature of `+0x2B8` bit `0x10` takes a bow once (`0x4D4A80`): the state 8, its slot 13 emptied, `BOW_SHORT` (1732) or `BOW_LONG` (1733) at random made at its level and held in slot 12, the bit cleared.
In the state 8 a creature that another targets from within 1.5 times the reach goes back to melee in one case of two (`0x4D4B70`): its bow away, `SWORD` (1753) in one case of four, else `DAGGER` (1712) in one of three, else `SWORD_BASTARD` (1724) or `STREITKOLBEN` (1719), and most take the bit back, to switch to the bow again.
Read again 2026-10-21 (#391): `0x4D4B70` serves every archer, of the bit or not, whose id is above 16, that is no companion (`+0x251` 1 to 16) and whose `+0x1F0` is not 12, when its target stands within 1.5 times the target's own reach (`0x4DBC60`) and targets it: the target's AI's state 4 or 8 with it as `+0x104`, or the target's action 3 or 5, or its AI's state 6 or 11, with it as `+0x158`; the new weapon is made at its level (`0x5D5080`) and held in slot 13.
It takes the bit back in three cases of four, but a creature of class 5 other than the black rider (256) never, and the goblin warrior (76) only in one case of three of those.
Who first gives a creature the bit is not found: the creature maker (`0x480D40`) sets only bit `0x80` of `+0x2B8`.
It shoots when its target is between 250 and 650, else it moves to the band of 300 to 800 (above); a shot that has left raises `+0x210` by 0.1 up to 1.7 for the next, any other action resets it.
The shot is the action 5 (`0x4DCB80`): the attack clip of what it holds at the attack speed times `+0x210`, turned to the target.
A creature's target farther than 1,000 sends it back to the state 8.

The arrow, read 2026-10-21 (#383), **confirmed** by code unless marked otherwise:

- It leaves once what is left of the clip is at most `1 − a` of it, `a` the `MotionDesc +0xA0` of the clip in play (`0x4135D0`, the slot `+0x64`): at the fraction `a`, at once for an `a` of 0, at the half for an `a` of 1 or more (`0x4DD90E`; **hypothesis** that Granny's parameters 0, 1 and 3 read there are the clip's start, end and loops).
- It is an object of type 3070 (`ARROW_LONG.GRN`, a `cArrow3D`) made at the shooter's place and started by a `cEventArrow` (`0x4EB700`, the event 9, taken by `0x5BD2D0`) from the shooter's left hand (`Bip01 L Hand`, `0x44CA30`) toward its target's point (`0x44CBB0`: the target's bone of kind 3, else 0, with the `x` and `y` of the target's position), at the speed 600 a second; it carries the shooter, the target, the weapon's type and the hit's record (below), and leads no moving target.
  The bones' kinds name the bones of a table of 32-byte names at `0x8E7200` (`0x401000`): 0 `_Root`, 1 `Bip01 L Hand`, 2 `Bip01 R Hand`, 3 `Bip01 Neck`; so the point is the target's neck, else its root, and `0x44CBB0` moves it by up to 5 along each axis at random unless its caller asks otherwise (read 2026-10-21, #391).
- It flies straight along that direction, fixed in three dimensions at the start, without gravity: each frame its point moves by the frame's time times 600 (`0x5BB010`; the game's clock `0xCD7A60` counts seconds, `0x641520`).
  In a frame (read 2026-10-21, #391), an arrow whose time is out, or that hit, is destroyed first; then its time runs down, its next point is tested (`0x5BAA60`, below) and taken only when it may fly there, its time ending otherwise; and the hits are tested on the way from its last point to its next either way.
- It lives 1.5 s (`+0x234`), so it reaches at most 900 from the hand, and is destroyed when that runs out or after a hit.
- It also ends where its new point's cell may not be walked on (`0x636C10`), unless the cell's ground nibble is 2, which it flies over, or where its height falls to the ground's there (`0x636520`; `0x5BAA60`): statics do not stop it.
- Each frame it takes the objects on its layer, in the sectors of its last and new points, whose position lies nearer the segment between those points, in the plane (`0x6226B0`), than the sum of the two types' sizes (`items.pak +0x14`, 50 for 0, `0x428CE0`, `0x5FFC70`; on file the arrow's 50, the wolf's 45, the goblin warrior's 25, the heroes' 50); the height is not tested.
  Of them it takes its target when the target is one, else the first found.
- That creature is hit when it is alive (not dying, its health above 0), hostile to the shooter and the shooter alive (`0x548F60`), not of `+0x251` 1 to 16 (**hypothesis**: a player's companion, which follows the player of that id), and does not turn the arrow aside (`0x561090`, below).
  Anything else lets the arrow fly on, a friend or a corpse included; but the effects of type 847 (`FX_BUBBLETRAP`) stop it, and those of 776 and 778 (`FX_FIREBALL`, `FX_FIRESTRIKE`) with their `+0x68` 0 stop an arrow of the types 3069 to 3071.
- A hit stops the arrow, but for a ghost (`Creature.pak` class 11, `0x426BC0`) or a shooter of `+0x4F0` bit `0x10000000`, which it passes through, hitting again at most every 0.5 s (`+0x240`); a stopped arrow sticks into the model of a creature of size above 20 for 5 s (`0x5BAB40`, a ray against the model; not into the local player's hero), and the hit's sound plays by the weapon's type.
- The hit is the event 5 from the shooter, sent by the client that made the arrow (`+0x39`): the strike's resolution of `0x56B130` ("A strike's resolution" below), with the bow (the shooter's slot 12, or 13) as the weapon and a share of 0, which scales nothing, so the whole damage (`0x56D187`); its chance is `A·f/(A·f + D)`, `f` the range's factor of the distance between the shooter and the creature at the hit (1 to 250, down to 0.75 by 1,000, 0.25 from there).
  A shooter of `+0x184` set gives the hit's record the kind 2 (0 otherwise), which the resolution's branch of the combat arts takes (not followed).
  The bow's values are those of its item made by its template at the creature's level (docs/re/items.md).
- A shot makes `+0x25D` arrows, 1 to 20: `+0x261` for the arts `0x3D` and `0x3F` (below), else 1, times the value of the effect `0x15` for a creature of `+0x2B7` bit 4 (the multi-shot); 2 for a creature of `+0x200` bit `0x400000`; the `i`-th after the first is turned in the plane by `⌈i/2⌉ · 0.05` rad, to either side in turn, and with the effect `0x15` of value `n` each arrow's damage is times `Multipfeil^n` (0.83, `0x56D173`).
- The byte `+0x18E` (the action's block `+0x154` at `+0x3A`; **hypothesis**: the combat art in use, 0 for a creature's plain shot) picks special projectiles: 4, 6, 7, 8, `0x3D`, `0x3E` and `0x3F` make the types 3078, 3074, 3075, 3077, 3069, 3068 and 3079, fanned by 0.15 rad (4, 6, 7), round the circle (8; the type 3077 homes at two thirds of the speed) or by 0.01 rad (`0x3F`); 3068 is a ray of up to 25 steps of 40 against the cells, living 1 s; not followed further.
- A creature with the effect `0x19` (the client's log calls it a ranged deflection; **hypothesis**: a hero's) turns an arrow aside by the chance `c/(L + 9 + c)`, `c = (5v + 9)·k`, `v` the effect's value, `k` 2 for one source of it and 1 for the others, `L` the shooter's level: by the effect's source it destroys the arrow, sends it back at the shooter, or sends it off in a random direction (`0x561090`; not followed further).

`FernAll` (0.5) is used by no code found.

## The hero's attack

A left click on a creature (`0x6172C0`) sends the hero the event 7: the AI's state 1 (`0x53A170`), which for a creature alive and hostile to the hero, without a dialogue, takes state 4, or 8 with a ranged weapon; a creature with a dialogue or a trader takes the talk (state 10) unless a hostile is within reach.
With Ctrl held the hero attacks at once, and a click on the ground strikes the air there.
In state 4 the hero runs the creatures' machine above, without fleeing or the leash, its band to `R + 45`, pushing a target nearer than the band, its chained strikes up to 1.7 times as fast; its hits resolve as the creatures'.

## A strike's resolution

The event 5 (`0x531DC8`) goes to `0x56B130`; a target dead or dying, talking, or farther than 1,500 from the attacker ignores it.
The attack record (`0x568D70`) gives, for each of the four kinds of damage (physical, fire, magic, poison), a damage rolled between a least and a most, the four resistances, the attack `A` and the defence `D`, from the weapons in hand, the items' bonuses and `Creature.pak`'s bonuses; the stats' recomputation (`0x5796A0`) scales them by the level and the attributes (the balance's `BalStatOff`, `BalChar*`).
An NPC's own come from its `Creature.pak` record and its level (below).

- At range, with a weapon of the kinds 7, 12, 13 or 14 (`0x427ED0`), `A` keeps its value to `FernAWMin` (250) of the distance between the attacker and the target, falls linearly to `1 − FernAWProz` of it (0.75) toward `FernAWMax` (1,000) and is `FernAWProz` of it (0.25) from there, a jump (`0x56B449`, read 2026-10-21, #383); such an attacker of class 10 (the dragons and flyers) has its damage share scaled alike.
- Bosses and champions: the attacker's `A` times `AWFakBoss` or `AWFakChamp`, the defender's `D` times `VWFakBoss` or `VWFakChamp`; a boss or champion of level 1 or 1000 takes, when first fighting, the attacker's level + `LevelUpBoss` or `LevelUpChamp`, within its sector's levels raised by `OffLevel` and `LevelKap` (`0x550E50`).
- The difficulty: an NPC attacking a player has `A` times `ProzAW` and its damages times `ProzDmg`; a player attacking an NPC meets `D` times `ProzAW` and resistances times `ProzRes`.
- The hit chance (`0x43EEE0`) is `p = 1 − 1/(1 + A/D)^k` with `k = −log₂(1 − 0.5) = 1`, that is `A/(A + D)` (`A` raised for a combat art); the strike misses when a roll of 0 to 1000, over 1000, exceeds `p`.
- Each kind's damage, through the same function, becomes `damage²/(damage + resistance)` (a larger `k` for an NPC attacker above the defender's level), times `BalanceDmg` and `BalanceRes`, then times `(100 − damping)/100`, the defender's `Creature.pak +0x42` for the kind and the difficulty; criticals, life leech and the skills' specials follow (partly read).
- That function, `0x43EEE0(x, y, c, p, out)` (`cdecl`), returns `x·w` and writes the float `w = 1 − 1/(1 + x/y)^k` to `out` when it is not null, with `k = −ln(1 − p)/ln(1 + c)`; the resolution passes the base `c` 1, so `k = −log₂(1 − p)` (read 2026-10-21, #406).
  For `y` within ±0.001, the bounds included (the double 0.001, so that the float 0.001, a little above it, divides), it takes 10,000 for `x/y`: against a defence within ±0.001 the chance is 0.9999 whatever the attack, so the roll of 1,000 misses, and a damage `d` against a resistance within ±0.001 becomes `d·(1 − 10,001^−k)`, 0.9999·d for `k` 1, which the sum's truncation can take a point off (15 becomes 14.9985).
  **Confirmed** by the vectors that `opensacred-oracle` records from the function (#363).
- The health falls by the sum, not below 0, and the handler shows the number over the target.

A creature at health 0 takes the AI's state 9 (`0x546060`, `0x53E7C0`): it plays its dying action 6, runs its death's scripts and leaves its group; the kill (the event 9) sends its killer the experience `0x57D860`: `Creature.pak`'s experience `a + E · b` (`+0x08`, `+0x0A`), times `ProzExp`, times `(LevelKap/H)²` when the killer's level `H` is above `LevelKap` on the first three difficulties, plus the killer's bonuses, times the factor `0x551530`: 0.04 up to `t`, `(r − t)/(1 − t)` up to `r = 1`, 1 up to `ObereEXPVoll`, then `ObereEXPVoll/r`, at least 0.04 (the details in "The hero's levels").
Before the experience the same handler (`0x533BA8`) makes the creature's drops (docs/re/items.md, "Drops", read 2026-10-21, #404).

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

## The hero's levels

Status: **confirmed** by code (#300) unless marked otherwise.
The experience is the stats' `+0x0C` (`u32`, the creature's `+0x3B4`), the level the stats' `+0x56` (the creature's `+0x3FE`).
Level `L` ends at `X(L) = ⌊(L⁴ + 40L³ + 150L² + 200L − 90)/100⌋·100` experience (`0x57F6E0`; read-only constants at `0x957020`, among them a term `110·1.0^L`, constant): level 2 at 300, 3 at 1,200, 5 at 5,900, 10 at 49,500.
`0x57F8D0` gives the level of an experience, at most 216, and `addExperience` (`0x57E160`) holds the experience below `X(216)`, one less than 2,586,931,600, which checks the formula.
Each level gained gives one attribute point to spend (the stats' `+0x23`) and, for each level `p` reached, `⌈n/2⌉` skill points (`+0x42`), `n` the skill slots held, those still to learn (`+0x44`) and the levels of the balance's `skilllearn1`…`skilllearn6` (3, 6, 12, 20, 30, 50) crossed up to `p`; each of those levels crossed opens one more skill to learn.
The level's setter `0x564D60` then recomputes the stats by the new level, refills the health to its new most, and raises the hero's companions to its level; a level-up effect appears at the hero.
The kill's experience (`0x57D860`), in order: `a + E·b`; times `ProzExp` of the difficulty's column; times `(LevelKap/H)²` above `LevelKap`, read by the difficulty itself (50 on bronze, 120, 190, none above); a multiplayer bonus; the killer's bonuses (the items' effect 843 and others); a boss's twice, a champion's 1.5 times; times `0x551530`; 1 for none; times the decay of a high hero `0x57E050` (1 up to 99, `0.985^(H − 99)` to 124, `0.98^(H − 99)` to 149, `0.975^(H − 109)` from 150); times the fade of a difficulty outgrown by the row `StartLevMulti` read by the difficulty (on bronze 1 up to level 70, then `(101 − H)/31`); at least 1, whole.
In a party the kill is shared by `Teambonus^min(n − 1, 3)` and each member's `(L + 9)²` over their sum.

## Health

A creature's health is its stats' (`+0x3A8`) `+0x130` (the creature's `+0x4D8`), its most `+0x12C` (`+0x4D4`) and the value shown `+0x128` (`+0x4D0`), which follows the health over time.
The most (`0x5658F0`) comes from `Creature.pak`'s base bytes `b0`, `b3` (`+0x0C`, `+0x0F`), the stats' attributes `a0` and `a3` (`+0x10`, `+0x16`) and the level `L`: `S = b0 + b3 + ⌊(L − 1)·b0/10⌋ + ⌊(L − 1)·b3/10⌋`, `g = 1.5 + G · max(0, 60 − b0 − b3)` with `G` the float at `0x891298`, 0.0039963233, the most `⌊S^g · (a0 + a3)/S · 10/31⌋` in double precision, that is `⌊S^g · 10/31⌋` for a creature without extra points or items' attribute bonuses, whose `a0 + a3` is `S` (**confirmed** by code and by the vectors of `opensacred-oracle`, #363, #408; read before as `⌊(b0 + b2) · S^(g − 1) · 10/31⌋`), raised by the skill 10, by `HPFakBoss` or `HPFakChamp`, and for a creature not a hero by a factor of its level between the balance's `HP_s1`…`HP_w5` points and by `ProzHP`; 0 becomes 100, and a new creature starts full.
It regenerates (`0x562730`) by its regeneration value (`+0x4E0`, above) times `RegInFight` for an NPC, and for a player `RegInFight` fighting, `RegEnemyNear` with a hostile within 1,000, else `RegNoEnemy`.
The value times the rate times the frame's time (**hypothesis**: in seconds) adds to an accumulator, each 4,096 of which gives 1 health; the remainder kept is the accumulator's low ten bits, not twelve (a quirk).

## Open

The weapon skills' predicates and the combat arts; the special behaviours of `+0x25F` and the bosses (`0x4D4E50`); the AI's state 4 from sub-state 6 on; the special projectiles, the arrows' deflection (`0x561090`) and `FernAll`, which no code found uses; whether the hero's click opens a door (#372, "Doors in the way"); the criticals and the skills' specials; whether a hit makes a creature that failed the test of the levels fight back (**hypothesis**: yes, by the event 5); who first gives a creature `+0x2B8` bit `0x10`, which takes a bow; which creatures are hostile to one another (`sdk_HOSTILITY_PREDICATE`, `0x548F60`).

## In `opensacred-game`

The fight follows in #285 and its sub-issues, of which #284 is still open; what the game follows of it is below.
The formulas named below as `fight::…` live in `opensacred-rules` (`opensacred_rules::fight`, #360); `opensacred-game`'s `fight` keeps `fight::Hit`, the components and the resources that hold their values, and the systems that call them (`fight::resolve`, `fight::level_up`).
The balance's rows of six by the difficulty are named in `tables/balance.ron` (#277; `Balance::row_f32`).
Every NPC has a health (#278, `fight::Health`), full when it is made: its level the hero's when it is made, held within its sector's (`fight::level`), jittered for a fill's creature; its most by `0x5658F0` from its base and its level, with the level's factor and bronze's `ProzHP` (`fight::most_health`; docs/decisions/2026-10-20-health.md); the overlay and the `state` command show the nearest creature's.
The hostile creatures come at the hero (#279, `opensacred_rules::minds::Mind::pursue`): awake, one that takes on the hero (`fight::takes_on`, by the hero's level) or has it within its reach comes at a hero within 800 to whom a straight line of walkable cells leads, its group's members with it in one case of two; it keeps the band of a reach of 130 about the hero, coming on at a run in 7 cases of 11 and after the hero as it moves, stepping back when nearer, turned to the hero in the band, and gives up farther than 2,000 (docs/decisions/2026-10-20-pursuit.md).
In the band it strikes (#280, `npcs::Strikes`): one of its model's attack clips (`motions::Slots::attacks`) at random, once, at its attack speed (#306: 140 %, the Wood Elf's 100 %; `fight::attack_speed`), which hits at the clip's fractions from `tables/motions.ron` (0.5 for a first of 0; a second where there is one) when the hero is then within three times the reach, sending a `fight::Hit` of a share of 1, or 0.5 for a creature with two weapons (#295); it decides again when the clip ends.
A hit resolves (#282, `fight::resolve`): every NPC has its attack, defence, damage and resistances by its base attributes, its level and its bonuses (`fight::combat`, above; an armed one's with its items' values, below), and a hit lands by the chance `A/(A + D)`; each kind's damage rolled, then `d²/(d + r)` less the damping of bronze (`fight::hit_chance`, `fight::dealt`: 0.9999 against a defence or a resistance within ±0.001, #406), the sum times the share, whole, takes the health down, and the number rises over the target for a second.
At 0 the NPC plays one of its dying clips (the slots 173 and 174; **hypothesis** for the action 6) once and lies there, without its mind, its walk, its strike or its place in the way (`npcs::Dies`).
No criticals, life leech or death scripts follow (docs/decisions/2026-10-20-hits.md).
The hero attacks (#283, `attack`): a left click on a hostile creature alive, within 45 of the cursor's line of sight up to 110 over its feet, makes it the hero's target, and the press does not steer the hero while it is held; the hero runs at it until within the band `R + 45`, 175, then stops, turns to it and strikes with one of its model's attack clips for bare hands at random, at its attack speed, hitting at the clip's fractions when the target is within 1.5 times the reach, 195, with a share of 1; it strikes again until the target dies, and a click elsewhere ends the attack with its strike.
With Ctrl held it strikes once, at once, where it stands, at the creature or in the air.
Its strikes on a target are chained (#312, `opensacred_rules::minds::Chain`): each after the first plays 0.1 faster than the last, up to 1.7 times, and coming on again breaks the chain.
The fills make bosses and champions (#318, `fight::Rank`): by their types and by `HarteSau`, at the hero's level plus `LevelUpBoss` or `LevelUpChamp` within their sector's, their health, attack, defence and kill's experience times their factors (docs/decisions/2026-10-20-bosses.md).
The hero's attack, defence, damage and resistances are its type's record's (`game::Class::creature`) by the same recomputation as an NPC's, unarmed, at its level, and a kill earns it the creature's experience by `0x57D860` and `0x551530` (`fight::experience_of`), which the overlay shows (docs/decisions/2026-10-20-hero-attack.md).
The hero takes the creatures' hits (#292): its health is its type's record's by `0x5658F0` without a creature's factors (`fight::hero_health`), and between the hero and a creature bronze's factors weigh a strike (`fight::weighed`): the creature's attack times `ProzAW` and its damages times `ProzDmg` when it strikes the hero, with the exponent `k` of a creature above the hero's level, and the creature's defence times `ProzAW` and its resistances times `ProzRes` when the hero strikes it.
The hero regenerates by its value `2·a3` times `RegInFight` while it attacks, `RegEnemyNear` with a hostile within 1,000, else `RegNoEnemy`, each 4,096 one health (`fight::Regenerates`); the overlay shows its health.
At 0 it plays one of its dying clips, the creatures leave it, and five seconds later it rises at the game's start with its health full (`hero::fall`; docs/decisions/2026-10-20-hero-health.md).
The skills count (#293, `fight::skills`): an NPC's first slots by its level at a third of it, a hero's two at level 1; by their curves (`0x566070`) the skill 2 raises the damages' multipliers, 8 the attack's and the defence's, 9 the defence's (more with a shield), 11 the resistances', truncating an attack or a defence so raised, and 10 the regeneration (`fight::combat`, `fight::regeneration`).
Every creature regenerates, an NPC at `RegInFight` (`fight::Regenerates`; docs/decisions/2026-10-20-skills.md).
A creature fighting the hero flees (#304, `opensacred_rules::minds::Mind::pursue`): at a third of its health or less, within 199 of the hero, not of the classes 1 and 5, once a fight, between its strikes while the hero does not strike, in one case of eight a frame, it runs 700 from the hero along the line, and comes on again after; in the band at full health, farther than 2,500 from home, it walks home and leaves the fight (docs/decisions/2026-10-20-pursuit.md).
The hero gains levels (#300, `fight::Experience`): its kills' experience by the cap, the decay and the fade of bronze, its level by `X(L)`, an attribute point and the skill points of each level counted but not spent yet; a new level makes its values, its most health and its regeneration anew and refills its health (`fight::level_up`); the creatures made afterwards take its level within their sectors', and their test of the levels weighs it; the overlay shows the hero's level (docs/decisions/2026-10-20-levels.md).
An armed NPC fights with its items' values (#309, `fight::Templates`): each item in its hands is made at its level by its template (docs/re/items.md), its damage, resistances, `A` and `D` added to the NPC's in place of `Creature.pak`'s damage bonuses (docs/decisions/2026-10-20-weapons.md).
The archers keep their distance and shoot (#391, `opensacred_rules::minds::Mind::pursue`): a creature with a ranged weapon in a hand (the kinds 7, 12, 13 and 14) fights in the state 8, by its sub-states above; deciding, it shoots from 250 to 650; nearer than 300 it backs off, as far again from the hero, at a walk or at a run by a coin, up to ten times between its shots; farther than 800 it comes on after the hero, stopping within 450; between, without a straight line of walkable cells, it goes round to its place turned about the hero by up to 30° either way, stopping within 25; else it stops, turns to the hero and shoots; farther than 2,000 it gives up.
A shot (`npcs::Strikes::shot`) plays one of its model's attack clips for the class of what it holds, at its attack speed times its chain, each shot 0.1 faster than the last up to 1.7 times, and at the clip's first fraction (at once for 0, the half for 1 or more; `opensacred_rules::minds::looses`) looses an arrow (`arrows`, `opensacred_rules::arrows`): `ARROW_LONG.GRN` from the shooter's left hand toward the hero's neck, at 600 a second for 1.5 s, ending on a cell that may not be walked on but the ground's nibble 2 (`terrain::Heights::flown_over`) or at the ground.
In each frame it takes the hero, or else the first creature on its level, nearer the part of its way it flew than the sum of the two sizes (`items.pak +0x14`, 50 for 0, `arrows::Size`), and hits it when it is the hero alive and the shooter alive and hostile to it: the fight's hit at range (`fight::Hit`), the whole damage of the shooter's values with its bow, its attack times the range's factor of the distance between them (`opensacred_rules::fight::range_factor`: 1 to 250, down to 0.75 by 1,000, 0.25 from there); the arrow is then gone, as the original's that hits the local player's hero, and one that hits a creature of a size above 20 would stick in it for 5 s.
The archers' arrows hit only the hero, the switch to melee (`0x4D4B70`) and the bow-taking (`0x4D4A80`) are left out, and an archer whose model has no attack clip for its weapon does not shoot (docs/decisions/2026-10-21-archers.md).
