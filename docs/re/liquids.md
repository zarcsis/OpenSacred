# Liquids: water, lava and sulphur

Recovered from the GOG `Sacred.exe` 2.28 with capstone on 2026-10-17. The cells' natures and the material table are in [docs/02 §5.2](../02-data-formats.md#52-a-sector-in-sectorswldx-the-32-b-floor-cell). Status: **confirmed** on the code unless marked otherwise.

## Materials

The client loads 14 liquid materials at `0x4189DF` into records of 0xD8 B from `this + 0x91E8`: the handles of up to 50 frames, `+0xC8` the frame count, `+0xCC` a byte (1 for the clear waters), `+0xD0` an int, `+0xD4` a byte (1 for the glowing ones); the table of series and values is in docs/02 §5.2.

## Which material a cell takes

The drawing pass (`0x62E0E2`) takes the cell's nature (`+0x1F & 0xF0`): for `0x90` the material index at `+0xF7` of the sector's object, for `0xA0` the one at `+0xF8`. These are the two liquid slots of the sector's `sectors.keyx` record (`+0x2E0`, `+0x2E1`); any other nature draws no liquid.

## Frames over time

If the animation is on (the byte at `0x182EE78`, the first field of the settings object, which the `DETAILLEVEL` handlers at `0x717E08`…`0x717EF9` set to 0, 1 or 2, is 1 or more; the GOG `Settings.cfg` has 2), the frame is

```
ms    = counter · 1000 / frequency          (0x6413E0's timer, _allmul and _alldiv)
phase = (ms >> 1) & 0x3FF                    (0 … 1,023)
frame = (int)(phase · frames · (1 / 1024))   (the float at 0x890710)
```

so every material runs through its frames once in 2,048 ms, whatever their count: 24.4 frames a second for the 50-frame series, 9.8 for the 20-frame ones. With the animation off the frame is 0.

## Mapping

The vertices of a liquid cell come from a template built at `0x624AB6`: 4 × 4 sub-cells of 4 vertices of 32 B (XYZRHW, diffuse `0xFFFFFFFF`, UV), the UV stepping by 0.25 (the float at `0x8907C4`) along each axis. A cell takes the sub-cell of its grid position `& 3` on both axes, so a frame spans 4 × 4 cells along the grid, its corners at the cells' corners. Which grid axis is u is not verified.

A second template at `0x81640` has 2 × 2 sub-cells, UV stepping by 0.5, diffuse `0x00AAAAAA`; the pass at `0x62DDD7` fills it only for the materials with `+0xCC` set (the clear waters) and positions it with the cell's parity (`& 1`). Hypothesis: a second, translucent layer over the clear waters (reflection or caustics, `CAUST00`…). What texture and blending the two layers use is not followed.

`opensacred-game` follows the slots, the timing and the 4 × 4 mapping (`liquids.rs`).
