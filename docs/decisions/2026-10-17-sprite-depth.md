# The sprites ordered through the depth buffer

Rendering (`opensacred-game`), 2026-10-17; changed 2026-10-19 (#100).

## The original

The ground first, then the objects in five draw lists, each by cell diagonals (docs/02 §6.1).

## Ours

The depth buffer: every sprite is a quad in the plane of the screen at the depth of its cell's centre, the ground lies behind all objects, the underlay types (lists 0 and 2, by the mask, 4,000 and 2,000 units back) and the statics' shadows of list 1 (3,000 back) lie between, and list 4 comes 1,000 units forward.

## Why

The hero is a 3D model that must interleave with the sprites; a large sprite owned by one cell can still be ordered wrongly against the hero.
