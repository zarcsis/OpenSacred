# Rotation curves as splines

Animations (glTF), 2026-10-16.

## The original

A rotation curve blends unnormalized control quaternions and normalizes the result.

## Ours

A cubic spline through the normalized curve, with keyframes added where it strays by more than 0.001°; within 0.0025° of the original on every curve.

## Why

glTF wants unit keyframe values, so the exact quadratic of unnormalized controls cannot be stored.
