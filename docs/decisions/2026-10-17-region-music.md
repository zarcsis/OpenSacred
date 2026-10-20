# The regions' music

Sound (`opensacred-game`), 2026-10-17; changed 2026-10-18 (#56).

## The original

Each sector names its music, which the client plays in peace and not at night, with a variant of `PEACE` where a sector names none, overridden in the sieged villages and by the scripts' variables (docs/re/sound.md, "The region's music").
At night, when a track ends, it plays a jingle of the group `JINGLE_FIGHT_OVER` and the ambient loop.

## Ours

The music of the hero's sector, switching at once as the sector's music changes, the track playing fading out (**hypothesis**: what `cMSS::playMusic` does with it is not read); a track that ends plays again; in a sector without music the tracks of the `PEACE` group play one after another, a random one first.
By night no track starts and the one playing plays to its end, without the jingle.
No sieges' or world changes' overrides and no fights' music.

From 2026-10-17 to 2026-10-18 the tracks of the `music` profile's first group (`MUSIC_WOOD01`–`03`) played one after another everywhere, before the sectors' music was read.

## Why

The overrides follow the campaign's quests and the scripts' variables (#55), which the game does not run, and where the client switches to the fights' music is not read.
