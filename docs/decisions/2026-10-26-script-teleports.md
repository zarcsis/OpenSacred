# The scripts' teleports of the hero land on their cell

Scripts (`opensacred-game`'s `scripts`), 2026-10-26 (#1002).

## The original

`Teleport` (46, `0x491D40`) puts the creature on its cell through `sdk_ENGINE_TP` (`0x54D9D0`).
Where the cell cannot take it, the handler tries 15 cells at random within 3 cells on the same layer, then 25 within 6 on that layer and the two above it, and turns the creature to the record's heading (`03`) (docs/re/netscript.md, "Base triggers").

## Ours

A base trigger's `Teleport` of the hero puts it on the cell and the layer it names, as the command `cell` does, once the sectors there are in (`hero::Arrival::On`).
The random tries and the heading are not followed.

## Why

The scripts' targets are cells the hero may stand on: the caves' entrances and exits of the Seraphim's tree land where the original's hero lands.
A random try would only matter for a target the hero cannot stand on, and the game's landing already finds the floor of the cell it is put on.
The heading needs the hero's turn by a degree, which the game does not have yet; the hero faces as it faced.
