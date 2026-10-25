# Sound: music, atmosphere and the ground

Recovered from the GOG `Sacred.exe` 2.28 with capstone on 2026-10-17.
The formats are in [docs/02 §10](../02-data-formats.md#10-sound-music-video), the natures of the cells in [docs/02 §5.2](../02-data-formats.md#52-a-sector-in-sectorswldx-the-32-b-floor-cell).
Status: **confirmed** on the code unless marked otherwise.

## Sound names

A table in `.data` at VA `0x964870` names every sound id: 6,869 records of `{u32 id; char name[64]}` from `SOUND_FX_INVALID` (docs/02 §10.2).
The music files carry the same names (`MUSIC_WOOD01.mp3` is `SOUND_FX_MUSIC_WOOD01`, id 6,560), so a sound id names a sample of `sound.pak` or a file of `mp3/` alike.
`opensacred-convert sound` writes the table to `sound.ron`.

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

## The fight's sounds

Read in Ghidra 2026-10-23 (#708, #709), **confirmed** by code unless marked, and seen with the driver of #448 (the Seraphim of level 1, the button held on an immortal goblin warrior of level 1 for three spells of 15 s, the god mode on: `trace sound-pick`, the `sound` group, a trace of `0x6852B0`).

**The voices.**
A creature speaks through `0x6852B0` (`this` the sound manager `0x6770E0()`; the creature's type `+0x3B0`, its id `+0xC`, the group, a special: 0 a line of the profile, 1 a cast's line, 2 an "unconscious" line, both of fixed ids).

- The type picks a case by a byte table at `0x68BBD0` (the types 1 to 714, the index type − 1) and the jump table at `0x68B9F8`: 118 cases, `0x75` no voice; 328 types have another case, of which the cases `0x49`, `0x4A`, `0x67` and `0x6A` (six types) name the profile 0, so that 322 types speak; `Creature.pak` has no part in it.
- Each case names the profile, a roll `rand() % N` (the line on 0; the `DEATH`, group 7, forced for most) and a wait, 1 s by default, 10 s for the cases that end the switch, else 2 to 5 s.
  The wait is kept per sound id, not per creature: a table of 128 entries at `+0xAF88` (`0x679170` tests it, `0x679120` records it).
- The heroes take one of two profiles at random at each call: the Seraphim 1008 or 1009, the Gladiator 1002/1003, the Mage 1006/1007, the Dark Elf 1000/1001, the Wood Elf 1004/1005, the Vampiress 1010/1011 (her case names 1012 too; **hypothesis**: by her form), the Dwarf 1013/1014, the Daemon 1015/1016; one in four, 10 s.
  Their profiles fill the groups 1 to 3 alone.
- **Checked** 2026-10-23 for every type (#710): `0x6852B0` called in the running original from a second Frida session for each type, the groups 1, 2, 3 and 7 and the specials 1 and 2, with `rand()` controlled and the play and the waits' test stubbed, gives each of the 118 cases the same values for all its types: the profiles, the rolls one in 1 to 7 (the thief's case one in seven for `ATTACK`, five for the hits; the death forced in every case that speaks), the waits of 1, 1.5, 2, 3, 4, 5, 10 or 20 s, and the special lines' fixed ids of 23 types.
  `opensacred-convert tables` writes them into `tables/voices.ron` by the byte table (`opensacred_formats::voices`, the cases in the converter's `voices.rs`).
- The start's creatures: the goblin warrior 76 2031 (one in four, the death always, 5 s), the thief 313 2023 (`ATTACK` one in seven, the others one in five, 10 s), the black magician 271 2033 (one in four, 10 s, cast lines), the novice 677 2055 (one in four, 10 s), the boss 689 2026 (one in four, 10 s), the bears 536–538, 591 and 600 2009 (one in two, 1 s), the wolves 512 and 567–588 2014 (one in two, 1 s), the boars 543 and 545 2004 (always, 1 s); the rabbits 516, 592 and 593, the pigs, chickens, cows and horses have none.

The groups are passed by eight calls:

| Group | When | Where |
|---|---|---|
| 1 `ATTACK` | a melee strike's start, the attack action's sub-state 0; a shot | `0x4E52D1`, `0x4DF06C` |
| 2 `NORMALHIT` | a hit that did damage, the event 5's handler after `0x56B130` | `0x532161` |
| 3 `CRITICALHIT` | such a hit when the target's sound record's `+0x2C` is below 26 (**hypothesis**: its health in per cent) | `0x532161` |
| 7 `DEATH` | death, without the roll, stopping the creature's line | `0x536389`, `0x533504` |
| specials | a cast, the unconscious | `0x51D438`; `0x53EAD1`, `0x56E428` |

`APPROACH`, `FOLLOW`, `FOLLOW_HEROFLEE` and `FLEE` are passed by no call of the fight; the sound manager's update passes them, by the creature's state, as its idle noises ("The idle noises" below).
A line plays only when its id waits for nothing (groups 1, 2), when the record's `+0x34` is 0 (3), and always (7); while the creature speaks (`0x678AF0`) the groups 0, 1, 4–6 and 3 give way, a normal hit stops the line and plays, a death always plays; one sample a creature (`0x678980`).
`playSFX` (`0x693510`) plays a voice at the volume 0.45 + (`rand() % 10`) / 90 with the distances of `AIL_set_3D_sample_distances` (100 and 10.1, or by the creature's size: size × 100 and size × 0.3 + 10; the units' scale not read); 28 channels.

- **Seen:** a strike that the held button chains (docs/re/fight.md, "The hero's attack") starts in sub-state 1, not 0, and calls no `ATTACK`: the hero's voice took group 1 three times in the 45 s of strikes, the goblin's, whose own strikes chain too, 32 times.
  Of those 32 the goblin's line (2031, group 1) was picked and started 15 times, of its 42 `NORMALHIT`s 11, of the hero's 21 `NORMALHIT`s 4 (1008 or 1009, group 2): the hits about as the roll of one in four gives, the goblin's `ATTACK` about twice as often, though `0x6852B0` picks a line only once its roll gives 0 (**hypothesis**: another caller picks the profile's group 1 too; not read).
- **Seen:** every sample of the fight started through `AIL_start_sample`, none through `AIL_start_3D_sample`: `playSFX` takes the 2D path when the sound manager's `+0x80` is 0 or the channel has no 3D sample (**hypothesis**: the driver's settings leave the 3D provider off).

**The weapons.**

- **The swing** (`WOOSH`, group 5): the sound event `0x67A7A0(5, 2, …)` at a strike's hit moment within reach (`0x4EA6FD`, in `0x4EA3F0`), and at a strike's start without a target (**hypothesis**: in the air), of a profile by the class of what the striker holds (`0x546F80`): 0 and 5 503, 1 and 2 500, 3 509, 4 505, 6 504, 8 500 or 516, 9 and 10 506, at (0.975 + (`rand() % 5`) / 90) × 0.5.
  **Seen:** the goblin's swings picked 503's group 5 at each of its strikes, the Seraphim's 500's group 5 at each of hers.
- **At the target** (`0x5320B2`–`0x532146`, `0x532731`): a hit that did damage plays the striker's weapon's `HIT` (1) by the item's profile (`items.pak +0x24`, docs/02 §6.1, read by `0x425EC0`, which falls back on a weapon of the record's kind when it is 0), the fist's (502) for bare hands, at 0.5; else its `PARADE` (2), the fist's `FIST_MISS`; the target types 512, 536, 543, 545 and 567–591 take another sound, by a name near `0x956038` (not read).
  **Seen:** the goblin's hits at the hero picked 502's group 1, its strikes that did nothing (the god mode) 502's group 2.
  The Seraphim's sword, the starting weapon `Weapon.pak` 7901 of the parent 1724 (whose `items.pak +0x24` is 500), picked the profile 0, its group 1 after a hit and 0 after a miss: nothing, so her starting sword's hits sound only by their swing (**hypothesis**: the starting weapons 7900–7909, whose `items.pak` slots are empty, take no profile; the fallback not read).
- **The shot** (`0x4DCB80`): the bow's group (**hypothesis**: 3, `SHOOT`); an arrow's hit by the weapon's type (`0x5BB010`).
- The pick `0x4261D0(profile, group)` takes one of the group's 8 slots by `rand() & 7`.

**The hero's lines**, MP3 samples by name (`0x693FE0`), part of the 3,404 MP3s of `sound.pak` (docs/02 §10.2):

- `HITENEMY_nn_<class>` (11 a class) from `0x6852B0` when a creature not the hero takes the group 2 or 3, 10 s since the last, one in five;
- `BATTLECRY_nn_<class>` (10 a class) at the sound event 5 with an argument from 1003 to 1104 (**hypothesis**: a combat art), one in three, always for the arts that play their own ids 2098 to 2104;
- `ENEMYKILLME_nn_<class>` (6 a class) at the hero's `DEATH`, with `MUSIC_DEATH` (6505, `0x694FE0`).

`opensacred-game` speaks the voices so (`voices`, #711): `ATTACK` at the first strike of the hero's attack and at a creature's strike that does not chain or its shot, `NORMALHIT` or `CRITICALHIT` (below 26 % of the health) at a hit that did damage and does not fell, `DEATH` at a death, each by its type's roll, the heroes' two profiles, a slot of 8 at random, the waits per sound id and one line a creature as above; at the volume above, lowered and panned by its place as the original does (below, "A sound's place", #731).
It plays the weapons' sounds (`sfx`, #712): a strike's swing by the class of what the striker holds at its hit moment within reach or in the air; at the target the striker's weapon's `HIT` when the hit did damage, else its `PARADE`, by the sound profile of the weapon's own type, so that a starting weapon without a slot of its own is silent there as the original's, and the fist's for bare hands; a shot's group 3 by the bow's profile (**hypothesis**: `SHOOT`); at the original's volumes, placed as the voices.
It plays the heroes' lines by name (`voices`, #713): a taunt when a creature takes a hit's line, one in five and 10 s after the last; a battle cry at the start of a combat art, one in three; the death's line at the hero's fall; one of the class's at random, as the hero's line.
It plays `MUSIC_DEATH` at the hero's fall (`music`, #734; below, "A music played once").
Not yet: the vampiress's `realvamp` lines and the arts that play their own cries.

### A sound's place

Recovered 2026-10-23 (#731).
On the 2D path `playSFX` takes a sample's volume and pan from `0x678260(id, factor, x, y)`, a method of the sound manager: the place `x`, `y`, else the creature's from its sound record (`+0xC`, `+0x10`, found by `0x6781D0`), against the listener's (`+0xAF2C`, `+0xAF30`).

- **The units:** the world's screen pixels at the zoom of 1, `48 (cx − cy)` right and `24 (cx + cy)` down for the cell `cx`, `cy` (**confirmed** against the creatures' cells: the Seraphim at 3236,2511 was the listener at 34,799 and 137,944, the creature at 3238,2518 at 34,560 and 138,167).
- **The distance:** the offsets as integers, then 1,007 of the longer side and 441 of the shorter per 1,024, less 40 of the longer while it is under 16 times the shorter, rounded (`+ 512 >> 10`); at most the hearing distance `H` = `+0xB790` × (1.5 − `+0x84`) × `factor`.
- **The volume** falls along three lines: from `+0x84` at the listener to `V1` = ((0.5 − `+0xB7A4`) × (`+0x84` − 0.5) + `+0xB7A4`) × `factor` at `+0xB7A0` × `H`, to `+0xB7AC` × `factor` at `+0xB7A8` × `H`, to 0 at `H`.
- **The pan,** from 0, left, to 1, right: with the sound right of the listener, 1 − `r` / 2, left of it `r` / 2, where `r` is the share of the (capped) distance that runs up or down the screen; straight above or below it, `r` with its sign, a quirk the next step mostly hides.
  Then it is pulled back to the middle by 1 − `d` / `+0xB798`, wholly at the listener, not at all from `+0xB798` on; the creature id 1, the hero, is always in the middle.
- **The values** (the driver of #448, the Seraphim's new game, a second-session probe of `0x678260` as creatures came at the hero, 40 calls): `+0x84` 0.75, `+0xB790` 1,200, `+0xB798` 260, `+0xB7A0` to `+0xB7AC` 0.5, 0.5, 0.9 and 0.1, the factor 1; so `H` = 900 pixels, and the volume runs from 0.75 to 0.5 at 450 pixels, to 0.1 at 810, to 0 at 900.
  Every call's volume and pan came out as above to three decimals, as the creature at 813 pixels (0.097, 0.711), the one at 322 (0.571, 0.346) and the one beyond 900 (0, 0.515).
  What sets the values is not read (**hypothesis**: `+0x84` the settings' volume of the effects, set by the driver's `--muted` to the game's 64).

`opensacred-game` places the voices and the weapons' sounds so (`sfx::placed`, #731): the offset from the hero in those pixels, the distance as the integers give it, the three lines over the same values, divided by the 0.75 at the listener so that a sound there plays at its own volume, and the pan from −1 to 1.

## The idle noises

Recovered 2026-10-23 (#714) from the sound manager's update (`0x69B770`) and its event handler (`0x67A7A0`), and checked in the running original under the driver of #448.

- **The lists.** The sound events 6 and 11 report the objects near the hero (the sub-command 0 or 1 with the object's id, place and type; 2 measures the distances again and drops those beyond the hearing; 4 and 5 set and clear a record's flag 2; 6 sets the flag 4 or 8): the sub-command 1 and the hero go into the first list, 32 records of `0x88` bytes at `+0xDA4`, the others into the second, 256 at `+0x1EA4` (a third, 16 at `+0xA6A4`, takes the event 10's objects, `+0xAF34` to `+0xAF3C` counting them).
  **Seen** (a trace of `0x67A7A0` at the Seraphim's start, 32,000 calls): the thieves and bandits, hostile to the hero, came with the event 6's sub-command 1, the rabbits, pigs and novices and the type 679 of the start, peaceful, with its 0, the birds (type 886) with the event 11, the hero (id 1) with 0 into the first list by its id.
- **The tick.** The update runs at the world's tick (**seen**: a call every 0.1 to 0.13 s, its time as the argument), and its idle noises' block when the time it gathers is above 0.05 s, so at each tick.
- **The first list**, within `+0xB790` (1,200 pixels) by the integer distance of `0x678260` (above): `rand() % 20` gives 0, the record's state `+0xDBC` is neither 2 nor 6, the creature does not speak (`0x678AF0`); the state gives the group, 0 → 0, 1 → 1, 3 → 4, 4 → 5, 5 → 6 (the names' `APPROACH`, `ATTACK`, `FOLLOW`, `FOLLOW_HEROFLEE`, `FLEE`), and the type a case through the byte table at `0x6A03C8` (the types `0x20` to `0x2CA`) and the jump table at `0x6A0240`: a roll of one in 2 to 5 or none, a profile, a wait of 0.5 to 60 s, a volume added.
  A record of the flag 4 or 8 takes another path by the type's name (not read).
  `0x68BEA0` keeps the state from the record's counters (`+0xDFE` to `+0xE08`, **hypothesis**: the ticks standing, moving and striking) and its moves against the hero's: 0 once `+0xDFE` is over 50 and the others low, 1 once `+0xE06` reaches 31, 2 once `+0xE08` reaches 2, 3, 4 or 5 by whether it goes towards the hero or away from it within 500 while `+0xE00` or `+0xE02` is over 50, and 6 when its `+0x1A` slot is under 1; a new record is in the state 0.
- **The second list**, within `+0xB790` × (1.5 − `+0x84`) (900 pixels): `rand() % 70` gives 0, the state is 0 (which nothing changes in this list), the creature does not speak and its own sample does not play (`+0x1ED8`, `0x678E40`); the type a case through a switch: a roll, a variant of a profile's group 0 or a sound id of a run at random, a wait of 0 to 10 s, a volume added.
  The townsfolk's murmurs (the runs from 13,450, 13,600 and 16,050) also wait for the flag 2 to be clear; the types 702 to 705 take one run while the manager's `+0x8C` is 1 and another while it is not (`+0x8C` not read); 52, 74, 88 and 100 to 103 ask `0x678DA0` for the group.
- **The noise** plays as a voice: unless its id waits (`0x679170`), at 0.45 + (`rand() % 10`) / 90 plus the case's volume (−0.2 to 0.7), the id then waiting the case's seconds (`0x679120`; the second list only for a wait that is not 0), one sample a creature (`0x678930`, `0x678980`).
  A roll of the first list that fails leaves the profile of the record before it, at first 0, which picks nothing.
- **The cases**, read off the two switches by a walk of the code with the type known and `rand()` free (311 types of the first list, 241 of the second): the first's profiles are the creatures' voices' (the wolves 2014 one in two, the bears 2009 and the boars 2004 one in three, the goblins 2031 one in three, 30 s, the thieves 2023 one in three, 60 s, the boss 2026 one in five, 5 s); the second's add the animals' (the pigs 2011 and the rabbits' none), the novices 2055, the townsfolk's murmurs and the children's.
  **Checked** at the start (the trace of `0x4261D0`, `sound-pick`): three pigs made 11 picks of 2011's group 0 in 64 s against 12.6 by these rolls, three thieves 38 of 2023 against about 30.
  `opensacred-convert tables` holds the cases as a table and writes `tables/idle_noises.ron` (`opensacred_data::tables::IdleNoises`).

`opensacred-game` makes them so (`voices`, #714): at each world tick a creature within the hearing rolls one in 70, or one in 20 if it is hostile to the hero and does not fight, then its type's noise, docs/decisions/2026-10-23-idle-noises.md says what it leaves out.

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

Before the switch it skips three values of a word in `esi` (`0x22D`, `0x22E`, `0x48`; hypothesis: creature types that make no footsteps of the ground) to a path that checks for water (`0x68EE24`); not followed.

Read 2026-10-19 (#141, **confirmed** by code): a creature's step, `0x639080`, calls the footstep with the nature of the ground's cell under it, but with `BRIDGE` (`0x70`) on a `WATERA` or `WATERB` cell whose second corner heights (`+0x18`, left, top, right, bottom) differ from its first and lie above 0 at the right or the bottom corner, or at both the left and the top: on a bridge over the water the steps are stone's, as the creature stands on the bridge's deck (docs/re/movement.md, "The height").

`opensacred-game` plays the hero's footsteps by this table (`sound.rs`), with `BRIDGE` on a bridge over a liquid (`terrain::Heights::step_nature`).

## Footprints

A function around `0x6392AE` loads a footprint texture by the nature of the cell (`nature − 0x10` up to `0x90`, byte table `0x639AB0`, jump table `0x639A9C`): `DESERT` `FOOTSTEPDESERT.TGA`, `ARCTIC` `FOOTSTEPSNOW.TGA`, `SUMPF` `FOOTSTEPSWAMP.TGA`, `WATERA` and `WATERB` the ripples `footstepWaterA_01.tga` to `05`; the other natures leave none.
On the two waters it then compares the cell's second corner heights (`+0x18`) with the first (`+0x10`), as for the footsteps above: on a bridge over the water and on `WATERB` it reads the byte at `+0xF8` of a table of the creature's (at `+0x17C`), else the one at `+0xF7` (not followed further).

## The atmosphere by the ground

A function around `0x696B70` chooses the ambient loop (an `ATMO_*` file of `mp3/`) and some of the music.
Its object holds the state at fixed offsets; what they are is read off the code and their use, so their names are **hypotheses**:

| Offset | Size | Meaning |
|---|---|---|
| `+0xA34` | u16 | the phase of the day, from the renderer's event of type 3 (docs/re/daynight.md): 0 the day, 1 the dusk, 2 the night, 3 the dawn; 1 and 2 select the night loops |
| `+0xA36` | u16 | the nature of the cell under the hero, the cell's `+0x1F` with the low nibble 0 |
| `+0xA3C` | u32 | a bit set: the bit of the region at `+0xA40` selects `MUSIC_VILLAGE_SIEGE_UNDEAD` |
| `+0xA40` | u32 | an index; 0x6E is replaced by 0xB |

It plays a loop through `0x694510(id, 0, 0)`, `cMSS::playMusic` (its log line), which plays the music too: it takes the volume at `+0x78` for a name that starts with `ATMO` (after `SOUND_FX_`) and the one at `+0x7C` for the rest.
Whether an ambient loop and a music track can sound at once is not read yet.
By the nature, through a switch on `nature − 0x10` (byte table `0x696F50`, jump table `0x696F38`, entry at `0x696C27`) by day and another (`0x696E54`, `0x696E40`, entry at `0x696BDA`) by night:

| Nature | Day | Night |
|---|---|---|
| `DESERT`, `KIES` | `ATMO_DESERT` | `ATMO_DESERT_NIGHT` |
| `ARCTIC` | `ATMO_ICE` | `ATMO_ICE_NIGHT` |
| `WALD`, `GRASS`, `SUMPF` | `ATMO_WOOD` | `ATMO_WOOD_NIGHT` |
| `LAVA` | `ATMO_VULCANO` | `ATMO_VULCANO_NIGHT` |
| `DUNGEON` | `ATMO_DUNGEON` | `ATMO_WOOD_NIGHT` |
| the rest (`BRIDGE`, the waters, the roads, the floors), none | no change of loop | no change of loop |

Before the switch the function leaves the loop alone while one of the region musics plays (`MUSIC_PILZWALD`, `MUSIC_KARIBIK`, `MUSIC_HELL`, `MUSIC_WAYTOHELL`, `MUSIC_PIRATENPUB`); after it, a switch on the playing id (`id − 6504`: byte table `0x697058`, jump table `0x697034`; by night another switch at `0x696D20`) handles the rest, including `ATMO_KARIBIK` and `ATMO_PILZWALD`; not analyzed yet.

`opensacred-game` plays the day's column by day and at the dawn, the night's in the dusk and by night (`atmosphere.rs`, by the game's clock, docs/re/daynight.md).

A static whose type has a sound profile (`items.pak +0x24`, docs/02 §6.1; the 22 waterfall types 15531 to 15560 have `atmospot_waterfall`, 10) sends the sound manager the event 13 with its cell and type at every frame it is drawn (`cWorldView_renderTileRow`, `0x62B9D5` to `0x62B9FE`); what the manager makes of it is not read.
**Seen** 2026-10-25 (#227, check 25) beside nine of them at 51,28: in 15 s nothing of the profile 10 was picked, no sample started and no stream opened, so the original's waterfalls are silent (**hypothesis**: the profile names the id 6507, a music file's, which `sound.pak` lacks); `opensacred-game` plays no static's sound either.

The regions' share is driven by the scripts (not followed): the campaign's functions set variables `atmos_rg<N>` (`bekannter1` does `IncVar atmos_rg1`), and the client holds the string `atmo_rg` (VA `0x94F4FC`), presumably to read them; 163 lines of the campaign's listing name them.
M4 recovers the link.

## The music profile

The sound profile `music` (slot 1 of `sndProfiles.pak`) names music by sound id in its groups, which the table above names: 0 `PEACE` and 1 `NEARBY` `MUSIC_WOOD01`–`03`, 2 `FIGHT` `MUSIC_FIGHT01`–`04`, 3 `JINGLE_DANGER01`–`08`, 4 `JINGLE_FIGHT_NORMAL01`–`08`, 5 `…HARD01`–`08`, 6 `JINGLE_FIGHT_OVER01`–`08`, 7 `JINGLE_FIGHT_EASY01`–`08` (docs/02 §10.3).
A function around `0x697730` plays `PEACE`: `0x4261D0(1, 0)` picks a variant of the `music` profile's group 0 and returns its sound id, which goes to `cMSS::playMusic`; the same function plays the `JINGLE_FIGHT_OVER` group (6) through `0x694FE0`.
Its conditions are read below, with the region's music.

### A music played once

Recovered 2026-10-23 (#734).
`0x694FE0(id, force, p4)`, a method of the sound manager, plays a music once on the music's stream, `+0x60C`: `MUSIC_DEATH` at the hero's death (`0x6852B0`, `0x694FE0(0x1969, 1, 0)`), the jingles of the `music` profile's groups.

- **When it plays nothing:** with the sound or the music off (the globals `0x17E9FA0` and `0xA1CD68`, the manager's `+0x11` and `+0x89`), with `+0x69C` 1, with `+0x6B1` set unless `p4` (which then forces it), while the music is `ATMOSPOT_ARENA_INDOOR` (`0x196C`), and at a music volume (`+0x7C`) of 0.001 or less or with bit 0 of `+0x64` clear; while the music is `0x196D` and one of the records that the manager follows (`+0xAF34` of them from `+0xDA4`, `0x88` bytes each) holds 689 (`0x2B1`) at its `+8` (**hypothesis**: a creature's type), it plays `0x198A` through `0x695E70` instead.
  None of `+0x69C`, `+0x6B1`, `+0x89` is read.
- **The track playing:** unless a track holds the stream (`0x679260`: Miles' `AIL_stream_status` 4, playing) and `force` is 0, the stream is paused (`AIL_pause_stream`), waited for up to 250 polls of a millisecond to report it stopped or done (`0x692790`: statuses 8 and 2), closed and forgotten (`0x678100`): the track stops at once, with no fade.
- **In peace or with `+0xAF4A` set** it plays only when forced; then, with `force` 1 and a music other than `0x198D`–`0x198F`, it calls `0x697710` first when `+0xAF4A` is 1, and in peace (`+0x698` 3) sets the double at `+0xB7B8` to 200.
  `+0xB7B8` is a time in seconds that `0x678F80`, the test of the fight's end, lowers to 120 once it passes 180 while `+0xAF4A` is set (**hypothesis**: the fight's music and its time; the fights' music is not in the game).
- **The music:** its file by the sound id (`0x6761C0`), its id at `+0x68E` and `+0x610`, opened as a stream (`AIL_open_stream`) and started at once at the music's volume times a factor of the id (`0x697820`: a table of 64 pairs of an id and a float at `+0x6B4`, 1 for an id not in it; how it is filled is not read), played once (`AIL_set_stream_loop_count` 1).
  As a track holds the stream until it ends, a sector entered meanwhile does not switch the music (`0x67A7A0`).

**Seen** (2026-10-23, the driver of #448, the Seraphim's new game, `trace stream` and `0x694FE0`): each of the hero's deaths at the start called `0x694FE0(6505, 1, 0)` and opened `MUSIC_DEATH.mp3` (17.8 s), the first while the `ATMO_WOOD` loop played on the stream; the hero rose at the start some 5 s later and the death's music went on.
After it ended, in the start's building (sector 50,39, whose music is 6560), nothing was opened on the stream for over 7 minutes, neither the loop nor a music; moved to sector 52,39 (music 6550), the game logged `cMSS::playMusic (SOUND_FX_ATMO_WOOD)` at once.
So the stream also carries the ambient loops there, and what restarts the music after a death is the sector's change, not the end of the track (**hypothesis**; `0x697710`'s part in it not read).

`opensacred-game` plays it at the hero's fall (`music`, #734): the track playing stops at once, `MUSIC_DEATH` plays once at the music's volume without fading in, by day or by night, and plays to its end in whatever sector the hero rises; then the music is silent until the hero enters a sector that another track would switch to, as above.
Its ambient loops play apart (`atmosphere`, above) and go on.
The factor of `0x697820` is taken as 1, and the jingles of the fights wait for the fights' music.

## The region's music

Recovered 2026-10-18.
Each sector names its music: `sectors.keyx +0x2D8` is a sound id, 0 in 1,892 sectors, else one of 24 (docs/02 §5.1): `MUSIC_WOOD01`–`03` (1,406 sectors, the start's `WOOD01`), `MUSIC_DESERT01` (524), `MUSIC_DUNGEON01`–`04` (550), `MUSIC_LAVA01`, `MUSIC_ICE01`, `MUSIC_VILLAGE01`, the regions' `MUSIC_PILZWALD`, `HAUPTSTADT`, `NUKNUK`, `KARIBIK`, `DRYADEN`, `WAYTOHELL`, `HELL`, `MASCARELL`, `DUNGEON_NUKNUK`, and a few loops: `ATMOSPOT_CEMETERY` in the 15 sectors of the start's cemetery (sector 50,40 among them), `ATMOSPOT_ARENA_OUTDOOR`, `ATMO_ICE_NIGHT`.

- **The event.**
  `0x6263B0`, a pass of the world view over its sectors, takes the record that the hero's sector's object points to at `+0x17C` and, when its `+0xEF` is not 0, posts the engine's event of type 2 and subtype 0 (`0x6770E0`) with its byte at `+0xD7`, the music `+0xEF` and a byte of the sector's object at `+0x2C`; in sector 59,5 the music is `0x196C` instead (`ATMOSPOT_ARENA_INDOOR`).
  **Hypothesis**: the record holds the sector's keyx record from its `+0x1E9`, as `+0xEF` and `+0xD7` are keyx's `+0x2D8`, the music, and `+0x2C0`, a byte of 56 values, the area, at the same distance.
  The same pass posts subtype 2 with the value of the scripts' variable `Globalchange1` and subtype 1 with that of `RegionChange` (below); the handler keeps them at `+0xA38` and `+0xA3C`, bit sets.
- **The handler** (`0x67A7A0`, type 2, subtype 0) keeps the music at `+0x692` (the one before at `+0x694`, when the state at `+0x698` is 3) and the area's region at `+0xA40` (`0x602250`, the table of 69 bytes at `0x95CBFC`).
  It overrides the music: `MUSIC_VILLAGE01` (`0x1996`) and `MUSIC_MASCARELL` (`0x1990`) become `MUSIC_VILLAGE_SIEGE_MILITARY` (`0x1997`) in the regions 8–10 and `0x6F`, and, with bit 1 of `+0xA38`, in the regions 2, 4–10, 12–17, `0x6E` and `0x6F`; a region whose bit is set in `+0xA3C` (bit 11 for the region `0x6E`) takes `MUSIC_VILLAGE_SIEGE_UNDEAD` (`0x1998`).
  Then, in peace (`+0x698` 3) and when nothing else holds the music (`0x679260`), it plays `+0x692` at once (`cMSS::playMusic`, `0x694510`); `0x1968`, `0x196D` and `0x196C` take their own ways (`0x691310`, `0x695400`).
- **When a track ends** (`0x697710`, above): with a music at `+0x692`, in peace and not at night (`+0xA34` other than 2) it plays it again; with none, a variant of `PEACE`; at night neither, but a jingle of group 6 and the ambient loop (`0x696A90`).

**The regions.**
`0x602250` turns an area (keyx `+0x2C0`) into a region by the table of 69 signed bytes at `0x95CBFC`, constant data: the areas 0–49 become 0, 1, 2, 3, 4, 7, 10, 9, 8, `0x6F`, 6, 5, 0, 25, 26, 24, 24, 24, 24, 0, 15, 12, 13, 0, `0x6E`, 0, 0, 25, 0, 0, 13, 17, 16, 18, 19, 0, 19, 20, 21, 23, 0, 0, 22, 22, 25, 27, 14, 28, 20, 26, and the areas 50–68 the regions 51–69.
The regions 8, 9, 10 and `0x6F` are the areas 8, 7, 6 and 9, of which no sector has `MUSIC_VILLAGE01` or `MUSIC_MASCARELL` (their 97 sectors lie in 19 other areas; every sector of `world.ron`, 2026-10-18): the overrides without the variables change no sector's music.

**The variables.**
The two words are the values of two named variables of the scripts.
The client keeps a copy of those variables, a vector of 36-byte records at `0xAB44D0` (the name, 32 bytes, and the value, a u32 at `+0x20`), which the game server's messages keep, in the client's loop over its message queue (`0x7DC260`): `0xA3` adds a record (`0x807460`), `0xA4` sets the record at an index (`0x8074D0`); `0x460720` puts their values into strings.
`0x6263B0` finds the two by name, the case aside (the scripts write `RegionChange`, `Regionchange` and `Globalchange1`).
In the campaign's trees (`NetScriptCamp`, the heroes' `type_npc_*`; the listings of `opensacred-convert netscript --listing`):

- the start script sets both to 0, so the game starts with no override but the regions';
- quest functions set bits of `RegionChange` (`SetVarBit`), a region's siege each: the quest 50's the bits 7, 8, 13 and 15, the quest 55's 3 and 5, 57's 2 and 4, 58's 1 and 6, 61's 9–12, 16 and 17, 67's 18, 71's 14;
- `GoodToBad`, called by the quest 45, sets bit 1 of `Globalchange1` and changes the creatures of 13 places (the scripts' commands `RegionChange` and `ReplaceSpawn`, of a number, 4–8, 10, 11, 20–24 or 31, whether an area or a region not read, and pairs of creature or item types: the soldiers of one type become another's); `BadToGood`, called by the quest 72, changes them back, clears the bit and sets `RegionChange` to 0.

The variables `atmos_rg<N>` (above, the atmosphere) are others.
So the sieges' and the world changes' music follows the campaign's quests, which the game has not (M4).

`opensacred-game` plays the music of the hero's sector (`music`, the converter's `world.ron` carrying each sector's `music` and `area`): entering a sector of another music switches to it at once, the one playing fading out (**hypothesis**: what `cMSS::playMusic` does with the track playing is not read); a track that ends plays again; out of a sector with music into one without, the track plays to its end, then `PEACE`.
By night no track starts, and the one playing plays to its end, without the jingle (#5).
Not in the game: the sieges and the world changes, which wait for the quests (#55), and the fights' music (docs/decisions/2026-10-17-region-music.md).
