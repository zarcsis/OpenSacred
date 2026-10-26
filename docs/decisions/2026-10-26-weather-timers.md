# The weather's times span their range, and the snow stops

The weather (`opensacred_rules::weather`), 2026-10-26 (#1083); the maintainer's decision.

## The original

The choice of the weather (`0x625290`, docs/re/weather.md) rolls a time off or on for each of rain, fog and snow from a sector's range, with the next byte `b` of a table of 256:
- the time is `shortest + (longest − shortest) / b`, so that it is nearly always the shortest, and infinite for a byte of 0, which the table holds twice: after some hours of play a kind's timer may never end, and that kind stays on or off for the rest of the run;
- the snow is taken for on by the rain's flag (`0x625459` tests `& 1`), so that in a sector of snow without rain it starts again, at an intensity rolled anew, at the end of each time on, and never stops.

## Ours

- A time is `shortest + (longest − shortest) · b / 255`: the shortest for 0, the longest for 255, every byte a time between.
- The snow goes on and off by its own flag, as the rain and the fog do.

The rolls, their table and their order are the original's.

## Why

Both read as slips: a range of times whose longest is never taken, a weather frozen for the rest of a run, and a snow that never stops are what no sector's entries ask for.
They change only what is drawn and heard; no rule of the play reads the weather.
