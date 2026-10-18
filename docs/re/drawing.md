# Drawing the world: the order of the objects

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra 12.1.4 on 2026-10-18. The draw lists and the cells' chains are in [docs/02 §6.1](../02-data-formats.md#61-itemspak-itm-v5-the-object-template-database) and [§5.5](../02-data-formats.md#55-staticpak-floorpak-triggerspak); the depth formula is in [§5.3](../02-data-formats.md#53-heights-and-the-render-contract). Status: **confirmed** on the code unless marked otherwise.

## The rows

A frame's world pass (the function around `0x632100`…`0x6328B0`) walks the screen's rows of diamonds from the top, two rows a step (the cell index advances by `0x41`, one cell along each axis, between the step's two rows), and hands each row to `0x62AE90`. A row is a diagonal of the grid, `cx + cy` constant. `0x62AE90` walks it from left to right: from a cell to the next the index moves by `−0x3F`, one column on and one row back, so `cx` grows by 1 and `cy` falls by 1, and the walk crosses into the neighbouring sector of the 3 × 3 that are loaded when it leaves one.

In each cell `0x62AE90` draws the ground's quad, then walks the cell's chain of statics (the head at `+0x04` of the cell, the next at `+0x1F` of each record) and appends each drawn object to a draw list. The lists are vectors of 24-byte entries (a kind, the object's index, its screen position as floats and as integers), and every entry is appended at the end, so a list holds its objects in the order of the walk: by row, then left to right, then in the order of the cell's chain. The kinds seen: 1 a sprite, 2 a miniature (object flag `0x20`), 8 an entry of the list read as list 1 (docs/02 §6.1), `0x11` a sprite of a type with bit `0x10000` when `DETAILLEVEL` is 1 or more, its mask is 2 or more and it lacks flag `0x800`.

An object with flag `0x8` is appended only when no room is current or when its mask (`+0x2B`) equals a value that the current room's trigger selects from a table of the world (`+0x268` of the object at `+0x04` of the view, 16-byte records, the word at `+0x0A`). **Hypothesis**: this is how the original shows a building's inside and hides its roof while the hero is in it (#3).

The lists are drawn after the last row: `0x62E410` and `0x62FF60`, called from the same function after its row loop, draw them. So every object of an earlier list lies under every object of a later list, whatever their rows.

## Depth

`0x62FF60` draws a list's sprites piece by piece. For a sprite whose type (the `items.pak` record's `+0x00` as a u32) has bit `0x200`, it switches two render states of the engine's state table (`0x100` and `0x1`; **hypothesis**: the depth test and the depth write) on and gives each of the quad's four vertices the depth that `0x6288A0` computes from the vertex's screen y and a height: the static's byte at `+0x33` (Static.PAK) times 28. `0x6288A0` returns the depth of the point at that screen y lying that high above the ground (docs/02 §5.3), so the quad lies in a horizontal plane at that height: what stands below it is hidden, what stands above shows. For every other sprite the two states are off and the depth is 0: it is drawn over what is there, in the order of its list.

In the game data 47 types carry bit `0x200`, with 220 drawn objects in the whole world; by their editor names they are the floors of towers' and houses' upper storeys.

## What the game does with it

`opensacred-game` orders the sprites through the depth buffer instead (docs/decisions.md 2026-10-17): each lies at the depth of its cell's centre, so all the sprites of one cell, and those of the cells of one diagonal at one height, have the same depth. Its sprite shader brings each one forward by a little in the order above: by its cell's distance along the screen's right, which grows with `cx` along a diagonal, and by its place in the cell's chain (`statics::SpriteMaterial`).

Not followed yet: where the models of the creatures and the objects of the start script go in this order (the "wobj" walk of `0x62AE90` appends the world's dynamic objects of a cell to the lists too), and what `0x62E410` draws.
