# The quests' NPCs at the start

Scripts (`opensacred-game`), 2026-10-19; narrowed the same day (#132); brought up to date 2026-10-23 (#692).

## The original

A new game runs the start script through: its `SetUpQuest` and `TriggerQuest` records set quests up and enter those whose trigger function runs through, running each one's `QIS_OnEnter<n>` (docs/re/netscript.md, "Quests").
Quest 1's `OnEnter` creates its giver, the Seraphim's novice at the `DefPos` `novizin1` among them, and orders her to walk to the chapel's benches (`NPC_Goto`, an event of walking that the creature's handler takes); it also writes the quest log and points the compass at her.

## Ours

The game places the NPCs and the objects that the `OnEnter` functions of the quests entered at the start create, with the start script's (`npcs`), and since #636 those of each quest's `OnSetUp` function after the `SetUpQuest` that first sets the quest up, as `SetUpQuest` runs it.
An NPC that such a function sends somewhere walks there as the hero walks: the hero's way round what blocks it and through the doors (docs/re/movement.md), at its creature's walk speed (`Creature.pak +0x26`) times the game's, with its model's walk clip, then stands with its idle clip; a target it cannot reach it walks straight toward, up to what blocks it.
A quest is entered when the start script sets it up and triggers it and its trigger function is empty; a trigger with records is taken as failing.
The rest of those functions (the quest log, the compass, the variables) is not run.
Of the other records, the game follows a `SetNPCState`'s level, dialogue, flags word, side, trades and home (#578, #584, #662; docs/re/netscript.md) and a `CreateNPC`'s level, group and hostility tags (#540, #669).

## Why

The game has no script interpreter yet, reading the records it follows from the trees, and how the creature's handler walks an NPC to its target (its way, its gait) is not read: the hero's way and walk stand in for it.
The start's triggers that the campaign uses are all empty.
