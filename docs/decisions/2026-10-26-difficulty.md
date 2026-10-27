# The difficulty

The rules and the choice of the hero (`opensacred-game`, `opensacred-rules`), 2026-10-26 (#1086).

## The original

A game has a difficulty from bronze to niobium, which the choice of the hero sets as Start Game starts it (docs/re/fight.md, "The difficulty").
The Ancaria campaign's new hero starts on bronze alone; a hero of the saves, which the Underworld campaign and the import offer, may start one difficulty above the highest it won, silver at least.
About seventy functions read the difficulty: the balance's rows by its column, a factor of 1.0 to 2.34, and rules of their own in the fight, the levels, the experience, the items, the AI, the scripts and the interface.

## Ours

The choice of the hero starts its new hero on bronze, as the original's, the difficulty's arrows changing nothing.
A game named by the command line, the console or a script may take another (`new-game --difficulty`), in place of a hero of the saves that won the difficulty below, which the game cannot load yet (#1095, #1117).
The game follows the difficulty in the balance's rows of the fight by its column, the scaling of the creatures' levels and the sectors' bounds, the experience's factor and cap, the levels on bronze's scale that the experience and the creatures' test of the levels weigh, the drops and the death's toll of gold.
It does not follow yet the other readers, which #1125 lists: the stats of a hostile creature on platinum and niobium, the potions' healing by `PotionDiv`, the quests' rewards and the scripts' tests of the difficulty, the items' sockets and sets, the specials' chances and the terror of the effect 852.
The cap of a creature's damage to a player follows the difficulty since #1124.

## Why

The new hero's bronze is the original's own; `--difficulty` lets the higher difficulties be played and checked before the saves bring the heroes that open them.
The readers left wait for the systems they belong to, the potions drunk, the items' sets, the quests' rewards, or are rare specials; the rows and the levels make most of the difference between the difficulties.
