# Walking: the cells and the way

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra 12.1.4 on 2026-10-18. The cell record is in [docs/02 §5.2](../02-data-formats.md#52-a-sector-in-sectorswldx-the-32-b-floor-cell). Status: **confirmed** on the code unless marked otherwise.

## Which cells may be walked on

`0x636C10` tests a cell, given by its global position and a layer: it fetches the cell (`0x634C50`) and returns whether it may be walked on.

- Not when the low nibble of the cell's ground byte (`+0x1F`) is 1 or 2, or its flags (`+0x1E`) have bit 3.
- A cell with flag bit 2, a door, is looked up by its position and layer (packed as `((x << 14 | y) << 4) | layer`) in a tree of the world's triggers; when it has one, the cell may be walked on when bit 0 of the word at `+0x0A` of the trigger's 16-byte record is set (the table at `+0x268` of the world, whose records also give the buildings' states, docs/re/drawing.md); otherwise the rule above decides.

**The layers.** For layer 0 `0x634C50` returns the sector's own cell. For another layer it finds the object that the layer names (`0x635E50`), and through a byte at its `+0x2D` a grid of cells of its own: an origin and a size in cells, and the cells, 32 bytes each, row by row; a position outside the grid has no cell. So a building's inside is a grid of cells of its own, which the hero walks on another layer. The grids are the layers of the sector's third chunk (docs/02 §5.2, **confirmed** on the files: 1,870 of them, the buildings' insides, their walls 1 or 2 in the low nibble and their doors flag bit 2).

The data agree (**confirmed** on the sector files): in the start sector, the monastery's footprint has 1 in the low nibble, so it is a mass to a hero outside it, and the pond, the fences and the walls have 2; its inside, a layer of 34 × 30 cells, has its rooms and corridors free and its walls 1 or 2. In sector 50,40 each house is a mass of 1 with one cell of flag bit 2 on its side, its door, and its inside layer has a door at the same cell; the cemetery is a railing of 2 with a gate of two such cells and no layer. The monastery's layer has no door: its corridors open onto cells that are walkable on the ground too, its courtyard and the road round it.

## The way

The client has two path searches: `cPathAstar`, whose search (the second function of its vtable, `0x421AD0`) returns 0 at once, and `cPathFloodfill`, whose search (`0x421B90`) does the work. It searches from the start's cell for the target's by the fewest steps: a queue of cells and a 64 × 64 byte grid around the start, in which each reached cell keeps the step it was reached by (1–9, by a table of offsets) and an unwalkable one `0xFF`; a neighbour is taken when it lies in the window, is not reached yet and passes `0x636C10`. Reaching the target's cell and layer, it walks the steps back to the start and returns the cells of the way; with the queue empty, it returns that there is none. Not read: the table of steps (filled at run time, nine entries, so perhaps the eight neighbours and a change of layer), how the hero then walks the way's cells, and what the caller does when there is no way.

## In `opensacred-game`

`terrain::Heights` keeps each loaded cell's walkability by the rule above, on the ground and on the sectors' inside layers; the doors' triggers have no states in the game, so the doors are open (**hypothesis**). `paths` searches the way as the client does, over the eight neighbours in a 64 × 64 window but not between two cells that touch at a corner (the client's search is not read to do so), at the hero's level and, at a cell walkable at another level too, the same cell at that level (**hypothesis**: how the client changes the hero's layer is not read; a house's door is such a cell, and so are the openings of the monastery's inside, whose layer has no door, onto its courtyard and the road), and the hero heads for the farthest cell of the way on its level that it reaches in a straight line, or for the target itself when nothing stands between; at a change of level on its way it goes onto the way's next level. A target it cannot reach it runs straight toward, until a step would take it into a cell it may not walk on. A hero on a building's mass on the ground is put on the inside layer whose grid holds its cell walkable (a start inside the monastery). The deviations are in docs/decisions.md.
