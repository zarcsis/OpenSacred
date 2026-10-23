# The fight's sounds' volume and pan by a plain falloff

Sound (`opensacred-game`, `voices`, `sfx`), 2026-10-23 (#711, #712).

## The original

A voice plays at 0.45 + (`rand() % 10`) / 90 (`0x6852B0`, docs/re/sound.md, "The fight's sounds").
With the 3D provider off, as the driver of #448 runs it, `playSFX` (`0x693510`) plays it as a 2D sample whose volume and pan `0x678260` takes from the creature's place against the listener's.
That function measures the distance with an integer approximation, caps it at a hearing distance (the sound manager's `+0xB790`, times a factor), lowers the volume along a curve of three points (`+0xB7A0` to `+0xB7AC`) and pans by the creature's side over another distance (`+0xB798`).
Those values are set at run time; they are not read yet, nor the units of the places.

## Ours

The game lowers a voice's or a weapon's sound's volume in a straight line from full at the hero to silence 1,600 world units away (about 30 cells), and pans it by its offset along the screen's width, fully to one side at 600 units (about the screen's half width at the zoom of 1).

## Why

The original's curve waits for its values, read in the running original (#731).
Until then the fight about the hero sounds at about the original's volume, and the lines of creatures off the screen fade.
