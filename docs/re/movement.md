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
For another layer it reads the `Static.PAK` record that the layer names (`0x635E50`, a cache of the records by index), and through the record's sector (`+0x0C`) and its byte at `+0x2D` a grid of cells of its own: an origin and a size in cells, and the cells, 32 bytes each, row by row; a position outside the grid has no cell.
So a building's inside is a grid of cells of its own, which the hero walks on another layer.
The grids are the layers of the sector's third chunk (docs/02 §5.2, **confirmed** on the files: 1,870 of them, the buildings' insides, their walls 1 or 2 in the low nibble and their doors flag bit 2).

Read 2026-10-19 (#104): a creature's layer is the record of a building's floor, and its position carries the floor's number (a byte at `+0x24` of the object).
`0x6354D0` finds the building of a position: the ground cell there, moved by its offsets (`+0x1C`, `+0x1D`) to the building's anchor, and in the anchor cell's chain the first static of flag `0x10`, the building's object.
`0x635440` gives a building's floor `n`: the building itself for 0, else its `n`-th child (its first child, `+0x17`, then the children's chain, `+0x1F`); `0x6353A0` gives a layer's floor, its place among its parent's (`+0x1B`) children from 1.
So floor 1, the ground floor inside, is the building's first child, floor 2 the next, and each child's `+0x2D` is the index from 1 of the inside layer of its sector that holds its grid.
The data agree (**confirmed** on the files, `tools/analysis/world/world_dump.py`): 1,245 buildings, of 1 (660), 2 (549), 3 (34) and 4 (2) floors; every floor lies in its building's sector and names one of its inside layers, and 1,868 of the 1,870 layers are a building's floor; every one of the 973 entrances leads onto a floor 1 whose grid holds the entrance walkable.

The data agree (**confirmed** on the sector files): in the start sector, the monastery's footprint has 1 in the low nibble, so it is a mass to a hero outside it, and the pond, the fences and the walls have 2; its inside, a layer of 34 × 30 cells, has its rooms and corridors free and its walls 1 or 2.
In sector 50,40 each house is a mass of 1 with one cell of flag bit 2 on its side, its door, and its inside layer has a door at the same cell; the cemetery is a railing of 2 with a gate of two such cells and no layer.
The monastery's layer has no door: its corridors open onto cells that are walkable on the ground too, its courtyard and the road round it; it is entered at its entrances (below).

## Changing layers

Read 2026-10-18 (#99).
`0x5FCCB0`, the object manager's move of an object to a new position (called by the creatures' movement and some forty other places), takes the cell at the new position on the object's layer and moves the object between the layers by the low nibble of that cell's ground byte (`+0x1F`):

- 9, an entrance: on a cell of a building (flag bit 0), a creature that is not on the building's inside yet goes onto it, the layer of the building's object's first child (its `+0x17`).
- 10, an exit: a creature on a layer goes onto the ground.
- 5, 6, 7 and 8: a creature on a building's layer goes one floor up, one down, two up or two down (not below the ground): onto the ground at the same cell for floor 0, else onto that floor of the building whose ground cell the position lies on (`0x635440`), and, when it was on a layer, moved by the stair cell's offsets (`+0x1C`, `+0x1D` of the layer's cell), where the stairs arrive on the other floor's grid; nothing where the building has no such floor.
  The move sets the creature's flag `0x100` (`+0x14`), which a cell outside the nibbles 5 to 8 clears and which keeps the stairs from moving it again: a creature changes floor only on a stair it stepped onto from off the stairs.
- 3: a creature is put at a cell and on a layer that an object of the cell names (`0x6356E0`: the first static of flag `0x80` in the chain of the cell on the creature's layer; the object's `+0x23` and `+0x25`, a global cell as two `i16`, and `+0x2D`, the floor of the building there, 0 for the ground), when that object's trigger is open (bit 0) or it has none: a teleport.
  None of the 17 cells of the ground with the nibble 3 has such a static in its chain in `Static.PAK` (**hypothesis**: the teleports are objects that the scripts or the game place at run time).
- A creature that comes onto a building's mass (1 or 2) on the ground, of a type outside `0x300`…`0x39C` (not followed), is put onto the building's layer 1 where the cell is walkable there.

For the local player's hero and the creatures it owns (an object whose id is the local player's, `0x7D84A0`, or a creature whose owner, `+0x1EC`, is), the same steps set the building's state, the word at `+0x0A` of its trigger's record (docs/re/drawing.md, "Buildings: masks and layers"), through `cTrigger::resetState` (`0x416280`, every bit) and `cTrigger::setState` (`0x415F00`):

