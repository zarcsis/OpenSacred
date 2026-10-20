# The magnified art's filter

Rendering (`opensacred-game`), 2026-10-19 (#42).

## The original

It draws its sprites with point filtering (`0x62FF60` turns the renderer's linear filter off before their pieces, docs/re/drawing.md, "Blending"), on a screen of 1,024 × 768, where a texel of the art covers a pixel at zoom 1.
Its miniatures and its ground it filters linearly only in the wider views, while the zoom (the world view's float `+0xB60`, 0.5 to 2, 2 the widest) is over 1.2 and a texel covers less than 5/6 of a pixel, and by point at 1.2 and closer (the state `0x800`; docs/02 §5.4).

## Ours

The ground and every sprite are filtered linearly at any zoom and resolution: the shaders mix the four texels about a sample, each colour weighted by its alpha (`ground.wesl`, `sprites.wesl`).
At 3,840 × 2,160 a texel covers 2.8 pixels at zoom 1 and 5.6 at zoom 0.5, the closest view.
So the game departs from the original for the sprites at every zoom, and for the miniatures and the ground at 1.2 and closer.

## Why

The maintainer's choice (#42), made in the game itself against the point filter and a sharp bilinear one (#124, `--filter`, removed since): magnified, the point filter's square texels are blocks that shimmer as the camera moves.
