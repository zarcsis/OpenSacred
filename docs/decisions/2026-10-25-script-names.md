# The scripts' misspelt function names and a swapped id mended

Scripts (`opensacred-convert netscript`), 2026-10-25 (#944, #949, #955); the maintainer's decision.

## The original

Some records name a function that no tree defines, a misspelling of a function that is defined and named by nothing else (docs/re/netscript.md, "The community's quest defects"):
- in the 11 trees that hold quest 1401, its soldier 17181 names the death function `attack1401`, where `attacke1401` is defined (#944);
- quest 1402's 13 smugglers name `schmugler1402`, where the function is `schmuggler1402` (#944);
- in all 20 trees, the decline button of quest 5003's seeker names `ablehnen5000`, where `ablehnen5003` clears the seeker's bit; a declined seeker keeps it, and the quest taken later from the shaman hires him (#955).

A record that names a missing function runs nothing.
Quest 602's `ok1_602` walks its knight 17136 once and then names 17163, the same digits swapped, in its three `NPC_Goto`s, its `Partikel` and its call of `ritterweg_602`, which deletes 17163 (#949).
17163 is quest 1202's target, some 1,470 cells away, so beating the knight removes 1202's target and its trade.

## Ours

`opensacred-convert netscript` mends the records as it writes the trees (`script_repairs`):
- every operand that names `attack1401`, `schmugler1402` or `ablehnen5000` names `attacke1401`, `schmuggler1402` and `ablehnen5003`, in a tree that defines the function meant and not the misspelt one;
- in `ok1_602` and `ritterweg_602`, the five operands and the one operand `res:17163` name `res:17136`.

A tree whose records are not what a repair expects is left as it is, with a warning.

## Why

Each record is a slip of the data: a function defined and named by nothing beside a record that names it misspelt, and an id whose function names the knight just before it.
The repairs change quests' courses:
- 1401's soldier's death now runs its function;
- 1402's smugglers' function now runs;
- a declined seeker is no longer hired;
- 602 no longer deletes 1202's target.
