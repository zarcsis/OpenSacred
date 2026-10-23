# The world's objects, the hero, the clock and the view

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra 12.1.4 on 2026-10-21, for the original's driver (#448, #451; docs/03 §3.8), from the community's labels of the Ghidra project (sacred-sdk, **community-reported**) and the code.
Status: **confirmed** on the code unless marked otherwise; "observed" marks what the driver read in the running original on 2026-10-21, at the Seraphim's start of a new game.
Distances are in world units, 53.66563 a cell.

## The objects' table

The object manager is kept at `0xAD5C40` (`sdk_OM`, a pointer): its `+4` and `+8` begin and end the vector of the objects' pointers, indexed by the object's id, a slot empty where none is.
An object's id is also at its `+0x0C`; the getter of an object by id (`0x5FE000`, or the same code inlined some 470 times) logs and mends a mismatch.
The ids 1 to 16 are the players' (docs/re/movement.md, "The hero's id is 1").

The game's context (`sdk_CTX`, a pointer at `0x182EBE8`, made by `sdk_ACTIVE_CLASS_GETTER`, `0x7D84A0`) keeps the local player's id at `+0x14`, and `0x603E30`, given the manager, returns the local player's object as a creature (observed: id 1).

An object:

| Offset | What it is |
|---|---|
| `+0x00` | the vtable |
| `+0x0C` | its id |
| `+0x10` | its type (`items.pak`'s index; a hero's class, 1 for the Seraphim, observed) |
| `+0x14` | flags (`0x10` removed; `0x40000000` and `0x200` set by a move; observed `0x44008000` for the hero) |
| `+0x18` | its position: a `u16` sector of the world's own numbering, then `i32 x` and `i32 y` in world units, then a `u8` floor (`sdk_BUILD_POS`, `0x6224B0`, makes one) |
| `+0x2C` | its layer: 0 on the ground, else a building's floor (docs/re/movement.md) |

