# The MP3 samples' leading zeros

Sound (the converter), 2026-10-20.

## The original

73 MP3 records of `sound.pak` start with one frame's worth of zero bytes (208 or 417) before the first frame (docs/02 §10.2); its decoder searches for the sync word, so they cost at most a frame of silence.

## Ours

`opensacred-convert sound` drops the zeros as it writes the sample, so every MP3 file of `assets/sound/samples/` starts with a frame; `opensacred_formats::sound` reads the record as it is and reports how many zeros it starts with (#354).

## Why

A file that starts with a frame is a plain MP3 for any decoder, and the repair belongs to the converter, not to the reader of the original's files (CLAUDE.md, "Code conventions").
The sound does not change.
