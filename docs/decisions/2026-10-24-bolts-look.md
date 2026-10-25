# The Seraphim's bolts: their look and their shots' clips

The combat arts (`opensacred-game`'s `bolts` and `attack`), 2026-10-24 (#754).

## The original

With the effect of 1019 each of the Seraphim's shots plays the slot 164 at the rate 1.0 and makes two objects 801 (`FX_RAILGUN`), and with the effect of 1018 a shot beyond 200 plays the slot 145 at 1.5 times its rate and makes one object 875 (`FX_BFG`) (docs/re/arts.md, "Her shots with 1018's and 1019's effects").
Each object is a particle system, of 15 particles for the railgun and one for the BFG, with a light of radius 100 and 120 (docs/re/arts.md, "The Seraphim's bolts").
The two railgun bolts were seen leaving 0.1 s apart.

## Ours

Each bolt is drawn as a ball of one colour, unlit, a small blue-white one for the railgun and a larger green one for the BFG, without a light.
The shot plays one of the hero's attack clips, at the rate of the slot the original plays, and the second railgun bolt leaves 0.1 s after the first whatever the clip.
The bolts fly, end and hit as the original's.
The Dark Elf's souls of his soulcatcher 1044 (#757), particle systems too, are drawn so: a small pale ball each; so are the Daemon's bolts of 1103 (#760), a violet ball each.

## Why

The game makes none of the original's particle systems yet, and a bolt that cannot be seen flies as the original's but plays as no shot at all; the slots 164 and 145 are not among the clips that the hero's graph holds for its arts, and the bolts' flight and hits do not depend on the clip but for when they leave.
