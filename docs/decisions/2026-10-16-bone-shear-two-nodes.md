# Bones with shear: two nodes

Models and animations (glTF), 2026-10-16.

## The original

A bone with shear in its bind pose or in a track (1,202 bones of the converted files).

## Ours

Two nodes: an outer one with the translation, the rotation `R · U` and the scale `D` of `SS = U · D · Uᵀ`, and the bone's own node turned by `Uᵀ`; the bind pose is exact.
A track whose scale/shear has shear (1,532 tracks) is sampled at its keys and at least every 1/60 s, glTF blending the samples linearly.

## Why

glTF scales along a node's own axes only; the eigenvectors of an interpolated matrix cannot be interpolated by glTF, so those tracks are sampled.
