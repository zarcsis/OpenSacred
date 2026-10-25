# The scripts' drops kept by the creature and the item

Scripts (the runtime's `SetDrop`, after #726), 2026-10-25 (#930); the maintainer's decision.

## The original

`SetDrop` (137, `0x4AFFF0`) keeps at most one entry per creature type: a second quest's drop on the same type overwrites the first's chance and item, and a chance of 0 removes the type's entry (docs/re/netscript.md, "`SetDrop`'s one entry per creature").
Five pairs of quests give one creature type a drop each, and the community reports them all: of two such quests taken, the earlier one's item stops dropping, and once the later one is counted, its chance 0 removes the entry, so the earlier one cannot be finished by drops.

## Ours

The runtime keeps the scripts' drops by the creature type and the item.
A `SetDrop` sets the chance of its own type and item, a chance of 0 removes that pair alone, and a kill rolls every entry of its type, each with its own chance, in the order in which they were set.

## Why

Two quests on one creature type are in the data, and the client's single entry per type makes the earlier of them impossible to finish by drops.
A repair of the data could only make the drops come one after the other, and no test of the partner quest being open was found for it.
The departure: with two such quests open, one kill can drop both items, where the original drops the later quest's item alone.
