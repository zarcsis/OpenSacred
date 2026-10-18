# Four bone influences a vertex

Models (glTF), 2026-10-16.

## The original

Up to 7 bone influences on a vertex (60 vertices in 16 meshes).

## Ours

The 4 largest, renormalized.

## Why

`JOINTS_0`/`WEIGHTS_0` carry 4, and Bevy reads no more.
