# The hero's attack, before its levels, skills and weapons

Fight (`opensacred-game`), 2026-10-20 (#283).

## The original

A left click on a hostile creature sends the hero at it (docs/re/fight.md, "The hero's attack"): it runs the creatures' machine of the AI's state 4, its band `R + 45`, pushing a target nearer than the band, its strikes chained faster up to 1.7 times while the player keeps attacking, at its attack speed; with Ctrl held it attacks at once, and a click on the ground strikes the air.
The click picks the creature by its model on the screen.
The hero's attack, defence and damage are its stats' recomputation's, with its attribute points, its skills, its combat arts and the weapon in its hand; its kills earn it experience, by which it gains levels, the cap `LevelKap` and its bonuses applying.

## Ours

The hero strikes with its bare hands' clips at its attack speed (#306), without chained strikes growing faster and without pushing its target; it strikes on until the target dies, which the original's player does by keeping the button held or clicking again (**hypothesis**: the hero's idle handler does not take it back into state 4 by itself).
A creature is picked within 45 units of the cursor's line of sight, up to 110 over its feet, rather than by its model's shape.
Its values are its type's record's at level 1, unarmed, with its two skills at level 1 (docs/decisions/2026-10-20-skills.md) and no attribute points or items (#294); its experience grows with its kills, without the cap of the level or the bonuses, and it gains no level (#300).

## Why

The hero's levels, skills, combat arts and items are features of their own (#293, #294, #300); a hero that fights the creatures that come at it, with the original's reach and hits, is what the fight needs next, and the picking by a model's shape needs the models' bounds, which the game does not keep yet.
