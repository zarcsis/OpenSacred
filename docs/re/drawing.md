# Drawing the world: the order of the objects

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra 12.1.4 on 2026-10-18.
The draw lists and the cells' chains are in [docs/02 §6.1](../02-data-formats.md#61-itemspak-itm-v5-the-object-template-database) and [§5.5](../02-data-formats.md#55-staticpak-floorpak-triggerspak); the depth formula is in [§5.3](../02-data-formats.md#53-heights-and-the-render-contract).
Status: **confirmed** on the code unless marked otherwise.

## The rows

A frame's world pass (the function around `0x632100`…`0x6328B0`) walks the screen's rows of diamonds from the top, two rows a step (the cell index advances by `0x41`, one cell along each axis, between the step's two rows), and hands each row to `0x62AE90`.
A row is a diagonal of the grid, `cx + cy` constant.
`0x62AE90` walks it from left to right: from a cell to the next the index moves by `−0x3F`, one column on and one row back, so `cx` grows by 1 and `cy` falls by 1, and the walk crosses into the neighbouring sector of the 3 × 3 that are loaded when it leaves one.

In each cell `0x62AE90` draws the ground's quad, then walks the cell's chain of statics (the head at `+0x04` of the cell, the next at `+0x1F` of each record) and appends each drawn object to a draw list.
The lists are vectors of 24-byte entries (a kind, the object's index, its screen position as floats and as integers), and every entry is appended at the end, so a list holds its objects in the order of the walk: by row, then left to right, then in the order of the cell's chain.
The kinds seen: 1 a sprite, 2 a miniature (object flag `0x20`), 8 an entry of the list read as list 1 (docs/02 §6.1), `0x11` a sprite of a type with bit `0x10000` when `DETAILLEVEL` is 1 or more, its mask is 2 or more and it lacks flag `0x800`.

After the last row the same function draws, in order (read 2026-10-18): the ground's transitions (`0x62D3C0`), the objects of the liquid cells (`0x62E410`, below, when there are any), the liquids' surfaces (`0x62DD00`, docs/re/liquids.md), and the five lists (`0x62FF60`), each list in turn and its entries in their order.
So every object of an earlier list lies under every object of a later list, whatever their rows.
`0x62FF60` draws a miniature where it stands among the list's sprites, as it does a sprite, without the depth test.

## The liquid cells

In a cell whose nature is a liquid (`+0x1F & 0xF0` of `0x90`, `WATERA`, or `0xA0`, `WATERB`, docs/02 §5.2), `0x62AE90` appends the miniatures that would go to list 0 (of a type with bit `0x4`) to a vector of their own instead (`0xAD7848`, of the lists' 24-byte entries), and with them the world's objects of the cell whose type passes `0x434070`, that lack flag `0x100000` at `+0x14`, whose virtual function at `+0x24` answers 0 to `0x2A`, and whose type is not a chest's (category 4, `0x428440`).
`0x434070` passes a type of the category `0x20` (`items.pak +0x2E`), the waters' crab and fish 163 to 166 (docs/re/ai.md), and a type whose `Weapon.pak` index (`+0x18`, docs/02 §6.1) is above 0 and below the count at `+0x563000` of the items table, an item (read 2026-10-25, #983, the call at `0x62CB0E`); a type that `0x4280B0` sends to list 0 goes there before these tests.
`0x62E410` draws that vector as `0x62FF60` draws a list, entry by entry with the same code for each kind, before the liquids' surfaces: those objects lie under the water.
So the surface, as opaque as the bed is deep (docs/re/liquids.md), and the clear waters' second layer, added to the frame, lie over a fish, its shadow, which its draw makes right before it ("The models' shadows" below), and an item lying in the water.
**Seen** 2026-10-25 with #448's driver: in 50,39's pond, over a bed of −10, the waters' shoals and three more made with `spawn` showed as pale, water-tinted shapes under the surface, the 164's leaping fish too, in an average of 16 frames against 16 without them; in a single frame they are faint slivers that the water's animation nearly hides.
An exception, by code (its meaning not followed): a cell whose liquid material is 13 sends its objects to the lists as a dry cell does while the building at the global cell (3563, 1336) has the state 2.
The material is the byte `+0xF7` (`WATERA`) or `+0xF8` (`WATERB`) of the sector's record of docs/re/daynight.md, at that record's place keyx `+0x2E0` and `+0x2E1`, the sector's two liquid slots (docs/02 §5.1).
Over the world 12,127 miniatures lie in liquid cells (12,024 of `WATERA`, 103 of `WATERB`), counted on the sector files 2026-10-18.

## Buildings: masks and layers

A cell whose flags (`+0x1E`) have bit 0 set belongs to a building: for such a cell `0x62AE90` asks `0x6354D0` for the building's object, which follows the cell's two signed offsets at `+0x1C` and `+0x1D` (the "offsets of the parent object" of docs/02 §5.2) to the cell that holds it.
The building's trigger (the object's `+0x27`) selects a 16-byte record of a table of the world (`+0x268` of the object at `+0x04` of the view), whose word at `+0x0A` is a mask of one bit, the building's current state.
The table holds the records of `Triggers.PAK` as the file has them, which `0x638570` loads (docs/02 §5.5): the word is 1 in every record of the file, and `cTrigger::setState` (`0x415F00`) and `cTrigger::resetState` (`0x416280`) set and clear its bits (read 2026-10-18).

- A static in such a cell is appended only when its mask (`+0x2B`, read as a word: 0 or one bit of 1 to 16) equals that state, if its type has bit `0x4` or `0x800000` (lists 0, 2 and 4) whatever its flags, or if it has flag `0x8` (list 3); the other statics of list 3, and the statics of cells outside buildings, are appended whatever their mask (`0x62BAD0`, read again 2026-10-18, #82).
  A mask of 0 equals no state: the 3,804 miniatures in the world's buildings' cells, all of mask 0 and without the flag, are never drawn (the slabs under the ends of the start's monastery chapel's stairs among them), and the 115 sprites of list 0 there without the flag, all of mask 1, only in the state 1 (counted on the sector files).
- The world's dynamic objects of the cell's own chain, layer 0, are drawn on every cell, a building's too, whatever its state (read again 2026-10-19, #148).
  Before them, when the layer that `0x416570` gives, the index of the lowest bit of the state (bit 0, layer 0; bit 1, layer 1; …), is not 0, the walk takes the objects of the building's floor of that number at the cell (`0x6354D0` for the floor, `0x634C50` for its cell); when the building's object has flag `0x400`, an open building (below), those of every floor of the building, whatever the state.
  The first reading (#99) had the floor's objects taken in place of layer 0's.

**Open buildings** (#149; counted on the files 2026-10-20): 253 of the 1,246 buildings' objects have flag `0x400` (`Static.PAK +0x08`), 252 of two floors and one of one.
By the parts in the cells that point to them (236 of the 253; no cell points to the other 17), 229 are the walls and towers of the castles, by the editor names of their parts the curtain walls `Waldburg1_*` and the crenellated towers `Wb_turm1_*` (over sectors 9 to 65 across and 20 to 55 down, 22 of the 253 in sector 45,52 alone).
Their floor 1 is the inside at the wall's foot, whose part of mask 2 is a single bush (`Busch 1`, in 227 of them); their floor 2 is the wall walk or the tower's top, whose parts of mask 4 repeat the shell's of mask 1, so that the wall looks alike from outside and from its walk.
None of them has a roof among its parts, while the 46 towers of the set `Wb_turm2_*`, which lack the flag, have theirs (`Wb_turm2_Dach_oben`).
One more is a gate of one floor (sector 55,20, `GATE_RAND_*`, `GATE_TUER_*`), and 6 have no part of flag `0x8` in their cells.
A render of sector 45,52 by `opensacred-game` shows a stone curtain wall with a wooden walk behind its battlements and a tower with a crenellated top.
So the flag marks a building open to the sky: what stands on its upper floor, a guard on the wall walk or an archer on a tower, shows to a hero outside, whatever the hero's state of the building.
Of the sixty-odd functions that look up a building or its object (`0x6354D0`, `0x635440`, `0x635E50`), two test the flag (read 2026-10-20): the row walk above, and the cursor's point (`0x616AD0`, docs/re/movement.md, "The cursor's point"), which takes every floor of such a building besides the ground; the changes of the states (`0x5FCCB0`), the objects' drawing (`0x62FF60`), the light (`0x62E410`) and the creatures' movement do not.
On the cells of these buildings the start scripts place 3 chests of layer 0 and nothing of layer 1 or above (the same in all 10 start trees), so the flag shows only once creatures walk on the walls.

The state is the building's exterior (with its roof) while the hero is outside, and the client changes it as the local player's hero walks in and out (**confirmed** by code 2026-10-18, #99; docs/re/movement.md, "Changing layers"): to 2 as the hero steps onto an entrance of the building and goes onto its inside, back to 1 as it steps onto an exit and comes out onto the ground, to `1 << n` as it climbs to the building's layer `n`.
This is how the original hides a roof and shows the rooms and their furniture (#3): at the chapel of the start, as the hero passes its doorway, not on its stairs.
The cells' room bit (bit 1) does not change the state: the client's event as the hero's cell gains or loses it (`0x594011`, docs/02 §5.2) is something else, not followed.

**Hypothesis**: a start script's position `(x, y, z)` puts its object in layer `z`: of its 2,096 placements at a cell, 653 objects and one NPC have a layer of 1 to 4, and 649 of them stand on cells of a building (bit 0), against 45 of the 1,442 of layer 0.
While the hero is outside, the original draws layer 0 of every building, so the furniture of layers 1 and above does not show.

The statics agree (**confirmed** on the sector files): in building cells, those flagged `0x8` of mask 1 are the buildings' shells (the 53 parts of the start's monastery chapel, by their editor names), those of masks 2, 4, 8 and 16 their rooms' furniture (candles, benches, plates, beds in the chapel): 9,224 and 48,942 statics over the world.
A building's state is 1 while the hero is outside: the file's value, and the exits' (above).
The parts of mask 2 include the rooms cut open: the pub of sector 50,40 has its shell as `TH_PUB_0_*` (mask 1) and its ground floor, floor boards, bar and inner walls as `TH_PUB_1_*` (mask 2, by their editor names), and every cell of a building points to one anchor cell through its offsets (the pub's 12 × 11 cells to the cell of `TH_PUB_1_BODEN`, its floor).

`opensacred-game` keeps a state per building, by its anchor cell (`buildings`): `1 << n` for the building on whose floor `n` the hero walks, 2 on its ground floor, which it goes onto at the building's entrance and leaves at an exit as the client does, and the upper floors up and down its stairs (docs/re/movement.md), else 1.
The parts (the statics above that the state chooses) show while the state is their mask, and the start script's objects on a building's floors while the state shows their floor: inside, the roof and the shell's walls go, the ground floor with its furniture comes (#3).
The start script's objects and NPCs of layer 0 show whatever the states, on a building's cells too, as the original draws every cell's objects of layer 0 (#148): quest 1's novice on the chapel's stairs, cells of the monastery, while the hero is in the chapel and the monastery's state is 2.
Before, they went while the hero was inside their building, by the first reading above, and the novice vanished on the stairs.
An NPC that a script sends somewhere takes the layer of the level it walks on, onto the building's inside at its entrance and out at an exit as the hero does (`0x5FCCB0`, docs/re/movement.md), and keeps it where it stops (#138).
Before, it stayed in layer 0 after going in: quest 1's novice, at the chapel's benches, showed through the chapel's wall while the hero stood outside.
The objects and NPCs on any floor of an open building show whatever its state, by the flag that the world format (version 8) carries with each building (#203).

## Depth

`0x62FF60` draws a list's sprites piece by piece.
For a sprite whose type (the `items.pak` record's `+0x00` as a u32) has bit `0x200`, it switches two render states of the engine's state table (`0x100` and `0x1`, the depth write and the depth test: `0x643470` sets `D3DRS_ZWRITEENABLE` and `D3DRS_ZENABLE` for them, confirmed 2026-10-18) on and gives each of the quad's four vertices the depth that `0x6288A0` computes from the vertex's screen y and a height: the static's byte at `+0x33` (Static.PAK) times 28.
`0x6288A0` returns the depth of the point at that screen y lying that high above the ground (docs/02 §5.3), so the quad lies in a horizontal plane at that height: what stands below it is hidden, what stands above shows.
For every other sprite the two states are off and the depth is 0: it is drawn over what is there, in the order of its list.

In the game data 47 types carry bit `0x200`, with 220 drawn objects in the whole world; by their editor names they are the floors of towers' and houses' upper storeys.
Their types lack bit `0x4`, so the floors go to list 3, among the sprites by their rows (counted on the sector files 2026-10-19, #143).
Every floor's object belongs to its building's part of its storey ("Buildings: masks and layers"), 203 parts in all, and lies in the first rows of the part: the first floor of a part 2 rows after the part's first (the median; 0 to 51).
So the client draws nearly all of a part's sprites after its floor, 7,664 of the 8,121 sprites and miniatures of those parts, the floors aside, and draws them over the floor whole.
4,205 of them reach further down the screen than half a cell below their cell's point on the floor, which the floor's plane, nearer there than their cells, would hide in a depth test: the four beds by the left wall of the house of sector 52,39 by 24 to 49 pixels, its carpet by 128, the top of its art at its cell's point on the floor.
So do 574 of the 1,356 shadows drawn with them (entries of kind `0x11`, below).

Read again 2026-10-19 (#120): `0x6288A0` returns `(2 · zoom · (200 − y · 400/768) − h / sin(atan ½) + b) / 3100 + 0.002` for the screen's `y` and the height `h`, `b` the renderer's `+0x970AC`, which the world view's constructor (`0x624A00`) sets to the distance from the camera to the point it looks at, `√(1200² + 600²)`, plus 600: the depth of the models' own projection (`ortho(±267, ±200, −600, 2500)` under the view of `0x6284E0`, which looks down at atan(1/2) from 1,200 units back and 600 up), of the point at that height in the models' frame.
So the plane is at `h` units of the models' height: `+0x33` 4, 112 units, lies 192 pixels above the ground at zoom 1 (a unit of height `768/400 · 2/√5` pixels, 1.72).
The 220 floors' `+0x33` are 4 (126 of them), 5 (39), 8 (34), 6, 7, 12, 14, 16 and 21.
A building's floor objects (its static of flag `0x10`'s children, docs/re/movement.md) carry the same byte: 0 for every floor 1, 3 to 10 for floor 2 (mostly 3 and 4), 8 to 28 for floor 3, 21 for the one floor 4 that has it (counted over the 1,245 buildings 2026-10-19).
The grids of a building's floors lie at one place (the two floors of the house of sector 52,39 at (3328, 2519)), so the cells of floor 2 lie over those of floor 1.
A creature on a building's floor stands at its floor object's height, `+0x33 · 28` units, above the second heights of the floor's grid, so the floor's plane does not hide it (**confirmed** by code 2026-10-19, #141, docs/re/movement.md, "The height").

The other states of that table (`0x643470`, the engine's switch of Direct3D 7 states, through `IDirect3DDevice7::SetRenderState` and `SetTextureStageState` at `+0x50` and `+0x94` of the device's table; read again 2026-10-18): `8` and `0x10` make the source and the destination blend factors `ONE` (else `SRCALPHA` and `INVSRCALPHA`), `0x40` the depth function `GREATER` (else `LESSEQUAL`; only `cGranny::renderShadowFake`, `0x4073B0`, sets it, for a second pass of a model's fake shadow), `0x800` the linear filter of both texture stages (else point), `0x2` the culling of counter-clockwise faces (else none), `0x4` the alpha blending, `0x20` the lighting, `0x4000` the alpha test.

## Blending

Read 2026-10-18 (#35).
The device's setup (`0x643190`) puts the alpha test's reference at `0x40` and its function at `GREATER`, turns the alpha blending on and the alpha test off.
Before a sprite's pieces `0x62FF60` turns the lighting and the culling off, the blend factors to `SRCALPHA` and `INVSRCALPHA`, the filter to point and the alpha test on (`0x630367`…`0x6303AA`); after them, the depth states and the alpha test off again (`0x63098F`…`0x6309B0`).
So the original drops a sprite's texels of alpha 64 or less, the atlases' 4-bit alphas 0 to 3 (`MIXnnnn.444`, docs/02 §3.3), and blends the rest by their alpha over what is drawn, in the order of its lists, a texel to a pixel at zoom 1.
A miniature (`0x630F2C`…) is blended by its alpha too, without the test, which the sprite's path leaves off; it is filtered linearly while the zoom is over 1.2, else by point (`0x630F5B`, and `0x62F321` for one of a liquid cell, `0x62E410`).
The zoom is the world view's float at `+0xB60`, which the two setters of the zoom (`0x628130` and `0x628310`) keep within 0.5 to 2 and which scales the projection, 2 the widest view (docs/02 §5.3, docs/re/ai.md); so the miniatures are filtered linearly in the wider views, where a texel of the art covers less than 5/6 of a pixel, and by point at 1.2 and closer (read again 2026-10-20, #343).
The ground takes the same switch: `0x62AE90` sets the state `0x800` by the same test against 1.2 at the head of each row, before the row's quads (`0x62AF32`; docs/02 §5.4).

## The statics' shadows

Read 2026-10-18 and 2026-10-19 (#100).
The shadows are not painted into the sprites: the trees' and the bushes' sprites have no half-transparent texels.
The world view's constructor (`0x624430`) loads `SHADOW_TREE00.TGA` (`texture.pak` record 14942, 512 × 512, black, its alpha up to 170) into `+0x97070` of the view, a grid of 16 × 16 silhouettes of 32 × 32 texels (trees, bushes, posts, fences, rocks, round and oblong blobs), and fills a table at `+0x30` with the texture coordinates of each cell's top-left corner, `(column / 16, row / 16)` for cell `16 · row + column`.

`0x62AE90` appends an entry of kind 8 to list 1 for every static whose type has bit `0x10000` (`items.pak +0x00` as a u32), that lacks flag `0x800`, whose mask is 1, while `DETAILLEVEL` is 1 or more (`0x62BA03`).
It does so before it tests the building's state or looks for the static's sprite, so the static need not be drawn, and a static of a liquid cell sends its shadow to list 1 too.
A sprite of such a type with a mask of 2 or more, the buildings' furniture, goes to its list as an entry of kind `0x11` instead of 1 (docs/02 §6.1): drawn with its building's part.

`0x62FF60` draws an entry of kind 8 with `0x631EB0` at the entry's screen position, the static's position, the sprite's top-left corner (`0x6301FF`); for an entry of kind `0x11` it draws the shadow the same way right before the sprite (`0x63025B`); `0x62E410` does the same for the entries of the liquid cells.
`0x631EB0` draws one strip of four vertices (FVF `0x1C4`: a transformed position, a diffuse and a specular colour, one pair of texture coordinates) from the static's type's fields (docs/02 §6.1): the cell `c` (`+0x5B`), the foot `(2·dx, 2·dy)` from the halves at `+0x5D` and `+0x5F`, the shape (`+0x63`) and the size `s` (`+0x64`), all in pixels at zoom 1 from the static's position, scaled by the zoom about the screen's centre.
Its top-left and top-right corners lie at `2dy − s`, from `2dx − s` to `2dx + s` for the shape 0 and from `2dx` to `2dx + 2s` for any other, the shadow leaning up and to the right; its bottom-left and bottom-right corners at `2dy`, from `2dx − s` to `2dx + s`.
The four corners take the cell's corners of the atlas in the same order, so the whole cell stretches over the quad.
The depth test and the depth write are off, the blending `SRCALPHA` and `INVSRCALPHA`, the filter linear (`0x643430` with `0x100`, `1`, `8`, `0x10` off and `0x800` on), and the alpha test as the list pass leaves it between two sprites, off.
Every vertex is white with an alpha of `255 − N` (`0x41B9E0`), `N` the night's level (docs/re/daynight.md): the shadows fade out over the dusk, are gone by night and come back over the dawn, in a dungeon too, where `N` follows the clock as well.

On the data: 2,043 types have the bit, 1,635 of them leaning and 408 rectangles (docs/02 §6.1); 356,356 statics are of those types, 347,123 of them of mask 1 and 9,232 of a mask of 2 or more; 6,195 have flag `0x800`.
In the starting sector 50,39, 267 statics carry a shadow: bushes, fences, benches, beds, posts, braziers, trees, rocks, chairs, chandeliers.

## The trees about the heroes

Read 2026-10-18.
As `0x62FF60` draws a sprite whose type has bit `0x10` (4,544 types: the 4,533 of type 25, the trees, and 11 floors of type 29), it fades the vertices of each piece that lie near the points of a list at `+0x96B3C` of the renderer (`+0x96FEC` of them, records of 12 bytes: the point's x and y on the screen as two i16, a counter that each faded vertex raises, and a float `k`).
For a vertex within `R / zoom` of a point across and down, `d` is its distance to the point (an integer square root of `dx² + dy²`), `t = min(d · zoom / R, 1)`, and the vertex's alpha becomes `G · (1 − k) + (255 − G) · t`: `G · (1 − k)` at the point, 255 at `R` and beyond, so a tree goes transparent about the point and stays whole away from it.
`R` and `G` are the floats at `0xAD4DF0` and `0xAD4DF4`, the balance keys `TransSize` and `TransValu`: `Balance.bin` holds them, 190.0 and 50.0 (docs/02 §6.3), so a tree is at an alpha of 50 out of 255 at the hero and whole 190 pixels from it at zoom 1.
The list is emptied before the rows (`0x6325C8`) and filled as `0x62E410` and `0x62FF60` pass the creatures, up to 100: the local player's hero (an object of id up to 16 that is the local player's, `0x7D84A0`), the creatures it owns (the owner at `+0x1EC`) and its companions (`+0x251` the player's, flag `0x104` at `+0x1F4`), each at the centre of its bounds on the screen, with `k` 0.
So a tree fades about the creatures drawn before it in the frame, not about one drawn after it.

`opensacred-game` fades them so (#122): the sprites of a type of bit `0x10` that its order puts after the hero's place take that alpha by their distance to the hero's point, 60 pixels above its feet, per point rather than per corner of their pieces (docs/decisions/drawing.md, "Trees' fade per pixel"), before the sprites' alpha test, which drops the faded texels within 13 pixels of the point (`statics::Fade`, `sprites.wesl`).
Type 25 holds the walls of the buildings' shells too (`KLOSTER_KAPELLE01_0U1_*`), which fade the same way.
For a hero behind the other sprites the client does nothing (read 2026-10-19, #69, **confirmed** by code): the sprites' path of `0x62FF60` (and of `0x62E410`, the same code) tests the hero's place nowhere else, and reads the points at `+0x96B3C` only for a type of bit `0x10`.
A sprite of any other type, a building's part of type 9 such as the chapel's tower, is neither faded nor skipped nor moved in the order for the hero: drawn after the hero, it hides it.
No model is drawn as a silhouette or an outline: none of the 24 modes of a model's draw (`0x44A9D0`) sets the depth function `GREATER` or the stencil, which the shadows alone use (`0x4073B0`, `0x406FF0`), and nothing after the lists draws the hero again; `0x41F7F0` draws there eight models that cross the map on straight paths at height 240, with their shadows (**hypothesis**: birds).
The types of bit `0x800000` (list 4) always cover the hero, and a building's levels show by its state alone (`0x62AE90`), so the game, which hides the hero behind such a sprite, does as the original.

## The models

The world's dynamic objects (the creatures, the start script's objects) are entries of the same draw lists.
After a cell's chain of statics, the "wobj" walk of `0x62AE90` takes the objects of its building's floor that the state shows, if any, then those of the cell's own chain, layer 0 (above), and appends each, as an entry of kind 0 with the object's index, to list 3 (the vector at `+0x96AF4` of the view), or to list 0 or list 4 by tests of its type (`0x4280B0`, `0x4280D0`, `0x4280F0`, not followed), or in a liquid cell to the liquid cells' vector (above).
`0x62FF60` walks the five lists in order (the vectors from `+0x96AD0`, 12 bytes apart) and draws an entry of kind 0 by calling the object's own draw (its virtual function at `+0x14`), in its place among the list's sprites; an entry of the kind `0x20`, a hidden stash's, is its blink (docs/re/containers.md, "The hidden stashes").
So a model is drawn after every sprite of an earlier row and of an earlier cell of its row, and after its cell's statics; the sprites, without the depth test, cover a model drawn before them whole and lie whole under one drawn after them.

The states of a model's own draw (read 2026-10-22, #516, **confirmed** by code and in the running original): `cGranny::render` (`0x405C90`) turns on, through the engine's switch (`0x643430`, the states above), the depth test (`1`, `ZENABLE`), the lighting (`0x20`), the culling of counter-clockwise faces (`2`), the alpha blending (`4`) with the factors `SRCALPHA` and `INVSRCALPHA` (`8` and `0x10` off), the depth function `LESSEQUAL` (`0x40` off), the linear filter (`0x800`) and the alpha test (`0x4000`), which it turns off again after the model.
The depth write (`0x100`, `ZWRITEENABLE`) is on, unless the top bit of its last argument asks for it off; the rest of that argument, when not 0, names a second texture, which the second stage blends over the first by its own alpha (`0x20000`).
The first texture stage modulates the texture's alpha by the diffuse's (`0x2000`, which the device's setup turns on, `0x643190`): a hook on `0x405C90` in the running original found the state on at all 1,088 models drawn in 3 s at the Seraphim's start.
At 300 of the models' draw calls (`DrawIndexedPrimitiveStrided` at `0x4061BC`) the device itself answered so: the alpha test on, its reference 64 and its function `GREATER`; the blending on, `SRCALPHA` and `INVSRCALPHA`; the depth test and write on, `LESSEQUAL`; the culling counter-clockwise; the first stage's colour and alpha the texture's times the diffuse's, filtered linearly without mipmaps; the second stage off; and the material's diffuse alpha 1, which the lighting gives the vertices, whose format (`0x112`) has no colour of its own.
So the original drops a model's texels of alpha 64 or less, filtered linearly, and blends the rest by their alpha over what is drawn, while writing their depth.
In the running original the blacksmith of (3253, 2867) shows the front flap of his apron cut to a strip under the belt, where his texture's two strokes of alpha 0 lie (docs/02 §4.6).
`opensacred-game` tests a model's texels so (`models::CUTOFF`, #516), and blends the translucent ones that the test keeps in a second pass (#519, "What the game does with it" below).

A script's object is in the list of its own cell, wherever its model is drawn (**confirmed** by code and in the running original 2026-10-22, #459):

- `CreateObj`'s handler (`0x485C10`) puts the object at its `0x04` cell's centre, and with a floor there `sdk_CREATE` (`0x5FBA40`) sets its layer (`+0x2C`) to that floor's record (`0x6354D0`, `0x4498F0`); `0x5FCCB0` then chains it into the list of the cell at its position on its layer (the cell's `+0x08`, the next object at the object's `+0x30`), which the walk above takes.
- The same handler, for an object with a model and a world position `0x20`, takes the object's height `h` at its cell's centre on its layer in units of the models' height (`0x449950`: the layer's cell's second heights times 2.5, plus the floor's `+0x33` times 28), turns the models' vector `(0, 2h, 0)` into the grid's plane (`0x623AD0`), `1.92 · h` world units along x and along y, and sets the floats `+0xCC`, `+0xD0` to `0x20` plus those and `+0xD4` to `h`, with the object's flag `0x800000`.
  The ground's height at `0x20` is never read, so the point shows on the screen where `0x20` at height 0 does: a raised doorway's `0x20`, on the ground up the screen from it (docs/02 §7.3), so brings its leaf into the doorway.
- `cObject3D`'s draw (`0x44B230`), at its first with the model loaded and the flag set, clears the flag and takes the point for the model's position (the `cGranny`'s `+0x88`) when it lies within 200 of the object's position in the models' frame (`0x6239C0`, the larger of the two axes) for a door (category 10, `0x428130`) or a chest (category 4, `0x428440`), within 1,000 for any other; else the model stays at the object's position (`0x44D520`).
  None of the 1,132 doors of the Seraphim's tree comes near 200 (**estimate**, a scratch script over the files).

In the original, the door of (272, 2379) on floor 2 of its building (`h` 168) has `+0xCC`, `+0xD0` at (14593.6, 127699.6) and its model there, unchanged over 45 s; the closed leaf is drawn whole in its doorway after the back wall's sprites of its cell (272, 2379), though its model's root stands in the cell (271, 2379) before it.
Not read: a sector whose record lacks bit 16 of `+0x30` (`0x634840`) makes the object without a position, so that a raised door's point would fail the test and its leaf stand at its cell; and which events run `0x44D520` again (the opening does not seem to: the open leaf stays hinged at its jamb).

**The effects among the creatures** (read 2026-11-06 for #2047, **confirmed** by code unless marked):

- An FX object is a dynamic object of its cell's chain like a creature, and the list draws it by its own draw: its system's draw is a virtual function of its class (`0x5A9C50`, from the table at `0x8919B0`), so the whole system, every particle wherever it flies, is drawn at the object's place in the list's order.
- `0x5FCCB0`, which every move of an object runs, chains the object into its new cell's list at its head (the object's `+0x30` the cell's `+0x08` before it, the cell's `+0x08` the object): in a cell, the object moved last is drawn first.
- An effect that follows a creature (the fire skin at its bearer, the systems of the flag `0x200`) is moved after it, at its own update (**hypothesis** for the order of the two updates in the frame), so it heads the cell's chain and is drawn before the creature.
  Its billboards, depth tested and not written (`0x761B00`), leave no depth, and the creature's model drawn after them, depth tested and written, covers every flame in front of it and lies under none.
  An effect whose object lies in a later row than the creature, nearer the camera, is drawn after it and adds its flames over it where they are nearer.
- **Seen** 2026-11-06 with the driver of #448, the Mage's fire skin (24) at her start: her legs and her body, lava-skinned, lie over the flames at her feet, only the flames about her outline showing; the purgatory's fire about her, its object at her cell, likewise (#2047's frames, 2026-11-04).
- `opensacred-game` draws every effect in the transparent pass after the opaque models, so its flames in front of a creature added over it: the fire skin covered the Mage's legs (#2047's frames).
  Since #2178 the systems that follow a creature lay their quads at the depth of its place in the order (`fx::Orders`, `fx::Quads::in_order`): each vertex moved along the line of sight, which the orthographic view leaves where it shows, to the depth of the creature's anchor and its root's own depth, squeezed as `models.wesl` squeezes a model's texels, less 20 units of the body behind its root (**estimate**), so that the creature's model, whose texels write their depths, covers them and they lie over what an earlier row draws.
  Since #2188 only the vertices nearer the camera than that depth are moved back to it: a part of a system further back keeps its own depth, hidden by a creature of an earlier row in front of it, as the original's depth test hides it, where moved forward it lay over that creature (a ring of the fear on the ground over the legs of the goblins it passed behind).
  They are the fire skin, the shields and the regenerations, the speed's sight, the multi-shot's comets, the plants' healing's butterflies, the burning light and the holy conversion's halo; the purgatory lays its quads at its object's place, before a creature standing in its cell.
  **Seen** 2026-11-07: the Mage's legs and body over her fire skin's flames at her start, as in the original.
  A creature on an upper storey's floor, whose model lies no further back than the floor's plane, keeps its systems no further back than that plane where each of their vertices shows, in its order among the quads standing on the floor, as its model's vertices lie (`models.wesl`'s `on_floor`, #2220; before, at their own depths), and so does a purgatory there (`fx::Orders::at_level`) (docs/decisions/drawing.md, "Effects at their bearers' places").
  Since #2188 the world's fires, smokes and geysers on the ground lay theirs after the statics of their cell, at the last place of its chain (`fx::Orders::after_statics`): a fire stood in its cell from the world's start, and a creature that comes into the cell later is chained before it and drawn first, so the fire lies over its brazier's sprite and under what a row nearer draws, its tall flames no longer over a creature standing in front of it.
  Since #2220 so does one in a building: on its ground floor as on the ground, on an upper storey's floor no further back than the floor's plane, as a model standing there (`fx::Orders::after_statics_in`); before, at its own depths.
  **Seen** 2026-11-07: the brazier's fire of 3239,2527 over its brazier and over the Seraphim's boots standing behind it, as before.
  The fire spiral's flames lie at their object's place, as the purgatory's (`fx::Orders::at`): an object that moves heads its cell's list at each move and is drawn before a creature standing in its cell.
  Since #2188 too the casts' sights, the Daemon's forms, the soulcatchers, the Vampiress's claws, the burning light's strikes, the teleports while they ride on their casters, the elements of the weapons in hand and the effects of #1092's later families (the Dark Elf's, the Vampiress's, the Gladiator's, the Seraphim's, the Daemon's and the Dwarf's) lay theirs just behind their creature's model, and the healings' sights, the teleports left behind and the arrows in flight at their objects' places.
  **Seen** 2026-11-07: the Daemon's form of fire's white ribbons behind her body, where before they lay over it.
  So do the bursts (`puffs`: the undead's rings and dust, the weapons' and the changeling's sparks, the scripts' smokes), just behind the creature they follow or at their places, and at their objects' places the explosions, the goblins' and the grenades' impacts, the rings of poison, the traps' landings and the raised creatures' rings, the plant cages just behind the creatures they hold.
  **Seen** 2026-11-07: a goblin changed into a deer, the deer drawn over its green sparks.
  So do, at their objects' places, the banishing's runes, the blood's fountains, the choir's wisps and discs, the fear's screams, the holy light's column, symbol and glow, and the burning bones and the summonings just behind the creatures they burn or summon.
  **Seen** 2026-11-07: the Daemon's fear among six goblins, its column in front of her behind the goblin standing before it, and its rings on the ground behind the goblins' legs.
  The flying objects lay theirs at their places as they fly, which a moving object heads in its cell: the bolts' looks (the fireball, the railgun, the BFG, the emperor's rays, those that follow a creature just behind it), the flying skulls, the homing fires, the ice splinters, the traps' balls and the meteors.
  **Seen** 2026-11-07: the Mage's fireball bursting at her hand as it leaves, her arm over its flames, where before its flames lay over her arm and body.
  So do, at their objects' places, the scripts' prisons, the lightning's strikes, the icebergs and the lost souls as they fly, and the killer plants just behind their own models.
  So do the gusts and the torches' flames just behind the creatures that bear or hold them, else at their places, the bolts' trails at their places as they fly, and the dwarves' magic lamps of the scripts after their cells' statics, as the world's fires; the chain lightning's impacts, the dragons' fires on the ground and the blades of light at their objects' places.
  So do, at their objects' places, the marks on the ground, the battle fogs, the summonings' sights and the forge's elements and beam (#2098), and the items' own glows just behind the creatures that hold them, else at the items' places.
  So do the items' glitter, each item's sparks at its place in their one mesh (`fx::Quads::append`), the lava's bursts and their small fires, the bubbles' glows and their sonar rings, the tentacles and their auras' rings (just behind their bearers), and the beacons' helices (#2220).
  So do the hidden stashes' blinks and the events' sparks at their cells' last places, after the cells' statics and objects, as the row walk lays their entries of the kind `0x20` (`0x62CF12`, docs/re/containers.md), the breath's flames and smoke at the breath's place as it flies, each fire of its wall at its own place, and the guns' flashes at their muzzles' places (#2220).
  **Seen** 2026-11-07: a wall of the breath's fires over the legs of a creature behind it and under one in front of it; a stash's blink under the boot of a creature a cell in front of it.
  So do the Vampiress's bats' sights and her change's ribbons at their places, the Dwarf's flamethrower just behind him and his cannon's and mortar's flashes at their muzzles' places, the light's ball just behind its bearer, and the souls' sparks at their souls' places as they fly (#2220).
  **Seen** 2026-11-07: the Vampiress's ribbons over a troll a cell behind her and under one a cell in front of her.
  So do, at their places, each segment of the laser beams (an object of its own at its start's cell, so that a beam across several cells lies among their creatures cell by cell), the pieces of the scripts' Pong, the weather's butterflies as their swarms wander, and the stargates' windows (#2220).
  The stargates' whirls and the summonings' triangles and bolts, which the original draws with their depth not tested, lie at their places' depths whole and depth tested (`fx::Quads::at_order`): over what the order draws before them, as the original's draw over it, and under what it draws after, which the original draws over them; before they lay over everything.
  **Seen** 2026-11-07 at the Kernland's waypoint: a troll a cell in front of the gate over its whirls, the trolls behind it under its window.
  Since #2220 every system lies in the order.
- Not read: which of the five lists an FX object's type goes to (`0x4280B0`, `0x4280D0`, `0x4280F0`; **hypothesis**: list 3 with the creatures, as the frames agree), and the order of two creatures' and an effect's updates within a frame.

## The models' turn

Read 2026-10-19 (#159).
An object keeps its direction at `+0x70`, a vector on the grid's plane, and `0x44DBF0` turns its model by it: it takes the object's position and the position moved by the direction to the screen (`0x6239C0`, the cells' projection `48 (x − y)`, `24 (x + y)` pixels a cell, then back through the models' view), normalises their difference to `(a, b)` in the models' frame (`x` right on the screen, `y` toward the viewer) and sets the model's orientation (`GrannySetSequenceOrientationM`) to the rows `(a, b, 0)`, `(−b, a, 0)`, `(0, 0, 1)`: the model's `x` points along the direction.
It does so at once when the object has its model (`+0x14` bit 26), and the model's load (`0x44C330`, the object's virtual function at `+0x28`, which its first draw calls) does it again from `+0x70`.
The mirror of the models' middle axis as the client loads them (docs/02 §4.1) does not show on the screen: a guard facing the viewer holds his sword in his right hand, on the screen's left, in the original as in `opensacred-game`, whose models are not mirrored (**confirmed** 2026-10-22, #445; which of the view and the turn undoes it is not read).

- A creature on the move turns by the vector of its step on the grid (`0x5941A0` through `0x44DD90`, at `0x5992A6`), so it faces where it goes.
- A heading in degrees (`0x44DDC0`, which `CreateNPC` and `CreateObj` call) is an angle on the screen: `(cos, sin)` of it, 0° to the right and 90° down, goes back through the inverse of the cells' projection (`0x622660`, the 2 × 2 matrix of `0x6239C0` inverted) into the grid's plane, so the model shows at that angle on the screen.
  Read 2026-10-19 (#161): static initialisers set the matrix from the constants at `0x892048`, 53.66563 (a cell's side in world units), 48.0 and 24.0: `0x622580` the grid's `x` to `(48, 24) / 53.66563` on the screen, `0x6225B0` its `y` to `(−48, 24) / 53.66563`, and `0x6225F0` the inverse of the determinant, `1 / 0.8 = 1.25` (`0xAD7788`).
  So `0x622660` takes the screen's `(cos, sin)` to `1.25 · (0.447 cos + 0.894 sin, 0.894 sin − 0.447 cos)` on the grid, the direction `(cos + 2 sin, 2 sin − cos)`; the degrees become radians by the constant at `0x8909F8`, π/180.
- As the object manager makes an object (`0x5FB530`), it sends it the event `0x40` with the float at `+0x57` of its `items.pak` record (docs/02 §6.1) at the event's `+0x10` (`0x5FB726`), which `cObject3D`'s receive (`0x44BCD0`) loads as a float and turns into its direction the same way, through `0x622660`: the angle on the screen an object faces until something turns it, −30° for the chests, barrels and book stands of the start scripts.
- `CreateObj`'s handler (`0x485C10`) turns the object by its heading only when it is above 0, so a script's object without one keeps its item's angle; `CreateNPC`'s takes a random heading without one (docs/02 §7.3).

The maintainer's screenshots of the original agree (2026-10-19): the black chest by the chapel's pews (no heading, `chest.grn`'s −30°) and the wooden chest of a house of sector 50,40 (heading 330) stand alike, their long sides up and to the right, and the chest by the chapel's outer corner (heading 20) almost level; `opensacred-game` drew them turned along the grid's axes before, a quarter turn off for the first two.

## The models' scale

Read off the client with Ghidra 2026-10-21 (#443), **confirmed** by code.

A model is drawn at its own scale, `ModelDesc +0x470` (docs/02 §4.1), which Granny applies to the model's data as `cGrannyModelManager::getModel` (`0x411F50`) loads it, and to the motions of its slots as `getMotion` (`0x4118B0`) opens them; the gold piles are a third of their files' size, the Medusa 0.43 of hers.
The world's draw adds no scale of its own: `cObject3D`'s draw (`0x44B230`) goes through `0x44A9D0` and its case `0x44A8C0` to `cGranny::render` (`0x405C90`), which hands Granny's rendering states to the device, and the model matrix of `cGranny` at `+0x0C` is set (`0x4021D0`) only by the interface's and the portraits' code, never for an object of the world.
Granny skins from the bind pose, so a bone's own scale in the bind pose (the gold coins' 0.242 × 0.086 × 0.242, the potions' 0.063) does not change the size of a mesh at rest.

## The models' shadows

Read off the client with Ghidra 2026-10-19 (#101), **confirmed** by code unless marked.

`cGranny::renderShadow` (`0x406FF0`) draws a model's shadow, right before the model, in the object's own draw: `cObject3D`'s (`0x44B230`, the virtual table at `0x890D10`) for an object of the categories 2, 3 and `0x20` (the creatures, `0x426610`) whose creature has no flag `0x10` (`NOSHADOW`, `0x426D30`), `cChest3D`'s (`0x5C97B0`) and `cArrow3D`'s (`0x5BAF00`).
The object factory (`0x451300`) makes an object of category 4 (`items.pak +0x2E`) a `cChest3D`: of the 1,990 objects of the Seraphim's start script, the chests, the barrels, the vases, the coffins, the cabinets, the book stands and the shrines.
The signposts are of category 15, `cSpecial3D`, whose draw is `cObject3D`'s, and cast none.
The size that the draws pass is the object's height (`+0xF0`, the third coordinate of its position).
They draw it when `DETAILLEVEL` is 1 or more (2 in the GOG `Settings.cfg`; the setting is stored at `0x814152`, and at 0 `0x407960` draws a few dots of `SHADOWDOT.TGA` about the bones instead, "The shadow dots" below) and the value at `+0x1FC` of the renderer is set (else `cGranny::renderShadowFake`, `0x4073B0`, draws a quad of `GRN_SHADETEXTURE03.TGA`, the texture at `+0xA4`).

The shadow's matrix is the identity with its third row `(0.5, −0.5, 0, 0)` and its fourth `(−s/2, s/2, s, 1)` for the size `s`, after the model's own matrix (`+0x0C`, when there is one) and before a mesh's own (`0x653B30` multiplies `A × B`, `A` applied first).
So a point `h` above the plane `z = s` goes to `(x + h/2, y − h/2, s)`: flattened onto the plane at the object's height, moved by half its height along `x` and against `y`.
**Hypothesis**: a deformed mesh's vertices come from Granny in the world's frame, without a matrix of their own, as the bounds test of the shadow (`0x4057D0`) takes the bones' state through the same matrix and the plane is at the object's height.
The frame is the models' (`0x6284E0` sets the view): the camera stands 1,200 units toward `+y` and 600 up from the point it looks at, looking down at atan(1/2), `z` up; `x` runs right on the screen and `y` down it, toward the viewer.
A creature's position becomes that frame's by the screen (`0x6239C0`): from its cell position to the screen's pixels, `48 (x − y)` and `24 (x + y)` over 53.67, then back through the view and `ortho(±267, ±200)`.
On the screen at zoom 1, the shadow of a point `h` model units high lies 0.96·h pixels right of its foot and 0.43·h up: leaning up and to the right, 24° above the horizontal, as the statics' shadows lean.

It draws every mesh with the model's deformed vertices (`GrannyLockSequenceForRendering` with `0x21`) through `DrawIndexedPrimitiveStrided` (FVF `0x142`), the vertices' colour opaque black (`0xFF000000`) and the texture at `+0x9C` of the `cGranny` object, `WEATHER_RAIN01.TGA` (`0x401030`), black at alpha 170 in all its 16 × 16 texels: a shadow leaves a third of the light, by day and by night.
The depth test is on and its write off, the culling on, the lighting and the alpha test off, the blending `SRCALPHA`, `INVSRCALPHA`.
The engine's state `0x10000` turns the stencil on with the reference 1 and the mask 1 (`0x643470`), over the device's `STENCILFUNC` `NOTEQUAL`, `STENCILPASS` `REPLACE` and `KEEP` otherwise (`0x643190`): a pixel is darkened by the first shadow drawn over it and by no other, until the stencil is cleared (**hypothesis**: once a frame).

## The models' effects

Read 2026-11-02 (#1309) off `Sacred.exe` and the converted files; **confirmed** by code unless marked otherwise.

- The object's draw (`0x44B230`) sets the renderer's texture override (`cGranny +0x34`) to the type's `+0x08` texture where the type has one and none is set, draws through `0x44A9D0`, and clears it after.
  `0x44A8C0` chooses the renderer by the type: one whose flags (the first word of its record of `items.pak`) have the bit `0x100000` (`0x426450`; `subclass` bit 4) is drawn by `cGranny::renderComplex` (`0x406510`) while `DETAILLEVEL` (`0x182EE78`, 2 in the GOG settings) is not 0, every other by `cGranny::render` (`0x405C90`); at `DETAILLEVEL` 0 the flagged types too are drawn by `render`, their effect left out.
- `renderComplex` reads the type's effect texture, the word `+0x66` (`0x4262D0`), and its kind 1 to 9, the byte `+0x6E` (`0x426520`); so the effect is the type's, not its model's, and a texture's name plays no part.
  It turns on the depth test, the culling and the blending and turns the alpha test off, then walks Granny's rendering states and draws each in up to three passes that the kind chooses: the effect pass (the `+0x66` texture moved by a texture matrix, unlit, the depth write off, blended by its alpha), the glow pass (`FX_MULTITEXGLOW01` moved by a second matrix, unlit, added `ONE`, `ONE`) and the main pass (the override texture, lit, the depth written, blended by its alpha without the alpha test).
- The clock of the matrices is the frame's real time, `t` the performance counter in milliseconds modulo 65,536 (`0x406592`), the same for every object; a matrix moves the texture's coordinates by its third row, `u + _31`, `v + _32`.
- **The kind 9**, the Seraphim's wings 3111 alone (`SeraWings05`, the effect texture 8636 `serawings05fx01`): the main pass draws every rendering state but the first with the type's texture 8635; a second walk then draws the first state alone with 8636, unlit, white, the depth tested and not written, added to the frame (`ONE`, `ONE`, `0x406EB1`), so the type's texture never reaches it.
  Its matrix (`0x4068B9`): `r = t mod 2048`; for `r` under 512, `_31 = r / 128` and `_32 = (r / 512)^1.3`; for `r` from 512 to 1,023, `_31 = r / 128` and `_32 = ((r − 512) / 512)^1.3`; else none.
  So in each cycle of 2,048 ms the texture runs along `v` twice, each run 512 ms long and speeding up, and stands for 1,024 ms; along `u`, a width each 128 ms, which hardly shows on a texture nearly even along `u`.
- The first rendering state is the first pass of the model's file (**hypothesis**, strong): `SeraWings05.GRN`'s first pass is the material of the effect texture, two mirrored tubes skinned to the bone `blitz_framework` at the wings' join, `v` 0.016 at the tubes' far ends to 0.657 at the join.
  The texture 8636 is black above its row 160 and holds one soft band, lavender at its core (about `v` 0.85) and violet to dark blue at its edges: as `_32` grows, the band enters the tubes at the join and runs out along them to their ends, a stripe about a fifth of their length.
- The other kinds (the jump table `0x406FC0`; 57 types carry the flag):

| Kind | Types | Passes and motion |
|---|---|---|
| 1 | 48: the Seraphim's 3809 and her gloves, boots, shoulders, belts, wings 01 and windweavers, the Vampiress's `vlady_spa_2` pieces, the zombies 34, 119–123, 711, 712, the spider 601, the element stones 6008–6017 | effect pass with `_31 = _32 = t / 16384`; glow pass with `_31 = _32 = t / 32767`; main pass over them |
| 2 | 4017 `SeraShield02` | effect with `_32 = t / 2048`, main |
| 3 | 4018 `SeraWings04` | effect with `_31 = t / 2048`, main |
| 4 | 1747 `BOW_ELB01` | effect with `_31 = t / 1024`, main |
| 5 | 53, 94 `BIG_DEMON`, 184 `AnducarGarde` | `render`, then the whole model twice with the effect texture added (`0x40D310`), once white and once grey `(sin(t·π/1024) + 1) · 127.5`: a pulse of 2,048 ms |
| 6 | 905 `FX_FLYINGSKULL` | effect with `_32 = t / 1024` alone |
| 7 | none | as 5 without the `render` |
| 8 | 135 `SERAPHIM_holo` | effect with two fresh `rand() / 32767` offsets at every draw and half its alpha, then the main pass added |
| 9 | 3111 `SeraWings05` | above |

The choice of the hero's figures (`0x6F1B30`) draws its figures' flagged items by `renderComplex` too, without the `DETAILLEVEL` test; an orthographic item draw (`0x40F270`) has the kinds 1 to 4 alone.
Nothing else glows for the wings: no FX object (`0x451300` traced 2026-10-28), no light, and no glitter while worn.

## The visions

Read 2026-11-10 for #2519 by a subagent (**confirmed** by code unless marked otherwise; **seen** in the original at the Daemon's cemetery gate).

- The creature's constructor marks the types 223 (Valor's ghost) and 707 (`TYPE_NPC_DAEMONIN_VISION`, the Daemon's quest giver at the gate, of her own model `Daemonia.grn`) with the object's `+0x14` bit `0x200000` (`0x52E2E2`–`0x52E2F0`), and `CreateNPC`'s tag `0xA1` marks any creature so (`0x48290F`, `0x4836BE`; the Underworld's quests 11 and 56 alone use it); **seen**: the vision's `+0x14` `0x4288000`.
- The model's draw (`0x44A9D0`, the table `0x44B1B8`) takes such an object out of its overlay state: the types 223, 322, 224 and 707 into the state 22, which calls `0x40C320` with the style `6 | (modes >> 3 & 1)`, the others into 23, the style 8; the plain model's pass (`0x44A8C0`) is never drawn, so the figure is that pass alone, the background through its dark parts (**seen**).
- The modes word (`0xAB73DC`, the scripts' interpreter's `+0xA45C`) takes its bit 8 from `InfoPlayer` 23, 24 and 25 (the intros, `0x4AEA16`, `0x4AE9F3`, `0x4AE9D0`), and every start script runs `InfoPlayer` 23 under the test `0xA0`, true in a game of one player: such a game draws the style 7 (**seen**: the bit `0x8` at the draw).
- `0x40C320`: the depth tested and written (`LESSEQUAL`), no light, the faces culled counter-clockwise, added (`ONE`, `ONE`), the alpha test on, linear filtering.
  The stage 0 is the model's own texture times the diffuse (`MODULATE`); the stage 1 the style's texture times twice that (`MODULATE2X`): 6 `fx_multiTexBase02` (blue), 7 `fx_multiTexBase06` (gold, `texture.pak` record 199), 8 `fx_multiTexBase03` (red), each 128 × 128 of opaque cells; its coordinates the vertices' normals' x and y (the strided draw of the format `0x252` hands the normals' pointer and stride to the texture set 1, `0x40C6B7`), so that the cells slide over the figure with its surface's direction.
  The diffuse is one grey for every vertex, `⌊127.9·(1.75 − 0.25·sin(0.004·t))⌋` of 255, `t` the real time in milliseconds (`0x6413E0`; the floats `0x8907BC`–`0x8907C8`): 191 to 255 about every 1.57 s, every vision's alike; unlit, she is as bright by night.
- **Seen**: a golden-yellow, translucent figure of the Daemon with bright cells' lines, the "?!" over her, her brightness changing between frames while the world was paused.
- Not read: the stats' recomputation's reading of the bit (`0x57B68D`) and an AI's test (`0x5409AC`), the alpha test's reference.

`opensacred-game` draws them so since #2519 (`skins::Overlay::Vision`, `models::Pass::Vision`, `opensacred_rules::spells::{VISIONS, vision, vision_grey}`): a creature of the types 223 and 707 wears the look for good, each of its meshes in a material of its own texture in the grey of the real clock, times twice the vision's texture taken by its normals in the model's frame as the water form's (`standard.wesl`), in the frame's sRGB values, unlit, added, its depth written; **seen** 2026-11-10, the Daemon's vision at the gate a golden translucent figure with its cells, as the original's.
Not followed: `CreateNPC`'s and `SetNPCState`'s tag `0xA1`.
The visions' bodies end at once in blue-white sparks since #2514 (docs/re/fight.md, "The bodies' end").

## What the game does with it

`opensacred-game` orders the sprites through the depth buffer instead (docs/decisions/drawing.md, "Sprites ordered through the depth buffer"): each lies at the depth of its cell's centre at height 0, whatever the cell's height, so all the sprites of one cell, and those of the cells of one diagonal, have the same depth.
Its sprite shader brings each one forward by a little in the order above: by its cell's distance along the screen's right, which grows with `cx` along a diagonal, and by its place in the cell's chain (`statics::SpriteMaterial`).
Before, the centre lay at the cell's height, 0.46 units forward per pixel against 0.15 from a cell to the next of its diagonal: a sprite on a cell a third of a pixel higher than the next came over that cell's sprites, as the chapel's back right wall (5 pixels high) over the cabinet against it at the start (#112).

An upper storey's floor (a type of bit `0x200`, #120) takes at each corner of its pieces the place of the ground point under the floor's point there, at its plane's height, 37 units further back (`statics::Spot`): what stands anywhere on a cell of the floor, at its cell's place, lies in front of the floor under it, and the floor's points further up the screen lie further back, as a plane through the depth buffer has them.
The sprites, miniatures and shadows of the floor's part that the original draws after the part's first floor stand on it (`statics::Standing`, #143): over the cells of the part, the sprites' shader keeps each no further back than the floor's plane brought forward by half a unit to 3 units, a little further for each in their order, and writes that depth (`fragment_on_floor`).
They lie over the floor whole, as the original draws them, and keep their order among themselves; before, the floor's plane cut them along horizontal lines on the screen, more than half a cell below their cells' points on the floor.
The models on the floor lie in front of them over their own cells, and the models under the floor's plane, the hero on the stairs from floor 1, behind them, as the floor hides them in the original.
A model standing on an upper storey's floor (the hero by its level, an NPC or an object by its layer, on a floor above floor 1, `models::place`, #146) lies no further back than the floor's plane where each of its vertices shows: the models' shader takes the plane of its building's part's floor (`statics::Floor`), brought forward to the model's place among the quads standing on the floor, between those the original draws before it and those it draws after, with a five-hundredth of the vertex's own depth (`models.wesl`).
Before, a vertex on the floor more than 37.5 units of depth in front of its cell's centre, 4.5 past the cell's front corner, lay behind the plane, which hid it; no frame has shown such a model yet.
The cabinet at the front edge of the floor of the house of sector 52,39 is not one (#144): the strip at its bottom lies under the sprite of the front wall that stays without the roof, which the original draws after the cabinet too.
Past the cells of the part, a wall that reaches from the floor down to the ground keeps its place, behind what stands on the ground in front of the building.
The hero, the NPCs and the objects on a building's floor stand at the floor's height (`terrain::Heights::lift`) above the second heights of its grid (`terrain::Heights::standing`, #141); before #120, at the ground's, 192 pixels below the floor of the house of sector 52,39, where the floor's carpet covered the hero's legs.

It draws a sprite in two passes (docs/decisions/drawing.md, "Sprites' opaque and translucent texels apart"): its opaque texels through the depth buffer, then the others blended by their alpha over what lies behind them, after the original's alpha test for the sprites and without it for the miniatures, each texel's colour weighted by its alpha as the shader filters them.
Bevy draws the blended things after the opaque ones, back to front by their meshes' centres; they go in layers in the original's order instead (`blended`): the miniatures of the liquid cells, the clear waters' second layer, the liquids' surfaces, the underlays of list 0, the statics' shadows of list 1, the underlays of list 2, the characters' shadows and the shadows drawn with their sprites, the sprites of lists 3 and 4 and the models' translucent texels, the lamps' glows, and a mesh's quads go far to near.

The statics' shadows (`statics`, #100) are quads of the shadow atlas's cells drawn by the sprites' shader in its blended pass, all their texels translucent, through two materials that every sector shares and whose alpha the night takes down to 0 (`daynight::shadows`).
List 1 lies 3,000 units back, between list 0 (4,000) and list 2 (2,000, 3,000 before list 1 came): a thousand units, 30 diagonals, from either, so that a shadow lies over every underlay of list 0 and under every one of list 2 whose cell is within 30 diagonals of its own.
A shadow drawn with its sprite takes the sprite's place, its list, its building's part, half a place behind it, and the characters' shadows' layer.
The sprites' shader samples a cell's own texels alone, where the original's linear filter takes in half a texel of the neighbouring cells at the edges.
Before, the sprites' edges went through alpha to coverage: the texels of alpha around a half came out dithered, those under it dropped, and a sprite's edge over another sprite showed as a dotted line (#35).
The blending takes the frame's sRGB values as they are, as the original's does (`srgb`, #109); before, an sRGB target blended in linear light, and a translucent dark texel darkened less.

The models take the same depth (`models`, docs/decisions/drawing.md, "Models among the sprites by cell depth"): every vertex of a model lies at the depth of the centre of the cell its root stands in, at height 0 as the sprites', its place after the cell's statics, plus a hundredth of its own depth from that centre at the cell's height, which keeps the model's parts in order among themselves.
A sprite of another diagonal, 33 units of depth away, is then wholly in front of a model or wholly behind it, as in the original; the sprites of the model's own diagonal within a few cells, a tenth of a unit apart, can still cross it.
Before, every sprite whose plane crossed a model cut it (#48).
Among themselves the models keep their true depths, as the original's depth buffer, which holds theirs alone, orders them (#998): the camera's depth prepass draws the models alone, their opaque texels at their true depths squeezed into the farthest sixteenth of the range, behind all that the main pass draws (`prepass.wesl`, `nearest.wesl`), and a model's fragment takes only the samples over which the prepass holds no model's texel nearer by more than half a unit (`standard.wesl`).
So over each sample the nearest model alone takes its place among the sprites, which is what the original's depth test leaves whether a sprite is drawn between two models or not: the sprite covers a model drawn before it, and a model drawn after it shows only where it is the nearer.
Before, a model of a later diagonal covered the models of earlier ones wherever they met on the screen: the gate of the barn of sector 53,40, whose logical cell lies two diagonals before its doorway, drew its rail across the NPCs who stood in front of the wall beside it.
The ground and the sprites stay out of the prepass; Bevy's own material, the bolts' and the souls' spheres, writes its true depth there, beyond the models' part, which the test passes over.
A mesh whose material has translucent texels, of alpha under 255 in its texture's image (read as the image loads) or in its colour, is drawn twice, as a sprite is (`models::Pass`, docs/decisions/drawing.md, "Models' translucent meshes drawn twice"): its opaque texels through the depth buffer, then, by a copy of the mesh (`models::Fringe`), those of alpha 65 to 254 blended by their alpha in the layer of lists 3 and 4, their depth tested and written as the original's.
Any other mesh is drawn once, its texels of alpha 64 or less dropped, the others opaque.
Before, every texel that the alpha test kept was drawn opaque, so the ghosts showed as solid figures and the insects' and the dragons' wings as solid membranes (#519).
A model's meshes are hidden from the frame its scene spawns in until they have their model material (`models::Veiled`, #1628), the frame after, or later where their images still load; a model whose holder is not in the order (the items of the choice's figures) is shown at once, as it was.
Before, they were drawn in that frame with the glTF's own material, lit by the scene's lights and at their true depths, and a worn item's on the item's own skeleton: the Seraphim's sword came into her hand dark for a frame at a weapon set's change.
A script's object takes its own cell's place, its logical position's, wherever its model stands (`models::place`, #459), and its model stands where the client's does, `0x20` at height 0 taken along the line of sight to the object's height (`npcs::model_position`); the limit of 200 or 1,000 is not followed (docs/decisions/drawing.md, "Script objects' models without the offset limit").
Before, the model took the place of the cell its root stood in, so that the leaf of the door of (272, 2379) on its upper storey, whose root stands in (271, 2379), lay a diagonal behind the back wall's sprites of its own cell and showed cut by them, and nearly hidden open; and `0x20` was taken on the ground's heights there, which drew 99 of the tree's doors off the original's, 95 of them higher by up to 46 pixels at zoom 1, the `Door_Villa02` of (3184, 2708) 34 pixels up with a gap under its leaf.
A model's shadow (`shadows`, #101) is a copy of each of the model's meshes, skinned to the same joints, that the shadows' shader flattens onto the plane at the root's height: a point moves 0.037 units along X and 0.670 against Z per unit of its height above the plane, which on the screen is the original's lean (`shadows.wesl`).
It takes its model's anchor and squeeze, in the shadows' layer: over what the original draws before the model, a floor or the stairs it stands on, and under the model and what it draws after.
For the stencil, its fragments take the depth of the plane in their pixel's row, computed from the row alone, a unit of the model's own depth behind, and write it, drawn only where they are nearer than what lies there: a shadow's own triangles give a pixel the same depth to the bit, and darken it once (docs/decisions/drawing.md, "Models' shadows by depth, not stencil").
It casts from the hero, from every NPC whose creature has no flag `0x10`, from the scripts' objects of category 4 and since #2380 from the arrows (`arrows`), each flattened onto the plane at its own height; an NPC's weapons cast none, as the client flattens the creature's own model alone.
Before, a soft ellipse lay under each character, and the objects had none (#102).

### The shadow dots

Read 2026-11-09 for #2377 (**confirmed** by code unless marked).
At `DETAILLEVEL` 0 `cObject3D`'s draw (`0x44B230`, at `0x44B776`) draws, in the shadow's place, the dots of `0x407960` for the same creatures: the chests' draw (`0x5C97B0`) and the arrows' do not call it.
Its size is the type's size for the arrows' hits (`items.pak +0x14`, 50 for 0, `0x428CE0`).

- It binds `SHADOWDOT.TGA` (white, its alpha falling from the middle) and draws quads of four `D3DLVERTEX`es (FVF `0x1E2`) as triangle strips, their colour `0x50000000`: black at an alpha of 80.
- The first quad has a half side of 0.6 times the size and lies at the root bone's (`__Root`) state, in the frame of the model's matrix with its translation's first two coordinates taken from the bone, on the plane `z = 0`.
- The offsets of `Bip01 R Foot` and `Bip01 L Foot` from the root (the string table `0x401000`'s entries `0x25` and `0x24`) stay 0 where the model lacks the bone.
- Two quads of two thirds the half side lie halfway from the root to each foot, and two of two thirds of that at the feet.
- So five dots lie under a creature, darkest where they overlap, about its root and its feet.

`opensacred-game` lays them since #2377 (`shadow_dots`): a mesh of the five quads for each caster but the scripts' objects, at the root's height about the bones' points on the ground, of half sides in models' units, in the effects' material laid over by alpha, just behind its model in the order (`fx::Orders::behind`), only at the detail 0 and hidden with the model.
**Seen** 2026-11-09 beside the original at its low detail: the Seraphim and a zombie 33 at the start, each over a soft dark patch about its feet, alike in size and darkness.

The kind 9 of the models' effects follows since #1309 (`model_effects`; "The models' effects" above): the types' effects are read from `items.pak` (`Item::model_effect`) by their models' files, and a held or worn model of the kind 9, the wings 3111, takes for its first glTF primitive the effect texture, white, unlit and added to the frame in the objects' layer, its depth tested and not written (`models::Pass::Added`), the other primitives the type's texture as before; the texture's offset runs on the real-time clock by the kind's matrix.
So the band of light runs from the wings' join out along the two tubes twice and then rests for a second, where before the tubes were drawn lit in the wings' texture, the original's look at `DETAILLEVEL` 0 alone; seen in the game, not yet beside the original's.
By night the band is darkened with the model (**hypothesis**: the original draws it with the model, before its night's pass).
Since #1834 the kind 1 too, the 48 types of the effect and the glow under every texel (the zombies, the spider 601, the element stones, the Seraphim's 3809 and her gloves, boots, shoulders, belts, wings 01 and windweavers, the Vampiress's `vlady_spa_2` pieces): a model's meshes are drawn in one opaque pass (`models::Pass::Effect`) whose shader lays each texel of the type's texture, lit, over the effect's texture and the glow's `FX_MULTITEXGLOW01` added to it, both unlit and moved by the kind's offsets on the real-time clock, by the texel's alpha, in the frame's sRGB values: the original's three passes laid in one, each texel of the effect's opaque textures covering what lies behind it.
`FX_OPAQUE` stays the texture of a type drawn so, the Seraphim's 3809, whose effect then shows whole, where before #1834 such an item took its model's own material.
Seen 2026-11-02 beside the original: a zombie (34) made at the Seraphim's start shows the blue effect through its texture's holes in both, where it showed its texture's pale texels before.
Since #1848 the kinds 2 to 4 alike, without the glow (the shield 4017's effect along `v` by `t / 2048`, the wings 4018's along `u` by `t / 2048`, the bow 1747's along `u` by `t / 1024`; `model_effects::offsets`); not seen in the game, as none of the three is put on by a command.
Since #1850 the kind 5, the big demons 53 and 94 and Anducar's guard 184: each mesh of the model takes a copy of itself, the same mesh skinned to the same joints, of the effect texture unlit and added to the frame in the objects' layer (`models::Pulse`, `models::Pass::Added`), its colour the original's two added passes laid in one, the white's 1 and the grey's `(sin(t·π/1024) + 1)/2` on the real-time clock, from 1 to 2 over 2,048 ms (`model_effects::pulse`; the colour taken in linear values, an **estimate** of the original's sum in its frame's values).
Seen in the game: a big demon's lava veins brighten and fade by the pulse; not yet beside the original's, whose start failed at that time.
Since #2172 its colour is the frame's value in linear light (`fx::linear`), so that the frame takes the texels' values times it, and its copies are drawn after the night's pass (`iso::UNTINTED`), as `0x40D310` draws the two passes unlit in their own colours, which no light of the night reaches.
**Seen** 2026-11-06 beside the original, a big demon 53 made beside the Seraphim at her start, frames about a quarter and half a second apart: at noon its lava-coloured pixels alike in both (their mean about (205–235, 140–185, 72–118), 13 to 35 % of them near white); at 22:00 the original's as bright (a mean of (199–219, 126–150, 54–65), 9 to 22 % near white), where the game's had been darkened with the frame (a mean of (170–177, 119–126, 58–63), none near white), and are alike now ((197–217, 130–158, 62–77), 8 to 26 %).
The kind 7 has no type.
Since #1850 the kind 8, `SERAPHIM_holo` 135, the Seraphim's hologram that the class trees of the Seraphim, the Vampiress and the Dwarf make in their quest 50 (`DlgHologram50`): each mesh is drawn once over its effect texture (`models::Pass::Hologram`), the effect blended at half its alpha at two fresh offsets `rand() / 32767` every frame (`model_effects::hologram`) and the model's own texture, lit and whole whatever its alpha, added over it, the two laid as one premultiplied colour in the frame's sRGB values, its depth not written.
Seen 2026-11-03 beside the original (`spawn 135` beside the Seraphim at 3221,2536): a translucent blue figure that flickers in both, ours a little paler; before, it stood solid blue.
The kind 6, the flying skull 905 (`FX_FLYINGSKULL`), names no model of its own: its object is the particle system `flyingskull` (docs/re/effects.md), which `renderComplex` has nothing to draw over, so the game draws no effect for it.
Since #1875 the effects are read at the start (`model_effects::Table`), and every model finds its own by its model's file, a worn or held item's, a creature's, else its glTF's (`npcs::retexture`): an item lying on the ground takes every kind, as the original's object draw, and the choice of the hero's figures theirs, as `0x6F1B30` draws them; before, only a creature and what it wore or held had an effect, and only once a game had loaded.
The interface's items (the backpack's grid, the equipment's slots, the cursor's item, the tooltips, the weapon sets, the smith's window) take the kinds 1 to 4 alone, as the orthographic item draw (`0x40F270`) has them.
Since #2352 a model of the world that spawns at the options' detail 0 takes no effect, drawn plain as `cGranny::render` draws it (`npcs::retexture`), and since #2377 an item of the interface too, as the orthographic item draw (`0x40E9C0`) calls `0x40F270` for a flagged type only while `DETAILLEVEL` is not 0 (`0x40EA06`) and draws it plain otherwise; the choice's figures keep theirs.
**Seen** 2026-11-09 beside the original at its low detail: a zombie 33 at the start drawn in its texture alone, without the blue effect, in both.
Not followed: a change of the detail in play reaches the models as they spawn, where the original's draw reads it at every frame.
A model drawn apart from the world, by a camera whose frame takes linear light (the interface's, the figures'), lies over its effect through `model_effects::ApartMaterial`: the shader of `models::Pass::Effect` (`standard.wesl`, `LINEAR`), laying the effect in sRGB values under the texel and turning the result back into linear light; a mesh waits for its effect's textures, drawn without it meanwhile.
Seen 2026-11-03 beside the original: the Seraphim's figure shows the blue effect on her wings 4006, gloves and belt; the wings 3111 on the ground run their band along dark tubes; the wings 4006 in the backpack show their effect, those 3111 in the equipment's slot lie still in their texture.

The miniatures of list 0 in the liquid cells lie between the ground and the liquids' surfaces, 4,600 units back against the surfaces' 4,450 (4,300 before the surfaces lay flat at height 0, #103) and list 0's 4,000, in the order of list 0 among themselves (`statics::UNDER_LIQUIDS_BACK`, #96).
A sprite is a quad in the plane of the screen while the bed and the surface slope away under it, 1.37 units nearer per pixel down the screen and 1.83 per pixel of height: the margins hold for the tallest of these miniatures, from 68 pixels above their cell's centre to 139 below, over a bed up to 61 pixels off their cell's height, and for the underlays of list 0 beside the water, up to 124 pixels below their cell's centre (counted on the sector files 2026-10-18).
The anchors at height 0 (#112) take a quad 0.46 units back per pixel of its cell's height: the liquid cells holding these miniatures lie from 67.5 pixels below height 0 to 25 above, and over every cell whose diamond's box meets one of them the miniatures stay at least 210 units in front of the ground and 155 behind the surfaces; of the underlays of list 0 beside the water, none whose margin over a surface held before loses it (counted on the sector files 2026-10-18 with the cells' boxes, which overstate the overlaps).
With the surfaces flat at height 0 and 4,450 back (#103), the miniatures stay at least 73 units behind them, and no underlay of list 0 whose margin held over the surfaces at their beds' heights loses it, while 331 pairs of an underlay and a cell that failed the boxes' count before now pass it (the same count, 2026-10-19).

The waters' crabs and fish and the items lying on the ground (`models::Sinks`) lie under the water while they stand in a liquid cell (#983, `models::Sunk`): their models' places as far back as the liquid cells' miniatures, under the surfaces, and their translucent texels and their shadows in the layer under the liquids (`blended::Layer::UnderLiquids`).
Before, they were drawn in list 3, over the surfaces, so that a shoal showed whole and opaque on top of the water with its shadow on the bed under it.

Not followed yet: which types the tests send to lists 0 and 4 (the game draws every model in list 3), the scripts' objects of an item's type in a liquid cell, the flag `0x100000` and the virtual function's answer of the test above, the exception of liquid material 13 (sectors 29,72, 30,72 and 55,20; the building at (3563, 1336) lies in 55,20), and the trees' fade about the creatures other than the hero (`k` above 0).
