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
`playSFX` (`0x693510`) plays a voice at its place's volume and at a rate of 0.45 + (`rand() % 10`) / 90 + 0.5 times its sample's, 0.95 to 1.05: its third argument is a factor of the rate, not a volume, the sample's rate times the argument + 0.5 (**confirmed** 2026-10-26 in the code of the menus' sounds, docs/re/menus.md, "The sound"; #1140), with the distances of `AIL_set_3D_sample_distances` (100 and 10.1, or by the creature's size: size × 100 and size × 0.3 + 10; the units' scale not read); 28 channels.

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
- `BATTLECRY_nn_<class>` (10 a class) at the sound event 5 with an argument from 1003 to 1104 (**hypothesis**: a combat art), one in three (one in two for the maker of type 2, the Gladiator), always for the arts that play their own ids 2098 to 2104;
- `ENEMYKILLME_nn_<class>` (6 a class) at the hero's `DEATH`, with `MUSIC_DEATH` (6505, `0x694FE0`).

`opensacred-game` speaks the voices so (`voices`, #711): `ATTACK` at the first strike of the hero's attack and at a creature's strike that does not chain or its shot, `NORMALHIT` or `CRITICALHIT` (below 26 % of the health) at a hit that did damage and does not fell, `DEATH` at a death, each by its type's roll, the heroes' two profiles, a slot of 8 at random, the waits per sound id and one line a creature as above; at the volume of its place and panned by it as the original does (below, "A sound's place", #731), and at the rate above (`voices::rate`, #1140).
It plays the weapons' sounds (`sfx`, #712): a strike's swing by the class of what the striker holds at its hit moment within reach or in the air; at the target the striker's weapon's `HIT` when the hit did damage, else its `PARADE`, by the sound profile of the weapon's own type, so that a starting weapon without a slot of its own is silent there as the original's, and the fist's for bare hands; a shot's group 3 by the bow's profile (**hypothesis**: `SHOOT`); at the original's volumes, placed as the voices.
It plays the heroes' lines by name (`voices`, #713): its comments in the fight on one clock (#1175, "The hero's comments" below); a battle cry at the start of a combat art, one in three, one in two for the Gladiator, and surely after an art's own cry (#1174); the death's line at the hero's fall; one of the class's at random, as the hero's line.
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
  **Seen** (a trace of `0x67A7A0` at the Seraphim's start, 32,000 calls): the thieves and bandits, hostile to the hero, came with the event 6's sub-command 1, the rabbits, pigs and novices and the type 679 of the start, peaceful, with its 0, the fires `FX_FIRE_L` (type 886, which this reading first took for birds) with the event 11, the hero (id 1) with 0 into the first list by its id.
- **The tick.** The update runs at the world's tick (**seen**: a call every 0.1 to 0.13 s, its time as the argument), and its idle noises' block when the time it gathers is above 0.05 s, so at each tick.
- **The first list**, within `+0xB790` (1,200 pixels) by the integer distance of `0x678260` (above): `rand() % 20` gives 0, the record's state `+0xDBC` is neither 2 nor 6, the creature does not speak (`0x678AF0`); the state gives the group, 0 → 0, 1 → 1, 3 → 4, 4 → 5, 5 → 6 (the names' `APPROACH`, `ATTACK`, `FOLLOW`, `FOLLOW_HEROFLEE`, `FLEE`), and the type a case through the byte table at `0x6A03C8` (the types `0x20` to `0x2CA`) and the jump table at `0x6A0240`: a roll of one in 2 to 5 or none, a profile, a wait of 0.5 to 60 s, a volume added.
  A record of the flag 4 or 8 takes another path by the type's name (not read).
  `0x68BEA0` keeps the state from the record's counters (`+0xDFE` to `+0xE08`, **hypothesis**: the ticks standing, moving and striking) and its moves against the hero's: 0 once `+0xDFE` is over 50 and the others low, 1 once `+0xE06` reaches 31, 2 once `+0xE08` reaches 2, 3, 4 or 5 by whether it goes towards the hero or away from it within 500 while `+0xE00` or `+0xE02` is over 50, and 6 when its `+0x1A` slot is under 1; a new record is in the state 0.
- **The second list**, within `+0xB790` × (1.5 − `+0x84`) (900 pixels): `rand() % 70` gives 0, the state is 0 (which nothing changes in this list), the creature does not speak and its own sample does not play (`+0x1ED8`, `0x678E40`); the type a case through a switch: a roll, a variant of a profile's group 0 or a sound id of a run at random, a wait of 0 to 10 s, a volume added.
  The townsfolk's murmurs (the runs from 13,450, 13,600 and 16,050) also wait for the flag 2 to be clear; the types 702 to 705 take one run while the manager's `+0x8C` is 1 and another while it is not (`+0x8C` not read); 52, 74, 88 and 100 to 103 ask `0x678DA0` for the group.
- **The noise** plays as a voice: unless its id waits (`0x679170`), at a factor of the rate of 0.45 + (`rand() % 10`) / 90 plus the case's value (−0.2 to 0.7; **hypothesis** that it acts as the voices' factor does, #1140), the id then waiting the case's seconds (`0x679120`; the second list only for a wait that is not 0), one sample a creature (`0x678930`, `0x678980`).
  A roll of the first list that fails leaves the profile of the record before it, at first 0, which picks nothing.
- **The cases**, read off the two switches by a walk of the code with the type known and `rand()` free (311 types of the first list, 241 of the second): the first's profiles are the creatures' voices' (the wolves 2014 one in two, the bears 2009 and the boars 2004 one in three, the goblins 2031 one in three, 30 s, the thieves 2023 one in three, 60 s, the boss 2026 one in five, 5 s); the second's add the animals' (the pigs 2011 and the rabbits' none), the novices 2055, the townsfolk's murmurs and the children's.
  **Checked** at the start (the trace of `0x4261D0`, `sound-pick`): three pigs made 11 picks of 2011's group 0 in 64 s against 12.6 by these rolls, three thieves 38 of 2023 against about 30.
  `opensacred-convert tables` holds the cases as a table and writes `tables/idle_noises.ron` (`opensacred_data::tables::IdleNoises`).

