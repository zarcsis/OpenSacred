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

## The other cases

Before the table, `0x5467A0` decides some creatures' slots itself:

- creature 71, the small spider, always plays slot 27;
- a creature whose flag `+0x14` has bit 2 and whose owner (`+0x1EC`) is a creature that `0x4266F0` accepts (**hypothesis**: a rider on its horse) plays 169 where its model has it, else 166, the `*_HORSE_IDLE` and `*_HORSE_RIDE` clips, and 193 to 196 or 234 attacking, by its class;
- a mounted creature (`+0x1F4` bit `0x200000`) other than creatures 190, 335 and 336 plays 179;
- a standing creature whose flag `+0x200` has bit 6 clear takes the fight stance instead where its model has the stance's slot and the first value of a sorted list at `0xAD3CE4` is below 550 (#155: what the flag and the list are);
- with that bit set, it takes the stance where its flag `+0x200` has bit `0x8000` or it is creature 2 holding class 2, and creatures 5, 6 and 9 have cases of their own (**hypothesis**: the heroes);
- an attack has cases of its own: a random one of the five where the model fills more than one, and fixed slots for some creatures.

## In `opensacred-game`

`motions` picks a clip as `0x5467A0` does for standing, walking and running, by the table and the fallbacks above, from the slots that the model's file keeps, and the class of what the creature holds as `0x546F80` gives it.
`npcs` gives each start script's NPC the clip it stands in and, for one that a script sends somewhere, the clip it walks with, by the class of the items in its hands (docs/re/equipment.md); one whose walk falls back to a run moves at its creature's run speed.
The other cases are not followed: no NPC rides or is mounted yet, none of the start scripts places a small spider, and the fight stance waits for #155.
The hero keeps its class's clips by name (`main.rs`).
