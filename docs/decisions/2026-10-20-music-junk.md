# The music files' junk before their first frame

Sound (the converter), 2026-10-20.

## The original

154 of the 173 music files of `mp3/` hold bytes before their first frame: zeros, and in two jingles the name of the jingle before them (docs/02 §10.3); the Miles decoder searches for the sync word, so they play.

## Ours

`opensacred-convert sound` writes each music file from its first frame (#367), as it does the samples of `sound.pak` (docs/decisions/2026-10-20-mp3-leading-zeros.md), so `assets/sound/music/` holds plain MP3 files.

## Why

Symphonia skips the junk too but warns at every file that starts; the repair of the data belongs to the converter (CLAUDE.md, "Code conventions").
The sound does not change.
