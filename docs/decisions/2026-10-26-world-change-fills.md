# The world's change turns area 2's fills back, and two fills take their side

Scripts (`opensacred-convert netscript`), 2026-10-26 (#1068); the maintainer's decision.

## The original

The scripts' `ReplaceSpawn`s change the fills of an area for good (docs/re/netscript.md, "`ReplaceSpawn` and `RegionChange`: the world's changes"), and two of them slip:
- **Area 2 after quest 72.**
  Quest 45's `GoodToBad` swaps the soldiers 279, 286, 288, 295 and 297 for the hostile 276, 277, 278, 287 and 296 in the fills of the areas 2, 4, 5, 7, 11 and 24; quest 72's `BadToGood` swaps them back in the areas 4, 5, 6, 7, 11 and 24, not in 2, so that area 2's fills keep the hostile soldiers for the rest of the game.
- **Two `ReplaceSpawn`s without a side.**
  Quest 50's `Dialog:DlgHologram50` (area 8, the type 35 into 48) and quest 61's `ElementAbfrage` (area 9, 257 into the skeleton warrior 32) carry neither `0x08` nor `0x46`, which every other `ReplaceSpawn` of their functions carries, so that the runs they change take the side 0 and the flags word 0 (**hypothesis**: those undead come peaceful).

Two other differences of the pair stay as the original has them: `GoodToBad`'s records of area 6 name one type each and store nothing, while `BadToGood`'s swap area 6's hostile soldiers for the good ones, and area 5's 290 and 281, which `GoodToBad` turns into 277 and 276, come back as 286 and 279.

## Ours

`opensacred-convert netscript` mends the records as it writes the trees (`script_repairs`):
- `BadToGood` gets five `ReplaceSpawn`s of area 2, its five of area 4 written for area 2, before them, in 11 trees;
- the two `ReplaceSpawn`s take `0x08` and `0x46`, in 9 trees each.

## Why

The world's change after quest 72 turns every other area's soldiers back, and the hologram's and the elements' undead come in their siblings' guards' mode and hostile; both differences read as slips of the scripts, and neither is the play the quests describe.
Area 6's and area 5's differences are left: what the scripts meant there is not clear.
