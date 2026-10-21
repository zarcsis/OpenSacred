# The dwarf's earth amulet

Models (the converter), 2026-10-21.

## The original

`DWARF_AM_ERDE.GRN` (model 792), the dwarf's earth amulet, binds its mesh rigidly to a bone of its own, `Amulett_erd`, straight under `__Root`, which the dwarf's skeleton (`DWARF.GRN`, 722) lacks, where the dwarf's 18 other amulets follow `Bip01 Spine3` (docs/02 §4.6).
Its skeleton also lacks the `Root` that theirs and the dwarf's have, so that its bind pose lies 0.34 units off theirs, while its mesh lies where theirs lie.
The community reports an amulet shown wrongly (**community-reported**); what the client shows for this one is not known.

## Ours

`opensacred-convert models` gives the file the siblings' `Root`, bit for bit, as the parent of `Bip01`, and binds the amulet's 46 positions rigidly to `Bip01 Spine3`, positions unchanged (#398).
Worn by bone names, it then sits on the dwarf's chest where the other amulets sit, and follows it.
A record of that name whose data is not what the repair expects is written as it is, with a warning.

## Why

The repair of the data belongs to the converter, not to the game (CLAUDE.md, "Code conventions").
The bone is the one the 18 siblings follow, and the bind pose theirs: by the file's own, the amulet would sit 0.34 units off the place that its siblings and the other classes' earth amulets give it.
The play does not change.
