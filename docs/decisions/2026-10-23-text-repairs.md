# The texts' defects repaired with words of the project's own, the goblins' god's name first

Texts (`opensacred-convert text`), 2026-10-23 (#403, #683); the maintainer's decision.

## The original

The English `global.res` of 2.28 has defects that the community reports and docs/02 §8 confirms on the data: the goblins' god keeps a German name, a goblin's form of the verb for stomping, 14 times in ten strings that no English string renders; 25 creature types and 566 `Weapon.pak` records have no name; four quest-log keys and one numeric reference resolve to no string; and the eleven gameboxes share one generic name.
No installed file holds the right text for any of them.

## Ours

`opensacred-convert text` repairs them as it writes `global.ftl`, with English words written for OpenSacred, never another release's or a mod's wording (docs/02 §8, "Repairing the texts without carrying them").
The repository carries the strings' ids, the original words' lengths and SHA-256 hashes and the project's words, never the game's text (docs/08 §3); a repair applies only to the text of 2.28 that its hash names.
Claude Code writes the words in the pull requests that add them, and each pull request lists them; the maintainer changes any of them through an issue.

The first repair (#683): the goblins' god is called **Stompa** in the ten strings, the first part of their one compound included: the German name's form, taken from the English verb.

The other defects are sub-issues of #403, each with its decision: the creature types' names (#684), the items' names (#685), the gameboxes' names (#686) and the scripts' keys that resolve to nothing (#687).

## Why

The maintainer chose to repair the texts, and of the four sources weighed (the same file, another installed language, the project's own words, an overlay the user supplies) the project's own words (#403): none of the defects has its right text elsewhere in the file, another language's file is outside the manifest and may not be installed, and an overlay would make the result depend on the user's copy and its terms.
A text written by the project is no longer the original's: each word is a choice of translation, recorded here or in the sub-issue's decision.
