# The scripts' models show where their world positions do, at their cells' heights

Drawing (`opensacred-game`), 2026-10-21 (#438).
Refines the reading of #429, which drew a `CreateObj`'s model at its world position `0x20` on the heights of the object's own level.

## The original

The `CreateObj` handler keeps an object's position at its `0x04` cell's centre and sets the floats `+0xCC…+0xD4` from its world position `0x20`, `(x, y, layer)`, which the third number makes no height (`0x485C10`, docs/02 §7.3).
A raised doorway's `0x20` lies on the ground up the screen from it, √(3/2) units along −x and −y for a unit of the doorway's height above the ground's heights there: so do the base tree's three doors of layer 2, on floors 263 and 439 units up, and the keep's door of (2345, 2014) on a landing 116 up (docs/02 §7.3).
Drawn at `0x20` on the ground's heights, whatever the object's layer, their leaves show in their doorways (**hypothesis**: the client draws so, the open question 9 of docs/02).

## Ours

`opensacred-game` shows the model where `0x20` on the ground's heights shows, standing it along the line of sight at its cell's height on its level (`npcs::model_position`), rather than at `0x20` itself.
Where the heights at `0x20` are not loaded yet, another sector's, it takes the ground's height at the object's cell.

## Why

On the screen the model shows where the original's would.
Standing at its cell's height rather than on the ground under a building, a leaf on an upper storey or a landing casts its shadow on its own floor and takes its place among the sprites by a cell of its doorway, not by one up to ten cells up the screen; for a door on the ground the two differ by a few units.
The reading of #429, `0x20` on the heights of the object's own level, showed the leaves of the upper storeys' doors their floor's height up the screen from their doorways, 455 to 760 units from where they stand now.
The play does not change.
