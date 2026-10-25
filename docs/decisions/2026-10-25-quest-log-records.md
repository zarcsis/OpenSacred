# Quests 1303, 1401, 10255, 1001 and 670 log what happens to them

Scripts (`opensacred-convert netscript`), 2026-10-25 (#937, #940, #941, #942, #950); the maintainer's decision.

## The original

Five quests write log lines that contradict their course (docs/re/netscript.md, "The log contradicting itself"):
- **1303 (Goods Messenger):** its timer `zeit1303` writes the failure line and then the generic completion line. The target's dialogue `ziel1303` tests nothing, so it still takes the goods, writes the completion line and gives the reward (#937).
- **1401 (Ancarian Snow):** its one-day timer `zeit1401` writes the completion line alone, while the quest stays open (#941).
- **10255 (Arachnophobia):** a failure writes its line without losing the quest, and the restart and the win write none, so the log ends on the failure line even once the quest is won (#940).
- **1001 (Captain Pigface):** `QIS_OnEnter1001` writes the line of the kill as the quest starts the usual way, while the target lives, and none where the target's item was taken first (#942).
- **670 (Glubba-Jar):** Glubba's death function `lose670` is empty, so his death leaves the quest open with no way to finish it, and `QIS_OnLose670` writes the completion line on a loss (#950).

## Ours

`opensacred-convert netscript` mends the records as it writes the trees (`script_repairs`):
- `zeit1303` loses its completion line and takes the target 17177's dialogue away, as the target's own dialogue does at the delivery, in 11 trees;
- `zeit1401` loses its line and runs nothing, in 11 trees;
- `btn_accept_10255_REstart` writes the quest's task again (`DQ_10255_LOG_HEADER`), in 20 trees, so each round of a failure and a restart takes two of the entry's ten lines;
- `QIS_OnEnter1001`'s line of the kill takes the place of the `ELSE`'s `NOP`, so that it is written where the target's item was taken first, in 11 trees;
- `lose670` loses the quest, as `tot301` loses 301, and `QIS_OnLose670` writes the generic failure line, in 9 trees.

## Why

Each record contradicts the quest's own course: a completion that is a failure, a completion while the quest stays open, a kill not yet made, a death that leaves the quest neither won nor lost.
The repairs change what the log says, and, for 1303 and 670, how the quest can end: 1303's goods can no longer be delivered after the failure, and 670 is lost at Glubba's death.