The cell of a position is its world point over 53.66563, truncated (observed: the hero at `(173688, 134781)`, the cell `(3236, 2511)`, on the chapel's floor 1), and its sector the cell's coordinates over 64: `sdk_SECTOR_RESOLVE` (`0x635C40`) takes the world's number of a sector from a table of 128 × 128 at the world's `+0x284`, by the cell's coordinates shifted by 6.

A creature is an object whose class derives from `cCreature` (its RTTI type descriptor at `0x8EB660`; the game asks with `__RTDynamicCast`, `0x84A961`), the hero too (observed: the hero's vtable is `cCreature`'s, `0x891184`, not `cCreatureHero`'s, `0x891250`).
Its fields that the driver reads, beside those of docs/re/ai.md and docs/re/fight.md:

| Offset | What it is |
|---|---|
| `+0xFC`, `+0xFE` | the AI's state and sub-state (docs/re/ai.md, "Two states") |
| `+0x104` | its target, an object's id while it has one (observed: 1, the hero, for the goblins fighting her; otherwise a number of no object) |
| `+0x150`, `+0x152` | the action's state and sub-state |
| `+0x1F8` | the gait, 1 walk, 2 run (observed: 0 for the hero until it first moved, then 2) |
| `+0x3B4` | the experience (`u32`, the stats' `+0x0C`, docs/re/fight.md) |
| `+0x3FE` | the level (`u16`) |
| `+0x4D4`, `+0x4D8` | the most health and the health, integers (observed: 119 and 119 for the new Seraphim, as her character's panel shows them, 112 of 119 after a goblin's hits) |

Observed at the Seraphim's start: 1,554 objects, 661 of them creatures; within 15 cells of the hero, one creature, the novice of quest 1 (type 679, `(3238, 2518)`, 48 health, level 1).

## Placing the hero

The scripts' `Teleport` (46, `0x491D40`, docs/re/netscript.md) puts a player through `sdk_ENGINE_TP` (`0x54D9D0`), a method of the creature with a cell, a floor and a flag of an effect: it tests the cell as a step would (`0x4EBCA0`, docs/re/movement.md), and when the creature may stand there it moves the object through the object manager (`0x5FCCB0`), brings the creature's followers (`+0x39C`) to its new place, clears its AI's and action's states, and returns 1; else 0, and `Teleport` tries cells at random within 3, then within 6.
One cell, `(0x189C, 0x591)`, starts the end of the game's script instead.
Observed: `(3296, 2528)` refused, `(3295, 2529)` taken; the hero stood there at once, the sectors about it came in, the creatures counted rose from 661 to 709.
The floor is the position's: 0 puts the creature on the ground's grid even within a building's walls, under its roof (observed at `(3226, 2500)`, in the chapel's walls, where the hero lay outside the chapel's floor and an outlaw reached it); a building's ground floor is floor 1.

## The hero's level, experience and health

Status: **confirmed** by code and in the running original (2026-10-22, #558).

- A creature's method `0x549500` (`thiscall`: a level and a flag) calls its stats' level setter `0x564D60` (`+0x3A8`) with the level, 1 and the flag (docs/re/fight.md, "The hero's levels").
- Its method `0x549520` (a number of points) calls its stats' `addExperience` (`0x57E160`), which a kill's experience goes through: it adds the points to the experience (`+0x3B4`), held below `X(216)`, takes the level of the new experience (`0x57F8D0`) and raises the level one at a time to it, as play does.
  Observed: 49,500 points took a new Seraphim to level 10, her health from 119 to 317; 50,500 more to level 12.
- The health is `+0x4D8` and its most `+0x4D4` (above); writing the most into the health heals a creature at once, as the stats' own refills do.

## Making creatures, kills and drops

Status: **confirmed** by code and in the running original (2026-10-22, #559), which the driver's `spawn`, `kill`, `drop` and `item` use.

- `FillSector`'s creature maker `0x480D40` (`thiscall` of the script interpreter, 12 arguments, docs/re/spawning.md) takes a cell's `x` and `y`, two zeroes, the type, a level that only an object of type 1162 uses, the ids of three items to equip, a zero, flags or'ed into `+0x1F4` (−1 none) and the hostility: 1 hostile (`+0x1F4` bit 1 and the fight stance), 0 peaceful (for some animals bits `0x10` or `0x40` too), −1 by the type (`0x549000`).
  It places the creature on the cell's middle on the ground (floor 0), turns a fifth of the type 344 into 359, and reads of the interpreter only the byte `+0x12` (an `Enter` running), which marks the creature as one its sector's exit takes away.
  It sets no level: `FillSector` then calls the level setter, a boss or champion at the marker level 1 with its mark (docs/re/fight.md, "Bosses and champions").
- The stats' level setter `0x564D60` (`thiscall`: the level `L`, flags `a` and `b`): with `a` 0, an NPC's `L` (not a boss's or champion's marker 1 or 1000) is raised by the difficulty's addition to the lowest (`0xAD4C18` by its column), scaled by the difficulty (`0x552230` with no time played) and raised by `0xAB4680`; with `b` 0, an NPC's (an id above 16) is held at its sector's highest (`+0xD5` of the information of its position, `0x63D0C0`) plus the difficulty's addition to the highest (`0xAD4E1C`), or that addition alone without the information, 300 for the type 701, and a boss's or champion's at 2 more (`0xAD4D48`, `0xAD4D4C`) unless its type is big; with `b` 1, or for a horse (`0x4266F0`), it is the level asked, at most 250.
  The column of the additions is 3, 2, 1 and 0 for the difficulties 4 down to 1 and 5 for bronze; seen in the original on bronze: the lowest's additions 0, 35, 70, 128, 0 and 0, the highest's 50, 120, 190, 250, 250 and 0, `0xAB4680` 0, the boss's and champion's 2 and 2.
  `FillSector` and `0x549500` pass `a` 1 and `b` 0, so a fill's creature is held at its sector's highest; `CreateNPC` passes 1 and 0 for its rule's level and 0 and 0 for a level tag (#540).
  Seen: a goblin warrior made in 51,39 at the level 20 through `0x549500` took the level 4, and with `b` 1 the level 20.
- A strike lands as the event 5 that `0x4EA3F0` sends its target: an event of the vtable `0x89095C`, its type 5 at `+4`, the attacker's id at `+8`, the target's at `+0x0C` and `+0x10`, then the strike's record (the weapon's type and id, the attacker's id, four words of fixed damages, a kind, a word and the factor, a float), handled at once by the target's `+0x18` of its vtable (`0x52E590` for a creature): a target farther than 1,500 from its attacker ignores it.
  The death that gives drops and experience is made there, with the attacker as the killer: an unarmed strike of the hero at a creature whose health is 1 kills it as the hero's kill (seen: the goblins of 51,39 died, each called the drop once with the killer 1, and the hero took 27 and 45 experience), while a health set to 0 alone makes the creature die with no drop and no experience.
- The drop `0x559BF0` (`thiscall` of the creature; the type asked, the bonus, the value and the killer's id) makes its objects through `sdk_CREATE` (`0x5FBA40`); with a type forced it makes that item even for a hero (docs/re/items.md, "One call of the drop"), at the creature's place and level.
  Its return value is not what it made: 0 for gold and for most items, an id at times (seen 2026-10-23, #594: 30 drops returned one potion while `sdk_CREATE` made 3 gold piles and 4 items more), so the driver collects what `sdk_CREATE` makes during each call.

## The console's cheats

The client's console (`UI_WND_CONSOLE`) passes a line to two handlers.
`0x614230` takes the weather's `setrain`, `setsnow` and `setquake`, `setdungeon`, `loadplayer`, the logs' `droplog`, `logref` and `logslot` (docs/re/items.md), the caches' `flush`, `flushcache` and `flushtex`, and `cheat` with a code of the developers, which sets bit `0x2000000` of `0x182EE38`.
`0x615550` takes the others, each in lower or upper case, and answers that the cheats are disabled without that bit (read 2026-10-22, #558):

- `lord` toggles bit 1 of `0x182EE3C` and says so; the strike's resolution (`0x56B130`) reads the bit for a player's hero (an id up to 16), which with it is no longer knocked down or stunned by an attacker four or more levels above it, and no other reader of the bit was found.
  Observed with the bit set: the goblins of 51,39 took 5 to 10 of a level-12 Seraphim's health a hit as before, so it is no god mode for the damage; the driver's `god` keeps the health itself (docs/03 §3.8).
- `full` runs the hero's regeneration (`0x562730`) 30 times for 50 s each.
- `killme` and `suicide` set the hero's health to 0, its AI's state 9 and its action 6.
- `upgrade` teaches the hero each combat art of the runes 5375 to 5782 that its type may learn (`0x611510`).
- `nq` and `hq` resolve a marker of the map (`sdk_MARKER_RESOLVE`; not followed).
- `win` with `all`, `bronze`, `silver`, `gold`, `platin` or `niob` passes the difficulty to `0x425290` and `0x424CF0` (**hypothesis**: marks it won, which unlocks the next).

## The clock

The clock of docs/re/daynight.md is reached through a pointer at `0xAA4540` (observed: the object's vtable `0x8908E8`, its `+0x0C` 24.0, the rates at `+0x10` and `+0x14` 1.0, the date at `+0x18` 1370, `+0x1C` 2 and `+0x1E` 14, the time `08:31:06` at `+0x20`, `+0x22`, `+0x24` a minute and a half into a new game).
Written on the render thread, the time and the rates take at once: 20:20 gives the dusk, and the rates 0 stop the clock (observed).

## The view and the zoom

The engine (`cEngine` at `0xAD6EF8`, docs/re/window.md) keeps its world views at `+0x08` and the current one's index at `+0x48` (`u16`).
A view's zoom is its float at `+0xB60` (docs/re/drawing.md): `cWorldView_setZoom` (`0x628130`) and `cWorldView_addZoom` (`0x628310`) set it or add to it, held within 0.5 and 2, and set the device's projection, an orthographic one of `±267` by `±200` times the zoom, so they run on the render thread; `cEngine_handleZoomInput` (`0x60DD20`) then eases a zoom between 0.5, 1 and 2 toward the nearest of them, frame by frame (observed: `zoom 2` held).
The view's sixth virtual function takes a pixel of the window to a position on the ground's plane (the cursor's update, `cEngine::updateWorldCursor`, `0x611E70`, calls it before it finds the cell under the cursor with `0x616AD0`, docs/re/movement.md, "The cursor's point"); being affine, three of its answers give the pixel of a cell, without the ground's heights.
Observed: a cell 2 and 4 cells off the hero at (418, 530) of the window; a click there walked the hero onto it.

## The render thread's frame

`cEngine_handleZoomInput` is called once a frame by the render thread (`cEngine::renderThread::run`, docs/re/window.md), after the hero's update and before the world's updates and the drawing, whether the game's time stands or not; the engine is its `ECX`.
The driver reads the world and makes its changes there, so that it never reads an object halfway through its update and changes it where the game itself does.
