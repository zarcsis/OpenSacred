# The way goes round a wall's corner into a house, where the original's hero stops at the door

Walking (`opensacred-rules`, `opensacred-game`), 2026-10-22 (#476).
Adds to [2026-10-18-walking](2026-10-18-walking.md), whose way does not slip between two cells that touch at a corner.

## The original

Seen with the original's driver (#448) at the merchant's house of sector 50,40, its door (3256, 2578) open, the hero sent by a click onto the floor 1 beside the door (docs/re/movement.md, "The way"):

- From (3258, 2584), below the wall, to (3258, 2577) or (3258, 2576), the hero walks to (3257, 2579), the exit at the door's right, and stops there, its walk ending.
  From that exit the shortest way in is the diagonal onto the entrance (3256, 2578), past the corner of the wall's cell (3257, 2578).
- From (3255, 2584) or (3254, 2582), left of the door, to (3258, 2577), it comes onto the entrance straight up the grid and goes in.
- To (3259, 2576) from (3258, 2584), it goes round by (3256, 2579) and in, through (3258, 2576).

**Hypothesis**: the client's search (`0x4FABE0` under `0x4F7710`, not read in detail) takes the diagonal past a wall's corner, which the walk does not step, so that the hero stops short of a target it could reach, from some starts and not from others.

## Ours

The game's way never steps between two cells that touch at a corner (`opensacred_rules::ways`), so it goes round by (3256, 2579) onto the entrance and in from every start: the hero reaches the floor beside an open door wherever it is sent from.

## Why

The original's stop is a flaw of its way rather than a rule of the play: it leaves the hero outside a house it can walk into, by where it was clicked from.
Going in spares the player a second click and changes nothing else; the maintainer chose to keep the game's way (#476).
