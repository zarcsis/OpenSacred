# The hero's health and death, before its resurrection

Fight (`opensacred-game`), 2026-10-20 (#292); changed 2026-10-20 (#300).

## The original

The hero's most health comes from its attributes and level by `0x5658F0`, with its items and its skill 10 (docs/re/fight.md, "Health"); the creatures' strikes take it down, weighed by the difficulty's factors and a larger exponent of the damage for a creature above the hero's level, and caps of a hit on a player that are partly read.
It regenerates by its value at the rates of a player fighting, with a hostile near or alone.
At 0 the hero dies; how and where it comes back is not read.

## Ours

The hero's most health, its values and its regeneration are its type's record's at its level (docs/decisions/2026-10-20-levels.md), with its skills (docs/decisions/2026-10-20-skills.md) and without items (#294); the strikes are weighed by bronze's factors and the exponent, without the caps of a hit on a player.
It counts as fighting while it attacks.
At 0 it plays one of its dying clips, the creatures stop coming at it, and five seconds later it rises at the game's start with its health full; the overlay shows its health, and the HUD's health bar does not come back yet.

## Why

The original's resurrection, its places and its costs are not read, and the game keeps no save; a hero that falls and comes back at the start lets the fight go on, and the time and the place are the simplest stand-ins until they are read.
