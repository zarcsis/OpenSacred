# Liquids: water, lava and sulphur

Recovered from the GOG `Sacred.exe` 2.28 with capstone on 2026-10-17.
The cells' natures and the material table are in [docs/02 §5.2](../02-data-formats.md#52-a-sector-in-sectorswldx-the-32-b-floor-cell).
Status: **confirmed** on the code unless marked otherwise.

## Materials

The client loads 14 liquid materials at `0x4189DF` into records of 0xD8 B from `this + 0x91E8`: the handles of up to 50 frames, `+0xC8` the frame count, `+0xCC` a byte (1 for the clear waters), `+0xD0` an int, `+0xD4` a byte (1 for the glowing ones); the table of series and values is in docs/02 §5.2.

## Which material a cell takes

The drawing pass (`0x62E0E2`) takes the cell's nature (`+0x1F & 0xF0`): for `0x90` the material index at `+0xF7` of the sector's object, for `0xA0` the one at `+0xF8`.
These are the two liquid slots of the sector's `sectors.keyx` record (`+0x2E0`, `+0x2E1`); any other nature draws no liquid.

## Frames over time

If the animation is on (the byte at `0x182EE78`, the first field of the settings object, which the `DETAILLEVEL` handlers at `0x717E08`…`0x717EF9` set to 0, 1 or 2, is 1 or more; the GOG `Settings.cfg` has 2), the frame is

```
ms    = counter · 1000 / frequency          (0x6413E0's timer, _allmul and _alldiv)
phase = (ms >> 1) & 0x3FF                    (0 … 1,023)
frame = (int)(phase · frames · (1 / 1024))   (the float at 0x890710)
```

so every material runs through its frames once in 2,048 ms, whatever their count: 24.4 frames a second for the 50-frame series, 9.8 for the 20-frame ones.
With the animation off the frame is 0.

## Mapping

The vertices of a liquid cell come from a template built at `0x624AB6`: 4 × 4 sub-cells of 4 vertices of 32 B (XYZRHW, diffuse `0xFFFFFFFF`, UV), the UV stepping by 0.25 (the float at `0x8907C4`) along each axis.
A cell takes the sub-cell of its grid position `& 3` on both axes, so a frame spans 4 × 4 cells along the grid, its corners at the cells' corners.
Which grid axis is u is not verified.

A second template at `0x81640` has 2 × 2 sub-cells, UV stepping by 0.5, diffuse `0x00AAAAAA`; the pass at `0x62DDD7` fills it only for the materials with `+0xCC` set (the clear waters) and positions it with the cell's parity (`& 1`): the second layer below.

## The surfaces

Read 2026-10-19 (#103), `0x62DD00`.
A cell's surface is a strip of four vertices, left, top, bottom and right, at the cell's position on the screen plus the four offsets of a diamond at `+0xA90` of the view: no height is added, so the surface lies flat at height 0 while the bed lies at its heights (`+0x10`…`+0x13`, docs/02 §5.3).
Each corner's alpha is its own corner's bed height times the material's int at `+0xD0` (docs/02 §5.2), held to 0…255: under the waters of −12, clear over a bed at 0 or above, 60 at −5, 120 at −10, 180 at −15 and opaque from −22 down (from −11 for `F_WATER`, −24); the liquids of −255 (the lavas, the sulphur, `E_WATER`) opaque over any bed below 0 and clear at 0.
So a pond is clear at its shore, where the bed comes up to 0, and as deep as its bed is opaque.
Its colour is white, or while the renderer's flag `0x20` (the night) or the dungeons' `+0x9E0C` is set, the light at the corner's point of the screen (`0x41B510`, docs/re/daynight.md).
It is blended by its alpha (`SRCALPHA`, `INVSRCALPHA`), filtered linearly, with the frame of the moment bound per cell (`0x628F90`).

At `DETAILLEVEL` 2, before the surfaces, the clear waters (`+0xCC`) have a second layer: the cell's own ground quad, on the bed, each corner moved 0.2 pixels inward over the zoom, with the 2 × 2 template's coordinates, added to the frame (`ONE`, `ONE`), its grey at each corner `clamp(−8 · h, 0, 255) · (255 − N) >> 8` for the bed's height `h` and the night's level `N`.
The pass binds no texture of its own for it (read 2026-10-19, #118): its quads and the surfaces' go into one batcher (`+0x818C0` of the view), which draws its quads with the texture that `0x628F90` last gave it, and only the surfaces call `0x628F90`, with the frame of the moment of their cell's material.
The second layer's quads are flushed (`0x62E0A3`) before the surfaces' first call, so they take the frame that the batcher kept from the frame before: the frame of the moment then of the material of the last liquid cell drawn (none at the very first frame).
So the second layer shows the water's own frames, mapped a frame over 2 × 2 cells, a frame of the clock behind the surfaces.
The view's constructor (`0x624A00`) also loads 32 textures `CAUST00` to `CAUST31` into handles at `+0x81840` that nothing else names: loaded and never bound.
After the quads, `0x6770E0` and `0x690690` get the average of the drawn cells' positions (`0x622620`), and with nothing drawn, 0: the place of the water's ambient loops (docs/re/sound.md), which `opensacred-game` pans by the clear cells' own middle since #1169 (docs/decisions/sound.md, "Water loops panned by the water").

`opensacred-game` follows the slots, the timing, the 4 × 4 mapping and the surfaces, flat at height 0 and as opaque as the bed is deep (`liquids.rs`).
It draws the clear waters' second layer before the surfaces (#118): each cell's ground quad on the screen where its bed lies, at the surface's depth there, with its own material's frame of the moment over 2 × 2 cells, added to the frame, each corner's grey by the bed's depth and the whole by the night's level (docs/decisions/drawing.md, "Clear waters' second layer per cell").
Its surfaces lie over the miniatures of the liquid cells and under the draw lists, as the original draws them between the two (docs/re/drawing.md, "The liquid cells"), and over the waters' crabs and fish and the items lying in a liquid cell, with their shadows (#983).
