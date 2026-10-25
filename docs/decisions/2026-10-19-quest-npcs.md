# The quests' NPCs at the start

Scripts (`opensacred-game`), 2026-10-19; narrowed the same day (#132); brought up to date 2026-10-23 (#692); the scripts run in Luau 2026-10-25 (#726).

## The original

A new game runs the start script through: its `SetUpQuest` and `TriggerQuest` records set quests up and enter those whose trigger function runs through, running each one's `QIS_OnEnter<n>` (docs/re/netscript.md, "Quests").
Quest 1's `OnEnter` creates its giver, the Seraphim's novice at the `DefPos` `novizin1` among them, and orders her to walk to the chapel's benches (`NPC_Goto`, an event of walking that the creature's handler takes); it also writes the quest log and points the compass at her.

## Ours

The game runs the start script in Luau (`scripts`, #726): its `SetUpQuest` runs the quest's `OnSetUp` and trigger in its middle, its `TriggerQuest` the trigger and, where that runs through, the quest's `OnEnter` (`opensacred_rules::quests`), and a `CallFunktion` the function it names; the game places the NPCs and the objects of the records so run, the start script's and those of the sections in their order (`npcs`).
An NPC that such a function sends somewhere walks there as the hero walks: the hero's way round what blocks it and through the doors (docs/re/movement.md), at its creature's walk speed (`Creature.pak +0x26`) times the game's, with its model's walk clip, then stands with its idle clip; a target it cannot reach it walks straight toward, up to what blocks it.
The conditions it answers are those on the variables (`0x48`, `0x49`, `0x4A`, `0x6D`, `0x6E`), which `SetVar`, `SetVarBit`, `UnsetVarBit`, `IncVar` and `RndVar` set, and `0x52`, the hero's tree (**hypothesis**: the hero's type, by its script's name); any other condition fails.
The rest of those functions' records (the quest log, the compass, the timers, the dialogues' buttons) is run as nothing.
Until #726 the game read the records off the trees, every branch of a condition alike and no `CallFunktion` followed: a quest was entered where the start script itself triggered it and its trigger function was empty, and a trigger with records was taken as failing; so the Daemon's tutorial guide Anducar was missing, whose quest 1 its own `OnSetUp` triggers.
Of the other records, the game follows a `SetNPCState`'s level, dialogue, flags word, side, trades and home (#578, #584, #662; docs/re/netscript.md) and a `CreateNPC`'s level, group and hostility tags (#540, #669).

## Why

How the creature's handler walks an NPC to its target (its way, its gait) is not read: the hero's way and walk stand in for it.
The records that the game does not follow yet do nothing; the conditions of no meaning yet fail, as a condition the client cannot test does.
