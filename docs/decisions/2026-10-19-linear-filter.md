# The magnified art's filter

Rendering (`opensacred-game`), 2026-10-19 (#42).

## The original

It draws its sprites with point filtering (`0x62FF60` turns the renderer's linear filter off before their pieces, docs/re/drawing.md, "Blending"), its miniatures and its ground linearly only at some zooms (while the renderer's float `+0xB60` is over 1.2, the state `0x800`; docs/02 §5.4), else by point, on a screen of 1,024 × 768, where a texel of the art covers a pixel at zoom 1.

## Ours

The ground and every sprite are filtered linearly at any zoom and resolution: the shaders mix the four texels about a sample, each colour weighted by its alpha (`ground.wesl`, `sprites.wesl`).
At 3,840 × 2,160 a texel covers 2.8 pixels at zoom 1 and 5.6 at zoom 2.

## Why

The maintainer's choice (#42), made in the game itself against the point filter and a sharp bilinear one (#124, `--filter`, removed since): magnified, the point filter's square texels are blocks that shimmer as the camera moves.
