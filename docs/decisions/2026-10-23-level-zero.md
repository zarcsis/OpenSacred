# A creature in a sector without levels at level 1, not 0

Fight (`opensacred-rules`), 2026-10-23 (#562).

## The original

The level's setter (`0x564D60`, docs/re/spawning.md, "The level's setter") holds an NPC's level at most at the difficulty's addition to the highest plus the highest level of its sector's information, and at that addition alone where the sector has no highest; on bronze the addition is 0, so a creature that `CreateNPC` or the creature maker makes in a sector without `SpawnValues` takes the level 0.
In the Seraphim's tree 16 `CreateNPC` records stand in such sectors (the start script's 8 at 4877,2905 in 76,45, 2 of a quest's in 25,95, the 6 of the `Enter` of 83,54); what a creature of level 0 then does is not read.

## Ours

Such a creature takes the level 1 (`fight::held_level`).

## Why

The game's formulas of a creature's values, its health, skills and items, take levels from 1, with terms of `L − 1`, and what the original makes of a level of 0 is not read; at 1 those few creatures are as weak as the original's weakest that is read.
