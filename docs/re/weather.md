# The weather: rain, fog, snow, the storm, the shakes and the butterflies

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra on 2026-10-26 (#1065, a subagent of the session), and on `World/sectors.keyx` and the NetScript trees by scans of the files.
Status: **confirmed** by code unless marked otherwise; the rain and the fog seen in the running original on 2026-10-27 (#1073, below).

The weather is drawn and heard only: no rule of the play reads it.
The sector the camera looks at chooses it, by the weather entries of its `sectors.keyx` record, on timers of real time that every sector shares.
One event, `cEventWeather`, carries the weather's changes, the screen's shakes, the lightning and the butterflies to one handler.
No script sets a weather.

## The weather's state

The weather lives in the renderer (docs/re/daynight.md), a singleton of `0x12E6C` bytes that `0x417E70` makes and keeps at `0xAA4544` and `0x417F00` sets up; it holds the night's level `+0x12E54` and the phases' times too.

- `+0x00`: on, set to 1 at the set-up.
- `+0x10`: flags: 1 rain, 2 fog, 4 a storm (its lightning armed), `0x10` snow, `0x20` the night (docs/re/daynight.md), `0x80` a quake, `0x100` a lightning's flash, `0x200` a vertical jolt.
- The rain: its drops now `+0x1C` and wanted `+0x20`.
- The fog: its alpha wanted `+0x178` and now `+0x17C`, and four drifts at `+0x180` to `+0x18C`.
- The lightning: its timer `+0x428` and its flash's frames `+0x430`.
- The snow: 3,000 flakes at `+0x4B4` (a position and a size of 2 to 10 each), their count now `+0x9154` and wanted `+0x9158`.
- The quake `+0x9DB8` and the jolt `+0x9DBC`, and the offsets of the screen they make, `+0x9DC0` and `+0x9DC4`.
- `+0x9E0C`: the hero in a dungeon; `0x625290` sets it at each frame that the hero's sector is a dungeon (docs/re/daynight.md), and `0x418FB0` writes it too (**hypothesis**: it clears it at each frame).

The renderer's handler of the events whose kind (the virtual `+8`) is `0x24` is `0x419A70`; its update of a frame is `0x41A4F0` and its drawing `0x4190D0`, both from `cEngine_renderThreadRun`.

## How the weather is chosen

At each frame `cWorldView0_render` calls `0x625290`, which reads the weather of the view's sector: its world record's `+0x17C`, which is `sectors.keyx +0x1E9` (docs/02 §5.1), a bit mask at `+4` and four entries of 28 bytes from `+5`.

- An entry is a `u16` type, then four `u32` in seconds, the shortest and the longest time off and the shortest and the longest time on, then two `u8`, the least and the most intensity.
- The type 0 is rain, 1 fog and 2 snow; 4 makes the sector a dungeon (docs/re/daynight.md); 5 is the butterflies (below); the code reads no other type.
- An entry counts where its bit of the mask is set.

Each of rain, fog and snow has one timer, shared by every sector: `0xAD77B4`, `0xAD77B0` and `0xAD77B8`.
They count real milliseconds, by the performance counter (`0x6413E0`), not the game's clock.

- A kind's timer is armed with a time off as a sector of that kind is first seen in the process (the bits of `0xAD77AC`, set once each).
- When its timer passes while the kind is off, the kind starts at the intensity `least + b % (most − least + 1)` and its timer takes a time on; when it passes while the kind is on, the kind stops, at 0, and its timer takes a time off.
  The log tells `cWorldView2::showWorld() RainStart`, `RainStop`, `FogStart` and `FogStop`.
- `b` is the next byte of a table of 256 at `0x95FA84`, read by a rolling index (`0xAD77C0`).
- A time is `shortest + (longest − shortest) / b`, with the constants 1.0 and 1000.0 (`0x89066C`, `0x890B54`), so nearly always the shortest; the table's byte 0 makes it infinite, so that its kind stays as it is for the rest of the run (**hypothesis**: a slip for a share of the span).
- The snow is taken for on by the rain's flag (`0x625459` tests `& 1`), so that in a sector of snow the snow starts again, at an intensity rolled anew, at each time off's end and never stops (**hypothesis**: a slip for the snow's `0x10`).

At each change of the view's sector (`0x6252F1`, the sector last seen at `0x95F930`) an event of kind 1 sets every wanted level to 0, so that the weather fades; it comes back only as its timer next passes.

### The sectors' entries

Counted on `sectors.keyx` (2026-10-26): the mask is 0 in 2,203 sectors, 1 in 3,835, 3 in 11 and 9 in one.
The entries whose bit is set:

| Type | What | Sectors |
|---|---|---|
| 0 | rain | 307 |
| 1 | fog | 1,031 |
| 2 | snow | 46 |
| 3 | none the code reads | 10 |
| 4 | a dungeon | 2,428 |
| 5 | butterflies | 37 |

- Rain, fog and snow nearly all take 480 to 600 s off, 480 to 600 s on and the intensity 20 to 40; one entry of snow takes 5 to 15 s, 10 to 30 s and 15 to 25.
- 88 entries of rain have the byte `+0x14` at 1, which nothing reads.
- The ten entries of the type 3 hold the intensities 50 to 75 and 75 to 100 (**hypothesis**: a storm left out), all in dungeons of the areas 25 and 37.
- The start sector 50,39 has fog; the nearest rain falls in 51,43 and 52,43, the nearest snow in 51,19 and 51,17, and the butterflies fly in 54,42 and 55,41.

The levels move by steps of about three a second (an accumulator `0xAA4548` grown by 3 · dt): the rain's drops by 10 a step, the snow's flakes by 20, the fog's alpha by 1, so that a rain of 20 to 40 (600 to 1,200 drops) takes 20 to 40 s to fall in full or to fade.

## What is drawn and heard

- **The rain**: lines without a texture (`WEATHER_RAIN01.TGA` is the shadows' texture, docs/re/drawing.md), each frame every drop at random on the screen, 5 to 20 pixels long, slanted by `L · [+0x74] · [+0x68]`, where `+0x68` is the cosine of the time of day (a turn in 1,440 minutes) and `+0x74` is `2 · |t/1439 − 0.5| + 0.2` (**hypothesis**: the shadows' vector, docs/re/drawing.md), each coloured by the light where it falls (`0x41B510`).
  A drop is a strip of two vertices, its top of the alpha 0 and its bottom of 255 (`0x00FFFFFF` and `0xFFFFFFFF` as `0x417F00` sets them up, the light's colour put in at each frame), so that it fades in from its top; it is blended by its alpha (`SRCALPHA`, `INVSRCALPHA`), under the interface.
  Its sound is a loop of the profile 101, the sample 205 `SOUND_FX_RAIN01`, at the volume of the drops' count over 3,000, stopped when the count reaches 0.
- **The storm**: a rain above 75 arms a timer of 20 to 40 s, at whose every end an event of kind 9 flashes the screen white, a quad added over it for 3 to 8 frames, and plays one of the thunders, the samples 200 to 204 (`THUNDER01` to `05`), but not in a dungeon (`0x63D260`).
  The quad (`+0x3A8`) is of the grey `0xCC` (`0xFFCCCCCC`) and blended `ONE`, `ONE`, so that it adds 0.8 to each value of the frame, which is white wherever it was 0.2 or more.
  No entry reaches 75, so a storm comes only from the console (**hypothesis** for the play).
- **The fog**: `WEATHER_FOG01.TGA` in two layers over the screen, their texture coordinates moved with the camera and drifting by (0.045, 0.028) and (0.079, 0.061) a second, at the fog's alpha, at most twice its intensity; white by day, and with the night's flag `0x20` of the colour `255 − 0.508 · N` in red and green and `255 − 0.273 · N` in blue (the constants at `0x890940`).
  It is filtered linearly and blended by its alpha.
- The flags of `0x643430` that these draws set are Direct3D 7's render states: 4 the alpha's blending, 8 the source's factor `ONE` (else `SRCALPHA`), `0x10` the destination's `ONE` (else `INVSRCALPHA`), `0x800` the linear filter of the textures; so the snow is added (`ONE`, `ONE`), as the flash is.
- **The snow**: `PARTICLE_SPARK03.TGA` on quads lit by the light where they are, falling at 10 · size pixels a second, swaying by `cos(0.1 · y) · dt · size · 5`, drifting by `[+0x74] · [+0x68]` at each frame (not by the frame's time), wrapping round the screen's sides and made again past twice its height.
  A flake made again lies at random on the screen, its size 2 to 10 (`0x41A4F0`); its quad's half side is its size over the zoom, and its height on the screen is its own wrapped at 1,024 (`& 0x3FF`), so that a flake that has fallen below the screen's 768 comes in again from the top until it passes 1,536.
  The quads are added (`ONE`, `ONE`), the texture's colour times the light's.
- None of the three is drawn while `DETAILLEVEL` (`Settings.cfg`, 0 to 2, the byte `0x182EE78` that `0x814152` reads) is 0, nor in a dungeon.

## `cEventWeather` and its kinds

Its virtual table is `0x890928`; `+8` is its kind, `+0xC` its value, up to 100, and `+0x14` whether it stays on this machine; `+4`, `+0x10` and `+0x18` are always 0.
It is made in place or by `0x51CB60`, which Ghidra names as an effect's constructor.
`0x5FCB70`, which Ghidra puts under `cEventWeather`, is the object manager's move of an object from sector to sector.

| Kind | What it does | Who sends it |
|---|---|---|
| 0 | every flag off at once | none found |
| 1 | every wanted level to 0: the weather fades | the choice above, at each change of sector |
| 2 | the rain, 30 · value drops | the choice; the console's `setrain`; the network's restore |
| 3 | the fog, its alpha 2 · value | the choice; `setfog`; the restore |
| 5 | the snow, 30 · value flakes, up to 3,000 | the choice; `setsnow`; the restore; the item set 39 |
| 7 | a quake of the screen | the combat arts and the effects; `setquake`; the restore |
| 8 | the night's level and its flag `0x20` | `0x41C2D0` (docs/re/daynight.md) |
| 9 | a lightning, ended by its own event of value 0 | the storm's timer |
| 10 | the butterflies of a sector made | a sector's `Enter` |
| 11 | the butterflies of a sector taken out | a sector's going |
| 12 | a vertical jolt of the screen | a creature's steps; `0x522480` |

- The set 39 (docs/re/items.md) sends the snow at 100 or 0 as the stats are made anew (`0x57D362`), and again at 100 as the hero changes sector while its query `0x4E` answers (`0x5FCB70`; **hypothesis**: the set's bonus, a snow about its wearer).
- The kinds 4 and 6 have no case; the fields `+0x210` and `+0x214` and 50 floats of speeds 0.003 to 0.025 at `+0x218` are set up and kept but never drawn (**hypothesis**: an effect taken out).
- **The quake (kind 7)**, while `SCREEN_QUAKE` is on (`0x81DDE0`; 1 in `Settings.cfg`): an amplitude of value/3 pixels (`+0x9DB8`), falling by 10 a second, moves the screen by a random offset within it on both axes at each frame, which the view's matrix adds to the camera (`0x6284E0`), and plays the sample 209 `SOUND_FX_EARTHQUAKE_LOOP` (`0x697220`), at most once every 0.1 to 0.35 s; so 30 shakes it by 10 pixels for a second, 45 by 15 for a second and a half.
  Each event sets the amplitude anew rather than raising it, and plays the sample as it comes; the kind 1 sets the amplitude to 0, its flag left for the next frame to take off.
  The Gladiator's jump 1003 sends 45 as it lands (`0x4FF0B0` at `0x500765`, after its regeneration starts; docs/re/arts.md, which took it for the ground's shaking), and the Daemon's executor sends 45 as one of its arts takes its effect 50 off (`0x5192E0` at `0x519EFB`; which art is not read); the particle systems and spells send 30, 24 or 21 (`0x7866C0`, `0x790340`, `0x79E730`, `0x7A7500`, `0x7AF550`, `0x7B70A0`, `0x7BAD70`, `0x7C1790`; which spells is not read).
- **The jolt (kind 12)**, under `SCREEN_QUAKE` too: a vertical offset of value/3 pixels, falling by 40 a second; a creature's step sends 18 (`0x639080`, for an object of the kind `0x5FE870` 0x35; **hypothesis**: a big creature's stomp), and so does `0x527716` in `0x522480` (not read).

## The butterflies and the birds

- As a sector's `Enter` runs (`0x6261A0`), the event of kind 10 makes, for each of its entries of the type 5, ten objects of the type 873 `FX_BUTTERFLY` (`sdk_CREATE` with the type `0x369`), each at a place at random that `0x638CD0` admits and sent the effect's event `0x61`, one in 31 with a flag of 1; it tests neither the hour nor the weather, whatever its log line says of good weather.
  As the sector goes (`0x6262A0`), the kind 11 takes them out (`0x5FF160`).
- Apart from the weather, `0x625290` sends, but by night, every 20 to 180 s, a flier of a pool of eight (`0x637060`) over the camera: the type 513 `CROW.GRN` where the camera's sector has the byte `+0x2C` at `0x10`, the type 514 `BAT.GRN` where it has `0x80` (read 2026-10-27, **confirmed** by code):
  - The byte is the sector's `sectors.keyx +0x1D8` (docs/02 §5.1; **confirmed** by a trace of `0x626017` in the running original: `0x40` in 50,39, `0x10` in 51,51).
  - In a sector whose byte is not 0, a timer of real time (`0xAD77BC`) is first armed `b % 161 + 20` seconds on by the weather's rolls (`0x95FA84`, whose index `0xAD77C0` the weather's timers share); once it passes, the byte's flier is sent and the timer armed anew the same way; another byte sends none but runs the timer.
  - It is sent but by night (`0x41BAB0`, 22:20 to 03:20), while `0x637060` finds a free place among its eight (`+0x828C`, `0x34` bytes each), and while a value of the world view (`0x6413E0`'s `+8`) is under 10 (not read).
  - It lives 20 s (`0x41F180`).
    As it is made (`0x41F400`), it takes its type's model and the clip of the motion slot 179 (`CROW_FLY_LOOP`, `BATX_FLY_LOOP`) and starts at even odds 1,124 world units behind the camera's point or 1,024 ahead of it along the world's x, at `rand() % 1536 − 768` along y, for a point as far on the other side and its own `rand() % 1536 − 768`, both turned into the models' space, 240 of its units up; it faces its way and flies straight along it at 115 of those units a second (`0x41F1B0`).
  - It calls first `trunc(rand()/32767 · 25 + 5)` seconds after it is made; at each call's time a roll of `trunc(rand()/32767 · 100)` above 60 silences it for good, else it calls in the middle at the volume 1 and a rate's factor of 0.5, the crow `ANIMAL_CROW01` (126), the bat `ANIMAL_BAT04` or `05` (103, 104) at even odds, and calls again `trunc(rand()/32767 · 20 + 10)` seconds on (`0x41F1B0`, its switch `0x41F3A4` by the type).

## The network, the console and the settings

- An event whose `+0x14` is 0 goes over the network while `0x182EBEC` is set (**hypothesis**: on the host); the kinds 10 and 11 never go.
  `0x41BBA0` writes the whole state: the host in `0x619BF0`, which logs `Weather`, and a client reads it in `0x61B6F0`, which logs `* Weather`; those two strings are the message's names, no setting and no command.
  `0x41C0F0` restores it, called by `cEngine_ctor` from the game's state `+0x37D4` and `+0x37D8`, which the client's dispatcher of messages `0x7DC260` fills, and by that dispatcher; no code of the saves is known to keep it (**hypothesis**: the weather is not saved).
- The console's commands (`0x614230`): `setrain N`, `setfog N`, `setsnow N` and `setquake N` send their kinds, staying on this machine, at most 100; `setdungeon N` sets the dungeon's byte to `N == 1`.
- `Settings.cfg` has no key of the weather's own; `DETAILLEVEL` and `SCREEN_QUAKE` only hold its drawing and its shakes back.
- `sim_talk_weather.tga` is one of seven icons of talk (the gamble, the war, the money, the politics, the weather, the music, the romance; `0x4090D0`; **hypothesis**: the townsfolk's), not of the weather.

## The scripts

The scripts' compiler knows a tag `Wetter:` (`0x6A`) beside `Region:` (`0x69`) and `Musik:` (`0x68`) (`0x454420`), but no record of any tree carries `0x6A` or `0x68`; `0x69` stands only in `ReplaceSpawn` (3,321 records) and `RegionChange` (360) (docs/re/netscript.md).
So no script sets the weather, and the sectors' `Enter`s only make the butterflies.

## In `opensacred-game`

The world's weather is followed (#1072): `opensacred-convert world` writes each sector's entries of rain, fog, snow and butterflies into `world.ron` (`weather`) and `tables` the rolls of `0x95FA84` (`tables/weather_rolls.ron`), and the game chooses and ramps it as above, by the sector the hero is in, at the time of real time and by the game's time (`opensacred_rules::weather`, `weather`), the two slips repaired (#1083, docs/decisions/2026-10-26-weather-timers.md: a time spans its range by its roll, never infinite, and the snow goes by its own flag); the overlay (F3) and `state` show its levels, and the console's `setrain`, `setfog` and `setsnow` set it.
`skies` draws the rain, the fog over it, the lightning's flash and the snow, in the original's order, and plays the rain's loop and the thunders (#1073): one material over the whole screen, over the world and under the interface, whose colours are premultiplied so that the flash and the flakes add to the frame; the drops lie anew at random each frame by a hash of the frame's count, as many as the weather's state holds, their slant by the time of day, and the fog's two layers move with the camera and drift.
The snow's 3,000 flakes move on the CPU as the original's (`skies::Flakes`), their drift by the slant at each of the original's frames taken at its 60 a second, and those of the weather's count are listed by the cells of a grid of 32 pixels in two storage buffers, so that a pixel tests only the flakes of its cell.
The drops and the flakes are lit by the night's colour over the whole screen, not by the light where each falls (`0x41B510`).
A quake (`opensacred_rules::weather::Event::Quake`) moves the camera after it follows the hero by a random offset within the amplitude, in the original's pixels at the zoom, at each frame but in a pause, and plays the earthquake's sound as it comes; the console's `setquake` and the Gladiator's jump send it, the Daemon's art and the spells not yet, and `SCREEN_QUAKE` has no setting.
The crows and the bats fly over the camera as above (`fliers`, #1073): `opensacred-convert world` writes each sector's byte into `world.ron` (`fliers`), `opensacred_rules::weather` keeps the timer on the rolls the weather shares, and the camera's point is the hero's, which the camera follows; a flier is a model among the others, drawn at the cell under it (`models::Ordered`), lit as they are, its call in the middle.
Seen 2026-10-27 in 51,51 at noon: a crow 134 s after the hero came, flying right and down over the screen, about 70 pixels across at zoom 1, and its call 13 s after it started; not yet set beside the original's.
The jolts and the butterflies (`FX_BUTTERFLY` is an effect's type, of the category 12, with no model) are still #1073 (M5).

Set beside the original on 2026-10-27, by the driver's `weather` (docs/03 §3.8) and the console's `setrain` and `setfog`, the start of a Seraphim's game at noon (frames in the scratchpad):

- A rain of 60: about 1,800 drops in both, of the same length, white at their bottoms and fading upwards, leaning to the left by about a fifth of their height, under the interface; the original's lines are two pixels wide on four rows in ten, opensacred-game's on one in ten (dgVoodoo2's lines; **hypothesis**).
- A fog of 40: the frame's mean over the world, about 100 in red without it, became 159, 153 and 145 in the original and 160, 155 and 146 in opensacred-game.
- A snow of 60: sparks of the same shapes and sizes in both, white over the floor, 10.7 % of a region of the floor's pixels lit in the original against 11.6 % in opensacred-game.

## To see in the original

With the driver (docs/03 §3.8), `trace 0x419A70` and the kind at `[[esp+4]+8]`, and the game's log:

1. A new game standing in 50,39: `FogStart` about 480 s in, `FogStop` about 480 s later, the times near the shortest; a step into 49,39 while the fog stands sends the kind 1, and the fog fades.
2. In 51,43 or 52,43: `RainStart` and the sample 205.
3. In 51,17: `RainStart` again and again, the snow starting anew, and no `RainStop`.
4. In 54,42 at any hour: objects of the type 873 made through `0x5FBA40` as the sector comes into the 3 × 3 about the hero.
5. `weather rain 80`: a lightning and a thunder every 20 to 40 s, none in a dungeon; `weather quake 30`: a shake of a second; `DETAILLEVEL 0`: no rain drawn.
6. Over two hours or more, whether a kind's timer freezes as the table's index reaches its 0.
7. By day in a sector whose `+0x2C` is `0x10` (51,51): crows over the camera every 20 to 180 s, their size and height beside `opensacred-game`'s.
