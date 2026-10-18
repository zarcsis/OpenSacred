# Day and night: the clock, the phases and the light

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra 12.1.4 on 2026-10-18. Status: **confirmed** on the code unless marked otherwise.

## The clock

The game's clock is an object of its own (`0x417560` makes it once, at `0xAA4540`). Its time of day is three words, hours, minutes and seconds (`+0x20`, `+0x22`, `+0x24`), with the fraction of a second in a float (`+0x28`); a date of three more (`+0x18` 1370, `+0x1C` 2, `+0x1E` 14; hypothesis: the year, the month and the day). A new clock reads **08:00**.

The renderer's update of a frame (`0x41A4F0`) advances it by the frame's time (the clock's second virtual function, `0x4176C0`): the time times 24.0 (`+0x0C`) times a speed of the day or of the night (`+0x10` from 07:00 to 19:59, `+0x14` otherwise, both 1.0) is added in seconds (`0x416F00`), carried into minutes, hours and days, and a day carries into the date (`0x416DA0`). **Hypothesis**: the frame's time is in seconds, so a day of the game lasts an hour. In a network game the server sets the clock (the message `0x8B` of `0x61B6F0`).

## The phases

The renderer holds four times in minutes of the day, which its set-up writes (`0x417F00`): 200 (03:20), 320 (05:20), 1220 (20:20) and 1340 (22:20). With `t` the minutes of the clock (hours × 60 + minutes):

| Phase | `t` | The night's level `N` | The sound manager's phase |
|---|---|---|---|
| Day | 320 < `t` < 1220 | 0 | 0 (event subtype 1) |
| Dusk | 1220 ≤ `t` ≤ 1340 | (`t` − 1220) · 2.125 | 1 (subtype 2) |
| Night | `t` < 200 or `t` > 1340 | 255 | 2 (subtype 3) |
| Dawn | 200 ≤ `t` ≤ 320 | (320 − `t`) · 2.125 | 3 (subtype 4) |

`0x41C2D0` sets the level `N` (`+0x12E54` of the renderer, 0–255) and the renderer's flag `0x20` (set but in the day); `0x41BA60`, `0x41BAB0`, `0x41BB00` and `0x41BB50` test for the day, the night, the dawn and the dusk. When the phase changes, the renderer posts the engine's event of type 3 with the phase's subtype; the sound manager keeps it at `+0xA34` (docs/re/sound.md: the night's ambient loops at 1 and 2, no music at 2). At the change into the dusk and into the dawn the renderer also sends every object of a certain type (`0x4286E0`) a message `0x115` (hypothesis: the lamps light and go out). A script can set the level itself (the renderer's command 8, `0x419A70`).

## The light

The colour that a point of the world takes (`0x41B510`, which the world's passes call with a point of the screen, `0x62DD00` among them; **hypothesis**: the colour of the vertices, by which the art is multiplied) comes from the lights around it and from `N`:

- **The light's level** at the point starts at 32; every light source of the world (a table of `{x, y, radius}` that a grid by position finds) whose radius reaches the point adds its radius less its distance, at most 223 each, and the sum stops at 255. The table is filled anew every frame, as the draw lists are drawn (`0x418FB0` swaps it in), up to 1,024 sources of a radius above 10, at their positions on the screen (`0x41B170`, or `0x41B200` from the world): an object of the draw lists whose type has the flag `0x20000` (`items.pak +0x00`) at its position, of its type's radius (`items.pak +0x32`: the invisible `SimpleLightTiny`, `Small`, `Medium` and `Big` statics of radii 220, 300, 450 and 600, some miniatures, fires and spells, docs/02 §6.1), and a creature (type flag `0x2`) of the radius `0x54A7B0` gives: 0 but for the players' heroes, 700 for them, and from the dusk to the dawn 1000 − `N`·700/255, doubled for the creatures 5 and 7 (the Wood Elf and the Vampiress by night), and changed by a torch in hand (`TORCH_SMALL_ON`, `TORCH_LARGE_ON`, not read).
- **By night** (flag `0x20`) the colour is a ramp of that level, built at the set-up from the setting `NIGHT_DARKNESS` (`Settings.cfg`, 50; `0x81D7A0` reads it as a fraction, 0.5, a value above 1 taken as 0): red and green `64 + 64·d + level/4`, blue `64 + 64·d + level/3`, with `d` the fraction. With 50 the dark far from any light is (104, 104, 107) and the brightest (160, 160, 181), a grey turning blue near a light.
- **Between**, each channel of that ramp has `255 − N` added, up to 255: so the day (`N` 0) is white, the night (`N` 255) the ramp, and the dusk and the dawn pass between them in two hours.
- **In a dungeon** (the renderer's `+0x9E0C`), at any hour, the colour is a second ramp, grey, `(blue ramp) · 0.75 + 64`.

The glows of the lamps (`PARTICLE_GLOW01`) take `N` in their alpha at night (`0x62E410`, `0x62FF60`), and a creature's distance of 700 (`0x54A7B0`; hypothesis: its sight) becomes 1000 − `N`·700/255 while the flag `0x20` is set.

## In `opensacred-game`

`daynight` keeps the clock (08:00 at the start, `--time HH:MM` for another hour), advances it by 24 seconds a second of the frames' time, and takes its phase and the night's level as above. The colour by the night is laid over the frame drawn, a fullscreen pass that multiplies each pixel's 8-bit values by it (`night.wesl`; the interface is drawn after it), rather than over each vertex: the same for the art, which the original multiplies by its vertices' colour, but not for what the original blends (docs/decisions.md). The light sources are the static objects of the light types in the loaded sectors, at their positions on the ground at height 0 (the original's are flat positions on the screen), and the hero, of the radii above; the 32 nearest the screen's centre that reach the view light it, the distance taken on the screen in the original's pixels, both ways alike (**hypothesis**: the distance's formula, done on the FPU, is not read). Not yet (#61): the creatures', the fires' and the spells' lights, the torches, the doubled radius and the dungeons' grey ramp. The ambient loops of the night play in the dusk and by night (`atmosphere`), and no music starts by night (`music`).
