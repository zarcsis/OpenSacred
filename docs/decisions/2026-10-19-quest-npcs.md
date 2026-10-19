# The quests' NPCs at the start

Scripts (`opensacred-game`), 2026-10-19.

## The original

A new game runs the start script through: its `SetUpQuest` and `TriggerQuest` records set quests up and enter those whose trigger function runs through, running each one's `QIS_OnEnter<n>` (docs/re/netscript.md, "Quests").
Quest 1's `OnEnter` creates its giver, the Seraphim's novice at the `DefPos` `novizin1` among them, and orders her to walk to the chapel's benches (`NPC_Goto`); it also writes the quest log and points the compass at her.

## Ours

The game places the NPCs and the objects that the `OnEnter` functions of the quests entered at the start create, with the start script's (`npcs`), and an NPC that such a function sends somewhere stands there from the start.
A quest is entered when the start script sets it up and triggers it and its trigger function is empty; a trigger with records is taken as failing.
The rest of those functions (the quest log, the compass, the variables) is not run.

## Why

The game has no script interpreter yet, nor NPCs that walk: the NPC stands where the original's arrives a few steps later, and the start's triggers that the campaign uses are all empty.
