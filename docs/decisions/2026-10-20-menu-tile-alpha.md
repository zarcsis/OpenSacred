# The Underworld main menu's faint tile

UI (the converter), 2026-10-20.

## The original

`GUI_UW_OPTION12.TGA`, one of the twelve tiles of the Underworld main menu, carries an alpha of 195 to 255 over its body where the other tiles carry 255 (docs/02 §3.1), and the client draws the tiles by their alpha, so the rocks at the menu's bottom right show the animated water through them (community-reported, with a community fix).

## Ours

`opensacred-convert textures` scales that record's alpha by 255/195, held at 255, as it writes the PNG: its body opaque, its edge's ramp kept (#352).

## Why

A tile of a painted screen is not meant to be seen through; the repair of the data belongs to the converter (CLAUDE.md, "Code conventions").
The play does not change; the game's own menu does not draw the original's screens yet (M5).
