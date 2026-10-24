# The traces of the original's functions

The original's driver (#448, #452; docs/03 §3.8) logs the calls of the original's functions: `exec "trace …"` or `original/trace`, a preset by its name or any address with the kinds of its arguments.
This page lists the presets, the functions they hook and what they read; each function is described in the page named.
Status: the functions as their pages have them; "observed" marks what the traces showed on 2026-10-21 in a new game of the Seraphim.

## A record

Each call becomes a record of the log (`trace` its target): the preset's name, the arguments or what the preset reads, the return, then the time of the performance counter in milliseconds, the frame (the flips counted), the game's clock and the thread.
A trace logs at most 200 calls a second (its `limit`), and says how many it left out.
The kinds of an argument: `int`, `uint`, `hex`, `u16`, `i16`, `float`, `ptr`, `object` (a pointer to an object: its id, type, cell and, for a creature, health), `id` (an object's id, read so), `cstr` (a C string), `out-float` (a pointer whose float is read as the call returns); the first argument is `ECX` for `thiscall` and `fastcall`, then `EDX` for `fastcall`.
A function that returns a float on the x87 stack gives no return to read.
The target may also be a module's export, `MODULE!EXPORT` (`GRANNY.DLL!_GrannySetAnimationParameter@24`), its module's name in any case.

A probe reads an instruction's state instead of a call's (#785): `--stack KIND@PLACE,...` with an address, inside a function too, logs as that instruction is reached each value at its place, a register (`eax`, `ebx`, `ecx`, `edx`, `esi`, `edi`, `ebp`, `esp`) read as an argument of that kind, or an offset from `ESP` whose slot is read so, or as a `double` (the kind of a probe alone), or a register plus an offset, `REGISTER+OFFSET`, the memory there read the same way (#792).
It waits for no return, so it never touches the stack as a call's trace does, which, at an instruction inside a function, would take a local for the return address.
As `trace 0x4dd855 --stack object@esi,double@228,double@140,double@348,double@108` logs a shooter's clip's start, end, loops and Granny's clock as its arrow's test reads them (docs/re/fight.md, "The archers"), and `trace 0x57c65d --stack uint@ebp+0x8,u16@ebp+0x3c,u16@ebp+0x3e,u16@ebp+0x40` the stats' type and their three speeds after the recomputation's skills.

## The presets

| Preset | Group | Function | What it logs |
|---|---|---|---|
| `strike` | `fight` | `0x56B130`, a strike's resolution (docs/re/fight.md) | the attacker (the id at the event's `+8`), the defender (the creature at `this +4`), its health before and after, the damage or none; "spared" for the hero under the driver's `god` |
| `chance` | `fight` | `0x43EEE0`, the hit chance and the damage through a resistance (cdecl) | `x`, `y`, `c`, `p` and the share `w` it writes |
| `death` | `fight` | `0x53E7C0`, the dead creature's state (every frame while dead) | the creature, once |
| `experience` | `fight` | `0x57D860`, a kill's experience (docs/re/fight.md, "The hero's levels") | its levels and the experience it returns |
| `drop` | `drops` | `0x559BF0`, one call of the drop (docs/re/items.md) | the creature, the type asked, the bonus, the value and the killer's id (the four of the stack, before #561 logged one place off: the bonus as the type, the value as the bonus, the killer as the value), and the objects that `sdk_CREATE` made during it: their types, a gold pile's amount (`+0x120`) |
| `item` | `drops` | `0x5D5080`, the item maker (cdecl; docs/re/items.md) | its first four arguments, the type and the level first; it also makes the creatures' own items as sectors come in |
| `regen` | | `0x562730`, the regeneration (every frame of every creature; docs/re/fight.md, "Health") | a creature's health as it changes, the hero's by default (`--ids`) |
| `wake` | `ai` | `sdk_WAKE` (`0x59F580`, docs/re/ai.md) | the creature |
| `look` | `ai` | `0x547070`, the hero's look about (every 0.3 s) | the hero, its argument and return |
| `idle` | `ai` | `0x54EA30`, the idle action's roll | the creature |
| `door-search` | `doors` | `0x4D4800`, a creature's search for a door in its way (docs/re/fight.md, "Doors in the way") | the creature, whether it found one |
| `door` | `doors` | `0x44BCD0`, an object's events, the doors' alone (types 5650 to 5766); the hero's click does not come this way (#480) | the door, the event's type (`+4`), its trigger's state (`+0x34`, the word `+0x0A`) before and after |
| `trigger-set`, `trigger-reset` | `doors` | `0x415F00` and `0x416280`, a trigger's state set or reset (`thiscall`: the bits, whether to send it on), which turn a door however it is turned | the trigger's id (`+0`), the object it serves (`+6`), the bits, its word (`+0x0A`) before and after |
| `sound-pick` | `sound` | `0x4261D0`, a variant of a profile's group (docs/re/sound.md) | the profile's and the group's indices, the sound id returned |
| `sound-event` | | `0x67A7A0`, the sound manager's events | its first four arguments (about 1,500 a second, mostly of the types 6 and 4; held to 20) |
| `sample-file`, `sample-3d-file` | `sound` | `mss32.dll`'s `AIL_set_named_sample_file` and `AIL_set_3D_sample_file` | the sample, the name and the data |
| `sample-start`, `sample-3d-start` | `sound` | `AIL_start_sample`, `AIL_start_3D_sample` | the sample |
| `stream` | `sound` | `AIL_open_stream` | the file opened |

The per-frame functions (`death`, `regen`, `idle`) cost a call of the agent each time; the presets filter them before logging.

## What the traces showed (observed)

- The hero's click on the merchant's door of 50,40 (the object 1163, type 5672) set its trigger 4220's bit 0 (`trigger-set`: the state 0 to 1) and reached no door's event of `0x44BCD0`: the click turns the door through the engine's event (`cEngine_receiveEvent` calls the trigger's set and reset, as `0x44BCD0` does), so `door` sees only the doors that an object's event turns (#480).
- A goblin warrior of 51,39 (type 76, level 1) at the Seraphim of level 1: 77 strikes, 1.27 to 1.60 s apart (1.49 s on average); 22 of them landed (29 %), for 6 to 11 of her 119 health; its attack against her defence 12.02 against 26.4, a share of 0.313 at the hit's roll; her four resistances against its damage 2.24, 1.77, 1.52 and 1.75.
- The Seraphim at the goblin: attack 28.5 against its defence 13.39, a share of 0.680; a hit took 26 of its 52 health.
- Six kills of that goblin: each called the drop once (type 0, bonus 0, value 0, the killer 1, read again by #561) and gave 27 experience; two dropped an item (the types 2022 and 3639), four nothing.
- The sounds play as samples loaded from memory (`AIL_set_named_sample_file` with the name `.MP3`), each after a pick of its sound id (the profile 104's group 1, the ids 1201 and 1202, while the hero stood); the music and the ambient loops are named by the game's own log (`cMSS::playMusic (…)`) rather than by `AIL_open_stream`, which opened nothing in those minutes.
- With `--muted` the game plays its sounds at the installation's volumes while the agent sets every volume of mss32.dll to 0 (5,942 settings in five minutes) and the drivers' master volumes to 0 as they open.
