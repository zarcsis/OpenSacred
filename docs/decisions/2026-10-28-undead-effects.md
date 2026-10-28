# The undead's dust: from the bones, without the wind

The effects of the class 5's deaths (`opensacred-game`'s `puffs` and `undead`), 2026-10-28 (#1261).

## The original

A corpse of the class 5 that vanishes at its last death makes the effect 781 of the variant 4 (`cParticleSystem_puzzleSolved_v`, `0x76D260` at `0x76D704`): 150 particles at once at the corpse model's skinned vertices (`0x44FC30`, `0x767FB0`), a pale bone-coloured dust that rises ever faster and grows over 2.55 s (docs/re/effects.md, "The undead's effects").
The variant also writes the renderer's vector into its layer's wind, as the smoke class does (docs/re/effects.md, "The fires and smokes").

## Ours

The dust leaves the corpse's bones (the skeleton's `Bip01 …` nodes at the moment of the death), each particle at one of them at random, rather than its model's skinned vertices; a model without bones gives one point, its root.
The wind is left out, as it is for the fires and smokes.

## Why

The vertices that the original's skinning puts on the screen live in Bevy on the GPU alone, and reading them back for a burst of 150 particles is not worth it: the bones lie inside the body as its vertices do, from its feet to its head, and the dust, of quads of 10 to 30 units that rise and grow, hides where each particle began.
The wind's vector is a hypothesis of docs/re/weather.md not followed by any effect yet.
