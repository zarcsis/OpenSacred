# Quests that cross others leave them able to end

Scripts (`opensacred-convert netscript`), 2026-10-25 (#935, #946, #947, #948, #953); the maintainer's decision.

## The original

Some quests' records take away what another quest needs, testing nothing of it (docs/re/netscript.md, "The community's quest defects"):
- **9009 (Whisky Supplier):** its target's dialogue first tests whether the hero carries the barrel 17713, quest 2203's reward, and offers to buy it, so that 9009's win (bit 25 of `goodies`) is never reached while the hero carries it (#935).
- **55's start (`Init55`):** it removes the people of quests 301 and 302, the farmers who hold 301's win among them, so that an open 301 whose orcs are beaten can no longer be won (#946).
- **55's end (`NimmtDaemonenhorn`):** it removes the group 501, the nobles through whom both of 501's wins go (#947).
- **72's `BadToGood`:** it makes quest 1702's target 17215 a peaceful creature, whose death 1702's win needs (#948).
- **2002's arena:** the building's exit clears the bit that offers quest 2002, testing nothing of the quest, so that leaving the building before taking it leaves it never offered (#953).

## Ours

`opensacred-convert netscript` mends the records as it writes the trees (`script_repairs`):
- `Dialog:rg22_goodie1_ziel` first tests 9009's win (bit 25 set, bit 26 clear) with the win's line and button, in 11 trees;
- `Init55` loses quests 301 and 302 before it removes their people, in 9 trees;
- `NimmtDaemonenhorn` loses quest 501 before it removes the nobles, in 9 trees, and a new `QIS_OnLose501` beside `QIS_OnEnter501` writes the failure's notice and line, as 301's loss does, in 11;
- after `BadToGood`, `ScriptAnimShareef` gives 17215 back the hostile side and the mode word of his `CreateNPC`, in 9 trees;
- the arena's exit `OMO-1tpraus20.2c` runs `monster2002` only once quest 2002 is entered (bit 1 of `2002`), in 20 trees.

A `LoseQuest` does nothing to a quest not entered or already finished, so the lost quests are only those still open.

## Why

Each record takes a quest's way to its end away while the quest stays open, and nothing in the game tells the player so.
The repairs change those quests' courses:
- 9009 can be won with the barrel carried;
- 301, 302 and 501 end as lost when their people go;
- 1702's target can be fought again;
- 2002 stays on offer until it is taken.
