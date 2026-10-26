# The models among the sprites

Rendering (`opensacred-game`), 2026-10-18.

## The original

A creature or an object of the world is drawn at its cell's place in the draw lists, after the cell's statics, and an ordinary sprite without the depth test (docs/re/drawing.md, "The models").

## Ours

A model's vertices take the depth of the centre of the cell its root stands in, after the cell's statics, plus a hundredth of their own depth from it (`models.wesl`); a weapon takes its holder's; every model goes in list 3.
Among themselves the models keep their true depths, as the original's depth test orders them (#998): the camera's depth prepass draws the models alone at their true depths, squeezed behind all that the main pass draws, and a model's fragment keeps only the samples over which no model's texel lies nearer, so that the nearest model alone takes its place among the sprites.

## Why

The sprites are ordered through the depth buffer (the row above): at their true depth, the models were cut by every sprite whose plane crossed them (#48).
A hundredth keeps a model's parts a tenth of a unit's steps of the depth buffer apart and the model within half a unit of its place, so only the sprites of its own diagonal within a few cells can still cross it.
That place alone set a model of a later diagonal wholly in front of the models of earlier ones, as a sprite would be, and the gate of the barn of sector 53,40 drew its rail across the NPCs in front of its wall; the prepass gives the models their own order without a second camera or another pass over the sprites.
