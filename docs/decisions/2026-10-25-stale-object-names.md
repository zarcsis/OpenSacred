# `SetObjState` by a name whose object is gone keeps the state for its next object

Scripts (the runtime's `SetObjState`, after #726), 2026-10-25 (#952); the maintainer's decision.

## The original

`SetObjState` (2, `0x48AE90`) resolves its name through the scripts' map of names, which answers the id kept for the name without testing that its object still exists.
For a name that the map does not hold, it keeps the state in a list by name, for the object that is made later (docs/re/netscript.md, "Order traps").
An object that a sector's `Enter` makes is destroyed as the sector leaves the 3 × 3, and its name stays mapped to the id that no longer exists.
So a state set by name after the sector was entered and left is neither applied nor kept, and the next object that the `Enter` makes under that name is made in its old state.
- Quests 2411 and 2201: their doors' unlocks from afar are lost where the cave or the dungeon was entered before, and the doors stay locked.
- In the Seraphim's tree the scripts set the state of 38 objects that an `Enter` makes, by name; only these two quests are reported.

## Ours

In the runtime, a name whose object is gone counts as a name never seen.
`opensacred-game` follows it since #952 by keeping the state with the object's trigger, which the object's name finds among the scripts' `CreateObj`s whether the object is in the world or not.
`SetObjState` keeps the state for it, and the next object made under that name takes the state.

## Why

The data's repair, a test of the quest's bits after each reported door's `CreateObj`, would mend the two reported doors alone.
The defect is the map's stale id, which reaches all 38 objects, so the runtime mends it once for all of them.
The departure from the client: a state set by name while its object's sector is away now reaches the object when the sector comes back.
