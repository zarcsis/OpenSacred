# The ambient loops

Sound (`opensacred-game`), 2026-10-17; changed 2026-10-18 (#5).

## The original

The ambient loop follows the nature underfoot by day and by night, and some regions and sieged villages have their own (docs/re/sound.md).

## Ours

The loops by the nature, the day's by day and at the dawn and the night's in the dusk and by night (#5), on a channel of their own beside the music; no regions' loops.

## Why

The regions' loops follow the scripts' variables `atmos_rg<N>`, which the game does not run (docs/re/sound.md); the client plays the loops and the music through one function, `cMSS::playMusic`, and whether both sound at once is not read.