`opensacred-game` makes them so (`voices`, #714): at each world tick a creature within the hearing rolls one in 70, or one in 20 if it is hostile to the hero and does not fight, then its type's noise, at the volume of its place and the voices' rate with its case's value added (#1140); docs/decisions/2026-10-23-idle-noises.md says what it leaves out.

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
- `GoodToBad`, called by the quest 45, sets bit 1 of `Globalchange1` and changes the creatures of 13 places (the scripts' commands `RegionChange` and `ReplaceSpawn`, of an area, 4–8, 10, 11, 20–24 or 31, docs/re/netscript.md, "`ReplaceSpawn` and `RegionChange`", and pairs of creature or item types: the soldiers of one type become another's); `BadToGood`, called by the quest 72, changes them back, clears the bit and sets `RegionChange` to 0.

The variables `atmos_rg<N>` (above, the atmosphere) are others.
So the sieges' and the world changes' music follows the campaign's quests, which the game has not (M4).

`opensacred-game` plays the music of the hero's sector (`music`, the converter's `world.ron` carrying each sector's `music` and `area`): entering a sector of another music switches to it at once, the one playing fading out (**hypothesis**: what `cMSS::playMusic` does with the track playing is not read); a track that ends plays again; out of a sector with music into one without, the track plays to its end, then `PEACE`.
By night no track starts, and the one playing plays to its end, without the jingle (#5).
Since #55 the sieges' and the world change's music follow the scripts' variables `RegionChange` and `Globalchange1` as the handler overrides it (`music::music_of`), whenever the hero's sector's music is chosen, so that a quest's siege changes the music of the sector the hero stands in at once (**hypothesis**: the original's pass posts the music at each of its runs).
In the game, the quest 58's `Init58` (the console's `quest set-up 58`, `script Init58`), which sets the bits 1 and 6, gave the start's sector `MUSIC_VILLAGE_SIEGE_UNDEAD`.
Not in the game: the world changes' creatures (`RegionChange` and `ReplaceSpawn`, #55) and the fights' music (docs/decisions/2026-10-17-region-music.md).

## The sources: what plays, and what `opensacred-game` does not yet

Read 2026-10-27 for #1093 in Ghidra and with capstone, against `opensacred-game` of that day (`sfx`, `voices`, `sound`, `music`, `menu_sound`, `skies`, `talk`, `choice`, `atmosphere`, `scripts`); **confirmed** by code unless marked otherwise, none of it heard in the original.
Sound names are the client's table (`0x964870`, `sound.ron`); the profiles are `sndProfiles.pak`'s slots.
Each source not played is an issue of its own under #1093 (#1166 to #1181).

### The play functions and their callers

| Function | What it does | Call sites |
|---|---|---|
| `0x693510` `playSFX` | plays a sound id once: (id, a flag, the rate's factor, 0.5, a flag, the creature's id or −1 for the middle, the volume, x, y); the rate is the sample's times (factor + 0.5) | 217 in 81 functions |
| `0x6934E0` | the same, looped, until `0x697410` stops its channel | 40 |
| `0x4261D0` | a profile's group: one of its 8 slots by `rand() & 7` | 83 in 8 functions |
| `0x425EC0` | an item type's profile (`items.pak +0x24`), else, once, a random weapon of the type's `Weapon.pak` kind (`+0x85`): the group's slot by `rand() & 7` | 9 |
| `0x693FE0` | a line by its name (the hero's MP3 lines), one at a time (`+0xB7E0`, the speaker `+0xB7E2`) | 507 in 7 functions |
| `0x694510`, `0x694FE0` | `cMSS::playMusic`; a music once (docs/re/sound.md) | — |
| `0x67A7A0` | the sound manager's events: (type, sub-command, values…) | 154 posts |

The events of `0x67A7A0` that play: 2 the region's music; 5 sub 1 an art's own cry and battle cry, 5 sub 2 the swing; 7 the runes' reading; 8 and 9 the hero's comments and the interface's and the items' sounds (below); 12 sub 2 the menus' music; 13 the statics of a profile; 14 sub 2 the interface's windows.
The event 9 takes sub-commands up to `0x15` by one switch (`0x15` sets `+0x84`, the volume at the listener, from 50 to 200, by `cWorldView_setZoom` and `addZoom`: the zoom in hundredths, below), and `0x25` to `0x100` by the byte table `0x684B58` and the jump table `0x684AF8`: only `0x25`, `0x26`, `0x30`, `0x35`, `0x3F`, `0x40`, `0x42`, `0x44`, `0x46`, `0x47`, `0x49`, `0x5C`, `0x5D`, `0x80`, `0x81`, `0x83`, `0x84`, `0x85`, `0xCA`, `0xCB`, `0xCC` and `0x100` have a case; `0x11`, `0x13`, `0x14`, `0x16`, `0x17`, `0x20`–`0x24`, `0x43`, `0x82`, `0xC0`, `0xC8` and `0xC9` are posted by the client and play nothing.

#### The profiles' groups against their callers

- **Weapons** (500–516): `MISS` (0) is picked only for the animals' parade (below), `HIT` (1), `PARADE` (2), `SHOOT` (3) and `WOOSH` (5) by the fight, which the game plays; `DROP` (4) is empty in all 17 profiles, so an item's landing has no sound in the original either.
- **Objects** (602–611): only `OPEN` (1) and `CLOSE` (3) hold samples, picked by `0x695800`; `USE` (0) is the potion's alone (600: `DRINK_POTION02`, `LEVELUP01`, `DRINK_POTION01`, `UI_INV_PUTFAILED01`), picked by no call found (**hypothesis**: unused, the potions and the level-up play their ids directly); `KNOCK` (2) is empty everywhere.
- **Generic** (`SOUND00`–`07`): **hypothesis** the names of the groups of the slots 10, 100 and 101; 100 `death` (`CHAR_SPLASH`, `THUNDER02`–`05`) is picked by no call, 101 `rain` by the rain's loop (`0x41A4F0`, played), 10 `atmospot_waterfall` by `0x425EC0` for the event 13 (a music's id, silent, docs/re/sound.md).
- **Footsteps** (102–114): the hero's (102, 103) are played; 104–114 are the other creatures' (below).
- **Music** (1, 2): `PEACE` is played, the jingles wait for the fights' music; `jingle_extension` (2) is picked by no constant call (**hypothesis**: `0x697710` picks the group 6 of a profile it holds in a register, 1 or 2).
- **Creatures and heroes** (1000–2146): played by `voices` (lines and idle noises); the Vampiress's 1012 `realvamp` is not.

### The sources

| Source | Code | Sound, profile and group | When | Trigger in the game | Played |
|---|---|---|---|---|---|
| Voices, idle noises, swings, hits, parades, shots | `0x6852B0`, `0x69B770`, `0x67A7A0` (5, 2), `0x5320B2` | creatures' profiles, weapons' 1, 2, 3, 5 | docs/re/sound.md | yes | yes |
| Heroes' `HITENEMY`, `BATTLECRY`, `ENEMYKILLME`, `MUSIC_DEATH` | `0x6852B0`, `0x67A7A0` (5, 1) | by name | docs/re/sound.md | yes | yes |
| Footsteps of the hero, rain, thunder, quake, menus, dialogues' voice-overs, introductions | `0x68EB90`, `0x41A4F0`, `0x419A70`, `0x697220`, `0x69B770`, `0x4A9730` | 102/103, 101, 200–204, 209, 7015–7018, `IDLEFX_*`, by name | as read before | yes | yes |
| Weapon's `MISS` on an animal | `0x532731`–`0x5329D0` | the striker's weapon's 0 | a strike that does no damage to an animal | yes | yes (`sfx`, #1168): 78 weapon types by their names |
| The hero's critical-hit cry | `0x6852B0`, `0x686F4F`–`0x686FC9` | 16301 or 16302 | the hero takes `CRITICALHIT` | yes | yes (`voices`, #1168) |
| Doors open and close | `0x44BCD0` → `0x695800` | 602–608, groups 1 and 3, by the type | a door turns | yes (`npcs::swing`) | yes (#1166) |
| A locked trigger rattles | `0x415F00` | 331 or 332 | the hero without the trigger's key | no (keys, #1088) | no |
| Chests, shrines, vases, graves, book stands | `0x695800` from `0x48AE90` (`SetObjState`) and `0x4968A0` | 605, 609, 610, 611, 324/325 | an object opened or closed | no | no |
| Pick-up of an item, of gold | `0x558080` → (9, `0x80`), `0x57FD80` → (9, `0x83`) | 7103, 7100 | an item into the pack or onto the cursor; gold added | yes (`pickup`, `scripts::collect`) | yes (#1167) |
| An item taken straight into a slot, a quest's item | `0x558080` → (9, 5), (9, `0x81`) | `UI_INV_PUT*` by kind, 7117 | as said | partly | the quest's item and the hurled weapon taken back into the hand (7005) (#1167); the slots wait for the inventory (#1088) |
| No room in the pack | `0x558080` → (8, 3) | `CANTCARRYMORE` lines | the pack full | no (#1088) | no |
| A rune dropped by a kill | `0x559BF0` at `0x55A4BD` | 7028 | the drop rolls a rune | no: the drops leave the runes out (docs/decisions/2026-10-21-drops.md) | no |
| Level-up | `0x57E160` → (9, `0x10`) | 7002 | the hero's level rises | yes (`fight::level_up`) | yes (#1167) |
| Spells' cast | `0x51E110` | 2000–2125 by the spell | the cast starts | yes (`making`, `spells`) | yes, the hero's (`art_sounds`, #1172) |
| Spells' act | `0x522480` | 2011–2117 by the spell | the spell acts | yes | yes, the hero's (#1172) |
| Spells' and arts' objects | `0x76A440`–`0x7C3AC0` | casts, flights, impacts, loops | the object's life | partly (stand-ins) | no |
| Arts' own cries | `0x67A7A0` (5, 1) | 2098–2104 by the art | an art's start | yes | yes, with the battle cry then sure (#1174) |
| Arts' executors | `0x506770`, `0x50E6C0`, `0x514790`, `0x5192E0` | 2076–2122 by the art | an art's run | yes | at the art's start (#1174; where in the run not read) |
| The Vampiress's night battle cries | `0x67A7A0` (5, 1), flag `+0xAF48` by (9, `0x49`) | `BATTLECRY_nn_REALVAMP` | a battle cry in her night form | yes (`forms`) | yes (`voices`, #1174) |
| Other creatures' footsteps | `0x68EB90` | 103–114, 1194–1199 | a creature's step near the hero | yes | yes (`steps`, #1171) |
| The giant spider's steps | `0x4E0300` | 1242, 1243, 1248, 1249 | its motion | yes | no |
| In-game idle ambience | `0x69B770` (`0x69FBC0`–`0x69FF5F`) | `IDLEFX_*` | every 0.5 s, by the region's music and the day | yes | yes (`ambience`, #1169) |
| Water ambience | `0x6906C0`, the count from `0x690690` | 216, 217, 218 loops | water cells drawn | yes (`liquids`) | yes (`ambience`, #1169) |
| Waterfalls | `0x69B770` (`0x69F88C`–`0x69F90D`) | 208 loop | six `WAFA_*` statics within 1,150 | yes (`statics`) | no |
| Fires, element stones, stargates | `0x690420`, `0x69B770` (`0x69F9EF`–`0x69FAB0`) | 210, 213, 214, 215, 2087 loops | such an object within 300 or 1,150 | the fires (`fx`, #1189); the stones and gates not made | the fires' (#1170) |
| Crows and bats over the camera | `0x41F1B0` | 126; 103 or 104 | their flight | yes (`fliers`, #1073) | yes (#1179) |
| The hero's horse | `0x68EB90`, (9, `0x25`), (9, `0x26`) | 105; 7012–7014; `NOTMYHORSE` | riding, the whistle, another's horse | no | no |
| Hero's comments in the fight | `0x6852B0`, `0x68BEA0` | `KILLENEMY`, `BOSSMOB`, `ENDMOB`, `ENEMYHITME`, `ENEMYTOOSTRONG`, `ENEMYTOOWEAK`, `ENEMYTYPE`, `BREATHING`, `FOE_ON_THE_RUN`, `ENEMYFLEES` | below | yes | the first six (`voices`, #1175); `ENDMOB` never plays |
| Hero's comments in the world | (9, `0x30`, `0x35`, `0x3F`, `0x40`, `0xCC`, `0x100`), `0x696060` | `REGIONTOOSTRONG`/`WEAK`, `REGIONPACIFIED`, `APPROACHQLOC`, `GETNQ`, `COMEINTOTOWN`/`VILLAGE`, `CANCELMYIDLE`, `REJECTNQ` | below | partly | no |
| Hero's comments on items and damage | (8, 1–4), (9, 4, 6, 7, 8, `0xCA`, `0xCB`) | `CM_*`, `NOTMYITEM`, `NOTMYITEMLVL`, `NOARMOR*`, `NOWEAPON*` | below | no (#1088) | no |
| Companions' click lines, equipment remarks | (9, `0x5C`), (8, `0x10`) | `*_HIRELING_ONCLICK*`, `SFX_CLICKMYSH*`, `*_EQUIPINVALID*`, `SFX_GIVE*` | a click on a companion; its equipment | no | no |
| Interface windows | (14, 2) | 7000, 7104, 7106, 7108–7112 | a window opens or closes | no (#1087, #1088) | no |
| Inventory | (9, 1, 2, 3, 5), `0x5D8000`, `0x5CAA60` | `UI_INV_*`, 7001/7003/7008, 7006 | an item put, refused, used; a potion drunk | no (#1088) | no |
| Runes read | `0x5CD100` → (7, 2) | 7010 (7009, 7011 by 7/3, 7/1) | an art learned | **hypothesis**: partly (console's `arts`) | no |
| Compass pointer, quest book | (9, `0x46`), (9, `0x81`) | 7027, 7117 | the pointer; a quest's entry written | no | no |
| Quest's fanfare | `0x46CBE0` → (9, `0x42`) | 6541 or 6542 (music once) | `ExitQuest` or `LoseQuest` | yes (scripts) | no |
| Scripts' music | `PlayMusic` (127, `0x4ADAA0`), `PlayJingle` (128, `0x4ADD60`), `SetScriptSoundControle` (129, `0x6791C0`) | a music by name | as the script runs | yes (scripts) | no |
| `InfoPlayer`'s sounds | `0x4AE350` | mode 3: 7000 | a notice | yes (`talk`) | no |
| Death extras | `0x53ECA0`, `0x4E8DA0`, `0x5334DD`, `0x536362` | 7029, 3000, the killer's weapon's 7 | below | partly | no |
| Boss and odd sounds | `0x5A3840`, `0x7BCBC0`, `0x797179`, `0x77BD60`, `0x5BB010` | 11426/11427, 16303, 2050, 711, 708, 2070 | below | partly | no |
| Music's alternation with the ambience | `0x69B770` (head), `+0xB7B8` | `PEACE` and the `ATMO_*` loop | every 180 s by day | yes | no |
| The listener's volume by the zoom | (9, `0x15`) from `cWorldView_setZoom` | every placed sound | the zoom changes | yes | yes (`sfx::Listener`, #1181) |

### Facts for the sources not played

#### The fight

- **The animals' parade** (`0x532731`): a strike that did no damage plays, for most targets, the striker's weapon's `PARADE` (`0x425EC0(type, 2)`) or the fist's (`0x4261D0(502, 2)`).
  For the target types 512, 536, 543, 545, 567–570, 573–582, 584–588 and 591 (as compared at `0x53274E`–`0x532850`) it searches the striker's weapon's type name (`0x43CED0`) from its 13th character, past `TYPE_WEAPON_`, for the strings at `0x956024`–`0x956038` (`SWORD`, `MACE` and `AXE` read in the code, `DAGGER` among them) and, with one found, plays the weapon's `MISS` (`0x425EC0(type, 0)`) at the target, else nothing.
- **The hero's critical-hit cry**: when the hero is the speaker of group 3 (`CRITICALHIT`), `0x6852B0` plays 16301 or 16302 at even odds at the hero, at the voice's rate factor, before the line's own test, and stops the hero's playing MP3 line (`0x693FB0`); its rate factor is 0.45 + (`rand() % 10`)/90, the voice's (`0x686F75`).
  `opensacred-game` plays it so, and stops the hero's line by name that plays (`voices`, #1168).
- **The killer's weapon at a death** (`0x5334DD`, `0x536362`): `0x425EC0(the event's weapon type, 7)`, which every weapon profile leaves empty: silent (**hypothesis** for the argument).
- **Death by fire** (`0x53ECA0`, the death's action, the action 6 of the AI's switch at `0x539918` in `0x5375C0`): an effect `0x50` at the creature and `DIEFLAMES` (7029) at it, at a rate factor of 0.5 (three sites).
  The action's sub-mode (`+0x152`) set, the creature standing (`+0x241`, `+0x214` 0), its type (`+0x10`, 32 to 143) chooses the case by the byte table `0x542A30` (the cases at `0x542A14`, read 2026-10-27): the fire ghoul, 73 (`feuerghul.grn`), makes the FX object 856 at itself, the effect and the sound, unless its flags word (`+0x1F4`) has bit `0x2000`; the magic skeletons 48, 80 and 81 and the undead goblins 142 and 143 each have a case of their own that plays it where their most health (`+0x4D4`) is 0, as they burn away at their last death rather than rise (docs/re/fight.md, "The undead that rise again", #1199).
  So it is not a death by fire's damage but these types' end.
  `opensacred-game` plays it at a fire ghoul's death (`sfx`, #1168) and as an undead burns away (`undead`, #1199).
- **`CHAR_SPLASH`** (3000) plays in one branch of `0x4E8DA0` (called from `addExperience`, the gold's handler, the deaths, the arrows and the save's load; what it does is not read) with an effect `0x27` at the creature and the event (6, 7); the branch's condition is not read.

#### The listener and the zoom

- (9, `0x15`, z) from `cWorldView_setZoom` (`0x6281AE`), `cWorldView_addZoom` (`0x628393`) and `0x6288F0` (`0x628BB9`) sets the volume at the listener `+0x84` from z held to 50…200: 1.0 − (z − 50)·0.005 below 100, 0.75 at 100, 0.75 − (z − 100)·0.0025 above (`0x67C105`); z is the zoom in hundredths, `cWorldView_setZoom` sending its `+0xB60`, held to its bounds, times 100 truncated (**confirmed** 2026-10-27 at `0x62817B`): 1.0 at the zoom 0.5, 0.5 at 2.
  As the hearing distance is `+0xB790`·(1.5 − `+0x84`) and the curve's points follow `+0x84` (docs/re/sound.md, "A sound's place"), the zoom changes how loud and how far every placed sound is heard: zoomed in a sound at the listener is louder and heard less far, zoomed out quieter and heard further.
  `opensacred-game` follows it (`sfx::Listener`, #1181): the camera's zoom sets the volume at the listener, by which the voices, the weapons', doors' and world's sounds and the water's count are placed, the volume's factor taken of the volume at zoom 1 (0.75) so that nothing changes at the zoom 1.

#### The objects

- **`0x695800(type, object, group)`** plays only the groups 1 and 3, at the object (its id), at a rate factor of 0.45 + (`rand() % 10`)/90, the volume 1.
  By the type: 5200–5213 and 5220 (`0x1450`–`0x145D`, `0x1464`) the chest 605; 5214, 5215 the shrine 609; 5218 the vase 610; 5222 the book stand 611; 5216, 5217, 5219 and 5223–5226 `GRAVE_OPEN01` or `02` (324, 325) at even odds whatever the group; 5674 (`0x162A`) the grid 603; 5673 and 5749 the barn 604; 5700–5709, 5740 and 5741 the fence 606; 5720–5729 the iron door 607; 5743 and 5745 the wooden fence 608; 5650–5672, 5675–5699, 5710–5719, 5730–5739, 5742, 5744, 5746–5748 and 5750–5758 the wooden door 602; any other type whose name holds `TYPE_OBJECT_DOOR` the wooden door, the rest nothing.
  (The comparisons before the switch at `0x6958E4` and the switch at `0x695B00` as read; the bounds are worth a test against `items.ron`.)
- **The doors** (`0x44BCD0`): the open plays group 1 before the clip `0xAA`, the close group 3 before `0xAB`, at the door.
  Every door of the category 10 has a profile but the secret door 5221 (`TYPE_OBJECT_S_DOOR`): the 118 others' names in the types' table (`0x8EC328`, read by `0x43CED0`) hold `TYPE_OBJECT_DOOR`, so the addon's dwarves' doors 5759–5766 (`TYPE_OBJECT_DOOR2_*`) and the second desert gate 5783 sound as wooden doors (counted 2026-10-27 against `items.ron`).
- **A locked trigger** (`0x415F00`, `cTrigger::setState`): a trigger of the flags `0x44` with a key (`+0xC`) that the hero lacks (`0x5DF1D0`) plays `DOOR_CLOSED01` or `02` at even odds in the middle, if its flag 4 is set.
- `0x695800`'s other callers, `SetObjState` (`0x48AE90`, `0x48B278`) and `0x4968A0` (not identified), open the chests and the like.

#### Items, gold and levels

- **`cCreature::pickupItem`** (`0x558080`, docs/re/items.md "Picking up") posts: a quest's item (9, `0x81`): `UI_QUESTBOOK_WRITING` (7117); gold: through `0x57FD80`, which posts (9, `0x83`): `GOLD01` (7100) at a rate factor of 0.3 + 0.4 `rand()`/32767; an item worn at once (9, 5, its category `0x426220`, `items.pak +0x2E`); onto the cursor or into the pack (9, `0x80`): `PICKUP01` (7103) at 0.4 + 0.2 `rand()`/32767, as 7117; no room (8, 3): the hero's `CANTCARRYMORE` line, more than 2 s since the last; the category 14 (9, `0x82`): nothing.
  All play in the middle (the creature −1), the volume 1.
- **(9, 5) and (9, 1)**, an item by its category: 6, 18, 19, 21, 23, 24 `UI_INV_PUTCLOTHES01` (7116); 8 `UI_INV_PUTRING01` (7007); 9 `UI_INV_POTION01` (7019); 13 `UI_INV_PUTSHIELD01` (7115); 14 `UI_INV_PUTKEYS01` (7114); 16 `UI_QUESTBOOK_TURN02` (7107); 17 `UI_INV_PUTHELMET01` (7113); 20, 28 `UI_INV_PUTAMULET01` (7020); the rest `UI_INV_PUTMETAL01` (7005) at 0.45 + 0.1 `rand()`/32767, the others at 0.4 to 0.6; (9, 2) `UI_INV_PUTFAILED01` (7004); (9, 3) 7005.
- **A rune's drop** (`0x559BF0`, the drop's call, docs/re/items.md): when the roll makes a rune of the 123 at `0x9565A0` and the dying creature is not 516, 592 or 593, `DROPRUNE` (7028) plays at the dying creature, rate factor 0.5.
- **The level-up** (`addExperience`, `0x57E160`, at `0x57EE84`): (9, `0x10`): `LEVELUP01` (7002) in the middle, 0.5.
- **A potion** (`0x5CAA60`, a `cItem` virtual): `DRINK_POTION01`, `02` or `03` (7003, 7001, 7008) by `rand() % 3` in the middle, then `0x5D8000`, the item used up, `UI_INV_USEITEM01` (7006).
- **A rune read** (`cItemUpgrade`, `0x5CD100`, at `0x5CD47F`): (7, 2) `UPGRADE_MOVE01` (7010); (7, 1) `UPGRADE_SPELL01` (7011) and (7, 3) `UPGRADE_FAILED01` (7009) are posted by no constant call found.

#### Spells and arts

The cast (`0x51E110`) switches on the spell (1–72) by the jump table `0x521C34`; the sound in each case's code, in the middle of the caster (its id), rate factor 0.5:

| Spells | Sound |
|---|---|
| 1 | `MAGE_TELEPORT` 2023 |
| 2 | `MAGE_STEINHAUT` 2013 |
| 4 | `MAGE_SUPERFLUID` 2012 |
| 5 | `MAGE_FEUERBALL_CAST` 2000 |
| 7 | `SPELL_HEALING` 2086 (at the target) |
| 11 | `MAGE_FIRECIRCLE_CAST` 2018 |
| 13 | `SERA_HIMMELSFEUER_CAST` 2034 |
| 15 | `SERA_HEILIGES_LICHT_CAST` 2030 |
| 17 to 22 | 2035, 2038, 2033, 2037, 2032, 2039 (`SERA_KLINGENWALL_CAST`, `LICHTGRIFF`, `BEKEHRUNG`, `LICHT`, `CRUSADERS_STRENGTH`, `LICHTSCHILD_CAST`) |
| 23 to 26 | 2005, 2003, 2014, 2007 (`MAGE_HELLFIRE_CAST`, `FIRESKIN_CAST`, `VERSTEINERUNG`, `METEORSCHLAG`) |
| 29, 30, 31, 33 | 2015, 2021, 2051, 2043 |
| 36, 37, 40 | `HELF_CAST` 2059 (36 also `HELF_VERWANDLUNG_CAST` 2063) |
| 41 | `HELF_CAST_LANG` 2065 |
| 60, 61, 66 | 2118, 2125, 2121 |
| 62 to 65, 67 | `DEMON_CAST` 2122 |
| the others | none in the case |

The act (`0x522480`, the jump table `0x52947C` by the spell less 1): 8 and 13 `SERA_HIMMELSFEUER` 2041; 16 `SPELL_BANNKREIS` 2029; 33 `MAGE_BLITZSCHLAG` 2011; 34 `MAGE_ICEBULLETS` 2026; 35 `SPELL_FROSTRING` 2052; 39, 40 `HELF_PFLANZENKONTROLLE` 2061; 42 `HELF_GENESUNG` 2060; 50 `VAMP_SPRUNG_HIT` 2084; 55 `DWARF_GIER` 2106; 57 `DWARF_EINGRABEN` 2112; 58 `DWARF_PANZER` 2108; 59 `DWARF_STAHL` 2109; 67 `DEMON_SCHREI` 2117; 71 `SPELL_PENTAGRAMMKILLER02` 2095 and `HELF_VERWANDLUNG_HIT` 2064.

The arts' own cries, the event (5, 1) by the art (`0x4FD0F0` at `0x4FD87A`, `0x4FDE34`, `0x4FE630`), rate factor 0.5 at the maker, before the battle cry's roll: 1003 `JUMPINPULK` 2098; 1009–1011 `NECKBREAKER1` or `2` at even odds; 1012 `HANDOFGOD` 2102 for a maker of type 2, else `AURA` 2101; 1013 `WEAPONTHROW_01` or `_02`; 1014–1016 `AURA`; each of them always takes a battle cry, the others by the class's roll (one in three, one in two for the maker of type 2).
The swing's event (5, 2) of a striker whose record type is 140 also plays `MAGE_BLITZSCHLAG` 2011.
The executors' (by their jump tables; the branch inside a case not read): the Gladiator's 1043 `SPELL_TURBOBOOST` 2081 (`0x506770`); the Vampiress's 1022 `VAMP_VAMPIRISMUS` 2079, 1026–1027 `VAMP_WOLFSRUF` 2080, 1028–1029 `SPELL_TURBOBOOST`, 1030 `VAMP_BLUTBISS` 2076, 1032 `VAMP_SPRUNG` 2083, 1034 and 1035 `VAMP_SHORTCAST` 2078 (`0x50E6C0`, the second switch `0x5146BC`); the Dwarf's 1084 `DWARF_WUCHT` 2107 (`0x514790`, `0x519234`); the Daemon's 1103 `DEMON_CAST` twice (`0x5192E0`).
The Vampiress's form posts (9, `0x49`, 1 or 0), which sets `+0xAF48`, under which her battle cries are `BATTLECRY_nn_REALVAMP` (the strings at `0x9D7B44`).

The spells' and arts' objects play from their own functions (the particle systems' classes, `0x76A440`–`0x7C3AC0`); by the sound, with the function: `MAGE_FEUERBALL_CAST` and `_HIT` (`0x76BE80`, `0x76BBD0`, `0x7BFA90`), `MAGE_WILDSCHWEIN` (`0x76A440`), `DELF_SOULCATCHER_ABSORP` (`0x76A860`) and `_LOOP` (`0x76AA40`), `VAMP_VAMPIRISMUS_EX` (`0x76D260`), `WATERFOUNTAIN01`/`02` (`0x76E2D0`), `DELF_SCHLACHTENNEBEL` (`0x7709F0`), `MAGE_PENTAGRAMM_HIT`, `_CAST`, `_LOOP` (`0x772C40`, `0x773790`, `0x79AB70`), `MAGE_HEALING` (`0x774330`), `TORCHFIRE_LOOP` (`0x7752B0`), `MAGE_FIRECIRCLE_LOOP` (`0x775DF0`), `MAGE_HELLFIRE_LOOP` (`0x776860`), `DELF_GIFTHAUCH_LOOP` (`0x778D10`), `DELF_VERWIRRUNG_LOOP` (`0x7792C0`), `FIREGHUL_FLAMEBREATH` (`0x77AE90`), `SPIDER_PUKE04`, `_PUKEIMPACT04` (`0x77B180`, `0x77B700`, `0x7C2320`, `0x7C2590`), `BLEEDING02` (`0x77BD60`), `SERA_PLASMASHOT_BIG` (`0x77F520`), `SERA_HEILIGES_LICHT_LOOP` (`0x7816A0`), `SERA_KLINGENWALL_LOOP` (`0x784C90`), `SERA_LICHTSCHILD_LOOP` (`0x785DE0`, `0x7B64D0`), `MAGE_METEOREINSCHLAG01`/`02` (`0x7866C0`), `MAGE_WIRBELWIND_LOOP` (`0x788D90`), `MAGE_MAGIESCHILD_LOOP` (`0x78B040`), `SPELL_AUE` (`0x78B4B0`), `SPELL_REIKI` (`0x78B900`), `HELF_CAST_LANG` (`0x790CE0`), `VAMP_BEHERRSCHUNG_LOOP` (`0x7938D0`, `0x79DBF0`), `DELF_CAST` (`0x794300`), `DELF_ADRENALIN_LOOP` (`0x794CE0`), `HELF_VERWANDLUNG_HIT` (`0x795E20`), `SPAWN` (`0x796E10`), `SPELL_BANNKREIS` (`0x79CC80`), `HELF_AHNEN_LOOP` (`0x79D310`, `0x7A6370`, `0x7BF3A0`), `VAMP_PENTA` (`0x79E5D0`), `DELF_SPRENGSATZ` (`0x79E730`), `DELF_WURFCAST` (`0x79EA50`), `LAVABURST` (`0x79F160`), `VAMP_BLUTBISS` and `SPELL_LASER` (`0x7A50B0`), `SPELL_BIGSUMMON` (`0x7A8430`), `DEMON_HOELLENDISK_LOOP` (`0x7AE040`), `VAMP_SHORTCAST` (`0x7AFD70`), `DEMON_TENTAKEL_LOOP` (`0x7B1990`, `0x7BEBF0`), `DEMON_CAST` (`0x7B27D0`), `DEMON_HOELLENFEUER` loop (`0x7B3110`), `DWARF_FLAMMENWERFER` (`0x7B3870`), `DWARF_KANONE_ABSCHUSS` (`0x7B4480`), `DWARF_MOERSER_ABSCHUSS` and `_EINSCHLAG` (`0x7B50F0`, `0x7B70A0`), `DWARF_KRIEGSSCHREI` (`0x7B8300`), `DWARF_TELLERMINE_LEGEN` and `_EXPLOSION` (`0x7BA610`, `0x7BAD70`), `PONG` (`0x7BCBC0`), `SPELL_FIRESTRIKE` (`0x7BF720`), `TORCH_HIT01` (`0x7BFA90`), `DWARF_PISTOL` (`0x7C3AC0`).
Which object type each class is (`0x5A0C50`'s byte table `0x5A3270`, docs/re/arts.md "The objects' systems") is not read; the names make most of them plain.
Outside them: `SERA_PLASMASHOT` (2072) with a railgun's shot (`0x4DCB80` at `0x4DDE12`), `HELF_SPINNENPFEIL` and `TORCH_HIT01` with arrows (`0x5BB010`), `SPELL_LASER` (`0x4D4E50`, `0x570BC3`), `FIREGHUL_FLAMEBREATH` (`0x4D3CD0`, `0x4E0300`, `0x5B2600`), `HELF_PFLANZENKONTROLLE` (`0x4D2E10`, `0x4E0300`, `0x4E4CF0`), `BOSS_SAKKARA_DEMON_GROWL01`/`02` (`0x5A3840`), `SPELL_TELEPORTER_ENGAGE` 2088 by (9, `0x47`, poster not found).

#### The other creatures' footsteps

`0x68EB90(creature, nature, flying)`, from a creature's step (`0x639080`), plays only for a creature that the manager follows (its first list, else its second; so within the hearing), at the creature, at a rate factor of 0.4 to 0.6:

- flying (the third argument 1) or of type 54: by its type name (the strings at `0x9DED9C`–`0x9DEDEC`), the small wings (1194, 1199) for a name holding `CROW`, `BAT`, `CHICKEN`, `EISVOGEL`, `PAPAGEI` or `VAMPBAT`, the big wings (1197, 1198) for `DRAGON`, `FLIGHT_LIZARD` or `FLIGHT_DEMON`, else the wings (1195, 1196), at even odds;
- the hero (`+0xAF40`): 102, or 103 for the types 1, 6, 7 and 9, by the ground's group as docs/re/sound.md gives it (played); the Daemon's form 1195/1196 (played); of the types 261, 550–558 and 72 (**hypothesis**: mounted, its record taking the mount's type): the horse 105, group 2 on water and swamp (`0x90`, `0xA0`, `0x50`), 5 on `0xB0`–`0xD0`, else 1;
- another creature whose type is in the manager's first sorted table (`+0xBCF4`–`+0xBCF8`): `footsteps_otherhuman` 104 by the ground as the hero's; in the second (`+0xBD00`–`+0xBD04`): `footsteps_otheranimals` 106, group 1; how the tables are filled is not read;
- else the types 33–36, 47, 50, 57–59, 69, 70, 73, 93, 95–98, 106, 107, 119–124, 189, 192, 197, 204, 210, 213, 329, 334, 339–343, 346, 350, 711, 712: 103 group 4; the types 37, 39–43, 45, 49, 51, 60–62, 68, 77, 84, 89–92, 137, 187, 196, 202, 205–208, 253, 254, 330–332, 344, 359, 360, 365–367: `footsteps_bigone` 107, group 2 on water, else 1;
- else by the type's name after its `TYPE_`: `NATURE_HORSE` and `NPC_UNICORN` 105 by the ground; `NPC_FEUERGHUL` 102 group 4; `NPC_DRAGON` and `NPC_BIG_DEMON` `footsteps_bigger ones` 113; `NPC_ORC_`, `NPC_WARRIOR`, `NPC_GOBLIN`, the three pirates, `NPC_ANDUCAR_GARDE`, `NPC_GLORB` and `NPC_DAEMONENFUERST` `footsteps_metalclothes` 109; `NPC_TATZELANNOY`, `NPC_GIGANT_SPIDER` (at a rate 0.3 lower), `NPC_KILLERKRABBE` and `NPC_RUINENASSEL` `footsteps_tatzeltannoy` 112; `NPC_SPIDER_M`, `NPC_SMALLSPIDER` and `NPC_MED_SPIDER` `footsteps_giant spider` 114; `NPC_SKELETON` and `NPC_DWARF_SKELETON` `footsteps_skelette` 108; all group 1; else none.

The giant spider's steps 1242, 1243, 1248, 1249 are played by its own code (`0x4E0300`, `0x4E378C`–`0x4E3831`).

Read further 2026-10-27 for #1171 (**confirmed** by code unless marked otherwise):

- **The two tables** are filled once, by `0x697860` from the manager's constructor `0x676200`, with constant name prefixes kept sorted: `+0xBCF4` 35 of `TYPE_NPC_` (`ANDUCAR`, `AMAZONE`, `BAR_FEM`, `BARBARIAN`, `BARONESS`, `BLACK_MAGICIAN`, `BLACKSMITH`, `CITIZEN`, `DARKELVE`, `DEMODREY`, `DRUID`, `DRYAD`, `ELVE`, `FARMER`, `GAUKLER`, `GARGADUK`, `MERCHAND`, `NOBLE`, `NOVIZIN`, `PRIEST`, `PRUTUK`, `RAEUBER`, `ROCHEFORD`, `SHARIFA`, `SHARIFO`, `SOLDIER`, `SORCERESS`, `THIEF`, `TUKTUK`, `VALOR`, `VILYA_UW`, `WIZARD`, `WITCH`, `WIRT`, `WUESTLING`), `+0xBD00` 12 (`TYPE_NATURE_` `BULL`, `BEAR`, `COW`, `DEER`, `PIG`, `RABBIT`, `RED_DEER`, `WILD_BOAR`, `WOLF`, and `TYPE_NPC_TERRORSPRINTER`, `_CHAMP`, `_MAGIER`).
  `0x68EB90` searches them by `strncmp` over the shorter length, so an entry matches as a prefix of the type's name: the `ANDUCAR_GARDE` rule (109) and the `FEUERGHUL` one (102) are never reached.
  Over the types' table (docs/02 §6.9) the cascade gives 104 to 163 creature types, 106 to 51, 103 group 4 to 42, 107 to 38 (its group 2 on swamp too), 109 to 36, 108 to 17, 105 to 10, 113 to 9, 112 to 8, 114 to 8, the wings to 54 always, and nothing to 91.
- **The steps** come from the creature's animation step `0x5941A0` alone, while it walks (its action 1, or 5 for an id above 16): at its clip's motion's moments `floats_46[1]` (the right foot), `[0]` (the left), and `[3]` and `[2]` (the forelegs' hands) where they are not 0, each once a loop of the clip, and only where the model has the bone (but the spiders' right foot); the nature is the cell's under the creature, the bridge's on a bridge, flying 0; a creature in the air (`+0x1F4` bit `0x200000`, set for the types of the `Creature.pak` flag 1 in some sectors; **hypothesis**: in the air) takes the wings instead.
- **The lists** are the idle noises': every animation step posts the event (6), and a creature within 1,200 pixels enters the first list (hostile, at most 32) or the second (at most 256); a step is heard within the hearing distance, 900 pixels at zoom 1.
- **The play**: `playSFX` at the creature at a rate's factor of 0.4 + 0.2·`rand()`/32767 (0.3 less for the giant spiders 55 and 237), the volume and pan of its place, no waits; the wings 1194/1199, 1195/1196 or 1197/1198 at even odds.
- **The Big Demon** (53, not 94): each of its steps makes an `FX_SMALLFIRE` (878) at its place and sends the weather's jolt of 18, then its sound 113.
- `opensacred-game` plays them (`steps`, #1171): the types' sounds as the cascade gives them, the steps at the clip's moments of the walk or the run that plays, at the creature, the fliers of the flag 1 always in the air (**hypothesis**); the Big Demon's jolt with them, not its fire.

#### Ambience

- **The idle effects in the game** (`0x69B770`, `0x69FB92`–`0x6A002F`; read again 2026-10-27, **confirmed** by code): the timer `+0xB7D0` adds the frame's seconds, and from 0.5 s (`0x890C58`) it is reset and `r = rand() % 10` is drawn; only in the game's states (`+0x698` not 0, 1 or 4).
  On 1: `IDLEFX_GLOBAL01`–`10` (16120 + `rand() % 10`) unless the id waits (`0x679170`), through `playSFX` once at a pan of 0.3 + 0.4·u and a rate factor of 0.3 + 0.4·u (the sample's × 0.8 to 1.2), `u` each a `rand()/32767` of its own (`0x5295A0`); the id then waits 5 s (`0x679120`), even if it did not start; then its volume is set to (0.2 + 0.3·u) of the effects' (`0x6772D0`, a volume's setter: `+0x9C`, times `+0x78`, the effects' volume, `SFXVOLUME`/128).
  After it a `rand() % 3` is always drawn; with `r` 2 and that 0, by the region's music (`+0x692`, the byte table `0x6A0988` into the jump table `0x6A096C`): `MUSIC_DESERT01` (6510) `IDLEFX_DESERT` (16100 + `rand() % 6`); the dungeons' and hell's (6520–6523, 6706, 6707) `IDLEFX_DUNGEON` (16220 + `rand() & 15`); `MUSIC_LAVA01` (6540) `IDLEFX_LAVA` (16180 + `rand() & 15`); `MUSIC_ICE01` (6563) `IDLEFX_ICE` (16140 + `rand() & 15`); the villages' and towns' (6550, 6551, 6544 `MASCARELL`, 6702) `IDLEFX_TOWN` by the phase of the day (`+0xA34`, 0 day, 1 dusk, 2 night, 3 dawn): day and dusk 16200 + `rand() % 14`, dawn one in two 16214 + `rand() % 4`, night one in two 16216 + `rand() % 2`; the woods' and the regions' (6560–6562, 6701, 6703–6705) `IDLEFX_WOOD`: day 16160 + `rand() % 20`, dusk 16170 + `rand() % 10`, dawn 16171 + `rand() % 9`, night 16173 + `rand() % 7`; unless the id waits, at a pan of 0.3 + 0.6·u and a rate factor of 0.3 + 0.6·u (× 0.8 to 1.4) and the same volume; only an id that started waits, 5 s, 60 s for 16202 `IDLEFX_TOWN03` and 120 s for 16169 `IDLEFX_WOOD10`.
  That is the 2D sound, which runs with `SOUNDQUALITY` 0 or 1 (the reference `Settings.cfg` has 1); with 2 and the DirectSound3D hardware provider (`+0x80` 1) the sample is placed instead at −5 to 5 and −8 to 8 of the sound library's units about the listener.
- **The water** (`0x6906C0`, every frame after the fires, but in the states 0, 1 and 4, which stop its loops; read again 2026-10-27, **confirmed** by code): the world view's liquids' pass (`0x62DD00`) counts its list of the liquid cells of the tile rows it drew (`+0x3FF2C`, `+0x80E3C`; the natures `WATERA` and `WATERB` of any material, the lava's too; **hypothesis**: the screen and its margin) and, at `DETAILLEVEL` 2 with a clear water's cell among them (the materials 0 to 3, 10 and 11, of `+0xCC`), hands the count and a place to the manager (`0x690690`), else a count of 0; at a lower detail it hands nothing and the manager keeps the last.
  With `v` the volume at the listener (`+0x84`, 0.75) the count `c` makes `d = c + 2c·((v + 0.5)³ − 1)`, 2.90625·c; from `d` 30 the loop `WATER_TINY` (216) at 0.4·min(d/300, 1), from 300 also `WATER_NORMAL` (217) at 0.4·min((d − 300)/400, 1), from 700 also `WATER_BIG` (218) at min((d − 700)/500, 1), each of the effects' volume (`+0x78`) by the count alone, set each frame; each starts looping at a rate factor of 0.4 + 0.2·u, drawn once, and stops at once below its threshold; with `SOUNDQUALITY` 0 the water is silent.
  The place sums, for each clear water's cell, its sector's bounds in world units (`sectors.keyx +0x3C`, `+0x40`) with its sector's first cell in cells and its column and row, divided by the count of every liquid cell, and turned into the screen's pixels: a slip of units that lands about 1.19 times the sector's coordinate in cells past the sectors' origins, not at the water; the loops' pan is that place's against the listener (`0x678260`), its volume unused.
- **`opensacred-game`** plays both (`ambience`, #1169): the idle effects as above, the region's music the last music of a sector the hero was in (**hypothesis** for `+0x692`); the water by the liquid cells within the original's 1,024 × 768 pixels about the hero at the zoom, panned by the clear waters' true middle (docs/decisions/2026-10-27-water-sound-place.md).
- **The waterfalls**: an event 13 static (any static whose type has a profile) whose type name is one of `WAFA_FRONT_MITTE`, `WAFA_LINKS_MITTE`, `WAFA_RECHTS_MITTE`, `WAFA_FRONT_SPRITZER`, `WAFA_LINKS_UNTEN_RE`, `WAFA_RECHTS_SPRITZER` (`0x6A0B30`, **hypothesis**: equality) loops `WATERFALL_LOOP` (208) while its distance (`+0xA6B8` of its record) is under 1,150, and stops beyond.
  docs/re/sound.md saw nothing beside nine waterfall statics at 51,28 (#227); **hypothesis**: those nine were of the other 24 types; to be heard in the original before it is made.
- **The fires** (`0x690420`): an object of the second list of the types 812, 813, 876 (`FX_FI_SMOKE_*`), 878 (`FX_SMALLFIRE`), 884–886 (`FX_FIRE_L`), 904 (`FX_FIRE_MAGIC`) within 300 and without its flag 1 loops `INDOORFIRE_LOOP` (210) at its place, at 0.3 (812, 886), 0.4 (813, 885, 904), 0.6 (876, 878, 884) of the volume, at most four at once; beyond 300 it stops.
  So the type 886 that docs/re/sound.md ("The idle noises") calls the birds is `FX_FIRE_L`.
  Read again 2026-10-27 (#1170): at each frame, while fewer than four channels play 210, it starts one more at the first such fire of the list within 300 (`0x6934E0`, looping, a rate's factor of 0.5, the sample's own rate, positional at the fire, the volume its type's), without asking whether that fire has one already: so four loops play as soon as a fire comes within 300, and a playing one stops when its fire goes or lies beyond 300; any other type of the list takes 0.25.
  `opensacred-game` plays them so at the nearest fire (`fx::crackle`, #1170), each loop placed at its fire by the listener's volume times its type's.
- **The element stones and the stargates** (`0x69F9EF`–`0x69FAB0`): `FX_ELEMENTSTONE01`–`05` (827–830, 841) loop `ELEMENT_LOOP` (213), `06` (939) `HEART_GOLD_LOOP` (214), `07` (768) `HEART_ROTTEN_LOOP` (215), `FX_STARGATE` (802) and `FX_STARGATE_UW` (1180) `SPELL_TELEPORTER` (2087), each within 1,150 (not at the distance 0), stopped beyond.
- **The crows and bats** (`0x41F1B0`, the fliers over the camera, docs/re/weather.md item 7): at each of the flier's timers, the crows (513, 589, 590) `ANIMAL_CROW01` (126), the bat (514) `ANIMAL_BAT04` or `05` at even odds, in the middle.
- **The music's alternation** (`0x69B770`'s head): by day (the phase not 1 or 2) `+0xB7B8` counts seconds; with `+0xAF28` set, past 180 s the peace music (`+0x698` 3) gives way to the ambient loop (`0x696A90`, state 2) and the loop to the music again (`cMSS::playMusic`, state 3), the count reset; with bit 2 of `+0x64` it counts 20 s less (**hypothesis**: `+0xAF28` the music on).

#### The hero's comments

All are MP3 lines by name (`0x693FE0`), one at a time, `+0xB7E8` the seconds since the last, the class by `+0xAF42` (set by (12, 3)).

- **In the fight** (`0x6852B0`): for a creature other than the hero and not an animal (no record of the types 512 to 596 for it), more than 10 s since the last line, one in three, and its record's flag 4 or 8 (set by (6, 6); **hypothesis**: a boss, a champion): at its `DEATH`, `KILLENEMY` by `rand() % 15` over the class's 14; at its hits or flight (groups 2, 3, 4, 6), more than 30 s, one in four, `BOSSMOB` (`rand() % 6`) for the flag 4, else `ENDMOB` for the flag 8.
  At the hero's `ATTACK`, more than 20 s, one in eight: `ENEMYTYPE` of a creature of the first list (its `+0xDB8` at most 149, **hypothesis** a distance) whose type is one of the kinds named at `0x9DE3E0`–`0x9DE74C`.
  At the hero's `CRITICALHIT`, more than 20 s, one in four: `ENEMYHITME` (`rand() % 6`).
  More than 15 s, one in 20, for a creature's hit (groups 2, 3) with a value of the hero's record (`+0xDD4`) at least 2 above the creature's and another of the record at least 71: `ENEMYTOOWEAK`; for a creature's `ATTACK` with the creature's value 2 above the hero's and the other at most 29: `ENEMYTOOSTRONG` (**hypothesis**: their levels and the hero's health share; the fields not read).
- **Read again** 2026-10-27 for #1175 (**confirmed** by code unless marked otherwise; a subagent of the session, checked where the game leans on it):
  - The flags 4 and 8 of a first-list record are set only by the event (6, 6, a), which every animation step of a boss (`+0x1F4` bit `0x80000`) or a champion (`+0x2B8` bit `0x40`) posts (`0x5941A0` at `0x59443B`); its `a`, meant to tell them apart, is computed so that it is always 0 (`0x59441C`), so that both take the flag 4, the flag 8 is never set and `ENDMOB` never plays.
  - The record's `+0x2C` is the health in per cent, `+0x30` the level, `+0x14` the distance in the place's pixels, and `+0x34` 1 while the creature's own line of its profile sounds; the clock `+0xB7E8` gains the frame's seconds and every comment that plays sets it to 0, `BATTLECRY` and `ENEMYKILLME` neither testing nor setting it.
  - In `0x6852B0`, before the speaker's own line and at most one a call: `HITENEMY` at a boss's or a champion's hit (groups 2, 3) while the hero's own line sounds (its `+0x34` 1, the opposite of every other test), more than 10 s on, one in five, one of 11; `ENEMYTYPE` at the hero's `ATTACK`, more than 20 s, one in eight, by the first hostile record within 149 whose type's name names a kind with lines for the class (`ORC` and `THIEF` for every class, ten more kinds for the Dwarf and the Daemon); `ENEMYHITME` at the hero's `CRITICALHIT` after its cry, more than 20 s, one in four, its own line silent, one of 6; then, for a creature not the hero nor an animal, the hero silent: at a boss's or a champion's death `KILLENEMY`, more than 10 s, one in three, `rand() % 15` of 14 lines (the last roll plays nothing and still sets the clock back), at its hit `BOSSMOB`, the same roll and then more than 30 s and one in four, one of 6; and, more than 15 s, one in 20, one of 6, `ENEMYTOOWEAK` at a hit of a creature two levels or more below the hero at 71 % or more of its health, `ENEMYTOOSTRONG` at a strike or shot of one two levels or more above it at 29 % or less (the sixth line a class lacks is its fifth again).
  - A comment plays in the middle at the voices' volume and stops the line by name that plays, whoever speaks it; only `FOE_ON_THE_RUN` and `ENEMYFLEES` wait for silence.
  - `opensacred-game` plays `HITENEMY`, `ENEMYHITME`, `KILLENEMY`, `BOSSMOB`, `ENEMYTOOWEAK` and `ENEMYTOOSTRONG` so (`voices`, #1175), its bosses and champions those of the flag 4; `ENEMYTYPE`, `BREATHING` and `FOE_ON_THE_RUN`/`ENEMYFLEES`, which need the types' kinds and the motions' history of sixteen updates, are not yet.
- **In motion** (`0x68BEA0`): while the hero's motion is a walk (`MOTION_WALK` in its name), `BREATHING` by its type loops twice; every 15 s with none playing, a breathing line by the type (`rand() % 5`); with a value of the record above 0.6 of its `+0xD96` (not read), one in five and more than 20 s, `FOE_ON_THE_RUN` or `ENEMYFLEES` (`rand() % 15`).
- **In the world**: `REGIONTOOSTRONG`/`WEAK` (9, `0x30`) from the sectors' pass (`0x6263B0`); `REGIONPACIFIED` (9, `0x35`) from three places (`0x475D0F`, `0x4AC92D`, `0x6268F2`); `APPROACHQLOC` (9, `0x3F`) and `COMEINTOTOWN`/`COMEINTOVILLAGE` (9, `0xCC`, 1 a town) from the step onto a trigger (`0x593CE0`), both only when `0x678F80` answers 1 (**hypothesis**: out of a fight): the first when a timer of the manager's passes 180 s, one in four, the second when `0x691700(0xCC, 300, 41, 1)` lets it (**hypothesis**: once in 300 s), by `rand() % 12` over its lines; `GETNQ` (9, `0x40`) as a quest is entered (`0x46C160`); `REJECTNQ` by the scripts' `RejectQuest` (126, `0x696060`; seven classes, not the Dwarf and the Daemon); `CANCELMYIDLE` (9, `0x100`) from `0x424A60`; `STARTUP_DWARF`/`DEMON` (9, `0x44`).
- **About items** (#1088): `CM_NOCOMBO`, `CM_WRONGITEM`, `CM_CANTUNMERGEITEMS` (8, 1, 2, 4); `NOTMYITEM` (9, 4 or 6, one of 6), `NOTMYITEMLVL` (9, 7 or 8, one of 7) from the inventory's window (`0x6B6690`, `0x6E1BC0`); `NOARMOR<kind>` and `NOWEAPON<kind>` (9, `0xCA`, `0xCB`, the kind 1 normal, 2 fire, 3 magic, 4 poison) from the strike's code (`0x56DE0C`, `0x56DF73`), at most once in 400 s.
- **Found in no code** (no string, no constant): `QUESTSOLVED`, `COMBATBREATH`, `HIT_BY_FOE`, `IDLE_BREATH`, `COMBATSPE`, `REGIONFIRE`/`MAGIC`/`NORMAL`/`POISON`/`UNDEAD`, `NOWEAPONUNDEAD`, `NOTMYHORDE`, `CHARACTER` (**hypothesis**: unused, or named by the scripts' `PlaySound`).
- The companions: a click on a hireling of the type 361 or on one of nine named companions (by the creature's name, `0x672BA0`) plays one of its four click lines, one in four, when no line plays (9, `0x5C`, from the mouse handler `0x6172C0`; this corrects docs/re/items.md's hypothesis of a click's sound); a hireling's equipment (8, `0x10`, from `0x6D6760`) plays its remark by its type (77, 83, 224, 322, 323, 361) and its health share (below 41 or from 70).

#### Scripts and notices

- `PlayMusic` (127, `0x4ADAA0`): the name (with `SOUND_FX_` added when missing) to an id (`0x676170`) and `cMSS::playMusic`; `PlayJingle` (128, `0x4ADD60`) the same through the music once (`0x694FE0`); `SetScriptSoundControle` (129) calls `0x6791C0` (not read); across the converted trees' files they appear about 204, 99 and 329 times, `RejectQuest` 40.
- `ExitQuest` and `LoseQuest` (`0x46CBE0`) post (9, `0x42`) and (9, `0x43`); `0x42` with the value 0 plays `JINGLE_FANFARE01` (`rand() % 3` of 0 or 1) or `02` through the music once, forced (`0x694FE0(id, 1, 0)`), having first, while the peace music plays or `+0xAF4A` is set, called `0x6792A0(0.5, 0.5)` and set `+0xB7D8` to 5 s, after which the update calls `0x6792A0` again (**hypothesis**: the music lowered for 5 s); `0x43` plays nothing (**hypothesis**: the exit posts `0x42`).
- `InfoPlayer` (`0x4AE350`): the modes 0 and 1 post (9, `0xC8`) and (9, `0xC9`), which play nothing, so docs/re/netscript.md's "plays the interface's sound 200, 201" reads an event's sub-command as a sound; the mode 2 also plays 7009 through the music once (`0x694FE0`), an id without a music file (**hypothesis**: silent); the mode 3 plays `UI_CLICK01` (7000) in the middle.

#### The interface (#1087, #1088)

(14, 2) when the manager's state (`+0x698`) is not 0, 1 or 4: the window 1 `UI_INV_OPEN01` or `CLOSE01` (7111, 7112) by its value 1 or 0; 2, 3, 4, 11, 16, 24, 64 `UI_CLICK01`; 8 `UI_QUESTBOOK_CLOSE01` (7104); 9, 10 `UI_QUESTBOOK_TURN01` (7106); 30 the smith's (7108); 31 the trader's (7109); 32–34 the combo master's (7110); all in the middle.
The buttons' class `0x72B560` plays its own sound (`+0x18`), `0x7172F0` `UI_CLICK01`; (9, `0x46`) `UI_COMPASSPOINTER` (7027) in the game's states 2 and 3.
`UI_ELEMENT_*_SFX` (7021–7026), `MUSIC_MENU`, `MUSIC_DEMON_ENDFIGHT`, `JINGLE_MAINQUEST_DONE*`, `SPELL_BLITZSCHLAG`, `SPELL_KUGELBLITZ`, `SPELL_MORPHING`, `SPELL_SHIELD_LOOP`, `TORCH_LOOP01`, `VERRUECKTER_SFX`, `LITTLETHROW_01` (but by its profile 504) and `GLYPHEN`/`HOELLENTOR`/`FLAMME_SFX` (220–222) are played by no call found.
