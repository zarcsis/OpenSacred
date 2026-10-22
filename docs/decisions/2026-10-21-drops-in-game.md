# The drops lie where they fall, and no one picks them up yet

Drops (`opensacred-game`), 2026-10-21 (#415).
Follows [2026-10-21-drops](2026-10-21-drops.md), the drops' rules.

## The original

A creature drops as it dies, when the local hero lives and is hostile to it, it is no player's companion, not of the types 71 or 75 nor of the state `+0x1F0` 7, not of the class 5 with a most health other than 0 (**hypothesis**: an undead that rises again), and its killer is a player or in the hero's list at `+0x39C` (docs/re/items.md, "Who drops").
Each drop is an object of its type, thrown from the creature's place, which flies and comes to rest on the ground ("On the ground"), within a unit of where it was thrown, its velocity along the ground turning back at every frame (seen, #478); holding Alt shows a label over each item in view; the hero picks an item up as it arrives at it (`cCreature::pickupItem`), gold into its purse, an item into its hands, a slot or its inventory; a network game sends the drops to the other players.
How long an item lies, and whether it stays when its sector is left, is not read.

## Ours

A creature drops as its `npcs::Dies` is added, when it is hostile to the hero, the hero lives, and its type is not 71 or 75; the drops fly as the original's and lie where they come to rest, drawn as their models with their items' textures, until the game ends.
But:

- The killer is not tested: the hero is the only creature that kills in the game, and it has no companions or summoned creatures.
- The class 5's test of the most health is not followed: every creature of the game has a most health, so it would keep every skeleton from dropping, and the game's undead do not rise again.
- The state `+0x1F0` 7 is not followed, the game giving its creatures no such state.
- No pick-up, which comes with the inventory (M5), and no labels on Alt; the drops make no sound and cast no shadow.
- A drop stays until the game ends, wherever the hero goes; one that flies out of the loaded sectors lies at the height 0.
- The time played that raises a kill's bonus is the game's since it started, there being no saves; the luck is 100 and the treasure of the effect 42 is 0, the game having no effects.

## Why

The rules of the drops and of their flight are read and followed; what the drops are for, the inventory and the purse, is a milestone of its own, and the labels, the sounds and the shadows of the items wait for it.
The class 5's test reads like the guard of a creature that does not stay dead, which the game does not have; following it to the letter would make the undead drop nothing, a departure greater than leaving it out.
