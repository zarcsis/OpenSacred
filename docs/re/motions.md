# Motions: the clip a creature plays

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra 12.1.4 on 2026-10-19 (#153).
A model's motion slots are in [docs/02 §4.1](../02-data-formats.md#41-the-mdlmdt-container-and-caches) (`ModelDesc +0x70`); the converter keeps them with each clip, as `extras.slots` of its glTF animation.
The weapon kinds are in [docs/02 §6.2](../02-data-formats.md#62-creaturepak-cif-and-weaponpak-wpn), the hands in [docs/re/equipment.md](equipment.md).
Status: **confirmed** on the code unless marked otherwise.

## The slots of the actions

`0x5467A0` gives the motion slot that a creature plays for an action, of six: 0 standing, 1 walking, 2 running, 3 the fight stance, 4 an attack, 5 defending; for another number it gives 0.
`0x593320` then plays that slot of the creature's model, blending from the clip before; where the model has nothing in the slot, it plays nothing new.

A table at `0x956E40` gives each action a row of 20 slots, one for each class of what the creature holds (below):

- standing, 1 to 13 for the classes 0 to 12;
- walking, 27 to 39;
- running, 40 to 52;
- the fight stance, 14 to 26;
- defending, 53 to 65;
- an attack, the first of five, 79 + 5 × the class.

Class 13 has slots of its own (235 standing, 237 walking, 238 running, 236 the stance, 241 an attack, 239 defending), class 14 takes class 12's, and the rest of each row is 0.

The classes agree with the names of the clips in the slots, over the 231 converted models that have a slot 1: 0 `BH` (bare hands), 1 `1H`, 2 `2H`, 3 `2H_AXT`, 4 `STAB`, 5 `DOLCH`, 6 `WURF`, 7 `BOGEN`, 8 `2WAFFEN`, 9 `1H_KLINGE`, 10 `2H_KLINGE`, 12 `ARMBRUST`; no model has a clip named for class 11.
Most models fill few slots: a thief's idle `THFM_IDLE_BH` fills 1, 14 and 21, its walk 27, its run 40, and its `THFM_HORSE_IDLE`, a rider's idle in the saddle, 169.

## Where the model lacks the slot

`0x5467A0` tests the model's slot (`0x4117B0`) and, where it is empty:

- standing, takes slot 2 where the model has it, else 1;
- walking, takes the first of 27, 28 and 29 that the model has, as a walk (it sets the creature's gait, `+0x1F8`, to 1), then of the runs 40, 41 and 42, as a run (gait 2), else the standing slot as above;
- running, takes the runs first, as a run, then the walks, as a walk, else the standing slot;
- in the fight stance or defending, takes the standing slot of its class from the table.

A creature that walks on a run's clip so moves at its run speed, which `0x4F75F0` takes by the gait (docs/re/movement.md, "The speed").

## The class of what a creature holds

`0x546F80` gives the class from the creature's two hands, the left (`+0x1D4`) and the right (`+0x1D8`), each counted when it holds a `cWeapon3D`:

- with a weapon in each hand and the left one not a shield (category 13), class 8, two weapons, unless neither has a kind, then 0;
- otherwise the kind (`Weapon.pak +0x1E`) of the right hand's weapon, else of the left's, else 0 for bare hands; a weapon without a `Weapon.pak` record has kind 0;
- a kind of 14 counts as 12 (unless its caller asks otherwise; `0x5467A0` does not).

A creature whose flag `+0x14` has bit `0x2000` takes class 0 whatever it holds.
A soldier with a sword and a kite shield is of class 1, one with a crossbow 12, a thief with a dagger 5, a dark elf with two blades 8.

## The arts' slots

A combat art's executor plays its own slot of the model for the art and the class of what the creature holds, not through `0x5467A0`'s table: the Gladiator's 201 to 222, the Seraphim's 153 and 211 to 216 (docs/re/arts.md, "The arts' clips", #509).
The other classes' executors take the same slots for their own `Rundum`, `Hardhit` and `Attacke` (219 to 225, 201 to 207, 210 to 216), 248 and 247 for the Dwarf's guns, 194 on horseback, 144 to 154 for the kicks and the Vampiress's arts, and the action 4's clip where a class has no slot of its own (docs/re/arts.md, "The other classes' clips", #510).

## The other cases

Before the table, `0x5467A0` decides some creatures' slots itself:

- creature 71, the small spider, always plays slot 27;
- a creature whose flag `+0x14` has bit 2 and whose owner (`+0x1EC`) is a creature that `0x4266F0` accepts (**hypothesis**: a rider on its horse) plays 169 where its model has it, else 166, the `*_HORSE_IDLE` and `*_HORSE_RIDE` clips, and 193 to 196 or 234 attacking, by its class;
- a mounted creature (`+0x1F4` bit `0x200000`) other than creatures 190, 335 and 336 plays 179;
- a standing creature whose flag `+0x200` has bit 6 clear takes the fight stance instead where its model has the stance's slot and the first value of a sorted list at `0xAD3CE4` is below 550 ("The fight stance" below);
- with that bit set, it takes the stance where its flag `+0x200` has bit `0x8000` or it is creature 2 holding class 2, and creatures 5, 6 and 9 have cases of their own;
- an attack has cases of its own: a random one of the five where the model fills more than one, and fixed slots for some creatures.

## The fight stance

Read 2026-10-20 (#155).
The list at `0xAD3CE4` is the hero's, not the standing creature's: the distances from the local player's hero to its nearest hostile creatures, and its first value the nearest's.

- `0x547070` fills it, called only by the creatures' update `0x5375C0` (at `0x53918C`), in its block that runs every period (`+0x204`: 0.3 s for the players' heroes, 5 s for the others, set by the constructor `0x52DE40`), at the first world tick that takes the timer above it, keeping what is above (docs/re/ai.md, "The world's tick", #617), and only for the creature whose id is the local player's, the hero.
- It takes the creatures (types of category 2 or 3, alive) of the hero's sector and its eight neighbours (`0x634340`) that are hostile to the hero (`0x548F30`, the hero first, or `+0x1F4` bit `0x40000`), whose list at `+0x588` is empty and whose distance is below a range: 1,500 in a network game, `(S + 45) · 25` in the single player's (`S` the float at `+8` of the timer `0x6413E0`, a smoothed rate of the frames, **hypothesis**: about 1,900 to 2,600 world units); at most 55, nearest first, the distance `√(dx² + dy²)` in world units (550 is about 10 cells).
- It keeps the count apart (`0xAD3BC8`, 0 while the hero is dead), which the AI and the scripts' getters read first; but it writes the values only when it finds a hostile, and nothing else writes or clears them, so the first value is 0 when the process starts and keeps the last distance found when none is near.
- `0x5467A0` compares that first value with 550 without the count (at `0x546A7E`).
- Bit 6 of `+0x200` is set by the constructor alone (with `0x80000`) for a creature of the heroes' class (`0x426650`, the class record's `+4` is 1) of id 1, or of id 16 or below in a network game: in the single player's game, the hero only, the NPCs of the heroes' classes not.
- Bit `0x8000` of `+0x200` is the hero's, set and cleared by `0x547070`: set while the nearest hostile is within 350, or within 1,000 when the hero holds a weapon of the kinds 7, 12, 13 or 14 (`0x427A40`, `0x427ED0`; **hypothesis**: the bows and the crossbows); with it set and the list not empty, `0x5375C0` posts the engine's event `(0x10, 2, 1)`, else `(0x10, 2, 0)` (**hypothesis**: the fight's music).
- A standing creature's action (`+0x150` 0, `0x544890`) asks `0x5467A0` again on every update (at `0x546579`) and changes its clip when the slot changes, at once where less than 0.15 s of a clip played once is left, else blending over 0.15 s (`0x593320`).

So every standing NPC whose model has the stance's slot for what it holds takes its fight stance while a hostile creature is within about 10 cells of the hero, wherever the NPC stands, and the hero takes its own within 350 (or 1,000 with a bow).
By the code, the NPCs also take it at the start of the process, before the hero has seen a hostile, and keep it after the hero kills the last hostile near it until another is found; not checked in the game.

## In `opensacred-game`

`motions` picks a clip as `0x5467A0` does for standing, walking, running and the attacks (`motions::Slots::attacks`, docs/re/fight.md), by the table and the fallbacks above, from the slots that the model's file keeps, and the class of what the creature holds as `0x546F80` gives it.
`npcs` gives each NPC of the scripts and the fills the clip it stands in and, for one that a script sends somewhere, the clip it walks with, by the class of the items in its hands (docs/re/equipment.md); one whose walk falls back to a run moves at its creature's run speed.
The hero stands, walks and runs, takes its fight stance and attacks by the class of the items its template puts in its hands (#488, `npcs::Npcs::hero_gear`), the class's preset clips only where its model's slots cannot be read: the Seraphim with her weapon of kind 1 those of class 1.
The other cases are not followed: no NPC rides or is mounted yet, and the small spider walks by the table.
The fight stance follows the code (#219): `crowd::Nearest` keeps the distance from the hero to its nearest hostile creature, looked for every 0.3 s of the world's ticks (#617) in the 3 × 3 sectors about the hero within 1,875 world units (the client's range at 30 frames a second) and kept while none is found, 0 until one is; a creature is hostile there when its flags word has bit 0 or 1 (**hypothesis** for the client's test of hostility to the hero).
While that distance is below 550, every standing NPC whose model has the stance's slot for what it holds plays its stance (`motions::Slots::stance`, `npcs::stance`), else its idle, the clips blending over 0.15 s; so the NPCs that have one stand in it from the start, as the client's 0 at the start of the process has them (not checked in the game).
The hero stands in its own stance (its bit `0x8000`, #230) while the last look found a hostile within 350 units, its model's stance for bare hands (the hero holding nothing yet, the bows' 1,000 does not arise), else in its class's idle, blending as its other clips do.
The hero keeps its class's clips by name (`game::Class`).
