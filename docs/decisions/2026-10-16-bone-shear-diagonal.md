# Bones with shear: the diagonal only

Models (glTF), 2026-10-16.

## The original

A bone's scale/shear is a full 3×3 matrix; 361 bones of the models have shear.

## Ours

Superseded the same day by [two nodes](2026-10-16-bone-shear-two-nodes.md): the glTF node kept the diagonal, which distorted posed wolves and other sheared rigs.

## Why

glTF node transforms are translation, rotation and scale; shear cannot be stored in one node.
