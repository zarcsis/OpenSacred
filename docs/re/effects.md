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
  | 802 `FX_STARGATE` `0x22`, 853 `0x4D`, 854 `0x4E`, 873 `FX_BUTTERFLY` `0x61`, 889 `0x6B`, 891 `0x6C`, 894 `0x6F`, 917 `0x7D`, 918 `0x7E`, 920 `0x80`, 921 `0x81`, 1156 and 1158 `0xA2`, 1157 `0xA3`, 1160 `0xA4`, 1161 `0xA5`, 1162 `0xA6`, 1165 `0xAE`, 1174 `0xB0`, 1179 `0xB2`, 1180 `0xB3`, 1181 `0xB4` | as listed | 0 (1158: 1) |

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
  By the system's flags it first places the system: `0x40` once at the followed object's bone (`+0x70`) with the bone's orientation (`0x7CBB50`), `0x100` once keeps the bone's point at the object's `+0xB8` (the followed creature's place where the bone is not found), `0x200` at each frame at the bone plus an offset (`+0x90` … `+0x98`; for 788 `FX_BURNING_BONE` from Granny's bone state), `0x400000` and `0x1000` three bones' points into the system's points (`+0x72` … `+0x76` into `0x7CBB90`), `4` the system's coordinates made relative to the object's world place, `2` the origin `+0x5C` at the position.
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
- Each particle takes the block of a layer that `0x7687F0` picks: with `n` 1 the first; with 2 the first four times in five, the second once; with 3 or 4 by the blocks' rates of emission (the inverse of their intervals).

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

- States first (`0x643430`, docs/re/drawing.md): the depth test on (`1`), the depth write off (`0x100`), the lighting off (`0x20`), the blending on (`4`), the linear filter (`0x800`), the source factor `ONE` with the flag `0x10` else `SRCALPHA` (`8`), the destination `ONE` with the flag `0x01` else `INVSRCALPHA` (`0x10`).
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
- When the system last emitted with two layers (`+0xA8` 2, or the byte `c`), the batch is drawn and the destination factor switched at each change of layer: the layer 0 additive (`ONE`) where the byte `b` is 0, the others by the alpha (`INVSRCALPHA`).
  So a fire's flames add to the frame and its smoke covers it.
- The particles are drawn in their slots' order, not sorted; the depth test against the models and the depth buffer's other contents is their only order inside the system.
- A trail's layer, when given (the fourth argument, tested at `0x7623D8`), draws each particle again as shrinking copies back along its velocity (0 in the classes read here).

### The other primitives

- **A glow** (`0x762E10`): one camera-facing quad of a half side `s` at the system's position or one of its points plus an offset, of one colour, the depth test as asked, the destination always `ONE`; its texture by a style: 0 `PARTICLE_FLARE02`, 1 `GLOW04`, 2 `GLOW05`, 3 `GLOW01` (source `SRCALPHA`), 4 `SPARK07`, 5 `GLOW03`, 6 `GLOW06`, 7 `GLOW07`, 8 `FLARE04`, 9 `GLOW08` (source `ONE`), `0xFE` and `0xFF` the texture already set with `SRCALPHA` or `ONE`.
  41 classes use it.
- **A flat quad** (`0x7631E0`): a square of half side `s` in the horizontal plane at the system's position, turned by an angle, of a texture and a colour, a frame of a grid, the source `ONE` or `SRCALPHA` and the destination `ONE` or `INVSRCALPHA` as asked: the magic circles, the ground symbols, the puddles, the decals (50 classes).
- **A ring** (`0x763530`): a band of `n` segments between two radii, flat or stood up (turned a quarter about x), of a texture, additive or blended (7 classes: the explosions, the shock waves of the Gladiator's hits, the rings of poison).
- **A helix** (`0x762C30`): two strips of 50 segments turning about the axis, additive (3 classes).
- Others draw their own geometry (`DrawPrimitive` directly, 50 classes), Granny models (`cGranny::render`, 16 uses: the killer plants, the bubble trap's bushes, the demons' illusions), and a few further helpers (`0x762650` a streak, `0x768600` and `0x768790` chains, `0x7638E0`, `0x763CB0`; not read).

### What the textures are

The particles' textures are `texture.pak`'s `PARTICLE_*.TGA` and `FX_*.TGA`, converted to `assets-src/textures/texture/<number>-<name>.png` (63 `particle_*`: the numbers 14865 to 14927).
Two kinds, by their pixels (counted 2026-10-27): white with an alpha mask, tinted by the vertices' colour and blended by alpha (`PARTICLE_FIRE03` 128², `SMOKE03` 256² of 2 × 2 frames, `WATER01`, `GLOW01` 16² of alpha up to 153, `BUTTERFLY01` 128² of 8 × 8 frames of 16², grey wings), and colour on black, opaque, drawn additively (`PARTICLE_FIRE01` 256², a 4 × 4 flip-book of a burst of flame, `GLOW03` 64², `MULTI02` 256², `SPARK06`).

## Lights

- An FX object gives light through its system: its draw (`0x5A9C50`) adds to the frame's table of lights (`0x41B200`, docs/re/daynight.md) the object's world place with the system's radius `+0x9C`, at each frame it is drawn, where the radius is above 0.
  The radii the classes' starts set: the fires 280, 370 and 530 (variants 10, 11, 12) and the fire smokes alike (4, 5, 6), the magic fire 230, the fireball 250, the fire strike 200, the burning bone 150, the dwarf's magic 350 or 400, the gates and the spells others (the catalogue).
- The draw lists' path for an object of the type flag `2` (`0x631358`) adds a light only for a creature (a cast to `cCreature`, then `0x54A7B0`), so the items table's radius of an FX type (`+0x32` with the flag `0x20000`) gives no light; nor are FX types statics, whose radius `0x41B170` takes.
- The torch's flame gives none of its own: the torch's light is the hero's rule (docs/re/daynight.md: 300 in a dungeon, half as much again outside, from the types `0x1600` and `0x1601` in the creature's objects).

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
| 13 | 904 `FX_FIRE_MAGIC` | `MULTI02`, `0x11D`, 2 × 2, a frame per layer | three layers | three blocks (0.0265, 0.065 and 0.022 s, chosen by their rates) | three ramps: white to pink, magenta and teal; orange to blue; pink to purple; each to black | 230 |

- Ramp A, the flame: from (254, 243, 131) at an alpha of 90 to (254, 80, 40) opaque over 22 entries, to (232, 67, 20) at 229 over 29, to black at 139 over 85, to transparent black over 120.
- Ramp B, the fire smoke: from a grey of 60 at an alpha of 0 to a grey of 33 at the variant's alpha over 110 entries, to transparent black over 146.
- Ramp C, the fire's smoke: from a grey of 159 at an alpha of 0 to a grey of 133 at 80 over 110, to a grey of 100 at 0 over 146.
- The flags `0x0C` with two layers draw the flames additively by their alpha (`SRCALPHA`, `ONE`) and the smoke by its alpha (`SRCALPHA`, `INVSRCALPHA`), turned, coloured by the ramps, not tinted: a fire keeps its colour by night.
  The flags `0x146` (the chimney smokes) tint by the light at the object; `0x10C` (the lava smokes) add their colour with the source `ONE`; `0x11D` adds all.
- Every flame and smoke particle rises by its own buoyancy (−35 ± 34 for a flame, a rise of up to 69 units a second gained each second) and drifts with the wind; its quad grows from about one unit to 10 to 30 units of half side over its life.
- The variants 5 and 6 set the system flag `0x400`, so that the fire smokes of those sizes burn the creatures in them (`FX_FI_SMOKE_M` 10 and `_L` 20 of the first two kinds, docs/re/arts.md); the fires 884 to 886 and the small fire smoke do not.
- The geyser's update keeps a timer (`+0x2E38`) of 3 to 20 s at its start: while it is between 0 and 1 s the jet bursts upward at 200 with the flag `0x400` (its hit of 100) and plays once the sample 211 or 212 (one in two) at 0.4 to 0.6 of the volume, through a shared latch (`0x17EEBC8`); below 0 it takes 30 to 100 s anew.
- **Seen** 2026-10-27 at 50,39: the four braziers around the cell (3239, 2532) burn on their bowls, a small orange flame of about 20 pixels at zoom 1 with a faint grey smoke above it; by night the flame keeps its colour while the scene darkens, its smoke lighter than the ground about it.

### The torches' flames: `cParticleSystem_torchsmoke`

787 `FX_TORCHSMOKE`, 200 particles (constructor `0x774F00`, start `0x7752B0`, update `0x774FE0`, draw `0x775050`), made and ended by the torch's `cWeapon3D` (above), following the torch's bone 15 `weapon_fx01` at each frame (system flags `0x204`).

- One layer: the life −800 a second (0.32 s), no growth.
- One block: a size of 7, a random angle, the velocity (0, 0, 50) ± 10 on each axis, the place ± 1.5 on each axis, every 0.03 s.
- The draw: `PARTICLE_FIRE01` as a 4 × 4 flip-book, the flags `0x15` (additive `ONE`, `ONE`, turned), white: each flame plays the sixteen frames of a burst over its 0.32 s while it rises 16 units; then a glow of style 3 (`GLOW01`, `SRCALPHA`, `ONE`) of a half side of 20, the colour (255, 234, 59) at an alpha of 128, at the bone.
- The start plays the sample 219 at the torch, looping, its handle kept at `+0xB0` and stopped with the system (**hypothesis**: the torch's crackle).
- An end sent by the off switch (`+0x20A8` 1) stops the emission; the system dies when its last flame does.
- No light of its own (above).
- **Seen** 2026-10-27: with `TORCH_SMALL_ON` in the Seraphim's left hand by night, a bright yellow-white ball of flame at the torch's head, with the scene lit about the hero.

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

- **The variant 3**, the ring of an undead that rises again (docs/re/fight.md, "The undead that rise again"): 75 particles on a ring of 17.5 about the system's place, at the angles of a 75th of a turn, each rising at 70, the even ones with a buoyancy of +100 that brings them back down from about 24.5 at 0.7 s; a size of 10, the life's rate −200 (1.275 s) and the size's −8; the ramp from (0.898, 0.0195, 0.996) opaque, magenta, to (0.0117, 0.422, 0.996) over 126 entries, to (0.0664, 0.621, 0.9375) at 0.949 over 45, to transparent black over 85; the system flags `0x204`, no light.
- **The variant 4**, the dust of a corpse that vanishes (docs/re/fight.md, "The other deaths of class 5"): a burst (the block's interval 0) of 150 particles at the corpse model's skinned vertices (`0x44FC30`, `0x767FB0`), the buoyancy −20 ± 8, the size 10, the angle π ± π, the angle's speed 0.5 ± 0.2, the velocity 0 ± 3 along each axis and a frame of 4; the layer's rates of the buoyancy −10 (they rise ever faster), the life −100 (2.55 s), the size +8 and the angle +1, its wind the renderer's vector as the smoke class's; the ramp of (0.898, 0.836, 0.664), a pale bone colour, from an alpha of 0.5 to 0 over 256 entries.
- The variant 5 is the variant 4's of (0.625, 0.684, 0.625), a pale green, the buoyancy −16 and the size 7 (its user not read); 0 is a burst of 500 at the skinned vertices (the buoyancy −5 ± 5, the size 2, the life's rate −150, from (0.8, 0.9, 1.0) to opaque black, which adds nothing); 1 a burst of 250 there (the size 10, the angle π ± π, the life's rate −150, the buoyancy's −1 and the angle's 1, a black ramp); 2 a ring of 50 at the 50th of a turn (0.1257) of 17.5 at a height of 40, rising and falling at ± 70 with a buoyancy of −10, of a size of 25, the life's rate −512 and the size's −30, from (1, 0.5, 0.5) to opaque black, with the sound event `0x825` and `(9, 0x49)`; none of them read for its users.

### Not followed here

The birds over the camera (`0x637060`, docs/re/weather.md) are models, not effects.
The lamps' glows are the miniatures' (docs/re/daynight.md), not FX objects.

## The families and an order for `opensacred-game`

The families, by what they need beyond the machinery:

1. **The machinery** (one module): the FX object made from a category-12 record and an FX event (its kind and variant), a system per type by `0x5A3570`'s table, particles, layers, blocks, ramps and the update and emission of `0x7640C0` and `0x764760`; the billboards of `0x761B00` with the three blend modes (`SRCALPHA`/`INVSRCALPHA`, `SRCALPHA`/`ONE`, `ONE`/`ONE`), the atlases' frames and the per-layer switch; the glows of `0x762E10`; the light by `+0x9C`; the tint by the light at the object; the entries in the draw lists (list 4, 0 or 3 by the type's flags) under the night pass.
2. **The world's fires and smokes** (the smoke class, 13 types, some 3,650 objects in the Seraphim's tree): already placed by the scripts' `CreateObj` with their heights; the variants are data (the table above), one texture each, no actions, and their lights are the night's (#61).
   The simplest first: the variants 10 to 12 (`FX_FIRE_L`), then 4 to 6 (with the hit of 5 and 6 already a rule), 1 to 3, 8 and 9, 7 (its timer and sound), 13.
3. **The torches' flame** (787): a flip-book and a glow following the bone `weapon_fx01` of the item in hand; the game has the bones (docs/re/arts.md uses them for the `Rundum` strikes).
4. **The butterflies** (873, #1073): the wander, the 8 × 8 flip-book with the alpha of the life, the fireflies' glows by the night's level.
5. **The hit sparks**: `FX_SERA_SM_HIT` (843, 151 particles: a burst at once, buoyancy +180 ± 40, a size of 6 ± 1 shrinking by 4 a second, the velocity (0, 0, 60) ± 40, the place ± 10, a life of 1.275 s, `SPARK06` additive with a ramp from (127, 127, 255) at the birth to black, a light of 200, the system flag `0x40`: placed once at the bone the event names), and its kin `FX_ELVE_SM_HIT` (860), `FX_DARK_SM_HIT` (864), `FX_VAMP_SM_HIT` (862), `FX_GLAD_SM_HIT` (842, 600 particles with rings and floops), all `SPARK06` with the flags `0x19`; the art 1003's event value shakes the screen (kind 7, 30) and changes the burst.
6. **The spells' and the arts' objects**, a class each (docs/re/arts.md has their rules): the fireball's flight (776 `GLOW01` with a light of 250; 778 `FIRE01` flip-book, light 200), then the rings, the decals, the helices, the beams and the models' effects.

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
| generic | 500 | 856 | ARROW2_GOLD, FX_HELIX01, FX_STREAKS02, PARTICLE_CLOUD01, PARTICLE_GLOW03, PARTICLE_SMOKE04, PARTICLE_SPARK02, PARTICLE_SPARK06 | FX_HELIX01 `5/g1`; FX_HELIX01 `0x11/g1`; PARTICLE_GLOW03 `0x19/g1`; PARTICLE_CLOUD01 `0xd/g1`; PARTICLE_CLOUD01 `0xd/g1`; PARTICLE_CLOUD01 `0xc/g1`; PARTICLE_SMOKE04 `0x4c/g1` | flat quad, helix, model | 100, 150 | 0x40 |
| glad_sm_cast | 100 | 803 | FX_AURA, PARTICLE_SPARK06 | PARTICLE_SPARK06 `0x19/g1`; PARTICLE_SPARK06 `0x19/g1`; PARTICLE_SPARK06 `0x19/g1` | flat quad, glow |  | 0x200 |
| glad_sm_hit | 600 | 842 | FX_GRADIENT3, FX_RINGX1, FX_RINGX2, FX_RINGX3, FX_RINGX4, FX_RINGX5, FX_RINGXGLOW, PARTICLE_FLOOP01, PARTICLE_GLOW03, PARTICLE_SPARK06 | PARTICLE_SPARK06 `0x19/g1`; PARTICLE_FLOOP01 `ebp/g?` | flat quad, ring | 200 | 0x200,0x202,0x3e800000,0x40 |
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

## In `opensacred-game`

The world's fires burn (#1092): `opensacred_rules::particles` holds the machinery (the particles, the layers' rates, the emission blocks with their spreads, the ramps, the update of `0x7640C0` and the steady emission of `0x764760` with its pick of a layer for two) and the smoke class's variants 4 to 6 and 10 to 12; the scripts' `CreateObj` of `FX_FIRE_L` (884 to 886) and `FX_FI_SMOKE_*` (812, 813, 876) places a fire at its `0x20` and its height `0x7E` in the models' units above the world's zero (`npcs`), and `fx` runs its particles at the frame's time and draws each layer as camera-facing quads of `PARTICLE_FIRE03` (sampled without its smaller levels), the flames added by their alpha and the smoke laid over by its alpha, their ramps' colours written as they are into the frame of sRGB values; each fire lights the night by its radius (280, 370, 530).
Set beside the original on 2026-10-27 at the braziers of 50,39 by noon: a flame of a yellow core and an orange top with a faint grey smoke, its pixels of red above 200 and green above 150 247 against the original's 239, its brightest (255, 255, 165) against (255, 255, 157), its extent within two or three pixels of the original's.
By night the original's flame keeps its colour (the flags `0x0C` leave out the tint), and so does the game's since #1191: a camera of its own (`iso::Untinted`) draws the systems without the flags' bit `0x40` after the world's camera and its night's pass, into the same frame and against its depth, so that the night's colour leaves them as they are; on 2026-10-27 the braziers of 50,39 by night showed the original's yellow and orange flame, where the night's pass had made it pale.
The chimneys' smokes (`0x146`) and the geyser's jet (`0x46`), which the light at them tints, stay with the world under the night's pass.
The smokes of the variants 1 to 3 (`FX_HS_SMOKE_*`, 771, 810, 811) and 8 and 9 (`FX_LV_SMOKE_*`, 882, 881) follow as one layer of `PARTICLE_SMOKE03`'s 2 × 2 frames, a frame at random each, turned and spinning, laid over by their alpha (the flags `0x146` and `0x10C` have neither `0x01` nor `0x10`, so the source is the alpha and the destination its inverse, whatever the table above says of the lavas'), the first three a grey of 224 at the alpha of the life, the lavas' ramp B starting opaque; seen 2026-10-27 in 24,86 as white plumes rising from the braziers, not yet set beside the original's.
The geyser (7, `FX_GEYSIR`, 879) follows its clock (read 2026-10-27 at `0x76E3E8`): its jet of `PARTICLE_WATER01` at rest rises at 40 every 0.1 s and lives 0.73 s; its first burst comes 3 to 20 s after it starts (**estimate** for the roll), and for the last second of each rest of 30 to 100 s (`rand()`/32767·70 + 30) its jet bursts upward at 200, living longer (the life's rate −75) every 0.035 s at the detail 2, and the first geyser to burst while the latch that every geyser shares (`0x17EEBC8`) is open plays `211` or `212` at it at a rate's factor of 0.4 to 0.6, which closes the latch until a geyser's rest starts again; the burst's hit of 100 (the system flag `0x400`) is not followed.
The wind of the smoke class's layers, the magic fire (13) and the other classes are not followed.
The undead's ring and dust follow since #1261 (`puffs`, `opensacred_rules::particles::{rise_ring, dust}`): an undead or a skeleton warrior that stands again makes the variant 3 of 781 at itself, following it, and a corpse of class 5 that vanishes the variant 4 where it lay, drawn as the fires' billboards and not tinted by the night; the dust leaves the corpse's bones rather than its skinned vertices, and without the wind (docs/decisions/2026-10-28-undead-effects.md).
The butterflies fly (`butterflies`, #1073): as a sector of the butterflies' weather entries comes in, ten swarms an entry, each at a walkable cell of the sector at random (a place that `0x638CD0` admits; **estimate**: 40 tries), one in 31 with a glow; a swarm of 3 to 6 wanders at 20 to 40 units of the models a second and turns back where it may not go, its butterflies fluttering about the point 50 units above it, drawn by day as `BUTTERFLY01`'s 8 × 8 wing beats in their own colours at the alpha of their life, and as fireflies by the night's level, by the same billboards as the fires.
Seen 2026-10-27 in 54,42: small coloured specks by noon; by night clusters of three to six yellow dots, which add to the frame after its night's colouring as the original's do, the butterflies' system drawn by the camera of what the night leaves untinted (#1191).
The spells and the arts still draw stand-ins of their own (balls and flames of one colour, docs/re/arts.md), and the torches burn without a flame (#1092, #61).
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
