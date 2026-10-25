# The quest log's missing lines written

Texts (`opensacred-convert text`), 2026-10-25 (#687); under the maintainer's decision of [2026-10-23-text-repairs](2026-10-23-text-repairs.md).

## The original

Four keys that the scripts' `QuestBook` records (type 53) name whole resolve to no string of the English `global.res` (docs/02 §8).
What the original's quest log shows for them is not read.

- `HQ_6_2_1_Log_preQuest`: the main quest 61 writes it as the hero takes the element of the air before the quest asks for it; the elements of fire, earth and water have a line of theirs for the same step.
- `NQ_7607_LOG_HEADER`: the heading of the quest 7607, the last of three insects that the hero kills for a tribe; the quests of the two others, 7605 and 7606, have theirs.
- `NQ_LOG_QLOSE`: the line of the quests 9009 and 9010 as they are lost (`QIS_OnLose`), beside the generic lines of a quest completed and failed, which the file has.
- `R09NQ01_Log_Qsieg`: the quest 901's line as it is won, the abducted women brought home.

A fifth reference, `res:174242`, is a script's typo that no text shows (docs/02 §8), and gets no line.

## Ours

`opensacred-convert text` adds four strings under the hashes of the keys, words of the project's own:

| Key | Line |
|---|---|
| `HQ_6_2_1_Log_preQuest` | An odd element, light and restless as a gust of wind! |
| `NQ_7607_LOG_HEADER` | The Last of the Brood |
| `NQ_LOG_QLOSE` | Quest lost. |
| `R09NQ01_Log_Qsieg` | The women are home again, safe in their village. |

## Why

The quest log (M5) would show nothing, or a key, where the scripts write these lines.
Each line says what its step is, read from its quest's script and its neighbouring keys by id, and takes the form of its siblings: a note of an element found, a heading, a short generic line, a line of the hero's log; none is another release's wording, and none names a place or a person of the game.
The maintainer changes any of them through an issue.
