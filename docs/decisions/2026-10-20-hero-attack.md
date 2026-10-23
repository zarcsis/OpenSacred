# The hero's attack, before its levels, skills and weapons

Fight (`opensacred-game`), 2026-10-20 (#283); changed 2026-10-20 (#300) and 2026-10-22 (#486, #488, #490); brought up to date 2026-10-23 (#692); changed 2026-10-23 (#704, #705).

## The original

A left click on a hostile creature sends the hero at it (docs/re/fight.md, "The hero's attack"): it runs the creatures' machine of the AI's state 4, its band `R + 45`, pushing a target nearer than the band, its strikes at its attack speed and at an even pace, one a click and more while the button is held (#704, #705); with Ctrl held it attacks at once, and a click on the ground strikes the air.
The click picks the creature by its model on the screen.
The hero's attack, defence and damage are its stats' recomputation's, with its attribute points, its skills, its combat arts and the weapon in its hand; its kills earn it experience, by which it gains levels, the cap `LevelKap` and its bonuses applying.

## Ours

The hero strikes with its bare hands' clips at its attack speed (#306), at an even pace, once a click and on while the button is held, as the original's (#704, #705), without pushing its target.
A creature is picked within 45 units of the cursor's line of sight, up to 110 over its feet, rather than by its model's shape.
Its values are its type's record's at its level, with its two skills at level 1 (docs/decisions/2026-10-20-skills.md), no attribute points, and the items of its class's template that it wears and holds, each record's damage, resistances, `A` and `D` (#486; docs/02 §9) and the bonus slots that the client's recomputation counts (#490), the effects of the skills and the arts (#651) and the other multipliers left out; its experience grows with its kills and gives it levels (#300, docs/decisions/2026-10-20-levels.md).
It holds its template's weapon in its hand and moves, stands and strikes with the clips of its class (#488).
Since #509 it makes its combat arts, the selected one by the right button, with their clips, strikes and regeneration (#511, docs/re/arts.md); the spells' objects (#672) and the special weapon arts (#653) are not made yet.

## Why

The arts' effects, the attribute points and the items it finds are features of their own (#651, and the character sheet and the inventory of M5); the picking by a model's shape needs the models' bounds, which the game does not keep yet.
