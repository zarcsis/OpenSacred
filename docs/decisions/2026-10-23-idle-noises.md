# The creatures' idle noises

Sound (`opensacred-game`), 2026-10-23 (#714).

## The original

The sound manager's update gives the creatures near the hero their idle noises at each world tick (docs/re/sound.md, "The idle noises"): one in 20 for those hostile to the hero, by a state that it keeps from their counters of standing, moving and striking, whose group follows the state (`APPROACH` at rest, `ATTACK`, `FOLLOW`, `FOLLOW_HEROFLEE`, `FLEE`); one in 70 for the others, always of the group 0.
It follows at most 32 hostile creatures and 256 others, a record of the flag 4 or 8 takes a path by the type's name, the townsfolk's murmurs wait for a flag of the record, the types 702 to 705 choose their run by a field of the manager, and a failed roll of the hostile list picks the profile of the record before it.

## Ours

* A creature hostile to the hero makes its noise only while it does not fight, of the group 0: the original's state of rest, approximated; the states of moving towards or away from the hero and of striking, and their groups 1, 4, 5 and 6, are left out.
* The lists have no limit; the path by the name, the flag 2 and the record before are left out; the types 702 to 705 play the run of the manager's `+0x8C` not 1, and 52, 74, 88 and 100 to 103 the group 0.
* A noise plays within the hearing, 900 pixels, where its volume is above 0; its id waits after it whatever the wait.
* The volume, with the case's own added, is at most 1.

## Why

The states come from counters that the creatures' updates and the sound events feed in ways not yet read; the fight already gives the creatures their lines of `ATTACK`, and a creature that does not fight is at rest or wandering, which the state 0 covers most of the time.
The limits, the name's path and the flags concern few creatures and are not read; a sound beyond the hearing is not heard.
