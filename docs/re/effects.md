# Effects: the FX objects and the particle systems

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra and scans of the executable (capstone) on 2026-10-27, for #1159 (a part of #1092).
Seen in the running original with the driver of #448 on 2026-10-27 (a Seraphim's new game: the braziers of 50,39 by noon and by night, the butterflies of 54,42 by noon and by night, a torch in the hero's hand by night; frames in the scratchpad only).
Status: **confirmed** by code unless marked otherwise.
Builds on docs/re/arts.md ("The spells' objects", "The objects' actions", "The objects' hits", "The objects' systems"), docs/re/weather.md ("The butterflies and the birds"), docs/re/daynight.md ("The light", "The glows") and docs/re/drawing.md (the draw lists, the engine's switch of render states `0x643430`/`0x643470`).

## Summary

- Every effect of the world, the spells, the arts and the items is an object of the `items.pak` category 12 (`cObjectFX`), and every such object draws one particle system (`cParticleSystem_*`), chosen by its type alone.
- An FX event (`cEventFX`) starts the object; its kind says how the object attaches and moves, and a value of it (`+0x38`) picks a variant inside the system.
- The systems share one machinery: 64-byte particles, up to four emitter layers, 256-entry colour ramps over a particle's life, camera-facing quads of pre-lit vertices, and a handful of helpers for glows, flat quads, rings and helices.
- The world's fires are not their own class: `FX_FIRE_L` (884, 885, 886), the fire smokes, the chimney smokes, the lava smokes, the geysers and the magic fires are all `cParticleSystem_smoke`, its 13 variants chosen by the scripts' `CreateObj`.
- The torch's flame is `FX_TORCHSMOKE` (787) on the torch's bone `weapon_fx01`; the butterflies are `FX_BUTTERFLY` (873), swarms of 3 to 6 that wander the walkable ground and turn into fireflies by night.

## The FX objects

### The types

- The items table holds 210 types of the category 12 (`items.pak +0x2E`): 205 of the kind 2 and 5 of the kind 6 (779 `FX_TEMPTEX`, 795 `FX_MAGICMARKER`, 903 `FX_BLOODPUDDLE`, 907 `FX_BLOODPUDDLE_BIG`, 918 `FX_BIG_DEMON_SUMMON`), in the slots 768 to 939, 1004 to 1023 and 1150 to 1181 (`tables/items.ron`).
  None has a model or a sprite; 24 have the light flag `0x20000` with a radius at `+0x32` (the fires 884 to 886 of 100, 70 and 40, the fire smokes 812, 813 and 876 of 40, 35 and 40, the traps 890 to 901 of 70, 778, 799, 832, 833, 872, 875, 887, 888), which the light table does not use for them ("Lights" below).
- The object factory (`0x451300`) makes a type of the category 12 a `cObjectFX` (constructor `0x5A0A10`, virtual table `0x89199C`, docs/re/arts.md).
  The constructor keeps the type at `+0x10` and the object's id at `+0x0C`, clears its fields, and calls the set-up `0x5A0C50`.

### The set-up: one system per type

- `0x5A0C50` picks the system by the type alone: a byte table at `0x5A3570`, indexed by the type less `0x300` (`0x19E` entries, the types 768 to 1181), gives a case of a switch of 183 (`0` to `0xB6`).
  Each case allocates the system and calls its constructor with a particle count and the object's id; the manager of the systems (`0x7CC0D0`, a singleton of `0xC24` bytes at `0x17EEBCC`, its vector of systems at `+0x18`) registers it (`0x7CB830`) and the object keeps the system's index at `+0x6E`.
- 204 types get a system of 175 distinct classes; the case `0x6E` makes none (the traps' fires 896 to 900, `FX_TRAP04_FIRE1` to `5`) and the case `0xB6` none and fails (770 `FX_CIRCLEOFFEAR`, and every slot that is not an FX type).
  The RTTI names 176 classes `cParticleSystem_*` and the base `cParticleSystem` (virtual table `0x897828`): the plan's "about 178".
  Several types share a class: the smoke class serves 13 types, the element stones 7, the forge elements 5, the underworld glyphs 4, the teleports 2 (the catalogue below).
- The set-up then marks the object (`+0x14`): bit `0x10000000` for every type but a list of persistent ones (768, 771, 787, 795, 802, 810 to 813, 827 to 830, 832, 833, 841, 853, 854, 876, 879, 881, 882, 884 to 886, 889, 890, 894 to 900, 904, 939, 1160, 1161, 1175 to 1181), and bit `4` for the smoke family, the torch smoke, the trap 892 and 893 and the blood puddle 903 (its meaning not read).
  The simulation's tick (`0x5A6210`, virtual `+0x04`) destroys an object of bit `0x10000000` whose sector is no longer active, unless its system follows something (system flags `0x1200`), and logs it; the persistent ones (the world's fires and smokes, the glyphs, the gates) stay.
  The FX event handler clears bit `0x10000000` too when the system's flags have `0x1200`.

### The FX event

`cEventFX` (constructor `0x41E1D0`, virtual table `0x8909B4`, 599 call sites) is a record of `0x7C` bytes:

| Offset | What |
|---|---|
| `+0x04` | the kind, which the handler switches on (1 to `0xB4`) |
| `+0x08` | the caster's id (for the world's effects 0; for a weapon's effect the weapon object) |
| `+0x0C` | the target's id |
| `+0x10` … `+0x1C` | four words (a colour or a quaternion by the kind; not followed) |
| `+0x20` … `+0x28` | a point in the models' frame (the position of the world's effects) |
| `+0x2C` … `+0x34` | a direction (the bolts', the fireball's) |
| `+0x38` | an integer: the variant of the system, or a switch (0 stops some systems) |
| `+0x3C` | an integer (an art's id for the hit sparks: 1003 shakes the screen) |
| `+0x40` … `+0x74` | an array of 14 words, copied to the object's `+0xE4` (the level, the damages' multipliers, the factor, the art's record; docs/re/arts.md) |
| `+0x78` | flags, or'ed into the object's `+0x48` (`0x2C0` for the scripts' effects) |

The systems' places, velocities, spreads and offsets lie in the models' frame, as the event's point does: its `x` along the screen's horizontal on the ground, the world's (1, −1), its `y` toward the viewer, the world's (1, 1), its `z` up (docs/re/drawing.md, "The models' turn"; "Their flight" below).
`opensacred-game` takes them so since #2072 (`bolt_looks::{world, models}`, `iso::fx_axes`), at `MODEL_SCALE` world units a unit along each axis as it draws the models; before it took their `x` along the world's x, a turn of 45° about each system's origin, which only the shapes not round about the upright showed: **seen** 2026-11-04 by the big demon's pentagram ("The big demon's summoning").

The object's handler (`0x5A3840`, virtual `+0x18`) takes the event when the object is set up (`+0x48` bit 1): it keeps the caster (`+0x54`, its sector `+0x58`) and the target (`+0x60`), the array and the kind (`+0x44`), plays the type's sound profile (`0x6770E0` with 6), and switches on the kind (`kind − 1` into the jump table `0x5A5EC4`).
Each case sets what the object follows, `+0x68` an object's id and `+0x70` a bone's index, sometimes `+0x72`, `+0x74` and `+0x76` three more, and other fields (a direction at `+0x78`, a turn rate at `+0x8C`), places the system (`0x7CBB00`, its position `+0x08`), and starts it: `0x7CC040` calls the system's virtual `+0x10` with the event.
Then the handler sets `+0x48` bit 2, the object started, which its draw requires.

**The bones.** A bone's index (`+0x70` and the three after it) names a bone of the followed object's model through a table of 32-byte names at `0x8E7200` (`0x401000`), which `0x5B6730` and `0x44CA30` find on the model: 0 `__Root`, 1 `Bip01 L Hand`, 2 `Bip01 R Hand`, 3 `Bip01 Neck`, 5 and 8 `bip01_particle01`, 11 and 12 `Bone_weapon_01` and `02`, 15 `weapon_fx01`, 16 `weapon_fx02`, 17 `weapon_fx03`, 18 `weapon_gl01`, 19 `weapon_gm01`, 20 and 38 `Bip01 Head`, 21 and 22 the third fingers, 23 and 24 the spine, 26 to 35 `fx_streak01` to `10`, 36 and 37 the feet, 40 `Bip01`, 42 the pelvis, 46 `canon_fx01` (48 entries read).

### Who makes them

- **The scripts' `CreateObj`** (`0x485C10`): for a type of the category 12 it builds the FX event by a switch on the type (`0x48907C` for 768 to 891 with the tables `0x48AC90`/`0x48AC24`, `0x489C3F` for 904 to 1157, `0x48A26F` for 1160 to 1181; 894 and 1158 apart), with the flags `0x2C0`, the point of the record's `0x20` turned into the models' frame (`0x6239C0`), and a kind and a variant by the type:

  | Types | Kind | Variant (`+0x38`) |
  |---|---|---|
  | 771 `FX_HS_SMOKE_S`, 810 `_M`, 811 `_L` | 7 | 1, 2, 3 |
  | 812 `FX_FI_SMOKE_S`, 813 `_M`, 876 `_L` | 7 | 4, 5, 6 |
  | 879 `FX_GEYSIR` | 7 | 7 |
  | 882 `FX_LV_SMOKE_M`, 881 `_L` | 7 | 8, 9 |
  | 886, 885, 884 `FX_FIRE_L` (radius 40, 70, 100) | 7 | 10, 11, 12 |
  | 904 `FX_FIRE_MAGIC` | 7 | 13 |
  | 827 to 830, 841, 939, 768 `FX_ELEMENTSTONE01` to `07` | `0x36` | 1 to 7 |
  | 832, 833 `FX_DWARFMAGIC` | `0x38` | 1, 2 |
  | 911 to 915 `FX_FORGE_*ELEM` | `0x7B` | 0 to 4 |
  | 1175 to 1178 `FX_UW_GLYPH01` to `04` | `0xB1` | 0 to 3 |
  | 802 `FX_STARGATE` `0x22`, 853 `0x4D`, 854 `0x4E`, 873 `FX_BUTTERFLY` `0x61`, 889 `0x6B`, 891 `0x6C`, 894 `0x6F`, 917 `0x7D`, 918 `0x7E`, 920 `0x80`, 921 `0x81`, 1156 and 1158 `0xA2`, 1157 `0xA3`, 1160 `0xA4`, 1161 `0xA5`, 1162 `0xA6`, 1165 `0xAE`, 1174 `0xB0`, 1179 `0xB2`, 1180 `0xB3`, 1181 `0xB4` | as listed | 0 (1158: 1; 918: the record's `0x03`, "The big demon's summoning") |

  The kind 7's case (`0x5A5AE2`) puts the system at the event's point and starts it: the world's effects stand still where the script put them.
  The point's third coordinate is the record's tag `0x7E` (its case at `0x4861BF` writes the slot that `0x489045` loads as the height; **hypothesis** on the units, the models' height): the braziers' fires of 50,39 carry 28 and sit on the braziers' bowls (**seen**), others 104 and 105, the height of a wall torch (**hypothesis**).
- In the Seraphim's tree (`assets-src/netscript/type_npc_seraphim`, counted 2026-10-27) the sectors' and quests' functions make 1,453 objects 886, 375 of 885 and 42 of 884 (the fires, 1,870), 750 of 812, 183 of 813 and 66 of 876 (the fire smokes), 248 of 882 and 167 of 881 (the lava smokes), 292 geysers 879, 185 of 832 and 715 of 833 (`FX_DWARFMAGIC`), 35 magic fires 904, 26, 4 and 12 chimney smokes (811, 810, 771), 65 gates 802, and a few dozen of the rest.
  The start's sector 50,39 has four fires 886 about the cell (3239, 2532), 50,40 two and 50,38 one; 54,42 one.
- **The spells and the arts**: the executors make their objects with `sdk_CREATE` and send them the kinds that docs/re/arts.md lists (`0x5B`, `0x63`, `0x21`, …).
- **The weapons and the torches**: `cWeapon3D`'s switch of its effects (`0x5CDB60`, with an argument on or off) makes, when on, an `FX_TORCHSMOKE` (787) for the types `0x1600` and `0x1601` (`TORCH_SMALL_ON`, `TORCH_LARGE_ON`, `0x428150`) with the kind `0x13` (the handler follows the torch object, bone 15 `weapon_fx01`), and the weapons' own effects by their bonuses and records: the kind `0xB` by a bonus's kind (`0x5CEA50`, a style 1 to 5), `0x52` for the record `0x6EB`, `0x33` for `0xC01`, `0x3C` for `0xFAA` and `0xFFC`, and `0x1C` or `0x1D` by the weapon's shares of its damage kinds (`+0x13C` to `+0x142`) on the bones 15 and 16; off, it sends the same kinds to end them (not followed further).
- **The weather**: the event of kind 10 makes ten butterflies per entry of type 5 (docs/re/weather.md), each sent the kind `0x61` from `0x419A70`.

### Each frame

- **The tick** (`0x5A6210`): while the object is started and its system alive, `0x5A6680` with the frame's time and 0; when the system is no longer alive (its virtual `+0x0C`, or its word `+0x74` bit 1, which `0x7CBA10(…, 1)` sets to end it), the system is destroyed (`0x7CB960`) and the object with it (`sdk_DESTROY`).
- **The update** (`0x5A6680`): runs the system (`0x7CC080`, virtual `+0x08` with the frame's time), counts the hits' cooldown down, sets the system's tint `+0xA0` to the light's colour at the object's place (`0x41B4A0`, which `0x41B510` computes, docs/re/daynight.md) and `+0xA4` to a height above the ground (`0x7CBBE0`, `0x7CBC20`), runs the action the system asks for (`0x7CBD90`, `0x5B2600`), and the flagged behaviours of the system's flags (`+0x7C`): the targets and the hits (`0x400`, docs/re/arts.md), the wandering (`0x8000`, "The butterflies" below), and others (`0x8`, `0x10`, `0x20`, `0x800`, `0x4000`, `0x10000`, `0x20000`, `0x40000`, `0x80000`, `0x200000`; not read).
- **The draw** (`0x5A9C50`, virtual `+0x14`): the object is an entry of the draw lists (below) and draws itself in its place.
  By the system's flags it first places the system: `0x40` once at the followed object's bone (`+0x70`) with the bone's orientation (`0x7CBB50`), `0x100` once keeps the bone's point at the object's `+0xB8` (the followed creature's place where the bone is not found), `0x200` at each frame at the bone plus an offset (`+0x90` … `+0x98`; for 788 `FX_BURNING_BONE` from Granny's bone state), `0x400000` and `0x1000` three bones' points into the system's points (`+0x72` … `+0x76` into `0x7CBB90`), `4` the system's origin at the object's world place on the ground (its `+0x1C`, `+0x20` in the models' frame at the height 0, `0x7CBC60`) and its position and three points made relative to it (`0x5AA134`), so that its particles ride with the object, `2` the origin `+0x5C` at the position.
  A followed object gone or in another sector ends the system.
  Then it runs `0x5A6680` with the frame's time and 1, adds the light ("Lights"), and has the manager draw the system (`0x7CB770`, the system's virtual `+0x04` with the device and the display's matrix at `sdk_DXDISPLAY + 0x250`, which turns the quads to the camera, **hypothesis** on what the matrix is).

## The particle systems

### The base class

`cParticleSystem`'s constructor (`0x7616F0`) takes a type's number for its logs (`+0x04`, the FX type: 771 for the smoke class, 873 for the butterflies), the owner object's id (`+0xAC`) and the count of particles; the fields that the machinery uses:

| Offset | What |
|---|---|
| `+0x08` … `+0x10` | the position in the models' frame (`0x7CBB00`, `0x7CBF00`) |
| `+0x14` … `+0x34` | an orientation, nine floats (`0x7CBB50`) |
| `+0x38` … `+0x5B` | three points, 12 bytes each (`0x7CBB90`): hands, blade ends |
| `+0x5C` … `+0x64` | an origin added to every particle as it is drawn (`0x7CBC60`) |
| `+0x68` … `+0x70` | the vector of particles, 64 bytes each |
| `+0x74` | a word: bit 1 ends the system |
| `+0x7C` | the flags the object reads (`0x7CBD30`, `0x7CBA90`) |
| `+0x80`, `+0x84`, `+0x88` | the action asked of the object and its data (`0x7CBD90`, docs/re/arts.md) |
| `+0x8C` … `+0x94` | a vector (`0x7CBE20`) |
| `+0x98` | a value: a speed, a radius by the class (`0x7CBEA0`) |
| `+0x9C` | the light's radius (`0x7CBE70`), 0 none |
| `+0xA0` | the tint, the light's colour at the object (`0x7CBBE0`) |
| `+0xA4` | a height above the ground (`0x7CBC20`; **hypothesis**: the object's height less the ground's, `0x4EB6F0` less `0x636310`) |
| `+0xA8` | the count of emitter layers last emitted with |
| `+0xB0` | a looping sound's handle, stopped by the destructor (`0x761A20`) |
| `+0xBC` | a vertex buffer, `count · 0xC0` bytes: six vertices of 32 bytes a particle |
| `+0xC0` … `+0x13F` | the four vertices of a quad's template |
| `+0x2080` … `+0x2090` | the emission's bookkeeping: the dead particles' count and their indices (`+0x1A40`, up to 400), the last emission's time and point |
| from `+0x20A0` | the class's own fields (its clock, its alive flag, its layers, emission blocks, ramps and textures) |

The virtual table, the same order in every class: `+0x00` the destructor, `+0x04` the draw (the device and a matrix), `+0x08` the update (the frame's time), `+0x0C` alive, `+0x10` the start (the FX event), `+0x14` and `+0x18` the state written to and read from a file (the saves and the network; `0xD9C` bytes from `+0x20A0` for the smoke class).

### A particle

64 bytes (`0x7640C0`, `0x764760`, `0x761B00`):

| Offset | What |
|---|---|
| `+0x00` … `+0x08` | the position |
| `+0x0C` … `+0x14` | free for the class (the butterflies' offset from the swarm's centre) |
| `+0x18` … `+0x20` | the velocity |
| `+0x24` | the buoyancy `g`: each second the vertical velocity falls by `g`, so a negative `g` rises |
| `+0x28` | the size, the half side of its quad; the particle dies at 0 |
| `+0x2C` | the life, from 255 at its birth down to 0, where it dies |
| `+0x30` | the angle in the screen's plane |
| `+0x34` | its own colour (ARGB) |
| `+0x38` | the angle's speed |
| `+0x3C` | a byte: the frame of the atlas |
| `+0x3D` | a byte: its layer |

### The update

`0x7640C0` (the frame's time `dt`, the class's layers): for each living particle (life and size above 0) the position moves by `dt` · velocity; with `k = dt · g`, the vertical velocity falls by `k` and the velocity gains `k` times the layer's wind (`+0x10` … `+0x18`); the angle turns by `dt` · its speed; with the layer's pull (`+0x1C`) not 0 the velocity is drawn toward the system's position; then `g`, the life, the size and the angle change by `dt` times the layer's rates.
A dead particle's index goes to the free list.
With its last argument 1 the life wraps from 0 to 255 (a looping particle).

**A layer**, 32 bytes, up to four side by side: `+0x00` the rate of `g`, `+0x04` the life's rate (negative: −200 lives 1.275 s), `+0x08` the size's rate, `+0x0C` the angle's rate, `+0x10` … `+0x18` the wind, `+0x1C` the pull.
The smoke class writes the renderer's vector (`+0x68` … `+0x70` times `+0x74`, docs/re/weather.md) into its layers' wind at each frame, so its smoke drifts with the time of day's vector (**hypothesis**: the shadows' direction).

### The emission

`0x764760` (the class's clock, its emission blocks, its layers, a point, the count of layers `n`): the particles leave the system's position (or one of its three points; the origin 0 with the system flag `2`).

**An emission block**, 100 bytes per layer, side by side as the layers are: `+0x00` `g` and `+0x04` its spread, `+0x08` the size and `+0x0C` its spread, `+0x10` the angle and `+0x14` its spread, `+0x18` the angle's speed and `+0x1C` its spread, `+0x20` … `+0x28` the velocity and `+0x2C` … `+0x34` its spreads, `+0x38` … `+0x40` an offset of the position and `+0x44` … `+0x4C` its spreads, `+0x50` a colour and `+0x54` its spread (a random colour about it where `+0x54` is not 0), `+0x58` the interval between two particles in seconds, `+0x5C` the count of a burst, `+0x60` a byte, the frame: 0 the first, `0xFF` the layer's index, another `m` a frame at random below `m`.
A spread `s` adds a uniform `[−s, s]`.

- With the first block's interval 0 the emission is a burst: up to `+0x5C` particles into dead slots at once (the classes call it at their clock's 0).
- Else it emits as many particles as whole intervals have passed since the last emission, each pre-aged by its share of the time, their places spread along the line from the last emission's point to the present one, so that a moving system leaves an even trail; a shortage of dead slots is logged (`ParticleSystem %s WARNING`).
  The interval is the first block's, and so is the offset `+0x38` of every particle's place but the very first's: a particle of another block takes that block's spreads `+0x44` about the first block's offset, never its own block's offset (read 2026-11-04 for #2070, **confirmed** by code; **seen** by the magic fire, below).
- Each particle takes the block of a layer that `0x7687F0` picks: with `n` 1 the first; with 2 the first four times in five, the second once; with 3 or 4 by the blocks' rates of emission (the inverse of their intervals), by one roll of `rand() / 32767`.

The intervals are multiplied by the detail (`DETAILLEVEL`, the byte `0x182EE78`): 1.0 at 2, 1.5 at 1, 2.2 at 0, in every class read.

### The colour ramps

`0x763F50` fills a table of 256 ARGB words (`0x400` bytes) from a start colour and keys of five words (red, green, blue and alpha in 0 to 1, then a count of entries), each key reached linearly over its count, from the entry 255 down: the entry 255 is the colour at the birth and the entry 0 at the death, and the counts sum to 256.
A class keeps one ramp per layer, side by side; the draw takes the entry of the particle's life, rounded down.

## Drawing and blending

### Where in the frame

- The FX objects are dynamic objects of their cells' chains, and the world pass appends them to the draw lists as entries of kind 0 (docs/re/drawing.md): at `0x62C3F8` … `0x62C633` (a building's floors; the cell's own chain at the other call sites of `0x4280B0`, alike, **hypothesis** for those), after the cell's other objects, a second walk takes the objects of the category 12 alone (`0x4280B0`): a type of the flag `0x800000` (`0x4280F0`) to list 4, a type of the flag `0x4` (`0x4280D0`) on the layer 0 to list 0, any other to list 3.
- So the chimney smokes (771, 810, 811, flags `0x801002`) lie over everything, the blood puddles, the magic marker, the temporary texture and the big demon's summoning (the five of kind 6) under everything on the ground, and the rest among the sprites and models of list 3 by their cells.

### The billboards (`0x761B00`)

The common draw of particles, called with the device, the matrix, a flags word, the ramps, the atlas's grid `G`, a layer for trails (0 for none), a size factor, a byte `b` and a byte `c`:

- States first (`0x643430`, docs/re/drawing.md): the depth test on (`1`), the depth write off (`0x100` called with 0; with 1 it is on, "The stargates"), the lighting off (`0x20`), the blending on (`4`), the linear filter (`0x800`), the source factor `ONE` with the flag `0x10` else `SRCALPHA` (`8`), the destination `ONE` with the flag `0x01` else `INVSRCALPHA` (`0x10`).
- Each living particle becomes a quad of its size times the factor about its position plus the origin, in the plane of the matrix (camera-facing), as two triangles of pre-lit vertices (FVF `0x1E2`, `D3DFVF_LVERTEX`: a position, a diffuse and a specular colour, one pair of coordinates) transformed on the CPU (`0x6545E0`), the world transform the identity, batched into the buffer `+0xBC` and drawn as a triangle list (`DrawPrimitive`, the device's `+0x64`).
- The flags word:
  - `0x01` the destination `ONE`, `0x10` the source `ONE`;
  - `0x02` the four corners' colours (the ramp's first four words) with the alpha of the life;
  - `0x08` the colour of the ramp, the particle's layer's at its life;
  - `0x20` the particle's own colour with the alpha of the life;
  - none of the three: the four corners' colours as they are;
  - `0x04` the quad turned by the particle's angle;
  - `0x40` every colour multiplied by the tint `+0xA0` (the light at the object, so the effect darkens by night), unless the tint is white;
  - `0x80` the quad standing on its foot, from 0 to twice the size up, instead of centred;
  - `0x100` the atlas's frame from the particle's byte `+0x3C`, else from its age, `⌊(255 − life) · G²/256⌋`, so that a particle plays the atlas's `G × G` frames once over its life.
- With `G` above 1 the texture is a grid of `G × G` frames and each quad takes its frame's square.
- When the system last emitted with two layers (`+0xA8` 2, or the byte `c`), the batch is drawn and the destination factor switched at each change of layer: the layer 0 additive (`ONE`) where the byte `b` is 0, the others by the alpha (`INVSRCALPHA`); where `b` is 1 the other way about, the layer 0 by the alpha and the others added (`0x761FD9`…`0x762005`, the Dwarf's guns' fires).
  So a fire's flames add to the frame and its smoke covers it.
- The particles are drawn in their slots' order, not sorted; the depth test against the models and the depth buffer's other contents is their only order inside the system.
- A trail's layer, when given (the fourth argument, tested at `0x7623D8`), draws each particle again as shrinking copies back along its velocity (0 in the classes read here).

### The other primitives

- **A glow** (`0x762E10`): one camera-facing quad of a half side `s` at the system's position or one of its points plus an offset, of one colour, the depth test as asked, the destination always `ONE`; its texture by a style: 0 `PARTICLE_FLARE02`, 1 `GLOW04`, 2 `GLOW05`, 3 `GLOW01` (source `SRCALPHA`), 4 `SPARK07`, 5 `GLOW03`, 6 `GLOW06`, 7 `GLOW07`, 8 `FLARE04`, 9 `GLOW08` (source `ONE`), `0xFE` and `0xFF` the texture already set with `SRCALPHA` or `ONE`.
  41 classes use it.
- **A flat quad** (`0x7631E0`): a square of half side `s` in the horizontal plane at the system's position, turned by an angle, of a texture and a colour, a frame of a grid, the source `ONE` or `SRCALPHA` and the destination `ONE` or `INVSRCALPHA` as asked: the magic circles, the ground symbols, the puddles, the decals (50 classes).
- **A ring** (`0x763530`): a band of `n` segments between two radii, flat or stood up (turned a quarter about x), of a texture, additive or blended (7 classes: the explosions, the shock waves of the Gladiator's hits, the rings of poison).
- **A helix** (`0x762C30`): two strips of 50 segments turning about the axis, additive (3 classes).
- Others draw their own geometry (`DrawPrimitive` directly, 50 classes), Granny models (`cGranny::render`, 16 uses: the bubble trap's bushes, the demons' illusions; the killer plants' class draws none, "The thorns' plants"), and a few further helpers (`0x762650` a streak, `0x768600` and `0x768790` chains, `0x7638E0`, `0x763CB0`; not read).

### What the textures are

The particles' textures are `texture.pak`'s `PARTICLE_*.TGA` and `FX_*.TGA`, converted to `assets-src/textures/texture/<number>-<name>.png` (63 `particle_*`: the numbers 14865 to 14927).
Two kinds, by their pixels (counted 2026-10-27): white with an alpha mask, tinted by the vertices' colour and blended by alpha (`PARTICLE_FIRE03` 128², `SMOKE03` 256² of 2 × 2 frames, `WATER01`, `GLOW01` 16² of alpha up to 153, `BUTTERFLY01` 128² of 8 × 8 frames of 16², grey wings), and colour on black, opaque, drawn additively (`PARTICLE_FIRE01` 256², a 4 × 4 flip-book of a burst of flame, `GLOW03` 64², `MULTI02` 256², `SPARK06`).

## Lights

- An FX object gives light through its system: its draw (`0x5A9C50`) adds to the frame's table of lights (`0x41B200`, docs/re/daynight.md) the object's world place with the system's radius `+0x9C`, at each frame it is drawn, where the radius is above 0.
  The place is the object's `+0x1C`, `+0x20` on the ground at the height 0 (`0x5AA26E`, `0x41B200` into `0x622620`), whatever the height its system draws at; the table takes it by night or in a dungeon alone, of a radius above 10.
  The radii the classes' starts set: the fires 280, 370 and 530 (variants 10, 11, 12) and the fire smokes alike (4, 5, 6), the magic fire 230, the fireball 250, the fire strike 200, the burning bone 150, the dwarf's magic 350 or 400, the gates and the spells others (the catalogue).
- The draw lists' path for an object of the type flag `2` (`0x631358`) adds a light only for a creature (a cast to `cCreature`, then `0x54A7B0`), so the items table's radius of an FX type (`+0x32` with the flag `0x20000`) gives no light; nor are FX types statics, whose radius `0x41B170` takes.
- The torch's flame gives none of its own: the torch's light is the hero's rule (docs/re/daynight.md: 300 in a dungeon, half as much again outside, from the types `0x1600` and `0x1601` in the creature's objects).

### The systems' Direct3D lights

Read 2026-11-05 for #2106 in Ghidra (**confirmed** by code unless marked).

- The light manager `cDX7LightMgr` (one object at `0xCD7A78`, made by `0x642880`, its set-up `0x642910` at the start): `0x642B90` (a place or a direction, a colour, a kind, a priority) makes a light in its first free slot, else in the slot of the lowest priority where the new one's is at least as high, else none (the handle 0); it sets it on the device (`SetLight`) and turns it on (`LightEnable`).
- The kind 2, which every system passes, is a point light: its diffuse and specular the colour, its ambient black, its range 200 and its attenuations (1, 0, 0.000125) set by the manager, so every system's light falls as `1 / (1 + 0.000125 d²)` within 200 of it, a sixth at its edge.
  The kind 1 is the world view's directional light (`0x624825`): white, its ambient 0.3 of its colour, the priority 4, recoloured before the models' draws (`0x642FD0`, the light at the model, docs/re/daynight.md); a system's light, of the priority 1 or 2, never takes its slot.
- The device's lights are as many as its count with hardware transform and lighting (`D3DDEVCAPS_HWTRANSFORMANDLIGHT`, the display's `+0x358`), else 4; the default material diffuse 1, ambient 0.7, specular 1 of the power 30.
- A system makes its light in its constructor at the world's origin, moves it at its draw (`0x642EF0`, or `0x642DF0` with a new colour), and takes it off in the base's destructor (`0x642D90`, `0x761A67`); it never fades with the system.
  So, once on, it lights every model within 200 of it in its colour in the fixed function's sum, the sprites and the ground, which the game draws unlit, untouched.

| Class | FX type | Colour | Priority | Where, and what changes |
|---|---|---|---|---|
| firestrike, the fireball | 778 | (1, 0.8, 0.4) | 2 | at its place |
| torchsmoke, the torches' flames | 787 | (1, 1, 0) | 2 | at its place |
| groundPoison | 792 | (0.3, 1, 0.3) | 2 | at its place |
| lightningStrike | 798 | (0.8, 0.8, 1.0) | 2 | at its place |
| freeze, the chain lightning | 799 | (0.7, 0.7, 1.0) | 2 | made only for its event's `+0x3C` 0, the first bolt (`0x77DE80`) |
| holylight | 804 | (0.8, 0.8, 1.0) | 2 | at its place |
| sera_sm_cast | 844 | (0.7, 0.7, 1.0) | 1 | made for 1018 alone, once its delay is over (`0x790680`) |
| highspeed | 846 | (0.2, 1, 0.2) | 1 | at its place |
| killerplants, the thorns' plants | 848 | (0.25, 1, 0.25) a hero's, (0.25, 0.25, 1) another's | 1 | at its place (`0x799640`) |
| plantcage | 849 | as 848's | 1 | at its event's point ± 50 along x and y at random |
| lostsouls | 850 | (0.2, 1, 0.2) | 1 | at its place |
| beherrschung | 857 | (1, 0.28, 0.28) | 2 | at its orbiting glow |
| vamp_sm_cast | 863 | (0.8, 0.8, 1.0) | 1 | moved for 1022 and 1023 alone, to its glow's point; else at the origin |
| freeze_impact | 872 | (0.7, 0.7, 1.0) | 2 | at its place |
| trap03_laser | 895 | (0.7, 0.7, 1.0) | 2 | only for its event's `+0x3C` 0 |
| flyingskull | 905 | (1, 0.8, 0.1) | 1 | at its place while it flies, then where it stopped |
| laserbeam_impact | 909 | (1, 0.8, 0.2) | 2 | at its place |
| bigdemondeath | 919 | (0.8, 0.8, 1.0) | 2 | at its place |
| shaddar_slave | 921 | (1, 0.8, 0.7) | 1 | at its place |
| shaddar_magic, the fire bolt | 922 | (1, 0.8, 0.4) | 1 | at its place |
| helldisk | 1005 | (1, 0.4, 0.2) | 1 | at its place |
| demonform | 1014 | by its variant: (0.5, 0.7, 1), (0.4, 1, 0.5), (1, 0.7, 0.4), (1, 1, 1) | 1 | at the first of its orbiting points |
| demonfire | 1015 | (1, 0.4, 0.2) | 1 | 45 + 3 sin(1.17 t) over its place |
| dwarf_flamethrower, dwarf_shoot, dwarf_grenade | 1016, 1017, 1018 | (1, 0.8, 0.4) | 2 | at the muzzle each frame, the flamethrower's off as its last flame dies, the shots' as their emission ends (their delay `+0x20A4` is always 0; no world light) |
| panzer | 1021 | (0.15 + 0.35(1 + c), 0.15 + 0.35(1 − c), 0.137), c = cos(0.5425 t) | 2 | at its place |
| stahl | 1022 | (0.25 + 0.2 c, 0.25 − 0.2 c, 1), c alike | 2 | at its place |
| dwarf_gier | 1155 | 2 sin(5π τ) times (1, 0.5, 0.1), τ a timer of 0.2 s | 1 | each 0.2 s at a new point within (18, 18, 20) of its place less 15 |
| dryad_ghost | 1162 | (0.2, 1, 0.2) | 1 | at its place |
| terrorsprinter | 1167 | (1, 0.1, 1) | 1 | for its event's `+0x38` 0 alone |
| gunfire | 1172 | (1, 1, 1) | 1 | 5 along its shot's way from its place (**hypothesis**) |

`opensacred-game` follows them since #2106 (`fx_lights`) for the systems it draws: the fireball's, the torches' flames', the ground poison's, the lightning strike's, the chain lightning's first bolt's and its impacts', the holy light's, the thorns' plants', the lost souls' and the dryad ghosts', the flying skull's, the fire bolt's, the hell disk's, the demon fire's and the guns' flashes', each a Bevy point light on the models of its colour and its range, following its system while it moves and taken off with its look.
Bevy's point light falls by the square of the distance alone, so it stands 89 of the models' units (`1/√0.000125`) over the effect's place, where along that place's plane it falls as the fixed function's, the sun's 4,000 lux its whole colour; it lights the models from above rather than from the effect's place, and fades to nothing at 200, where the fixed function's is cut at a sixth (docs/decisions/drawing.md).
**Seen** 2026-11-05 in the game: a goblin standing in the Wood Elf's thorns, its legs lit green as the original's (#2106's frames).
Not followed: the device's count of lights and the manager's priorities (every light shines), the demon fire's wobble, and the systems the game does not draw yet.

## The world's effects

### The fires and smokes: `cParticleSystem_smoke`

One class of 200 particles (constructor `0x76E200`, virtual table `0x8979D0`, start `0x76E980`, update `0x76E2D0`, draw `0x76E600`), its textures `PARTICLE_SMOKE03`, `FIRE03`, `WATER01` and `MULTI02` (fields `+0x2E40`, `+0x2E3C`, `+0x2E44`, `+0x2E48`).
The start keeps the event's variant (`+0x20A8`), sets the clock (`+0x20A0`) to 0 and alive (`+0x20A4`); the update advances the clock, writes the wind, updates the particles over the variant's layers and emits continuously from them; the draw binds the variant's texture and draws the billboards once.
The layers are at `+0x2CAC` (32 bytes each), the emission blocks at `+0x2D0C` (100 bytes each), the ramps at `+0x20AC`, `+0x24AC` and `+0x28AC`.
Distances in the models' units, the intervals at the detail 2 (multiply by 1.5 or 2.2 below), "±" a spread:

| Variant | Types | Texture, flags, grid | Layers (life, size per second) | Emission (buoyancy, size, place, velocity, interval) | Colours | Light |
|---|---|---|---|---|---|---|
| 1 | 771 `FX_HS_SMOKE_S` | `SMOKE03`, `0x146`, 2 × 2 | −300 (0.85 s), +10 | −50, 6 ± 2, ± 2, (0, 0, 1) ± 0.3, spin ± 3, a frame of 4, 0.04 s | `0xE0E0E0`, its alpha the life | 0 |
| 2 | 810 `_M` | `SMOKE03`, `0x146`, 2 × 2 | −120 (2.1 s), +10 | +1.5, 5 ± 2, ± 2, (0, 0, 23) ± 2, spin ± 3, a frame of 4, 0.1 s | as 1 | 0 |
| 3 | 811 `_L` | `SMOKE03`, `0x146`, 2 × 2 | −75 (3.4 s), +8 | +2, 5 ± 2, ± 2, (0, 0, 30) ± 2, spin ± 3, a frame of 4, 0.1 s | as 1 | 0 |
| 4 | 812 `FX_FI_SMOKE_S` | `FIRE03`, `0x0C`, whole | flame −200 (1.275 s), +15; smoke −180 (1.4 s), +10 | flame −35 ± 34, 0.5 ± 0.2, x ± 4, ± 0.2; smoke −35 ± 5, 1.0 ± 0.5, x ± 4, ± 0.3; 0.02 s | flame ramp A; smoke ramp B at 0.5 | 280 |
| 5 | 813 `_M` | as 4 | flame −150 (1.7 s), +17.5; smoke −130 (2 s), +10 | flame −35 ± 34, 0.75 ± 0.35, x ± 9, ± 0.35; smoke −35 ± 5, 2 ± 0.75, x ± 9, ± 0.4; 0.02 s | ramp A; ramp B at 0.77 | 370 |
| 6 | 876 `_L` | as 4 | flame −100 (2.55 s), +20; smoke −80 (3.2 s), +10 | flame −35 ± 34, 1 ± 0.5, x ± 9, ± 0.5; smoke −35 ± 5, 3 ± 1, x ± 9, ± 0.5; 0.02 s | ramp A; ramp B at 1.0 | 530 |
| 7 | 879 `FX_GEYSIR` | `WATER01`, `0x46`, whole | −350 (0.73 s), +15 | +100 (falls), 5 ± 1, ± 1, (0, 0, 40) ± (0, 0, 5), 0.1 s; in a burst (0, 0, 200), life −75, 0.035 s | `0xD0D4E4`, the alpha the life | 0 |
| 8 | 882 `FX_LV_SMOKE_M` | `SMOKE03`, `0x10C`, 2 × 2 | −200, +12 | −35 ± 5, 1 ± 0.75, x ± 5, ± 0.4, spin ± 1, a frame of 4, 0.03 s | ramp B at 0.77 (start at alpha 1) | 0 |
| 9 | 881 `_L` | as 8 | −150, +14 | −35 ± 5, 1.5 ± 1, x ± 20, ± 0.5, spin ± 2, a frame of 4, 0.03 s | ramp B at 1.0 (start at alpha 1) | 0 |
| 10 | 886 `FX_FIRE_L` (radius 40) | `FIRE03`, `0x0C`, whole | flame −200 (1.275 s), +8; smoke −200, +10 | flame −35 ± 34, 0.5 ± 0.2, (x ± 3.5, y ± 1), ± 0.2; smoke −35 ± 5, 1 ± 0.5, same, ± 0.3; 0.02 s | ramp A; ramp C | 280 |
| 11 | 885 (70) | as 10 | flame −160 (1.6 s), +10; smoke −150 (1.7 s), +10 | flame −35 ± 34, 0.75 ± 0.35, x ± 7, ± 0.35; smoke −35 ± 5, 2 ± 0.75, x ± 7, ± 0.4; 0.02 s | ramp A; ramp C | 370 |
| 12 | 884 (100) | as 10 | flame −100 (2.55 s), +12; smoke −100, +12 | flame −35 ± 34, 1 ± 0.5, x ± 9, ± 0.5; smoke −35 ± 5, 3 ± 1, x ± 9, ± 0.5; 0.02 s | ramp A; ramp C | 530 |
| 13 | 904 `FX_FIRE_MAGIC` | `MULTI02`, `0x11D`, 2 × 2, a frame per layer (`0xFF`): the specks, the cloud, the star | specks −180 (1.42 s), +11; cloud −256 (1 s), +1; star −768 (0.33 s), 0 | specks −35 ± 5, 5, spin ± 1.5, ± (4, 4, 0), 0.0265 s; cloud −70, 5, ± (4, 4, 0), 0.06545 s; star 0, 8 ± 0.5, ± 2, 0.02222 s; every one turned π ± π, the second and third blocks' offset (0, 0, 20) unread | specks from (255, 255, 255) over 20 entries to (255, 205, 248), over 82 to (254, 12, 223), over 65 to (39, 137, 162), over 89 to black; cloud from (255, 184, 97) over 166 to (90, 52, 253), over 90 to black; star from (247, 167, 245) over 110 to (172, 9, 142), over 146 to black (out of 256) | 230 |

- Ramp A, the flame: from (254, 243, 131) at an alpha of 90 to (254, 80, 40) opaque over 22 entries, to (232, 67, 20) at 229 over 29, to black at 139 over 85, to transparent black over 120.
- Ramp B, the fire smoke: from a grey of 60 at an alpha of 0 to a grey of 33 at the variant's alpha over 110 entries, to transparent black over 146.
- Ramp C, the fire's smoke: from a grey of 159 at an alpha of 0 to a grey of 133 at 80 over 110, to a grey of 100 at 0 over 146.
- The flags `0x0C` with two layers draw the flames additively by their alpha (`SRCALPHA`, `ONE`) and the smoke by its alpha (`SRCALPHA`, `INVSRCALPHA`), turned, coloured by the ramps, not tinted: a fire keeps its colour by night.
  The flags `0x146` (the chimney smokes) tint by the light at the object; `0x10C` (the lava smokes) add their colour with the source `ONE`; `0x11D` adds all.
- Every flame and smoke particle rises by its own buoyancy (−35 ± 34 for a flame, a rise of up to 69 units a second gained each second) and drifts with the wind; its quad grows from about one unit to 10 to 30 units of half side over its life.
- The variants 5 and 6 set the system flag `0x400`, so that the fire smokes of those sizes burn the creatures in them (`FX_FI_SMOKE_M` 10 and `_L` 20 of the first two kinds, docs/re/arts.md); the fires 884 to 886 and the small fire smoke do not.
- The geyser's update keeps a timer (`+0x2E38`) of 3 to 20 s at its start: while it is between 0 and 1 s the jet bursts upward at 200 with the flag `0x400` (its hit of 100) and plays once the sample 211 or 212 (one in two) at 0.4 to 0.6 of the volume, through a shared latch (`0x17EEBC8`); below 0 it takes 30 to 100 s anew.
- The variant 13 emits through `0x764760` with its three blocks (`0x76E2D0`), the system every 0.0265 s, each particle of a block by its share of their rates (0.38, 0.16, 0.46); all are added whole (`ONE`, `ONE`), the three layers in one batch.
  **Seen** 2026-11-04 at 97,60 (the braziers about the altar of `Sector97060Enter`, heights 26 and 28): a bright pink and white ball on each bowl, blue and teal wisps rising above it.
- **Seen** 2026-10-27 at 50,39: the four braziers around the cell (3239, 2532) burn on their bowls, a small orange flame of about 20 pixels at zoom 1 with a faint grey smoke above it; by night the flame keeps its colour while the scene darkens, its smoke lighter than the ground about it.

### The torches' flames: `cParticleSystem_torchsmoke`

787 `FX_TORCHSMOKE`, 200 particles (constructor `0x774F00`, start `0x7752B0`, update `0x774FE0`, draw `0x775050`), made and ended by the torch's `cWeapon3D` (above), following the torch's bone 15 `weapon_fx01` at each frame (system flags `0x204`).
Its frame's origin is the holder's place on the ground (the flag `4`), so that its flames ride with the holder and trail only the bone's swing about it.

- One layer: the life −800 a second (0.32 s), no growth.
- One block: a size of 7, a random angle, the velocity (0, 0, 50) ± 10 on each axis, the place ± 1.5 on each axis, every 0.03 s.
- The draw: `PARTICLE_FIRE01` as a 4 × 4 flip-book, the flags `0x15` (additive `ONE`, `ONE`, turned), white: each flame plays the sixteen frames of a burst over its 0.32 s while it rises 16 units; then a glow of style 3 (`GLOW01`, `SRCALPHA`, `ONE`) of a half side of 20, the colour (255, 234, 59) at an alpha of 128, at the bone.
- The start plays the sample 219 at the torch, looping, its handle kept at `+0xB0` and stopped with the system (**hypothesis**: the torch's crackle).
- An end sent by the off switch (`+0x20A8` 1) stops the emission; the system dies when its last flame does.
- No light of its own (above).
- **Seen** 2026-10-27: with `TORCH_SMALL_ON` in the Seraphim's left hand by night, a bright yellow-white ball of flame at the torch's head, with the scene lit about the hero.
  **Seen** 2026-11-04 with `TORCH_LARGE_ON` in her right hand: as she runs, the flame stays a ball at the torch with a short streak down her arm, no trail behind her.

`opensacred-game` follows the flame since #1092 (`torches`, `opensacred_rules::particles::Torchsmoke`): a torch's model in a hand of the hero or an NPC, `torch_small_on` or `torch_large_on`, burns at its bone as above, its flames in its holder's frame, untinted by the night, with its loop at 0.4; the torch gone, its flames burn out.
**Seen** 2026-11-04 beside the original at the Seraphim's start by night (the start's torch in her right hand): the flame of the same size and place standing, the original's a little more orange, and the same streak down her arm as she runs.
**Seen** 2026-11-04 beside the original at 52,39 by night: the garrison's guard of the type 295 with the torch in its left hand that it takes by night (docs/re/ai.md, "The torches by night"), its flame of the same size at the torch's head.

The world's wall torches and fire pits are statics (`DUN_torch1` to `4`, `TORCH001`, `Firepit 1`, `firepit002` to `5`, `FIREPIT06`, `3S_FIRE`, `UW_FIRE_01`, `02`: type 9 sprites); their flames are the scripts' FX objects placed with them (`FX_FIRE_L`, `FX_FI_SMOKE_S`, at their heights `0x7E`; **hypothesis** for which goes with which).

### The butterflies: `cParticleSystem_butterfly`

873 `FX_BUTTERFLY`, eight particles (constructor `0x79AF80`, virtual table `0x8984DC`, start `0x79B730`, update `0x79B020`, draw `0x79B3E0`), its textures `PARTICLE_BUTTERFLY01` (`+0x2144`) and `PARTICLE_GLOW03` (`+0x2148`).

- **The event** (kind `0x61`, `0x5A3B0E`): the object takes a random direction in the plane (`+0x78`, `+0x7C`; each part uniform in −1 to 1, normalised) and a turn rate of 0 to 0.3 radians a second (`+0x8C`); the event's `+0x38` of 1 (one swarm in 31, docs/re/weather.md) sets the object's `+0x48` bit 4 and the system's glow (`+0x2140`).
- **The start**: a swarm of `3 + rand % 4` butterflies (3 to 6, `+0x213C`), the system flag `0x8000`, a speed `+0x98` of `⌊20 · (1 + U)⌋` (20 to 40) units a second, no light; one block: a size of 4, the place 20 units up ± (5, 5, 12), the colour grey 128 with a spread of 127 on each channel (a random colour each), a burst of all of them.
- **The swarm wanders** (the update's flag `0x8000`, `0x5A8885` …): at each tick the system's position moves by the speed along the direction, and the direction turns by the turn rate times the frame's time; where the new place is blocked or may not be walked on (`0x636D70`, `0x636D20`) the swarm goes back to where it was, the direction turns back (by π) and the turn rate takes a new 0 to 0.2.
  **Seen** 2026-10-27: four swarms drawn in 54,42 at once, of 3, 5, 6 and 6, each moving some 0.3 units a frame, about 20 a second.
- **Each butterfly** (the system's update): its life falls by 100 a second and wraps (a loop of 2.55 s); a timer (`+0x30`) of 0.5 to 1.2 s, at whose end it takes a new velocity: a random direction in three dimensions times 13 plus 8 toward the point 50 units above the swarm's centre; its offset from the centre moves by its velocity, its position is the centre plus the offset.
- **The draw**:
  - while the night's level `N` (the renderer's `+0x12E54`) is below 240: the butterflies as billboards of `BUTTERFLY01` with the flags `0x20` and an 8 × 8 grid, so each plays its 64 frames (a wing's beat) over the 2.55 s loop and takes its own colour with the alpha of its life, blended by alpha; then, for a swarm of the glow, a glow of style 5 (`GLOW03`, additive) of a half side of 7.5 at the first butterfly in its colour;
  - while `N` is above 16: for each butterfly a glow of style 5 of a half side of 5 in the colour (`N`, `N`, `N/2`), additive: fireflies, faint at dusk, yellow by night.
  - So by day the swarms are tinted butterflies, by night only fireflies, and both at dusk and dawn.
  - **Seen** 2026-10-27: by noon small coloured specks in 54,42; by night (23:30) clusters of three to six yellow-green glowing dots.
- The swarm's butterflies are born together, so their alpha and their frames run in step: the whole swarm fades from opaque to nothing over 2.55 s and comes back at once (by code; **hypothesis** that it shows so, not checked frame by frame).

### The undead's effects: `cParticleSystem_puzzleSolved_v`

Read 2026-10-28 for #1261 (**confirmed** by code): the class of 781 (`TYPE_FX_PUZZLE_SOLVED_V`), its virtual table `0x897964` (the draw `0x76CF60`, the update `0x76CF20`, the start `0x76D260`), makes its particles once as it starts and only ages them by its one layer (`0x7640C0` with `+0x24A8`), alive while one lives.
Its layer is at `+0x24A8` and its one emission block at `+0x24C8`, the ramp at `+0x20A8`; the start clears both and keeps the event's variant (`+0x38`) at `+0x252C`, by which the draw chooses the texture and the flags: the variants 0 and 2 `+0x2530` (`PARTICLE_SPARK06`) with `0x19`, 1 `+0x2534` (`SMOKE03`) with `0x106` in a 2 × 2 grid, 3 `+0x2538` (`GLOW03`) with `0x19`, 4 and 5 `+0x2534` with `0x10C` in a 2 × 2 grid.

- **The variant 2**, the ring of the Vampiress's change (docs/re/arts.md, "Her change's event"; read 2026-11-05 for #1092): 50 particles on a ring of 17.5 about her, 40 over her place, at the angles of a 50th of a turn, the even ones falling at 70 and the odd rising at 70, all of the buoyancy −10; a size of 25, the life's rate −512 (half a second) and the size's −30; the ramp from (1, 0.5, 0.5) opaque to black over 256 entries, added whole (`PARTICLE_SPARK06`, the flags `0x19`); the system flags `0x202`, riding with her; the sound 2085 once (`0x76D545`).
  So a pink ring flares about her waist and splits in two, one rising over her head and one sinking to her feet, gone in half a second.
  `opensacred-game` makes it since #1092 (`forms`, `opensacred_rules::particles::change_ring`, drawn by `puffs`) at each change; **seen** 2026-11-05 in the game, the pink ring splitting about her at her change to the night's form; the original's frame of the same moment shows it under her cast's sight (863, `vamp_sm_cast`, not drawn yet).
- **The variant 3**, the ring of an undead that rises again (docs/re/fight.md, "The undead that rise again"): 75 particles on a ring of 17.5 about the system's place, at the angles of a 75th of a turn, each rising at 70, the even ones with a buoyancy of +100 that brings them back down from about 24.5 at 0.7 s; a size of 10, the life's rate −200 (1.275 s) and the size's −8; the ramp from (0.898, 0.0195, 0.996) opaque, magenta, to (0.0117, 0.422, 0.996) over 126 entries, to (0.0664, 0.621, 0.9375) at 0.949 over 45, to transparent black over 85; the system flags `0x204`, no light.
- **The variant 4**, the dust of a corpse that vanishes (docs/re/fight.md, "The other deaths of class 5"): a burst (the block's interval 0) of 150 particles at the corpse model's skinned vertices (`0x44FC30`, `0x767FB0`), the buoyancy −20 ± 8, the size 10, the angle π ± π, the angle's speed 0.5 ± 0.2, the velocity 0 ± 3 along each axis and a frame of 4; the layer's rates of the buoyancy −10 (they rise ever faster), the life −100 (2.55 s), the size +8 and the angle +1, its wind the renderer's vector as the smoke class's; the ramp of (0.898, 0.836, 0.664), a pale bone colour, from an alpha of 0.5 to 0 over 256 entries.
- The variant 5 is the variant 4's of (0.625, 0.684, 0.625), a pale green, the buoyancy −16 and the size 7 (its user not read); 0 is a burst of 500 at the skinned vertices (the buoyancy −5 ± 5, the size 2, the life's rate −150, from (0.8, 0.9, 1.0) to opaque black, which adds nothing); 1 a burst of 250 there (the size 10, the angle π ± π, the life's rate −150, the buoyancy's −1 and the angle's 1, a black ramp); 2 a ring of 50 at the 50th of a turn (0.1257) of 17.5 at a height of 40, rising and falling at ± 70 with a buoyancy of −10, of a size of 25, the life's rate −512 and the size's −30, from (1, 0.5, 0.5) to opaque black, with the sound event `0x825` and `(9, 0x49)`; none of them read for its users.

### The hit sparks: `cParticleSystem_*_sm_hit`

Read 2026-11-04 for #1092 (**confirmed** by code unless marked).

- Five types alone, no others: 842 `FX_GLAD_SM_HIT`, 843 `FX_SERA_SM_HIT`, 860 `FX_ELVE_SM_HIT`, 862 `FX_VAMP_SM_HIT`, 864 `FX_DARK_SM_HIT`, of the FX events' kinds `0x42`, `0x44`, `0x55`, `0x57` and `0x59`.
  Their handler's cases (`0x5A522D`, `0x5A527D`, `0x5A52A3`) follow the event's `+0x08`, the struck creature, at the bone of its `+0x38`, and the system flag `0x40` places the system there once, at its first draw.
- The heroes' executors make one right after each strike they send (`0x4EA3F0`), with no test of whether it lands, so a miss sparks too; the event's `+0x3C` is the art's id.
  The bone is 3 (`Bip01 Neck`), or 0 (`__Root`) where the struck model has no neck (`0x44CB80`); the kicks' sparks burst at the kicker's own foot (36 `Bip01 R Foot` for the Gladiator's 1060 and the Vampiress's 1041 and 1076, 37 `Bip01 L Foot` for the Seraphim's 1072), the Dark Elf's kick's at its target's neck (**hypothesis** on whose).
- By the art: 842 for the Gladiator's `Attacke`, `Hardhit`, `Rundum`, 1012 and his roar 1014 at each creature roared at, the Dwarf's `Attacke`, `Hardhit`, `Rundum` and 1084, the Daemon's `Attacke` and his charge's landing 1091, and the Vampiress's kick; 843 for the Seraphim's `Attacke`, `Hardhit`, `Rundum`, 1020, 1054 and her jump 1017 (at the struck's root), and the Mage's `Hardhit` and `Rundum` (1001, 1000); 860 for the Wood Elf's `Attacke`, `Hardhit` and `Rundum` and the Mage's `Attacke` (1002); 862 for the Vampiress's, by day and by night; 864 for the Dark Elf's and her kick.
  The Dwarf's `Attacke` and `Hardhit` with a gun make none, but the gun's 1172 and 1151 (`0x514DC4` to `0x515011`); the other arts make none.
- The object is made at the striker's place, which it keeps: its light, 200, lies there while the sparks fly at the struck creature.
- Its burst, the same in the five classes' starts' default (`0x78F180`, `0x790340`, `0x791410`, `0x7938D0`, `0x794CE0`): every particle at once at its first update, a buoyancy of 180 ± 40 (they fall), a size of 6 ± 1 shrinking by 4 a second, the velocity (0, 0, 60) ± 40 and the place ± 10 on each axis, living 1.275 s (the life's rate −200); drawn as billboards with the flags `0x19`, `ONE`, `ONE` (`0x761BA2`: the bit 1 makes the destination `ONE`, the bit `0x10` the source, `0x643430`), not turned, not tinted, in a ramp from its colour at the birth to black, so that the ramp's alpha of 0 adds nothing.
  The counts and the colours: 842 600 green (127, 255, 127), 843 151 blue (127, 127, 255), 860 152 green, 862 250 red (255, 127, 127) of `PARTICLE_GLOW07` rather than `SPARK06`, 864 153 yellow (255, 255, 127).
- The other values of `+0x3C` are no hits: 842's 1015 and 1016, the Gladiator's rings under him, and 1077, a horse's floops; 843's 1003, a burst of its own with the screen's quake (kind 7, 30), which only the Seraphim's executor reaches; 860's 1007, a ring at the club arrow's hit; 862's 1030, 1031 and 1033, the Vampiress's bites and her skull's curse; 864's 1042, 1046 to 1048 and 16, the Dark Elf's symbols and her hands' sparks.
- **Seen** 2026-11-04 with the driver of #448 at the Seraphim's start: her `Attacke` (1071) on a goblin bursts a white glow of sparks at the strike, which scatter violet-blue and fall within about a second.

`opensacred-game` follows the hits' bursts since #1092 (`hit_sparks`, `opensacred_rules::particles::{sparks_of, hit_sparks}`): each strike of a hero's art that the table names sends its sparks from `making`, drawn as the bursts of `puffs`, untinted, with the light at the striker; **seen** beside the original at the Seraphim's start, a white burst at the goblin as she strikes and violet-blue sparks falling about it.
Not followed: the other values' looks, the thrown weapons' hits' 864 (`0x44E570`) and the Wood Elf's arrows' 860 (`0x5BC660`, `0x5BC965`), and the creatures' specials of the melee state's sub-states 7, 11 and 13 (842, `0x4E166B`).

### Not followed here

The birds over the camera (`0x637060`, docs/re/weather.md) are models, not effects.
The lamps' glows are the miniatures' (docs/re/daynight.md), not FX objects.

## The families and an order for `opensacred-game`

The families, by what they need beyond the machinery:

1. **The machinery** (one module): the FX object made from a category-12 record and an FX event (its kind and variant), a system per type by `0x5A3570`'s table, particles, layers, blocks, ramps and the update and emission of `0x7640C0` and `0x764760`; the billboards of `0x761B00` with the three blend modes (`SRCALPHA`/`INVSRCALPHA`, `SRCALPHA`/`ONE`, `ONE`/`ONE`), the atlases' frames and the per-layer switch; the glows of `0x762E10`; the light by `+0x9C`; the tint by the light at the object; the entries in the draw lists (list 4, 0 or 3 by the type's flags) under the night pass.
2. **The world's fires and smokes** (the smoke class, 13 types, some 3,650 objects in the Seraphim's tree): already placed by the scripts' `CreateObj` with their heights; the variants are data (the table above), one texture each, no actions, and their lights their systems' radii (`fx`, #1189).
   The simplest first: the variants 10 to 12 (`FX_FIRE_L`), then 4 to 6 (with the hit of 5 and 6 already a rule), 1 to 3, 8 and 9, 7 (its timer and sound), 13.
3. **The torches' flame** (787): a flip-book and a glow following the bone `weapon_fx01` of the item in hand, riding with its holder.
4. **The butterflies** (873, #1073): the wander, the 8 × 8 flip-book with the alpha of the life, the fireflies' glows by the night's level.
5. **The hit sparks** ("The hit sparks" above): a burst at the struck creature's neck at each strike of a hero's art, of its class's count and colour, `SPARK06` but the Vampiress's `GLOW07`, with the flags `0x19`; the Seraphim's class's 1003 shakes the screen (kind 7, 30) and changes the burst.
6. **The spells' and the arts' objects**, a class each (docs/re/arts.md has their rules): the bolts first ("The bolts" below: the railgun's, the BFG's, the fireball's, 778's `FIRE01` flip-book, and the Daemon's), then the rings, the decals, the helices, the beams and the models' effects.

The families 1 and 2 are done (`fx`, #1189), and so are 3 (`torches`, #1092), 4 (`butterflies`, #1073) and 5 (`hit_sparks`, #1092), and of 6 the bolts (`bolt_looks`, #1092).

## The catalogue

Generated 2026-10-27 by a scan of the executable (scratch scripts, not in the repository): each class by the constructor `0x5A0C50` calls, its particle count, the FX types of `0x5A3570`'s table, the textures its constructor loads, the billboards its draw makes (the texture last bound and the flags word and grid pushed to `0x761B00`; `ebp` or `g?` where they come from a register), its other primitives, the light radii its start stores at `+0x9C` as constants, and the words it stores at `+0x7C`.
An **estimate**: a linear scan, which misses values computed at run time and may take a store of another field for `+0x7C` (`0x3E800000` for `glad_sm_hit`); the classes above were read by hand.

| System | Particles | FX types | Textures | Billboards (texture `flags/grid`) | Other draws | Light | System flags |
|---|---|---|---|---|---|---|---|
| anducar_demon | 1 | 1163, 1169 | Andukar_demon_512, Andukar_demon_glow |  | model |  |  |
| anducar_garde | 300 | 1164 | PARTICLE_FIRE03 | PARTICLE_FIRE03 `0xd/g1` |  | 250 | 0x200 |
| aracattack | 10 | 855 | PARTICLE_SMOKE01 | PARTICLE_SMOKE01 `0xd/g1` |  |  | 0x40 |
| arrowtrail | 200 | 807 | PARTICLE_FIRE03, PARTICLE_GLOW03 | PARTICLE_FIRE03 `0xc/g1`; PARTICLE_GLOW03 `0x31/g1` |  |  |  |
| ball | 100 | 867 | PARTICLE_GLOW03 | PARTICLE_GLOW03 `3/g1` | model |  | 0x108,0x80108 |
| batsignal | 100 | 902 | PARTICLE_GLOW03 | PARTICLE_GLOW03 `0x19/g1` |  |  | 0x204 |
| battlefog | 100 | 868 | PARTICLE_SMOKE03 | PARTICLE_SMOKE03 `0x14c/g2` |  |  |  |
| beherrschung | 200 | 857 | FX_TRIBALS_VAMP, FX_TRIBALS_VAMP_G, PARTICLE_GLOW03 |  | chain, glow, own geometry |  | 0x204 |
| bfg | 1 | 875 | FX_BFG_1, FX_BFG_2, FX_BFG_3, PARTICLE_GLOW03 |  | flat quad, glow | 120 | 0x50 |
| big_demon_illusion | 1 | 917 | BIG_DEMON_TEX_FX |  |  | 200 |  |
| bigdemondeath | 1 | 919 | FX_GHOST2 |  | glow, own geometry |  | 0x40 |
| bigdemonsummon | 400 | 918 | PARTICLE_GLOW03 | PARTICLE_GLOW03 `0x19/g1` |  | 600 |  |
| bigsummon | 20 | 777 | FX_TRIBALS3_ELVE, FX_TRIBALS3_ELVE_G, FX_TRIBALS3_VAMP, FX_TRIBALS3_VAMP_G |  | flat quad, glow, own geometry |  |  |
| bleed | 50 | 772 | PARTICLE_DOTS03 | PARTICLE_DOTS03 `6/g1` |  |  | 0x200 |
| bloodfontain | 400 | 908 | particle_dots03 | particle_dots03 `0x4c/g1` |  |  | 0x200 |
| bloodpuddle | 100 | 903 | FX_BLOB_ALL | FX_BLOB_ALL `ebp/g?` |  |  |  |
| bloodpuddle_big | 1 | 907 |  |  | flat quad |  |  |
| bodyparts | 100 | 800 | PARTICLE_CLOUD01 | PARTICLE_CLOUD01 `0x46/g1` | model |  | 0x800 |
| bossdeath | 1 | 789 | FX_HELIX01 |  | glow, own geometry |  | 0x40 |
| bubbleprison | 1 | 853 | FX_RINGXGLOW, PARTICLE_BLOB01 |  | flat quad, own geometry |  |  |
| bubbletrap | 75 | 847 | ElveStrauch, ElveStrauch2, particle_glow03 | particle_glow03 `0x19/g1` | model |  | 0x50 |
| bubbletrap_explo | 100 | 906 | FX_SONAREFFECT |  | ring | 200 | 0x400 |
| burningBone | 300 | 788 | PARTICLE_FIRE01 | PARTICLE_FIRE01 `0x15/g4` |  | 150 | 0x1200 |
| burninglight | 200 | 816 | PARTICLE_SPARK06 | PARTICLE_SPARK06 `0xd/g1` |  |  | 0x200 |
| butterfly | 8 | 873 | PARTICLE_BUTTERFLY01, PARTICLE_GLOW03 | PARTICLE_BUTTERFLY01 `0x20/g8` | glow |  | 0x8000 |
| callofdeath | 150 | 1010 | Fx_transdim_sign |  | flat quad |  |  |
| castSpell | 300 | 791 | Fx_twirl_rays, PARTICLE_GLOW01, PARTICLE_GLOW03, PARTICLE_LINE01, PARTICLE_SPARK06, PARTICLE_SPARK08 | PARTICLE_SPARK06 `0x19/g1`; PARTICLE_GLOW01 `8/g1`; PARTICLE_GLOW03 `0x31/g1`; PARTICLE_GLOW03 `0x13/g1`; PARTICLE_SPARK08 `0x1d/g1` | flat quad, glow |  | 0x200,0x40 |
| changeling | 500 | 845 | PARTICLE_SPARK06 | PARTICLE_SPARK06 `0x11/g1` |  |  | 0x40 |
| changeling_cast | 100 | 888 | PARTICLE_GLOW03 | PARTICLE_GLOW03 `0x19/g1` | flat quad |  |  |
| circleofice | 50 | 840 | FX_LIGHTSTRIKE01, FX_LIGHTSTRIKE02, FX_LIGHTSTRIKE03, FX_LIGHTSTRIKE04, ICEBERG, ICEBERG2, PARTICLE_GLOW03, PARTICLE_SPARK02 | PARTICLE_SPARK02 `0x19/g1` | flat quad, model, own geometry | 150 | 0x400 |
| cloud | 100 | 809 | PARTICLE_CLOUD01 | PARTICLE_CLOUD01 `0x46/g1` |  |  |  |
| confusion | 1 | 871 | FX_HELIX01, FX_MAGICCIRCLE01, FX_MAGICCIRCLE02 |  | flat quad, helix |  | 0x400 |
| crusaderstrength | 200 | 818 | FX_LIGHTSTRIKE01, FX_LIGHTSTRIKE02, FX_LIGHTSTRIKE03, FX_LIGHTSTRIKE04 |  | glow, own geometry | 200 | 0x1200 |
| daemonenfuerst | 1 | 1166 | Andukar_demon_512, andukar_demon_glow2 |  | model |  |  |
| daemonfire | 300 | 910 | PARTICLE_FIRE03 | PARTICLE_FIRE03 `0xd/g1` |  | 250 | 0x204,0x3f0c0000 |
| dark_sm_cast | 200 | 865 | FX_HELIX01, FX_MAGICCIRCLE05, PARTICLE_SPARK06, PARTICLE_SPARK07 | PARTICLE_SPARK07 `0x19/g1` | flat quad, helix |  | 0x200,0x40 |
| dark_sm_hit | 153 | 864 | FXGROUND_FEAR, FXGROUND_FEUER, FXGROUND_GIFT, FXGROUND_MAGISCH, FXGROUND_PHYSISCH, FX_MAGICCIRCLE04, PARTICLE_LIGHTNING01, PARTICLE_SPARK06 | PARTICLE_SPARK06 `0x19/g1`; PARTICLE_LIGHTNING01 `0x8d/g2`; PARTICLE_LIGHTNING01 `0x8d/g2` | flat quad, glow | 200 | 0x200,0x204,0x40 |
| dayofthetentacle | 150 | 1013 | fx_tentacle_rune%d, particle_glow01 |  | custom, own geometry |  | 0x200,0x400 |
| demonfire | 500 | 1015 | A_LAVA%.2d, PARTICLE_LINE01 | PARTICLE_LINE01 `0x8d/g1` | model | 250 |  |
| demonform | 1 | 1014 | fx_demonform_demon |  | glow, own geometry |  | 0x200 |
| doorspikes | 1 | 1174 | door_spike |  | model |  |  |
| dragon_groundfire | 100 | 923 | PARTICLE_FIRE02, PARTICLE_FIRE05, PARTICLE_FIRE08 | PARTICLE_FIRE02 `0x99/g4` |  |  | 0x400 |
| dragonfire | 500 | 852 | PARTICLE_FIRE01, PARTICLE_FIRE06, PARTICLE_FIRE07 | PARTICLE_FIRE01 `0x15/g4` |  |  | 0x200 |
| dryad_ghost | 200 | 1162 | PARTICLE_DOTS01 | PARTICLE_DOTS01 `0xd/g1` | chain, glow |  | 0x10000 |
| dustcloud | 100 | 1171 | particle_smoke04 | particle_smoke04 `0x4c/g1` |  |  |  |
| dwarf_flamethrower | 999 | 1016 | particle_flamethrower, particle_smoke04 | particle_smoke04 `0xc/g1`; particle_flamethrower `0x10d/g2` |  |  | 0x1e,0x200,0x27 |
| dwarf_gier | 100 | 1155 | particle_spark07 | particle_spark07 `0x19/g1`; particle_spark07 `0x19/g1` |  |  | 0x202 |
| dwarf_grenade | 500 | 1018 | particle_shoot | particle_shoot `0x10d/g2` |  | 1046216704 | 0x2e |
| dwarf_mine | 100 | 1153 | dwarf_mine, dwarf_mine2, particle_smoke04 | particle_smoke04 `0x4c/g1` | model |  | 0x400 |
| dwarf_mine_sign | 500 | 1154 | PARTICLE_FLOOP01 | PARTICLE_FLOOP01 `ebp/g?` |  |  | 0x2202 |
| dwarf_shield | 1 | 1152 | Fx_dwarf_shield, Fx_dwarf_shield2, particle_glow03 |  | flat quad |  | 0x200 |
| dwarf_shoot | 500 | 1017 | particle_shoot | particle_shoot `0x10d/g2` |  | 1048969216 | 0x3f7f0000 |
| dwarfmagic | 100 | 832, 833 | PARTICLE_SPARK04 | PARTICLE_SPARK04 `0x1d/g1` |  | 350, 400 |  |
| eiterblume | 500 | 1170 | particle_blob01 | particle_blob01 `0x164/g2` |  |  | 0x40108 |
| elementstone | 1 | 768, 827, 828, 829, 830, 841, 939 | FX_LIGHTSTRIKE05 |  | glow, own geometry |  |  |
| elve_sm_cast | 200 | 861 | PARTICLE_SPARK06, PARTICLE_SPARK07 | PARTICLE_SPARK06 `0x19/g1` | glow |  | 0x200 |
| elve_sm_hit | 152 | 860 | PARTICLE_SPARK06, RUNESHIELD_GRADIENT | PARTICLE_SPARK06 `0x19/g1` | ring | 200 | 0x40 |
| emperor | 400 | 1159 | PARTICLE_LIGHTNING01, particle_glow03 | PARTICLE_LIGHTNING01 `0x18d/g2` | glow |  | 0x53 |
| explosion | 100 | 794 | FX_STREAKS02, Fx_ExplosionCircle01, PARTICLE_FIRE01, PARTICLE_FIRE04 | PARTICLE_FIRE04 `7/g4` | flat quad, ring |  | 0x400 |
| fernumleitung | 1 | 883 | MAGICSHIELD2 |  | chain | 300 | 0x200 |
| fireCircle | 100 | 790 | PARTICLE_FIRE02, PARTICLE_FIRE09, PARTICLE_GLOW03 | PARTICLE_FIRE09 `0x89/g4` |  | 300 | 0x200000 |
| fireball | 300 | 776 | PARTICLE_GLOW01 | PARTICLE_GLOW01 `9/g1` | glow | 250 | 0x1200 |
| fireskin | 100 | 821 | PARTICLE_FIRE02, PARTICLE_FIRE09, PARTICLE_GLOW03 | PARTICLE_FIRE09 `0x89/g4` | flat quad | 200 | 0x600 |
| firestrike | 50 | 778 | PARTICLE_FIRE01, PARTICLE_FIRE04 | PARTICLE_FIRE01 `0x15/g4` |  | 200 | 0x51 |
| flash | 1 | 838 | FX_FLASH04, FX_FLASH_TOP |  | own geometry |  | 0x400 |
| flyingskull | 200 | 905 | PARTICLE_GLOW03 | PARTICLE_GLOW03 `0x19/g1` |  |  | 0x10000 |
| forceofhell | 500 | 1173 | PARTICLE_GLOW01 | PARTICLE_GLOW01 `3/g1` |  |  | 0x400004 |
| forgebeam | 50 | 916 | FX_HORIZON, LASERBEAMPART2, PARTICLE_GLOW05, PARTICLE_RING01 | PARTICLE_RING01 `0x13/g1` | chain, glow, own geometry |  |  |
| forgeelem | 1 | 911, 912, 913, 914, 915 | FX_FORGE01, FX_FORGE02, FX_FORGE03, HQ_AIRELEMENT, HQ_EARTHELEMENT, HQ_FIREELEMENT, HQ_LOSTELEMENT, HQ_WATERELEMENT |  | flat quad, own geometry |  |  |
| freeze | 1 | 799 | FX_LIGHTNING01 |  | glow, own geometry |  |  |
| freeze_impact | 15 | 872 | PARTICLE_SPARK06 | PARTICLE_SPARK06 `0x19/g1` | glow |  |  |
| generic | 500 | 856 | ARROW2_GOLD, FX_HELIX01, FX_STREAKS02, PARTICLE_CLOUD01, PARTICLE_GLOW03, PARTICLE_SMOKE04, PARTICLE_SPARK02, PARTICLE_SPARK06 | PARTICLE_SPARK02 `5/g1`; PARTICLE_SPARK06 `0x11/g1`; PARTICLE_GLOW03 `0x19/g1`; PARTICLE_CLOUD01 `0xd/g1`; PARTICLE_CLOUD01 `0xd/g1`; PARTICLE_CLOUD01 `0xc/g1`; PARTICLE_SMOKE04 `0x4c/g1` | flat quad, helix, model | 100, 150 | 0x40 |
| glad_sm_cast | 100 | 803 | FX_AURA, PARTICLE_SPARK06 | PARTICLE_SPARK06 `0x19/g1`; PARTICLE_SPARK06 `0x19/g1`; PARTICLE_SPARK06 `0x19/g1` | flat quad, glow |  | 0x200 |
| glad_sm_hit | 600 | 842 | FX_GRADIENT3, FX_RINGX1, FX_RINGX2, FX_RINGX3, FX_RINGX4, FX_RINGX5, FX_RINGXGLOW, PARTICLE_FLOOP01, PARTICLE_GLOW03, PARTICLE_SPARK06 | PARTICLE_SPARK06 `0x19/g1`; PARTICLE_FLOOP01 `ebp/g?` | flat quad, ring | 200 | 0x200,0x202,0x40 |
| goblinimpact | 200 | 786 | PARTICLE_GLOW03, PARTICLE_LIGHTNING01, PARTICLE_LINE01 | PARTICLE_LINE01 `ebp/g?`; PARTICLE_LIGHTNING01 `ebp/g?` | flat quad |  |  |
| grenadeimpact | 300 | 1020 | fx_dwarf_explosion1, fx_dwarf_explosion2, fx_dwarf_explosion3, fx_dwarf_explosion4, fx_krater04 | fx_dwarf_explosion3 `0x10d/g2` | flat quad, own geometry | 1048969216 | 0x3f7f0000,0x400 |
| groundPoison | 200 | 792 | FX_GROUNDPOISON, FX_MAGICCIRCLE03, PARTICLE_SMOKE03 | PARTICLE_SMOKE03 `0x109/g2` | flat quad | 280 | 0x400 |
| groundsymbol | 50 | 805 | particle_glow03, runeshield_gradient | particle_glow03 `0x13/g1` | own geometry | 50 |  |
| gunfire | 1 | 1172 | particle_gunfire01, particle_gunfire02, particle_gunfire03, particle_gunfire04 |  | own geometry | 300 |  |
| hardhitbow | 30 | 887 | PARTICLE_GLOW03 | PARTICLE_GLOW03 `0x19/g1` |  |  |  |
| healing | 400 | 784 | PARTICLE_SPARK02 | PARTICLE_SPARK02 `5/g1` |  |  | 0x200 |
| hellFire | 100 | 820 | PARTICLE_FIRE02, PARTICLE_FIRE09, PARTICLE_GLOW03 | PARTICLE_FIRE09 `0x89/g4` | flat quad, own geometry | 200 | 0x420 |
| helldisk | 300 | 1005 | A_LAVA%.2d, fx_multiTexGlow01, particle_glow03 | particle_glow03 `0xd/g1` | glow, model |  | 0x10000 |
| hellgate_down | 999 | 1161 | particle_multi03 | particle_multi03 `ebp/g?` |  |  |  |
| hellgate_head | 250 | 1181 | PARTICLE_SMOKE03 | PARTICLE_SMOKE03 `0x14c/g2` | chain, glow |  |  |
| hellgate_up | 999 | 1160 | particle_glow03, particle_glow08 |  | flat quad, glow |  |  |
| hellscream | 150 | 1012 | PARTICLE_GLOW01, PARTICLE_GLOW03, laserbeampart |  | flat quad, own geometry, ring | 350 | 0x400 |
| highspeed | 250 | 846 | PARTICLE_DOTS02, PARTICLE_TWIRL02 | PARTICLE_TWIRL02 `ebp/g?` |  | 1064108032 | 0x202,0x3f1b0000 |
| holyconvert | 40 | 806 | PARTICLE_GLOW01, PARTICLE_GLOW03 |  | own geometry |  | 0x200 |
| holylight | 50 | 804 | FX_GROUNDSYMBOL01, FX_HELIX01, PARTICLE_GLOW03 | PARTICLE_GLOW03 `0x31/g1` | flat quad, own geometry |  | 0x400 |
| holyshield | 200 | 819 | FX_SHIELD, FX_SHIELDGROUND |  | flat quad, own geometry | 300 | 0x600 |
| holywall | 50 | 814 | PARTICLE_GLOW01, PARTICLE_GLOW03, PARTICLE_SPARK06 |  | flat quad, own geometry |  | 0x600 |
| hpregen | 1 | 836 | FX_HPREGEN, PARTICLE_GLOW03 |  | flat quad |  | 0x200 |
| icebullets | 200 | 839 | FX_CIRCLE02, ICICLE, ICICLE2, PARTICLE_SPARK07 | PARTICLE_SPARK07 `0x19/g1` | model, own geometry |  | 0x50 |
| kamikaze | 150 | 1168 | Fx_twirl_rays, PARTICLE_SPARK06, fx_ringofpoison01 | PARTICLE_SPARK06 `0x19/g1` | flat quad, ring | 200 | 0x1000400 |
| killerplants | 1 | 848 | PARTICLE_GLOW01, PARTICLE_GLOW03 |  | flat quad, own geometry |  | 0x400 |
| knockout | 5 | 874 | PARTICLE_BUTTERFLY01 | PARTICLE_BUTTERFLY01 `0x40/g8` |  |  | 0x200 |
| kriegsrausch | 1 | 1019 | fx_kriegsrausch |  | own geometry | 150 | 0x3200 |
| kriegsschrei | 700 | 1023 | fx_dwarf_ghost, particle_glow03, particle_spark07 | particle_spark07 `0x19/g1` | flat quad |  | 0x200 |
| kriegsschrei_sign | 1 | 1150 | particle_glow03, particle_line02 |  | flat quad, own geometry |  | 0x200 |
| kugelblitz | 500 | 785 |  |  |  |  |  |
| laserbeam | 1 | 901 | FX_LIGHTSTRIKE05, FX_VAMPBLOODLINE, LASERBEAMPART, LASERBEAMPART3, LASERBEAMPART4 |  | own geometry | 80 |  |
| laserbeam_impact | 50 | 909 | PARTICLE_SPARK06 | PARTICLE_SPARK06 `0x19/g1` |  |  | 0x40 |
| lavaburst | 100 | 877 | C_LAVA00, PARTICLE_GLOW03 | PARTICLE_GLOW03 `9/g1` | glow, model |  |  |
| lightningStrike | 150 | 798 | FX_LIGHTSTRIKE05, FX_LIGHTSTRIKE06, PARTICLE_SPARK06 | PARTICLE_SPARK06 `0x13/g1` | glow, own geometry | 200 | 0x1200 |
| liveundead | 100 | 866 | FX_PENTA, FX_PENTA_G, FX_TRIBALS3_VAMP, FX_TRIBALS3_VAMP_G |  | flat quad |  | 0x200 |
| lostsouls | 200 | 850 | PARTICLE_DOTS01 | PARTICLE_DOTS01 `0xd/g1` | chain, glow |  | 0x10000 |
| magicfade | 500 | 797 | FADESTREAK, PARTICLE_POINT01 |  | streak |  | 0x1200 |
| magicfire | 500 | 796 | PARTICLE_GLOW01 | PARTICLE_GLOW01 `3/g1` |  |  | 0x1204 |
| magicgift | 500 | 808 | PARTICLE_GLOW01 | PARTICLE_GLOW01 `3/g1` |  |  | 0x1204 |
| magicmarker | 50 | 795 |  |  |  |  |  |
| magicprison | 100 | 854 | PARTICLE_GLOW03 | PARTICLE_GLOW03 `0x19/g1` | flat quad |  |  |
| magicshield | 100 | 834 | PARTICLE_GLOW01, RUNESHIELD_GRADIENT, RUNESHIELD_GRADIENT2 |  | own geometry | 300 | 0x200 |
| magicstreak | 50 | 837 | PARTICLE_GLOW01 |  | own geometry |  | 0x200 |
| magicwhip | 50 | 824 | PARTICLE_GLOW01 |  | own geometry |  | 0x200 |
| magicworms | 100 | 858 | PARTICLE_GLOW01 | PARTICLE_GLOW01 `3/g1` |  |  | 0x1204 |
| magieumleitung | 1 | 870 | MAGICSHIELD |  | chain | 300 | 0x200 |
| maglite | 1 | 817 | PARTICLE_GLOW03 |  | flat quad |  | 0x600 |
| manaregen | 1 | 835 | FX_MANAREGEN, PARTICLE_GLOW03 |  | flat quad |  | 0x200 |
| meteor | 700 | 823 | C_LAVA00, FX_KRATER02, PARTICLE_SMOKE04 | PARTICLE_SMOKE04 `0xc/g1` | flat quad, glow, model |  |  |
| naturehealing | 200 | 851 | PARTICLE_BUTTERFLY01, PARTICLE_GLOW03 | PARTICLE_BUTTERFLY01 `0x20/g8`; PARTICLE_GLOW03 `0x11/g1` |  |  | 0x240 |
| netgate | 500 | 815 | PARTICLE_SPARK02 | PARTICLE_SPARK02 `5/g1` |  | 300 | 0x200 |
| panzer | 100 | 1021 | fx_dwarf_panzer |  | flat quad |  | 0x200 |
| parade | 50 | 773 |  |  | own geometry |  | 0x200 |
| pentagrammKiller | 300 | 782 | PARTICLE_CLOUD01, PARTICLE_GLOW03, PARTICLE_SPARK02 | PARTICLE_CLOUD01 `0xd/g1` | flat quad, own geometry |  |  |
| plantcage | 1 | 849 | PARTICLE_GLOW01, PARTICLE_GLOW03 |  | flat quad, own geometry |  | 0x200 |
| plong | 40 | 793 |  | None `7/g1` |  |  |  |
| pong_ball | 1 | 1157 | fx_twirl_rays, particle_glow03 |  | flat quad, glow |  |  |
| pong_batter | 1 | 1156, 1158 | Fx_dwarf_shield |  | own geometry |  | 0x2000 |
| puzzleSolved | 500 | 780 | PARTICLE_FACE01, PARTICLE_SPARK06 | PARTICLE_SPARK06 `0x13/g1` |  |  |  |
| puzzleSolved_v | 500 | 781 | PARTICLE_GLOW03, PARTICLE_SMOKE03, PARTICLE_SPARK06 | PARTICLE_SMOKE03 `0x106/g2`; PARTICLE_GLOW03 `0x19/g1`; PARTICLE_SMOKE03 `0x10c/g2` |  |  | 0x202,0x204 |
| railgun | 15 | 801 | FX_PLASMA_CENTER |  | own geometry | 100 | 0x50 |
| reanimate | 500 | 924 | PARTICLE_LINE01 | PARTICLE_LINE01 `0x89/g1` |  |  |  |
| ringofpoison | 150 | 1008 | Fx_twirl_rays, PARTICLE_SPARK06, fx_ringofpoison01 | PARTICLE_SPARK06 `0x19/g1` | flat quad, ring | 200 | 0x400 |
| sera_sm_cast | 200 | 844 | PARTICLE_SPARK06, PARTICLE_SPARK07 | PARTICLE_SPARK07 `0x19/g1`; PARTICLE_SPARK07 `0x19/g1` | glow |  | 0x200 |
| sera_sm_hit | 151 | 843 | PARTICLE_SPARK06 | PARTICLE_SPARK06 `0x19/g1` |  | 200 | 0x40 |
| shaddar_magic | 200 | 922 | PARTICLE_SMOKE03 | PARTICLE_SMOKE03 `0x10d/g2` |  | 200 | 0x53 |
| shaddar_penta | 400 | 920 | PARTICLE_GLOW03 |  |  |  |  |
| shaddar_slave | 400 | 921 | PARTICLE_LINE01 | PARTICLE_LINE01 `0x8d/g1` |  |  | 0x8002 |
| shielded | 200 | 769 | FX_LIGHTSTRIKE01, FX_LIGHTSTRIKE02, FX_LIGHTSTRIKE03, FX_LIGHTSTRIKE04 |  | glow, own geometry | 200 | 0x1200 |
| shoot_impact | 200 | 1151 | particle_dots03 | particle_dots03 `0x4c/g1` |  |  | 0x40 |
| smallfire | 200 | 878 | FX_GROUNDSTEP, PARTICLE_FIRE02, PARTICLE_FIRE03 | PARTICLE_FIRE03 `0xc/g1`; PARTICLE_FIRE02 `0x81/g4` | flat quad | 100, 150 | 0x400,0x78 |
| smoke | 200 | 771, 810, 811, 812, 813, 876, 879, 881, 882, 884, 885, 886, 904 | PARTICLE_FIRE03, PARTICLE_MULTI02, PARTICLE_SMOKE03, PARTICLE_WATER01 | PARTICLE_SMOKE03 `0x146/g2`; PARTICLE_FIRE03 `0xc/g1`; PARTICLE_WATER01 `0x46/g1`; PARTICLE_SMOKE03 `0x10c/g2`; PARTICLE_MULTI02 `0x11d/g2` |  | 280, 370, 530, 230 | 0x400 |
| songsfromhell | 150 | 1011 | PARTICLE_GLOW01 | PARTICLE_GLOW01 `2/g1` | flat quad |  | 0x2200 |
| soulcatcher | 100 | 869 | PARTICLE_TWIRL01 | PARTICLE_TWIRL01 `0xc/g1` | glow |  | 0x204 |
| sparks | 80 | 1009 | PARTICLE_SPARK06 | PARTICLE_SPARK06 `0x19/g1` |  | 200 |  |
| speedup | 1 | 825 | FX_LIGHTSTRIKE01, FX_LIGHTSTRIKE02, FX_LIGHTSTRIKE03, FX_LIGHTSTRIKE04, FX_SPEEDUP |  | glow, own geometry | 200 | 0x1200 |
| spiderpuke | 500 | 880 | particle_blob01 | particle_blob01 `0x164/g2` | model |  | 0x40108 |
| stahl | 1 | 1022 | fx_dwarf_stahl |  | flat quad |  | 0x200 |
| stargate | 1 | 802 | FX_WHIRL01, FX_WHIRL02, MINIMAP034035, MINIMAP044041, MINIMAP044042, MINIMAP044043, MINIMAP044044, MINIMAP045044, MINIMAP049032, MINIMAP050035, MINIMAP050036, MINIMAP050037, MINIMAP051034, MINIMAP051035, MINIMAP051036, MINIMAP058028, MINIMAP061032, MINIMAP063031 |  | own geometry |  |  |
| stargate_uw | 1 | 1180 | FX_WHIRL03, FX_WHIRL04, MINIMAP005005, MINIMAP005026, MINIMAP009015, MINIMAP024018, MINIMAP027022, MINIMAP033009, MINIMAP050070 |  | own geometry |  |  |
| stoned | 1 | 822 | FREEZE_STONE01, FX_AURA |  | model |  | 0x200 |
| superfluid | 1 | 783 | FX_LIGHTSTRIKE01, FX_LIGHTSTRIKE02, FX_LIGHTSTRIKE03, FX_LIGHTSTRIKE04 |  | glow, own geometry | 200 | 0x1200 |
| teleport | 500 | 774, 775 | PARTICLE_SPARK02 | PARTICLE_SPARK02 `5/g1` |  | 170 | 0x200 |
| temptex | 1 | 779 |  |  | texture swap |  |  |
| terrorsprinter | 1 | 1167 | Fx_circle03 |  | custom, flat quad |  | 0x200,0x2200 |
| torchsmoke | 200 | 787 | PARTICLE_FIRE01 | PARTICLE_FIRE01 `0x15/g4` | glow |  | 0x204 |
| transdim | 1 | 1004 | FX_NOISE, fx_transdim_sign |  | flat quad |  | 0x2200 |
| trap01 | 1 | 890 | FX_RUNES01 |  | flat quad |  |  |
| trap01_parent | 1 | 889 |  |  |  |  |  |
| trap02_ghost | 1 | 893 | PARTICLE_FACE01 |  | chain, glow |  | 0x20000 |
| trap02_parent | 1 | 891 |  |  |  |  |  |
| trap02_pill | 1 | 892 | PARTICLE_BUBBLE01 |  | chain, glow |  |  |
| trap03_laser | 1 | 895 | FX_LIGHTNING01 |  | glow, own geometry |  | 0x10 |
| trap03_parent | 1 | 894 |  |  |  |  |  |
| twister | 250 | 826 | PARTICLE_CLOUD01 | PARTICLE_CLOUD01 `0x44/g1` |  |  | 0x600 |
| uw_glyph | 1 | 1175, 1176, 1177, 1178 | FX_MULTITEXBASE06, fx_demonform_demon |  | glow, own geometry |  |  |
| uw_glyph_fire | 300 | 1179 | FX_MULTITEXBASE03, FX_MULTITEXBASE06, PARTICLE_GLOW03 | PARTICLE_GLOW03 `0x19/g1` |  |  |  |
| vamp_sm_cast | 200 | 863 | FX_TRIBALS2_VAMP, FX_TRIBALS2_VAMP_G, PARTICLE_GLOW03, PARTICLE_MULTI01, PARTICLE_SPARK06, PARTICLE_SPARK07 | PARTICLE_GLOW03 `0x19/g1`; PARTICLE_MULTI01 `0x119/g2` | flat quad, glow | 35, 250 | 0x200,0x40 |
| vamp_sm_hit | 250 | 862 | FX_GHOST, PARTICLE_DOTS03, PARTICLE_GLOW07, PARTICLE_MULTI02 | PARTICLE_GLOW07 `0x19/g1`; PARTICLE_DOTS03 `0x4c/g1` | glow, own geometry | 150, 200 | 0x1200,0x204,0x40 |
| walloffire | 250 | 1006 | PARTICLE_FIRE03 | PARTICLE_FIRE03 `0xc/g1` | glow | 100 | 0x78,0xc00 |
| walloffire_line | 250 | 1007 | PARTICLE_FIRE02 | PARTICLE_FIRE02 `0x99/g4` |  |  | 0x400 |
| windstrike | 200 | 831 | PARTICLE_CLOUD01 | PARTICLE_CLOUD01 `0x64/g1` |  |  | 0x50 |
| zeitkontrolle | 250 | 859 | PARTICLE_GLOW03, PARTICLE_SPARK04 | PARTICLE_SPARK04 `0x19/g1` | flat quad |  | 0x200 |
| zhadum | 1 | 1165 | particle_glow01 |  | glow |  | 0x400 |

## The bolts

Read 2026-11-04 for #1092 in Ghidra and with capstone (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Seraphim with the effects 16 and 17, the Mage's spell 5, the Daemon's 1103, a goblin at six to eight cells).

| Bolt | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| The Seraphim's railgun | 801 | `cParticleSystem_railgun` (`0x897E20`) | `0x77E7A0`, `0x77EBC0`, `0x77EB70`, `0x77EF50` |
| Her BFG | 875 | `cParticleSystem_bfg` (`0x897E40`) | `0x77F0F0`, `0x77F290`, `0x77F1C0`, `0x77F520` |
| The Mage's fireball (spell 5) | 778 | `cParticleSystem_firestrike` (`0x897904`) | `0x76BAE0`, `0x76BDD0`, `0x76BBD0`, `0x76BE80` |
| The Daemon's 1103 | 1159 | `cParticleSystem_emperor` (`0x898E9C`) | `0x7BD9B0`, `0x7BDB50`, `0x7BDA50`, `0x7BDC50` |

The fireball is 778 (`sdk_CREATE(0x30A)` at `0x523834`): 776, the catalogue's `fireball`, is a weapon's glow that `cWeapon3D` makes (`0x5CDDAD`, below), not the spell's.
The Daemon's 1103 makes 1159 by the FX event `0x91` (`0x51B0EF`).
Other makers of these types, not followed: 801 at `0x4D3FA8` and `0x4E369E`; 778 at `0x4D4315`, `0x4E3A84`, `0x5354AA` and `0x57865F`; 1159 at `0x5357B6`, `0x57529B` and `0x6AAEB6`.

### Their flight

- The system flag `0x10` is the flight (`0x5A67D2` in `0x5A6680`): at each run the system's place moves by its whole speed `+0x98` times the time along its direction `+0x78` to `+0x80`, the direction turns by `+0x8C` (for the type 778 alone its height's part shrinks by 0.9 at each run, `0x5A67F8`), and the object goes to the system's place (`0x5AA330`).
  The flight gathers its targets (`0x5AB490`, `0x5ABD80` at `0x5A694E`) and tests its cell (`0x636D70`, `0x636D20`); a cell that stops it writes `+0x74` 2 (`0x7CBA10`, `0x5A6AEA`).
- The system flies in a frame of its own whose axes lie along the screen's horizontal and vertical on the ground, and the object follows it to the ground through the view (`0x5AA330` by `0x623AD0`, its globals `0xAD7788` to `0xAD77A4` not read): the frame's first axis goes to the world's (1, −1) direction scaled by 1.516 (`MODEL_SCALE`), its second to (1, 1) scaled by `2/√5` of that, 1.356, the camera's tilt.
  So a flight of the speed `v` along the world's unit heading `w` goes `v / √((a / 1.516)² + (b / 1.356)²)` world units a second, `a` and `b` the shares of `w` along the screen's horizontal and vertical: 1.43 `v` along a world axis.
  **Seen** 2026-11-04 (#2018, a probe at `0x5A67E9`): the railgun of 400 went 570 world units a second along the world's x, its direction (0.667, 0.745) in its frame, and as fast along the world's y, its direction (−0.667, 0.745).
- The flag `0x01` ends a flyer under the ground (`0x5AA3CE`, the ground `0x636310` above it: `+0x74` 1, an end at once); 778 (flags `0x51`) and 1159 (`0x53`) have it, 801 and 875 (`0x50`) not.
- The flag `0x40` puts the system once at its followed object's (`+0x68`) bone (`+0x70`), with the bone's turn, in the object's draw (`0x5A9CAF` to `0x5A9D5C`), and clears itself; so a bolt leaves from its bone and keeps its height (**seen**: the railgun flies at her hand's height over the goblin's head).
- `+0x74`, a word written whole (`0x7CBA10`): 1 ends the system at once, 2 tells its update that it hit or stopped.
- The FX handler's cases (the table `0x5A5EC4`, `0x5A3B02`): the railgun's kind `0x21` follows the event's caster at its bone 2 (`Bip01 R Hand`, the event's `+0x38`); the BFG's `0x63` the event's target, the staff 3809 (`0x556F50(13)`), at its bone 15 (`weapon_fx01`); the fireball's 1 its caster at the bone 1 (`Bip01 L Hand`, the event's `+0x3C`); the emperor's `0x91` its caster at the bone 2; each takes the event's direction, none the event's point.
- `+0xA9`, written by the handler (`0x5A3A4A` to `0x5A3A7B`): 0 for a caster of an id up to 16 (a hero), else 1 where the caster's object is gone and 2 where it is there, 0 again where the event's `+0x28` is 1.0.
- The base's `+0xB4` is a Direct3D 7 light (`0x642880`, `0x642B90`), moved at each draw (`0x642EF0`), taken off by the base's destructor (`0x761A67`).

### The railgun (801)

- No particles drawn: its 15, its block and its layer are filled and never emitted (no call of `0x764760`, `0x7640C0` or `0x761B00` in its class).
- Its body (`0x77E7A0`): twelve white vertices of two crossed quads, one flat and one upright, 40 along it and 30 across (`±20`, `±15`), of `FX_PLASMA_CENTER` (128 × 64, a cyan comet whose head lies a quarter along the texture), its `u` 0 at the front and 1 at the back; turned once at its start so that its length lies along the flight (`0x642690` about the axis `x × d`, `0x77EF50`).
  Drawn laid over by its alpha (`SRCALPHA`, `INVSRCALPHA`), the depth tested and not written, both faces, untinted (`0x77EBC0`).
- Before it, for a hero's bolt (`+0xA9` 0), a line of eight glows of `PARTICLE_GLOW01` facing the camera (`0x40DA80`), each of a half side of 15 and the colour (32, 32, 64), added, the first 20 behind its centre and each 5 on.
- It flies at 400 for 2 s at most and ends at its hit or a cell that stops it, with no burst; its light 100; the shot plays `SERA_PLASMASHOT` (2072) with the first bolt (`0x4DDE02`).
- **Seen**: a cyan streak, its head forward, in a pale lavender haze about three times its width, leaving her right hand; a shot's two bolts close together; nothing stays at the hit.

### The BFG (875)

- Three glows (`0x762E10`) at its place of `FX_BFG_1`, `2` and `3` (256 × 256 opaque starbursts), each of a half side of 35, added (`ONE`, `ONE`), the depth tested, in greys that pulse by its clock `t` (`0x77F1C0`, `a = 4.21 t`, `0x897E5C`): `⌊127 (sin a + 1)⌋`, 255 less that, and `⌊127 (cos a + 1)⌋`, a period of about 1.49 s.
- A flat quad (`0x7631E0`) of `PARTICLE_GLOW03` under it, of a half side of 60 and the colour (72, 185, 255), added, at its place less `+0xA4` (**hypothesis**: its height over the ground).
- It flies at 200 for 3 s at most, through what it hits (its hit, `0x5AC485`, writes nothing to `+0x74`), and ends at 3 s or at a cell that stops it; its light 120; its start plays `SERA_PLASMASHOT_BIG` (2073) at its owner.
- **Seen**: a white starburst leaving the staff's tip, breathing in its size and whiteness, a cyan glow on the ground below it; it passed through the goblin and flew on; by night as by day.

### The fireball (778)

- `PARTICLE_FIRE01`'s flames, 50 at most, left in the world along its way (no flag 2): one every 0.07 s (times 1.5 and 2.2 at the detail levels 1 and 0), each of a half side of 15 growing by 16 a second, turned at random, drifting by up to 4 a second along each axis, living 0.73 s (its layer's life −350 a second), drawn added and turned (the flags `0x15`), white, its frame of the 4 × 4 its age's (`⌊(255 − life) · 16 / 256⌋`: the flip-book once over its life).
- Its life is `2 + 0.5 L` seconds and its speed `150 + 10 L` (for a hero, `+0xA9` 0; else `130 + 8 L`), at most 500, `L` the event's word `+0x40`; its start plays `MAGE_FEUERBALL_CAST` (2000).
- At its hit (`0x5AC015`) or a cell that stops it: `MAGE_FEUERBALL_HIT` (2001), its flight stopped, one flame more of its burst's block (a half side of 10, a drift of up to 20, within 15 of its place), and no more; at its time's end no more and no burst; the system ends with its last flame.
- Its light is 200, and a Direct3D point light (`0x642B90`) of (1, 0.8, 0.4), a range of 200 and the attenuation (1, 0, 0.000125) follows it.
- `PARTICLE_FIRE04` (a cyan green) takes the place of `FIRE01` where `+0xA9` is 1, its caster gone, and its variant not 1.
- **Seen**: a white and yellow head with a broad orange plume back to the Mage's hand; at the goblin it stops and blooms into an orange ball with dark red edges, gone within about a second.

### The emperor (1159)

- Rays of `PARTICLE_LIGHTNING01`'s 2 × 2 frames (forked bolts rooted at their foot), 400 at most, riding with it (the flag 2): one every 0.005 s for its first 1.14 s, each of a half side of 10 ± 7, turned at random about its foot, a frame at random, living 0.21 s (some 42 at once), drawn standing on their foot at its centre (the flags `0x18D`: `0x80` a quad its half side across and twice that up from the particle, `0x04` turned about that foot), added by their alpha, in a ramp whose entry `i` is (`⌊220 i / 256⌋`, `⌊220 i / 256⌋`, `⌊255 i / 256⌋`).
- While it makes them, a glow of the style 5 (`PARTICLE_GLOW03`, `ONE`, `ONE`) of (224, 224, 255) at its centre, of a half side of `(10 + 4 · rand)` times its scale, flickering.
- It flies at 500, slowing by `⌊300 · dt⌋` at each run; at its hit (`0x5B097E`: `+0x78` 1, `+0x74` 2) it stops and follows its first target's neck (bone 3, `Bip01 Neck`), and flares 2 s more, its rays' size and spread and its glow 1.75 times as large; a cell that stops it flares where it stops; no light, no sound.
- **Seen**: the bolts leave about 2 s after the click, each a small white ball with violet and white rays; two hit the goblin and stayed on its chest as a large crackling ball for about 2 s, the third flew on.

### 776, a weapon's glow

`cWeapon3D` (`0x5CDD5D` to `0x5CDE3E`) makes it for a weapon of a bonus's style 1 to 5 (`0x5CEA50`) with the bone 18 `weapon_gl01`, by the FX event `0xB` with the weapon its caster, `+0x38` 1 and `+0x3C` the style (**confirmed** by code, not seen).
Its start (`0x76B5F0`) fills a ramp of white, a style's colour over 8 entries, a darker one at the alpha 0.8 over 192 and to transparent over 56 (1 red, 2 green, 3 grey, 4 blue, else violet); its layer's life −1000 and size −2.7, a block of the size 3 every 0.005 s; its flags `0x1200`, its light 250.
Its update (`0x76B4C0`) emits at its point 0 in the state 0; its draw (`0x76B540`) draws `PARTICLE_GLOW01` billboards (the flags 9) and, while it emits, a `PARTICLE_FLARE02` glow at the point 0 of a half side of `17 + 3 p`, `p = (sin 2t + 1) / 2`.

### In `opensacred-game`

The four bolts look as their systems since #1092's first family (`bolt_looks`, `opensacred_rules::particles::{Firestrike, Emperor, bfg_greys, age_frame}`): each look an entity of its own that follows its bolt and outlives it as its system does (the fireball's flames burning out, the emperor's flare on its target's neck), told of its bolt's end (`bolt_looks::Ended`), untinted by the night, their sizes at the models' scale.
Their flight follows the frame's map since #2018 (`bolts::flight_scale`), every bolt's: the railgun's, the BFG's, the fireball's, the Daemon's, the whirlwinds', the splinters', the bubble's and the fire bolt's.
Their lights follow since #61 (`bolt_looks`): the railgun's 100, the BFG's 120 and the fireball's 200, at their places on the ground at height 0 while their looks show, the fireball's while its flames burn out.
Their height follows since #2018 (`bolts::leaving`): each leaves from its shooter's bone that its FX handler's case names (the railgun, the Daemon's bolt and the bubble the right hand, the fireball, the fire bolt and the splinters the left, the BFG the staff's `weapon_fx01`, the whirlwind the root `__Root`), at the moment it leaves, and flies level at that height, the fireball and the Daemon's bolt ending at once where the ground rises above them (the flag `0x01`), the BFG's ground glow on the ground under it (its place less `+0xA4`).
**Seen** 2026-11-04 in the game: the Mage's fireball leaving her raised left hand at 90 over the ground and flying at her chest's height to the goblin it struck.
A shooter of no such bone keeps its bolts 80 over the ground under them (`bolts::HEIGHT`, a stand-in).
Not followed: the looks of a caster gone (`+0xA9` 1); the fireball's point light on the models follows since #2106 ("The systems' Direct3D lights").
**Seen** 2026-11-04 beside the original's frames: the railgun's cyan streak and its haze alike in size and colour; the fireball's plume alike; the emperor's flare alike in size, its rays whiter than the original's violet white; the BFG's starburst and its ground glow of their quads' size, but only about half as wide to the eye, their dim middle tones lost, until #2017 ("In `opensacred-game`" below): since, alike.

## The breath of fire

Read 2026-11-04 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Daemon's new game at 352,1722, his form 1098 and its breath 1102 toward a point 174 off).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| The Daemon's breath 1102 | 1006 | `cParticleSystem_walloffire` (`0x898AB4`) | `0x7AE200`, `0x7AE4F0`, `0x7AE290`, `0x7AE5A0` |
| A fire of its wall | 1007 | `cParticleSystem_walloffire_line` (`0x898AD8`) | `0x7AEAF0`, `0x7AEBF0`, `0x7AEB80`, `0x7AEC60` |

### The breath (1006)

- Its start keeps its place as its start (`+0x20B8`), its event's `+0x4C` as its reach, held to 650 (`+0x20B4`), its `+0x48` (`+0x20A8`, unused), its speed `+0x98` 150, its flags `0xC00` and its light 100.
  **Seen** 2026-11-04 (a probe at `0x7AE5B9`): `+0x48` 10 and `+0x4C` 173.6, the distance to the point.
- Its particles are a fire of the smoke class's make ("The fires and smokes" above), two layers of `PARTICLE_FIRE03` emitted at its place (no flag 2, so they stay in the world as it moves): the flame (the layer's life −100 a second and size +12; the block of buoyancy −35 ± 34, size 5 ± 0.5, angle π ± π, velocity ± 0.5, place ± 9 along x, 0.01 s) in the fires' ramp A, and one in five the smoke (life −130, size +15; buoyancy −35 ± 5, size 3 ± 1, angle π ± π, spin ± 2, the rest alike) in the fire smokes' ramp B at its whole alpha; the intervals times 1.5 and 2.2 at the detail levels 1 and 0; 250 particles at most.
- Its run (`0x7AE290`), in its state `+0x20C4`:
  - 0, flying: the layers' wind from the renderer's vector, as the smoke class takes it; its particles aged, and new ones emitted along its way; farther than its reach from its start it clears its flags (no more hits) and goes to 1.
  - 1, stopped: its particles aged, none emitted; `+0x20AC` grows by 200 a second to 200, and every 0.15 s (`+0x20B0`) it asks for the action 1 of the type 1007 (`+0x84`), its data the spread `⌊+0x20AC⌋` (`+0x88`) and its direction its way turned a quarter (`+0x8C` … `+0x94`), which makes the wall's pair of fires (`0x5B2600` at `0x5B3E39`: at ± the spread along that direction, each on ground that `0x636890` admits, at the ground's height `0x636310`, its FX event a copy of the breath's); at 200 it lives while a particle does.
- Its draw (`0x7AE4F0`): its particles as billboards of the flags `0x0C` (the ramp's colour, turned; neither `0x01` nor `0x10`, so with its two layers the flame added by its alpha and the smoke laid over, "The billboards"), untinted; then a glow (`0x762E10`) at its place, of a half side of 20, the style 5 (`PARTICLE_GLOW03`, `ONE`, `ONE`), white, its depth tested; both while it lives, so that the glow stays where it stopped until its last flame dies.

### A fire of its wall (1007)

- Its start: its event's `+0x48` its seconds (`+0x20A4`; **seen** 10 at each fire that a probe at `0x7AEC67` caught, the breath's), its flags `0x400` (its hits) and its light 200.
- One layer of `PARTICLE_FIRE02`'s 4 × 4 frames: the layer's life −100 a second and size −17; the block of size 40 ± 10, place ± (40, 40, 0), nothing else, every 0.08 s (times 1.5 and 2.2 at the detail levels 1 and 0); 250 particles at most.
- Its ramp from a transparent black through a grey of 199 over 10 entries, held over 200, to a transparent black over 46: its alpha 0 throughout, which its draw does not take.
- Its run (`0x7AEB80`): its particles aged, and new ones while its clock is below its seconds; then it lives while a particle does.
- Its draw (`0x7AEBF0`): billboards of the flags `0x99`, `ONE`, `ONE` (the source `0x10` and the destination `0x01`), the ramp's colour (`0x08`), standing on their feet (`0x80`), each its frame by its age, not turned, untinted.
  `PARTICLE_FIRE02`'s texels carry their brightness in their alpha as well (its alpha equals its colour), which the source `ONE` leaves out.

### In `opensacred-game`

The breath and its wall look as their systems since #1092 (`breath`, `opensacred_rules::particles::{walloffire, walloffire_line}`): the breath's look an entity of its own that follows the breath while it flies, leaving its flames and smoke along the way, its glow and its light of 100 at it (at height 0, as the bolts'), outliving the breath until its last flame dies; each fire of the wall its flames on the ground for its 10 s (`arts::WALL_BURNS`), its light of 200 with it until its last; all untinted by the night.
The wall's flames and the glow are added whole (`fx::whole_material`: the particles' material that takes no alpha, `ONE`, `ONE`).
**Seen** 2026-11-04 beside the original's frames at the Daemon's start by night: the breath an orange plume with a dark smoke where it stops, and the wall a row of tall flames that run together into a white and yellow sheet, alike in size; the original's smoke drifts and spreads more.
Not followed: the layers' wind (as the world's smokes'), the detail's factor of the intervals, and the steps' small fire of the variant 1 that the wall's end makes ("The steps' small fire" below).

## The ring of poison

Read 2026-11-04 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Daemon's new game at 352,1722, his form 1099 and its ring 1101 among three goblins).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| The Daemon's ring of poison 1101 | 1008 | `cParticleSystem_ringofpoison` (`0x898B1C`) | `0x7AF490`, `0x7AF650`, `0x7AF550`, `0x7AF840` |

- Its constructor loads `fx_ringofpoison01` (`+0x252C`), `PARTICLE_SPARK06` (`+0x2530`) and `Fx_twirl_rays` (`+0x2534`); 150 particles.
- Its start (`0x7AF840`): one layer (the life −170 a second, the size −6) and one block, a burst of all its particles (the interval 0, the count the particles'): buoyancy 180 ± 40, size 7 ± 2, angle π ± π, velocity (0, 0, 150) ± 40 along each axis, place ± (7, 7, 0); its ramp white (254) for 100 entries and to black over 156, its alpha 0 throughout; its flags `0x400` (its hits), its light 200.
- Its run (`0x7AF550`): at its clock's 0 the burst and a weather event of the kind 7 and the value 30, the screen's quake (docs/re/weather.md); then its particles aged; past 3 s it ends.
- Its draw (`0x7AF650`), by its clock `t`:
  - below 0.75 s a ring (`0x763530`) of 50 segments flat at its place on the ground (its height less `+0xA4`), between the radii `150 t` and 40 more, of `fx_ringofpoison01` (each segment the texture's width along it, its height from the inner radius out), added (`ONE`, `ONE`), of the colour (34, 128, 34) times a share that holds 1 to 0.45 s and falls to 0 at 0.75 s, each byte taken whole;
  - below 3 s two flat quads (`0x7631E0`) at its place of `Fx_twirl_rays`, of the half sides `100 (1 − (t/3)²)` and `90 (1 − (t/3)²)`, turned by `−1.7 t` and `−1.3 t` radians, of the colour (65, 255, 66), added (`ONE`, `ONE`), their depth tested;
  - its particles as billboards of the flags `0x19` (`ONE`, `ONE`, the ramp's colour, not turned) of `PARTICLE_SPARK06`.
- `Fx_twirl_rays`' texels carry their brightness in their alpha as well, which the source `ONE` leaves out.

`opensacred-game` makes it since #1092 (`poison_rings`, `opensacred_rules::particles::{poison_ring, poison_twirls, ring_sparks}`): where the Daemon's 1101 burns about him, its sparks (`puffs`), its quake of 30 (`weather`), its ring and twirls on the ground added whole and untinted by the night, and its light of 200 at it, for 3 s.
**Seen** 2026-11-04 beside the original's frames at the Daemon's start by night: the green twirls shrinking at his feet, the white fountain of sparks over him, and the green ring spreading past the twirls, alike in size; the original's ring a darker green.

## The Daemon's blast

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code) and seen in the running original under the driver of #448 (the Daemon's charge 1091 at the level 10 in his form 1094 by a goblin).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The blast of his charge 1091 | 1168 `FX_KAMIKAZE` | `cParticleSystem_kamikaze` (`0x898FCC`) | `0x7C16D0`, `0x7C1AB0`, `0x7C1790`, `0x7C1890` |

It is the ring of poison 1008 ("The ring of poison") line for line but for three things: its ring's reach is the event's `+0x44`, the blast's `R = min(150 + 2·L, 400)` (`0x51A2AA`), where the ring of poison's is 150; its colours, the band's (100, 100, 128) and the twirls' (224, 224, 255), its sparks' ramp turning pale blue (0.820, 0.820, 0.996) over its first 100 entries; and its flags `0x1000400`, whose `0x1000000` has its hits search their targets within its `+0x98` radius (`0x5FF2F0`), which its draw sets to `⌊R·t⌋` while its band spreads and to three quarters of itself at each frame after (docs/re/arts.md, "The objects 1168 and 842").
It loads `fx_ringofpoison01`, `PARTICLE_SPARK06` and `Fx_twirl_rays`, makes its 150 sparks at once with the screen's quake of 30 at its start, and ends at 3 s; its light 200.

`opensacred-game` draws it since #1092 (`poison_rings` of the kind `Blast`, `opensacred_rules::particles::{blast_ring, blast_sparks, BLAST_TWIRL_COLOUR}`), where `blast` makes it, in place of the stand-in ring.
**Seen** 2026-11-05 beside the original: in both a white fountain of sparks and pale twirls on the ground about the Daemon where his charge lands.

## The holy light

Read 2026-11-04 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Seraphim's new game in the chapel of 50,39, her spell 15 at the level 5 on a goblin).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| The Seraphim's holy light 15 | 804 | `cParticleSystem_holylight` | `0x780C70`, `0x781100`, `0x781030`, `0x7816A0` |

- Its constructor loads `PARTICLE_GLOW03` (`+0x258C`), `FX_GROUNDSYMBOL01` (`+0x2590`) and `FX_HELIX01` (`+0x2594`, bound at the draw's end and drawn with nothing), makes a Direct3D point light of (0.8, 0.8, 1.0) (`+0xB4`), its light 200, and its column: 13 pairs of vertices about the upright at the radius 7, every π/6, at the heights 0 (white) and 1,000 ((160, 160, 255) at an alpha of 112), and two bands across it in its frame's x–z plane from 7 out to 17.5, their inner edges white at the foot and (160, 160, 255) at the top at an alpha of 128, their outer edges of an alpha of 0; 50 particles.
- Its start (`0x7816A0`): its seconds `0x4D26B0(0, L) · [+0x70]` (`+0x2148`, the object's), its fade `+0x20B0` 1, its base size 7 (`+0x20A8`); one layer (the life −300 a second) and one block (buoyancy −1,500, the size `3.5 ± 1.75`, the place ± (7, 7, 0), the colour `0x708080F0` ± `0x0F` each byte, every 0.03 s, times 1.5 and 2.2 at the detail levels 1 and 0); its flags `0x400`, its loop 2031 (`SERA_HEILIGES_LICHT_LOOP`).
- Its run (`0x781030`): its pulse `+0x20AC` `0.8 + 0.1 sin 4t`; its block's size and place times its fade; its particles aged and emitted; its fade down by `dt / seconds`, ending it at 0.01.
- Its draw (`0x781100`), its texture stage emptied, added by the vertices' alpha (`SRCALPHA`, `ONE`), the depth tested and not written, its world matrix its fade along x and y at its place: the column, its foot's alpha its pulse times 112, and the bands, their inner foot's its pulse times 128; then flat quads (`0x7631E0`, `ONE`, `ONE`) of `FX_GROUNDSYMBOL01` of a half side of 50 in a grey of 128 turning at 0.5 radians a second, and of `PARTICLE_GLOW03` of a half side of 100 in (75, 233, 247); then its particles as billboards of the flags `0x31` (their own colour, `ONE`, `ONE`) of `PARTICLE_GLOW03`.
- Its frame's x lies along the screen's horizontal, as the bolts' ("Their flight" above): its bands face the camera, so that the column shows 35 across.

`opensacred-game` draws it since #1092 (`holy_lights`, `opensacred_rules::particles::{HolyLight, HOLY_COLUMN, holy_pulse}`): the object that `spells` stands at the point gets its look for its seconds, its column of its vertices' colours alone (`fx::plain_material`) with its bands across the screen's horizontal, its symbol and glow on the ground and its sparks added whole, untinted by the night, and its light of 200 at it.
**Seen** 2026-11-04 beside the original's frames in the chapel of 50,39: a soft white column rising out of the frame with sparks climbing it, over a cyan glow and the turning symbol, alike in breadth, both narrowing as they fade.
Not followed: the sparks' colours' spread; its Direct3D point light on the models follows since #2106.

## The blades of light

Read 2026-11-04 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Seraphim's new game in the chapel of 50,39, her spell 17 at the level 5).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| The Seraphim's blades of light 17 | 814 | `cParticleSystem_holywall` | `0x784380`, `0x7846C0`, `0x784630`, `0x784C90` |

- Its constructor loads `PARTICLE_GLOW01` (`+0x21B8`), `PARTICLE_GLOW03` (`+0x21BC`) and `PARTICLE_SPARK06` (`+0x21C0`), and two quads of its own: a dot (`+0x20B8`) of a half side of 4, its corners (0, 0, 128) at the bottom left, (64, 64, 128) at the top left, (0, 64, 128) at the bottom right and (64, 0, 128) at the top right, and a spark (`+0x2138`), white.
- Its start (`0x784C90`): its turn and height 0, its radius `+0x20B0` its caster's size (`0x428CE0`, 50 for the heroes), its seconds `0x4D26B0(0, L) · [+0x70]` (`+0x20B4`), its loop 2036 (`SERA_KLINGENWALL_LOOP`), the effect 35 on its caster for its seconds where it is a creature (`0x582D50`), its flags `0x600` (with its caster, and its hits).
- Its run (`0x784630`): its turn on by 5 radians a second (`+0x20A8`), its height up by 33 a second to 100 (`+0x20AC`); past its seconds it ends.
- Its draw (`0x7846C0`), its world matrix the draw's own (the camera-facing one) with its translation replaced, so that its dots and sparks face the camera:
  - a flat quad (`0x7631E0`) of `PARTICLE_GLOW01` of a half side of its radius and 20, of (175, 175, 255), laid over by its alpha, on the ground under it;
  - added (`ONE`, `ONE`), `PARTICLE_GLOW03`: 100 dots on its ring at its radius at its foot, every 1/50 of a turn (each place twice); then two spirals of 200 dots, from a quarter turn either side of its turn, round twice the one way and the other, rising from its foot to its height;
  - `PARTICLE_SPARK06`: a spark at each spiral's top (a quarter turn either side of its turn, at its height), of a half side of 15 and up to 10 more at random at each draw.
- The dots are drawn as strips of their bottom left, top left, bottom right and top right corners, so that each is split from its top left to its bottom right and its middle, where `PARTICLE_GLOW03` is brightest, takes the mean of those two corners' colours, (32, 64, 128).

`opensacred-game` draws it since #1092 (`light_blades`, `opensacred_rules::particles::Blades`, `fx::Quads::push_facing`): the object that `spells` puts on the Seraphim gets its look for its seconds, its glow on the ground, its dots and sparks facing the camera and added whole, untinted by the night, in the systems' frame ("Their flight" above, `iso::fx_axes`).
**Seen** 2026-11-04 beside the original's frames in the chapel of 50,39: the ring and the two spirals rising about her as stacked rings of blue dots, their bright pixels' mean (139, 148, 215) against the original's (163, 169, 225), alike in size; before the dots were split as the original's strips, their middles were (32, 0, 128) and the spirals violet.

## The lightning

Read 2026-11-04 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Seraphim's new game in the chapel of 50,39, her spell 13 at the level 5 on a goblin).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| The lightning 13 | 798 | `cParticleSystem_lightningStrike` | `0x77CD70`, `0x77D000`, `0x77CF70`, `0x77D5C0` |

- Its constructor loads `FX_LIGHTSTRIKE05` (`+0x21E8`), `FX_LIGHTSTRIKE06` (`+0x21EC`) and `PARTICLE_SPARK06` (`+0x21F0`), makes a Direct3D point light of (0.8, 0.8, 1.0), and a quad of its own (`+0x2168`), white, its top corners at (−10, 0, 250) and (10, 0, 250) and its foot's at (−10, 0, 0) and (10, 0, 0) in its frame.
- Its start (`0x77D5C0`): its bolt and its glow on, three bolts of slants `2 rand()/32767 − 1` (`+0x20BC`), its colour (160, 176, 255) for a hero's (`+0xA9` 0) and (231, 73, 255) else (`+0x20A8`); one layer (the life −250 a second, the size −2) and one block, a burst of all its 150 particles: buoyancy 150 ± 40, size 7 ± 1, angle π ± π, velocity (0, 0, 50) ± (40, 40, 20), place ± 10; its light 200, its action 2 of the spell 13, its flags `0x1200`.
- Its run (`0x77CF70`): the burst at its clock's 0; its bolt off past 0.25 s, its glow past 0.1 s; it lives while a particle does.
- Its draw (`0x77D000`), while its bolt is on, its depth neither tested nor written, added (`ONE`, `ONE`), of `FX_LIGHTSTRIKE05` (a creature's `06`): for each bolt its quad at its place, its top shifted by 40 times its slant along x, and again 250 up and 40 times its slant on, while the point it climbs from lies on the screen (`0x60D6C0`, `0x6251E0`), so that the bolt rises out of the screen; each segment's `u` from its slant at the top to one more at its foot, and its `v` across the texture's first eighth, turned over and its `u` shifted by a half where `rand() & 1` is 1, at each draw.
  While its glow is on, a glow (`0x762E10`) of a half side of 25, the style 2 (`PARTICLE_GLOW05`, `ONE`, `ONE`), in its colour, at its first point less 2 up, its depth not tested.
  Then its particles as billboards of the flags `0x13` (its corners' colour, `ONE`, `ONE`) of `PARTICLE_SPARK06`.
- `FX_LIGHTSTRIKE05` is a strip of a bolt along the top eighth of its 512 × 512, seamless along its width, its `u` repeated.

`opensacred-game` draws it since #1092 (`lightning_strikes`, `opensacred_rules::particles::{LightningBolt, lightning_sparks}`, `fx::repeated_texture`): at what the Seraphim's lightning strikes, and at what a creature's lightning strikes in violet (#1852), its three bolts of 12 segments, its glow and its sparks (`puffs`), added whole and untinted by the night, its light of 200 while its sparks fall.
**Seen** 2026-11-04 beside the original's frames in the chapel of 50,39: three slanting blue zigzags rising out of the frame from the goblin, a white flash at their foot and a shower of blue-white sparks after, alike.
Not followed: the bolts over all else (their depth test off), and the glow at the system's first point, which the game takes for its place (the original's flash lies a little higher, at the goblin's body).

## The light

Read 2026-11-04 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Seraphim's new game in the chapel of 50,39, her spell 20 at the level 5 on herself).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| The light 20 | 817 | `cParticleSystem_maglite` | `0x784DD0`, `0x784E90`, `0x784E60`, `0x7850D0` |

- Its start (`0x7850D0`): its seconds `0x4D26B0(0, L) · [+0x70]` (`+0x20A8`), its light `⌊0x4D26B0(1, L) · [+0x70]⌋` (`+0x20AC`, then `+0x9C`), the effect 35 on its bearer for its seconds, its flags `0x600`; it draws no particle.
- Its FX event's case `0x2C` (`0x5A4AC0`) has it follow the event's object at its bone 20 (`+0x68`, `+0x70`, `Bip01 Head`), which the flag `0x200` keeps it at at each frame.
- Its run (`0x784E60`): past its seconds it ends.
- Its draw (`0x784E90`): 21 flat quads (`0x7631E0`) of `PARTICLE_GLOW03` at the heights `2.5 (k − 10)` about its place, `k` of 0 to 20, each of the half side `√(3025 − h²)` (a ball of the radius 55 in slices), of (175, 175, 255) at an alpha of 24 (`0x18AFAFFF`), added by their alpha (`SRCALPHA`, `ONE`), their depth tested.

`opensacred-game` draws it since #1092 (`magic_lights`, `opensacred_rules::particles::maglite_slices`): the object that `spells` puts on the light's bearer gets its ball at the bearer's head at each frame, untinted by the night, and its light of the spell's second value at its level (525 at the level 5) at its place on the ground.
**Seen** 2026-11-04 beside the original's frames in the chapel of 50,39: a pale blue-white glow about her head and raised arms, alike in size.

## The fire skin

Read 2026-11-04 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Mage's new game, her spell 24 at the level 5 on herself).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| The fire skin 24 | 821 | `cParticleSystem_fireskin` | `0x78B9D0`, `0x78BBA0`, `0x78BB00`, `0x78BE10` |

- Its constructor makes its ramp (`+0x20C0`): from a transparent white to white over 32 entries, to black over 224; and loads `PARTICLE_FIRE02` (`+0x254C`), `PARTICLE_FIRE09` (`+0x2550`) and `PARTICLE_GLOW03` (`+0x2554`); 100 particles.
- Its start (`0x78BE10`): its seconds `0x4D26B0(0, L) · [+0x70]` (`+0x20A4`); its glow's colour (255, 192, 1) for a hero's (`+0xA9` 0), (64, 64, 255) else (`+0x24C0`); the effect 25 on its bearer for its seconds; one layer (the life −350 a second, the size −50) and one block (the size 20 ± 10, the place ± (15, 15, 0), every 0.01 s, times 1.5 and 2.2 at the detail levels 1 and 0); the action 6 of `A_LAVA00` (`+0x80` 6, `+0x84` 10, `+0x88` the texture); its flags `0x600`, its light 200.
- Its run (`0x78BB00`): its particles aged, and emitted until its seconds are over, when its flags are cleared and it ends with its last particle.
- Its draw (`0x78BBA0`): while it burns, a flat quad (`0x7631E0`) of `PARTICLE_GLOW03` of a half side of 60 in its glow's colour, added (`ONE`, `ONE`), its depth tested; then its particles as billboards of the flags `0x89` (the ramp's colour, standing on their feet, added by their alpha), its 4 × 4 frames by their age, of `PARTICLE_FIRE02` (a creature's `09`).

`opensacred-game` draws it since #1092 (`fire_skins`, `opensacred_rules::particles::fire_skin`, the flames by `breath::wall_quads`): the object that `spells` puts on the Mage, and on a creature that casts it (#1852) in blue, gets a look of its own that leaves its flames about its bearer while it burns, its glow under it and its light of 200, its flames burning out after, untinted by the night.
**Seen** 2026-11-04 beside the original's frames at the Mage's start: flames rising about her feet over an orange glow, alike in size.
- **The skin of lava**, read 2026-11-05 for #2042: the object's action 6 (`0x5B2600`'s case 6) gives its effects and sends its bearer the event `0x109` (`0x5B576F`) of its data (`+0x14`, 10), its seconds (`+0x18`, the record's value 0 at its level, truncated, the effects' own; 1 without a record) and its texture (`+0x1C`); the creature's events' handler (`0x52E590`, the case `0x109` of its switch from `0x101`) keeps them as its overlay's state (`+0x60`), its timer (`+0x68`, a float) and its texture (`+0x6C`).
  The objects' update (`0x44A520`) counts the timer down by the frame's time and ends the state 10 at 0 (the states 1, 4 and 7 go on to 3, 5 and 8 for 2 s; 11 counts up; 13 rises and falls by its `+0x6C`).
  Its draw (`0x44B230` through `0x44A9D0`, whose jump table at `0x44B1B8` holds 24 states): the state 10 draws the model as ever (`0x44A8C0`), then again (`0x40D310`) with the texture by the model's own coordinates, its colour a grey of `⌊128 + 64·sin(4·t)⌋` for `t` the seconds left (`0x44AD7B`, the floats at `0x89098C`, `0x890914` and `0x890D2C`), unlit, added (`ONE`, `ONE`), its depth tested and not written, the faces culled as the model's.
  The pass draws the creature's own sequence, onto which its worn items are merged (`GrannyWearSequence`): the Mage's hood takes the lava, her staff, an object of its own, does not.

`opensacred-game` draws the skin since #2042 (`skins`, `opensacred_rules::spells::{LAVA_SKIN_STATE, lava_skin_grey}`): the fire skin, the hero's or a creature's, asks its bearer for it for its seconds, a new one setting them anew, and each mesh of the bearer and of the items it wears, not of those in its hands or on its back, gets a copy skinned to the same joints, of `A_LAVA00` unlit and added in the objects' layer of the blended things (`models::Pass::Added`), whose grey pulses by the seconds left; the seconds over, the copies go.
**Seen** 2026-11-05 beside the original at the Mage's start, her spell 24 at the level 10: in both games the Mage orange all over under her flames, her hood with her and her staff as it was.
  The game's lava looked the brighter of the two in the frames, its pulse of the same grey (not measured apart from the flames in front of it).
  Its grey was then taken in linear light, which the models' material encodes for the frame, so that a grey of 0.5 added about 0.73 of the texture and one of 0.25 about 0.54; and the texture's smaller levels averaged its pattern out.
  Since #2169 the skins' grey multiplies the texels' values in the frame, as the device's modulation by the vertices' colour does (`fx::linear`), and their textures are sampled at their one level, linearly, as `0x40D310` asks (its state flag `0x800`, `0x643470`).
  Since #2172 the copies are drawn after the night's pass (`iso::UNTINTED`): `0x40D310` draws unlit in the grey alone, which no light of the night reaches, as the big demons' pulse showed beside the original (docs/re/drawing.md, "The models' effects").

## The gust

Read 2026-11-04 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Mage's new game, her spell 28 on herself at the start).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| The gust 28 | 826 | `cParticleSystem_twister` | `0x788850`, `0x788B80`, `0x7888E0`, `0x788D90` |

- The executor (`0x524D7E`) makes it at its bearer with the colour of the ground under it: the cell's nature (its `+0x1F` high nibble, `0x634C50`) indexes the renderer's table at `+0x9DC8` (`0x41BA00`), else (128, 128, 128).
  The table (`0x418EF3`): 1 (239, 239, 192), 2 (224, 224, 239), 3 and 4 (48, 64, 16), 5 (134, 154, 38), 6 (239, 144, 25), 7, 11 and 12 (128, 128, 128), 8 (192, 192, 192), 9 (25, 193, 239), 10 (25, 74, 239), 13 (181, 179, 179), 14 (227, 117, 43), 15 (236, 203, 48), all opaque; the nature 0's entry is left 0.
  **Seen** 2026-11-04 (a probe at `0x788D9B`): the colour `0xFF304010` at the Mage's start, on grass.
- Its start (`0x788D90`): its four corners' colours the event's `+0x38`, its seconds `0x4D26B0(0, L) · [+0x70]` (`+0x20C8`), the effect 25 on its bearer, its flags `0x600`, its loop 2017 (`MAGE_WIRBELWIND_LOOP`); its 250 particles' lives 1.0.
- Its run (`0x7888E0`), at its first each particle rolled: its angle about its bearer a whole turn at random (`+0x0C`), its radius 30 to 35 (`+0x10`), its share of the column's height the cube of a roll of 0 to 1 (`+0x14`), its speed 0.3 to 4 radians a second (`+0x18`), its quad's angle at random (`+0x30`).
  Then at each run each particle stands at its angle at the radius `(0.6 s + 1) r` and the height `H s` about its bearer, of the half side `7 (2 s + 1)`, its angle on by its speed; the column `H` rises at 30 a second to 100 (its phase 0), stands to its seconds (1), then climbs at 50 a second while its particles spin faster by 6 a second, for 2 s (2), when it ends.
  The particles never age (no layer's update).
- Its draw (`0x788B80`): billboards of the flags `0x44` (its corners' colours as they are, turned, tinted by the light at it, laid over by their alpha) of `PARTICLE_CLOUD01`.

`opensacred-game` draws it since #1092 (`twisters`, `opensacred_rules::particles::{Twister, GROUND_COLOURS}`, `terrain::Heights::nature`): the object that `spells` puts on the Mage gets a look of its own about her that outlives it by its 2 s of going, in the colour of the ground under her at each frame, drawn with the world under the night's pass.
**Seen** 2026-11-04 beside the original's frames at the Mage's start: a dark green swirl of dust widening and rising about her, alike.

## The battle fog

Read 2026-11-04 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Dark Elf's new game, her trap 1053 at the level 5 on a goblin).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| The battle fog | 868 | `cParticleSystem_battlefog` | `0x770670`, `0x7707D0`, `0x770700`, `0x7709F0` |

- Its makers: the Dark Elf's fog trap 1053, four at (±60, ±60) about where it lands (docs/re/arts.md), and the dryad druid's blink at each end (#1883).
  **Seen** 2026-11-04 (a probe at `0x770A02`): four starts at the trap's landing, each of the variant (`+0x3C`) 0, the level 5 and the factor 1.
- Its start (`0x7709F0`): its seconds `0x4D26B0(0, L) · [+0x70]` (`+0x20A4`); its ramp from the variant's colour (1 (193, 183, 173) and 2 (173, 183, 193) at an alpha of 150, else a grey of 120 at none, in 256ths) to a grey of 193 at 150 over 64 entries, to one of 150 at none over 192; one layer (the life −50 a second) and one block: buoyancy 0 ± 3, size 60 ± 20, angle π ± π, spin 0 ± 0.3, place (0, 0, 50) ± (30, 80, 50), a frame below 4, every 0.3 s (times 1.5 and 2.2 at the detail levels 1 and 0); its flags 0, no light; the sound 2055 once.
- Its run (`0x770700`): the renderer's vector in its layer's wind, as the smoke class's; its particles aged, and emitted for its seconds; then it lives while a particle does.
- Its draw (`0x7707D0`): billboards of the flags `0x14C` (its frame of `PARTICLE_SMOKE03`'s 2 × 2 its own, the ramp's colour, turned, tinted by the light at it, laid over by its alpha).

`opensacred-game` draws it since #1092 (`battle_fogs`, `opensacred_rules::particles::battle_fog`): the four fogs of the trap 1053 and the dryad druid's blink's puffs, drawn with the world under the night's pass.
**Seen** 2026-11-04 beside the original's frames at the Dark Elf's start: grey fog billowing up over the trap's cells, alike in size and colour.
Not followed: its layer's wind (as the world's smokes'), and its variants 1 and 2, whose makers are not read.

## The banishing circle's runes

Read 2026-11-04 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Mage's new game, her spell 16 at the level 5).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| A rune of the banishing circle 16 | 805 | `cParticleSystem_groundsymbol` | `0x781830`, `0x781DC0`, `0x781D60`, `0x7825E0` |

- The executor (`0x522DE3`) makes eight about the caster, the rune `i` at the angle `i·π/4 + π − 0.5`, 200 off along the world's `sin` and `cos`, on the ground where a creature may stand (`0x636890`, `0x449950`), its FX event's `+0x38` the index `i`.
- Its constructor makes its quads: the rune (`+0x2144`), 64 across and 32 high about its place; the bar (`+0x21C4`), 12 across and 0 to 64 up; and three rays (`+0x2244`, `+0x22C4`, `+0x2344`), 16 across from 4 below to 48, 48 and 64 up, the second's texture twice across; and loads `runeshield_gradient` and `particle_glow03`.
- Its start (`0x7825E0`): its seconds `0x4D26B0(0, L) · [+0x70]` (`+0x2140`); the textures `runeshield<i+1>`, its `_glow1` and `_glow2`; one layer (the life −500 a second) and one block (buoyancy −200, the size 1 ± 0.25, the velocity (0, 0, 30) ± (0, 0, 5), the place ± (6, 6, 0), every 0.05 s, times 1.5 and 2.2 at the detail levels 1 and 0, the colour (128, 64, 0)); its light 50, its flags 0.
- Its run (`0x781D60`): its particles aged and emitted; past its seconds it ends.
- Its draw (`0x781DC0`), with the draw's own matrix, so that its quads face the camera, its translation its place, the depth tested and not written:
  - the rune, white, laid over by its alpha;
  - then added (`ONE`, `ONE`): its first glow in the grey `⌊128 (sin t + 1)⌋`, its second white; the bar of the gradient in `(2c, c, 0)` for `c = ⌊16 (sin t + 1) + 32⌋`; the rays in the same of `c = ⌊4 (sin t + 1) sin²(k t) + 8⌋`, `k` 2.1 for the first and 1.5 for the others;
  - its particles as billboards of the flags `0x13` (its corners' colour, `ONE`, `ONE`) of `particle_glow03`.

`opensacred-game` draws it since #1092 (`banishing_runes`, `opensacred_rules::particles::{Rune, rune_colours}`): the eight runes that `spells` makes about the Mage for the circle's seconds, at their angles and where she may stand, untinted by the night, each its light of 50.
**Seen** 2026-11-04 beside the original's frames at the Mage's start: glowing red runes on the ground about her with an orange bar of light rising from each, alike in place and size.

## The fire spiral

Read 2026-11-04 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Mage's new game, her spell 11 at the level 5).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| A flame of the fire spiral 11 | 790 | `cParticleSystem_fireCircle` (`0x897BA8`) | `0x775930`, `0x775BC0`, `0x775A70`, `0x775DF0` |

- Its constructor makes its ramp (`+0x20C4`): from a transparent (255, 178, 127) to the same opaque over 32 entries, held over 224; and loads `PARTICLE_FIRE02` (`+0x25B0`), `PARTICLE_FIRE09` (`+0x25B4`) and `PARTICLE_GLOW03` (`+0x25B8`, unused); 100 particles.
- Its start (`0x775DF0`): its seconds `0x4D26B0(0, L) · [+0x70]` (`+0x20A4`), 3 for a creature's (`+0xA9`); one layer (the life −350 a second, the size −70) and one block (the size 40 ± 20, the place ± (7, 7, 0), every 0.01 s, times 1.5 and 2.2 at the detail levels 1 and 0); the loop 2019 (`0x7E3`) for its event's `+0x38` 0; its flags `0x200000`, its light 300.
- Its run (`0x775A70`): its particles aged and emitted at its place; its value `+0x98` the least of `⌊150 t⌋` and 175, `t` its clock, which the object's run takes for the spiral's growth (`0x5A6B1E`, docs/re/arts.md, the fire spiral); past its seconds, or at its object's end (`+0x74` bit 1, its flags cleared), it emits no more and ends with its last particle.
- Its draw (`0x775BC0`): its particles as billboards of the flags `0x89` (the ramp's colour, standing on their feet, added by their alpha), its 4 × 4 frames by their age, of `PARTICLE_FIRE02` (a creature's `09`).
- The object's run (`0x5A6B1E`) moves it about the caster's place at the height of that place (`+0xC0`), along the ground's two axes alone.

`opensacred-game` draws it since #1092 (`fire_skins`, `opensacred_rules::particles::fire_circle`, the flames by `breath::wall_quads`): each of the five objects that `spells` makes leaves its flames where it goes, untinted by the night, its light of 300, its flames burning out after its end.
**Seen** 2026-11-04 beside the original's frames at the Mage's start: the five flames' fires merged into a white-hot blob about her at first, then a ring of fire about her as they spread, alike in size.
The original's frames under the driver show the flames gathered in tall columns at a few places at times, a hitch's step of the object between two frames by **hypothesis**, as the screenshots stall it.

## The purgatory

Read 2026-11-04 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Mage's new game, her spell 23 at the level 5 at a goblin).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| The purgatory 23 | 820 | `cParticleSystem_hellFire` (`0x897BC8`) | `0x775F80`, `0x776390`, `0x7762D0`, `0x776860` |

- Its constructor loads `PARTICLE_FIRE02` (`+0x298C`), `PARTICLE_FIRE09` (`+0x2990`) and `PARTICLE_GLOW03` (`+0x2994`) and makes its column's vertices (`D3DFVF_LVERTEX`): a strip of 13 pairs (`+0x254C`), at the angles `k·π/6` on a circle of the radius 7, each at the height 0 and 200; and a strip of 8 (`+0x288C`), across x at −17.5, −7, 7 and 17.5, each at the height 0 and 200; 100 particles.
- Its start (`0x776860`): its seconds `0x4D26B0(0, L) · [+0x70]` (`+0x20A4`); its colour (`+0x24C0`) (255, 192, 1) for a hero's (`+0xA9` 0), (64, 64, 255) else; its ramp (`+0x20C0`) from a transparent (255, 178, 127), a creature's white, to the same opaque over 32 entries, held over 224; one layer (the life −350 a second, the size −70) and one block (the size 40 ± 20, the place ± (25, 25, 0), every 0.01 s, times 1.5 and 2.2 at the detail levels 1 and 0); its speed `+0x98` 40, its flags `0x420`, its loop 2006 (`0x7D6`), its light 200.
- Its run (`0x7762D0`): its particles aged, and emitted until its seconds are over, when its flags are cleared and it ends with its last particle; at its object's end (`+0x74` bit 1) it ends at once.
- Its draw (`0x776390`), while it burns: the states of the depth test on, its write off, the lighting off, the blending `SRCALPHA` and `ONE`, the linear filter and no faces culled; its vertices' colours its colour, of the alphas 160 at the cylinder's foot and 128 at the bands' inner feet, 0 elsewhere; no texture, the world transform the identity; the cylinder (26 vertices) and the bands' outer halves (from 7 to 17.5 on either side, 4 vertices each) as triangle strips; then a flat quad (`0x7631E0`) of `PARTICLE_GLOW03` of a half side of 75 in its colour, added (`ONE`, `ONE`).
  Then, burning or not, its particles as billboards of the flags `0x89` (the ramp's colour, standing on their feet, added by their alpha), its 4 × 4 frames by their age, of `PARTICLE_FIRE02` (a creature's `09`).
- The object's run (`0x5A780B`, the flag `0x20`) moves it at its speed toward its target's place along the unit vector between them in three dimensions.

`opensacred-game` draws it since #1092 (`purgatories`, `opensacred_rules::particles::{hellfire, hellfire_column}`, the column by `holy_lights::column_quads`, the flames by `breath::wall_quads`): the object that `spells` makes, the Mage's or a creature's (#1854) in blue, gets a look of its own on the ground under it, its flames, its column of light and its glow while it lives, untinted by the night, its light of 200, its flames burning out after.
**Seen** 2026-11-04 beside the original's frames at the Mage's start: a golden column of light rising out of the frame from a fire, alike in breadth and place, as it goes from the goblin to the Mage.
In the original the Mage shows inside the fire where it stands on her, as if drawn over its flames; in `opensacred-game` its flames in front of her cover her.

## The hail of meteors

Read 2026-11-04 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Mage's new game, her spell 26 at the level 5 at a goblin, by day).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| A meteor of the hail 26 | 823 | `cParticleSystem_meteor` (`0x898050`) | `0x786390`, `0x786AC0`, `0x7866C0`, `0x786EF0` |

- Its constructor makes its two ramps: the fire's (`+0x20AC`) from a grey of 86 opaque to (134, 116, 20) at an alpha of 134 over 14 entries, to (233, 128, 47) at 152 over 13, to (152, 83, 31) at 230 over 60, to nothing over 107 and 62; the smoke's (`+0x24AC`) from a grey of 43 opaque to (120, 128, 128) over 24, to a grey of 60 at 203 over 44, to nothing over 188 (the values out of 256).
  It loads the rock `gibs01.grn` (`+0x29C0`), `PARTICLE_SMOKE04` (`+0x29C8`), `C_LAVA00` (`+0x29D0`), and `FX_KRATER02` where a `rand()` is even, else `FX_KRATER01` (`+0x29CC`); 700 particles.
- Its start (`0x786EF0`): its rock's two angles a whole turn at random each (`+0x29B8`, `+0x29BC`); its wait its event's `+0x38` in milliseconds (`+0x20A4`); two layers (`+0x28AC`): the fire's of the life −400 a second and the size −5, the smoke's of −150 and +8; and two blocks (`+0x28EC`, `+0x2950`) at the head (−100, 0, 500) of its frame, one every 0.0012 s (times 1.5 and 2.2 at the detail levels 1 and 0): the fire of the size 6 ± 1, the velocity 0 ± 3 along each axis, the smoke of the size 5 ± 1, the buoyancy 5 ± 10, the angle's speed 0 ± 3, both turned at random; its flags 0, no light.
- Its run (`0x7866C0`), by its phase (`+0x29B4`):
  - 0: past its wait it plays `MAGE_METEOREINSCHLAG02` where a `rand()` is even, else `01`, at a rate of `0.4 + (rand() % 10) / 45`, the rate rolled first, and falls;
  - 1: its particles aged and emitted, two layers, the second one in five (`0x7687F0`); its blocks' head moves by (160, 0, −800) a second; at the height 0 it sends the weather's event of the kind 7 (a quake of 30), its blocks made anew at its place, the fire within 3 of it and the smoke within 2 (its buoyancy −5 ± 2), both of the size 15 ± 5 flying at (15, 15, 30) ± 5 along each axis, turned at random, a burst of 10 (the interval 0, `+0x5C` 10); its flags `0x400`;
  - 2: its flags 0; once its last particle is gone, 15 s over the detail's factor (`+0x20A4`);
  - 3: then 3 s;
  - 4: then it ends.
- Its draw (`0x786AC0`):
  - falling: its rock (`0x405C90`, lit as the models are) at its head, four times its size, turned about x by its first angle and about y by its second (`0x6539D0`, `0x653910`, `0x653850`); and a glow (`0x762E10`) of the style 5 (`PARTICLE_GLOW03`) of a half side of 30 in (176, 117, 10) at its head;
  - in the phases 2 and 3 its crater (`0x7631E0`) of a half side of 50 in the light at it (`+0xA0`), opaque, laid over by its alpha (`SRCALPHA`, `INVSRCALPHA`); in the phase 4 of the alpha `⌊85 t⌋`, `t` its seconds left;
  - always its particles as billboards of the flags `0xC` (the ramp's colour, turned) of `PARTICLE_SMOKE04`, the fire added (`ONE`) and the smoke laid over by its alpha.

`opensacred-game` draws it since #1092 (`meteors`, `opensacred_rules::particles::Meteor`): each meteor that `spells` makes, the Mage's or a creature's (#1852), gets a look of its own on the ground where it lands, its rock and glow and its trail as it falls, its quake, its burst and its crater; the fire, the smoke and the glow untinted by the night, the crater darkened by it as the ground is.
**Seen** 2026-11-04 beside the original's frames at the Mage's start by day: streaks of fire falling from the upper left onto the goblin, alike in slant and length, and dark craters left on the ground.
Its smoke shows darker than the original's, which lies over the place as a light grey haze.

## The ring of frost

Read 2026-11-04 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Mage's new game, her spell 35 at the level 5, by day).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| An object of the ring of frost 35 | 840 | `cParticleSystem_circleofice` (`0x898238`) | `0x78D870`, `0x78DD00`, `0x78DC10`, `0x78E070` |

- Its constructor makes its strike's quad (`+0x2144`, `D3DFVF_LVERTEX`): from −60 to 60 along x and from −40 to 400 up, the texture's `u` from 0 at its foot to 1 at its top and its `v` across; and loads `FX_LIGHTSTRIKE01` to `04` (`+0x21D4`, each twice, the frames 0 to 7), `PARTICLE_GLOW03` (`+0x21F4`), `PARTICLE_SPARK02` (`+0x21F8`), `Iceberg01` to `04.grn` by `rand() % 4` (`+0x21C4`), `ICEBERG` and `ICEBERG2` (`+0x21CC`, `+0x21D0`); its sparks' ramp (`+0x21FC`) the grey `k` at the entry `k`; 50 particles.
- Its start (`0x78E070`): its iceberg's angle a whole turn at random (`+0x20AC`); its colour (`+0x20B8`) white for a hero's (`+0xA9` 0), `0xFFFF4040` else; one layer (the life −150 a second) and one block: the buoyancy 140 ± 20, the size 10, the velocity 0 ± 10 along each axis, the place (0, 0, 30) ± 30 along each, all its particles in a burst; its seconds `0x4D26B0(0, L) · [+0x70]` (`+0x2140`), its flags `0x400`, its light 150.
- Its run (`0x78DC10`): striking (`+0x20B0` 0), past 1.5 s its burst, and it stands; every 0.07 s its strike's frame `rand() % 8` (`+0x20B4`); standing, its particles aged, bouncing on the ground (`0x7645C0`), until its seconds are over.
- Its draw (`0x78DD00`):
  - striking: a flat quad (`0x7631E0`) of `PARTICLE_GLOW03` of a half side of 25 in its colour, added by its alpha (`SRCALPHA`, `ONE`); then, the depth test off, the lighting off, the same blending and no faces culled, its world transform its place, its strike in its colour, opaque: the texture of its frame's half (the frames 0 and 1 `FX_LIGHTSTRIKE01`, …), an even frame's `v` from a half at its left edge to 0 at its right, an odd one's from 1 to a half;
  - standing: the glow of a half side of 50; its iceberg (`0x405C90`, lit as the models are, its specular highlights on) at its place turned about the upright by its angle, in `ICEBERG` (a creature's `ICEBERG2`); its particles as billboards of the flags `0x19` (the ramp's colour, `ONE`, `ONE`) of `PARTICLE_SPARK02`.

`opensacred-game` draws it since #1092 (`icebergs`, `opensacred_rules::particles::Iceberg`): each object that `spells` makes gets a look of its own at its place, its strike facing the camera and its glow, then its burst of sparks and its iceberg, untinted by the night but for the iceberg, its light of 150.
**Seen** 2026-11-04 beside the original's frames at the Mage's start by day: white strikes of light flickering down at the objects' places, then white icebergs standing there over a glow, alike in size and place.
Not followed: its strike over all else (its depth test off).

## The chain lightning

Read 2026-11-04 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Mage's new game, her spell 33 at the level 5 at two goblins on open ground, by day).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| A segment of the chain lightning 33 | 799 | `cParticleSystem_freeze` | `0x77D770`, `0x77DB20`, `0x77DAC0`, `0x77DE80` |
| The impact at its end | 872 | `cParticleSystem_freeze_impact` (`0x897E00`) | `0x77E1C0`, `0x77E3A0`, `0x77E2A0`, `0x77E630` |

- The executor (`0x526CAA`) takes the line from the caster's `Bip01 L Hand` (`0x44CBB0` of the bone 1) to the target's (`0x44CA30` by the name `0x401000(1)`), its length `T` and its unit vector, and lays segments along it from the hand: at most 19, each 30 long, the last its rest rounded down, while any of it is left.
  Each segment's cell must be walked on (`0x636A60`), else the spell ends there, with no strike.
  Each end of a segment is offset across it by a vector `(0, y, z)` of its own frame, each of `y` and `z` `16 · rand() / 32767 − 8` (the `z` rolled first, `0x63FDE0`), times `sin(π l / T)`, `l` what is left of the line from that end; a segment's far offset is rolled anew, its near one its predecessor's far one, so that the bolt bulges at its middle.
  Each is an object 799 kept in the caster's array (`+0x263`), given the FX event `0x20` of its index, its length and its two offsets; then an object 872 at the target.
- A segment's constructor makes its 12 vertices (`+0x20F0`, `D3DFVF_LVERTEX`, a triangle list): two quads from 0 to 30 along its `x`, one from −50 to 50 along its `y` and one along its `z`, grey (128, 128, 128); and loads `FX_LIGHTNING01` (`+0x2270`).
- Its start (`0x77DE80`): its world matrix (`+0x20A8`) turns its `x` onto the line (`0x642690`); its far vertices at its length; the texture's `u` from `30 k / 300` to `(30 k + L) / 300` on the first quad and `0.02` of the same on the second, `k` its index, `L` its length; its offsets added to its ends' `y` and `z`; its fade (`+0x20EC`) 1; the first segment a Direct3D light of (0.7, 0.7, 1.0).
- Its run (`0x77DAC0`): after 0.4 s its fade falls by 20 a second; at 0 it ends.
- Its draw (`0x77DB20`): its vertices' colour the grey of half of `⌊255 f⌋`, `f` its fade; the first segment a glow (`0x762E10`) of the style 6 (`PARTICLE_GLOW06`) of a half side of 20 at its place in the grey `⌊255 f⌋`; its light `150 f` for the first, `100 f` for the others; the depth tested and not written, the lighting off, added (`ONE`, `ONE`), no faces culled; the quads in its world matrix at its place.
- The impact's start (`0x77E630`): a burst of all its 15 particles (`PARTICLE_SPARK06`): the buoyancy 50 ± 40, the size 4 ± 1, the velocity 0 ± 150 along each axis, the place ± 3; one layer, the life −400 a second and the pull 150 (`+0x1C`, by which `0x7640C0` turns each particle's velocity toward the system's place); its ramp (`+0x2130`) of `(k/2, k/2, k)` at the entry `k`; its action 2 (`+0x80`, `+0x84` `0x21`), by which its object strikes its target; and its leaps left, the event's `+0x38` less 1 (`+0x2531`).
- Its run (`0x77E2A0`): its burst at its clock's 0; its particles aged; past 0.2 s, with leaps left, its action 4 with them, by which the chain leaps on (`0x5B46CA`); past 0.4 s its fade (`+0x20A8`) falls by 20 a second; it ends with its last particle.
- Its draw (`0x77E3A0`): its Direct3D light; its light `200 f`; its particles as billboards of the flags `0x19` (the ramp's colour, `ONE`, `ONE`); a glow of the style 6 of a half side of 30 in the grey `⌊255 f⌋`.

`opensacred-game` draws it since #1092 (`chain_bolts`, `opensacred_rules::particles::{ChainBolt, ChainImpact}`): each strike's bolt that `spells` makes, its first from the Mage's left hand and each leap from the last target's, to the target's left hand (80 over its place where it has none, **estimate**), its segments' crossed quads in world space along the line, its glow, its segments' lights, and its impact at its end; all untinted by the night.
**Seen** 2026-11-04 beside the original's frames at the Mage's start by day: a thin bright bolt from her hand to the goblin between a glow at her hand and a bright glow and sparks at the goblin, alike in place and size.
Not followed: the bolt's end at a cell that may not be walked on, which in the original ends the spell with no strike (the goblins under a pine at the start took none); the Direct3D lights on the models follow since #2106.

## The lost souls

Read 2026-11-04 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Wood Elf's new game, her spell 41 at the level 5 by two goblins, by day).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| A soul of the lost souls 41 | 850 | `cParticleSystem_lostsouls` (`0x898580`) | `0x79CDD0`, `0x79CFB0`, `0x79CEB0`, `0x79D310` |

- Its constructor loads `PARTICLE_DOTS01` (`+0x2534`) and makes a Direct3D light of (0.2, 1, 0.2); 200 particles.
- Its start (`0x79D310`): its seconds `0x4D26B0(1, L) · [+0x70]` (`+0x20A8`, the record's value 1); its loop 2058 (`0x80A`); one layer (the life −400 a second, the size −4) and one block (the size 7, the angle's speed 6, every 0.01 s, times 1.5 and 2.2 at the detail levels 1 and 0); its ramp (`+0x20B0`) from (60, 255, 60) opaque to nothing over 256 entries (out of 256); its flags `0x10000`, its speed `+0x98` 150; no light of the table's.
- Its run (`0x79CEB0`): its block's place (`+0x2508`) `5 sin` of 6.58, 6.87 and 7.49 times its clock along each axis, its particles emitted there and aged; past its seconds or at its object's end (`+0x74` bit 1) it emits no more and ends with its last particle.
- Its draw (`0x79CFB0`): its Direct3D light; its particles as billboards of the flags `0xD` (the ramp's colour, turned, added by their alpha); then a glow (`0x762E10`) of the style 5 (`PARTICLE_GLOW03`) at its block's place, its half side `20 + 8 b` (`0x768790`) and its colour (128, 239, 128) plus `b` times (64, 16, 64) (`0x768600`), `b` a beat of its clock: from 0 up to 1 over the first 0.3 s of each second, down to 0 over the rest.

`opensacred-game` draws it since #1092 (`lost_souls`, `opensacred_rules::particles::{LostSoul, soul_glow, soul_wobble, beat}`): each soul that `spells` makes leaves its dots along its way and its glow at its wobbling point, untinted by the night, its dots fading out after its object goes.
**Seen** 2026-11-04 beside the original's frames at the Wood Elf's start by day: green balls of light trailing green dots, alike in colour and size, rising from her in a fan and curving about as their headings turn (#2058).
Not followed: its green Direct3D light on the models, which follows since #2106.

## The choir

Read 2026-11-04 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and cast in the running original under the driver of #448 (the Daemon's new game, his spell 66 at the level 5 on two goblins, by day).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| The choir 66 on a creature | 1011 | `cParticleSystem_songsfromhell` (`0x898B60`) | `0x7AFFB0`, `0x7B0110`, `0x7B0040`, `0x7B0200` |

- The executor (`0x527DA9`) makes one at each creature it holds, its FX event's `+0x38` the creature's size (`0x428CE0`, `items.pak +0x14`, 50 for 0).
- Its constructor loads `PARTICLE_GLOW01` (`+0x2148`); 150 particles.
- Its start (`0x7B0200`): its seconds `0x4D26B0(0, L) · [+0x70]` (`+0x20A4`); its base (`+0x20AC`) the size over 6; its fade (`+0x20B0`) 0; the action 9 (the effect 15 of its factor, docs/re/arts.md); one layer (the life −100 a second, the size −2) and one block (the buoyancy −20 ± 15, the size 6 ± 3, the place ± (base, base, 0), every 0.07 s, times 1.5 and 2.2 at the detail levels 1 and 0); its corners' colours black (`0xFF000000`); its flags `0x2200`.
- Its run (`0x7B0040`): its fade rises by its time to 1, and only then its particles age and, while its seconds last, are emitted; with the event's `+0x3C` set, past 2 s the action 10 and its clock started anew (not read further).
- Its draw (`0x7B0110`): five flat quads (`0x7631E0`) of `PARTICLE_GLOW01`, the `k`-th `3 k` over its place, of the half side `(10 − k)(sin t + b) f`, `t` its clock, `b` its base and `f` its fade, black, laid over by their alpha (`SRCALPHA`, `INVSRCALPHA`), their depth tested; then its particles as billboards of the flags 2 (its corners' colour with the alpha of their life, laid over by their alpha) of `PARTICLE_GLOW01`.

`opensacred-game` draws it since #1092 (`choirs`, `opensacred_rules::particles::Choir`): the object that `spells` makes with each creature the choir holds gets a look of its own going with it, its discs a dark shadow on the ground under it and its black wisps rising from it, untinted by the night.
The original's frames show its strikes on the goblins, 37 a second, but its dark look hard to tell among the lilac lights behind them at the Daemon's start (#2063); the game's shows the shadow under each goblin.

## The magic prison

Read 2026-11-04 for #2063 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Daemon's new game, by day).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| A magic bar or barrier of the scripts | 854 `FX_MAGICPRISON` | `cParticleSystem_magicprison` (`0x898560`) | `0x79C960`, `0x79CA40`, `0x79C9F0`, `0x79CC80` |

- **Seen**: the row of lilac lights along the south-west edge of the Daemon's starting platform is eleven objects 854, one a cell from 344,1727 to 354,1727 (`cell-info` about his cell 352,1722), the scripts' `CreateObj` of their magic bars and barriers (#1964).
- Its constructor loads `PARTICLE_GLOW03` (`+0x252C`); 100 particles.
- Its start (`0x79CC80`): one layer (`+0x24A8`: the life −200 a second, the size +10) and one block (`+0x24C8`: the buoyancy 0 ± 20, the size 5 ± 2, the place (0, 0, 50) ± (2, 2, 50), every 0.05 s, times 1.5 and 2.2 at the detail levels 1 and 0); its ramp (`+0x20A8`) the entry `k` of `⌊64 k / 256⌋` red and green and `⌊128 k / 256⌋` blue; `SPELL_BANNKREIS` (2029) once at a rate's factor of 0.5; its flags 0, no light.
- Its run (`0x79C9F0`): its particles aged and emitted, with no end of its own: it stands until its object goes (`DelNPC`).
- Its draw (`0x79CA40`): a flat quad (`0x7631E0`) of `PARTICLE_GLOW03` of a half side of 40 in (128, 128, 255), added by its alpha (`SRCALPHA`, `ONE`); then its particles as billboards of the flags `0x19` (the ramp's colour, `ONE`, `ONE`).

`opensacred-game` draws it since #2063 (`prisons`, `opensacred_rules::particles::magic_prison`): `npcs` makes an entity for each `CreateObj` of 854 at its place and the height of its record's `0x7E`, as the fires, which draws its glows and its ground glow untinted by the night and sounds once as it is made.
**Seen** 2026-11-04 beside the original's frames at the Daemon's start: the same row of lilac lights along the platform's edge, alike in place, height and colour.

## The fear

Read 2026-11-04 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Daemon's new game, his spell 67 at the level 5 by two goblins, by day).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| An object of the fear 67 | 1012 | `cParticleSystem_hellscream` (`0x898B84`) | `0x7B05F0`, `0x7B0870`, `0x7B07B0`, `0x7B0AF0` |

- The executor (`0x528024`) puts its five objects 160 off along the world's `cos` and `sin` of `0.733 + k·2π/5` (a vector turned by `0x529720`), at the ground's height there (`0x636310`), with no test of the cell.
- Its constructor loads `PARTICLE_GLOW01` (`+0x2134`), `PARTICLE_GLOW03` (`+0x2138`) and `laserbeampart` (`+0x213C`), and makes its column's quad (`+0x20B4`); no particles.
- Its start (`0x7B0AF0`): its seconds `0x4D26B0(0, L) · [+0x70]` (`+0x20A4`), its fade (`+0x20AC`) 0, its ring's radius (`+0x20B0`) 10, its flags `0x400`, its light 350.
- Its run (`0x7B07B0`): while its seconds last, its fade rises by its time to 1; after, it falls by 0.3 a second and at 0 it ends; its ring's radius grows by 70 a second, back to 10 past 100.
- Its draw (`0x7B0870`):
  - a flat quad (`0x7631E0`) of `PARTICLE_GLOW03` of a half side of 60 in (255, 130, 70), added (`ONE`, `ONE`);
  - a ring (`0x763530`) of 50 segments flat at its place between its radius and 20 more, of `laserbeampart`, added, of (255, 162, 102) times a share that holds 1 to the radius 75 and falls by 0.04 a unit to 0 at 100, each byte taken whole;
  - with the draw's own matrix, so that they face the camera, its translation its place raised by `4 i f`, 32 quads of `PARTICLE_GLOW01` in (255, 162, 102), added by their alpha (`SRCALPHA`, `ONE`), the `i`-th of the half side `1.7 sin(0.3 i + t) + 1.6 (sin(i − 3t) + 1) + 6 + f`, `t` its clock and `f` its fade: a column of light that rises to 124 as it fades in.

`opensacred-game` draws it since #1092 (`hell_screams`, `opensacred_rules::particles::HellScream`): each object that `spells` makes gets a look of its own on the ground under it, untinted by the night, its light of 350, fading out after its object goes.
**Seen** 2026-11-04 beside the original's frames at the Daemon's start by day: five tall golden columns of light about him, each over an orange glow, with golden rings spreading from their feet over and over, alike in place, size and colour.

## The scripts' Pong

Read 2026-11-04 for #2008 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Daemon's new game).

| Object | FX type | Class (virtual table) | Its constructor, draw, update, start |
|---|---|---|---|
| The hero's bat | 1156 `FX_PONG_BATTER` | `cParticleSystem_pong_batter` (`0x898E54`) | `0x7BBD90`, `0x7BCC60`, `0x7BCBC0`, `0x7BD2C0` |
| The other bat | 1158 `FX_PONG_BATTER_AI` | the same | the same |
| The ball | 1157 `FX_PONG_BALL` | `cParticleSystem_pong_ball` (`0x898E7C`) | `0x7BD4C0`, `0x7BD630`, `0x7BD560`, `0x7BD7A0` |

- Every tree's `Sector84080Enter` makes the three on a court of stone in a town's yard: the other bat at the cell (5407, 5133), the ball at (5401, 5147) and the hero's bat at (5406, 5161), the court's rows from 5133 to 5161 and its columns from 5398 to 5419 (the bats' bounds below).
  So the campaign reaches it, with no quest: a hero that walks into the court plays.
- Their objects' tick (`0x5A6680`) runs, for the system flag `0x2000`, a step of each type (`0x5ABD80`, the cases `0x484` to `0x486`):
  - the hero's bat takes the nearest hero of the objects about it (`0x426650`) and, while that hero's cell lies in the columns 5398 to 5419, puts its x at the hero's, and keeps the hero as its holder (`+0x68`);
  - the other bat puts its x at the ball's within the same columns, so that it never misses (the second hero's in a game of the network);
  - the ball, nearing a bat (its y within 30 world units of the bat's on the court's side, its x within 80 of the bat's) and going toward it, turns its direction back along y (the hero's bat `+1`, the other `−1`), signals the bat (its action 8) and gives the bat's holder experience of twice its speed less 400 (`0x5B1400`, `0x549520`, `addExperience` `0x57E160`); so the hero earns 100 experience at each ball he returns at its start's speed of 250, up to 1,600 at 1,000, and the other bat, of no holder, earns nothing.
- The ball flies under its flag `0x800000` (`0x5A9152` in `0x5A6680`): its place moves at its speed (`+0x98`) along its direction (`+0x78`); where the cell ahead stops it (`0x636D70`, `0x636D20`) it turns back along the axis of the stopping neighbour, off the court's walls; where it stops outside the rows 5132 to 5162, past a bat, it signals itself (its action 8), goes back to its start (`+0xB8`) and turns its direction by `rand() / 32767 − 0.5` radians (`0x529720`).
- The ball's start (`0x7BD7A0`): its speed 250, its flags 0 (it stands), a wait (`+0x20AC`) of 1; its run (`0x7BD560`): the wait falls by 0.5 a second, and at 0 the flags `0x802000` set it flying; every 5 s its speed rises by 20, to 1,000 at most; its action 8 stops it again, the wait 1 and the speed 250.
- The bat's start (`0x7BD2C0`): the flags `0x2000`; its run (`0x7BCBC0`): its action 8 plays `PONG` (16303) at the bat, at a rate's factor of 0.5, and brightens it for 1 s (`+0x20A8`, falling by 1 a second).
- The bat's draw (`0x7BCC60`): a box of twelve lines of `0xFFA0A0FF` (a lilac) about its place (`0x40DA80`) and its faces, 36 vertices of `Fx_dwarf_shield` added whole, of the grey `(sin t + f) · 20 + 100` out of 255 (`t` its clock, `f` the brightening); the box is 140 by 60 by 30 in the models' units (`0x7BBD90`), turned 45° in the frame so that its length lies along the cells' x, across the court.
- The ball's draw (`0x7BD630`): while it waits, two flat quads (`0x7631E0`) of `fx_twirl_rays` turning at −1.7 and −1.3 radians a second, of the half sides `100 w²` and `90 w²` (`w` its wait) in `0xFFA0A0FF`; always a flat quad of `PARTICLE_GLOW03` of the half side `2 r` in `0xFF4040FF` (a blue), `r = (1 − w)(7 cos 4t + 35)`, and a glow of the style 6 (`GLOW06`) of the half side `r` 15 up.
- **Seen** 2026-11-04 at (5398, 5147) and (5406, 5165): the two bats as lilac boxes about four cells long across the court's ends and the ball a blue-white glow between them, the court's walls about it; with the hero standing still behind his bat, it returned five balls in 150 s, each of 100 experience (a trace of `0x549520`), the others passing it.

`opensacred-game` makes them since #2008 (`pong`, `opensacred_rules::pong`): `npcs` places the three pieces at their points, the hero's bat follows the hero's column within 16 cells of it and the other the ball's, within the court's columns, the ball waits, rises in speed, flies at the bolts' scale of a flight (`bolts::flight_scale`), turns back off the cells that may not be walked on and off the bats and is lost past them, and each return sounds `PONG` at its bat, brightens it and gives the hero twice the ball's speed less 400 experience.
The bats' boxes are drawn as their faces of `Fx_dwarf_shield` added whole and their edges as thin lilac quads facing the camera, the ball as its ground glow, its rings while it waits and its white glow over it, untinted by the night.
Simplified: the ball's heading turns in the world's plane where the original turns it in the flight's frame, whose axes are scaled apart (a turn off a bat along the rows exactly, the original's about 3° off them), and a wall turns it back along the axis of the step that crossed into the stopping cell (**hypothesis** for the original's choice of the axis).
**Seen** 2026-11-04 in the game at (5406, 5163): the bats and the ball as the original draws them, and the hero earning 140 experience from a ball his bat returned at the speed of 270.

## The big demon's summoning

Read 2026-11-04 for #2006 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Daemon's new game).

| Object | FX type | Class | Its start, run, draw |
|---|---|---|---|
| A part of the pentagram of a big demon's summoning | 918 `FX_BIG_DEMON_SUMMON` | `cParticleSystem_bigdemonsummon` | `0x7A92E0`, `0x7A8C10`, `0x7A9060` |

- The scripts make six objects at once at one world position and no cell (docs/02 §7.3, #2069), their `0x03` their variants: `HornTot74` and `HornTot2_74` of the quest 74 the variants 0 to 5 at (325413, 134903), the middle of the big demon's lair about the cell (6063, 2513), and the add-on's `hq15_close_gate` 10 to 15.
- `CreateObj`'s FX switch passes the record's `0x03` as the event's variant (its case 7 at `0x489FBA`), of the kind `0x7E`.
- The event's case (`0x5A48F0`) plays `BOSS_SAKKARA_DEMON_GROWL01` (11426) or `_02` (11427) at even odds (`rand() & 1`) at the object, at a rate's factor of 0.5, as each object is set up: six growls at once.
- Its start: one layer (the life −150 a second, the size −2) and one block at its emitter (the size 10, every 0.01 s for the variants 0 and 10 and 0.02 s for the others, at the detail 2); its ramp from black over 5 entries to (150, 150, 196), over 50 to (196, 105, 30) and over 201 to black, the variants from 10 to (150, 100, 100) and (196, 38, 30) instead, out of 256; the variants 0 and 10 a light of 600, the others none.
- Its run (`0x7A8C10`): its emitter's share of a turn rises by 0.25 a second and wraps.
  The variants 0 and 10 put the emitter on a circle of the radius 285 about the object, at `(285 cos 2πp, 285 sin 2πp)`.
  The others put it on a side of the pentagram, from its point `v − 1` to its point `v` over the first half of the turn and back over the second, `v` the variant's last digit; the points (`0x7A8A60`) lie on the circle at `(285 sin a, 285 cos a)` for `a` of 0, 4π/5, 8π/5, 2π/5, 6π/5 and 0 in turn.
  It leaves glows along the emitter's way while its clock is below 8 s; they then age out, in about 1.7 s.
- Its draw (`0x7A9060`): its glows as billboards of `PARTICLE_GLOW03` of the flags `0x19`, the ramp's colour added whole.
- **Seen** 2026-11-04 with the hero at (6063, 2513): a circle and a five-pointed star of orange glows on the ground about the lair's middle, each moving end white-hot, the circle drawn clockwise on the screen, all gone about 10 s after the script; the star's point toward the viewer and a side across the screen below its middle.

`opensacred-game` draws it since #2006 (`summonings`, `opensacred_rules::particles::DemonSummon`): `npcs` makes an entity for each `CreateObj` of 918 at its point and the height of its record's `0x7E`, with its variant, which draws its glows untinted by the night, gives the circle's light and growls once as it is made.
**Seen** 2026-11-04 beside the original's frames at the same cell and times: alike in size, colours, the circle's way and time and the moving ends; the star, first turned 45° about its middle, lies as the original's since #2072, a point toward the viewer and a side across the screen below its middle.

## The blood fountain

Read 2026-11-04 for #1964 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Daemon's game, its options' "Display of violence" ticked for the check and unticked after it).

| Object | FX type | Class | Its start, run, draw |
|---|---|---|---|
| The blood that a creature spurts from a bone | 908 `FX_BLOODFONTAIN` | `cParticleSystem_bloodfontain` | `0x77BD60`, `0x77BA90`, `0x77BB40` |

- **Who makes it.** The death's action (`0x53ECA0`) as the dying clip starts (`0x53F37C`): a creature that bleeds (`0x4281F0`, below), in a roll of `rand() % 101` below 10 (30 with `+0x2B8` bit 1, not read), without `+0x2B7` bit 8 (a follower that rises, docs/re/ai.md), with no dialogue (`+0x245` below 1) and whose master (`+0x251`) is not the local player, calls the maker `0x4E87E0` with its part `+0x574` and 1.
  The part is rolled as the creature is made, `rand() % 6` (`0x52DFA1`).
  The maker makes nothing unless `VIOLENCE` is 1 (`0x819FD0`, docs/re/fight.md, "The burst"), the options' "Display of violence", which the GOG installation's `Settings.cfg` holds at 0, and, called with 1, nothing once the creature's `+0xB0` has bit `0x20`.
  The parts 0 to 4 make the 908 at the bones 20 (`Bip01 Head`), 43 (`Bip01 L UpperArm`), 44 (`Bip01 R UpperArm`), 6 (`Bip01 L Thigh`) and 7 (`Bip01 R Thigh`), setting the bits `0x20`, `0x40`, `0x80`, `0x100` and `0x200` of `+0xB0`, the part 5 none; the part 100, which the sets' bleeding sends at a hit (docs/re/sets.md, M15), one at a bone it rolls.
  Each sends the FX event `0x78` (its case `0x5A5555` keeps the system at the creature's bone, `+0x38`), its `+0x3C` the creature's gib set (`0x427630`, `items.pak +0x6F`).
- **Who bleeds** (`0x4281F0`): no creature of the `Creature.pak` class 5 (the undead), none of the gib sets 3 or 4, and none of the types 1 to 7, 46, 52, 53, 55, 63, 74, 88, 100 to 103, 125, 126, 130, 700 and 706, the dragons among them.
- **Its start** (`0x77BD60`): its strength 1; one layer (the life −180 a second, the size +12) and one block (the buoyancy 45 ± 10, the size 5 ± 3, the angle π ± π, the velocity (0, 0, 35) ± 3 on each axis, the place ± 1 on each axis, every 0.005 s at the detail 2, 1.5 and 2.2 times that below); its ramp by the gib set from (255, 12, 0) at the whole alpha over 124 entries to (165, 3, 0) at 165, over 61 to (138, 0, 0) at 140 and over 71 to nothing, out of 256, the set 1 green ((12, 255, 0), (3, 165, 0), (0, 138, 0)) and the set 2 near black ((12, 12, 0), (3, 3, 0), black); the system's flags `0x200`, no light.
  It plays `BLEEDING01` (710) or `BLEEDING02` (711), by the truncation of `rand()` / 32767 times just below 2, at the object, at a rate's factor of `0.4 + 0.2·U`.
- **Its run** (`0x77BA90`): its clock runs, and past 5 s its strength falls by 0.5 a second; at 0 the system ends (its alive flag `+0x20A8`, `0x7C7A40`), its drops with it.
  The block's velocity is `(35·strength, 0, 0)` turned by the system's orientation (`0x653EA0`), the bone's: the drops leave along the bone's own axis `x`.
  The particles' update (`0x7645C0`) takes its mode 2: a drop at or below the system's place less its height over the ground dies (its life set to 0), where the mode 0 bounces it; then the steady emission (`0x764760`), up to 400 drops.
- **Its draw** (`0x77BB40`): its drops as billboards of `particle_dots03` of the flags `0x4C`, of their ramp's colour, turned and tinted by the light at them, laid over the frame by their alpha.
- **Seen** 2026-11-04 with the violence on: the wolves 588 killed by the driver made the 908 at a few of their deaths (`0x4E87E0` and the start traced); the same option also leaves blood on the ground under the dead and the struck (**hypothesis**: the puddles 903 and 907, `FX_BLOODPUDDLE`, makers not read), which the frames' clutter of corpses kept from comparing with the fountain's drops.

`opensacred-game` makes it since #1964 (`blood_fountains`, `opensacred_rules::particles::{BloodFountain, bleeds}`) under `[gameplay] violence`, off by default as the installation's `VIOLENCE` (docs/10 §3): a creature that dies, but the hero, the hero's followers and those of a dialogue, bleeds by its class, its gib set (`Item::gib_set`) and its type, in one case of ten at the bone of a part that its death rolls rather than its making (the same odds, as a creature that bleeds dies once), with its sound, its drops dying on the ground where it fell.
**Seen** 2026-11-05 in the game with the violence on: a few of the wolves 588 killed by the console spurted, a jet of red drops from the head or a limb that fell to the ground beside the corpse.
Not followed: the 30 of `+0x2B8` bit 1, the sets' fountains at a hit (the part 100) and the blood on the ground.

## The dragons' fire

Read 2026-11-05 for #1964 in Ghidra (**confirmed** by code unless marked otherwise).

| Object | FX type | Class | Its start, run, draw |
|---|---|---|---|
| A dragon's breath of fire | 852 `FX_DRAGONFIRE` | `cParticleSystem_dragonfire` | `0x77AE90`, `0x77AAE0`, `0x77AC20` |
| A fire that the breath lays on the ground | 923 `FX_DRAGON_GROUNDFIRE` | `cParticleSystem_dragon_groundfire` | `0x7AB1A0`, `0x7AB070`, `0x7AB0E0` |

- **Who makes it.** A creature of the mark 4 (`0x43C540`: the types 52, 53, 55, 73 to 75, 88, 94, 100 to 103 and 348) that comes on to its target on its level from 371 away takes, by a roll of `rand() % 101` below 5 at a tick, its melee's state 9 (`0x4E1F5C`), but the big demons 53 and 94, which cast (docs/re/ai.md).
  The state plays the clip 0x4F (0x50 for the fire ghoul 73, 0x91 for the types 186 and 333) and turns the creature to its target, and by the type: the medium spider's shot (75, docs/re/ai.md), the fire ghoul's own, nothing for 55 and 348 (their spit is their attack's, 880), and for the others, the dragons 52, 74, 88 and 100 to 103, the 852 at the dragon's place (`0x4E3B40`) with the FX event `0x4C` (its case `0x5A55A1` keeps the system at the caster's bone 20, `Bip01 Head`), its variant 1 for 74 and 100, 2 for 88 and 0 for the others, its array's first word the dragon's level; then `FIREGHUL_FLAMEBREATH` (11122) at the object, at a rate's factor of 0.5 (`0x4E3DB9`).
- **The breath's start** (`0x77AE90`): its clock 0, a timer of the fires on the ground of `0.3·U`; one layer (the life −1,300 a second, the size +80) and one block (the size 7 ± 3, the angle π ± π, the velocity ± 50 on each axis, the place ± 5 on each axis, every 0.001 s at the detail 2); the system's flags `0x200`, no light; and `FIREGHUL_FLAMEBREATH` once more, at the same rate.
- **Its run** (`0x77AAE0`): its clock runs, and nothing happens until it passes 1.5 s; then its light is 400, the block's velocity `(0, −1000, 0)` turned by the system's orientation (`0x653EA0`), the head's, its particles age (`0x7640C0`) and new ones leave (`0x764760`, up to 500); past 4 s it ends (`+0x20B4` 0); else its timer counts down, and below 0 is set to 0.3 and asks the action 1 (`0x5B2600`) of the type 923, of its variant, at its place: a fire on the ground there.
  The engine's frame mirrors the model's `y` (the FX frame is left-handed), so the flames leave along the head's own `+y`, out of the mouth.
- **Its draw** (`0x77AC20`): its flames as billboards of `PARTICLE_FIRE01`, `PARTICLE_FIRE06` or `PARTICLE_FIRE07` by its variant, of the flags `0x15` and the grid 4: added whole, turned, of the four corners' white, their 16 frames played once over their lives.
- **A fire on the ground** (923): its start (`0x7AB1A0`) one layer (the life −100, the size −17 a second) and one block (the size 40 ± 10, the place ± 50 along the ground's two axes, every 0.04 s at the detail 2), its ramp from nothing over 10 entries to a grey of 200/256, held over 200 and to nothing over 46, at an alpha of 0; its light 200; its system's flags `0x400`, the targets and the hits ("The FX objects" above).
  Its hits (`0x5AF75D`, read 2026-11-05 for #2084): at its first update with targets (`+0x11C` 0, then 1), each kind's damage is `⌊k·(1 + v/100)·(L + 9)⌋`, held to 65,534, `k` the balance's `dragonfire_dmg_ph`, `_fi`, `_ma` and `_po` (2, 2.5, 0 and 0), `v` the damage multipliers' curve at the level of the dragon's skill 2 (`0x582450(2)`, `0x565D70` with `WKoff` to `WK_w`) and `L` the object's array's first word, the dragon's level; each target then takes the event 5 of `0x51CB00` from the dragon with those damages, once.
  Its run (`0x7AB070`): its flames age, new ones rise while its clock is below 2 s, and then it lives while one does.
  Its draw (`0x7AB0E0`): billboards of `PARTICLE_FIRE02`, `PARTICLE_FIRE05` or `PARTICLE_FIRE08` by its variant, of the flags `0x99` and the grid 4: standing on their feet, added whole, of their ramp's colour, their 16 frames played once over their lives.

`opensacred-game` makes them since #1964 (`dragon_fires`, `opensacred_rules::particles::{DragonFire, GroundFire}`, `casts::{coming_on, DRAGON_FIRE}`): a dragon coming on to the hero from 371 away breathes at a tick by the same roll (`minds::Mind::cast`), by the clip 0x4F at the creature's speed, the breath at its head with both its sounds, its flames untinted by the night, its light while it burns, and a fire on the ground under the head every 0.3 s, each lit by 200.
**Seen** 2026-11-05 in the game: a dragon 52 of level 10 woken by the hero and left 12 cells behind came on and, now and then, stopped to breathe a jet of flames from its mouth toward her about 2 s after its clip began, with fires rising on the ground under its head.
Since #2084 each fire on the ground strikes, at its first frame with the hero or a follower of the hero's nearer it than their two sizes (923's `items.pak +0x14`), each of them once with those damages from the dragon, as a spell's (`opensacred_rules::fight::Rules::ground_fire_damages`); **seen** 2026-11-05: a dragon of level 10 breathing at the hero put under its head struck her twice for 55 each through her resistances, its fires' damages 48 and 60.
**Seen** in the original 2026-11-05 at the sector 12,74 ("The lava bursts" below): its lava dragon breathed at the Daemon beside its mouth, a white-hot jet from the mouth along its heading and white flames standing under it, as the game's; the dragon 52 by the start under the driver had kept its AI's state 0 by the Daemon, struck too, and walked off.

## The lava bursts

Read 2026-11-05 for #1964 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Daemon's game at the sector 12,74).

| Object | FX type | Class | Its start, run, draw |
|---|---|---|---|
| A rock of lava thrown out of deep lava | 877 `FX_LAVABURST` | `cParticleSystem_lavaburst` | `0x79F160`, `0x79EEE0`, `0x79EFB0` |
| The small fire where it leaves and where it lands | 878 `FX_SMALLFIRE`, the variant 0 | `cParticleSystem_smallfire` | `0x79F970`, `0x79F6D0`, `0x79F7D0` |

- **Who makes it.** The view, as it draws a liquid cell (`cWorldView_renderTileRow`, `0x62B680`): a cell of the liquid materials 4 to 6 (`a_lava`, `b_lava`, `c_lava`, docs/re/liquids.md) whose four corners' beds lie 2 or more below the surface (`+0x10` to `+0x13` at most −2) and whose eight neighbours are liquid (the natures `0x90` and `0xA0`) calls `0x41AF60` with its middle.
  While the renderer's cooldown `+0x9E08` is not above 0, by a roll of `rand()` whose share of 100 is below 0.3, that makes an 877 (the FX event 100) and an 878 (the event `0x65`, its variant 0) there and sets the cooldown to 1.0; the renderer's update counts it down by its frame's time (`0x41A614`; **hypothesis** that the time is in seconds).
  So while deep lava is in view a burst comes about once a second, the draw's rolls among its cells.
- **The event 100** (its case `0x5A3C88`), of the variant 0: the object's flag 8 throws the system in an arc from the event's point, its share of the way rising by `0.4 + 0.2·U` a second, its height `80 + 100·U`, toward a heading `2π·U` along the ground, `150 + 100·U` away; the arc's place is the way's point at its share raised by `(1 − 4(s − ½)²)` of its height (`0x5A7108`), and at a share of 1 the object lands (`+0x74` bit 2).
  Its variant 1 throws the fire skin's stopped arrows (`arrow_long.grn`, `0x561751`; not read further).
- **The burst's start** (`0x79F160`), of the variant 0: its rock `gibs01.grn` at 4 times its size, turned by two angles at `2 + 2·U` radians a second each; its ramp at the entry `k` the colour `⌊128·k/256⌋`, `⌊50·k/256⌋`, `⌊10·k/256⌋` of 255, opaque; one layer (the life −200 a second, the size −10) and one block (the size 5 ± 2, the place ± 1 on each axis, every 0.02 s at the detail 2); the system's flags 8; and `LAVABURST` (206) at the object at a rate's factor of `0.4 + 0.2·U`.
- **Its run** (`0x79EEE0`): its trail ages and, until it lands, leaves new glows at its place; its rock turns; landed, it asks the action 1 of the type 878 at its place (`0x5B2600`, on the ground) and ends with its trail.
- **Its draw** (`0x79EFB0`): while it flies, its rock textured with `C_LAVA00` at its place (the world's models' lighting), its trail as billboards of `PARTICLE_GLOW03` of the flags 9 (their ramp's colour, added by their alpha), and a glow of the style 5 (`PARTICLE_GLOW03`) of a half side of 30 in (128, 53, 10), added whole.
- **The small fire's variant 0** (`0x79F970`): no ramp but its four corners' orange `0xFFFF701A`; one layer (the life −100, the size −12 a second) and one block of no interval and a burst of 5 (the size 20 ± 3, the place ± 5 along the ground's axes), made at its clock's 0; its light 100; its system's flags 0, so that it strikes nothing.
  Its draw (`0x79F7D0`): billboards of `PARTICLE_FIRE02` of the flags `0x81` and the grid 4: standing on their feet, added by their alpha, of the corners' orange, their 16 frames played once over their lives.
- **Seen** 2026-11-05 at the sector 12,74: bursts traced every few seconds (`0x79F160`) and small flames standing on the lava and on the rock about it.
  The lava dragon there breathed its fire at the Daemon beside its mouth at the same time ("The dragons' fire" above): a white-hot jet from its mouth along its heading and white flames standing under it.

`opensacred-game` makes them since #1964 (`lava_bursts`, `opensacred_rules::particles::{LavaBurst, LavaFlames, deep_lava}`): `liquids` keeps each sector's cells of deep lava (a cell on its sector's edge left out), and once a second at most each of them in view rolls the burst in turn, the first that wins throwing its rock with its sound and a small fire there; the rock is the meteors' (`meteors`), the trail, the glow and the flames untinted by the night, the flames lighting it.
**Seen** 2026-11-05 at the sector 12,74 in the game: rocks of lava arcing out of the pool with their glows and trails about once a second, small fires on the lava where they leave and on the rock where they land.

## The dryad ghost

Read 2026-11-05 for #1964 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Seraphim's game at the sector 16,79).

| Object | FX type | Class | Its constructor, start, run, draw |
|---|---|---|---|
| The dryad ghost | 1162 `FX_DRYAD_GHOST` | `cParticleSystem_dryad_ghost` (`0x898F18`) | `0x7BF100`, `0x7BF3A0`, `0x7BF1E0`, `0x7BF2C0` |

- **Its maker.** `FillSector`'s reader turns the type 1249 (`TYPE_NPC_DUMMY`) into 1162 in both its entry tags (`0x47C893`, `0x47C909`), its weight and third word kept; 60 entries of the Seraphim's tree name it, all hostile, in the dryads' sectors (16,79, 17,79, 19,72 and others).
  The fill's creature maker (`0x480D40`) makes the object at the middle of the cell and, the object no creature and of the type 1162 (`0x4811F5`), sends it the FX event `0xA6` at its place on the ground (`0x636520`, 28 a floor) with no caster, no target and no array, and returns the object's followed creature with the fill's level, before its own course of the flags (`+0x1F4`), the stance and the hostility (`0x4812FD`).
- **The event `0xA6`** (`0x5A593F`): the object's `+0x48` takes the bits `0x20` (turned back rather than ended) and `0x100` (its own creature no target), `+0xA8` 1.5 (not read), its heading straight up; it makes a creature 1249 at its place, of the side 2 (3 for a player's caster), keeps it at `+0x68` and, without a caster, takes it for its caster.
- **The dummy 1249** draws nothing (`0x44B230` returns for it before its shadow and its model, `0x44B6F4`); returned past the maker's flags, it is hostile without the flag 1 and never wakes (the original's dummies stood at the AI's state 0 with the hero beside them, seen).
  Its death takes a branch of its own (`0x53ECA0`; **hypothesis**: it leaves no corpse).
- **The flight** is the lost souls' homing (`0x5A6680` under the flag `0x10000`): at the system's speed 120 (`+0x98`), its heading turned toward its aim by at most 3 radians a second; its target the nearest creature hostile to its dummy within a squared 100,000 (`0x600570`), kept while it lives and keeps its byte `+0x38` (**hypothesis**: its sector), aimed at its bone 3 (`Bip01 Neck`) else 0; without one it hovers, aiming 10 over its place at the speed 0 (`0x5A7F28`).
  A cell it may not walk on turns its heading's horizontal part about and puts it back where it was; the ground at or over its height turns its vertical part (`0x5A851D`).
- **Its strikes** (`0x5AB490`, `0x5ABD80`'s case at `0x5B17AB`): every creature hostile to its dummy within its size and theirs (`0x5FF7C0`, 50 each by default) takes, once a second (`+0x11C`), the strike's event from the object of `⌊dryadghost_dmg_k · (1 + v/100) · 9⌋` of each kind, held to 65,534, `v` the damage multipliers' curve at its dummy's skill 2 and 9 its first array word 0 plus 9: the formula of the dragons' fire on the ground ("The dragons' fire"), 18 physical and 22 fire at `v` 0.
- **Its look** is the lost souls' ("The lost souls", the draw `0x79CFB0` word for word): 200 dots of `PARTICLE_DOTS01` from a point wobbling by 5 about its place, one every 0.01 s, of a size of 7 shrinking by 4 a second, turning at 6 radians a second, living 0.64 s, their ramp (60, 60, 255) of 256 fading out, added by their alpha; and the green glow beating once a second at that point; its green Direct3D light; and `HELF_AHNEN_LOOP` (2058) looping from its start, as the souls' (docs/re/sound.md).
- **Its end**: at once where its target or its dummy changes its byte `+0x38`, or its place has no sector; fading where it has no target and its dummy is gone; with its sector going idle.

`opensacred-game` makes it since #1964 (`dryad_ghosts`, `opensacred_rules::particles::{DRYAD_GHOST, LostSoul::dryad_ghost}`, `opensacred_rules::fight::Rules::dryad_ghost_damages`): each fill's dummy 1249 is hidden as it is made, its flags' low 16 bits cleared, and its ghost rises on the ground at its place, flies, strikes the hero and its followers and looks and sounds as described (`lost_souls`, `hums::Hum`); it goes with its dummy, its dots fading after it.
The bytes `+0x38`, the strikes' resolution from an object, the dummy's death and the light are not followed.
**Seen** 2026-11-05 at the sector 16,79 beside the original: the dummies of neither game showing or stirring; in both a green glow trailing blue dots that rose and flew at the Seraphim within some six cells and circled about her head, striking her.

## The raised creature's ring

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The ring of a raised creature | 924 `FX_REANIMATE` | `cParticleSystem_reanimate` (`0x898A1C`) | `0x7AB500`, `0x7AB6F0`, `0x7AB590`, `0x7AB680` |

- Its maker: the black magicians' raising 43 (docs/re/arts.md, "The creatures' own spells"), an object 924 at the raised creature's place (the FX event `0x84`), standing there (its system's flags 0).
- Its constructor loads `PARTICLE_LINE01` (`+0x2530`).
- Its start: one layer of the life −100 a second (2.55 s) and the size −10; one block of the size 40 ± 10, one every 0.004 s at the detail 2 (1.5 and 2.2 times at 1 and 0); its ramp (100, 100, 256) of 256, from the alpha 0 to 1 over its first 10 entries, 1 over 200, to 0 over the last 46; its light 200.
- Its run: while its angle (`+0x20A8`) is under 2π, the block's offset is (25, 0, 0) turned by it about the vertical, and it emits there; the angle grows by 10 a second, so the point goes round once in some 0.63 s; then it ends with its last particle.
- Its draw: billboards of the flags `0x89` (the ramp's colour, standing on their feet, added by their alpha), the grid 1, the size factor 0.125: pale blue lines standing on a ring of 25 that fade where they were born.

`opensacred-game` draws it since #1092 (`reanimations`, `opensacred_rules::particles::Reanimate`) where `creature_casts` raises a creature, in place of the undead's rising ring that stood in for it, untinted by the night, lighting it by 200.
**Seen** 2026-11-05 in the game: a black magician 264 raised a dead 567 beside the Seraphim, and a ring of pale blue lines stood about it and faded in under two seconds.
Not set beside the original: its black magician under the driver slept under the trees out of the Seraphim's reach.

## The Dwarf's mines

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code unless marked otherwise) and seen in the running original under the driver of #448 (the Dwarf's new game, his spell 56 at the level 5).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| A mine | 1153 `FX_DWARF_MINE` | `cParticleSystem_dwarf_mine` (`0x898E0C`) | `0x7BAC30`, `0x7BB0B0`, `0x7BAD70`, `0x7BAF30` |
| The mines' sign | 1154 `FX_DWARF_MINE_SIGN` | `cParticleSystem_dwarf_mine_sign` (`0x898DEC`) | `0x7BA580`, `0x7BA8D0`, `0x7BA610`, `0x7BA830` |
| A blast | 794 `FX_EXPLOSION` | `cParticleSystem_explosion` (`0x897C7C`) | `0x779960`, `0x779F60`, `0x779A50`, `0x779B60` |

- **A mine** (1153), laid by the sign object's hits (`0x5B0719`) at its place on the ground with the FX event `0xA0`, standing there: its constructor loads `dwarf_mine.tga`, `dwarf_mine2.tga`, `particle_smoke04.tga` and the model `dwarf_mine.grn` (a squat dome); its start keeps its 30 s, a turn of `2π·U` and a flash's clock from 0; one layer of the life −200 a second (1.275 s) and the size +8; one block, a burst of 20 at its first run, of the buoyancy −35 ± 5 (rising ever faster), the size 6 ± 2, the angle π ± π, the velocity (0, 0, 15) ± (20, 20, 0), at 0 ± 5 on each axis; its ramp a grey (50, 50, 50) of 256 at the alpha 0.996, to 0.39 over 60 entries, then to (74, 56, 71) at 0 over 196; its flags `0x400`, no light.
  Its run: the flash's clock, past 1.5 (`0x8909A8`), goes back to −0.2, so the flash shows for 0.2 s in every 1.7 s, the first 1.5 s after it is laid.
  Its draw (`0x7BAF30`): the model turned by its turn about the vertical and scaled by 0.4 (`0x653790`, `0x6539D0`), in `dwarf_mine.tga` (a dark casing, a red button), lit as the world's models are (`0x405C90`); while the flash shows, the model again (`0x40D310`) in `dwarf_mine2.tga` (black but the button), white, unlit, added whole, the depth not written; then its puffs, billboards of the flags `0x4C` (the ramp's colour, turned, the light at the object), laid over by their alpha.
- **The sign** (1154), made by the spell (`0x528995`) at the Dwarf with the FX event `0x9F` (`0x5A57E6`: it follows him at his bone 0, `__Root`, its particles riding with him, its flags `0x2202`): its ramp from (17, 7, 228) of 256 at the alpha 1, to (51, 126, 203) at 0.79 over 96 entries, to (60, 141, 141) at 0.55 over 49, to (64, 64, 64) at 0.25 over 111; one layer of the life −200 a second and the size +8; its mines left and their all `0x4D26E0(0, 1, L)`.
  Its run (`0x7BA610`): an accumulator grows by `left · dt · 200 / all`, set back to 0 above 100, and while it is 1 or more a tongue of the size 4 ± 2 stands at a point at random along the arc `2π·(left/all)·U` of the circle of 25, at the turn `π/2 − a ± 0.1` so that it points out; each mine laid plays 2110 (`0.45 + 0.1·U`), and at the last the sign goes at once.
  Its draw (`0x7BA830`): billboards of `PARTICLE_FLOOP01` by the identity matrix, so that they lie flat on the ground, of the flags `0x8D` (the ramp's colour, turned, standing on their feet, added by their alpha), the culling off: blue tongues spread about his feet, their wide part at the circle and their tip out.
- **A mine goes off** (its run `0x7BAD70`) past its 30 s, or as the hits' case (`0x5B096B`) marks it, a hostile creature touching it while the Dwarf is 100 or more off: it asks for the action 1 for the type 794 with the data `0x80000064`, shakes the screen by 21 (`cEventWeather` of the kind 7) and plays 2111, and goes.
  The action (`0x5B2600`'s case `0x31A`, `0x5B2895`) makes the 794 at its place on the ground, of the variant 100 (`+0x38`) and the event's `+0x3C` 0, the data's top bit set; the Dark Elf's bomb trap's ball makes it of the data 100 (`0x79E730`), `+0x3C` 1, with the same quake.
- **The blast** (794), of the scale `s` 1 for both (the variant over 100):
  - its constructor loads `PARTICLE_FIRE01`, `PARTICLE_FIRE04`, `Fx_ExplosionCircle01` and, by the low bit of a `rand()`, `FX_STREAKS02` (even) or `FX_STREAKS01`;
  - its start: the scorch's turn `2π·U` and its alpha 1, the ring's radius 10; one layer of the life −300 a second (0.85 s) and the size +20; one block of the size 10 ± 5, the angle π ± π, at (0, 0, 10) ± 20 on each axis, one every 0.02 s at the detail 2; its flags `0x400` while it burns, no light;
  - its run: for 0.8 s it makes flames and strikes (each creature once, docs/re/arts.md); then its flags are 0; past 10 s its scorch fades by 0.2 a second, and it ends at 0, some 15 s in all; its ring grows by 50 a second;
  - its draw: the scorch, a flat quad of a half side 50 turned by its turn on the ground, white at its alpha, laid over by its alpha (black with an alpha mask: a dark star of streaks); the trap's alone, while its ring is under 50, a flat ring from its radius to 20 beyond of `Fx_ExplosionCircle01`, 15 segments, added whole; then the flames, billboards of the flags 7 (white at the alpha of their life, turned, added) and the grid 4, of `FIRE01` for the mine's and `FIRE04` for the trap's, their frames played once over their life.
  It plays no sound of its own.

`opensacred-game` draws the blast since #1092 (`explosions`, `opensacred_rules::particles::Explosion`): a mine's and the bomb trap's, each with its quake, the flames untinted by the night, the scorch on the ground with the world as the meteors' craters; the blast still strikes once at its start (`spells`).
**Seen** 2026-11-05 beside the original, the Dwarf's mines going off after their 30 s: in both a white and orange fireball over each mine, and a dark round scorch left on the ground where it lay.
Since #1092 the mines and their sign look as the original's (`mines`, `opensacred_rules::particles::{DwarfMine, MineSign}`): each mine its model turned at random, its button's flash in copies of its meshes added whole, its puffs darkened by the night; the sign its tongues on the ground about the Dwarf along the arc of his mines left, untinted.
**Seen** 2026-11-05 beside the original, the Dwarf laying his mines as he walks: in both a small dark dome with a red button lying where each was laid, and in the game blue tongues spread about his feet.

## The flying skull

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code) and seen in the running original under the driver of #448 (the Vampiress in her night form, her 1033 at the level 5 on a goblin).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The death's kiss's skull | 905 `FX_FLYINGSKULL` | `cParticleSystem_flyingskull` (`0x898884`) | `0x7A5DE0`, `0x7A6370`, `0x7A5F40`, `0x7A6040` |

- Its constructor loads `PARTICLE_GLOW03` (`+0x2534`) and the model `skull.grn` (`+0x2538`, `+0x253C`), and makes a Direct3D light of the colour (1.0, 0.8, 0.1).
- Its start: its seconds the event's `+0x4C` (`+0x20A8`); the loop 2058 as the souls' (docs/re/sound.md); one layer of the life −300 a second (0.85 s) and the size −15; one block of the size 18 ± 2, one every 0.05 s at the detail 2; its ramp from nothing to (0.996, 0.852, 0.051) over 10 entries, to (0.879, 0.383, 0.160) at the alpha 0.781 over 126, to nothing over 120; its flags `0x10000` (the homing flight, "The lost souls") and its speed 150.
- Its run: while it flies, the block's offset wobbles as a soul's (`5 sin(6.58 t)`, `5 sin(6.87 t)`, `5 sin(7.49 t)`) and it emits along its way; past its seconds, or its object's word `+0x74` bit 2, it stops (`+0x20AC` 1, its flags 0) and ends with its last glow.
- Its draw: while it flies, its light at its place, then its model at twice its size, turned about the vertical by `atan2(−y, x) + π/2` of its heading, at its place and the wobble, drawn by `0x406510` as the world's models; then its glows, billboards of the flags `0x19` (the ramp's colour added whole), the grid 1.

`opensacred-game` draws it since #1092 (`flying_skulls`, `opensacred_rules::particles::{FlyingSkull, skull_turn}`): each skull of the death's kiss shows its model turned to its heading and its trail, untinted by the night; its Direct3D light follows since #2106.
**Seen** 2026-11-05 beside the original: in both a small red skull at the head of a trail of orange glows flying from the Vampiress at the goblin; the game's trail shorter and wider.

## The thorns' plants

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code) and seen in the running original under the driver of #448 (the Wood Elf's 39 at the level 5 on a goblin).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| A thorn of the spell 39, the hero's or a creature's | 848 `FX_KILLERPLANTS` | `cParticleSystem_killerplants` (`0x898474`) | `0x798820`, `0x799640`, `0x798E50`, `0x798F10` |

The class makes no particle and no model: its plant is drawn of its own geometry.
- Its constructor loads `PARTICLE_GLOW01` (`+0x3560`) and `PARTICLE_GLOW03` (`+0x3564`), makes a Direct3D point light of the colour (0.2, 1, 0.2), the range 200 and the attenuations 1, 0 and 0.000125, and rolls the plant's shape, `U` a share `rand() / 32767`:
  - first its seven spurs' points (`+0x34C4`), the `m`-th (0 to 6) `⌊128 m / 7⌋ + rand() % 7`, at least 1;
  - then its stem of 128 points (`+0x20C4`) of five rolls `U0` to `U4`: the point `k` at `sin(0.1 k (U2 + 0.5))·(k/128)·(2 U1 + 4.5)` along x, `sin(0.1 k (U4 + 0.7))·(k/128)·(2 U3 + 6.5)` along y and `0.5 k` up, turned by `2π U0` about the vertical;
  - its widths (`+0x32C4`), `1 + (128 − k)/64`, 3 at its foot to about 1 at its tip;
  - and its two sways of five rolls each, `V` (`+0x26C4`) `sin(0.1 k (0.8 V2 + 0.2))·(3 V1 + 3.5)` and `sin(0.1 k (0.4 V4 + 0.1))·(3 V3 + 6)` turned by `2π V0`, and `W` (`+0x2CC4`) `sin(0.1 k (W2 + 0.5))·(3 W1 + 2.5)` and `sin(0.1 k (W4 + 1))·(2 W3 + 7)` turned by `2π W0`, each rolled for `k` 0 to 63 and mirrored for 64 to 127 (the entry `64 + i` the entry `63 − i`), a loop.
- Its start: its clock 0, its rise (`+0x20A8`) −3; a player's plant (the system's `+0xA9` 0, which the FX handler `0x5A3A7B` sets to 1 or 2 for an owner of an id above 16) green, `0xFF40FF40`, its light (0.25, 1, 0.25), standing the record's value 1 at the level in seconds; another's blue, `0xFF4040FF`, its light (0.25, 0.25, 1), standing 3 s.
- Its run: its rise grows by 3 a second to 0, it stands its seconds, and its rise falls by 3 a second until it ends below −3, striking all the while (docs/re/arts.md).
- Its draw, its clock `t`, `a = ⌊30 t⌋` and `b = ⌊27 t⌋`:
  - its light at its place;
  - a glow of `PARTICLE_GLOW03` flat on the ground at its place (`0x7631E0`), of the half side `5·(rise + 3)`, 15 risen, in its colour, added by its alpha (`SRCALPHA`, `ONE`);
  - its vine: for each `k` the point `stem[k] + (k/128)·(V[(k − a) mod 128] + W[(k − b) mod 128])` from its place, a square of `PARTICLE_GLOW01` facing the camera (**hypothesis**: the display matrix with the point's translation), of the half side `width[k] + rise` where that is above 0, in its colour, added by its alpha, unlit, no depth written;
  - at each spur's point so drawn while its rise is above −1, squares along the vine's way from the point before, `0.75 n` on from it for `n` 1, 2, …, of the half side `1.5 + rise − 0.075 (n − 1)` while above 0.5, 14 risen.
- So a twisting vine of soft dots stands 64 high, thick at its foot, as two waves climb it at 15 and 13.5 units a second, seven straight spurs out along its bends; it grows from its foot up over a second and sinks from its tip down.

`opensacred-game` draws it since #1092 (`killer_plants`, `opensacred_rules::particles::{KillerPlant, Vine}`): each thorn of `spells` grows its plant at its standing height, its dots and its glow added, untinted by the night; its Direct3D light, which tints the creatures about it green, follows since #2106.
**Seen** 2026-11-05 beside the original: in both a green vine of white-hot dots with its spurs over a green glow on the ground.

## The tentacles

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code) and seen in the running original under the driver of #448 (the Daemon's 65 at the level 5, a goblin killed under it).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The spell 65's aura about the Daemon (the variant 0) and a tentacle at a corpse (the variant 1) | 1013 `FX_DAYOFTHETENTACLE` | `cParticleSystem_dayofthetentacle` (`0x898BB0`) | `0x7B0DF0`, `0x7B1990`, `0x7B1480`, `0x7B14F0` |

- The spell's executor (`0x52829F`) makes the aura at the Daemon, the FX event `0x8E` of the variant 0, which follows his bone 0 (`__Root`, the system's flags `0x200`); his kill under the effect 41 (`0x53382F`) bursts the corpse and makes a tentacle at its place, the variant 1 (docs/re/arts.md).
- Its constructor makes no particle of use (its 150 never emit), loads `particle_glow01` (`+0x7DB4`) and 50 runes (`+0x7CEC`), each `fx_tentacle_rune<n>` of `n = rand() % 15 + 1`, and rolls five tendrils: each a stem and two sways of the thorns' plant's forms and order ("The thorns' plants"), and a foot's offset `o` (`+0x7CB0`) of `80 U − 40` along y, then along x; the widths (`+0x20B0`) `1 + (128 − k)/51.2`, 3.5 at the foot to about 1.
- Its start: the variant the event's `+0x38`; the aura for the record's value 0 at the level in seconds, giving a creature that casts it the effect 41 for as long, and silent; the tentacle for its value 1, 10 s, striking (the flags `0x400`) and looping `DEMON_TENTAKEL_LOOP` (docs/re/sound.md).
- Its run: past its seconds its sink (`+0x20AC`) falls from 0 by 3 a second, and it ends below −3.
- Its rings (`0x7638E0`, read here): flat on the ground at its place, turned about the vertical by its turn, each segment `i` of `n` a quad between the angles `(i − 1)·2π/n` and `i·2π/n` at `(sin θ, cos θ)` times its inner and outer radii, of the object's `i`-th rune, its `u` along it and its `v` from the outer edge in, in its colour, blended by its alpha (`SRCALPHA`, `INVSRCALPHA`) unless added, no depth written.
- Its draw of the variant 0: four rings, black and opaque, whole until it ends:

| Ring | Its radii | Its segments | Its turn, radians a second |
|---|---|---|---|
| 1 | 25 to 32 | 17 | −0.87388 |
| 2 | 30 to 39 | 22 | 0.78345 |
| 3 | 37 to 46 | 27 | −0.54321 |
| 4 | 44 to 53 | 32 | 0.37646 |

- Its draw of the variant 1, `g = (sink + 3)/3`: two black rings, 25 g to 32 g of 17 segments turning at 0.58463 radians a second and 30 g to 39 g of 17 turning at −0.75235; then for each tendril and each `k` the point `(k/128)·(stem[k] + V[(k − ⌊30 t⌋) mod 128] + W[(k − ⌊27 t⌋) mod 128]) + (k/128 + 0.2)·o` from its place, a black square of `particle_glow01` facing the camera, of the half side `width[k] + sink` where that is above 0, blended by its alpha.
- So a tentacle is five dark tendrils writhing up to 63 high from within 8 of the corpse, leaning out to within 48, over two rings of black runes turning against each other, all shrinking away over its last second; the aura is four such rings about the Daemon's feet (**hypothesis**: his root bone at his feet).

`opensacred-game` draws both since #1092 (`tentacles`, `opensacred_rules::particles::{Tentacle, TentacleAura, RuneRing}`): each tentacle of `spells` at its corpse, and the aura that `spells` makes as he casts 65, following him for its seconds and its sink's second, laid over the scene in the world's camera.
**Seen** 2026-11-05 beside the original: in both a ring of black runes about the Daemon's feet, and at the burst goblin's place five black tendrils over their runes.

## The thrown balls

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code) and seen in the running original under the driver of #448 (the Dark Elf's poison trap 1052 at the level 5 on a goblin).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The ball of the Dark Elf's traps 1049 to 1053 and of the goblin mages' spell 53 | 867 `FX_BALL` | `cParticleSystem_ball` (`0x89860C`) | `0x79E630`, `0x79EA50`, `0x79E730`, `0x79E900` |

- Its constructor (100 particles, the FX handler's `0x5A1F07`) loads the model `bottle01.grn` (`+0x2150`) and `PARTICLE_GLOW03` (`+0x2158`).
- Its start: its variant the event's `+0x3C`, the art or the spell that throws it, which gives its glows' colour (`+0x20AC` … `+0x20B8`, its four corners'): 1049 and 1050 (128, 240, 255), 1051 (160, 160, 160), 1052 (32, 255, 32), 1053 (192, 192, 192), and 53 white (its flags `0x80108`, the others' `0x108`); `DELF_WURFCAST` (2057) at its object; two turns' rates (`+0x2148`, `+0x214C`) of `2·(1 + U)` radians a second each; one layer of the life −350 a second (0.73 s) and the size −30; one block of the size 7 ± 2, the buoyancy 0 ± 5 and the place ± 3 along each axis, one every 0.005 s at the detail 2 (0.0075 at 1, 0.011 at 0).
- Its run: its glows age (`0x7640C0`, a glow dying at a size or a life of 0), and while it flies it leaves new ones along its way (`0x764760`); its turns grow by their rates; its object's word `+0x74` bit 2, as it lands, ends its emission and makes its trap (its action, `+0x80` 1, of the type `+0x84`: 1049 the explosion 794 with its `DELF_SPRENGSATZ` (2056) and a quake of 21, 1050 the pentagram 782, 1051 the confusion 871, 1052 the ground's poison 792, 1053 the battle fog 868; docs/re/arts.md), and it ends with its last glow.
- Its draw: its model at its place, turned about the models' x (`0x653910`) and then y (`0x653850`) by its two turns, drawn as the world's models; then its glows, billboards of the flags 3 (`0x761B00`: added by their alpha, `ONE` the destination, their corners' colour times their life's share), the grid 1, the factor 1.
- So a small bottle tumbles along the ball's arc (docs/re/arts.md, #2112) trailing a streak of glows in its trap's colour, its head brightest and its tail thinning to dots.

`opensacred-game` draws it since #1092 (`trap_balls`, `opensacred_rules::particles::{TrapBall, ball_colour}`): each ball of `spells` and of `creature_casts` shows its bottle tumbling and its glows, added and untinted by the night.
**Seen** 2026-11-05 beside the original: in both a green streak thick at its head and dotted at its tail arcing from the Dark Elf's hand over her head to the goblin.

## The demon fire

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code) and seen in the running original under the driver of #448 (the Daemon's 64 at the level 5 by a goblin).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The hellfire of the spell 64 | 1015 `FX_DEMONFIRE` | `cParticleSystem_demonfire` (`0x898C14`) | `0x7B2B00`, `0x7B3110`, `0x7B2CA0`, `0x7B2DF0` |

- The executor (`0x528566`) makes it at the action's point, at the ground's height there (`0x636310`); the FX event `0x90` places it there, and it follows nothing.
- Its constructor (500 particles) loads `PARTICLE_LINE01` (`+0x2538`), the lava's 50 frames `A_LAVA00` to `A_LAVA49` (`+0x253C`), the model `sphere.grn` (`+0x2604`, a sphere of the radius 20, no motion) and a Direct3D point light of (1.0, 0.4, 0.2), the range 200 and the attenuations 1, 0 and 0.000125.
- Its start: its seconds (`+0x24B0`) the record's value 0 at the level times the event's factor; its growth `g` (`+0x20A8`) 0; one layer of the life −200 a second (1.275 s) and the size −25; one block of the size 30 ± 7, the angle π ± π, the place ± 1 along each axis, one every 0.004 s at the detail 2; its ramp from nothing in (0.922, 0.363, 0.035) to its full alpha 0.996 over 76 entries, to (0.996, 0.617, 0.25) over 66, to nothing in (0.566, 0.371, 0.184) over 114; its flags 2 (its particles kept about its place); its light 250; `DEMON_HOELLENFEUER` looping (docs/re/sound.md).
- Its run (#2114): it grows `g` by 0.5 a second to 1 over 2 s, then sets its flag `0x2000`, which lets it strike; it stays until its clock passes its seconds, making rays all the while; then it clears the flag and shrinks `g` by 0.5 a second to 0, making no rays, and ends.
- Its draw, its clock `t`:
  - its block's place and its ball's centre `(0, 0, 45 + 3 sin(1.17 t))` over its place, its light there;
  - an inner sphere of the size `(0.3 + 0.05 sin t)·g`, turned about the vertical by `t` radians, in the lava's frame `⌊fmod(70 t, 49.5)⌋`, lit and its depth written, its shadow under it (`0x406FF0`);
  - an outer sphere of the size `(0.35 + 0.05 sin t)·g`, turned by `1.2456 t`, in the same frame, white, unlit, added (`ONE`, `ONE`) and its depth not written (`0x40D310`);
  - its rays, billboards of `PARTICLE_LINE01` of the flags `0x8D` (`0x761B00`: standing on their feet, turned, the ramp's colour, added by their alpha), the grid 1, the factor 0.125: each `0.125·size` to each side and `2·size` long from its foot at the ball's centre, at its angle in the screen's plane, so that thin rays up to about 74 long stand out about the ball, their feet hidden within the inner sphere, whose depth they test.
- So a ball of flowing lava bobs about 45 over the point in a starburst of thin orange rays, growing in over two seconds and shrinking away over its last two; its strikes' beam is not its own (#2119).

`opensacred-game` draws it since #1092 (`demon_fires`, `opensacred_rules::particles::{DemonFire, lava_frame}`): each demon fire's object of `spells` shows its two spheres in the lava's frames, the outer added, and its rays added and untinted by the night, their feet within the inner sphere left out (`Quads::push_ray`), as its depth hides them in the original; its shadow is not followed, its Direct3D light follows since #2106.
**Seen** 2026-11-05 beside the original: in both an orange lava ball in a starburst of thin rays over the point.

## The hell disk

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code) and seen in the running original under the driver of #448 (the Daemon's 60 at the level 5 by a goblin).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The hell disk of the spell 60 | 1005 `FX_HELLDISK` | `cParticleSystem_helldisk` (`0x898A8C`) | `0x7AD8C0`, `0x7AE040`, `0x7ADB10`, `0x7ADBC0` |

- Its constructor (300 particles) loads `fx_multiTexGlow01` (`+0x2534`, which nothing of it draws), `particle_glow03` (`+0x2538`), the lava's 50 frames `A_LAVA00` to `A_LAVA49` (`+0x253C`), the model `hell_disk.grn` (`+0x2604`: three rings, a flat one and two tilted against each other) and its motion of the slot 1 as a controlled animation from the time 0 (`+0x260C`, `+0x2614`; **hypothesis**: the converted clip `FX_F_IDLE_BH` of 1.32 s, looping for ever by its parameter 3 of the largest float), and a Direct3D point light of (1.0, 0.4, 0.2), the range 200 and the attenuations 1, 0 and 0.000125.
- Its start: its seconds (`+0x20A8`) the record's value 0 at the level times the event's factor; one layer of the life −200 a second (1.275 s) and the size −4; one block of the size 10, one every 0.015 s at the detail 2; its ramp of (0.996, 0.391, 0.234) throughout, its alpha 0 for 20 entries, up to 0.781 over 30 and down to 0 over 206, so that a glow shows from 0.1 s after its birth, brightest at 0.25 s; its flags `0x10000` (its flight, docs/re/arts.md, #2115), its speed 250, its light 250; `DEMON_HOELLENDISK_LOOP` looping (docs/re/sound.md).
- Its run: while it flies it leaves glows at its place, so that its trail lies along its way; past its seconds, or its object's word `+0x74` bit 2, it stops (`+0x20AC` 1, its flags 0) where it is, still drawn, and ends with its last glow.
- Its draw, its clock `t`:
  - its light at its place;
  - its model at 0.08 of its size, not turned, at its place, in the lava's frame `⌊fmod(100 t, 49.5)⌋`, lit, blended by its alpha and its depth written (`0x405C90`), its shadow under it (`0x406FF0`);
  - its model again in the frame `⌊fmod(100 t + 50, 49.5)⌋`, half a frame on, white, unlit, added (`ONE`, `ONE`) and its depth not written (`0x40D310`);
  - a glow of the style 5 (`0x762E10`: `PARTICLE_GLOW03` added whole) at its place, of the half side `30 + 10·(1 + sin t)`, in (176, 113, 81);
  - its trail, billboards of the flags `0x0D` (`0x761B00`: the ramp's colour, added by their alpha), the grid 1, the factor 1.
- So a small gyroscope of flowing lava whirls along its flight in a pulsing dull orange glow, trailing orange glows that light up a little behind it and fade within 1.3 s.

`opensacred-game` draws it since #1092 (`hell_disks`, `opensacred_rules::particles::HellDisk`): each hell disk of `spells` shows its model at 0.08 of its size playing its clip's loop, in the lava's frames, lit in its own material and again added over itself on copies of its meshes, and its glow and its trail added and untinted by the night; stopped, it stays where its disk ended until its last glow; its shadow is not followed, its Direct3D light follows since #2106.
**Seen** 2026-11-05 beside the original: in both a whirling ring of lava at the goblin's neck in a bright orange glow, a curving trail of orange glows behind it.

## The traps' landings

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code) and seen in the running original under the driver of #448 (the Dark Elf's traps 1050, 1051 and 1052 at the level 5 by goblins).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The pentagram of the trap 1050 | 782 `FX_PENTAGRAMMKILLER` | `cParticleSystem_pentagrammKiller` (`0x897AA4`) | `0x7727E0`, `0x773790`, `0x772C40`, `0x773020` |
| The confusion of the trap 1051 | 871 `FX_CONFUSION` | `cParticleSystem_confusion` (`0x897C3C`) | `0x778E80`, `0x7792C0`, `0x778F40`, `0x779020` |
| The ground's poison of the trap 1052 | 792 `FX_GROUND_POISON` | `cParticleSystem_groundPoison` (`0x897C1C`) | `0x778760`, `0x778D10`, `0x7788F0`, `0x778980` |

The trap's ball makes them where it lands ("The thrown balls"); their strikes are docs/re/arts.md's.
The spells' executor still makes 782 for the spell 6 and 792 for the spell 12 (`0x5238EF`, `0x523B19`), which no hero holds (their class masks 0, their runes missing) and a creature casts only as a script's spell; only the trap makes 871.

**The pentagram.**
- Its constructor (300 particles) loads `PARTICLE_GLOW03`, `PARTICLE_SPARK02` and `PARTICLE_CLOUD01`, a spark's quad of the half side 4 in (128, 240, 255) at the alpha 96, and its puffs' ramp from white to (1, 0.5, 0.2) over 60 entries and to nothing in (1, 0.2, 0) over 196.
- Its start: its seconds the record's value 0 at the level; `MAGE_PENTAGRAMM_CAST` (docs/re/sound.md).
- Its run: its turn grows by a radian a second, its star's points at the turn and `4π/5`, `8π/5`, `2π/5` and `6π/5` on, on the radius 40; arming, its fade grows by 0.5 a second, and at 1, 2 s on, it strikes (its flags `0x400`) and loops `MAGE_PENTAGRAMM_LOOP`; burst (its object's word `+0x74` bit 2), it plays `MAGE_PENTAGRAMM_HIT`, its radius falls from 40 to 0 over a second while it makes puffs, one every 0.003 s at the detail 2, of the size 30, at random angles, the velocity (0 ± 15, 0 ± 15, 350 ± 150) and the place ± 10 across, their life falling by 650 a second, and it ends with its last; past its seconds unburst its fade falls by 0.5 a second and it ends at 0.
- Its draw: a glow of `PARTICLE_GLOW03` flat on the ground at its place (`0x7631E0`), of the half side 70 in (96, 192, 207), added whole; its sparks of `PARTICLE_SPARK02` facing the camera (**hypothesis**: the display matrix), added by their alpha and each laid anew at random each frame: `⌊4·f·r⌋` on its ring within the share `f` of its turn that its fade draws, two comets of `⌊r⌋` each armed, from `1 − turn` and π on, spreading over `i/30` radians, and `⌊2·f·r⌋` along each of its star's five sides, `r` its radius; its puffs, billboards of the flags `0x0D` (the ramp's colour, turned, added by their alpha).

**The confusion.**
- Its constructor loads `FX_MAGICCIRCLE01`, `FX_MAGICCIRCLE02` and `FX_HELIX01`; its start its seconds, its flags `0x400`, `DELF_VERWIRRUNG_LOOP`, its helix's turn `θ₀` at random.
- Its run: past its seconds it ends at once; its helix's phase grows by 0.15 a second, at 1 back to 0 on a turn rolled anew, its alpha else `⌊255·sin(π·phase)⌋`; its small circle's height grows by itself times the frame's time (so faster at higher rates of frames), back to 1 past 100.
- Its draw: a circle of `FX_MAGICCIRCLE01` flat on the ground, of the half side 70, white, turned by its clock; one of `FX_MAGICCIRCLE02` of the half side 17, turned by `−5` times its clock, at its height over it, both added whole; its helix (`0x762C30`) of `FX_HELIX01` in (13, 253, 228) at its alpha, added by its alpha: 50 points at `θ₀ − 0.3·(k + j)` and `0.5·(k + j)` high on the radius 30, `k = 100·phase`, a band 4 across flat and one 4 high upright.

**The ground's poison.**
- Its constructor (200 particles) loads `FX_GROUNDPOISON`, `PARTICLE_SMOKE03` and `FX_MAGICCIRCLE03`, its ramp from (0.5, 1, 0.5) to (0.2, 1, 0.2) over 128 entries and to black over 128, opaque, and a Direct3D point light of (0.3, 1, 0.3), the range 200 and the attenuations 1, 0 and 0.000125; its start its seconds, its flags `0x400`, its light 280, `DELF_GIFTHAUCH_LOOP`; one layer of the buoyancy's rate −5, the life −200 a second and the size −2; one block of the buoyancy −100, the size 18, the place ± 15 across and a frame below 4, one every 0.04 s at the detail 2.
- Its run: past its seconds it ends at once; its puffs rise and age, and it makes new ones.
- Its draw: its light; for a player's trap (the system's `+0xA9` 0) a circle of `FX_MAGICCIRCLE03` flat on the ground, of the half side 65, in (160, 160, 160), turned by 0.2 times its clock, added whole; a stain of `FX_GROUNDPOISON` flat over it, of the half side 70, laid by its alpha; its puffs, billboards of `PARTICLE_SMOKE03` of the flags `0x109` (the ramp's colour, added, the frame of the grid of 2 by their byte).

`opensacred-game` draws them since #1092 (`trap_landings`, `opensacred_rules::particles::{PentagramKiller, ConfusionCircle, GroundPoison, helix_ribbons}`): each landing of `spells` takes its look, the glows, the sparks, the puffs, the helix and the smoke added and untinted by the night, the poison's stain in the world's camera; the pentagram's look outlives its object while it bursts or fades out; their flat parts lie level over the ground, not hidden by it where it rises, and the poison's Direct3D light follows since #2106.
**Seen** 2026-11-05 beside the original: in both the poison's green stain over its turning circle in green smoke, the confusion's turquoise rune circle with its small purple one, and the pentagram's bluish ring and star of sparks drawing themselves.

## The laser beams

Read 2026-11-05 for #1092 and #2119 in Ghidra (**confirmed** by code), and the hellfire's seen in the running original under the driver of #448 (the Daemon's 64 at the level 5 by a goblin).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| A segment of a laser beam | 901 `FX_LASERBEAM` | `cParticleSystem_laserbeam` (`0x8987CC`) | `0x7A3790`, `0x7A50B0`, `0x7A3DF0`, `0x7A4D50` |

- `createLaserbeam` (`0x604030`, on the objects' manager) lays a beam from a point to another: `n = ⌊d / 53.66563⌋` segments, at least 1, `d` the ground distance of the ends' whole places; for `k` below `n` it tests the cell at the start's place and `k·(Δ/n)` on (a whole division) with `0x636A60`, which passes a cell that may be walked on (`0x636C10`) or failing that one of the low nibble 2, and at the first that fails it stops, returning false unless the flags' bit `0x80` goes on past it; else it makes an object 901 at that cell and starts it by the FX event `0x72`, its point `from + k·(to − from)/n`, its direction `(to − from)/n`, its variant (`+0x38`, the flags' low seven bits) and `k | n << 16` (`+0x3C`); the event's case puts it at its point, and it follows nothing.
- Its callers: the magic skeleton's drain (`0x533029`, the variant 2, from its killer's `Bip01 Spine2` to its own); the hellfire's strike (`0x5B0259`, 1, from the demon fire's ball to its target's `Bip01 Neck`, else its root), which a false skips; the big demons' eye lasers (`0x527392`, `0x52741C`, 1, from `Left_EyeBone_FX` and `Right_EyeBone_FX` to the target's neck), which strike where either reaches; the Vampiress's bite 1030 (`0x510140`, 0, from her spine to its target's), which a false skips; the `Gehirnfüßler` 203, 351 and 352 (`0x4D4E50`, 10 to 12 with `0x80`, from its spine to a bone of its target's at random); the strike handler's three health's transfers (`0x56B130`, 2) and the network's copies; the zhadum 1165 (`0x5B2263`, 5).
- Its constructor makes two sets of two crossed quads along its `x` from 0 to 1, one across of ±5 in `y` and one upright of ±5 in `z`, half grey, the texture's `u` along it, and loads `FX_VAMPBLOODLINE`, `LASERBEAMPART`, `FX_LIGHTSTRIKE05`, `LASERBEAMPART3` and `LASERBEAMPART4`; no particle, no Direct3D light.
- Its start: its fade 1; its world matrix turned about `(0, −n.z, n.y)` by `acos(n.x)`, so that its `x` lies along its direction `n`; its quads' far ends at its length; its light 80; by its variant, its hold and the fall of its fade a second, and a sound at each segment as it starts:

| Variant | Its hold | Its fade's fall | Its life | Its sound |
|---|---|---|---|---|
| 0, the bite's | 4.4 s | 20 | 4.45 s | `VAMP_BLUTBISS` (2076) |
| 1, the lasers' | 0 | 2 | 0.5 s | `SPELL_LASER` (2074) |
| 3 | 2 s | 1 | 3 s | none |
| 4 | 0 | 2 | 0.5 s | none |
| 5, the zhadum's | 1 s | 1.5 | 1.67 s | none |
| 2, the drains', and the others | 0 | 1 | 1 s | none |

- Its run: past its hold its fade falls by its rate, and below 0 it ends; by its variant:
  - 0, 2 and the others: its quads 20 wide (±10), a quarter of the texture's height, stepping 0, ¼, ½ and ¾ every 0.1 s, all segments in step; each joint `j` of the chain (`k` at its start, `k + 1` at its end) swayed across by `(4 cos(1.45634 j + 3t + 1.78356) + 3 cos(2.85261 j + 5.895635 t))·sin(jπ/n)` and up by `(4 cos(0.96452 j + 3t + 0.74372) + 3 cos(2.57838 j + 5.83623 t))·sin(jπ/n)`, so that the chain sways most at its middle and keeps its ends; its colour `⌊255 f⌋` halved, grey, else (127, 50, 50) for 10, (50, 50, 127) for 11 and (127, 127, 50) for 12, times its fade;
  - 1 and 4: straight, 30 wide (±15), the whole texture, (255, 146, 39) and (255, 235, 235) times its fade;
  - 3: the top eighth of `FX_LIGHTSTRIKE05`, 20 wide, half grey, jittering each frame; 5: both sets 70 wide, `LASERBEAMPART3` and `LASERBEAMPART4` flowing along at 0.44 and 0.78 a second, white.
- Its draw: depth tested, its depth not written, unlit, added whole (`ONE`, `ONE`), its two crossed quads in its world matrix at its place, not facing the camera, not tinted by the night: `FX_VAMPBLOODLINE` for 0, 2 and the others, `LASERBEAMPART` for 1 and 4, `FX_LIGHTSTRIKE05` for 3, `LASERBEAMPART3` and `LASERBEAMPART4` for 5.
- So the hellfire's and the lasers' are straight yellow-orange streaks gone in half a second, the drains' swaying chains of red strands fading over a second, the bite's the same whole for 4.4 s.

`opensacred-game` lays them since #1092 (`beams`, `opensacred_rules::particles::{LaserSegment, laser_axes}`, `opensacred_rules::ways::laser_places`): the drains', the hellfire's, the eye lasers' and the bite's, each segment staying where it was laid with its quads, its band, its sway and its colour by its variant, and its sound; the line's test of the strikes is #2123's; the segments' lights of 80, the `Gehirnfüßler`'s, the strike handler's and the zhadum's beams are not followed.
**Seen** 2026-11-05 beside the original: in both the hellfire's yellow-orange streak from its ball to the goblin's neck at a strike; the drain's chain of red strands from the Seraphim to the skeleton in the game.

## The homing fires

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The big demons' fire, the creatures' spell 50 | 910 `FX_DAEMONFIRE` | `cParticleSystem_daemonfire` (`0x8988C4`) | `0x7A6A40`, `0x7A6F50`, `0x7A6AD0`, `0x7A6D30` |
| Anducar's guards' fire, the spell 70 | 1164 `FX_ANDUCAR_GARDE` | `cParticleSystem_anducar_garde` (`0x898F38`) | `0x7BF690`, `0x7BFA90`, `0x7BF720`, `0x7BFA20` |

The two run one code; their FX events (`0x7A`, `0xA8`) follow the caster at its bone 14 (`Bip01 R Finger31`), the flag `0x200` putting the system there at each frame while it charges.
- Its constructor (300 particles) loads `PARTICLE_FIRE03`; no Direct3D light.
- Its start: its charge 2.5 s (910) or 0.9 s (1164); one layer of the life −300 a second (0.85 s) and the size +15 (910) or +10 (1164); one block of the buoyancy −35 ± 34 (rising), the angle π ± π, the velocity ± 0.5, the size 12 ± 1.5 within 15 (910) or 6 ± 0.5 within 5 (1164), one every 0.006 s at the detail 2; its ramp from nothing to a pale yellow (0.996, 0.953, 0.516) at the alpha 0.352 over 5 entries, to (0.996, 0.313, 0.156) at 0.996 over 40, to (0.910, 0.262, 0.078) at 0.898 over 29, to black at 0.547 over 85, to nothing over 97 (a second ramp, layer and block filled and never used); its light 250; 1164 plays `TORCH_HIT01` (708).
- Its run: the renderer's wind in its layer; charging, it makes flames; at its charge's end the homing flight (its flags `0x10001`, its speed 250, docs/re/arts.md), and 1164 plays `SPELL_FIRESTRIKE` (2091); 5 s on it stops and burns out; at its contact or a cell that stops it (its word `+0x74` bit 2), it stops, its layers set to the life −200 and the size −15, and bursts all its dead flames at once within 15 of it, flying out at up to 25 along each axis, and 1164 plays `MAGE_FEUERBALL_HIT` (2001).
- Its draw: billboards of the flags `0x0D` (the ramp's colour, turned, added by their alpha), the grid 1, untinted; the additive blend leaves the ramp's black part unseen.
- So a flickering fire burns in the caster's right hand as it charges, trails its flames behind it as it homes, and bursts in a ball of up to 300 flames gone within a second.

`opensacred-game` draws them since #1092 (`homing_fires`, `opensacred_rules::particles::{HomingFlames, HomingKind}`): each homing fire of `creature_casts` charges at its caster's `Bip01 R Finger31`, leaves its flames in the world along its way and bursts where it strikes the hero or a cell stops it, its flames added and untinted by the night; the demons' flames riding with their caster (the flag 4) are left in the world as the guards' (**estimate**: how its riding origin carries into the flight is not read).
**Seen** 2026-11-05 in the game: an Anducar's guard 184 of the level 20 striking the Seraphim with its fire, a ball of orange flames bursting at her.

## The whirlwind and the fire bolt

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The Mage's whirlwind, the spell 29 | 831 `FX_WINDSTRIKE` | `cParticleSystem_windstrike` (`0x898134`) | `0x789E10`, `0x78A170`, `0x789EA0`, `0x789F50` |
| The fire bolt, the creatures' spell 52 | 922 `FX_SHADDAR_MAGIC` | `cParticleSystem_shaddar_magic` (`0x8989DC`) | `0x7AA980`, `0x7AAC20`, `0x7AAA60`, `0x7AAB90` |

Their FX events take the event's direction and the flag `0x40`, so each flies level at its caster's bone: the whirlwind's (`0x37`) the root `__Root` (bone 0), the fire bolt's (`0x82`) the left hand (bone 1).
- The whirlwind's constructor (200 particles) loads `PARTICLE_CLOUD01`; no model and no light.
  Its event's `+0x38` holds the colour of the ground under its caster (`0x52507C`, the cell's nature by `0x634C50` into the renderer's table that the gust takes too, `0x41BA00`), else `0xFF808080`.
- Its start: that colour on its block, spread by ± 16 in each channel; one layer of the life −400 a second (0.64 s) and the size +30; one block of the buoyancy 700 (falling), the size 10 ± 5, the angle π ± π, the velocity (0, 0, 350) ± (0, 0, 50), the place ± 15 along x and y, one every 0.01 s at the detail 2; its speed 200, its flags `0x50`.
- Its run: it makes its puffs, left in the world along its way, for 2 s, then makes no more and ends with its last; a cell that stops it (its word `+0x74` bit 2) ends it at once, its puffs gone with it.
- Its draw: billboards of the flags `0x64` (each puff's own colour at the alpha of its life, turned, tinted by the light at it), laid over by their alpha (`SRCALPHA`, `INVSRCALPHA`), the grid 1.
- So about 64 puffs of the ground's colour live along its last 0.64 s of way, each shooting up to 87.5 at 0.5 s and grown to about 29: a column of dust about 90 high trailing behind it.
- The fire bolt's constructor (200 particles) loads `PARTICLE_SMOKE03` and a Direct3D light of (1.0, 0.8, 0.4), its range 200.
- Its start: its seconds 2; one layer of the life −450 a second (0.57 s) and the size +13; one block of the size 12 ± 6, the angle π ± π, the spin 0 ± 10, the velocity ± 18 along each axis, the first frame of its 2 × 2, one every 0.05 s at the detail 2; its ramp at the alpha 0.781 from (0.332, 0.980, 0.758) to (0.555, 0.984, 0.645) over 76 entries, to (0.129, 0.996, 0.641) over 66, to nothing over 114; its speed 150, its flags `0x53` (the flag 2 its puffs about its place, riding with it), its light 200.
- Its run: it makes its puffs while it flies; at its hit or a cell that stops it (bit 2), or past its 2 s, it makes no more, one puff burst at its end (a block of the size 10, the place ± 15, the velocity ± 20), and ends with its last.
- Its draw: its light at its place; billboards of the flags `0x10D` (the ramp's colour, turned, the frame of the particle's byte), added by their alpha (`SRCALPHA`, `ONE`), the grid 2.
- So about 11 green smoke puffs travel with it, a pale green glow from its caster's left hand to its target, fading where it ends.

`opensacred-game` draws them since #1092 (`bolt_trails`, `opensacred_rules::particles::{BoltTrail, TrailKind}`), in place of their stand-in balls: the whirlwind leaves its caster's `__Root` and flies level at its height, its dust left along its way in the colour of the ground where it leaves (`terrain::Heights::nature`, `GROUND_COLOURS`), at the alpha of its life, laid over and tinted by the light; the fire bolt leaves its caster's left hand, its smoke riding with it, added and untinted by the night.
**Seen** 2026-11-05 beside the original: the Mage's whirlwinds' dark green dust about her and along their way, gone where a tree stops them; the fire bolt's teal and white glow at the caster's hand and at its target alike in size and colour (Sharifo's in the game, the Mage's taught the spell 52 in the original).
Not followed: the dust's colour spread and the fire bolt's one puff at its end (its Direct3D light follows since #2106) (**hypothesis**: drawn from a stale origin, far off).

## The ice splinters

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The Mage's ice splinters, the spell 34 | 839 `FX_ICEBULLETS` | `cParticleSystem_icebullets` (`0x898218`) | `0x78CA30`, `0x78D640`, `0x78CD10`, `0x78CF50` |

Its FX event (`0x3F`) takes the event's direction and the flag `0x40`, so each bolt flies level at its caster's left hand (bone 1).
- Its constructor (200 particles) loads the models `Icicle01.grn`, `Icicle02.grn` and `Icicle03.grn`, of which it draws the first alone, and the textures `ICICLE`, `ICICLE2`, `FX_CIRCLE02` and `PARTICLE_SPARK07`; no light.
- Its start: its heading along its direction, its point the first of the ten places it keeps (`+0x25E4`), its timer 1; five icicles (`+0x2540`), each at an offset within 20 of its place along each axis (`+0x2544`); its sparks' ramp (`+0x20A8`) of each entry `i` a hero's `(⌊i/2⌋, ⌊i/2⌋, i)` blue white, another's grey `⌊i/4⌋`, its alpha 0; its speed 150, its flags `0x50`.
- Its run: flying, its timer falls by 10 a second, and at 0 its places shift back by one, its place the first, its timer 1 again: a place every 0.1 s; past 3 s, or at a cell that stops it (its word `+0x74` bit 2), it bursts, once: one layer of the life −150 a second (1.7 s), one block of the buoyancy 140 ± 20, the size 2, the velocity 70 along its direction within 10 along each axis, the place ± 2, and 40 sparks at each icicle's offset; then its sparks alone, their upward velocity turned back where they reach the ground under it (`0x7645C0` of the mode 0); it ends with its last.
- Its draw, flying: each icicle `Icicle01` in its world matrix, laid along its heading and turned about it by its clock (a radian a second), at its place and its offset, in `ICICLE` for a hero's (`+0xA9` 0) or `ICICLE2` for another's, lit, its depth written; then, unlit, depth tested and not written, both faces, a square of `FX_CIRCLE02` at each of its first four places and each icicle's offset, in the plane across its flight and turned with its icicles, its half side `2·((2 − t) + j)` for the `j`-th place back and the timer `t`, a hero's added (`ONE`, `ONE`) in a grey of 128, another's laid over (`SRCALPHA`, `INVSRCALPHA`) in black at the alpha 128, the fourth's 128 taken times the timer; burst, its sparks as billboards of the flags `0x19` (the ramp's colour, added whole), the grid 1.
- So five icicles fly in a loose cluster rolling slowly about their way, each trailing four pale rings growing from 2 to 10 about 15 apart, and at 3 s or a wall they burst into 200 blue-white sparks that fall and bounce for 1.7 s; `MAGE_ICEBULLETS` (2026) sounds once as the spell is cast (docs/re/sound.md).

`opensacred-game` draws them since #1092 (`ice_splinters`, `opensacred_rules::particles::IceSplinters`), in place of their stand-in balls: each splinter bolt leaves its caster's left hand, its icicles the model's point (its `+Z`) forward, another caster's in `ICICLE2`; its rings and its sparks untinted by the night.
**Seen** 2026-11-05 beside the original: the Mage's splinters of the level 5, a cluster of white icicles and pale rings alike in size and colour flying to the goblin, and their sparks at their end.
Not followed: which end of the icicle leads (**hypothesis**: its point, the model's thin end), and its turn's sense in the mirrored frame.

## The bubble

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The Wood Elf's bubble, the spell 38 | 847 `FX_BUBBLETRAP` | `cParticleSystem_bubbletrap` (`0x898430`) | `0x797B30`, `0x798380`, `0x797D80`, `0x798030` |
| Its burst | 906 | `cParticleSystem_bubbletrap_explo` (`0x898450`) | `0x798460`, `0x7985D0`, `0x7984F0`, `0x798530` |

Its FX event (`0x47`) takes the event's direction and the flag `0x40`, so the bubble flies level at its caster's right hand (bone 2).
- Its constructor (75 particles) loads the model `ElveStrauch.grn`, its motion 1 and its bone `Bone01`, its controlled animation (`0x4040D0`), and the textures `ElveStrauch.tga`, `ElveStrauch2.tga` and `particle_glow03`.
- Its start: its heading along its direction; its size's share 0; its seconds its record's value at the level, then its animation's length (**hypothesis**: its clip `FX_C_IDLE_BH`, 7.35 s); its speed 100, its flags `0x50`, its light 100.
- Its run: its share grows by 3 a second to 1; half a second before its seconds' end it shrinks by 2 a second and ends below 0, without a burst; a cell that stops it (its word `+0x74` bit 2) stops it, shrinking where it is; an arrow that passes through it (bit 8, docs/re/arts.md) bursts it: one layer of the life −150 a second, one block of the size 10 ± 3, the buoyancy 0 ± 10, the velocity ± 10, the place its `Bone01` ± 10, 75 glows at once, its ramp of each entry `i` green `(⌊i/2⌋, i, ⌊i/2⌋)`, its light 150, and the object 906 made; burst, its glows alone, and it ends with its last.
- Its draw, not burst: its bush at twice its share, turned to its heading, at its place, in `ElveStrauch` for a hero's or `ElveStrauch2` for another's, its shadow on the ground under it (`0x406FF0`) and its model lit, its depth written; burst, its glows as billboards of the flags `0x19` (the ramp's colour, added whole), the grid 1.
- 906's start: its corners' colour `0x0080FF80`, its radius 10, its flags `0x400` (it strikes, docs/re/arts.md), its light 200; its run: its radius grows by 250 a second, and it ends past 0.5 s; its draw: one flat ring of `FX_SONAREFFECT` on the ground under it (`0x763530`), from its radius to 30 more, of 50 segments, added whole, depth tested and not written, unlit, both faces, its colour `(⌊128 f⌋, ⌊255 f⌋, ⌊128 f⌋)` for `f = 2·(0.5 − t)`.
- So a thorny bush of orange vines and green leaves, about 38 across, grows in a third of a second at the Elf's hand, flies whirling for 7.35 s and shrinks away; burst, a puff of 75 green glows drifts and fades for 1.7 s and a green ring spreads on the ground from 10 to 135 in half a second.

`opensacred-game` draws them since #1092 (`bubble_traps`, `opensacred_rules::particles::{BubbleTrap, SonarRing}`), in place of its stand-in ball: the bubble leaves its caster's right hand, its bush its clip's loop playing, along its flight; burst by an arrow (`bolts::Pops`), its glows where it was and its ring on the ground under it, both untinted by the night.
**Seen** 2026-11-05 beside the original: the Wood Elf's bubble of the level 5, an orange ball of vines and green leaves alike in size flying from her hand; in the game her arrow burst it over a chasm, its green puff and the ring spreading on the chasm's floor (the original's arrow struck the goblin before it reached the bubble).
Not followed: its bush's shadow, its glows' place at its bone's state (taken at its place), the world's lights of 100 and 150 and the ring's 200 (`+0x9C`).

## The soulcatcher

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The Dark Elf's soulcatcher, the art 1044 | 869 `FX_SOULCATCHER` | `cParticleSystem_soulcatcher` (`0x8978A0`) | `0x76A6D0`, `0x76AA40`, `0x76A860`, `0x76A960` |

Its FX event (`0x5D`) follows its target, the Dark Elf, at his neck (`0x44CB80`: the bone 3 `Bip01 Neck`, else 0 `__Root`).
- Its constructor (100 particles) loads `PARTICLE_TWIRL01` and fills two ramps: the gold (0.977, 0.883, 0.1875) from nothing to the alpha 0.25 over 128 entries and back to nothing over 128, and the black from nothing to 0.996 over 128 and back over 128.
- Its start: its seconds its record's value 0 at the level times the event's factor, the effect 35 keyed by 1044 on its target for them (docs/re/arts.md); two layers of the life −500 a second (0.51 s) and −250 (1.02 s); two blocks alike, of the size 20 ± 8, the angle π ± π, the spin 6 ± 2, the place (0, 0, 40) ± 1 along each axis, one every 0.02 s at the detail 2; its flags `0x204` (the flag 4 its particles riding with its bearer, `0x200` at his bone each frame), no light; `DELF_SOULCATCHER_LOOP` (2068) looping at the volume 0.5.
- Its run: its twirls age; its blocks' size is its pulse and 20, its pulse falling by 13 a second; while its seconds last it makes twirls, four in five of the first layer and one in five of the second (`0x7687F0`); past them it ends at once, its twirls with it; a soul's signal (its word `+0x74` bit 8) plays `DELF_SOULCATCHER_ABSORP` (2069) once and sets its pulse to 20.
- Its draw: billboards of `PARTICLE_TWIRL01` of the flags `0x0C` (the ramp's colour, turned), the grid 1, in one batch in their slots' order, the first layer added by its alpha (`SRCALPHA`, `ONE`) and the second laid over (`SRCALPHA`, `INVSRCALPHA`), the blend switched at each change of layer; while its pulse lasts a glow of the style 4 (`PARTICLE_SPARK07` added whole, `0x762E10`), its half side twice the pulse, in `0xFFFFEF80`, 40 over the bone, depth tested.
- So for its time a cluster of spinning twirls 24 to 56 across hangs 40 over his neck, riding with him, about 30 alive: faint gold, darkened where a black one falls late in the batch; at each soul its twirls swell to twice their size and shrink back over 1.5 s under a pale yellow flash.

`opensacred-game` draws it since #1092 (`soulcatchers`, `opensacred_rules::particles::Soulcatcher`): the soulcatcher's object that `making` hangs at the Dark Elf takes its look, its twirls over his `Bip01 Neck` riding with him, each run of one layer in their order a mesh of its own drawn after the one before (`depth_bias`), untinted by the night; each soul that `souls` absorbs swells them and lights the glow.
**Seen** 2026-11-05 beside the original: the Dark Elf's soulcatcher of the level 5, a gold vortex over his head alike in its place, its colour and its spiral, a little smaller; the swelling and the flash at a soul in the game.
Not followed: the order of the twirls' slots, which reuses the dead ones (ours their births'), so that its fully dark moments are rarer.
In the original a creature's ordinary death makes no soul (780): a goblin killed by the Dark Elf without his soulcatcher started none (the trace of `0x76CD50`), one killed with it one.

## The souls

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| A soul homing on the soulcatcher | 780 `FX_PUZZLE_SOLVED` | `cParticleSystem_puzzleSolved` (`0x897944`) | `0x76C930`, `0x76CD50`, `0x76C9D0`, `0x76CB10` |

Its FX event (`0xC`, docs/re/arts.md) puts it at the dying creature's place at its height; homing on a soulcatcher, its flag `0x4000`.
- Its constructor (500 particles) loads `PARTICLE_SPARK06` (and `PARTICLE_FACE01`, never drawn).
- Its start: its four corners `0x00A0FFB0`, pale green (160, 255, 176); homing, its layer's pull (`+0x1C`) 50, its life's rate 0 and its size's −2, its block's buoyancy −50 (rising) and size 5; then, its target a model, a particle at every `⌊V/500⌋`-th vertex of each of its meshes, skinned to its pose (`0x767FB0`, `0x44FC30`), up to 500, of its block's values and the life 255.
- Its run: its speed `⌊10 t⌋` toward its target's neck (`0x44CBB0`, the bone 3, else the root); its pull grows by 100 a second and its size's rate by 0.5 a second up to 0; its sparks age, each drawn toward its place (`0x7640C0`: its velocity less `dt` times the pull along the unit way from its place to it); at its arrival, or at a cell that stops it (its word `+0x74` bit 2), every spark's velocity is zeroed and its life's rate −250; with no spark alive it asks the soulcatcher's signal 8; past 10 s it ends.
- Its draw: billboards of `PARTICLE_SPARK06` of the flags `0x13` (the corners' colour at the alpha of its life, added whole), the grid 1.
- So the dead creature's figure in up to 500 pale green sparks, their half side 5 shrinking to 1 over 4 s, rises and is pulled ever harder to a place speeding from its feet to the Dark Elf's neck: a swirling green cloud streaming to him; at its arrival the sparks stop and vanish together 1.02 s later.

`opensacred-game` draws them since #1092 (`soul_sparks`, `opensacred_rules::particles::SoulSparks`, the pull of `Layer::pull` and `System::update_toward`), in place of the stand-in ball: each soul that `souls` sends takes the vertices of its creature's meshes but its worn items', skinned on the CPU by its joints' places as it dies, and flies from its feet to the Dark Elf's `Bip01 Neck` (90 over the ground before); a soul that a cell stops fades and is absorbed without healing him, as the original's.
**Seen** 2026-11-05 beside the original: a goblin's figure in pale green sparks streaming up and to the Dark Elf in both, its sparks vanishing at his soulcatcher.
Not followed: the souls that no soulcatcher draws (no system starts, docs/re/arts.md).

## The spikes

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| Anducar's demon's rings, the spell 69 | 1163 `FX_ANDUCAR_DEMON` | `cParticleSystem_anducar_demon` (`0x898F58`) | `0x7C0020`, `0x7C04B0`, `0x7C0150`, `0x7C02B0` |
| The demon prince's fan, the spell 71 | 1166 `FX_DAEMONENFUERST` | `cParticleSystem_daemonenfuerst` (`0x898F80`) | `0x7C0830`, `0x7C0CC0`, `0x7C0960`, `0x7C0AC0` |

The two draw no particles: each object raises spikes about its place, its constructor loading the models `anducar_spikes1.grn` to `anducar_spikes5.grn`, the texture `Andukar_demon_512.tga` and a glow, `Andukar_demon_glow.tga` (1163) or `andukar_demon_glow2.tga` (1166).
- Their start: 3, 6 or 10 spikes by the detail (0, 1, 2); each of a size `0.65 + 0.55 r` (1163) or `0.45 + 0.25 r` (1166), a turn `2π r` about the vertical, an offset of `140 r − 70` (1163) or `130 r − 65` (1166) along each of y and x, a wait of its distance from the wave's origin (the event's `+0x2C`: the target's place for 1163, the caster's for 1166) over 350 (1163) or 550 (1166), a standing time of 0.75 s (1163) or 0.5 s (1166), and one of the five models by `rand() % 5`, `r` each a roll of `rand()/32767`; 1166 keeps its event's `+0x38`, the colour that the act gives by the prince's type (`0x529037`: 250 `0xFFFFA010` orange, 185 `0xFF20FF20` green, 251 `0xFF6060FF` blue, others a register's value, **hypothesis** white).
- Their run: each spike waits, then its growth rises by 2 a second to 1 (its object's flags `0x400` set, so it strikes, docs/re/arts.md), it stands for its time, and its growth falls by 2 a second to 0, when it is gone; the object lives while one is not gone.
- Their draw: each spike not waiting nor gone in its world matrix, its scale half its size times its growth across and its size times its growth up, turned and at its place and offset; its texture overridden by `Andukar_demon_512` (`0x401BD0`); at the detail 2 its shadow (`0x406FF0`); its model lit, its depth written (`0x405C90`); at the detail 2 again added in its glow's texture and colour (`0x40D310`), white for 1163.
- So a field of tall thin dark red spikes, their tips aglow in orange, springs from the ground as a wave from the target (1163) or from the prince along his line (1166), stands and sinks back within 2 to 3 s.

`opensacred-game` draws them since #1092 (`spikes`, `opensacred_rules::particles::{Spikes, SpikesKind}`), in place of the stand-in balls of flame: each object of `creature_casts`' rings and fan raises its ten spikes on the ground, its models in `Andukar_demon_512`, their glow added in copies of their meshes; the last stand-in ball of the bolts gone with them (`bolts::Looks`).
**Seen** 2026-11-05 beside the original: Anducar's demon's rings about the Mage, a field of dark red spikes with glowing tips in both, the game's a little shorter and thinner; in the game alone, the demon prince 250's fan, its smaller spikes' tips orange, along his line to the Mage.
Not followed: their shadows.

## The stargates

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code unless marked).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The waypoints' gates | 802 `FX_STARGATE` | `cParticleSystem_stargate` (`0x897E9C`) | `0x77FBE0`, `0x780BB0`, `0x780450`, `0x780790` |
| The add-on's gates | 1180 `FX_STARGATE_UW` | `cParticleSystem_stargate_uw` (`0x899118`) | `0x7C5C10`, `0x7C6BE0`, `0x7C6480`, `0x7C67C0` |

The scripts make them by `CreateObj` (the Seraphim's tree's 65 of 802, the waypoints' functions `OMO_Wegpunkte_*` after their `teleporter` where the bit `Teleporter_WP` is not set, the quests' and the regions'; the add-on's of 1180 alike); their FX events (`0x22`, `0xB3`) put the system at the event's point, a cell's centre at the height 0, with no variant.
The two run one code; 1180 takes other textures.
- Their constructor loads sixteen pictures of the world map's sectors (`MINIMAP<x><y>.TGA`: 802 058028, 063031, 049032, 034035, 061032, 044044 to 044041, 045044, 050036, 050035, 050037, 051034 to 051036; 1180 seven of the add-on's sectors in its sixteen slots, two of them thrice) and two whirls (`FX_WHIRL01` and `02`, bluish; 1180's `03` and `04`, reddish); it builds a window of 20 columns by 34 rows of pre-lit vertices in the plane of the models' `x` and `z` (upright, facing the viewer), at `x = 6(i − 10)` and `z = 6(j − 17)`, its texture's `u = i/33` and `v = 1 − j/33` (the picture's left 57 % at its whole height), twice, for the picture shown and the next; each vertex's fade `3.2·(10 − d)` truncated to 0 to 15, `d` its distance in steps from the segment of (9.5, 10) to (9.5, 24), so a capsule whose sides fade over their outer five columns and whose round ends fade out; the alpha of a picture's level `a` and a fade `b`, 0 to 15 each, `a·b·17/15` of 255; two quads of the whirls in `0x8FFFFFFF` and `0xAFFFFFFF`.
  Its start: two pictures at random of the sixteen, the shown at the level 15 and the next at 0, its whirls' turns π/4, its world light 200 (`+0x9C`), its flags 0.
- Its run (nothing past a second's frame): its clock; its ripples' phase by 5 a second; its growth `g` by 0.5 a second to 1; at 3 s the next picture's level rises as `15.9·(c − 3)`, at 4 s the shown's falls as `15.9·(5 − c)`, and at 5 s the next is shown, a new one rolled, its clock again 0; each vertex's ripple `5·cos(2r − t)` toward the viewer, `r` its distance in steps from (9.5, 17) and `t` the phase, its grey `127.9·(1 + N·L)` of a slope `sin(r − t)` along `r` under the light (2/3, −2/3, 1/3), 213 when flat; its whirls turning by `0.3 k` and `0.4 k` a second, `k` 1/`g` (0.1 while `g` is below 0.1), their corners on an ellipse of 125 across and 220 up.
- Its draw: a world matrix of the scale `sin(g·π/2)`, 95 over its point; the depth tested and written, unlit, laid over by their alpha (`SRCALPHA`, `INVSRCALPHA`): the window in the shown picture while its level is above 0, then in the next over it while its level is; then the depth neither tested nor written: the first whirl laid over, and the second added by its alpha (`SRCALPHA`, `ONE`); the night's tint left out.
  Note: `0x643430`'s bit `0x100` sets the depth's write on with its second argument 1 (the billboards' draws call it with 0, the write off).
- So an upright window 114 by 198, centred 95 over the waypoint, shows a sector of the map rippling in rings from its middle, its edges fading; every 5 s another picture fades in over it; two whirls of 250 by 440 turn about it over everything, the blended one at 0.3 a second and the added one at 0.4; it grows from a point over 2 s, its whirls spinning fast while it is small; by night it lights the ground within 200 of it, its own colours as by day.

`opensacred-game` draws them since #1092 (`stargates`, `opensacred_rules::particles::Stargate`): each gate object of the scripts (`npcs`, which loops its sound, `fx_loops`) shows its window's two copies, rewritten each frame, its whirls over everything (`fx::over_material`) and its light in the world's table, untinted by the night.
**Seen** 2026-11-05 beside the original at the Kernland's waypoint (2642, 3057, `OMO-1Wegpunkte_Kernland`): in both a window of a sector's map inside the stone arch, a violet and blue whirl about it, alike in size and colour.
Not followed: the window's depth written over its whole rectangle, its clear corners included (**hypothesis**: the original's alpha test is off there).

## The dwarves' magic

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code unless marked).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The dwarves' magic | 832 and 833 `FX_DWARFMAGIC` | `cParticleSystem_dwarfmagic` (`0x898154`) | `0x78A2D0`, `0x78A5D0`, `0x78A360`, `0x78A3B0` |

The scripts' `CreateObj` makes them (about 900 in the Seraphim's tree, the sectors' `Enter` functions of the dwarves' halls and the caves: 34,83, 70,14, 10,95 and others), 832 with the variant 1 and 833 with the variant 2 (`0x4898B2`, `0x489920`); their FX event's kind `0x38` takes the fires' case (`0x5A5AE2`), the system at the record's point at its height (`0x7E`), following nothing.
- Its constructor (100 particles, either type) loads `PARTICLE_SPARK04`, a white star of four points on black; no model, no Direct3D light, no sound.
- Its start, by its variant: one layer of the buoyancy −10 (−12 for 2), the life −175 (−150: 1.46 s, 1.7 s), the size +1.5 (+2.5) and the angle +2.1 a second; one block of the buoyancy −30 (−40), the size 1 ± 0.5 (2 ± 1), the angle π ± π, the place (0, 0, 100) ± 5 (± 15) along each axis, one every 0.2 s, not scaled by the detail; one ramp from white to (1, 1, 0.6) over 128 entries (pale blue (0.7, 0.7, 1) for 2) and to black over 128; its world light 350 (400); its flags 0.
- Its run: one star every 0.2 s, its stars aged; it never ends (its alive byte stays, and the types are on the set-up's persistent list).
- Its draw: billboards of the flags `0x1D` (the ramp's colour, turned, added whole, `ONE`, `ONE`), the grid 1; untinted by the night.
- So about 100 over its place a faint twinkle of small stars, seven to nine at once, each rising some 37 (68) units as it swells, turns and fades through pale yellow (pale blue) to nothing; and a pool of the world's light of 350 (400) about it by night or in a dungeon.

`opensacred-game` draws them since #1092 (`dwarf_magics`, `opensacred_rules::particles::DwarfMagic`): each such object of the scripts (`npcs`) twinkles at its place and height and lights the world's table, untinted by the night.
**Seen** 2026-11-05 beside the original at 4510, 915 (the dark elves' cave, 70,14): in both the blue stars twinkling over the blue crystal lamps, alike in size, colour and count.

## The Daemon's forms

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code unless marked).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The Daemon's forms 1097 to 1100 | 1014 `FX_DEMONFORM` | `cParticleSystem_demonform` (`0x898BEC`) | `0x7B1D50`, `0x7B27D0`, `0x7B20F0`, `0x7B2230` |

The Daemon's executor makes the object 1014 at the form's bearer as the form takes hold (`0x51A47B`), its FX event `0x8F` with the variant 3 for 1097 (physical, white), 2 for 1098 (fire, orange), 1 for 1099 (poison, green) and 0 for 1100 (energy, pale blue), and the form's seconds `min(44 + L, 180)` (the event's `+0x48`); its case (`0x5A56D2`) follows the bearer at his head (the bone 20, `Bip01 Head`), the system flag `0x200` putting it there at each frame, its orientation the head bone's.
- Its constructor (1 particle, never emitted) loads seven ray bursts, each `fx_punk_1` to `3` at random, the streak `fx_demonform_demon`, and its flares' `PARTICLE_FLARE04`; a Direct3D light (`fx_lights`'s table).
- Its start: its colour by its variant, (116, 190, 255), (107, 255, 128), (255, 170, 101) or white, and its light's (0.5, 0.7, 1), (0.4, 1, 0.5), (1, 0.7, 0.4) or (1, 1, 1); its growth 0; its flags `0x200`, no world light; `DEMON_CAST` (2122) once.
- Its run: its growth by 0.5 a second to 1; three rings of nine points about its place, the point `i` of the ring `r` at `R·(cos φ, sin φ, E + D·sin(C·φ))`, `R = H + G·sin(F·φ)`, `φ = π·(A·t − i·B/9)`, of the figures (A, B, C, D, E, F, G, H) (2, 0.6, 0.35463, 1.2, −0.7, 1.463, 7, 18), (1.7, 0.5, 0.6351, 1.1, −0.9, 1.946, 5, 16) and (1.5, 0.65, 0.2755, 1.05, −0.78, 1.102, 5.8, 17): each ring a comet's tail along its head's path, turning once a second or so, 11 to 25 about the head and from 12 over it to some 45 under it; its light at the first ring's head; past its seconds it goes at once, and with its form (the effect 60's end, docs/re/arts.md).
- Its draw, untinted by the night: seven squares of the ray bursts added whole (`ONE`, `ONE`), each of a half side `13·g·(1.5 + 0.5·sin(t + 0.8k))` in the head bone's own plane, turned by `(k − 3)/17` about it, 8 over the head, its colour its light's times `0.2 + 0.3·(1 + sin(2.4t + 1.5k))`; three ribbons of `fx_demonform_demon` added whole along the rings, each point's edges 3 either side across its way and the line of sight (0, −2, −1), its texture's `v` from the head to the tail; three flares of the style 8 of a half side of 8 at the rings' heads, in its colour.
- So three comets of the form's colour whirl about the Daemon's head and chest, each led by a flare, and a crest of rays in its colour grows over his head over 2 s, pulsing, as long as the form holds.

`opensacred-game` draws it since #1092 (`demon_forms`, `opensacred_rules::particles::DemonForm`): `making` makes the look as a form's effects take hold, at the Daemon's `Bip01 Head`; it goes as his form's effects end, a new form's look taking its place.
**Seen** 2026-11-05 beside the original: the Daemon's fire and energy forms, the comets whirling in their colour about his head and chest in both, his crest of rays over his head.
Not followed: the crest in the head bone's own plane, which the game faces to the viewer as the original's frames show it from the front (**estimate**); the tongues of flame about his hands in the original's frames are his form's casts' sights ("The casts' sights"), not the crest.

## The casts' sights

Read 2026-11-05 for #1092 in Ghidra (**confirmed** by code unless marked).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| A cast's sight | 791 `FX_CAST_SPELL` | `cParticleSystem_castSpell` (`0x897BE8`) | `0x776AB0`, `0x7773D0`, `0x776BB0`, `0x776CA0` |

The spells' cast (`0x51E110`, its table `0x521C34` by the spell) makes them for the spells that it names, the Daemon's executor (`0x51B46E`) for 1101 to 1103 and his forms, and the scripts' queued `Partikel` of `n0` −2; each its own object at the caster's place, its FX event 23 (`0x5A496E`) following the caster at its bone (`+0x38`: 21 `Bip01 L Finger3`, 22 `Bip01 R Finger3`, 0 the root, 36 and 37 the feet), its style at the event's `+0x3C`, its clock's rate at the point's `x` (the caster's casts' speed over 100 for the cast, 1 for the Daemon's arts); no seconds.
- Its constructor (300 particles) loads `PARTICLE_GLOW03`, `GLOW01`, `SPARK06`, `SPARK08`, `LINE01` (never drawn) and `Fx_twirl_rays`; no light of either kind.
- Its start: its style, 13 instead where the handler marks the system's `+0xA9` (a caster above the id 16 not found, or the event's `+0x28` not 1.0), so that a creature's casts take the style 13, as the cast writes the point's `x` alone (**seen** 2026-11-05: Anducar's demon's fire bolt's sights drawn of the style 13, their `+0xA9` 2, a probe of `0x776CA0`); its flags `0x200` (`0x40` for the style 22), so that it stands at its bone each frame and leaves its particles where they are born; by its style one layer, one block and one ramp:
  - the clock's times from which and to which it shows: 0 1.0–4.0, 1 0.7–2.3, 2 0.5–1.2, 3 0.5–2.0, 4 0.3–3.5, 5 1.0–2.9, 6 0.7–2.9, 7 2.0–3.5, 8 and 24 4.2–8.0, 9 and 12 0.1–1.0, 10 0.5–2.3, 11 1.0–2.9, 13 0.5–2.5, 14 to 16 0.2–1.7, 17 to 20 0.2–2.8, 21 0.7–1.7, 22 1.8–10;
  - its particles: 0 and 1 the life −300 and the size +2, the size 8 drifting by 5 within 2, one every 0.02 s, magenta (0) or blue (1) ramps to black; 2 and 3 the life −150 and −200, rising (−10), the size 4 within 2, every 0.005 s; 4 the life −100, the size 2.5, every 0.015 s, green; 5 to 8 and 24 their own colours about 0x808080, 0x5050E0, 0x505050 and 0x505080 by a channel's spread, dying as they shrink; 9 and 12 the orange (255, 175, 47) or pale cyan (143, 252, 244) glows of a size 10 within 1 shrinking by 16, living 0.36 s, one every 0.045 s; 11 turned sparks of a five-key ramp from green to orange to black to white; 14 and 15 orange and green turned sparks of the size 10 ± 3 and 7 ± 2, the life −80 and the size −7; 16 dark puffs laid over by their alpha; 17 to 21 rising turned sparks of the size 4 ± 2 in the form's colour, green, orange, pale blue, grey-white and pale blue;
  - the intervals times 1.5 and 2.2 at the detail levels 1 and 0.
- Its run: its clock by the time times its rate; from its first time its particles age by the real time, and to its second it makes them at its bone; past it it ends with its last; the styles 10, 13 and 22 end past their second time.
- Its draw, untinted by the night: billboards added whole, `SPARK06` (0 to 4, 14, 15, 17 to 21), `GLOW03` (5 to 9, 12, 24), turned `SPARK08` (11), and `GLOW01` laid over (16); a glow at its bone while it shows, its half side `S·sin(π·(t − A)/(B − A))`: the style 10 green of 35 and 13 blue of 40 (the glow style 7), 17 to 21 of 15 in their colours (the style 8); the style 22 two twirls of `Fx_twirl_rays` flat on the ground at the root in `0xFF804122`, of the half sides `80·(1 − f²)` and `70·(1 − f²)`, turning by `−1.7 t` and `−1.3 t`.
- The sights by the spell: 3 both hands 3; 5 the left 9 (12 for an undead caster); 6 the left 0; 11 the right 9; 13 both 1; 15 the right 6; 17, 20, 21 and 22 both 8; 18 the right 7; 23 and 43 the left 5; 24 the left 6; 26 the left 1; 29 the left 9; 35 and 41 both 5; 36 both 10; 37 both 11; 39 the left 2; 40 the right 4; 52 both 12; 60 both 14 and the root 22; 61 to 65 and 67 both 14; 66 both 16; 72 both 24; by the Daemon's art: 1101 both 15, 1102 both 14, 1103 the right 21, his forms 1097, 1098, 1099 and 1100 both 20, 18, 17 and 19.
- So the Mage's fireball pops small orange glows at his left hand as he casts, left along its way; the Daemon's forms raise sparks and a flare of their colour at both his hands; the others alike in their colours.

`opensacred-game` draws them since #1092 (`cast_sights`, `opensacred_rules::particles::{CastSight, cast_sights, art_sights}`) for the hero's own casts and the Daemon's arts as their clips start, and for a creature's casts as `creature_casts` begins their clips, each of the style 13 at the bones of its spell's sights (`CREATURES_CAST_STYLE`); each at its bone (`Bip01 L Finger3`, else the hand), its particles left in the world, untinted by the night.
**Seen** 2026-11-05 beside the original: the Mage's fireball's orange glows at his left hand in both; the Daemon's fire form's orange sparks at his hands, the tongues of flame that the original shows about his raised hands.
**Seen** 2026-11-06 in the game: Anducar's demon's blue glows at his hands as he casts his fire bolt.
Not followed: an undead hero's fireball (no hero is one); the scripts' `Partikel` of `n0` −2; a particle's own colour's spread is rolled once a particle by one byte, its channels apart.

## The Dwarf's guns' fires

Read 2026-11-06 for #1092 in Ghidra (**confirmed** by code unless marked).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The flamethrower's flames (1085) | 1016 | `cParticleSystem_dwarf_flamethrower` (`0x898C40`) | `0x7B34D0`, `0x7B3870`, `0x7B35F0`, `0x7B37A0` |
| The cannon's flash (1087) | 1017 | `cParticleSystem_dwarf_shoot` (`0x898C68`) | `0x7B40D0`, `0x7B4480`, `0x7B41B0`, `0x7B4410` |
| The mortar's burst (1086) | 1018 | `cParticleSystem_dwarf_grenade` (`0x898C94`) | `0x7B4CB0`, `0x7B50F0`, `0x7B4DC0`, `0x7B5080` |

- Their makers are the Dwarf's executor (`0x514790`) alone, at the moments of docs/re/arts.md: the flamethrower at the clip's first hit point (`0x516BF9`), the cannon's flash at `1 − h₀ + 0.05·r` (`0x51724D`), the mortar's at its first hit point (`0x516894`), `r` the clips' rate (1.2).
  Their FX events `0x94` to `0x96` follow the Dwarf at his bone 46 (`canon_fx01`, the muzzle of his back's cannon); none has a delay, a variant or a world light; each its Direct3D light (1, 0.8, 0.4); none tinted by the night.
- The flamethrower (999 particles; `particle_flamethrower`, a 2 × 2 grid, and `particle_smoke04`): its flag `0x200` stands it at the muzzle each frame with the bone's turn; its three blocks' velocity is 200 along the bone's `x` axis, set again each frame, their sizes 2 ± 1, 2 ± 1 and 2 ± 0.7, spinning, the velocity ± 5, ± 8 and ± 10, every 0.003, 0.001 and 0.001 s (times 1.5 and 2.2 at the detail levels 1 and 0), each particle of its block's frame, three layers of the life −250 and the sizes +13, +4 and +20 a second; it streams them until `3.25/r` s, and once the last is gone a smoke rises at the muzzle every 0.05 s until `6.25/r` (a buoyancy of −55 ± 10, the size 1, the life −200 and the size +4), ending with its last puff.
  Its draw: the flames turned, of their layer's ramp's colour (the flags `0x10D`, the byte `b` 1), the layer 0's dark smoke laid over by its alpha and the layers 1 and 2, white-yellow and orange, added; the smoke laid over by its alpha.
  Its light shines at the muzzle until the last flame dies; its sound 2113.
- The cannon's flash and the mortar's burst (500 particles each; `particle_shoot`, a 2 × 2 grid): at the second run the flag `0x40` places it once at the muzzle with the bone's turn, where it stays; four blocks along the bone's `x` axis, smoke at 25 (a buoyancy of −60 ± 20, the size 10 ± 4), sparks at 100 ± 40 (the size 4 ± 1), fire at 15 (4 ± 2, its place ± 5) and streaks at 1500 (5 ± 1, a buoyancy of 11 ± 10), every 0.001, 0.00007, 0.0006 and 0.0001 s, chosen by their rates; their layers the smoke's of the life −100 and the size +20, the sparks' of −300 drawn back by 80 a second, the fire's of −350 and +17, the streaks' of −1200 blown back along the shot; streamed until `0.18/r` s (the cannon's) or 0.15 s (the mortar's), lit until then, and ended with the last particle; drawn as the flamethrower's, the smoke laid over and the rest added; the sounds 2116 and 2114.
  The mortar's object asks its impact, the object 1020 (`cParticleSystem_grenadeimpact`), at its target's place 0.775 s on (its action 1, `0x5B40C4`); it does not fly.

`opensacred-game` draws them since #1092 (`dwarf_fires`, `opensacred_rules::particles::{DwarfFire, DwarfFireSight}`) as `making` sounds them, at the Dwarf's `canon_fx01`, else his right hand, the flamethrower's along the bone's glTF `x` each frame, the shots' placed at their first frame, their lights at the muzzle while they fire, always at the detail 2.
**Seen** 2026-11-06 in the game: the Dwarf's flamethrower streaming from his back's cannon over a goblin and smoking after, and his cannon's flash toward it; not beside the original, whose driver's right button does not make the Dwarf's 1085 to 1087 (#2162).
The impact 1020 follows since #2160 ("The mortar's impact").

## The light strikes

Read 2026-11-06 for #1092 in Ghidra (**confirmed** by code unless marked).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The stone skin (2) | 769 `FX_SHIELDED` | `cParticleSystem_shielded` (`0x897A20`) | `0x770C70`, `0x7712E0`, `0x770EA0`, `0x770F30` |
| The water form (4) | 783 `FX_SUPERFLUID` | `cParticleSystem_superfluid` (`0x897AE0`) | `0x773880`, `0x773EC0`, `0x773AB0`, `0x773B40` |
| The crusader's strength (21) | 818 `FX_CRUSADERSTRENGTH` | `cParticleSystem_crusaderstrength` (`0x897FEC`) | `0x7851B0`, `0x785800`, `0x7853E0`, `0x785480` |
| The quickening (27) | 825 `FX_SPEEDUP` | `cParticleSystem_speedup` (`0x8980C4`) | `0x788180`, `0x7887E0`, `0x7883C0`, `0x788460` |

- The four are copies of one code with a few constants changed, each derived from `cParticleSystem` alone.
  None emits a particle (their particles lie dead), none has a layer, a block, a ramp or a Direct3D light, and the scripts' `CreateObj` makes none of them: the spells' executor alone does.
- Their makers: 769 the executor's case 2 (`0x522CDD`), its object at its bearer's place, its FX event of the kind 5 with the bearer as its caster and its target; 783 the case 4 (`0x52313A`), the event `0xF`; 818 the case 21 (`0x5244EF`), the event `0x2D`; 825 the case 27 (`0x524C8C`), the event `0x34`; those three at the caster's place.
  Each follows its creature at `Bip01 Neck` (3; the flags `0x1200`): 769 its bearer, 783 and 825 their caster, who is the bearer under the launch type 5, and 818 its target.
  A creature's stone skin on its friend comes through the same executor (docs/re/ai.md), and `ECS_SUPERFLUID` gives a script's creature the water form.
- Their start: the clock, the flicker and the frame 0, the world light 200, and for all but 818 the event's `+0x20`, the cast's rate.
- Their run: the clock by the frame's time, times the rate but for 818's; the flicker by the frame's time alone, and each time it passes 0.07 s it loses 0.07 and the frame becomes `rand() & 7`.
  Once the clock is above 1.6 s (`0x8912CC`) the system ends and asks its action 6, which gives the spell's effects and sends the bearer the event `0x109` of its overlay (below).
  A bearer gone or in another sector before ends it at once, and the action never runs (**hypothesis** in play: the spell is lost).
- Their draw, its depth test and write off, unlit, both faces drawn: a strike of light, a quad of four pre-lit corners in the models' upright plane `x`–`z` about the neck, from −30 to 30 across and from 20 under the neck to 200 over it, opaque at its foot and clear at its top, the texture's `u` up it and its `v` across, added by its alpha (`SRCALPHA`, `ONE`), untinted by the night, turned neither to the camera nor by the bone; then a glow facing the camera (`0x762E10`) of the half side 30, 2 under the neck, added whole.
  The frame `f` takes `FX_LIGHTSTRIKE0(⌊f/2⌋ + 1)`, an even frame the top half of its breadth (`v` from 0.5 at the left to 0 at the right), an odd frame the bottom half (from 1 to 0.5); each texture holds two pale blue-white bolts, one along each half, so that a bolt flickers down onto the neck.
  The ring of frost's strike is the same at twice the size, without the fade ("The ring of frost").

| | 769 | 783 | 818 | 825 |
|---|---|---|---|---|
| The strike's colour | white, `0xFF8080` where its bearer is no hero (`+0xA9`) | white | white | white |
| The glow | `PARTICLE_GLOW04` in the strike's colour | `PARTICLE_GLOW05`, white | `PARTICLE_SPARK07`, white | `PARTICLE_GLOW03`, (160, 160, 255) |
| The clock | times the rate | times the rate | the frame's time | times the rate |
| The action 6's data and texture | 7, none | 1, none | 6, `FX_CROSS` | 10, `FX_SPEEDUP` |

- The overlays that their action 6 asks (the event `0x109`, kept at the creature's `+0x60`, `+0x68` and `+0x6C`; `0x44A520`, `0x44A9D0`), for the seconds `⌊v₀(L)·f⌋`:
  - the state 1, the water form (`0x44AB44`): the model drawn instead by `0x40BD70`, lit, its depth tested and written, laid over by its alpha with the alpha test on, in `GRN_SHADETEXTURE02` (64 × 1, white, its alpha from 0 at its ends to 136 in its middle) by its vertices' normals as its texture's coordinates, a translucent white figure (**hypothesis** on the look); then the state 3 for 2 s, that look and the plain model in turns every 0.2 s (`⌊5t⌋` odd or even of the seconds left);
  - the state 7, the stone skin (`0x44ACAA`): the model, then again by `0x40CA20` in `FX_MULTITEXGLOW01` by its own coordinates scrolled by 0.05 a second of the real clock along both, unlit, its depth tested and not written, added whole, white; then the state 8 for 2 s, blinking so;
  - the state 6, the crusader's strength (`0x44AD29`): the model, then again by `0x40CD30` in `FX_CROSS` by its coordinates times 3, unlit, its depth tested and not written, added whole, in a grey of `⌊50·(1 + sin(0.007·ms))⌋` of the real clock's milliseconds, a pulse of about 0.9 s from 0 to 100: crosses glowing over the whole body, to its seconds' end without a blink;
  - the state 10, the quickening (`0x44AD7B`): the fire skin's skin of lava's draw in `FX_SPEEDUP` ("The fire skin").

`opensacred-game` draws the strikes since #1092 (`light_strikes`, `opensacred_rules::particles::{LightStrike, StrikeKind, strike_frame}`) as `spells` casts the four spells, the creatures' stone skins on their friends too, their clocks those of the spells' effects (`spells::SHIELD_TIME`, `STRENGTH_TIME`), the light of 200 standing where the object is made.
**Seen** 2026-11-06 beside the original: the stone skin's and the quickening's bolts and their glows, white and violet, alike in both.
It draws the overlays since #1092 too (`skins`, `opensacred_rules::spells::{cross_skin_grey, skin_shows}`), asked with the spells' effects as the strikes end: the stone skin's glow and its blink of 2 s, the crusader's strength's crosses and the quickening's speed, each a copy of the bearer's meshes added over them, dressed again on the new meshes where the model is made anew (the hero's is after the quickening; **hypothesis**: by its pace).
**Seen** 2026-11-06 beside the original: the stone skin's pale glow over the Mage, alike in both; the quickening's blue skin brighter in the game than the original's (#2169).
  The skins' grey taken in the frame's values since #2169 ("The fire skin"), **seen** again at the Mage's start at 22:00 and the zoom 2, the frames about half a second apart over the pulse: the quickening's blue streaks over the Mage's own colours rise and fall in both, the bluish pixels' mean from (43, 55, 74) to (82, 101, 135) in the original and from (66, 74, 101) to (93, 109, 151) in the game, where it lay between (91, 105, 141) and (105, 122, 164) before, a pale blue all over; the crusader's crosses on the Seraphim faint in both, where the game's had laid white crosses over her body.
Not followed: the strikes over all else (their depth test off), whether the light follows its object's creature (**hypothesis**: it stands), and the water form's overlay (the states 1 and 3).

## The holy shield, the shield wall, the mana's spring and the regeneration

Read 2026-11-06 for #1092 in Ghidra and by scans of the executable (**confirmed** by code unless marked).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The holy shield (22) | 819 `FX_HOLYSHIELD` | `cParticleSystem_holyshield` (`0x898010`) | `0x785860`, `0x785DE0`, `0x785970`, `0x785A20` |
| The shield wall (30) | 834 `FX_MAGICSHIELD` | `cParticleSystem_magicshield` (`0x898174`) | `0x78A8A0`, `0x78B040`, `0x78AAF0`, `0x78AC00` |
| The mana's spring (31) | 835 `FX_MANAREGEN` | `cParticleSystem_manaregen` (`0x898198`) | `0x78B160`, `0x78B4B0`, `0x78B200`, `0x78B230` |
| The regeneration (32) | 836 `FX_HPREGEN` | `cParticleSystem_hpregen` (`0x8981B8`) | `0x78B5B0`, `0x78B900`, `0x78B650`, `0x78B680` |

- `0x5A0C50` makes them with 200, 100, 1 and 1 particles; none of the four emits or draws a particle, none calls the light manager, none rolls `rand()` or reads the detail level, and none uses the tint: all four keep their colours by night.
- Their world light is 300 for 819 and 834, none for 835 and 836.
- The scripts' `CreateObj` makes none of them; the spells' executor makes each (`0x524611`, `0x5252C3`, `0x5253AF`, `0x52549E`), its object following its bearer (the order's target, else the caster) at the bone 0 (`__Root`), for the record's value 0 at the level times the factor: 43 + 2·L s for 819 and 44 + L s for 834, at most 120, and 70 + 2·L s for 835 and 836, at most 180.
- The flat quads' helper (`0x7631E0`) turns a square by `(x, y)` to `(x·cos θ + y·sin θ, −x·sin θ + y·cos θ)`, so that a growing angle turns it counterclockwise on the screen, the texture's `u = (x/s + 1)/2` and `v = (y/s + 1)/2` over it.

**The holy shield.**

- Its cylinder: 31 pairs of pre-lit vertices, the pair `i` at the angle `a = i·2π/30` about the bearer, at `(35 sin a, 35 cos a)` in the models' frame, from 0 to 120 up, its `u` 0.1·i (its texture `FX_SHIELD` wrapped three times about it) and its `v` 1 at the foot and 0 at the top; the pair 30 at the pair 0's place.
- Its run: its band's phase falls by half a turn a second, wrapping from 0 to 1; its flash, an integer from 255, falls by `⌊400·dt⌋` each frame; past its seconds it dies at once, its loop 2040 with it.
- Its draw, the depth tested and not written, unlit, the culling off: first `FX_SHIELDGROUND` (an arc of a radius of about 0.875 of its half side on black) flat at the bearer's root, the half side 41, white, turned by `2π·phase + π − 0.1`, added whole; then the cylinder, added whole, the feet of the pairs `k`, `k + 1`, … (modulo 31, `k = ⌊31·phase⌋`) in the greys 255, 223, 191, 159, 127, 95, 63 and 31 and the rest black, each plus the flash, held at 255, and every top in the flash's grey.
- So a band of eight columns lights the cylinder's foot and fades up each column, its head stepping clockwise on the screen a turn every 2 s over the ground's arc beneath it.
- An arrow that its effect `0x19` keyed by 22 turns aside is ended and flashes it (`0x5B6620(8)`, its system's `+0x74` bit 8): the whole cylinder lit, fading over some 0.6 s.
  The FX event's case (`0x5A4AEF`) also flashes or ends it where its `+0x38` is set, which no maker read sends (**hypothesis**: the network's copies).

**The shield wall.**

- Its strip is the holy shield's but 70 high, its `v` 1 at the foot and 0.02 at the top, every vertex the opaque (255, 128, 128); its glows' square of the half side 10, flat, of the corners (255, 128, 128), (255, 64, 128), (255, 128, 64) and (255, 64, 64).
- Its run: built by 30 columns a second up to 30, when it asks its action 6 (the effects 7 to 10, 1 s after the act); standing until its seconds, its flash, a float set to 1, falling by 0.75 a second; then unbuilt by 30 a second and gone, 1 s after its seconds; its loop 2022.
- Its draw, the depth tested and not written, unlit, the culling off: at the foot of each of its `⌊build⌋` built columns `PARTICLE_GLOW01` flat, its half side scaled by `2 + 0.3 cos t`, added by its alpha; then its strip's first `⌊build⌋` segments twice, in `RUNESHIELD_GRADIENT` scaled up by `1 + 0.2 cos 3t + flash` and in `RUNESHIELD_GRADIENT2` by `1 + 0.2 sin 2.6t + flash`, added whole.
- So the walls rise column by column from the front counterclockwise on the screen over a second, bob between 56 and 84 high out of step, and double their height at once at an arrow turned aside by its effect `0x19` keyed by 30, or a spell by its effect `0x1A` (`0x5622D6`).

**The mana's spring and the regeneration.**

- One class but for their textures, their colours, their turns and their sounds: a symbol (`FX_MANAREGEN`, blue on an alpha mask; `FX_HPREGEN`, pale pink-white) flat at the bearer's root, the half side 40, white, turned by `−0.5·t` (clockwise) or `+0.5·t`, added by its alpha; then five flat `PARTICLE_GLOW03` of the half sides 100, 90, 80, 70 and 60 at the heights 0, 3, 6, 9 and 12, unturned, in (143, 143, 255) or (255, 79, 79) at the alpha 64, added by their alpha.
- Their run: the clock, and past their end they die at once; their start asks the action 6 (the effects 11 and 12); 835 loops 2028, 836 plays 2042 once.
- The effects' adder also makes an 835 of 7.5 s for the Mage of the set 32's seventh piece, as it takes the effect 40 anew (`0x582D50`'s case 40, `0x57AF53`); no adder's path gives an 836.

`opensacred-game` draws the four since #1092 (`shields`, `opensacred_rules::particles::{HolyShield, ShieldWall, Regen}`), as `spells` casts the spells and the ice elf's creature cast its holy shield, at their bearer's root for the spells' time, on the layer that the night leaves untinted, the two shields with a light of 300 that goes with them; an arrow that the hero's holy shield or shield wall turns aside flashes it (`arrows`, `shields::Flash`).
**Seen** 2026-11-06 beside the original at the Seraphim's and the Mage's starts at 22:00 and the zoom 2: the holy shield's pale blue band sweeping about her over its arc on the ground, the shield wall's red walls over their bright ring of glows, and the mana's spring's blue symbol over its glow, alike in both.
Not followed: a spell turned aside by the shield wall, which the game turns aside nowhere yet, and the set 32's spring.

## The Wood Elf's sights

Read 2026-11-06 for #1092 in Ghidra and by scans of the executable (**confirmed** by code unless marked).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The changeling (36) | 845 `FX_CHANGELING` | `cParticleSystem_changeling` (`0x8983EC`) | `0x795B30`, `0x795E20`, `0x795BC0`, `0x795C00` |
| The speed (37) | 846 `FX_HIGHSPEED` | `cParticleSystem_highspeed` (`0x8984BC`) | `0x79A750`, `0x79AB70`, `0x79A840`, `0x79A8F0` |
| The plants' healing (42) | 851 `FX_NATUREHEALING` | `cParticleSystem_naturehealing` (`0x898520`) | `0x79BE50`, `0x79C1D0`, `0x79BEF0`, `0x79BF60` |
| A soul's mark (41), and other marks | 779 `FX_TEMPTEX` | `cParticleSystem_temptex` (`0x897990`) | `0x76DA70`, `0x76DD40`, `0x76DAC0`, `0x76DB40` |
| The multi-shot's sight (1005), and 1061's ring | 861 `FX_ELVE_SM_CAST` | `cParticleSystem_elve_sm_cast` (`0x8983C0`) | `0x7950A0`, `0x795880`, `0x795140`, `0x7952C0` |

- Only 846 calls the light manager: a Direct3D point light of (0.2, 1, 0.2) for each of its two objects, moved to the system's place at each draw.
  None sets a world light; none of the billboards passes the tint flag `0x40`, so 845, 846, 851 and 861 keep their colours by night, and 779 alone is tinted.
- **The changeling (845).** As the local hero's spell 36 changes a creature (`0x525E5A`), or as its effect 20 ends and the taker changes it back (`0x587F91`), an object follows the creature at its neck.
  Its start bursts up to 500 `PARTICLE_SPARK06` sparks at the skinned vertices of the creature's model as it stands (every `⌊V/500⌋`-th of its `V` vertices), in (128, 255, 128), added whole, of the size 5 ± 2 shrinking by 2 a second, never ageing.
  At the change they fall (the buoyancy 150 ± 30, the velocity ± (3, 3, 0)) and bounce at the ground; at the change back they rise ever faster (the buoyancy −150 ± 30).
  The change's start gives the effect 20 of one of the 14 animal types at random and plays 2064 once.
- **The speed (846).** The spell 37 makes two objects at the bearer's root, riding with it, for the record's value 0 (19 + L s, at most 60), each its loop 2010.
  Each emits squares drawn with the identity matrix in place of the camera's, so that they lie flat on the ground about the bearer's feet, turned at random, added whole in a ramp of their life of 1 s, born within ± 5 and drifting ± 5 a second.
  The first: `PARTICLE_TWIRL02` every 0.1 s, of the half side 40 ± 1, its ramp green, yellow, green, cyan, deep blue, sea green, blue and near white.
  The second: `PARTICLE_DOTS02` every 0.25 s, of the half side 30, its ramp at half brightness from dark blue through teal and green to grey.
  Both grow from 0.2 of their size by 0.66 a second; both go at once at the spell's end.
- **The plants' healing (851).** The spell 42's object follows the bearer's root for 3 s, emitting one particle every 0.01 s (times the detail's factor), left in the world.
  A particle rises (the buoyancy −35 ± 10, the velocity (0, 0, 15) ± (15, 15, 5)), born within ± 12 of the root, of the half side 3, in a colour at random about the grey 128 (± 127 each channel), living 1.457 s.
  It is drawn as `PARTICLE_BUTTERFLY01`'s 64 frames by its age, laid over by the alpha of its life, then `PARTICLE_GLOW03` in (80, 80, 80) added whole.
  The object asks the heal (the action 3) at its start; the executor plays 2060 at the bearer.
- **A soul's mark (779).** The spell 41 lays at each soul's starting point (`0x526B49`) a square of `FX_STREAKS01` (a black blot with streaks raying out, on an alpha mask) of the half side 30, flat on the ground at a turn at random, laid over by its alpha in the light's colour, for 10 s, its alpha then falling by 0.25 a second.
  Its kind 6 puts it in the list 0, under what stands on the ground.
  The same class lays a creature's blood under its burst of `VIOLENCE` (60 s, the half side 45) and the body parts' (30 s, 25).
- **The multi-shot's sight (861).** The art 1005 makes it at the Wood Elf's `Bip01 L Finger3` for the art's seconds (20 s).
  `min(L + 1, 6)` comets circle the hand on axes at random at 5 radians a second: each a `PARTICLE_SPARK07` star of the half side 5 and 20 `PARTICLE_GLOW03` glows behind it of the half sides `k/15`, in (128, 255, 128), added whole.
  At its first frame 200 `PARTICLE_SPARK06` sparks burst out at up to 60 along each axis and are pulled back to the hand (250 a second), from (127, 255, 127) to black over 0.64 s.
  A `Rundum` with a bow (1061) makes it for 1.3 s at the shooter's root: a horizontal ring of 50 `PARTICLE_GLOW03` glows through the left hand's place, biggest at the hand, and a `PARTICLE_SPARK07` star of the half side `25·(1.3 − t)` there, all shrinking away.

`opensacred-game` lays the souls' marks since #1092 (`marks`, `opensacred_rules::particles::{Mark, SOUL_MARK}`), at each soul's starting point on the ground, on the frame that the night tints; seen in the game as dark blots under the souls' starts.
Not followed yet: the changeling, the speed, the plants' healing and the multi-shot's sight, and the blood's and the body parts' marks (#1935).

## The burning light and the holy conversion

Read 2026-11-06 for #1092 in Ghidra (**confirmed** by code unless marked).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The burning light | 816 `FX_BURNINGLIGHT` | `cParticleSystem_burninglight` (`0x897E64`) | `0x77F5C0`, `0x77FA60`, `0x77F6B0`, `0x77F840` |
| The holy conversion | 806 `FX_HOLYCONVERT` | `cParticleSystem_holyconvert` (`0x897F20`) | `0x7827D0`, `0x782EF0`, `0x782A60`, `0x782AD0` |

- The burning light (200 particles, `PARTICLE_SPARK06`, no light): the FX event `0x2B` follows its creature at its neck (3), else its root.
  Its makers: the Seraphim's irritation (18) at the act, for the record's value 0 times its factor (docs/re/arts.md); every knock-back, the creature's event 3 (`0x531BB6`), for 3 s; the Dwarf's 1083 on its held target, its flyer's blows, the Gladiator's 1003 at each creature it stuns, the stun of 862 (0.5 s) and the laser stare of the types 203, 351 and 352.
  Its start: its spread the half of its creature's size (`items.pak +0x14`, 50 for none); its seconds, at least 0.5; for the irritation its action 7, the hold.
  Its run, skipped for a frame longer than a second: until its seconds, each time its timer of 0.01 to 0.2 s runs out, 30 sparks burst together at a point within its spread across and 10 up and down of its place, falling (a buoyancy of 30), of the size 4 ± 2 shrinking by 2 a second, turned at random, flying ± 17 on each axis, living 1.02 s, left in the world; it ends with its last spark.
  Its draw: the sparks turned and added by their alpha, from (0.7, 0.7, 1) to black over their life, untinted.
- The holy conversion (40 particles, `PARTICLE_GLOW03`; `GLOW01` loaded and never drawn; no light): the Seraphim's conversion (19) alone, at a target of no big type; its FX event `0x25` follows its creature at its head (20), standing there each frame with the bone's turn.
  Its start: its seconds the record's value 0, its action 7, the hold; its forty glows at `a = 2πi/40`, at `(10 − 0.2·(5 cos a + 2), 5 cos a + 2, 5 sin a)` in the head bone's axes, a ring of 5 about a point 9.6 along the bone's `x`, over the head.
  Its run: past its seconds its scale grows by 10 a second and its fade falls by 1; it ends faded.
  Its draw, unlit, added whole, untinted: at each glow a quad facing the camera of the half side of its scale in (100, 100, 255) times its fade, and at the tenth, the twentieth and the thirtieth one of eight times its scale in a grey of 64 times its fade: a halo over the creature's head.

`opensacred-game` draws them since #1092 (`holding_sights`, `opensacred_rules::particles::{BurningLight, HolyConversion}`, `spells::holding_sight`): the irritation's light and the conversion's halo as `spells` holds their target, and the light on every creature that a blow knocks back (`making::Knocked` inserted), for 3 s; the halo's offset taken in the head bone's glTF axes, the original's `y` its `z`.
**Seen** 2026-11-06 beside the original: the irritation's bursts of white and pale blue sparks about the goblin, and the conversion's white halo over its head, alike in both, the game's halo a little bluer.
Not followed: the Dwarf's 1083's and the Gladiator's 1003's own lights, the stun of 862's and the lasers' stares.

## The mortar's impact

Read 2026-11-06 for #2160 in Ghidra (**confirmed** by code unless marked).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The mortar shell's impact | 1020 `FX_GRENADEIMPACT` | `cParticleSystem_grenadeimpact` (`0x898CDC`) | `0x7B67E0`, `0x7B70A0`, `0x7B6BE0`, `0x7B6DD0` |

- Its constructor (300 particles) loads `fx_dwarf_explosion1` to `4` and its crater, `fx_krater04` or `fx_krater03` one in two, and lays out 75 tongues: each a quad turned on from the one before by up to a whole turn, its near edge from (−10, 10) to (10, 10) on the ground and its far edge from (15, 25) to (−15, 25) at a height of 21 to 33, across scaled by 0.3 to 1, of a cell of `explosion4`'s 2 × 2 grid at random.
- Its maker is the mortar's burst 1018's action 1 (`0x5B40C4`), at the shell's point on the ground 0.775 s after the burst; the FX event `0x97` places it there.
- Its start shakes the screen by 8 pixels (`cEventWeather` 7, the value 24) and plays 2115; no light of either kind; the hit flag `0x400` is its object's strike (docs/re/arts.md).
- Its run: while it bursts, its flash's half side grows by 200 a second and past 70 its alpha falls by 4, its cloud's grows by 170 and past 25 falls by 10, its tongues' scale grows by 4 and past 1.3 their alpha falls by 4; at its first run 210 sparks burst at its place, each of its block by a roll of 211 against their counts (150, 30, 30): the first of the size 3 ± 1 flying up at (0, 0, 150) ± (100, 100, 50), living 0.64 s, the others of the size 2 ± 1 turning, flying up at 130 ± 40 and falling (a buoyancy of 300 ± 50), living 1.02 s, bouncing on the ground; with all faded and the sparks gone (about 1 s) it waits until 10 s from its start, then its crater's alpha falls by a quarter a second and it ends (about 14 s).
- Its draw: the crater flat on the ground, of the half side 50, laid over by its alpha and tinted by the night; while it bursts, `explosion2`'s flash flat on the ground added, `explosion1`'s cloud laid over, untinted; the tongues of `explosion4` scaled about its place, laid over and tinted; the sparks of `explosion3` facing the camera, each of its layer's frame, the first layer added and the others laid over (the byte `b` 0), untinted, the first's ramp from white through orange and grey to nothing, the others' white fading.

`opensacred-game` draws it since #2160 (`grenade_impacts`, `opensacred_rules::particles::GrenadeImpact`) where the Dwarf's shell lands (`guns`), on the ground there.
**Seen** 2026-11-06 beside the original: the Dwarf's mortar shell's flaming ring, dark cloud and crater at a goblin, alike in both.
Not followed: the sparks' roll of 211 that falls to an unset fourth block, dropped.

## The teleport

Read 2026-11-06 for #1092 in Ghidra (**confirmed** by code unless marked).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The departure and the arrival | 774 `FX_TELEPORT_OUT`, 775 `FX_TELEPORT_IN` | `cParticleSystem_teleport` (`0x897A40`) | `0x771370`, `0x771E20`, `0x771410`, `0x771C10` |

- One class for both (500 particles, `PARTICLE_SPARK02`, its world light 170 constant; no Direct3D light, no sound of its own); the spell 1's executor (`0x522742`) alone makes them at the act, 775 at the arrival on the ground (the FX event 3, placed there) and 774 at the caster (the event 2, following him at his root, carrying the arrival), both of the cast's rate `s` (the caster's `+0x3E6` times 0.01).
  The spells 9 and 10 make none: the black magician's blink makes the object 856 (docs/re/ai.md).
- Its start: the arc and the height 0, its corners' blues `0xFF5566FF`, `0xFF6655FF`, `0xFF6666FF`, `0xFF5555FF`.
- Its run, by the time times `s`, skipped for a frame longer than a second: at its first, each spark on the ring at the angle 0 at its own rate of ± (0.5 to 3) a second, its bob's phase at random; each frame a spark on the ring at `(30 cos φ, 30 sin φ, 50 + H sin θ)` about its place, `φ` held within the arc and `θ` running at 1 a second, and one of 5001 breaking away at 10 to 20 a second outward from its place, falling by 90 a second, bouncing on the ground at 0.9 and shrinking by 2 a second from 7 until it is put back on the ring.
  Its stages: the arc opens by 6 a second to 2π, the height rises by 25 a second to 50, it holds 0.3 s, when the departure's puts the hero at the arrival (the event 8, `0x52F206`) and stays where he was and the arrival's rides on him; 0.3 s more; the height sinks and the arc closes as they came; it ends, `6.69/s` s after the act (the hero put at `3.35/s`).
- Its draw: its sparks of the half side 7, the loose ones' their life, now and then 10 to 12 more, facing the camera and added by their alpha in its corners' colours, untinted.

`opensacred-game` draws them since #1092 (`teleports`, `opensacred_rules::particles::TeleportSight`) at the Mage's feet and at the arrival on the ground as his teleport acts (`spells`), the hero put there by `spells`'s own timer at the same moment.
**Seen** 2026-11-06 in the game: the two blue rings opening, rising into cylinders, and closing; not beside the original, whose driver's right button on the ground did not make the Mage's teleport.
Not followed: the sparks' fixed turns (drawn unturned); the fog of the world map revealed about the arrival.

## The healing

Read 2026-11-06 for #2149 in Ghidra (**confirmed** by code unless marked).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| A heal | 784 `FX_HEALING` | `cParticleSystem_healing` (`0x897B00`) | `0x773F30`, `0x774330`, `0x773FC0`, `0x774120` |

- Its constructor (400 particles) loads `PARTICLE_SPARK02`; no light of either kind.
- Its makers send the FX event `0x10` (`0x5A49E6`: following the target at its root, the start's flag `0x200`): the spell 7's executor (`0x523A36`), the hero's or a healer's, at the order's object, else the caster; and the goblin mages' ball at each creature it lands among not hostile to its caster (`0x5A7336`), of the spell 7's record at ⌊caster's level / 4⌋, at least 1, its factor 1.
- Its start: a layer of the life −104, the size −8 and the angle +3 a second; its corners' colours, a hero's `0xFFFF0000`, `0xFFFF7F7F`, `0xFFFF0000`, `0xFFFF0000`, a creature's `0xFF8080FF` each; its sound 2020 once.
- Its first run: all its 400 particles on a ring of 20 about its place, each at an angle at random, the velocity (0, 0, 2), a buoyancy of −(80U + 10) (rising ever faster), the size 20 and an angle at random; and its action 3, the heal, once (`0x5B53A3`): the record's value 0 at its level times its factor times `1 + lore/100`, the caster's magic lore, given to the target up to its most health, with the effect 30 for 1 s keyed 100 (**hypothesis**: the end of a poisoning); none where the caster or the target changed since the event.
- Its run ages them and ends with them (2.45 s); its draw turns them and adds them by their alpha in its corners' colours, whatever their life, untinted.

`opensacred-game` draws it since #2149 (`healings`, `opensacred_rules::particles::{HealingSight, healing_corners, ball_heal_level}`, `fx::Quads::push_facing_turned`): the hero's heal and a healer's, and the goblin mages' ball's heal of the creatures it lands among not hostile to its caster (`creature_casts::Ball`, by `spells::healing` with no magic lore, as a creature's heals are), the sparks left in the world where they were born.
**Seen** 2026-11-06 beside the original: the Mage's heal as a white disk at his feet rising into a pink column and its sparks, alike in both.
Not followed: the heal's effect 30.

## The summoning

Read 2026-11-06 for #1092 in Ghidra (**confirmed** by code unless marked).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| A summoning | 777 `FX_BIGSUMMON` | `cParticleSystem_bigsummon` (`0x897874`) | `0x7699A0`, `0x76A440`, `0x769C60`, `0x769FC0` |

- Its constructor (20 particles) loads `FX_TRIBALS3_VAMP`, `FX_TRIBALS3_VAMP_G`, `FX_TRIBALS3_ELVE` and `FX_TRIBALS3_ELVE_G`; no Direct3D light.
- Its makers send the FX event 4 (`0x5A3BE8`: at the event's point, following nothing), its `+0x38` the creature type, its `+0x3C` the colour of its caster's type (`0x427130`: the Wood Elf's (5) `0x40FF40`, the Vampiress's (6, 7) `0xFF4848`, any other's `0xE749FF`): the spell 3's executor (`0x523413`), the heroes' and the creatures' above the id 16, and the Vampiress's wolves 1026 and 1027 (`0x512B39`).
- Its start: its middle's colour at the alpha `0xAA`, its rim's at none; its sound 2002 (docs/re/sound.md).
- Its run, skipped for a frame longer than a second: its clock by the time, its phase by twice it; at its first run each particle's azimuth and elevation of a whole turn (6.28) at random and their rates of 1 to 3 a second either way by an odd roll, and ten bolts' steps of ±30 along each axis; each frame every particle at `80·(1 − cos φ)` along its angles from the middle (the phase before the step), the angles on by their rates within 6.29; its world light `⌊100·(1 − cos φ)⌋`; its bolts while φ lies within 2.9 and 3.4; once φ passes 3.0 its action 1 makes the creature (docs/re/arts.md); it ends as φ passes 6.28 (3.14 s).
- Its draw, `k = 1 − cos φ`: for a hero's summoning (`+0xA9` 0) its runes flat on the ground (the Vampiress's for her colour, else the elves'), the plain pair black at the alpha `⌊127k⌋` laid over by its alpha, of the half sides 55 turned by its clock and 40 against it, then the glowing pair grey at that alpha added whole; ten triangles from its middle 20 over its place to two particles each, laid over by their alpha with no depth test and no texture; while they show, ten white bolts of six points from the middle, a step its bolt's and ±10 more along each axis rolled each frame, alike; two glows of `PARTICLE_FLARE04` (`0xFFFFA0A0`) at the middle added whole, of the half sides `20k` and `15k²`.

`opensacred-game` draws it since #1092 (`summon_looks`, `opensacred_rules::particles::{BigSummon, summon_colour}`, `fx::plain_over_material`) at each summoning of `followers::summon`, the hero's, and of `creature_casts`, a creature's (its runes the hero's alone), untinted by the night, its light swelling with it; the bolts are thin quads facing the camera.
**Seen** 2026-11-06 beside the original: the Wood Elf's unicorn summoned in a green flare, its rays fanning out over the ground and its green rings of runes under it, alike in both.

## The plant cage

Read 2026-11-06 for #1092 in Ghidra (**confirmed** by code unless marked).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| A cage about a held creature | 849 `FX_PLANTCAGE` | `cParticleSystem_plantcage` (`0x898498`) | `0x799720`, `0x79A610`, `0x799D30`, `0x799E10` |

- It is the thorns' plant's code (848, "The thorns' plants") on another vine: its constructor loads `PARTICLE_GLOW01` and `PARTICLE_GLOW03` and makes a Direct3D light (range 200, the attenuations (1, 0, 0.000125)); no world light; no sound of its own.
- Its makers send the FX event `0x49` (the caster, the target, the target's place on the ground), whose case (`0x5A4CC5`) follows the target at its bone 3 (`Bip01 Neck`), else its root, the start's flag `0x200` standing it there each frame: the spell 40's executor (`0x52634B`), the hero's or a creature's, `⌊min(20, ⌊v₀·f⌋)/5⌋` cages at the target; the hero's terror (`0x4E26F1`), ten about the creature that stands in it, the hero their caster, of the level ⌊hero's level / 3⌋; the set bonus `UI_SET_ROOTEN` in melee (`0x4E074A`, ten at the level 2) and the killer crab 348 (`0x4E58EF`, ten on an odd roll).
  Every maker but the terror's plays `DRYADE_DORNEN` (2061) at every fifth cage, from the first.
- Its rolls: its seven spurs' points as the thorns' plant's; its stem's five rolls, `x = sin(0.1k(U₂ + 0.5))·sin(k/128)·(U₁ + 2.5)`, `y = sin(0.1k(U₄ + 0.7))·sin(k/128)·(U₃ + 3.5)`, `z = k/2`, turned by `2π·U₀` about the upright; its two sways', `sin(0.1k(0.8V₂ + 0.2))·(2V₁ + 1.5)` and `sin(0.1k(0.4V₄ + 0.1))·(V₃ + 3)`, `sin(0.1k(W₂ + 0.5))·(W₁ + 1.5)` and `sin(0.1k(W₄ + 1))·(W₃ + 3)`, each turned and its 64 points mirrored; at its start its base point within 50 of the event's along each axis on the ground, its `y` rolled first.
- Its start: its rise −2; a hero's cage (`+0xA9` 0) green (`0xFF40FF40`, its light (0.25, 1, 0.25)) holding for the record's value 0 times the factor, a creature's blue (`0xFF4040FF`, (0.25, 0.25, 1)) for 3 s.
- Its run: its rise grows by 3 a second to 0 (0.667 s), when its action 7 holds the creature (the effect 15 for the hold's seconds, 3 s for a caster above the id 16, `0x5B5CC8`); it stands its hold's seconds; its rise falls by 3 a second, and it ends at −2.
- Its draw: its light at its base point; a glow of `GLOW03` flat on the ground at the base, of the half side `10·(rise + 2)`, added by its alpha; its vine of `GLOW01` dots facing the camera added by their alpha in its colour, the point `k` the stem's, the sways' times `sin(πk/127)` and the share `k/127` of the way from the stem's end to the base, so that it runs from the neck to the base, of the half side `1 + k/128 + rise` where above 0, so that it grows from the ground up; at each spur's point while the rise is above −1, dots 0.75 apart along the stem's way there, of the half side `1.5 + rise` less 0.1 a dot while above 0.5.

`opensacred-game` follows it since #1092 (`plant_cages`, `opensacred_rules::particles::PlantCage`): the Wood Elf's cages (`spells`), a creature's at the hero (`creature_casts`) and the terror's ten (`ai`), each vine from its creature's neck to its base, its glow and its light at the base, untinted by the night, every fifth cage of a cast sounding; a creature's cage holds the hero 3 s (`spells::holds`, #2154).
**Seen** 2026-11-06 beside the original: the Wood Elf's cages about a goblin, green vines from its neck to their glowing bases on the ground, alike in both.
Not followed: the hold given as the cage has grown (0.667 s on) rather than at once; the set bonus's cages and the killer crab's; the quirk by which a creature's cage whose event's height is exactly 1.0 takes a hero's look.

## The flesh to stone

Read 2026-11-06 for #1092 in Ghidra (**confirmed** by code unless marked).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The stone about a held creature | 822 `FX_STONED` | `cParticleSystem_stoned` (`0x898030`) | `0x785EE0`, `0x786120`, `0x786000`, `0x786030` |

- Its constructor loads the model of the item type 5095 (`freeze_stone.grn`, a mound of about 60 across and 31 high), its texture `FREEZE_STONE01` and `FX_AURA` (never drawn); one particle, never used; no light of either kind.
- Its maker is the spells' executor's flesh to stone alone (`0x5249CC`), the hero's or a creature's cast at a target of no big type; its FX event's kind `0x31` (`0x5A57D5`) follows the target at its root.
- Its start: a turn about the upright at random (`rand()` / 32767 of a whole turn), its life the hold's (`v₀·f` for a hero's cast, 3.0 s for a creature's), the flag `0x200` (it stands at its creature's root each frame), and its action 7 with the stone's texture, which holds the creature (the effect 15, docs/re/arts.md) and sends it the event `0x109` of the state 9 for the hold's seconds.
- Its run ends it at its life's end, at once.
- Its draw: the model at the scale 1 turned by its turn at its place, in the item's texture, lit as the models are, laid over by its alpha with its depth written: a block of stone about the creature's feet.
- The state 9 (`0x44A9D0`'s case 9, `0x44AD50`): the creature's model drawn as ever, then again in the stone's texture by its own coordinates, lit, laid over by its alpha (`0x40D060`), the texture opaque; the items it wears, merged onto it, take it too; the state ends with its seconds (`0x44A520`).

`opensacred-game` follows it since #1092 (`stones`, `skins`, `opensacred_rules::spells::{STONED_MODEL, STONED_TEXTURE, STONED_STATE}`): the hero's stone on a creature (`spells`) and a creature's on the hero (`creature_casts`) put the block at the target's feet for the hold's seconds, following it, and dress its model and the items it wears in the stone: their own materials take the stone's texture, lit, while the hold lasts and give it back, which shows as the original's opaque pass over them.
**Seen** 2026-11-06 beside the original: the Mage's goblin stands grey-olive in stone on its block in both.
Not followed: `FX_AURA`, loaded and never drawn.

## The Vampiress's casts

Read 2026-11-06 for #1092 in Ghidra (**confirmed** by code unless marked).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The Vampiress's casts' sights | 863 `FX_VAMP_SM_CAST` | `cParticleSystem_vamp_sm_cast` (`0x898328`) | `0x7915C0`, `0x791FB0`, `0x7918D0`, `0x791AB0` |

Its constructor (200 particles) loads `FX_TRIBALS2_VAMP`, `FX_TRIBALS2_VAMP_G`, `PARTICLE_SPARK07`, `PARTICLE_GLOW03`, `PARTICLE_MULTI01` (2 × 2), `PARTICLE_SPARK06` and the model `cast01` (its bone `particle_source_01` and its clip `FX_D_IDLE_BH`, 3.98 s), and makes a Direct3D light (0.8, 0.8, 1.0) for every object, which only the case 0's draw moves (to its ribbon's head); the case is the art at its event's `+0x3C`.
- The case 1, the bats (1035 to 1037): the executor (`0x50E6C0`, its running phase: `0x510B89`, `0x51190A`, `0x51210B`) makes one once what is left of the art's clip falls below 0.87, at the called bat's place, following nothing.
  Its start bursts ten sparks at once: a buoyancy of 180 ± 40 (they fall), a size of 6 ± 1, the velocity (0, 0, 100) ± (20, 20, 10) and the place ± 10 on each axis, living about 1 s (the life's rate −250) and shrinking by 4 a second, in a ramp worked out entry by entry, the entry `i` (i, ⌊i/4⌋, ⌊i/4⌋) (the rounding **hypothesis**), from (255, 63, 63) to black.
  Its run turns its runes by 2 a second and fades them from 1 by 0.75 a second; it ends with its last spark, so that its runes are cut off at about a quarter of their alpha.
  Its draw: four squares flat on the ground at its place (`0x7631E0`), `FX_TRIBALS2_VAMP` of the half sides 18, turned by the turn, and 30, against it, white at the fade's alpha, laid over by their alpha, then `FX_TRIBALS2_VAMP_G` alike, grey at the fade, added whole; then the sparks added (the flags `0x19`), each with its trail: copies a hundredth of a second apart back along its way, its fall undone at each step, of the half side `min(size, 0.63)` less 0.03 a step, up to 21, a young spark's stopping where its life stepped back would pass its birth; none tinted by the night.
- The cases 2 to 4, the claws (the night's `Rundum` 1038, `Hardhit` 1039 and `Attacke` 1040): the executor's start of the art (`0x513E3B`) makes two in the tick its clip starts, at the bones 22 and 21 (`Bip01 R Finger3`, `Bip01 L Finger3`, the names a **hypothesis**), each standing at its bone each frame (the flag `0x200`) and gone with its owner.
  Each makes a glow of the size 6 every 0.01 s (times 1.5 and 2.2 at the detail levels 1 and 0) until 2.2, 0.7 and 1.5 s, left where it is born, of the life −400 and the size −20 a second, so gone in 0.3 s, in the red ramp of the case 1; drawn `PARTICLE_GLOW03` added (the flags `0x19`), untinted.
- The case 0, the change (1022 and 1023): the executor's start of the art (`0x51431A`) makes four in the tick its clip starts, at the caster's root once (the flag `0x40`), of the angles 0, π/2, π and 3π/2 (`+0x2C`); each samples the bone `particle_source_01` of the model `cast01` on its clip at each run, turns the point about the upright by its angle, and makes its three blocks' particles along that way (block 0's interval 0.01 s; the sizes 8 ± 1, 8 ± 1 and 2 ± 0.3, the velocity 0 ± 12 on each axis, the frame its block's of `MULTI01`), of three layers (the life −260 and the size −1, the life −320, the life −150) in three ramps from magenta to orange to nothing; at the ribbon's head while its clock is within the clip's length a red glow (`GLOW03`, half 25) and a white spark (`SPARK06`, half 12), added; the object of the angle 0 alone lights the night by 250 at the caster's cell.

`opensacred-game` draws them since #1092 (`vamp_casts`, `opensacred_rules::particles::{VampClaw, VampBats, VampRibbon, spark_trail, vamp_red}`): the claws' glows as the night's arts' clips start (`making::Playing` added), at her fingers, else her hands; the bats' sight at the bat's place as `followers` makes it; and the change's four ribbons as its clip starts, about her place, each along the bone `particle_source_01` of the model `cast01` playing its clip once in a scene of its own, the models' frame taking the glTF's (x, z, y), its particles born along the head's way in the frame (**estimate**, as the goblin mages' ring's), its head's glow and spark and its pale blue light, the first ribbon lighting the night by 250; all untinted by the night.
**Seen** 2026-11-06 in the game: the Vampiress's night `Attacke` with red glows trailing from both her hands, and her bat's call with the red rings of runes on the ground under its sparks; beside the original, her change's ribbons alike, chains of magenta glows with bright heads rising over her head and sweeping about her, turning orange, with golden stars among them.
Not followed: the cases 1 to 4's Direct3D lights (they stand at the world's origin); the ribbons' end read from the model's control (the clip's length 3.98 s taken).

## The goblin mages' ball

Read 2026-11-06 for #2147 in Ghidra (**confirmed** by code unless marked).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| The ball's impact | 786 `FX_GOBLINIMPACT` | `cParticleSystem_goblinimpact` (`0x897B40`) | `0x774670`, `0x774C40`, `0x774730`, `0x774910` |

- Its constructor (200 particles) loads `PARTICLE_LIGHTNING01` (a 2 × 2 grid of white bolts), `PARTICLE_GLOW03` and `PARTICLE_LINE01`; no Direct3D light; its world light 200 for every variant.
- Its makers are the goblin mages' ball's landing alone (docs/re/arts.md, "The creatures' own spells"): the variant 0 at each hostile creature it strikes, at its place on the ground; the variants 1 to 3 at its own place where it lands on no creature, by its caster (2 the god 144, 3 the undead goblin mage 143, else 1).
  Its FX event's kind `0x12` (`0x5A3BE8`) puts it at the event's point, following nothing.
- The variant 0, the hit: one layer of the life −256 a second (about 1 s), one block of a size 30 ± 8 turned by π ± π, one of the grid's four frames at random, one every 0.01 s, no velocity nor offset; its ramp pale blue (0.6, 0.8, 1.0), its alpha from 0 to 1 over 10 entries and back to 0 over 15, nothing over the last 231, so that a crack shows over the first tenth of its life.
  It makes them while its glow's half side (`+0x20AC`), from 20 falling by 5 a second, stays above 10, and ends at once at 10 (2 s).
  Its draw: each crack a quad flat on the ground (the identity matrix, the flags `0x18C`: its frame, anchored at its foot, its ramp's colour, turned), from its place out to twice its size along its angle, its size to each side, laid over by its alpha; and while the half side is above 0 a flat square of `GLOW03` of that half side in (160, 176, 255), added whole: lightning cracks radiating on the ground about the struck creature under a fading glow.
- The variant 1, the miss: one layer of the life −100 (2.55 s) and the size −10 a second, one block of a size 40 ± 10 every 0.004 s; its ramp blue (0.39, 0.39, 1.0), its alpha from 0 to 1 over 10 entries, held over 200, to 0 over 46.
  While its turn (`+0x20A4`), growing by 10 a second, stays below 2π, it sets the block's offset to the turn's point of a circle of 15 about its place and makes its streaks there, so that it traces the circle once in 0.628 s; once the turn passes π (0.314 s) its action 1 makes its creature (76, 145 or 142 by the variant; docs/re/arts.md); it ends with its last streak.
  Its draw: each streak a quad upright in the frame's `x`–`z` plane (turned about `x` by π/2, the flags `0x89`: anchored at its foot, its ramp's colour, added by its alpha), an eighth of its size to each side and twice its size high: a ring of blue light streaks traced once about the spot.
- The variants 2 and 3 emit nothing and draw nothing: they only summon.

`opensacred-game` follows it since #2147 (`goblin_impacts`, `opensacred_rules::particles::{GoblinImpact, GoblinLanding}`, `creature_casts::Ball`): the ball's hit on the hero cracks the ground under her, and its landing on no creature traces the ring and summons the creature 0.314 s on, hostile and of the caster's level and side (`creature_casts::Summoning`), the impacts untinted by the night and lighting it by 200.
The ring's streaks are born along the turn's way in the frame, where the original sets the block's offset to the turn's point once a frame and a frame's streaks stand together (**estimate**: at the game's frame rate they stood apart as bright bars).
**Seen** 2026-11-06 beside the original: the miss's column of blue streaks alike in size, brighter at its right edge and its foot in both; in the game the hit's cracks radiating from the hero's feet.
Since #2149 its heal of the caster's friends follows ("The healing").
Not followed: its strikes on the hero's followers (the game's ball strikes the hero alone), the summoned creature hired by its caster (it is made hostile, of the caster's side, as a creature's summons are), the type 145's item `0x73A` in its slot 13, and the event's sound.

## The guns' sights

Read 2026-11-06 for #1092 in Ghidra (**confirmed** by code unless marked).

| Object | FX type | Class (virtual table) | Its constructor, start, run, draw |
|---|---|---|---|
| A gun's flash | 1172 | `cParticleSystem_gunfire` (`0x89906C`) | `0x7C3200`, `0x7C3AC0`, `0x7C38C0`, `0x7C38F0` |
| A shot's impact | 1151 | `cParticleSystem_shoot_impact` (`0x898D78`) | `0x7B9020`, `0x7B9180`, `0x7B90B0`, `0x7B9110` |

- The flash is made by the shot's maker (`0x4DCB80`) for any shooter that holds a gun, as a shot's clip starts (`0x4DD1FE`) and as each shot repeated starts its own (`0x4DF442`, `0x4DFE85`), and by the Dwarf's executor (`0x514790`) at each of his `Rundum`'s strikes (`0x515E27`), his `Hardhit`'s point (`0x515298`) and each of his `Attacke`'s points (`0x514E8F`) with a gun.
  Its point is the gun's bone 48 (`Canon_fx01`), its direction that bone's `y` axis, the barrel's way; its FX event's kind `0xAF` places it there and follows nothing.
- Its constructor builds 24 lit vertices, four quads in its own frame whose `y` is the shot's axis: a narrow flame of (255, 176, 159) flat (`z` 0) of the half width 5 and upright (`x` 0) of the half height 10, from 2.5 behind its place to 12.5 ahead, and a wide one of a grey of 160 of the half sides 10, from 5 behind to 25 ahead; the texture's top, the flame's tip, ahead.
  One corner of the narrow flat quad lies 12.5 behind in its table, read as a slip of its 2.5 (**hypothesis**).
- Its start chooses one of `particle_gunfire01` to `04` by `rand() % 4`, plays its sound (docs/re/sound.md), scales it by the gun's type (the event's `+0x38`: 1855 2, 1856 0.7, 1857 1.4, any other 1) and turns its `y` onto the event's direction; its world light 300, and a white Direct3D light (1, 1, 1) 5 ahead of it along the shot (**hypothesis**: `+0x2C` times 5).
- Its run lives 0.1 s and moves nothing; its draw, depth tested and not written, unlit and untinted by the night, adds the texture times the vertices' colours whole (`ONE`, `ONE`), no face culled.
- The impact is made only by the Dwarf's executor: for each creature that his `Rundum`, `Hardhit` and `Attacke` strike with a gun, right after their flash (`0x515F04`, `0x515372`, `0x514F67`), unless the struck object's `+0x14` has the bit `0x10` (not read); and for each hostile creature near his cannon's (1087) line (`0x517A10`).
  Its FX event's kind `0x9C` follows the struck creature at its bone 3 (`Bip01 Neck`), the system's flag `0x40` placing it there once; its direction the gun's bone's `y` axis, or for the cannon the line's far end less the creature.
- Its start bursts all its 200 particles of `particle_dots03` at its first run: a buoyancy of 30 ± 5 (they fall), a size of 5 ± 1 growing by 4 a second, turned by π ± π, the velocity 120 along the shot ± 40 on each axis and the place ± 5, drawn back to its place by 80 a second, living 0.73 s (the life's rate −350); laid over by their alpha, turned and tinted by the night (the flags `0x4C`), in its red ramp (`+0x3C` 0) from (254, 12, 0) opaque to (164, 3, 0) at 164 over 124 entries, to (137, 3, 0) at 139 over 61 and to nothing over 71; no light; it never ends itself, drawing nothing once its particles are gone.
- An ordinary shot of a gun, the hero's or a creature's, makes the flash alone.

`opensacred-game` draws them since #1092 (`gun_fires`, `opensacred_rules::particles::{gunfire_quads, gunfire_scale, shoot_impact}`): each `sfx::Gunfire` of a shooter holding a gun flashes at its muzzle, of its gun's scale and one of the four textures, lighting the night by 300 and the models white ahead of it; it keeps to the muzzle and turns with the barrel while it shows, where the original leaves it where it was made (docs/decisions/drawing.md, "The guns' flash at the muzzle").
Each strike of the Dwarf's `Rundum`, `Hardhit` and `Attacke` with a gun, and each creature his cannon strikes, sprays the impact at the struck creature's neck, else its place (`puffs`, its pull `Layer::pull`).
**Seen** 2026-11-06 in the game: the Dwarf's pistol's flash at its muzzle, turning with it as he raises it to shoot a goblin (a flash held 3 s in a scratch build, to see it whole); the red spray at the goblin's neck at his `Attacke` and his cannon.
Not followed: the impact's test of the struck object's `+0x14` bit `0x10`; the cannon's own flash (1017, `dwarf_shoot`).

## The arrows' elements

Read 2026-11-05 for #2061 in Ghidra (**confirmed** by code unless marked otherwise).

- **The element.** As an arrow is shot (`0x5BD2D0`, its handler of the event 9) it takes from its shooter's weapon record (`0x5BEB30`, each kind's most damage at `+0x24` to `+0x2A`) one element: the largest of the fire, the poison and the magic where it is larger than both others (`+0x248` 0, 1 or 2; 3 where all three are 0), its share that most over twice the physical most, held to 0 … 1 (`+0x1B0`, `+0x1B8`, `+0x1C0`).
  An element shows where its share passes `+0x1DC`, 0.1 for every item (`cItemBase`'s constructor `0x5BE130`): a fire makes an object 796 (`FX_MAGICFIRE`, the FX event `0x1C`, its variant 1) kept at `+0x1AC`, a poison an object 808 (`FX_MAGICGIFT`, the event `0x1D`, variant 1) at `+0x1B4`, a magic sets `+0x1BC`; the client logs each.
- **A weapon in hand** (`cWeapon3D::toggleVisuals`, `0x5CDB60`, as its model is put into a hand) takes its element from its own most damage of each kind, whole: the largest of the fire, the poison and the magic where it is larger than both others and than a third of the physical; the fire's and the poison's share `(v − min/2) / (max/3 − min/2)`, the magic's `(v − min/3) / (max/4 − min/3)`, `min` and `max` the balance's `SpawnWeaponMin` and `SpawnWeaponMax`, held to 0 … 1, shown where it passes 0.1.
  It makes the same FX events `0x1C` and `0x1D` on its bones 15 and 16, of the variant 1 (below), and draws the same line of glows.
  The whole switch (`0x5CDB60`, with its argument on) makes first the torch's `0x13` (`0x428150`), else the `0x52` of `0x428360`, else the bonus's `0xB` (`0x5CEA50`) where its model has the bone `stdfx_bone01`, else the `0x33` of `0x428190`, which ends it, else a `0x3C` for each of its model's bones `fx_streak01` on (`0x4282F0`, kept at `+0x1C4` by its model's load `0x5BE5E0`); then the elements' shares and their objects for every one but the `0x33`'s.
- **The objects 796 and 808** (`cParticleSystem_magicfire`, start `0x77A8E0`, run `0x77A650`, draw `0x77A6C0`; `cParticleSystem_magicgift`, start `0x77C3E0`): the events' variant 1 keeps the system at the arrow's bones 0, 15 and 16 (`weapon_fx01`, `weapon_fx02`; their cases `0x5A5212` and `0x5A4FAC`), the system's flags `0x1204` (its points at those bones at each frame, its origin at the arrow's place, so that its particles ride with it).
  Their event's `+0x38` is 1, and its `+0x3C`, which the start takes times 0.001 as its share, is 0 for an arrow (the shot's pushes `0x5BD76E` of `EDI`, which the test of `+0x1AC` just before shows to be 0) and the weapon's share a thousand times, truncated, for a weapon in hand (`0x5CE308`; #2093); one layer of the life −600 a second and the size `15·share + 10`; one block of the size `2·(share + 1)`, every 0.1 s at the detail 2, each spark at a point at random along the segment between the system's two points and the way it swept since the last emission (`0x766200`); drawn as billboards of `PARTICLE_GLOW01` of the flags 3, their four corners' colour (`0x00FF8844`, orange, and `0x0044FF44`, green) at the alpha of their life, added; the fire's light `200 + 200·share` (`0x77A9C7`), the poison's none.
- **The item's own glows** (`0x5BEE90`, after the model's own draw in `0x5BEE60`, the draw of every `cItemBase` that its classes' tables name: the weapons in hand, the items worn, the arrows), read again 2026-11-05 for #2091:
  - for each element shown, a line of glows (`0x40DA80`: glows of a half side `s` facing the camera, `⌊length · d⌋` of them, at least one, from the line's start along it, added by their alpha) over the bones 15, 16 and 17 that the model has, counted: a single glow at the bone 15 for one, from 15 to 16 for two, and on from 16 to 17 for three; the magic's of `d` 0.7, `s` `10·share` and the colour `0x507033FF`, the poison's and the fire's of `d` 0.5, `s` `4·share + 2` and `0x4044FF44` and `0x40FF8844`;
  - at each of its model's bones `stdfx_bone01` to `stdfx_bone10` that its load keeps at `+0x1D0` (`0x5BE5E0`, the names of `0x401010` from `0x8E7840`, while they are found from the first), a single glow (`0x40E2B0`: one glow facing the camera, added whole, the source `ONE` of the render flag 8, where the lines take the source's alpha) of a half side 15 and `0xFF1080FF`: the Seraphim's wings 2 and 4, her lance 4019 and the long swords 1742 and 1851 have one;
  - the type `0xEE1` (3809, the Seraphim's staff `SeraBFG`): a line from 15 to 16 of `d` 0.3, `s` 15 and `0x388888FF`;
  - a type whose entry has the second renderer's effect 9 (`0x426520`, `items.pak +0x6E`; the wings 3111 `SeraWings05`): a single glow at each of its model's bones `sera03_fx0` to `sera03_fx3`, white, of a half side `3 + rand()/32767` rolled at each draw;
  - a lightsaber (`0x4283C0`: the type `0xC00`, or one whose own weapon record's parent `+0x24` is `0xC00`, the ten of 2296 to 2305; not 3073, of the same model): two lines from 15 to 16, of `d` 0.8, `s` 1.5 and `0xFF8888FF`, a thin core, and of `d` 0.1, `s` 10 and `0x408888FF`, a faint halo.
  The lines take the display's `+0xD4`, `PARTICLE_GLOW01`; the single glows its `+0xD8`, `PARTICLE_GLOW03`, and the wings' glitter its `+0xE0`, `PARTICLE_SERA03` (the display's textures loaded at `0x401341` and on: `+0xD8` `PARTICLE_GLOW03`, `+0xDC` `GRN_SHADETEXTURE05`, `+0xE0` `PARTICLE_SERA03`).
- **At its hit** (`0x5BCCF4`), an arrow with its fire's object, of the local player, makes the burning bone 788 at its target unless one burns there (`0x5FA150` with 20 and 1, then 2), and plays `TORCH_HIT01` (708) at it, at a rate's factor of `0.4 + (rand() % 20) / 90` (#2007); the fire skin's hits make one at each target likewise (`0x5AD4F0`, its event of the same zeros).
- **The burning bone** (`cParticleSystem_burningBone`, 788, start `0x775750`, run `0x7754B0`): its event `0x14` keeps it at its creature's bone 20 (`Bip01 Head`, the case `0x5A556E`), the event's `+0x34` and `+0x3C` both 0 from the arrow; its clock starts at `−0.1` times `+0x3C`, and until it passes 0.5 s it makes a flame of `PARTICLE_FIRE01` every 0.02 s within 6 of the bone, of the velocity (5, 5, 15) ± 3, the buoyancy −100 ± 20 falling by 100 a second (rising ever faster), the size 10 for a `+0x34` of 0 (a fifth of it up to 60, else 12) growing by 4 a second, living 0.36 s (−700); its four corners' grey `+0x34 + 35`, white where that is 35 or above 255; drawn of the flags `0x15` and the grid 4 (added whole, turned, its frames by age); its light 150, its system's flags `0x1200`; it ends with its last flame.

`opensacred-game` shows them since #2061 (`arrow_elements`, `opensacred_rules::arrows::{element, element_line}`, `opensacred_rules::particles::ArrowSparks`): `arrows` gives each arrow the element of its shooter's most damaging item (the hero's held items, a creature's arms), and the arrow's look draws its line of glows along `weapon_fx01` and `weapon_fx02` and its sparks riding with it, untinted by the night, the fire lighting it; an arrow model without the two bones shows nothing.
Since #2093 an arrow's sparks are of the share 0 that its event carries (`ArrowSparks::of_arrow`: of the size 2, growing by 10 a second, the fire's light 200), its own share sizing its line alone, and a weapon's of its share in thousandths (`ArrowSparks::of_weapon`).
**Seen** 2026-11-05 in the game: the Wood Elf's bow of 27 to 34 physical and 13 to 16 fire (share 0.24) loosed arrows with an orange glowing shaft and sparks about it; her starting bow of no fire, none.
Since #2061 a hero's burning arrow and the fire skin set what they strike burning too (`burning_bones`, `opensacred_rules::particles::BurningBone`), one at a time on a creature, with the arrow's `TORCH_HIT01`; **seen** 2026-11-05: the Wood Elf's bow of 42 to 53 physical and 32 to 45 fire (share 0.42) set a goblin's head burning in white flames at its hit, and the Mage's fire skin the goblins about her.
Since #2089 a weapon put into a hand shows its own (`arrow_elements`, `opensacred_rules::arrows::weapon_element`): a creature's by the values of its arms, the hero's by those of its weapon set's half for that hand, each of its type's record where the item maker gave it none; its look is the arrow's, along the weapon's `weapon_fx01` and `weapon_fx02`, while the model is in the hand.
**Seen** 2026-11-05 beside the original at the cell 3440,2703: the Wood Elf at level 25 with the bow 1789 of 42 to 53 physical and 32 to 45 fire (share 0.41) held it across her shoulders at rest in both games, an orange line of glows along it of about the same length and width (the original's upright with the bow in another phase of her idle).
Since #2091 the line of an element runs over the third bone too (`opensacred_rules::particles::glow_segments`), and every item's model held, carried or worn draws its own glows (`item_glows`, `opensacred_rules::particles::{own_glows, BLUE_GLOW, wings_glitter}`): the blue glows at its `stdfx_bone*`, by its type (`drops::Tables::own_glows`) the staff's line, the lightsabers' two and the wings' glitter, untinted by the night; a model without the bones shows nothing of them.
**Seen** 2026-11-05 beside the original, the Seraphim at her start by night: the long sword 1742 with a blue glow at its hilt, the wings 3111 with a white glitter at each of their roots, the staff 3809 with a broad lavender glow along it and the lightsaber 3072 with its bright core and faint halo, in both games alike.
  The original's lines of the lightsaber, traced at `0x40DA80`, ran 59.6 from end to end, as the model's 60 between its two bones, its magic's line beside them.

## In `opensacred-game`

The world's fires burn (#1092): `opensacred_rules::particles` holds the machinery (the particles, the layers' rates, the emission blocks with their spreads, the ramps, the update of `0x7640C0` and the steady emission of `0x764760` with its pick of a layer for two) and the smoke class's variants 4 to 6 and 10 to 12, and 13 since #2070 (the magic fire, with the pick of a block among three and every particle at the first block's offset); the scripts' `CreateObj` of `FX_FIRE_L` (884 to 886), `FX_FI_SMOKE_*` (812, 813, 876) and `FX_FIRE_MAGIC` (904) places a fire at its `0x20` and its height `0x7E` in the models' units above the world's zero (`npcs`), and `fx` runs its particles at the frame's time and draws each layer as camera-facing quads of `PARTICLE_FIRE03` (sampled without its smaller levels), the flames added by their alpha and the smoke laid over by its alpha, their ramps' colours written as they are into the frame of sRGB values; each fire lights the night by its radius (280, 370, 530).
Since #2017 every effect's material (`fx::Effect`: the fires, the smokes, the steps' fire, the glitter, the butterflies, the beams, the puffs, the beacons, the stashes' sparks and the bolts) encodes its colour for the frame before an added one takes its alpha (`standard.wesl`'s `PARTICLES`), and its quads' colours go to it in linear light, so that a texel's value times a colour's is laid or added as it is, as the original's device does; Bevy's own material had written linear light into the frame of sRGB values (`srgb`), so that the middle tones of every texture came out dark: the BFG's starburst half as wide to the eye as the original's, the braziers' flames of 50,39 pale and their smoke white where the original's are orange (**seen** 2026-11-04 beside the original by noon, and alike since).
Set beside the original on 2026-10-27 at the braziers of 50,39 by noon: a flame of a yellow core and an orange top with a faint grey smoke, its pixels of red above 200 and green above 150 247 against the original's 239, its brightest (255, 255, 165) against (255, 255, 157), its extent within two or three pixels of the original's.
By night the original's flame keeps its colour (the flags `0x0C` leave out the tint), and so does the game's since #1191: a camera of its own (`iso::Untinted`) draws the systems without the flags' bit `0x40` after the world's camera and its night's pass, into the same frame and against its depth, so that the night's colour leaves them as they are; on 2026-10-27 the braziers of 50,39 by night showed the original's yellow and orange flame, where the night's pass had made it pale.
The chimneys' smokes (`0x146`) and the geyser's jet (`0x46`), which the light at them tints, stay with the world under the night's pass.
The smokes of the variants 1 to 3 (`FX_HS_SMOKE_*`, 771, 810, 811) and 8 and 9 (`FX_LV_SMOKE_*`, 882, 881) follow as one layer of `PARTICLE_SMOKE03`'s 2 × 2 frames, a frame at random each, turned and spinning, laid over by their alpha (the flags `0x146` and `0x10C` have neither `0x01` nor `0x10`, so the source is the alpha and the destination its inverse, whatever the table above says of the lavas'), the first three a grey of 224 at the alpha of the life, the lavas' ramp B starting opaque; seen 2026-10-27 in 24,86 as white plumes rising from the braziers, not yet set beside the original's.
The geyser (7, `FX_GEYSIR`, 879) follows its clock (read 2026-10-27 at `0x76E3E8`): its jet of `PARTICLE_WATER01` at rest rises at 40 every 0.1 s and lives 0.73 s; its first burst comes 3 to 20 s after it starts (**estimate** for the roll), and for the last second of each rest of 30 to 100 s (`rand()`/32767·70 + 30) its jet bursts upward at 200, living longer (the life's rate −75) every 0.035 s at the detail 2, and the first geyser to burst while the latch that every geyser shares (`0x17EEBC8`) is open plays `211` or `212` at it at a rate's factor of 0.4 to 0.6, which closes the latch until a geyser's rest starts again; the burst's hit of 100 (the system flag `0x400`) is not followed.
The wind of the smoke class's layers is not followed; the magic fire (13) follows since #2075 (above), and the other classes as their sections tell.
The undead's ring and dust follow since #1261 (`puffs`, `opensacred_rules::particles::{rise_ring, dust}`): an undead or a skeleton warrior that stands again makes the variant 3 of 781 at itself, following it, and a corpse of class 5 that vanishes the variant 4 where it lay, drawn as the fires' billboards and not tinted by the night; the dust leaves the corpse's bones rather than its skinned vertices, and without the wind (docs/decisions/drawing.md, "Undead's dust from the bones").
The magic skeleton's drain draws its beam since #1263 (`beams`, the `laserbeam` of the catalogue, 901): from its killer's `Bip01 Spine2` to its own, of the variant 2 ("The laser beams"); **seen** beside the original's (the Daemon's and the Seraphim's level 1 killing magic skeletons of the level 3).
Not followed: the chain's stop at a cell that fails `0x636A60`, and its light of 80.
The last burnings of the 856 (`cParticleSystem_generic`, its virtual table `0x89840C`) follow since #1262 (read 2026-11-02 by code, **confirmed** unless marked otherwise; `opensacred_rules::particles::{burn_variant, generic_sparks, death_flames, scorch_alpha}`).
- The death's action (`0x53ECA0`) makes one at the last death of three kinds of creature, then sounds `DIEFLAMES`: the variant 0 for the undead goblins 142 and 143 (its case 5, `0x541A0D`), the variant 7 for the fire ghoul 73 once its dying clip has played (its case 4, `0x5416DD`), the variant 8 for the magic skeletons 48, 80 and 81 (its case 3, `0x541E8D`); each at the creature's place on the ground, following nothing.
- The variant 0 (`0x796E86`) takes the colour of the event's `+0x38`, `0xFFFF4040` for the goblins: 500 sparks at its model's vertices (every `⌊V/500⌋`th), each falling at a buoyancy of 150 ± 30, shrinking from 5 ± 2 by 2 a second, moving at ± (3, 3, 0), bouncing at the ground of its frame without loss (`0x7645C0`, the mode 0); `PARTICLE_SPARK06` added in the colour, a light of 100 while one lives.
- The variants 7 and 8 (`0x797394`, `0x79749F`) emit every 0.003 s for their first second `PARTICLE_CLOUD01` of a half side of 20, turned at random, shot up at 350 ± 130 (± 15 across) from within 8 of the place, living 0.34 s, added by their alpha: white to orange to a fading red for 7, a pale cyan to blue for 8.
  Under them lies the scorch (`0x7631E0`), a flat quad of a half side of 50 of `FX_STREAKS02` where the start's `rand()` is even, else `FX_STREAKS01`, laid over by its alpha, whole for 10 s and fading by 0.5 a second after; the system ends with it.
- The game draws them (`undead::burn`, `undead::burn_after_clip`): the goblins' sparks by `puffs` from the corpse's bones rather than its skinned vertices (docs/decisions/drawing.md, "Undead's dust from the bones"), the ghoul's and the skeletons' flames and scorch by `fx::last_burning` on the corpse's floor, untinted by the night; **seen** 2026-11-02 in the game (a fire ghoul's in the chapel of 50,39, a magic skeleton's and a goblin's in 52,40), not yet beside the original's.
- An earlier note (#1202) saw red-orange flames and a white flash at a magic skeleton's last death in the original, against the variant 8's pale cyan that the code reads; which creature showed which colour is a **hypothesis** until both are seen in the original.
- Not followed: the detail's factor of the flames' interval (1.5 at `DETAILLEVEL` 1, 2.2 at 0), whether the tick and the draw both update the class in one frame, and the 856's other variants (the Vampiress's weapons' glow of the effect 17, the poison's at a burning target).
- The variant 10 (`0x796E10`'s case 10, its update `0x7962F0`; read 2026-11-03 for #1846, **confirmed** by code and in the running original), a script's smoke (quests 10251 and 10252, `Partikel` in red at a grave): at its clock's 0 one burst of 25 particles (`0x764760`, the block's interval 0 and its count `+0x5C` 25) within ± (15, 15, 4) of the system's place, each of a velocity of (0, 0, 5) ± (15, 15, 0), a buoyancy of −60 ± 10, a size of 8 ± 1 that grows by 10 a second and an angle of π ± π, living 1.275 s (−200); then only the update (`0x7640C0`), the system ending as its last particle dies.
  Its ramp is the event's colour, each byte over 256, from an alpha of 0.996 through 0.39 over 60 entries to 0 over 196 (`0x763F50`); drawn as `PARTICLE_SMOKE04` with the flags `0x4C`: laid over by its alpha, turned, and tinted by the light at it, so that it darkens by night.
  **Seen** 2026-11-03 with the driver at 1509,1216 (`script OMO10251GrabTriggerArea_10251`): a bright red puff over the grave by noon, a dark red one by night.
  The variants 11 and 12 are the same smoke, larger (60 and 80 particles).
  Since #1846 the game makes it (`opensacred_rules::particles::generic_smoke`, `puffs` with `Burst::tinted`, drawn with the world under the night's pass); **seen** beside the original's, its red dark by night.
- The class's textures (`0x796150`, read 2026-11-03 for #1846, **confirmed** by code): the variants 0, 2 and 5 draw `PARTICLE_SPARK06` (`+0x2570`, `+0x2578`, `+0x2584`), 1 and 4 `PARTICLE_SPARK02`, 6 `PARTICLE_GLOW03`, 7 to 9 `PARTICLE_CLOUD01`, 10 to 12 `PARTICLE_SMOKE04`, and 3 the model `arrow2.grn` in `ARROW2_GOLD` with `FX_HELIX01`.
- The variant 5 (250 sparks of a creature's body, of a size of 3 falling by 1.5 a second, their buoyancy growing by 30 a second, bouncing, a light of 100) is never seen in the Sacred campaign: its one `Partikel`, `special_R2_OD`'s, names `cpos:res:R2_special_Zombie`, whose key the reader leaves −1, so it finds no creature and has no cell (probed 2026-11-03, #1888).
- The variant 3 (`0x796E10`'s case 3, its update `0x7962F0`, its draw `0x7967B0`; read 2026-11-03 for #1846, **confirmed** by code and in the running original), the gold arrow of the Gladiator's quest 30 over its two teleports (3784,335 and 3791,328, `trigger2_2`), never ending:
  - the arrow, `arrow2.grn` in `ARROW2_GOLD`, drawn as a model (`0x405C90`) with its specular highlights (the switch's `0x80`, `SPECULARENABLE`), 45 over the place and bobbing by 10 at 7.5 radians a second (`+0x255C`), turning about the upright at 2 radians a second (`+0x2558`, `0x653790`);
  - the helix (`0x762C30`), two ribbons of 50 segments of `FX_HELIX01`, one flat from the radius 15 to 19, one upright 4 high at the radius 17, each segment a quarter of a unit higher and a fifth of a radian on, the texture's `u` from 0 to 1 along them and `v` across, starting 50 over the place at a cycle's share `s` times 25 up and 20 radians on from an angle at random; the cycle runs by 0.3 a second, the helix's alpha `sin(πs)`, a new angle at each cycle's end; added by its alpha (`SRCALPHA`, `ONE`), unlit, both faces, the depth tested and not written, in the event's colour;
  - a light of the radius `100 + 75 sin 5t` (`+0x9C`).
  **Seen** 2026-11-03 with the driver in a Gladiator's game (`script extquest1`, `extquest2`, `trigger2_2`): a gold arrow pointing down over the stairs, turning, with a thin gold ring climbing about its lower half.
  Since #1846 the game makes it (`beacons`, `opensacred_rules::particles::Beacon`): the arrow among the world's models, the helix untinted by the night; **seen** beside the original's, the two arrows over the stairs.
  Not followed: the arrow's specular highlights (its gold is duller than the original's), and whether the original keeps the arrow through a save, which the game does not.
The butterflies fly (`butterflies`, #1073): as a sector of the butterflies' weather entries comes in, ten swarms an entry, each at a walkable cell of the sector at random (a place that `0x638CD0` admits; **estimate**: 40 tries), one in 31 with a glow; a swarm of 3 to 6 wanders at 20 to 40 units of the models a second and turns back where it may not go, its butterflies fluttering about the point 50 units above it, drawn by day as `BUTTERFLY01`'s 8 × 8 wing beats in their own colours at the alpha of their life, and as fireflies by the night's level, by the same billboards as the fires.
Seen 2026-10-27 in 54,42: small coloured specks by noon; by night clusters of three to six yellow dots, which add to the frame after its night's colouring as the original's do, the butterflies' system drawn by the camera of what the night leaves untinted (#1191).
### The steps' small fire (878, `cParticleSystem_smallfire`)

Read 2026-11-02 for #1808 by code, **confirmed** unless marked otherwise.

- A step of a player's hero whose creature `+0x1FC` has bit 1 (six pieces of set 23, docs/re/sets.md M8) makes an object 878 at its place (`0x639222`, `sdk_CREATE(878, place, 0, 0, 0)`, not sent over the network) and starts it with the FX event `0x65`, its caster the hero, its variant (`+0x38`) 2, no target and no array; the kind `0x65` takes the kind 7's case (`0x5A5AE2`): the system stays at the foot's point (the bones 36 and 37) at the ground's height, following nothing.
- The class (`0x79F600`, 200 particles; start `0x79F970`, update `0x79F6D0`, draw `0x79F7D0`), its variant 2: the smoke class's `FX_FI_SMOKE_L` in its flame (ramp A; the layer of life −100 a second and size +20; the block of buoyancy −35 ± 34, size 1 ± 0.5, angle π ± π, velocity ± 0.5, place ± (15, 15, 0), 0.02 s), a smoke of life −180 and size +10 (buoyancy −35 ± 5, size 3 ± 1, spin ± 2, ramp B at the alpha 0.996), emitted while its clock is below 8 s (`0x890918`), then living until its last particle dies (about 10.5 s); a light of 150; the particles drawn at `DETAILLEVEL` 2 alone, as the fires' billboards of `PARTICLE_FIRE03`, untinted.
- Its burnt ground: a flat quad of a half side of 50, turned by a half turn, of `FX_GROUNDSTEP`'s 4 × 4 frames, added (`ONE`, `ONE`), white, at the ground's height with the depth test: the frames 4 to 0 over the first 0.3 s, the frame 0 to 8 s, the frames 0 to 15 over the ninth second (`⌊(t − 3)·16⌋ mod 16`), none after.
- No sound of its own; the fires' loop takes it at 0.6 of the volume (`0x690420`).
- Its hits (the system flag `0x400`, `0x5AB490`, `0x5AE436`): at most once a second, each living creature hostile to the hero, never the hero, within 30 and its size (50 for none) of the fire's place in x and y on its floor takes the event 5 from the hero of the physical `⌊min(0.2·S·m_p, 1000)⌋` and the fire `⌊0.2·min(D·m_f, 1000)⌋`, `S` and `D` his strength and dexterity and `m_p`, `m_f` his first damage multipliers (`+0x96`, `+0x9A`): no hit chance, through the resistances, not turned aside by the magic defence; each fire keeps its own second.
- Its variant 1 (the end of a wall of fire 1006), the Big Demon's steps (type 53) and the spell 50, which make it too, are not followed; the variant 0 of the lava bursts is ("The lava bursts" below).

`opensacred-game` follows the steps' fire since #1808 (`fx`, `opensacred_rules::particles::{small_fire, ground_frame}`, `fight::small_fire_damages`): a fire at each footstep's moment of the hero's walk or run (`sound::HeroStep`) while his sets give it, drawn always (the game has no detail setting), its ground lifted a little (docs/decisions/drawing.md, "Small fire's burnt ground lifted"), its light, its loop and its strikes as above.
**Seen** 2026-11-02 beside the original at the Seraphim's start with the six statues carried: a trail of flames over glowing cracks behind both heroes, the game's flames a little brighter at their cores; a goblin beside the trail lost 42 of its 52 health to the fires in a few seconds.

Since #1092 no stand-in is left: every spell's, art's and creature's object that the game makes looks as its system ("The bolts" to "The soulcatcher" and the sections after them), and those it does not draw yet, show nothing; the torches give no world light of their own (#61), and the systems' Direct3D lights on the models follow since #2106 ("The systems' Direct3D lights").
The order above is #1092's: the machinery with the world's fires first, then the torches' flames, the butterflies (#1073), the hit sparks and the spells' and arts' objects.

## Open questions

- What the object's `+0x14` bit `4` means (the smoke family, the torch smoke, the traps 892 and 893, the blood puddle 903), and the other system flags of the update (`0x8`, `0x10`, `0x20`, `0x800`, `0x4000`, `0x10000`, `0x20000`, `0x40000`, `0x80000`, `0x200000`).
- What the update's second argument (0 from the tick, 1 from the draw) changes, so whether an effect off the screen runs at half the rate of one on it.
- The matrix at `sdk_DXDISPLAY + 0x250` the systems are drawn with (**hypothesis**: the view's rotation inverted, for camera-facing quads), and the scale of the models' units on the screen for a particle's size (1 unit about 1.92 pixels at zoom 1 by docs/re/drawing.md).
- The units and the frame of the scripts' tag `0x7E` (the effects' height; the game takes it in the models' units above the world's zero, which puts the braziers' fires of 50,39 on their bowls as the original's), and which statics the fires and fire smokes stand on.
- The renderer's vector `+0x6C`, `+0x70` that the smoke's wind takes besides `+0x68` and `+0x74`.
- Whether the butterflies' fade in step shows in the original (frames at 60 a second over 3 s).
- The weapons' effects of `cWeapon3D` (the kinds `0xB`, `0x1C`, `0x1D`, `0x33`, `0x3C`, `0x52`) and their systems.
- The FX event's words `+0x10` … `+0x1C`, and the kinds of the handler not listed in docs/re/arts.md.
- The remaining helpers (`0x762650`, `0x7638E0`, `0x763CB0`, `0x768600`, `0x768790`) and the classes that draw their own geometry or models.
- `cParticleSystem_anducar_human`'s virtual table (`0x898FEC`) while `0x5A0C50` builds 1169 with `anducar_demon`'s constructor.
