# The waters' crabs and fish by the loaded sectors' cells

Spawning (`opensacred-game`, `npcs`), 2026-10-23 (#575).

## The original

A sector that comes in with no creatures kept puts crabs and fish on its shallow liquid cells near a shore (`0x4939F0`, docs/re/spawning.md, "The waters' crabs and fish").
For each of its cells the runner tests the two rings about the cell, up to two cells away.
At the sector's edge these reach into the next sectors, which the world loads if it must (`0x634340`).

## Ours

The game makes a sector's crabs and fish once no sector is building after it comes in, and tests the cells of the sectors loaded then.
A cell whose rings reach a sector not loaded takes none.
The hero sees about nine sectors; a sector at the edge of them, which just came in, has three or five neighbours not loaded.
So the two rows or columns of its cells by those neighbours, where a river runs on into them, take none of their crabs and fish, up to one in 16 of the sector's cells.

## Why

The game streams the sectors that it builds and draws (`sectors`), and reading a sector's cells alone for the rings would mean a second reader of the sector files beside the streamer's.
The difference is a few fish on the edges of a sector's waters, made or not by a roll of one in 11 anyway.
