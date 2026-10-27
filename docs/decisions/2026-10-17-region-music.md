# The regions' music

Sound (`opensacred-game`), 2026-10-17; changed 2026-10-18 (#56) and 2026-10-27 (#1177).

## The original

Each sector names its music, overridden in the sieged villages and by the scripts' variables (docs/re/sound.md, "The region's music").
By day the music and the ambient loop of the place take one stream in turns of 180 s, crossfading over 7 s, the first by a roll; the dusk and the night ask for the ambient loop and hold the turns; a sector without music keeps the ambient loop; the death's music and the jingles play once over the stream, on a stream of their own (docs/re/sound.md, "The peace stream's alternation").
A fight's music cuts in and out of the stream, with jingles at its start and end.

## Ours

Since #1177 the peace stream alternates as the original's, its timer and its switches the rules' (`opensacred_rules::music::Stream`): the music on its channel (`music`), the ambient loop on another (`atmosphere`), each rising as the other falls over the same 7 s, which is one stream to the ear.
It departs where nothing in the game reads more:

- The ambient loop changes with the nature under the hero in its phase, after 2 s on the other ground (**hypothesis**: the original chooses it again at the region's events).
- The spot loops of the cemetery and the arenas (`ATMOSPOT_*`) take the music's channel while the hero stands in their sectors, the turns held (**hypothesis**: the original plays the arenas' on a stream of their own and asks for the ambient loop at the cemetery).
- The music's volume of 0.001 or less, which takes the ambient loop always, and the factors of the ids (`0x697820`, taken as 1) are not followed: the game has no music's volume yet, only the music on or off.
- No fights' music and no jingles yet, so no `PEACE` variant after a fight; a track left is not resumed where it was.

From 2026-10-17 to 2026-10-18 the tracks of the `music` profile's first group (`MUSIC_WOOD01`–`03`) played one after another everywhere, before the sectors' music was read; until #1177 the region's music played by day beside the ambient loop on its own channel, none by night, and the `PEACE` tracks one after another in a sector without music.

## Why

The fights' music waits for the fight's states it follows (`0x695E70`, `0x678F80`), and the volumes for the game's options.