- an entrance sets 2, unless a closed door lies near: `0x63A160` looks at the 5 × 5 cells around for a door whose trigger's bit 0 is clear;
- an exit sets 1;
- a step between floors sets `1 << n` for the new layer `n`;
- a move onto another building's layer sets the old building's state to 1 and the new one's to `1 << n`.

The data agree (**confirmed** on the sector files): over the world, 973 cells of the ground have the nibble 9, all of them a building's, and 2,545 the nibble 10; the inside layers have 968 and 2,535 at the same places, and 214, 237, 3 and 22 of the nibbles 5 to 8; 827 of the 1,870 layers have an entrance.
Of the stairs (counted 2026-10-19), 20 lead down onto the ground and the rest onto a floor that their building has, 7 of them at the same cell (offsets 0) and the others a few cells off; 438 arrive on a cell of the nibble 0, 9 on a cell the creature may not walk on, 4 on a stair back (5 onto 6, 6 onto 5, 7 onto 8, 8 onto 7), where the flag `0x100` keeps the creature, and 3 on a cell of the nibble 9.
In the start sector the chapel's door is an entrance, (3237, 2519) and (3238, 2519), and the cells just outside it, (3237, 2520) to (3239, 2520), are exits; its stairs and porch in front of them (rows 2521 to 2523) are ordinary cells of the building, bit 0 without bit 1.
The monastery has two more entrances, (3227, 2516) and (3221, 2523), each with its exits.
In sector 50,40 the house's door (3256, 2578), flags 5, is its entrance and the three cells in front of it, (3255, 2579) to (3257, 2579), its exits.

## The way

The client has two path searches: `cPathAstar`, whose search (the second function of its vtable, `0x421AD0`) returns 0 at once, and `cPathFloodfill`, whose search (`0x421B90`) does the work.
It searches from the start's cell for the target's by the fewest steps: a queue of cells and a 64 × 64 byte grid around the start, in which each reached cell keeps the step it was reached by (1–9, by a table of offsets) and an unwalkable one `0xFF`; a neighbour is taken when it lies in the window, is not reached yet and passes `0x636C10`.
Reaching the target's cell and layer, it walks the steps back to the start and returns the cells of the way; with the queue empty, it returns that there is none.
Not read: the table of steps (filled at run time, nine entries, so perhaps the eight neighbours and a change of layer), how the hero then walks the way's cells, and what the caller does when there is no way.

