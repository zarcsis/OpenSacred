# The water's loops pan by the water on the screen

The sound (`opensacred-game`, `ambience`), 2026-10-27 (#1169).

## The original

The water's three loops (`0x6906C0`, docs/re/sound.md, "Ambience") take their volume from the count of the liquid cells drawn alone, and their pan from a place that the world view's liquids' pass hands the sound manager (`0x62DD00`, `0x690690`).
That place mixes units: for each clear water's cell it adds the sector's bounds in world units (`sectors.keyx +0x3C`, `+0x40`) to the sector's first cell in cells and the cell's column and row within the sector, divides the sums by the count of every liquid cell, the lava's too, and turns the result into the screen's pixels.
So the place lies about 1.19 times the sector's coordinate in cells past the sectors' origins, far from the water and from the listener, and its pan by the sound's place (`0x678260`) depends on the camera's sector rather than on where the water is; a far place can even give a pan outside 0 to 1.

## Ours

The loops pan by the middle of the clear waters' cells on the screen, at the place of the sound (`sfx::placed_at`): to the side the water lies on, and to the middle within 260 pixels of it.
Their volumes, thresholds and rates are the original's.

## Why

The original's place reads as a slip of units: it cannot be what the pass meant to hand on, and copying it would need the sectors' bounds in `world.ron` for a pan that has nothing to do with the water.
It changes only how the water's loops are panned.
