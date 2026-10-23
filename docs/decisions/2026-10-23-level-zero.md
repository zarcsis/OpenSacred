# A creature in a sector without levels at level 1, not 0

Fight (`opensacred-rules`), 2026-10-23 (#562); the original's creatures seen 2026-10-23 (#541).

## The original

The level's setter (`0x564D60`, docs/re/spawning.md, "The level's setter") holds an NPC's level at most at the difficulty's addition to the highest plus the highest level of its sector's information, and at that addition alone where the sector has no highest; on bronze the addition is 0, so a creature that `CreateNPC` or the creature maker makes in a sector without `SpawnValues` takes the level 0.
In the Seraphim's tree 16 `CreateNPC` records stand in such sectors (the start script's 8 at 4877,2905 in 76,45, 2 of a quest's in 25,95, the 6 of the `Enter` of 83,54); what a creature of level 0 then does beyond its health is not read.
Seen in the original (2026-10-23, a fresh start of the Seraphim on bronze), the `Enter`'s creatures of 83,54 are at level 0 with the most health of 123 (the types 60 and 62), 61 (272), 55 (the two of 307) and 58 (308).

## Ours

Such a creature takes the level 1 (`fight::held_level`).
Its most health is then about a seventh lower than the original's at 0: 106 for the types 60 and 62, 52 for 272, 48 for 307 and 50 for 308.

## Why

The game's formulas of a creature's values, its health, skills and items, take levels from 1, with terms of `L − 1`, and what the original's formulas make of a level of 0 is not read, only its health seen in these few creatures; a level of 0 throughout the formulas waits until they are read.
