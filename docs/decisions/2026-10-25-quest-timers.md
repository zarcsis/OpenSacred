# Quests 1202 and 1803 last the days that their givers state

Scripts (`opensacred-convert netscript`), 2026-10-25 (#931, #932); the maintainer's decision.

## The original

`SetTimer`'s `0x38` counts minutes of the game's clock (docs/re/netscript.md, "`SetTimer`'s minutes and the calendar").
- **1202 (Magical Thistle):** `QIS_OnEnter1202` sets `zeit1202` for 4,320 minutes, three days. The giver's start dialogue states four days, while the log's start and the giver's dialogue while the quest is open state three (#931).
- **1803 (Kalim-Khor):** `QIS_OnEnter1803` sets `zeitum1803` for 10,080 minutes, seven days, the usual way of taking the quest from the giver, and for three days only where the quest's item was picked up first. The giver's start dialogue states three days (#932).

## Ours

`opensacred-convert netscript` writes 1202's timer for 5,760 minutes, four days, and 1803's second timer for 4,320, three days (`script_repairs`), in the 11 trees that hold the quests.

## Why

The giver's start dialogue is where the player learns the limit.
The converter has no clean way to change the number in a dialogue, so the timers follow the dialogues:
- 1202 lasts a day longer than the log states;
- 1803, taken the usual way, lasts four days less than before.
