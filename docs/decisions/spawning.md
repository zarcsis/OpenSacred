# Spawning

The sectors' creatures as they come and go (docs/re/spawning.md).

| Decision | Ours, against the original | Why | Kind | Issues |
|---|---|---|---|---|
| Crabs and fish by the loaded sectors | A sector's crabs and fish test only the cells of the sectors loaded as it comes in, so that the edge rows next to neighbours not loaded get none, up to one in 16 of its cells. | Reading a sector's cells for the rings would need a second reader of the sector files beside the streamer's, for a few fish made by a roll of one in 11 anyway. | limitation | #575 |
| Sectors' creatures kept and seeded | Sectors go two from the hero (5 × 5) rather than as they leave the 3 × 3, and the fills roll from a generator seeded by the sector and the record, so that a sector comes back with the same creatures on the same cells. A `CreateNPC`'s extra level is rolled by a generator seeded by its cell, and the start script's creatures are made as their sector first comes in. | The wider ring serves the streaming, and the seeded rolls keep the fills' creatures in place and a village's at the same levels, where a new roll at each coming would move them about. | deviation | #221, #329, #530 |
