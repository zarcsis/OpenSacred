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

In a cell whose nature is a liquid (`+0x1F & 0xF0` of `0x90`, `WATERA`, or `0xA0`, `WATERB`, docs/02 §5.2), `0x62AE90` appends the miniatures that would go to list 0 (of a type with bit `0x4`) to a vector of their own instead (`0xAD7848`, of the lists' 24-byte entries), and with them the world's objects of the cell whose type passes `0x434070` (not followed), that lack flag `0x100000` at `+0x14` and whose virtual function at `+0x24` answers 0 to `0x2A`.
`0x62E410` draws that vector as `0x62FF60` draws a list, entry by entry with the same code for each kind, before the liquids' surfaces: those objects lie under the water.
An exception, by code (its meaning not followed): a cell whose liquid material is 13 sends its objects to the lists as a dry cell does while the building at the global cell (3563, 1336) has the state 2.
The material is the byte `+0xF7` (`WATERA`) or `+0xF8` (`WATERB`) of the sector's record of docs/re/daynight.md, at that record's place keyx `+0x2E0` and `+0x2E1`, the sector's two liquid slots (docs/02 §5.1).
Over the world 12,127 miniatures lie in liquid cells (12,024 of `WATERA`, 103 of `WATERB`), counted on the sector files 2026-10-18.

## Buildings: masks and layers

A cell whose flags (`+0x1E`) have bit 0 set belongs to a building: for such a cell `0x62AE90` asks `0x6354D0` for the building's object, which follows the cell's two signed offsets at `+0x1C` and `+0x1D` (the "offsets of the parent object" of docs/02 §5.2) to the cell that holds it.
The building's trigger (the object's `+0x27`) selects a 16-byte record of a table of the world (`+0x268` of the object at `+0x04` of the view), whose word at `+0x0A` is a mask of one bit, the building's current state.
The table holds the records of `Triggers.PAK` as the file has them, which `0x638570` loads (docs/02 §5.5): the word is 1 in every record of the file, and `cTrigger::setState` (`0x415F00`) and `cTrigger::resetState` (`0x416280`) set and clear its bits (read 2026-10-18).

