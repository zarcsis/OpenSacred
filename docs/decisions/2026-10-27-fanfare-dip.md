# The music does not dip under a quest's fanfare

The sound (`opensacred-game`, `music`), 2026-10-27 (#1243).

## The original

As an `ExitQuest` of a side quest sounds its fanfare in the music's phase or under a fight's music, the sound manager asks the music to fall to half its level over 0.5 s (`0x6792A0(0.5, 0.5)`), and to rise back to its level over 5 s once 5 s have passed (`+0xB7D8`, `0x6792A0(5.0, 1.0)`; docs/re/sound.md, "Scripts and notices").
The fall stops after its first frame: the update that moves the volumes (`0x6794C0`) ends the ramp once both its rates are at most 0.001, and a falling rate is below 0, so the music drops by its level times one frame's time, some 1.7 % at 60 frames a second, and stays there until the rise.
Only a music whose stream stands below half its factor moves at once, up to half.

## Ours

The music keeps its level under the fanfare: neither the dip nor the rise.

## Why

The dip the original plays is a step of one frame, inaudible, and the rise brings back what it took; reproducing it would need a volume ramp on the music's channel for a change no one hears.
The fanfare itself, its roll and its stream are the original's.
