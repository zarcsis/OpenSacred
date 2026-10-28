# Movement

Walking and the way (docs/re/movement.md).

| Decision | Ours, against the original | Why | Kind | Issues |
|---|---|---|---|---|
| Walking the way by straight lines | The hero heads for the farthest cell of its way that it reaches in a straight line, finds its course once per order, keeps its point while the line to it stays clear, and runs straight toward a target it cannot reach; the goals (`+0x259`) are not followed. | How the client walks between a way's cells, goes round the creatures and treats a target out of reach is not read. | deviation | #212, #216, #981, #989, #996 |
| Way never cuts a corner | The way never steps between two cells that touch at a corner, so that the hero goes round into a house where the original's sometimes stops at its door. | The original's stop leaves the hero outside a house it can walk into, by where it was clicked from; the maintainer's choice (#476). | deviation | #476 |
