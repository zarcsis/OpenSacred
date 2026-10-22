# The scripts' models stand where the client's do, without its limit of their offset

Drawing (`opensacred-game`), 2026-10-21 (#438); changed 2026-10-22 (#459).
Refines the reading of #429, which drew a `CreateObj`'s model at its world position `0x20` on the heights of the object's own level.

## The original

The `CreateObj` handler keeps an object at its `0x04` cell's centre, in the draw list of that cell on its layer, and sets the floats `+0xCC…+0xD4` to its world position `0x20` moved by `1.92 · h` world units along x and along y for the object's height `h` at its cell's centre on its layer, and to `h` (`0x485C10`; docs/re/drawing.md, "The models").
Its draw takes that point for the model's position when it lies within 200 of the object in the models' frame for a door or a chest, within 1,000 for any other, and leaves the model at the object's position otherwise (`0x44B230`).
So the model shows where `0x20` at height 0 does, at the object's height, the ground's height at `0x20` never read: a raised doorway's `0x20` lies on the ground up the screen from it by its height (docs/02 §7.3), and its leaf shows in its doorway, drawn after its cell's sprites.

## Ours

`opensacred-game` shows the model where `0x20` at height 0 shows, standing it along the line of sight at its cell's height on its level (`npcs::model_position`), and takes its place among the sprites by the object's cell (`models::place`), as the client does.
But it tests no limit: every model with a `0x20` stands at its point.
Where a sector's record lacks bit 16 of `+0x30`, the client makes the object without a position, which the game does not follow either (not read).

## Why

None of the 1,132 doors of the Seraphim's tree comes near the limit of 200 (an **estimate**, a scratch script over the files), so the test would change nothing that shows, and the models' frame of `0x6239C0` is not set beside the game's units in the code.
Until #459 the game took `0x20` on the ground's heights there (#438, a **hypothesis** then), which drew 99 of the tree's doors off the original's, by up to 46 pixels at zoom 1, and gave a model the place of the cell its root stood in, which put the leaves of the upper storeys' doors behind the back walls of their own cells.
