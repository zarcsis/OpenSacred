# A group's change gives each member its own word

Scripts (`opensacred-game`'s `npcs`), 2026-10-26 (#1021).

## The original

A `SetGroupState` without a removal writes its flags word into each member of the group in turn, each member keeping its own bits `0x6682304`, but the word so written is kept for the next member, so that every later member also takes the kept bits of the members before it: a boss's `0x80000`, a raised dead's `0x2000000` (docs/re/netscript.md, "`SetGroupState`").
Its `0x60` puts the bit `0x400000` into that carried word rather than into the member's, so that only the members after the first take it, and a member destroyed already takes the list's last id out of the group with it.

## Ours

Each member takes the record's word with its own kept bits alone (`opensacred_rules::ways::grouped_word`), the `0x60`'s bit is not set, and the group's list loses no id but those of the members removed.

## Why

These are slips of the handler, not a design: a boss among a group's members would make every member after it a boss, and a raised dead's bit would keep the later members out of their group's `GroupIsDead`.
In the Seraphim's tree one record has a `0x60` (1401's smugglers of the group 14011), and the bit `0x400000` is read by nothing the game follows.
Where the original's play shows otherwise, the driver of #448 can tell, and the choice can be undone.
