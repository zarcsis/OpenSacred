# A scripts' teleport of a mounted hero dismounts the hero first

Scripts and the engine (the runtime's `Teleport`, after #726), 2026-10-25 (#954); the maintainer's decision.

## The original

`sdk_ENGINE_TP` (`0x54D9D0`) does not place a hero who rides: it teleports the horse instead and answers 1, whatever the horse's teleport answered.
The horse's teleport moves nothing into a dungeon's sector, and `Teleport` (46, `0x491D40`) tries other cells only on an answer of 0.
So a mounted hero sent into a dungeon by a script stays where he is, and the script goes on as if he had moved (docs/re/netscript.md, "Order traps").
Quest 5003's altar deletes its trigger and then teleports the hero into the dungeon sector 15,85, where it makes the god, so a mounted hero is left outside with the quest stuck (that the trigger fires for a mounted hero is community-reported).

## Ours

The runtime's `Teleport` of a hero who rides dismounts the hero first, leaving the horse where it stands, and then places the hero as for one on foot.
The game's hero does not ride yet: since #1002 the runtime's `Teleport` places the hero itself, and the dismount comes with the riding.

## Why

A script that teleports the hero expects the hero to arrive.
The client's answer of success for a teleport that moved nothing is a defect of the engine, and it can leave a quest with no way on.
