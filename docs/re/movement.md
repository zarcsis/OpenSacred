# Walking: the cells and the way

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra 12.1.4 on 2026-10-18.
The cell record is in [docs/02 §5.2](../02-data-formats.md#52-a-sector-in-sectorswldx-the-32-b-floor-cell).
Status: **confirmed** on the code unless marked otherwise.

## Which cells may be walked on

`0x636C10` tests a cell, given by its global position and a layer: it fetches the cell (`0x634C50`) and returns whether it may be walked on.

- Not when the low nibble of the cell's ground byte (`+0x1F`) is 1 or 2, or its flags (`+0x1E`) have bit 3.
- A cell with flag bit 2, a door, is looked up by its position and layer (packed as `((x << 14 | y) << 4) | layer`) in a tree of the world's triggers; when it has one, the cell may be walked on when bit 0 of the word at `+0x0A` of the trigger's 16-byte record is set (the table at `+0x268` of the world, whose records also give the buildings' states, docs/re/drawing.md); otherwise the rule above decides.
  The tree is the scripts' (`0xAB44C8`, `+0x7548` of the script manager at `0xAACF80`), and the scripts' `TriggerPatch` records fill it at run time, setting the flag on the cells they name (#211, "The objects in the way"): the doors of the sector files are not its only cells.

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

**Positions and layers** (read 2026-10-23, #568).
A position is 16 bytes: the sector's number (`u16` at `+0`, 0 for none or not looked up yet), x and y (`i32` at `+4` and `+8`) and the floor (`u8` at `+0x0C`, 0 the ground, 1 a building's ground floor).
A cell position gives the global cell; a world position, as an object's at `+0x18` (docs/re/objects.md), the world units.
`sdk_BUILD_POS` (`0x6224B0`) fills one from its four values, and `0x6224E0` makes the cell position of a world position, its x and y divided by a cell's side (the float 53.66563 at `0x892044`) and truncated.
The world (`+0` of the object manager, `sdk_OM`; kept at `0xAD3560`) numbers its sectors in a table of 128 × 128 `u16` at `+0x284`, by the sector's coordinates (the cell's divided by 64), row by row; `0x6342C0` looks a position's number up there, and `0x635D10` and `sdk_SECTOR_RESOLVE` (`0x635C40`) keep a number whose sector holds the position and look it up otherwise, returning whether there is one.
`0x634C50` and the functions below do the same, so a position built with the number 0, as the teleport builds one, serves them all.
A sector (`0x634840`: the world's vector at `+0x240`, by the number) has its first cell at `+0x44` and `+0x48`, its cells at `+0x6C`, and bit `0x10000` of its flags at `+0x30` while its cells are in memory; `0x634340`, which `0x634C50` takes a sector's cells from, reads them from the sector file where they are not, into a cache of up to 96 sectors.
A layer is 0 for the ground, or the index of a building's floor's `Static.PAK` record; `0x635E50` reads a record of 64 bytes by its index (through the file's table of 12-byte entries at `0x100`, into a cache of up to 8,192 records), its sector's number at `+0x0C`, the index of its inside layer at `+0x2D` and the floor's height at `+0x33`.
`0x6354D0` (the world, a cell position, `n`) gives the record of a position's building for `n` 0 and of its floor `n` above, or 0; `0x635400` (the world, a building's record) counts its floors.
The scripts' teleport (`sdk_ENGINE_TP`, `0x54D9D0`) builds the position of its cell and floor with the number 0, takes the floor's record as the layer (0 for the ground), and places the creature only where the creature's own test of a cell, `0x4EBCA0` (the creature, the position, the layer and 1, as the test of a step `0x4F7E10` calls it), lets it stand.
`0x636310` (the world, a world position) gives the height a position stands at on its floor: `0x636520`'s on the ground, else `0x636520`'s on the floor's layer plus the floor's `+0x33` times 28; `0x636520` (the world, a world position, a layer) returns it on the x87 stack.
The scripts' tree of trigger cells (`0xAB44C8`, below) is a binary search tree from that root: a node's key at `+4`, its trigger's index at `+8`, and the nodes of the lower and higher keys at `+0x0C` and `+0x10` (`0x4C48E0` searches it); the index names a 16-byte record of the world's table from `+0x268` to `+0x26C` (`0x634150`).

The original's driver asks these functions of a cell with `cell-info` (#568, docs/03 §3.8): for the ground, each floor of the building there and the hero's own layer, the cell's bytes, `0x636C10`, `0x4EBCA0` for the hero, the height at the cell's middle and the objects of the cell's chain.
Seen with it 2026-10-23 in the Seraphim's start: the chain of a cell is its level's own, the hero on a floor in the chain of the floor's cell and not of the ground's, and a chest on floor 3 of the building at (3342, 2507) in its floor 3's.
The height at a cell's middle is the mean of its four second heights times 2.5 (a cell of 1, 2, 1, 0 stands at 2.5), plus the floor's `+0x33` times 28 (224 and 448 on that building's floors 2 and 3, of `+0x33` 8 and 16).
The house's door of 50,40 (3256, 2578) is trigger 4220's cell on the ground and on floor 1, closed at the start, so neither walkable nor a cell the hero may stand on.
At the cells checked (the monastery's floor 1, the chapel's entrance and exits, that house's door and exits, the buildings of 52,39 at (3333, 2531) and (3342, 2507) with their stairs), the cells' bytes, the floors, the stairs' offsets, the buildings' anchors and the walkability agree with opensacred-game's `cell-info`.

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
The 9 that land on a cell the creature may not walk on (found again 2026-10-25 by a scan of the converted world): in the three alike buildings of 17,11, 17,14 and 17,15, floor 2's stair up lands on a wall cell of floor 3 and its stair down on a wall cell of floor 1, at (1136, 748), (1134, 750), (1116, 913), (1114, 915), (1108, 989) and (1106, 991); and the stairs down at (2579, 1863) of 40,29, (2530, 2042) of 39,31 and (2505, 2076) of 39,32.
**Seen** 2026-10-25 (#227, check 24) on the two of 17,11: the hero put on the landing's wall cell stood on the walkable cell next to it 0.15 s later, (1135, 747) to (1136, 747) and (1135, 749) to (1136, 750), so nobody is stuck there; what moves it is not read (#978).
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
It tries the straight line first, cell by cell, and else searches the window (`0x4F8AF0`, or `0x4F9410` for some creatures by their type, **hypothesis**: those that fly); every cell either way passes the test of a step, `0x4F7E10` (below).
Read again 2026-10-25 (#981): the straight line runs from the creature's cell to the target's cell by whole cells, a step along the longer axis each time and one along the shorter where the line's error carries over, so that it does not depend on where in its cell the creature stands; at a diagonal step it tests the two cells beside the corner too.
Each cell it tests with `0x4FAA60`, which takes the cells within a square of a radius about it, `+0x399` of the creature: 0 for most, 1 or 2 for some types by their id, set at the search's start (**hypothesis**: the big ones).
The line counts as clear at the target's cell, or, longer than 5 cells, once two thirds of it are clear.
Seen 2026-10-25 with #448's driver: on open grass about (3183, 2503) the hero sent 7 cells east and 2 south ran along the line, through (3181, 2500), (3182, 2501) to (3185, 2501), (3186, 2502), not along a way's diagonals; by the barrel of (5938, 404) it walked its way's cells.
Not read in detail: the search's order, its costs, and the points it keeps.

Seen 2026-10-22 in the original with its driver (#476): at the merchant's house of 50,40, its door open, the hero clicked onto the floor 1 beside the door from (3258, 2584), below the wall, walks to the exit (3257, 2579) and stops there, while from (3255, 2584) or (3254, 2582), left of the door, it comes onto the entrance (3256, 2578) straight up the grid and goes in.
From that exit the shortest way in is the diagonal onto the entrance past the corner of the wall's cell (3257, 2578) (**hypothesis**: `0x4FABE0` takes such a diagonal, which the walk does not step).

## The objects in the way

Read 2026-10-19 (#154).
A cell's `+0x08`, 0 in every file (docs/02 §5.2), is at run time the head of its chain of the world's dynamic objects: the id of the first object, which the object manager resolves (`0x5FE000`), each object's `+0x30` the id of the next.
The object manager links an object into the chain of its cell as it places it (`0x5FDE80`) and out as it removes it (`0x5FC8B0`); the world pass walks the chain to draw a cell's objects (`0x62C306`).

`0x4F7E10` tests whether a creature may step into a cell, on the ground or on a building's floor (by the cells of the floor's grid):

- the cell's own test, `0x4EBCA0`;
- then each object of the cell's chain: an object that is not a creature (the category of its type, `items.pak +0x2E`, other than 2, 3 and `0x20`) stops the step when its category is 4, a `cChest3D` (the chests, the barrels, the vases, the coffins, the cabinets, the book stands and the shrines), and the creature's id (`+0x0C`) is above 16;
- another creature (a `cCreature`) stops it under the conditions below;
- with bit 2 of the global `0x182EE3C` set, a creature of id 16 or below may step anywhere (**hypothesis**: a debug switch; nothing found sets it).

`0x5941A0`, the creatures' update, tests the cells of a straight line to where the creature heads with the same test (`0x4F7DD0`, at `0x5961F5`), and the way's search tests every cell with it, so a creature goes round a chest, and stops at one that stands in its way.

**The hero's id is 1** (read 2026-10-20, #157): a new single-player game (`0x70D6E0`) and a loaded one (`0x71CC90`) store 1 as the local player's id (`+0x14` of the context that `0x7D84A0` gives), and the object manager hands out no id below 17 to other objects (`0x600AA0` keeps the slots 0 to 16 apart); the ids 1 to 16 are the players'.
So `0x4F7E10` does not stop the hero at a chest; yet the maintainer saw in the game (2026-10-19) that the hero does not pass through a chest or a barrel, and goes round one to a point beyond it.

**The chests' cells** (read 2026-10-20, #211): the cell keeps the hero off, not the object.
Every start script creates one trigger closed, `CreateTrigger "chestTrigger" 28:0 27` (type 48, `0x496520`: tag `0x27` clears bit 0 of the state, `0x26` would set it), and names the cells of each chest before its `CreateObj` in a `TriggerPatch 01:"chestTrigger" 2A:(x, y, floor)` (type 50, `0x496F20`), several for a wide object.
`0x463F50`, for each position of the patch (up to 100), puts the key `((x << 14 | y) << 4) | (floor ≥ 2 ? floor : 0)` in the scripts' tree of trigger cells (`0xAB44C8`, `0x4BF460`) with the trigger's index, keeps the position in the script manager's list (`+0x37C`), and sets bit 2 of the cell's flags (`+0x1E`): of the ground's cell for the floors 0 and 1, of the floor's cell above; `0x493440` sets the flags again as a sector comes in (`0x637AF0`).
The cell's own test `0x4EBCA0`, which `0x4F7E10` makes first for every creature, refuses a cell of that flag whose trigger lacks bit 0 of its state (but to a creature of `+0x1F4` bit `0x200000` on the ground, outside a dungeon).
No script opens `chestTrigger` (it is named only by 310 `CreateTrigger`, 49,241 `TriggerPatch` and 279 `CreateObj` records over the trees; a chest opens by its own trigger), so the chests' cells stay closed to every creature, the hero included.
In the Seraphim's tree, 1,843 of the 1,930 objects of category 4 stand on a cell of `chestTrigger`, 67 book stands on cells of `PanelTrigger` and 4 on their own trigger's; 16 have no patch (5 barrels, 11 chests, coffins or cabinets), which only the rule of `0x4F7E10` keeps the NPCs off, not the hero, as read; yet the hero's way kept off the one such barrel through whose cell a way passes, (5938, 404), both ways along its diagonal (**seen** 2026-10-25, #227, check 2; #981), though the step's test lets it onto the cell (traced 2026-10-25: `0x4F7E10` answered 1 for the hero at (5938, 404) each of the five times the way's search asked); it is its course that keeps it off: the straight line from cell to cell to its target passes the corners of the closed cells beside the barrel, and the way that the search finds instead goes round it, which the hero then walks ("The way" above, and "In `opensacred-game`" below); 413 cells of `chestTrigger` hold no object of their own, the side cells of wide objects; and the signposts (category 15) stand on cells of `WWTrigger` and `WWTriggerMP`, closed as well.
The same patches close the scripts' other blocking cells (triggers of names such as `nogo…`, `…stopper…`, `Blocker…`), which open as the scripts set their states.
A click on a closed cell sends the hero to the walkable neighbour of the eight nearest to it (`0x4F8560`), so it stops beside a chest.
(**confirmed** by code and on the trees' listings)

**The creatures in the way** (read 2026-10-20, #157).
A creature in the cell stops the step when all of these hold:

- it is not the stepping creature, nor the one the stepper heads for (the stepper's `+0x259`, **hypothesis**: its goal, which the movement and the AI set), and the stepper's `+0x259` is not the stepper itself;
- the cell is not the stepper's own;
- the stepper does not pass through creatures (its `+0x1F4` bit `0x200000`, which the creature's constructor `0x52DE40` sets for the hornets and the movement sets and clears on the way) and is not mounted (**hypothesis** for its `+0x2B7` bit `0x10`, which the two dismounts, `0x551E40` and `0x552050`, clear on the horse and the rider);
- the creature is alive (`0x549080`: its action `+0xFC` is not 9, its `+0x150` not 6, its health `+0x4D8` not 0; `+0x4D4` the maximum) and is not big (its `Creature.pak` record's flags byte, `+6`, lacks bit `0x02`, `0x426CB0`: the ogres, the minotaurs, the trolls, the dragons, the big demons, the giant spiders, about 60 types);
- the creature is not one of the stepper's followers (the list at `+0x39C`, records of 9 bytes from the creature's id, which `sdk_HIRE`, `0x54CF70`, fills through `0x54B200` for the hired, the summoned and the saved party, and `sdk_PARTY_REMOVE`, `0x551300`, empties), unless that follower is hostile to it (`0x548F60`), of class 7 (the soldiers, `0x43ADC0`: the record's `+4`) or of the guards' mode (`+0x1F4` bit `0x4000`);
- the creature never stops others unless its `+0x1F4` lacks bit `0x1000000`, which the constructor sets for the critters (small spiders, crows, bats, rabbits, chickens: some types by id, and every type whose record's flags have `0x20`);
  `CreateNPC` writes the whole word from its tags (below), so that only a fill's critters keep the bit (read in the original 2026-10-22, #468: a fill's rabbit has `0x1000040`, its variants `0x1000000`, `CreateNPC`'s chickens and its rabbit of 50,40 `0x20` alone), and `CreateNPC`'s critters stop the others' steps;
- and the stepper's id is above 16, or the creature's `+0x1F4` has bit 0 or 1, or the stepper's `+0x245` is above 0 (the index of the dialog it is in, which `sdk_STAMP_IDX`, `0x5498F0`, sets as a dialog starts and `CreateNPC` clears).

So for the hero, of id 1, a creature in the way stops it only when its `+0x1F4` has bit 0 or 1.
`CreateNPC` (`0x482510`) writes that word from the record's tags (at `0x4839F6`): one mode in its low 16 bits, the last of the tags `0x12` (1), `0x13` (2), `0x14` (0), `0x2B` (8), `0x2E` (`0x10`), `0x30` (`0x20`), `0x2F` and `0x31` (`0x40`), `0x32` (`0x80`), `0x42` (`0x400`), `0x43` (`0x800`), `0x44` (`0x1000`), `0x46` (`0x4000`), `0x4C` (`0x8000`), and bits above it from others; the tags `0x08` and `0x0E` set a creature hostile or not.
In the Seraphim's start script, the monsters have bit 0 (2,411 records with `08 12`), the guards the mode `0x4000` (`0E 46`), the citizens 0, 8, `0x400` or `0x800`, and of the start's people only the captain `hauptmann3` has bit 1 (`0E 46 13`); the add-on's trees give bit 1 to dormant hostile groups (`08 14 13`).
The creatures that the game makes otherwise (`0x480D40`, the spawners `0x4A02A0`, `0x4E4CF0`, `0x5B2600`, `0x53ECA0`) get bit 0 when hostile to the heroes, and `sdk_HIRE` clears bits 0 and 1 of a creature a hero hires.
So the hero passes through the soldiers and the citizens of the start, as the maintainer saw (2026-10-19), and the monsters, the captain and the hostile spawns stop it (**confirmed** by code; not checked in the game).
Every other creature, of an id above 16, is stopped by every live creature that passes the conditions above, the hero among them: the NPCs stop at the hero and at one another, and go round them (**confirmed** by code; not checked in the game).
`0x54A6D0`, which skips the walk of the cell's objects altogether, holds for a creature in a visual state 1 to 3 (`+0x60`, **hypothesis**: fading or invisible) and for the Dark Elf in some states of its own, not in an ordinary walk.

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
`0x4F75F0`, as a creature takes a gait, sets its speed (`+0x375`, an integer) to its `Creature.pak` record's walk speed (`+0x26`, for the gait 1 at `+0x1F8`) or run speed (`+0x28`), to a default of `0x4F73C0` where that is 0, or to 200 for a mounted creature (`+0x1F4` bit `0x200000`), times the balance variable `GameSpeed` (1.25 in `Balance.bin`, docs/02 §6.3) times `f`, truncated; `f` is the word `+0x40` of the creature's stats at `+0x3A8`, its pace (`0x585070`; a companion's owner's in some cases, and the Daemon's (type 9) otherwise under the effect `0x449`), times the creature's float at `+0x2C9`, 1.0, times 0.01.
The pace is the same for both gaits (read 2026-10-23, #599): the stats' recomputation `0x5796A0` starts it at 130 for every creature but a horse and the type 5 (the Wood Elf), 120 for that type (at `0x579A61`, beside the attack speed `+0x3C`, 140 and 100, and the word `+0x3E`, 120 and 100); a horse's comes from a table by the item of its slot 13, its saddle (`0x4462B0`), and a creature hostile to the hero takes 160 on the difficulty 3 and 190 on 4.
The items' effect 812 (`0x32C`) adds its value to it, truncated (`0x57A8C6`), as 811 does to the attack speed; the skill 11 adds a fifth curve beside its four resistances' (`0x5681E1`), of the balance's `RoffSP`, `R_s_SP` and `R_w_SP` (1, 50 and 10: 1 at the skill's level 1, 2 from level 4), truncated; an item above the creature's level lowers it later in the recomputation, not read further.
So a townsperson walks at 130 and the Seraphim, her blade's 812 of 2 on her 130, at 132, as the original's driver reads it (docs/re/items.md).
**Confirmed** in the original (2026-10-23, #599, the hero's position sampled every 20 ms): the Seraphim runs 294 units a second, by the formula `180 · 1.25 · 1.32`, 297; a townsperson of type 655 walks about 102, by `60 · 1.25 · 1.30`, 97.5; so the speed is in world units a second.
The same call sets the rate of the creature's clip (`0x413610`, Granny's animation parameter 2) to `GameSpeed · f`, the pace's too, times the motion's own factor where that lies in [0.8, 5) and is not 1: `MotionDesc +0x9C`, which is `MOTIONS.PAK +0x62`, `floats_62[0]` of `tables/motions.ron` (docs/02 §4.1).
29 records hold one and 13 of them lie in that range, all attacks: 12 with a bow or a crossbow, on foot or riding, and one special (counted on `MOTIONS.PAK` and `Models.tmp` 2026-10-20, #322); no walk or run has one.
**Hypothesis**: the factor applies only if this call runs while one of those attacks plays, which is not read.
So the creatures move and their clips play `GameSpeed · f`, about 1.6 times, faster than the tables and the clips have it, together.

## The hero's orders and its stop

Read 2026-10-23 (#640).
The render loop's tick (docs/re/ai.md, "The world's tick") calls `0x60F130`, which takes the mouse's order on the world: while the left button is held (the engine's `+0x54` bit `0x10`, its press past), once more than 0.4 s have gone by since the press, it posts the hero an order to walk to the cursor's point (the event 4, through `0x617030`, above) at every tick; otherwise it keeps the press's time.
The orders reach the hero in its update, on the tick: **confirmed** for an order of the driver's `go` below; where the press's own order is posted is not read (**hypothesis**: the same).
**Confirmed** by a probe of the hero's run of 9 cells (2026-10-23, its position sampled about every millisecond, the ticks marked by the drops of `0xAD6EFC`): the action `+0x150` went from 0 to 1 exactly at the first tick after the order, and the first step came in that frame; the position then changed at every frame, about 5 units each at 300 a second, at every offset within the ticks.
The hero reached its target 51 ms into a tick, its last change of position, but its action stayed 1 until the second tick after (183 ms later), went to 2 there and to 0 at the next tick.
A capture of the screen around an arrival (a frame every 33 ms) showed the run clip playing in place on the frames between the last step and the action leaving 1, then the blend into the stand: so the hero runs in place for one or two ticks after it arrives.

## The cursor's point

Read 2026-10-20 (#149, #204).
`0x616AD0` finds the cell under a point of the screen for a creature, the local player's hero: `cEngine::updateWorldCursor` (`0x612A00`) calls it for the cursor, and `0x617030` for the point of an order.

- It takes the screen point into the world at height 0 (`0x6239C0`, through the view's matrices) and the cell there, and walks from that cell down the screen, one cell along both axes a step (`+1, +1`), for `|h / 24| + 8` steps, `h` the creature's own height (`0x449950`: its cell's height on its level, plus its floor's).
  At each step it tests three cells, the step's own and the next along each axis (a table at `0x95F208`: `(0, 0)`, `(1, 0)`, `(0, 1)`), so that the walk covers the cells whose ground, raised, shows under the point.
- In each cell it tests the ground and every floor of the building there (`0x635400` gives their count), each at its height: the cell's height on that level (`0x636520`) plus, from floor 2 up, the floor's (`+0x33 · 28` units, docs/re/drawing.md, "Depth").
- It skips the cells of a mass or a wall (the ground byte's low nibble 1 or 2) and those that may not be walked on (`0x636C10`), and keeps the one whose point lies within √1200 units of where the screen point's line of sight crosses the level of that point's height, the line's direction `(0, −400, −200)`; of several, the one of the smallest parameter along the line, the nearest the eye (**hypothesis** for the sense).
  `0x6239C0` takes a world point to the screen's pixels (`(x · 48 − y · 48) / 53.66563`, `(x · 24 + y · 24) / 53.66563`, from the constants at `0x892044`…`0x892050` that `0x622580` and `0x6225B0` load) and then through the view's matrices (`0x623940`), so the test is made in the models' frame: √1200 of its units are about 52 world units across the screen, nearly a cell's side (**hypothesis**: the frame's unit is the models', 1.52 world units).
- A floor `n` ≥ 1 counts when the building's object has flag `0x400` (an open building, docs/re/drawing.md, "Buildings: masks and layers"), when `n` is 1 and the creature is on the ground (its layer at `+0x2C` is 0, as `0x449950` reads it), when the building's state shows the layer `n`, or when `n` is the creature's own floor (its position's byte `+0x24`), of whatever building.
  So from outside, a click on a house reaches its ground floor inside.

The cursor's update shows the action cursor (`mouse_action.tga`) over a cell of the nibbles 3 (a teleport) and 5 to 8 (the stairs), and over a cell that a map keyed by the packed cell and layer holds (`0xAB4498`, `layer << 26 | y << 13 | x`).
An order of the player (`0x617030`, events of the types 4 and `0x105` with a point) takes the cell found as its target, its point and its layer, unless that map holds the cell, whose stored cell and layer then replace it, and passes the order on to the creature (its vtable's `+0x18`); where nothing is found it passes the order unchanged.
Not read: where the map is filled, and how the creature's order reaches the way's search (`0x4F7710`).
So a click sends the hero to the walkable cell whose ground or floor, at its own height, shows under the cursor: up a hill, onto a bridge's deck, onto a castle's wall walk from outside, and onto the upper floors of a closed building only while the hero is in it.

## In `opensacred-game`

`terrain::Heights` keeps each loaded cell's walkability by the rule above, on the ground and on the sectors' inside layers; a door that names a trigger (`CreateObj`'s `0x29`) opens and closes its trigger's cells with it, so a door that a script closes keeps every creature out until it opens (#372).
The hero opens or closes a door by a left click on it (`doors`, docs/re/fight.md, "Doors in the way"): the door nearest the cursor's line of sight, as a creature is picked, which the hero turns once it stands within 300 of it; the hero walks where the click sends it, beside the door's cells, rather than to the door's outer point (#273).
`paths` searches the way as the client does (`opensacred_rules::ways`), over the eight neighbours in a 64 × 64 window but not between two cells that touch at a corner (the client's search is not read to do so; the way so goes round into a house where the original's hero stops at the door, docs/decisions/2026-10-22-doorway-corner.md), at the hero's level and, at an entrance on the ground or an exit on a layer, only onto the level it leads to (`terrain::Heights::neighbours`), and the hero heads for the farthest cell of the way on its level that it reaches in a straight line, or for the target itself when nothing stands between; at a change of level on its way it goes onto the way's next level.
The hero and the walking NPCs find that course once, as they take their order (`paths::Course`, #981, #989): straight on when the line to the target is clear, between the two cells' centres as well as from where the walker stands, else the way; and they keep to it, the way's legs crossing the way's own cells alone, past corners between cells they may step on (`opensacred_rules::ways::clear_by`), until the next order, a stair's landing off the way, or for an NPC the end of a leg short of its target.
A creature that comes into the way later stops the walker at its step rather than turning it round, as quest 1's novice stops at the hero put in its way and stays there (seen in the original 2026-10-25, #227's check 1), and the hero keeps off the barrel of (5938, 404), whose cell it may step on, both ways along its diagonal, as the original's did.
Before, the way and the straight line were found anew at every frame, from wherever the walker stood: the NPCs went round whoever stepped into their way, and the hero, a little off its cell's centre, cut through the barrel's cell beside the way.
A line counts as passing a corner when its crossings of the two edges lie within 0.002 of a cell of each other (`ways::CORNER`): a point of the world in `f32` carries about 0.0006 of a cell, and the line between two cells' centres along their diagonal otherwise passed a hair beside the corner, through one of the cells beside it.
On its way a walker heads for the farthest place it reaches in a straight line from where it stands, and for the target itself where the line to the target clears, past the way's last place (`paths::follow`, #996).
It keeps the point it took while the line to it from where it took it stays clear (`paths::Aim`), and takes a farther one when that one's line clears; with none, it heads for its cell's centre, and from the centre itself tests the next step of its way, stopping where that is not clear (`Step::Blocked`), a creature having come beside the corner.
It walks along its line, put back onto it at every frame (`paths::advance`), and a walker put more than a unit off it (a leap) takes its next point anew.
Before #996 the line was tested from where the walker stood at every frame and the target's line not at all, and the walker turned round at every frame: from its cell's centre the diagonal onto the way's last place cleared, and a step toward a target off that place's centre took it off the line, back to its own centre; the farthest place it took cleared at the corner's tolerance, which the rounding of its position at the world's coordinates crossed a few frames on; and a step toward its cell's centre that brought it within half a unit of it stopped it there, the diagonal not clearing from where it stood.
In a run of 19,684 walks on random 16 × 16 grids, 30% of their cells closed, at the world's coordinates, 3,924 had turned round in a loop and 2,505 stopped short; now the 16,902 that have a way arrive but for 3 that step into a closed cell along a line that starts on a cell's edge.
The line's two thirds and the radius of `+0x399` are not followed: the line is clear only to its end, and every walker is a cell wide.
A target it cannot reach it runs straight toward, until a step would take it into a cell it may not walk on.
A click sends the hero to the cell under the cursor and onto its level as the original finds them (`pick`, #208), the way's search ending on that level (`opensacred_rules::ways::path_onto`); the test of nearness is a circle of √1200 · 1.52 world units, not the models' frame's slightly squeezed one, and where nothing is found the hero goes to the point of the cursor's line of sight at its own height.
The overlay and the remote state show the target's level (`to X,Y inside X,Y:L` or `ground`).
A hero on a building's mass on the ground is put on its building's floor 1 where its grid holds the cell walkable (a start inside the monastery; the console's `cell` and `sector`, which the client lacks, only in a room that the building's entrances lead to, docs/10, #458), and a hero that steps onto an entrance or an exit goes in or out as the client's does (`terrain::Heights::transition`), the floors read from the world format (version 6: the buildings and their floors, `opensacred-convert world`).
A hero that steps onto a stair from off the stairs goes to the stair's floor at the cell its offsets give (`terrain::Heights::climb`, `hero::run`), and the way's search knows the stairs as the client's flag `0x100` makes them: a place of the way carries whether the step onto it climbs (`terrain::Footing`).
A hero sent to a stair arrives when it climbs it, and stands where the stair put it: walking back onto the stair would take it away again, round and round, as it did at the towers of 17,15, 17,14 and 17,11, whose floor 2's hatch lands on a closed cell (#373).
The original's hero that such a stair lands in a wall steps out at once (**seen** 2026-10-25, #227's check 24, #978): put on floor 3 at (1135, 747), it stood at (1136, 747) 0.16 s later, and put on floor 1 at (1135, 749), at (1136, 750) 0.15 s later in one run and at (1134, 750), the stair's own cell on floor 1, in another; what moves it is not read.
The game's hero landed on a cell it may not walk on steps out to the first neighbour it may walk on and that is no stair, in the way's order of steps, the sides first (`hero::step_out`): (1136, 747) and (1135, 750) at those two stairs.
The buildings' states follow the hero's floor, `1 << n` (`buildings`); the teleports (3) are not followed, there being none in the files.
The hero and the NPCs stand at the height that `0x636520` and `0x449950` give (`terrain::Heights::standing`, #141): on the second heights of their level's cell, a bridge's deck over the water, and on a building's floor at the floor's height above them.
The cells that the start script's and the sectors' functions' `TriggerPatch`es put under a trigger their `CreateTrigger` made closed (its markers taken in order, `0x26` open, `0x27` closed, `0x2C` closed and barred, `0x2D` unbarred, `0x496520`; #1006) are closed to every creature at their levels (`terrain::Heights::occupy`, placed by `npcs::place` as their sectors come in, #216, #221): the chests', the barrels' and the book stands', the side cells of the wide objects, the signposts' and the scripts' blocking cells and the barred doors', 4,192 in the Seraphim's tree; the way of the hero and of a walking NPC goes round them, and a step into one stops, as into a cell that may not be walked on.
The objects of category 4 stop the walking NPCs alone besides, as `0x4F7E10` has them stop a creature of id above 16 (`opensacred_rules::ways::InTheWay::object`), so the hero passes the few that no patch closes.
A door turns its trigger's cells with it (#372), and a barred trigger's door opens to nobody, the hero's turn and the townsfolk's alike, as the door's event handler opens no barred door (`0x44BCD0`, docs/re/fight.md, "Doors in the way"; #1006): the door of quest 2411 in 38,94 (trigger 519) stays shut, its cells closed, as in the original.
The creatures stop one another's steps by the conditions above (`opensacred_rules::ways::InTheWay`, which `crowd` keeps for each creature and object at its level, #212): each frame takes the cells the hero and the NPCs stand on at their levels, and the hero's way and steps go round a cell only where a creature of bit 0 or 1 of its flags word stands (the start's captain `hauptmann3`), a walking NPC's round any creature's but its own; a big creature (`Creature.pak` flags `0x02`) or a critter stops none, a fill's critter by its constructor's bit (`opensacred_rules::ways::constructed`, #468), `CreateNPC`'s, its word written from its tags, as the others.
The flags word is made from each `CreateNPC` record's tags as the client makes it (`opensacred_rules::ways::flags_word`); the goals (`+0x259`) are not followed.
A follower (#895) does not stop its master's steps, the hero's, unless it is hostile to it, of class 7 or of the guards' mode (`opensacred_rules::ways::stops_its_master`, `crowd::Crowd::stops`, #1013), so that a summoned one, of bit 0, stops only the others'.
At the start the hero walks through the guard `wache2_rg1` (3347, 2535) and stops before the captain beside it (3349, 2534) (checked through the remote control).
The hero takes its orders at the world's next tick (`hero::take_orders`, #640): a press of the left button orders it once, and held longer than 0.4 s at every tick after (`hero::steer`), as do the commands (`go`) and its attack's and art's approaches; it moves at every frame, and arrived, or stopped by a cell it may not step into, it runs in place until the second tick after, when its clip blends into the stand.
The deviations are in docs/decisions/2026-10-18-walking.md and 2026-10-22-doorway-corner.md.

The hero's and the NPCs' walk and run speeds are their `Creature.pak` pairs times their pace's factor (#599, `opensacred_rules::fight::Combat::pace_factor`, the pace of `fight::pace` with the items' 812 and the skill 11's curve; a horse's 130, its table not read) times `GameSpeed`, in world units a second, rather than times the models' scale, 1.52, as until #599; a walk's and a run's clips play at `GameSpeed` times that factor, every other clip at `GameSpeed`'s rate (`hero::GameSpeed`, read from `tables/balance.ron`); the footsteps, taken at fractions of the clip (docs/re/sound.md), follow it.
The motions' own factors (`MotionDesc +0x9C`, above) are not followed.
