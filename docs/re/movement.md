# Walking: the cells and the way

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra 12.1.4 on 2026-10-18.
The cell record is in [docs/02 §5.2](../02-data-formats.md#52-a-sector-in-sectorswldx-the-32-b-floor-cell).
Status: **confirmed** on the code unless marked otherwise.

## Which cells may be walked on

`0x636C10` tests a cell, given by its global position and a layer: it fetches the cell (`0x634C50`) and returns whether it may be walked on.

- Not when the low nibble of the cell's ground byte (`+0x1F`) is 1 or 2, or its flags (`+0x1E`) have bit 3.
- A cell with flag bit 2, a door, is looked up by its position and layer (packed as `((x << 14 | y) << 4) | layer`) in a tree of the world's triggers; when it has one, the cell may be walked on when bit 0 of the word at `+0x0A` of the trigger's 16-byte record is set (the table at `+0x268` of the world, whose records also give the buildings' states, docs/re/drawing.md); otherwise the rule above decides.

**The layers.**
For layer 0 `0x634C50` returns the sector's own cell.
For another layer it finds the object that the layer names (`0x635E50`), and through a byte at its `+0x2D` a grid of cells of its own: an origin and a size in cells, and the cells, 32 bytes each, row by row; a position outside the grid has no cell.
So a building's inside is a grid of cells of its own, which the hero walks on another layer.
The grids are the layers of the sector's third chunk (docs/02 §5.2, **confirmed** on the files: 1,870 of them, the buildings' insides, their walls 1 or 2 in the low nibble and their doors flag bit 2).

The data agree (**confirmed** on the sector files): in the start sector, the monastery's footprint has 1 in the low nibble, so it is a mass to a hero outside it, and the pond, the fences and the walls have 2; its inside, a layer of 34 × 30 cells, has its rooms and corridors free and its walls 1 or 2.
In sector 50,40 each house is a mass of 1 with one cell of flag bit 2 on its side, its door, and its inside layer has a door at the same cell; the cemetery is a railing of 2 with a gate of two such cells and no layer.
The monastery's layer has no door: its corridors open onto cells that are walkable on the ground too, its courtyard and the road round it; it is entered at its entrances (below).

## Changing layers

Read 2026-10-18 (#99).
`0x5FCCB0`, the object manager's move of an object to a new position (called by the creatures' movement and some forty other places), takes the cell at the new position on the object's layer and moves the object between the layers by the low nibble of that cell's ground byte (`+0x1F`):

- 9, an entrance: on a cell of a building (flag bit 0), a creature that is not on the building's inside yet goes onto it, the layer of the building's object's first child (its `+0x17`).
- 10, an exit: a creature on a layer goes onto the ground.
- 5, 6, 7 and 8: a creature on a building's layer goes one layer up, one down, two up or two down (not below the ground).
- 3: a creature is put at a cell and on a layer that an object of the cell names (`0x6356E0`; the object's `+0x23`, `+0x25` and `+0x2D`), when that object's trigger is open (bit 0) or it has none: a teleport, not followed further.
- A creature that comes onto a building's mass (1 or 2) on the ground, of a type outside `0x300`…`0x39C` (not followed), is put onto the building's layer 1 where the cell is walkable there.

For the local player's hero and the creatures it owns (an object whose id is the local player's, `0x7D84A0`, or a creature whose owner, `+0x1EC`, is), the same steps set the building's state, the word at `+0x0A` of its trigger's record (docs/re/drawing.md, "Buildings: masks and layers"), through `cTrigger::resetState` (`0x416280`, every bit) and `cTrigger::setState` (`0x415F00`):

- an entrance sets 2, unless a closed door lies near: `0x63A160` looks at the 5 × 5 cells around for a door whose trigger's bit 0 is clear;
- an exit sets 1;
- a step between floors sets `1 << n` for the new layer `n`;
- a move onto another building's layer sets the old building's state to 1 and the new one's to `1 << n`.

The data agree (**confirmed** on the sector files): over the world, 973 cells of the ground have the nibble 9, all of them a building's, and 2,545 the nibble 10; the inside layers have 968 and 2,535 at the same places, and 214, 237, 3 and 22 of the nibbles 5 to 8; 827 of the 1,870 layers have an entrance.
In the start sector the chapel's door is an entrance, (3237, 2519) and (3238, 2519), and the cells just outside it, (3237, 2520) to (3239, 2520), are exits; its stairs and porch in front of them (rows 2521 to 2523) are ordinary cells of the building, bit 0 without bit 1.
The monastery has two more entrances, (3227, 2516) and (3221, 2523), each with its exits.
In sector 50,40 the house's door (3256, 2578), flags 5, is its entrance and the three cells in front of it, (3255, 2579) to (3257, 2579), its exits.

## The way

The client has two path searches: `cPathAstar`, whose search (the second function of its vtable, `0x421AD0`) returns 0 at once, and `cPathFloodfill`, whose search (`0x421B90`) does the work.
It searches from the start's cell for the target's by the fewest steps: a queue of cells and a 64 × 64 byte grid around the start, in which each reached cell keeps the step it was reached by (1–9, by a table of offsets) and an unwalkable one `0xFF`; a neighbour is taken when it lies in the window, is not reached yet and passes `0x636C10`.
Reaching the target's cell and layer, it walks the steps back to the start and returns the cells of the way; with the queue empty, it returns that there is none.
Not read: the table of steps (filled at run time, nine entries, so perhaps the eight neighbours and a change of layer), how the hero then walks the way's cells, and what the caller does when there is no way.

## The speed

Read 2026-10-18 (#72).
`0x4F75F0`, as a creature takes a gait, sets its speed (`+0x375`, an integer) to its `Creature.pak` record's walk speed (`+0x26`, for the gait 1 at `+0x1F8`) or run speed (`+0x28`), to a default of `0x4F73C0` where that is 0, or to 200 for a mounted creature (`+0x1F4` bit `0x200000`), times the balance variable `GameSpeed` (1.25 in `Balance.bin`, docs/02 §6.3) times `f`, truncated; `f` is a short of the creature's attributes (`0x585070`, `+0x40` of its block at `+0x3A8`, a mount's or an owner's for some) times the creature's float at `+0x2C9` times 0.01 (**hypothesis**: a percentage, 100 for a hero without bonuses, and 1.0).
The same call sets the rate of the creature's clip (`0x413610`, Granny's animation parameter 2) to `GameSpeed · f`, times the motion's own factor where that lies in [0.8, 5) and is not 1: `MotionDesc +0x9C`, which is `MOTIONS.PAK +0x5A`, zero in every record (docs/02 §4.1), so it never applies.
So the creatures move and their clips play a quarter faster than the tables and the clips have it, together, and the steps match the ground as they were made to.
In what unit of time the speed is, is not read.

## In `opensacred-game`

`terrain::Heights` keeps each loaded cell's walkability by the rule above, on the ground and on the sectors' inside layers; the doors' triggers have no states in the game, so the doors are open (**hypothesis**).
`paths` searches the way as the client does, over the eight neighbours in a 64 × 64 window but not between two cells that touch at a corner (the client's search is not read to do so), at the hero's level and, at an entrance on the ground or an exit on a layer, only onto the level it leads to (`terrain::Heights::neighbours`), and the hero heads for the farthest cell of the way on its level that it reaches in a straight line, or for the target itself when nothing stands between; at a change of level on its way it goes onto the way's next level.
A target it cannot reach it runs straight toward, until a step would take it into a cell it may not walk on.
A hero on a building's mass on the ground is put on the inside layer whose grid holds its cell walkable (a start inside the monastery), and a hero that steps onto an entrance or an exit goes in or out as the client's does (`terrain::Heights::transition`), onto the first of the inside layers whose grids hold the cell walkable (**hypothesis**: the building's first child); the floors (5 to 8) and the teleports (3) are not followed (#104).
The deviations are in docs/decisions/2026-10-18-walking.md.

The hero's walk and run speeds are its `Creature.pak` pair times `GameSpeed`, in world units a second (**hypothesis** for the unit), and every clip, the hero's and the NPCs', plays at `GameSpeed`'s rate (`hero::GameSpeed`, read from `tables/balance.ron`); the footsteps, taken at fractions of the clip (docs/re/sound.md), follow it.
