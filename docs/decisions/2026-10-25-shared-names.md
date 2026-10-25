# Quests 2203 and 501 keep their count and their group apart from others'

Scripts (`opensacred-convert netscript`), 2026-10-25 (#943, #951); the maintainer's decision.

## The original

Two quests share a name of the scripts with another quest, which the client keeps global (docs/re/netscript.md, "Variables and groups"):

- **2203 (Pure Water):** it counts its six objects in the variable `2303a` (`obj2203`), whose bit 2 quest 2303's dialogue of its win sets once (`Dialog:ziel2303`), a bit that nothing reads.
  `SetVarBit` ORs the bit in, so with 0 or 1 object taken as the dialogue runs, 2203's test of six holds at the second object, and with 2 or 3 never, so the quest cannot be won (#943).
- **501 (Nucquam Astralis):** `QIS_OnEnter501` makes 13 ghosts in the group 5001, the number of quest 5002's camp.
  The groups are global by their number, so 5002's test of its camp waits for 501's ghosts wherever they are, its attacks send the group 5002 at them, and its removal destroys them (#951).

## Ours

`opensacred-convert netscript` mends the records as it writes the trees (`script_repairs`), in the 11 trees that hold the quests:
- `Dialog:ziel2303` loses its `SetVarBit` of `2303a`;
- the 13 ghosts of `QIS_OnEnter501` take the group 50014, which no record of any tree names, beside 501's other groups 50012 and 50013.

`QIS_OnEnter501` still makes again the four ghosts of the group 551 that `Region5Init` made, so that `exx501` waits for eight: that doubles a fight and blocks nothing.

## Why

In each case another quest's record reaches into a quest through a name they share by chance, and nothing tests the other quest.
The write removed is read by nothing, and no record tests or removes 501's 13 ghosts by their group, so neither change touches the quest that owns the name.
The repairs change those quests' courses: 2203 counts its six objects whatever 2303 does, and 5002's camp is its own.
