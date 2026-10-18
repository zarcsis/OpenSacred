# Corners without a normal

Models (glTF), 2026-10-16.

## The original

2,476 triangle corners in 144 meshes use the normal `(0, 0, 0)`.

## Ours

The normal of their triangle.

## Why

glTF requires unit normals; how the original lights such a corner is unknown.
