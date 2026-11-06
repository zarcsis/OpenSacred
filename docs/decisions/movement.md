# Movement

Walking and the way (docs/re/movement.md).

| Decision | Ours, against the original | Why | Kind | Issues |
|---|---|---|---|---|
| Walking the way by straight lines | The hero heads for the farthest cell of its way that it reaches in a straight line, finds its course once per order, keeps its point while the line to it stays clear, and runs straight toward a target it cannot reach; the goals (`+0x259`) are not followed. | How the client walks between a way's cells, goes round the creatures and treats a target out of reach is not read. | deviation | #212, #216, #981, #989, #996 |
| Way never cuts a corner | The way never steps between two cells that touch at a corner, so that the hero goes round into a house where the original's sometimes stops at its door. | The original's stop leaves the hero outside a house it can walk into, by where it was clicked from; the maintainer's choice (#476). | deviation | #476 |
| The search's goal on the ground | The search over floor numbers ends at the floor of the level that the target's cell is at on the target's floor number, the ground's where no building there has that floor, where the original's ends at the target's floor byte (`0x4F8AF0`): a walk from a building's floor toward a target on the ground takes the building's exits. | With the floor byte, the hero of `annehmen2003` stepping onto the chapel's west exit on its way at the floor number 1 stands there, every way back to the floor number 1 closed by the exits, until the queue drops its walk (docs/re/movement.md, "The chapel's west exit"); the maintainer kept the floors' rules but not the stranding (#2013). | deviation | #1344, #2013 |
