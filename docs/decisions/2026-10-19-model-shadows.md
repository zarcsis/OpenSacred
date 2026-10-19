# The models' shadows

Rendering (`opensacred-game`), 2026-10-19.
Replaces [2026-10-17-shadows](2026-10-17-shadows.md).

## The original

A creature's or a chest's shadow is its mesh flattened onto the plane at its height along a fixed light, black at alpha 170, drawn right before the model through the stencil: the first shadow drawn over a pixel darkens it and no other does (docs/re/drawing.md, "The models' shadows").
Its depth test is against the scene's true depths, the models' and the buildings' sprites'.

## Ours

The same flattened meshes at the same darkness, leaning the same way on the screen, at their models' places among the sprites (`shadows`, `shadows.wesl`).
The depth test stands in for the stencil: a shadow's fragments take the depth of the plane in their pixel's row, computed from the row alone, and write it, and a fragment is drawn only where it is nearer than what lies there.
A shadow's own triangles then darken a pixel once, as the stencil has it.
Of two shadows over one pixel, the nearer darkens it, and both do if the nearer comes after the other, where the stencil lets the first alone.
The depth that a shadow writes hides, at its pixels, what the blended layers after it hold behind it: the translucent edges of the sprites of earlier places and the lamps' glows, which the original draws under the shadow.
The lean is the game's ground moved so that a point goes as far on the screen as the original's shadow takes it, per unit of the model's height.

## Why

Bevy's main pass has a depth buffer without a stencil, and a game cannot choose another format for it.
The depth test gives the stencil's result within a shadow, where a flattened mesh's overlapping parts, an arm over the body, would otherwise darken twice, and differs from it only where two shadows overlap.