Read 2026-10-19 (#154): `cPathFloodfill`'s search has no caller in the code but through its vtable, and a creature ordered somewhere finds its way otherwise.
`0x4F7710`, the order to go to a point (the AI's states and the player's commands call it), finds the way with `0x4FABE0` and keeps the way's points in the creature (`+0x2ED`).
`0x4FABE0` works in a window of 55 × 55 cells about the creature (`0x1B` cells each side); a target further than 25 cells along an axis it first brings nearer along the line, to the farthest cell of the line within the window that `0x4EBCA0` (the rule of `0x636C10`, with the water's exceptions for some creatures) lets it stand on.
It tries the straight line first, cell by cell (`0x4FAA60`), and else searches the window (`0x4F8AF0`, or `0x4F9410` for some creatures by their type, **hypothesis**: those that fly); every cell either way passes the test of a step, `0x4F7E10` (below).
Not read in detail: the search's order, its costs, and the points it keeps.

## The objects in the way

Read 2026-10-19 (#154).
A cell's `+0x08`, 0 in every file (docs/02 §5.2), is at run time the head of its chain of the world's dynamic objects: the id of the first object, which the object manager resolves (`0x5FE000`), each object's `+0x30` the id of the next.
The object manager links an object into the chain of its cell as it places it (`0x5FDE80`) and out as it removes it (`0x5FC8B0`); the world pass walks the chain to draw a cell's objects (`0x62C306`).

`0x4F7E10` tests whether a creature may step into a cell, on the ground or on a building's floor (by the cells of the floor's grid):

- the cell's own test, `0x4EBCA0`;
- then each object of the cell's chain: an object that is not a creature (the category of its type, `items.pak +0x2E`, other than 2, 3 and `0x20`) stops the step when its category is 4, a `cChest3D` (the chests, the barrels, the vases, the coffins, the cabinets, the book stands and the shrines), and the creature's id (`+0x0C`) is above 16;
- another creature stops it under conditions of the two creatures' flags and states (`+0x1F4`, `+0x245`, `+0x2B7`, a list at `+0x39C`) that are not read yet;
- with bit 2 of the global `0x182EE3C` set, a creature of id 16 or below may step anywhere (**hypothesis**: a debug switch).

`0x5941A0`, the creatures' update, tests the cells of a straight line to where the creature heads with the same test (`0x4F7DD0`, at `0x5961F5`), and the way's search tests every cell with it, so a creature goes round a chest, and stops at one that stands in its way.

The maintainer checked in the game (2026-10-19): the hero does not pass through a chest or a barrel, and sent to a point beyond one, it goes round it.
So the hero's id is above 16 (**hypothesis**: the ids up to 16 are the network game's players', and the single player's hero is not one of them; `0x54ACC0` holds a creature of id 16 or below to the equipment slots of its class, and the update of the hostile creatures about the hero, `0x547070`, treats such a creature apart).

## The height

Read 2026-10-19 (#141).
`0x636520` gives the height that a position on a layer stands at: the layer's cell there (`0x634C50`), or the ground's cell where the layer's grid has none, and of it the second corner heights (`+0x18`…`+0x1B`, left, top, right, bottom), each times 2.5, in units of the models' height; the first corner heights (`+0x10`), the drawn ground's, are not read.
It interpolates them linearly in one of the four triangles that the diamond's centre, at the mean of the four corners, makes with the diamond's sides: the one toward the right or the left corner as the point lies right or left of the centre on the screen, and toward the bottom or the top corner as it lies below or above it.
`0x449950`, for an object on a building's floor (its layer at `+0x2C`), and `0x636310`, for a position that carries its floor's number, add the floor object's `Static.PAK +0x33` times 28 units (0 for floor 1).
`CreateNPC` (`0x48377E`) takes a new creature's height from `0x449950`, and so does the creatures' update (`0x5941A0`, at `0x595290`) for the place it moves a creature to.
So a creature stands on the second heights: on a bridge on its deck, above the water or the ground; on an upper floor at the floor's height above its grid's, which confirms the hypothesis of docs/re/drawing.md, "Depth".
The data agree (**confirmed** on the sector files, docs/02 §5.2): the second heights follow the drawn ground over most of the walkable cells and rise above it on the bridges and walkways.

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
A hero on a building's mass on the ground is put on its building's floor 1 where its grid holds the cell walkable (a start inside the monastery), and a hero that steps onto an entrance or an exit goes in or out as the client's does (`terrain::Heights::transition`), the floors read from the world format (version 6: the buildings and their floors, `opensacred-convert world`).
A hero that steps onto a stair from off the stairs goes to the stair's floor at the cell its offsets give (`terrain::Heights::climb`, `hero::run`), and the way's search knows the stairs as the client's flag `0x100` makes them: a place of the way carries whether the step onto it climbs (`terrain::Footing`).
The buildings' states follow the hero's floor, `1 << n` (`buildings`); the teleports (3) are not followed, there being none in the files.
The hero and the NPCs stand at the height that `0x636520` and `0x449950` give (`terrain::Heights::standing`, #141): on the second heights of their level's cell, a bridge's deck over the water, and on a building's floor at the floor's height above them.
The start scripts' objects of category 4 take their cells at their levels (`terrain::Heights::occupy`, placed by `npcs::place`, #154), as `0x4F7E10` has a chest stop a creature of id above 16: the way of the hero and of a walking NPC goes round them, and a step into one stops, as into a cell that may not be walked on.
The creatures do not stop one another yet: the conditions under which `0x4F7E10` has a creature stop a step are not read (#157).
The deviations are in docs/decisions/2026-10-18-walking.md.

The hero's walk and run speeds are its `Creature.pak` pair times `GameSpeed`, in world units a second (**hypothesis** for the unit), and every clip, the hero's and the NPCs', plays at `GameSpeed`'s rate (`hero::GameSpeed`, read from `tables/balance.ron`); the footsteps, taken at fractions of the clip (docs/re/sound.md), follow it.
