# The blacksmith's apron made whole

Models (the converter), 2026-10-22 (#401).

## The original

The blacksmith's apron texture, record 8202 of `texture.pak` (`BLACKSMITH_BELT.TGA`), has alpha 0 over two strokes across it, 1,176 texels, 406 of them under 16 of the 154 triangles of the flaps hanging below the belt, while its colour runs on unbroken beneath them (docs/02 §4.6).
Drawn by its alpha, the front flap is mostly gone, a strip left under the belt: the small hole in the blacksmith's model that the community reports.
The warriors who share the blacksmith's meshes wear an opaque apron (6825), and Valor's transparent texels are its flaps' ragged tips, not their middle.

## Ours

`opensacred-convert textures` writes record 8202 opaque, every texel's alpha 255 and its colour unchanged, so the apron shows whole.

## Why

The strokes cut through the flaps' middle over a colour painted whole beneath them, a mask of the alpha rather than a design; the repair is as narrow as the defect, one texture's alpha.
The maintainer chose it (#401).
The game draws the models opaque for now (#516), so the repair shows only once the models' alpha is drawn.
