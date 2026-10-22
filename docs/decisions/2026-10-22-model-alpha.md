# The models' alpha

Rendering (`opensacred-game`), 2026-10-22 (#519).

## The original

A model is drawn at its place in the draw lists with the alpha test on, which drops its texels of alpha 64 or less, and the alpha blending on (`SRCALPHA`, `INVSRCALPHA`), its depth tested and written, its texture filtered linearly (docs/re/drawing.md, "The models").
Its translucent texels lie over what the lists drew before it and under what they draw after it.

## Ours

A mesh whose material has translucent texels, of alpha under 255 in its texture's image or in its colour, is drawn twice, as the sprites are (docs/decisions/2026-10-18-sprite-alpha.md).
Its opaque texels go through the depth buffer at the model's place among the sprites, as before; a copy of the mesh draws the others that the original's alpha test keeps, blended by their alpha over what lies behind them, in the blended things' layer of lists 3 and 4 (`blended::Layer::Objects`), their depth tested and written as the original's (`models::Pass`).
Within the layer they go by their meshes' centres, back to front, and a mesh's triangles in their order.
A mesh whose material has no translucent texel is drawn once, through the depth buffer after the alpha test, as before.
The game reads each image's alpha as it loads, the item's texture of a retextured model too, and a model waits for its images before its meshes take their materials.

## Why

The models are ordered among the sprites through the depth buffer (docs/decisions/2026-10-18-models-among-sprites.md), where a blended texel cannot be drawn in the original's order.
Blended in their own pass after the opaque things, the translucent texels lie over every opaque thing behind them and under every one in front, as in the original; they go wrong only where they overlap another translucent thing of the same layer that comes in the other order: a sprite's translucent texels, another model's, or another mesh of the same model, when the meshes' centres order them otherwise than the draw lists.
The depth they write keeps a mesh's own triangles as the original's: a translucent part drawn first hides the parts behind it that come after it, and one drawn after the parts behind it lies over them.
Of the 334 textures that the converted models name and that have any alpha under 255, 169 have texels of alpha 65 to 254 under their triangles (counted 2026-10-22, #519): the ghosts almost whole, the bubbles, the element crystal, and in part the wings of the dragons, the hornets, the giant fly and the flight lizard; drawn opaque before, the ghosts showed as solid figures and the wings as solid membranes.
The game reads the images rather than the converter marking the materials (glTF's `alphaMode` `BLEND`): a retextured model's item's texture replaces its materials' only in the game (`npcs::retexture`).
A model's meshes are drawn only once their textures are loaded, so waiting for the images delays nothing.
