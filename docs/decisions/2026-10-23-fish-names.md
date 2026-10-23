# The waters' shoals of fish named

Texts (`opensacred-convert text`), 2026-10-23 (#684); under the maintainer's decision of [2026-10-23-text-repairs](2026-10-23-text-repairs.md).

## The original

25 of the 474 creature types have no name in the English `global.res` (docs/02 §8).
The scripts give every creature of those types that they make a name of its own, so the types' missing names show nowhere they reach.
The fish 164 and 165 are the exception: the sectors' runner puts them in the waters without a script, beside the crabs 163, which have a name (docs/re/spawning.md, "The waters' crabs and fish").
Each is a shoal in one model: three small fish and a middle-sized one in 164 (`fisc_01.grn`), three small fish in 165 (`fisc_02.grn`).
Whether the original shows a fish's name on hover is not read.

## Ours

`opensacred-convert text` adds two strings under the hashes of the types' numbers: 164 is the **Shoal of Fish** and 165 the **Shoal of Small Fish**.
The other types without a name get none: no script makes the 0, 166, 167, 599, 706 or 1879, and the scripts name the rest.

## Why

The fish are the only creatures without a name that the scripts and the runner make, and a name harms nothing where it does not show: once the game names the creature under the cursor (M5), a fish shows a name of the same kind as its crab's.
The names describe the models, as the project's own words (docs/02 §8).
