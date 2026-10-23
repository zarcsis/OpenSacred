# The creatures of hostile sides fight one another, in melee

AI and fight (`opensacred-game`), 2026-10-23 (#630).

## The original

A guard awake attacks the nearest creature of the 3 × 3 sectors about it to which it is hostile by their sides, within 500, the hero among them, and an idle hostile creature, in one tick of three, takes for its target instead of the hero a creature of its sector that fights, nearer than the hero (docs/re/ai.md, "Foes among the creatures").
They fight as they fight the hero, in melee or at range, and their strikes and arrows resolve as against the hero, without the difficulty's factors between two creatures (docs/re/fight.md).
A boss or champion not hostile to the hero takes its level at its first fight, by its opponent's; a creature killed by another drops nothing, and its killer has its experience in its own stats.
The start's bears by the boss 689, hostile guards of the side 14, run their guards' behaviour and attack the hero or the boss.

## Ours

The guards and the idle hostile creatures take their foes as the original's, and fight them in melee as they fight the hero: steered by the foe alone, without the places of the hero's list of hostiles about it, the doors in their way sought as for the hero; they leave the fight once the foe is dead or gone (**hypothesis**: as giving up).
A creature with a ranged weapon takes no creature for its foe (#670).
The hostile creatures of a behaviour, the start's bears, take their behaviour once woken (#669), and a struck creature's group's alarm wakes its members or sends them into its fight: the start's bears, of one group by `CreateNPC`'s tag `0x11`, wake as the boss strikes one of them and attack the hero or the boss.
A creature killed by another drops nothing, and its killer earns its experience and the levels it reaches, as the original's (#690, docs/re/fight.md); until #690 the killer's experience was not kept, which the maintainer asked to follow.
The bosses' levelling at the first fight is followed for those not hostile to the hero (docs/decisions/2026-10-20-bosses.md).

## Why

The melee, the sides and the levelling are read and the game had them; the archers' arrows hit only the hero (docs/decisions/2026-10-21-archers.md), so the fights at range wait for an issue of their own (#670).
