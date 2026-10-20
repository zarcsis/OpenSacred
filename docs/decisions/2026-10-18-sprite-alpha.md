# The sprites' alpha

Rendering (`opensacred-game`), 2026-10-18; changed 2026-10-19 (#100, #118).

## The original

Every sprite is blended by its alpha (`SRCALPHA`, `INVSRCALPHA`) over what is drawn, after an alpha test that drops its texels of alpha 64 or less, point-filtered and without the depth test, in the order of the draw lists; a miniature is blended without the test (docs/re/drawing.md, "Blending").

## Ours

A sprite is drawn twice.
Its opaque texels go through the depth buffer with the opaque things, as before; the others are blended by their alpha over what lies behind them, tested against the depth but not written to it, after the original's alpha test for the sprites.
The blended things go in layers in the original's order: the miniatures of the liquid cells, the clear waters' second layer, the liquids' surfaces, the underlays of list 0, the statics' shadows of list 1, the underlays of list 2, the characters' shadows and the shadows drawn with their sprites, the sprites of lists 3 and 4, the lamps' glows (`blended`); within a layer, by their meshes' centres, and a mesh's quads far to near.
The shader filters the texels linearly, each colour weighted by its alpha, and a sprite's image is its size from its pieces' smallest corner.

## Why

The sprites are ordered through the depth buffer (docs/decisions/2026-10-17-sprite-depth.md), which a blended texel does not write.
The texels between transparent and opaque are not only edges: most lie in translucent areas, shadows and glass (an estimate on a twentieth of the atlases: 3 % of the texels, 70 % of them among other such texels).
Alpha to coverage, which drew them before, dithered those of alpha around a half and dropped those under it, and a sprite's edge over another sprite showed as a dotted line that moved with the camera by a fraction of a pixel (#35).
Blended in their own pass, they only go wrong where two of them overlap in the wrong order, between two meshes of one layer.
Linear filtering, for the magnified sprites at 4K (docs/05 §3.6), would fade a sprite's art half a texel short of an edge where it goes on in another sprite, and what lay behind both showed through as a line; the image cut to the sprite's size, whose texels past it are transparent, keeps the art opaque to the edge, as the original's points do.
The same filtering without the weights darkened every edge with the black of the transparent texels.
