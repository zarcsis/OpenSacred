# Bones that no clip moves

Animations (glTF), 2026-10-16.

## The original

A bone that no track of the playing clips moves takes its bind pose (Granny builds the pose from the rest pose every frame; hypothesis).

## Ours

A clip holds every bone that another clip of the same file moves in its bind pose, with constant channels (21,384 bones).

## Why

An engine keeps what the last clip left on a bone that the next clip does not move.
