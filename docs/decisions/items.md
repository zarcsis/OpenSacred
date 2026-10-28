# Items

The drops and the items on the ground (docs/re/items.md).

| Decision | Ours, against the original | Why | Kind | Issues |
|---|---|---|---|---|
| Drops lie until the game ends | A drop lies until the game ends wherever the hero goes, at height 0 if it flew out of the loaded sectors. | How long the original's items lie, and whether they stay when their sector is left, is not read. | deviation | #415 |
| Drops rolled by the game's generator | The drops' rolls come from the game's own generator rather than the C library's `rand()`, so that a death drops what the original's might, not what it would. | The C library's `rand()` is one sequence that every draw of the client shares, an order that the game does not reproduce. | deviation | #414 |
