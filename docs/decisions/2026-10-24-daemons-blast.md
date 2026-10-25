# The Daemon's blast: its edge, its level and its look

The combat arts (`opensacred_rules::arts::Blast`, `opensacred-game`'s `blast`), 2026-10-24 (#760).

## The original

As the Daemon's charge 1091 lands it makes the object 1168 (`FX_KAMIKAZE`) about him, whose radius grows for 0.75 s to three quarters of its most `R` (150 + 2·L, at most 400) and which strikes each hostile creature it reaches once (docs/re/arts.md, "The objects 1168 and 842"):

- The strike's factor is `(R − d)/R · max(1, f/2)`, `d` the Daemon's distance from the creature.
  `R − d` is loaded as an unsigned 64-bit number (`0x5B1EBC` to `0x5B1EC8`).
  A creature farther than `R` from him, which the blast reaches where its size is more than a quarter of `R`, takes a factor of some four thousand million over `R`: a blow that kills.
- Then the kicks' chance of the object's level word, against the creature's level, decides its knockback.
  The FX event that makes the object leaves that word as the stack held it.
- It is drawn as a particle system of 150 particles.

## Ours

- A creature farther than `R` from the Daemon takes no strike from the blast; one within `R` takes the original's share.
- The knockback's chance takes the charge's level in play as the object's level.
- A translucent orange ring, as wide as the blast's radius, stands in for its particles.

## Why

- The unsigned difference is a defect: its blow does not follow from the factor's form, which falls to 0 at `R`, and it reaches only creatures at the blast's edge in the last tenth of its growth.
- The level word has no value of its own on that way, and the charge's level is the one the event's other values are made of.
- The game makes none of the original's particle systems yet, and a blast that cannot be seen strikes from nowhere.
