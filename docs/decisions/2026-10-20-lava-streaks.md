# The lavas' streaks

Liquids (the converter), 2026-10-20.

## The original

The frames of `A_LAVA`, `D_LAVA` and `E_LAVA` hold texels that are not opaque at the end of row 0 (and, in `E_LAVA`, one in row 2), junk of the tool that made them (docs/02 §5.2).
The client blends a liquid's surface by its texture's alpha times its corner's (docs/re/liquids.md), so by its code it shows them as grey and purple streaks along one edge of every 4 × 4 cells (**hypothesis**: not seen in the original; a community fix of the lava's textures, "Sacred Gold: Miscellaneous Fixes", supports it).

## Ours

`opensacred-convert textures` writes those frames opaque: every texel that is not opaque takes the colour of the nearest opaque texel of its column (#347).
`assets/` holds clean frames, so the game and the HD packs made from the PNGs later (M7) need not know about it.

## Why

The streaks are a defect of the data, not a look of the lava, and its repair belongs to the converter (CLAUDE.md, "Code conventions").
The play does not change.
