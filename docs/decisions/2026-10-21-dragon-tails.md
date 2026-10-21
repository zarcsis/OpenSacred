# The large dragons' tails

Models (the converter), 2026-10-21.

## The original

The eight large dragons, `DRAGON.GRN` to `DRAGON_UNDEAD.GRN` (models 1193 to 1200), share one mesh `Dragon` whose tail's root is weighted wholly to `Bip01 L Thigh` or `Bip01 R Thigh` on its sides and underside, next to positions wholly on `Bip01 Tail` (docs/02 §4.6).
In each of their three sets of clips those are the most stretched sides of the mesh, 4.7 times in the walk and 10.6 in the dying clip: the surface between the tail and the thighs tears into long thin triangles that follow the legs.
The community reports a defect at the start of the large dragons' tails (**community-reported**); which weights its fix gave is not known.

## Ours

`opensacred-convert models` blends the tail into the thighs over two rings of positions wholly on a thigh (#400).
The six that share a side with a position wholly on `Bip01 Tail` take 2/3 of the tail and 1/3 of their thigh, the six that share a side with those 1/3 of the tail and 2/3 of their thigh; the positions themselves do not move.
A record of those names whose mesh does not hold those rings at the places the repair knows is written as it is, with a warning.

## Why

The repair of the data belongs to the converter, not to the game (CLAUDE.md, "Code conventions").
The rings and the shares are chosen from the data: with them, no side at the tail's root stretches beyond the most stretched sides of the rest of the mesh in any of the dragons' clips (at most 3.9 times, against 10.6 before), while one ring alone leaves 4.3 to 6.5 in the dying clip.
The original's own weights at the rest of the root (half on the tail and half on the spine or a thigh) blend in the same way.
The play does not change; the dragons' look in motion follows the rest of their mesh.
