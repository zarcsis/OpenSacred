# Quests 2203, 7619 and 5005 end once, as their logs say

Scripts (`opensacred-convert netscript`), 2026-10-25 (#936, #938, #939); the maintainer's decision.

## The original

Three quests can end twice, or without being entered (docs/re/netscript.md, "Moorbrook" and "The log contradicting itself"):
- **2203 (Pure Water):** its six objects count whatever the quest's state, and `tot2203` tests the count before the branch that enters the quest. So with the objects taken first, the giver offers the win of a quest never entered: no log, and `QIS_OnExit2203` never runs (#936).
- **7619 (Death to the Dead):** the follower keeps his death function `od_threepwood_7619`, which writes the failure line and loses the quest. Dying after the group is dead but before his last button, he loses the quest after its completion was shown; dying after the button, he writes the failure line after the win (#938).
- **5005 (Pony Express):** `losepe` loses the quest but leaves the mage 17277 the dialogue of the last leg, so after a failure he still offers the win line, the completion line and the reward (#939).

## Ours

`opensacred-convert netscript` mends the records as it writes the trees (`script_repairs`):
- `tot2203`'s first branch enters the quest first (`TriggerQuest 2203`), in 11 trees;
- `od_7619`, as the group's death gives the follower his last dialogue, gives him the death function `od_threepwood_7619_sieg`, a new function of quest 7619 with no records, in 11 trees;
- `losepe` gives the mage his dialogue `NPC_Dialog_Magician` back before it loses the quest, as `exitpe` does, in 20 trees.

## Why

Each quest's records let it end against its own course: won without being entered, lost after being won, won after being lost.
The repairs change those courses: 2203 is entered as its win is offered, 7619's follower dies without the quest once it is won, and 5005's mage no longer rewards a lost quest.
