# Sound: music, atmosphere and the ground

Recovered from the GOG `Sacred.exe` 2.28 with capstone on 2026-10-17. The formats are in [docs/02 §10](../02-data-formats.md#10-sound-music-video), the natures of the cells in [docs/02 §5.2](../02-data-formats.md#52-a-sector-in-sectorswldx-the-32-b-floor-cell). Status: **confirmed** on the code unless marked otherwise.

## Sound names

A table in `.data` at VA `0x964870` names every sound id: 6,869 records of `{u32 id; char name[64]}` from `SOUND_FX_INVALID` (docs/02 §10.2). The music files carry the same names (`MUSIC_WOOD01.mp3` is `SOUND_FX_MUSIC_WOOD01`, id 6,560), so a sound id names a sample of `sound.pak` or a file of `mp3/` alike. `opensacred-convert sound` writes the table to `sound.ron`.

## Profile groups

A second table in `.data`, at VA `0x9D6908`, names the 8 groups of a profile for each kind of profile: 49 records of 68 B `{u32 group; char name[64]}`, six sets of eight and `SND_MAX` (8):

| Set | Groups 0–7 |
|---|---|
| creatures | `APPROACH`, `ATTACK`, `NORMALHIT`, `CRITICALHIT`, `FOLLOW`, `FOLLOW_HEROFLEE`, `FLEE`, `DEATH` |
| weapons | `MISS`, `HIT`, `PARADE`, `SHOOT`, `DROP`, `WOOSH`, two reserved |
| footsteps | `ENVIRONMENT`, `FOOTSTEP_GRASS`, `_WATER`, `_SNOW`, `_SWAMP`, `_STONE`, `_SAND`, `_WOOD` |
| objects | `USE`, `OPEN`, `KNOCK`, `CLOSE`, four reserved |
| music | `PEACE`, `NEARBY`, `FIGHT`, `JINGLE_DANGER`, `JINGLE_FIGHT_NORMAL`, `JINGLE_FIGHT_HARD`, `JINGLE_FIGHT_OVER`, `JINGLE_FIGHT_EASY` |
| generic | `SOUND00` to `SOUND07` |

The footstep profiles' samples bear the footstep names out (`FOOTSTEP_HEROMALE_GRASS01` in group 1, the horse's neighing in group 0), and so do the music profile's (below).

## Footsteps by the ground

A function around `0x68EDDA` plays a footstep: it switches on the nature (`nature − 0x10`, byte table `0x68FED0`, jump table `0x68FEB0`) and calls `0x4261D0(sound, group)` with the footstep group:

| Natures | Group |
|---|---|
| `DESERT`, `KIESROAD`, `KIES` | 6 `FOOTSTEP_SAND` |
| `ARCTIC` | 3 `FOOTSTEP_SNOW` |
| `WALD`, `GRASS`, none and any other byte | 1 `FOOTSTEP_GRASS` |
| `SUMPF` | 4 `FOOTSTEP_SWAMP` |
| `LAVA`, `BRIDGE`, `DUNGEON`, `STONEROAD`, `IND-STONE` | 5 `FOOTSTEP_STONE` |
| `WATERA`, `WATERB` | 2 `FOOTSTEP_WATER` |
| `IND-WOOD` | 7 `FOOTSTEP_WOOD` |

Before the switch it skips three values of a word in `esi` (`0x22D`, `0x22E`, `0x48`; hypothesis: creature types that make no footsteps of the ground) to a path that checks for water (`0x68EE24`); not followed. `opensacred-game` plays the hero's footsteps by this table (`sound.rs`).

## Footprints

A function around `0x6392AE` loads a footprint texture by the nature of the cell (`nature − 0x10` up to `0x90`, byte table `0x639AB0`, jump table `0x639A9C`): `DESERT` `FOOTSTEPDESERT.TGA`, `ARCTIC` `FOOTSTEPSNOW.TGA`, `SUMPF` `FOOTSTEPSWAMP.TGA`, `WATERA` and `WATERB` the ripples `footstepWaterA_01.tga` to `05`; the other natures leave none. On the two waters it then compares the cell's second corner heights (`+0x18`) with the first (`+0x10`) and goes on if they differ and one of the second is above 0 (not followed further).

