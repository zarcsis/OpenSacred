# Items

The drops and the items on the ground (docs/re/items.md).

| Decision | Ours, against the original | Why | Kind | Issues |
|---|---|---|---|---|
| Class runes given counted for the session | The count of the runes of its class that the quests gave the hero, which weighs against a side quest's rune, lasts for the session and starts again with it. | The original keeps it with the player (`+0x5710`), which the game has no save of yet. | deviation | #1386 |
| Containers' items at the hero's level | A container's items are made at the hero's level, one or two above in a case of 101, of a quality key that a death's pass of no bonus rolls, held at 5 or above; its gold takes the hero's level unheld; a grave's or a shrine's creature stands on the ground beside the hero. | The original's item maker `0x5D57C0` makes them at a level held within the container's sector's levels and rolls their key by the hero's luck and the time played, at most 11, and `0x45FF80` places the creature; none of them is read. | deviation | #1381 |
| Drops lie until the game ends | A drop lies until the game ends wherever the hero goes, at height 0 if it flew out of the loaded sectors. | How long the original's items lie, and whether they stay when their sector is left, is not read. | deviation | #415 |
| Drops rolled by the game's generator | The drops' rolls come from the game's own generator rather than the C library's `rand()`, so that a death drops what the original's might, not what it would. | The C library's `rand()` is one sequence that every draw of the client shares, an order that the game does not reproduce. | deviation | #414 |
| Rewards' items laid at the hero's level | A quest's reward's item and a side quest's rune are made at the hero's level and rise from 60 over the ground 40 units before it, as an item dropped off the cursor. | The original makes them at a level of the sector's levels raised by the difficulty and of the hero's, not read in full, and puts them at that point without a flight; seen at the hero's level 1. | deviation | #1386 |
