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


The lists are drawn after the last row: `0x62E410` and `0x62FF60`, called from the same function after its row loop, draw them.
So every object of an earlier list lies under every object of a later list, whatever their rows.

## Buildings: masks and layers

A cell whose flags (`+0x1E`) have bit 0 set belongs to a building: for such a cell `0x62AE90` asks `0x6354D0` for the building's object, which follows the cell's two signed offsets at `+0x1C` and `+0x1D` (the "offsets of the parent object" of docs/02 §5.2) to the cell that holds it.
The building's trigger (the object's `+0x27`) selects a 16-byte record of a table of the world (`+0x268` of the object at `+0x04` of the view), whose word at `+0x0A` is a mask of one bit, the building's current state.

- A static with flag `0x8` in such a cell is appended only when its mask (`+0x2B`, one bit of 1 to 16) equals that state; statics without the flag, and those of cells outside buildings, are appended whatever their mask.
- The world's dynamic objects of such a cell are taken from the layer that `0x416570` gives, the index of the lowest bit of the state (bit 0, layer 0; bit 1, layer 1; …), when it is not 0 or the building's object has flag `0x400`; otherwise from layer 0, like the cells outside buildings.

**Hypothesis**: the state is the building's exterior (with its roof) while the hero is outside and switches when the hero enters, which the client signals when the hero's cell gains or loses bit 1 (docs/02 §5.2): this is how the original hides a roof and shows the rooms and their furniture (#3).
Where the state's table is filled and changed is not followed.

**Hypothesis**: a start script's position `(x, y, z)` puts its object in layer `z`: of its 2,096 placements at a cell, 653 objects and one NPC have a layer of 1 to 4, and 649 of them stand on cells of a building (bit 0), against 45 of the 1,442 of layer 0.
While the hero is outside, the original draws layer 0 of every building, so the furniture of layers 1 and above does not show; `opensacred-game` keeps those placements hidden until it has the buildings' states.

The statics agree (**confirmed** on the sector files): in building cells, those flagged `0x8` of mask 1 are the buildings' shells (the 53 parts of the start's monastery chapel, by their editor names), those of masks 2, 4, 8 and 16 their rooms' furniture (candles, benches, plates, beds in the chapel): 9,224 and 48,942 statics over the world.
**Hypothesis**: a building's state is 1 while the hero is outside.
The parts of mask 2 include the rooms cut open: the pub of sector 50,40 has its shell as `TH_PUB_0_*` (mask 1) and its ground floor, floor boards, bar and inner walls as `TH_PUB_1_*` (mask 2, by their editor names), and every cell of a building points to one anchor cell through its offsets (the pub's 12 × 11 cells to the cell of `TH_PUB_1_BODEN`, its floor).

`opensacred-game` keeps a state per building, by its anchor cell (`buildings`): 2 for the building whose cells the hero stands on, which its walls let it reach only from inside, else 1.
The parts flagged `0x8` show while the state is their mask, and the start script's objects of a building's cells while the state shows their layer: inside, the roof and the shell's walls go, the ground floor with its furniture comes (#3).
The client's own choice of the state when the hero enters is not read yet; not all buildings flag their rooms' cells (the monastery does, bit 1; the pub does not), so the room bit is not the criterion here.

## Depth

`0x62FF60` draws a list's sprites piece by piece.
For a sprite whose type (the `items.pak` record's `+0x00` as a u32) has bit `0x200`, it switches two render states of the engine's state table (`0x100` and `0x1`, the depth write and the depth test: `0x643470` sets `D3DRS_ZWRITEENABLE` and `D3DRS_ZENABLE` for them, confirmed 2026-10-18) on and gives each of the quad's four vertices the depth that `0x6288A0` computes from the vertex's screen y and a height: the static's byte at `+0x33` (Static.PAK) times 28.
`0x6288A0` returns the depth of the point at that screen y lying that high above the ground (docs/02 §5.3), so the quad lies in a horizontal plane at that height: what stands below it is hidden, what stands above shows.
For every other sprite the two states are off and the depth is 0: it is drawn over what is there, in the order of its list.

In the game data 47 types carry bit `0x200`, with 220 drawn objects in the whole world; by their editor names they are the floors of towers' and houses' upper storeys.

The other states of that table (`0x643470`, the engine's switch of Direct3D 8 states): `8` and `0x10` make the source and the destination blend factors `ONE` (else `SRCALPHA` and `INVSRCALPHA`), `0x40` the depth function `GREATER` (else `LESSEQUAL`; only `cGranny::renderShadowFake`, `0x4073B0`, sets it, for a second pass of a model's fake shadow), `0x800` the linear filter of both texture stages (else point).

## The trees about the heroes

Read 2026-10-18.
As `0x62FF60` draws a sprite whose type has bit `0x10` (4,544 types: the 4,533 of type 25, the trees, and 11 floors of type 29), it fades the vertices of each piece that lie near the points of a list at `+0x96B3C` of the renderer (`+0x96FEC` of them, records of 12 bytes: the point's x and y on the screen as two i16, a counter that each faded vertex raises, and a float `k`).
For a vertex within `R / zoom` of a point across and down, `d` is its distance to the point (an integer square root of `dx² + dy²`), `t = min(d · zoom / R, 1)`, and the vertex's alpha becomes `G · (1 − k) + (255 − G) · t`: `G · (1 − k)` at the point, 255 at `R` and beyond, so a tree goes transparent about the point and stays whole away from it.
`R` and `G` are the floats at `0xAD4DF0` and `0xAD4DF4`, the balance keys `TransSize` and `TransValu`: `Balance.bin` holds them, 190.0 and 50.0 (docs/02 §6.3), so a tree is at an alpha of 50 out of 255 at the hero and whole 190 pixels from it at zoom 1.
The list is filled anew each frame as `0x62E410` passes the creatures, up to 100: the local player's hero (an object of id up to 16 that is the local player's, `0x7D84A0`), the creatures it owns (the owner at `+0x1EC`) and its companions (`+0x251` the player's, flag `0x104` at `+0x1F4`), each at the centre of its bounds on the screen, with `k` 0.

The game has no such fade: a hero behind a tree is hidden (#69).

## The models

The world's dynamic objects (the creatures, the start script's objects) are entries of the same draw lists.
After a cell's chain of statics, the "wobj" walk of `0x62AE90` takes the objects of the cell's layer (the layer of its building's state, above) and appends each, as an entry of kind 0 with the object's index, to list 3 (the vector at `+0x96AF4` of the view), or to list 0 or list 4 by tests of its type (`0x4280B0`, `0x4280D0`, `0x4280F0`, not followed).
`0x62FF60` walks the five lists in order (the vectors from `+0x96AD0`, 12 bytes apart) and draws an entry of kind 0 by calling the object's own draw (its virtual function at `+0x14`), in its place among the list's sprites.
So a model is drawn after every sprite of an earlier row and of an earlier cell of its row, and after its cell's statics; the sprites, without the depth test, cover a model drawn before them whole and lie whole under one drawn after them.
The depth state of a model's own draw is not read.

## What the game does with it

`opensacred-game` orders the sprites through the depth buffer instead (docs/decisions/2026-10-17-sprite-depth.md): each lies at the depth of its cell's centre, so all the sprites of one cell, and those of the cells of one diagonal at one height, have the same depth.
Its sprite shader brings each one forward by a little in the order above: by its cell's distance along the screen's right, which grows with `cx` along a diagonal, and by its place in the cell's chain (`statics::SpriteMaterial`).

The models take the same depth (`models`, docs/decisions/2026-10-18-models-among-sprites.md): every vertex of a model lies at the depth of the centre of the cell its root stands in, its place after the cell's statics, plus a hundredth of its own depth from that centre, which keeps the model's parts in order among themselves.
A sprite of another diagonal, 33 units of depth away, is then wholly in front of a model or wholly behind it, as in the original; the sprites of the model's own diagonal within a few cells, a tenth of a unit apart, can still cross it.
Before, every sprite whose plane crossed a model cut it (#48).

Not followed yet: which types the tests send to lists 0 and 4 (the game draws every model in list 3), and the trees' fade above.
`0x62E410` draws the miniatures (and the lamps' glows over them, docs/re/daynight.md).
