# Quest 10260's item named, so that later quest items reach the list

Scripts (`opensacred-convert netscript`), 2026-10-25 (#957); the maintainer's decision.

## The original

The right vase of quest 10260 (`fillchest_chestQJ_03`) gives the item 4609 with `CreateObj` `0x61` and no `0x01` name, so the hero's list of main quest items gets the entry (4609, 0) (docs/re/netscript.md, "The log's quest items (10260)").
`cInventoryEntry::addMainQuestItem` (`0x5D8280`) refuses every later pair once an entry of the key 0 is in the list.
So no quest item of any quest reaches the list again:
- 154, and through it 9011, cannot be finished;
- 402 and 601 cannot be finished;
- 204 ends without its reward.

The record is identical in all 20 trees and the only one of its kind.
Raven Rock 1.3b reports mending it in the interpreter (community-reported).

## Ours

`opensacred-convert netscript` names the record's item `res:4609`, in all 20 trees, as 402 and 601 name theirs (`script_repairs`).
The runtime is to keep the client's list as it is.

## Why

The defect is the interpreter's, but one record of the data sets it off, and naming that record is the narrowest repair.
The list then works as it does for every other quest item, and the runtime need not depart from the client.
