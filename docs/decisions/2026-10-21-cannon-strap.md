# The cannon's strap

Models (the converter), 2026-10-21.

## The original

`DWARF_LEATHER_CANNON.GRN` (model 1415, the cannon of item 3015) weights five of the 43 positions of its strap `shoulder_leather01`, at shoulder height, wholly on `Bip01 Pelvis`, 17 to 23 units below them, among positions on `Bip01 L Clavicle` and `Bip01 Neck` (docs/02 §4.6).
Worn on the dwarf, the strap's sides from them stretch up to 22 times in the dwarf's clips.
The community reports the cannons shown wrongly (**community-reported**); what the client shows for this one is not known.

## Ours

`opensacred-convert models` weights those five positions wholly on `Bip01 L Clavicle`, the positions unchanged (#399), so that the strap follows the shoulder there.
A record of that name whose strap is not what the repair expects, five positions wholly on the pelvis at the places it knows, is written as it is, with a warning.

## Why

The repair of the data belongs to the converter, not to the game (CLAUDE.md, "Code conventions").
The bone is the one that 10 of the 13 sides from those positions to other bones' positions run to (docs/02 §4.6).
Two of the five lie nearest a position on `Bip01 Neck`, but giving them the neck instead leaves about as many sides stretched more than twice in the dwarf's clips (34 against 32).
The strap's own split between the neck and the clavicle stays as the original has it.
The play does not change.