## The atmosphere by the ground

A function around `0x696B70` chooses the ambient loop (an `ATMO_*` file of `mp3/`) and some of the music. Its object holds the state at fixed offsets; what they are is read off the code and their use, so their names are **hypotheses**:

| Offset | Size | Meaning |
|---|---|---|
| `+0xA34` | u16 | the time of day: 1 and 2 select the night loops (hypothesis: night and dusk) |
| `+0xA36` | u16 | the nature of the cell under the hero, the cell's `+0x1F` with the low nibble 0 |
| `+0xA3C` | u32 | a bit set: the bit of the region at `+0xA40` selects `MUSIC_VILLAGE_SIEGE_UNDEAD` |
| `+0xA40` | u32 | an index; 0x6E is replaced by 0xB |

It plays a loop through `0x694510(id, 0, 0)`, `cMSS::playMusic` (its log line), which plays the music too: it takes the volume at `+0x78` for a name that starts with `ATMO` (after `SOUND_FX_`) and the one at `+0x7C` for the rest. Whether an ambient loop and a music track can sound at once is not read yet. By the nature, through a switch on `nature − 0x10` (byte table `0x696F50`, jump table `0x696F38`, entry at `0x696C27`) by day and another (`0x696E54`, `0x696E40`, entry at `0x696BDA`) by night:

| Nature | Day | Night |
|---|---|---|
| `DESERT`, `KIES` | `ATMO_DESERT` | `ATMO_DESERT_NIGHT` |
| `ARCTIC` | `ATMO_ICE` | `ATMO_ICE_NIGHT` |
| `WALD`, `GRASS`, `SUMPF` | `ATMO_WOOD` | `ATMO_WOOD_NIGHT` |
| `LAVA` | `ATMO_VULCANO` | `ATMO_VULCANO_NIGHT` |
| `DUNGEON` | `ATMO_DUNGEON` | `ATMO_WOOD_NIGHT` |
| the rest (`BRIDGE`, the waters, the roads, the floors), none | no change of loop | no change of loop |

Before the switch the function leaves the loop alone while one of the region musics plays (`MUSIC_PILZWALD`, `MUSIC_KARIBIK`, `MUSIC_HELL`, `MUSIC_WAYTOHELL`, `MUSIC_PIRATENPUB`); after it, a switch on the playing id (`id − 6504`: byte table `0x697058`, jump table `0x697034`; by night another switch at `0x696D20`) handles the rest, including `ATMO_KARIBIK` and `ATMO_PILZWALD`; not analyzed yet.

`opensacred-game` plays the day's column (`atmosphere.rs`).

The regions' share is driven by the scripts (not followed): the campaign's functions set variables `atmos_rg<N>` (`bekannter1` does `IncVar atmos_rg1`), and the client holds the string `atmo_rg` (VA `0x94F4FC`), presumably to read them; 163 lines of the campaign's listing name them. M4 recovers the link.

## The music profile

The sound profile `music` (slot 1 of `sndProfiles.pak`) names music by sound id in its groups, which the table above names: 0 `PEACE` and 1 `NEARBY` `MUSIC_WOOD01`–`03`, 2 `FIGHT` `MUSIC_FIGHT01`–`04`, 3 `JINGLE_DANGER01`–`08`, 4 `JINGLE_FIGHT_NORMAL01`–`08`, 5 `…HARD01`–`08`, 6 `JINGLE_FIGHT_OVER01`–`08`, 7 `JINGLE_FIGHT_EASY01`–`08` (docs/02 §10.3). A function around `0x697730` plays `PEACE`: `0x4261D0(1, 0)` picks a variant of the `music` profile's group 0 and returns its sound id, which goes to `cMSS::playMusic`; the same function plays the `JINGLE_FIGHT_OVER` group (6) through `0x694FE0`. Its conditions (the words at `+0x692` and `+0x698`, the time of day at `+0xA34`) are not analyzed. `opensacred-game` plays `PEACE`.
