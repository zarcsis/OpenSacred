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