- A static in such a cell is appended only when its mask (`+0x2B`, read as a word: 0 or one bit of 1 to 16) equals that state, if its type has bit `0x4` or `0x800000` (lists 0, 2 and 4) whatever its flags, or if it has flag `0x8` (list 3); the other statics of list 3, and the statics of cells outside buildings, are appended whatever their mask (`0x62BAD0`, read again 2026-10-18, #82).
  A mask of 0 equals no state: the 3,804 miniatures in the world's buildings' cells, all of mask 0 and without the flag, are never drawn (the slabs under the ends of the start's monastery chapel's stairs among them), and the 115 sprites of list 0 there without the flag, all of mask 1, only in the state 1 (counted on the sector files).
- The world's dynamic objects of such a cell are taken from the layer that `0x416570` gives, the index of the lowest bit of the state (bit 0, layer 0; bit 1, layer 1; …), when it is not 0 or the building's object has flag `0x400`; otherwise from layer 0, like the cells outside buildings.

The state is the building's exterior (with its roof) while the hero is outside, and the client changes it as the local player's hero walks in and out (**confirmed** by code 2026-10-18, #99; docs/re/movement.md, "Changing layers"): to 2 as the hero steps onto an entrance of the building and goes onto its inside, back to 1 as it steps onto an exit and comes out onto the ground, to `1 << n` as it climbs to the building's layer `n`.
This is how the original hides a roof and shows the rooms and their furniture (#3): at the chapel of the start, as the hero passes its doorway, not on its stairs.
The cells' room bit (bit 1) does not change the state: the client's event as the hero's cell gains or loses it (`0x594011`, docs/02 §5.2) is something else, not followed.

**Hypothesis**: a start script's position `(x, y, z)` puts its object in layer `z`: of its 2,096 placements at a cell, 653 objects and one NPC have a layer of 1 to 4, and 649 of them stand on cells of a building (bit 0), against 45 of the 1,442 of layer 0.
While the hero is outside, the original draws layer 0 of every building, so the furniture of layers 1 and above does not show.

The statics agree (**confirmed** on the sector files): in building cells, those flagged `0x8` of mask 1 are the buildings' shells (the 53 parts of the start's monastery chapel, by their editor names), those of masks 2, 4, 8 and 16 their rooms' furniture (candles, benches, plates, beds in the chapel): 9,224 and 48,942 statics over the world.
A building's state is 1 while the hero is outside: the file's value, and the exits' (above).
The parts of mask 2 include the rooms cut open: the pub of sector 50,40 has its shell as `TH_PUB_0_*` (mask 1) and its ground floor, floor boards, bar and inner walls as `TH_PUB_1_*` (mask 2, by their editor names), and every cell of a building points to one anchor cell through its offsets (the pub's 12 × 11 cells to the cell of `TH_PUB_1_BODEN`, its floor).

`opensacred-game` keeps a state per building, by its anchor cell (`buildings`): `1 << n` for the building on whose floor `n` the hero walks, 2 on its ground floor, which it goes onto at the building's entrance and leaves at an exit as the client does, and the upper floors up and down its stairs (docs/re/movement.md), else 1.
The parts (the statics above that the state chooses) show while the state is their mask, and the start script's objects of a building's cells while the state shows their layer: inside, the roof and the shell's walls go, the ground floor with its furniture comes (#3).
The start script's objects and NPCs of layer 0 that stand on a building's cells go while the hero is inside it, as the original draws its cells' objects of layer 1 then.
An NPC that a script sends somewhere takes the layer of the level it walks on, onto the building's inside at its entrance and out at an exit as the hero does (`0x5FCCB0`, docs/re/movement.md), and keeps it where it stops (#138).
Before, it stayed in layer 0 after going in: quest 1's novice, at the chapel's benches, showed through the chapel's wall while the hero stood outside.

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
A miniature (`0x630F2C`…) is blended by its alpha too, without the test, which the sprite's path leaves off; it is filtered linearly while the float at `+0xB60` of the renderer is over 1.2, else point (the float not followed).

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

`opensacred-game` fades them so (#122): the sprites of a type of bit `0x10` that its order puts after the hero's place take that alpha by their distance to the hero's point, 60 pixels above its feet, per point rather than per corner of their pieces (docs/decisions/2026-10-19-tree-fade.md), before the sprites' alpha test, which drops the faded texels within 13 pixels of the point (`statics::Fade`, `sprites.wesl`).
Type 25 holds the walls of the buildings' shells too (`KLOSTER_KAPELLE01_0U1_*`), which fade the same way.
For a hero behind the other sprites the client does nothing (read 2026-10-19, #69, **confirmed** by code): the sprites' path of `0x62FF60` (and of `0x62E410`, the same code) tests the hero's place nowhere else, and reads the points at `+0x96B3C` only for a type of bit `0x10`.
A sprite of any other type, a building's part of type 9 such as the chapel's tower, is neither faded nor skipped nor moved in the order for the hero: drawn after the hero, it hides it.
No model is drawn as a silhouette or an outline: none of the 24 modes of a model's draw (`0x44A9D0`) sets the depth function `GREATER` or the stencil, which the shadows alone use (`0x4073B0`, `0x406FF0`), and nothing after the lists draws the hero again; `0x41F7F0` draws there eight models that cross the map on straight paths at height 240, with their shadows (**hypothesis**: birds).
The types of bit `0x800000` (list 4) always cover the hero, and a building's levels show by its state alone (`0x62AE90`), so the game, which hides the hero behind such a sprite, does as the original.

## The models

The world's dynamic objects (the creatures, the start script's objects) are entries of the same draw lists.
After a cell's chain of statics, the "wobj" walk of `0x62AE90` takes the objects of the cell's layer (the layer of its building's state, above) and appends each, as an entry of kind 0 with the object's index, to list 3 (the vector at `+0x96AF4` of the view), or to list 0 or list 4 by tests of its type (`0x4280B0`, `0x4280D0`, `0x4280F0`, not followed), or in a liquid cell to the liquid cells' vector (above).
`0x62FF60` walks the five lists in order (the vectors from `+0x96AD0`, 12 bytes apart) and draws an entry of kind 0 by calling the object's own draw (its virtual function at `+0x14`), in its place among the list's sprites.
So a model is drawn after every sprite of an earlier row and of an earlier cell of its row, and after its cell's statics; the sprites, without the depth test, cover a model drawn before them whole and lie whole under one drawn after them.
The depth state of a model's own draw is not read.

## The models' shadows

Read off the client with Ghidra 2026-10-19 (#101), **confirmed** by code unless marked.

`cGranny::renderShadow` (`0x406FF0`) draws a model's shadow, right before the model, in the object's own draw: `cObject3D`'s (`0x44B230`, the virtual table at `0x890D10`) for an object of the categories 2, 3 and `0x20` (the creatures, `0x426610`) whose creature has no flag `0x10` (`NOSHADOW`, `0x426D30`), `cChest3D`'s (`0x5C97B0`) and `cArrow3D`'s (`0x5BAF00`).
The object factory (`0x451300`) makes an object of category 4 (`items.pak +0x2E`) a `cChest3D`: of the 1,990 objects of the Seraphim's start script, the chests, the barrels, the vases, the coffins, the cabinets, the book stands and the shrines.
The signposts are of category 15, `cSpecial3D`, whose draw is `cObject3D`'s, and cast none.
The size that the draws pass is the object's height (`+0xF0`, the third coordinate of its position).
They draw it when `DETAILLEVEL` is 1 or more (2 in the GOG `Settings.cfg`; the setting is stored at `0x814152`, and at 0 `0x407960` draws a few dots of `SHADOWDOT.TGA` about the bones instead) and the value at `+0x1FC` of the renderer is set (else `cGranny::renderShadowFake`, `0x4073B0`, draws a quad of `GRN_SHADETEXTURE03.TGA`, the texture at `+0xA4`).

The shadow's matrix is the identity with its third row `(0.5, −0.5, 0, 0)` and its fourth `(−s/2, s/2, s, 1)` for the size `s`, after the model's own matrix (`+0x0C`, when there is one) and before a mesh's own (`0x653B30` multiplies `A × B`, `A` applied first).
So a point `h` above the plane `z = s` goes to `(x + h/2, y − h/2, s)`: flattened onto the plane at the object's height, moved by half its height along `x` and against `y`.
**Hypothesis**: a deformed mesh's vertices come from Granny in the world's frame, without a matrix of their own, as the bounds test of the shadow (`0x4057D0`) takes the bones' state through the same matrix and the plane is at the object's height.
The frame is the models' (`0x6284E0` sets the view): the camera stands 1,200 units toward `+y` and 600 up from the point it looks at, looking down at atan(1/2), `z` up; `x` runs right on the screen and `y` down it, toward the viewer.
A creature's position becomes that frame's by the screen (`0x6239C0`): from its cell position to the screen's pixels, `48 (x − y)` and `24 (x + y)` over 53.67, then back through the view and `ortho(±267, ±200)`.
On the screen at zoom 1, the shadow of a point `h` model units high lies 0.96·h pixels right of its foot and 0.43·h up: leaning up and to the right, 24° above the horizontal, as the statics' shadows lean.

It draws every mesh with the model's deformed vertices (`GrannyLockSequenceForRendering` with `0x21`) through `DrawIndexedPrimitiveStrided` (FVF `0x142`), the vertices' colour opaque black (`0xFF000000`) and the texture at `+0x9C` of the `cGranny` object, `WEATHER_RAIN01.TGA` (`0x401030`), black at alpha 170 in all its 16 × 16 texels: a shadow leaves a third of the light, by day and by night.
The depth test is on and its write off, the culling on, the lighting and the alpha test off, the blending `SRCALPHA`, `INVSRCALPHA`.
The engine's state `0x10000` turns the stencil on with the reference 1 and the mask 1 (`0x643470`), over the device's `STENCILFUNC` `NOTEQUAL`, `STENCILPASS` `REPLACE` and `KEEP` otherwise (`0x643190`): a pixel is darkened by the first shadow drawn over it and by no other, until the stencil is cleared (**hypothesis**: once a frame).

## What the game does with it

`opensacred-game` orders the sprites through the depth buffer instead (docs/decisions/2026-10-17-sprite-depth.md): each lies at the depth of its cell's centre at height 0, whatever the cell's height, so all the sprites of one cell, and those of the cells of one diagonal, have the same depth.
Its sprite shader brings each one forward by a little in the order above: by its cell's distance along the screen's right, which grows with `cx` along a diagonal, and by its place in the cell's chain (`statics::SpriteMaterial`).
Before, the centre lay at the cell's height, 0.46 units forward per pixel against 0.15 from a cell to the next of its diagonal: a sprite on a cell a third of a pixel higher than the next came over that cell's sprites, as the chapel's back right wall (5 pixels high) over the cabinet against it at the start (#112).

An upper storey's floor (a type of bit `0x200`, #120) takes at each corner of its pieces the place of the ground point under the floor's point there, at its plane's height, 37 units further back (`statics::Spot`): what stands anywhere on a cell of the floor, at its cell's place, lies in front of the floor under it, and the floor's points further up the screen lie further back, as a plane through the depth buffer has them.
The sprites, miniatures and shadows of the floor's part that the original draws after the part's first floor stand on it (`statics::Standing`, #143): over the cells of the part, the sprites' shader keeps each no further back than the floor's plane brought forward by half a unit to 3 units, a little further for each in their order, and writes that depth (`fragment_on_floor`).
They lie over the floor whole, as the original draws them, and keep their order among themselves; before, the floor's plane cut them along horizontal lines on the screen, more than half a cell below their cells' points on the floor.
The models on the floor lie in front of them still, and the models under the floor's plane behind them, as the floor hides them in the original.
A model on the floor that reaches further forward than its cell's front corner is still cut by the floor's plane, as the cabinet at the front edge of that house's floor (#144).
Past the cells of the part, a wall that reaches from the floor down to the ground keeps its place, behind what stands on the ground in front of the building.
The hero, the NPCs and the objects on a building's floor stand at the floor's height (`terrain::Heights::lift`) above the second heights of its grid (`terrain::Heights::standing`, #141); before #120, at the ground's, 192 pixels below the floor of the house of sector 52,39, where the floor's carpet covered the hero's legs.

It draws a sprite in two passes (docs/decisions/2026-10-18-sprite-alpha.md): its opaque texels through the depth buffer, then the others blended by their alpha over what lies behind them, after the original's alpha test for the sprites and without it for the miniatures, each texel's colour weighted by its alpha as the shader filters them.
Bevy draws the blended things after the opaque ones, back to front by their meshes' centres; they go in layers in the original's order instead (`blended`): the miniatures of the liquid cells, the clear waters' second layer, the liquids' surfaces, the underlays of list 0, the statics' shadows of list 1, the underlays of list 2, the characters' shadows and the shadows drawn with their sprites, the sprites of lists 3 and 4, the lamps' glows, and a mesh's quads go far to near.

The statics' shadows (`statics`, #100) are quads of the shadow atlas's cells drawn by the sprites' shader in its blended pass, all their texels translucent, through two materials that every sector shares and whose alpha the night takes down to 0 (`daynight::shadows`).
List 1 lies 3,000 units back, between list 0 (4,000) and list 2 (2,000, 3,000 before list 1 came): a thousand units, 30 diagonals, from either, so that a shadow lies over every underlay of list 0 and under every one of list 2 whose cell is within 30 diagonals of its own.
A shadow drawn with its sprite takes the sprite's place, its list, its building's part, half a place behind it, and the characters' shadows' layer.
The sprites' shader samples a cell's own texels alone, where the original's linear filter takes in half a texel of the neighbouring cells at the edges.
Before, the sprites' edges went through alpha to coverage: the texels of alpha around a half came out dithered, those under it dropped, and a sprite's edge over another sprite showed as a dotted line (#35).
The blending takes the frame's sRGB values as they are, as the original's does (`srgb`, #109); before, an sRGB target blended in linear light, and a translucent dark texel darkened less.

The models take the same depth (`models`, docs/decisions/2026-10-18-models-among-sprites.md): every vertex of a model lies at the depth of the centre of the cell its root stands in, at height 0 as the sprites', its place after the cell's statics, plus a hundredth of its own depth from that centre at the cell's height, which keeps the model's parts in order among themselves.
A sprite of another diagonal, 33 units of depth away, is then wholly in front of a model or wholly behind it, as in the original; the sprites of the model's own diagonal within a few cells, a tenth of a unit apart, can still cross it.
Before, every sprite whose plane crossed a model cut it (#48).
A model's shadow (`shadows`, #101) is a copy of each of the model's meshes, skinned to the same joints, that the shadows' shader flattens onto the plane at the root's height: a point moves 0.037 units along X and 0.670 against Z per unit of its height above the plane, which on the screen is the original's lean (`shadows.wesl`).
It takes its model's anchor and squeeze, in the shadows' layer: over what the original draws before the model, a floor or the stairs it stands on, and under the model and what it draws after.
For the stencil, its fragments take the depth of the plane in their pixel's row, computed from the row alone, a unit of the model's own depth behind, and write it, drawn only where they are nearer than what lies there: a shadow's own triangles give a pixel the same depth to the bit, and darken it once (docs/decisions/2026-10-19-model-shadows.md).
It casts from the hero, from every NPC whose creature has no flag `0x10` and from the start script's objects of category 4; an NPC's weapons cast none, as the client flattens the creature's own model alone.
Before, a soft ellipse lay under each character, and the objects had none (#102).

The miniatures of list 0 in the liquid cells lie between the ground and the liquids' surfaces, 4,600 units back against the surfaces' 4,450 (4,300 before the surfaces lay flat at height 0, #103) and list 0's 4,000, in the order of list 0 among themselves (`statics::UNDER_LIQUIDS_BACK`, #96).
A sprite is a quad in the plane of the screen while the bed and the surface slope away under it, 1.37 units nearer per pixel down the screen and 1.83 per pixel of height: the margins hold for the tallest of these miniatures, from 68 pixels above their cell's centre to 139 below, over a bed up to 61 pixels off their cell's height, and for the underlays of list 0 beside the water, up to 124 pixels below their cell's centre (counted on the sector files 2026-10-18).
The anchors at height 0 (#112) take a quad 0.46 units back per pixel of its cell's height: the liquid cells holding these miniatures lie from 67.5 pixels below height 0 to 25 above, and over every cell whose diamond's box meets one of them the miniatures stay at least 210 units in front of the ground and 155 behind the surfaces; of the underlays of list 0 beside the water, none whose margin over a surface held before loses it (counted on the sector files 2026-10-18 with the cells' boxes, which overstate the overlaps).
With the surfaces flat at height 0 and 4,450 back (#103), the miniatures stay at least 73 units behind them, and no underlay of list 0 whose margin held over the surfaces at their beds' heights loses it, while 331 pairs of an underlay and a cell that failed the boxes' count before now pass it (the same count, 2026-10-19).

Not followed yet: which types the tests send to lists 0 and 4 (the game draws every model in list 3), which world objects `0x434070` sends under the water, the exception of liquid material 13 (sectors 29,72, 30,72 and 55,20; the building at (3563, 1336) lies in 55,20), and the trees' fade about the creatures other than the hero (`k` above 0).
