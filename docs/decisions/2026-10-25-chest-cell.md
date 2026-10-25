# The chest whose cell patch is cut short has its cell closed like the other chests'

Scripts (`opensacred-convert netscript`), 2026-10-25 (#723); waits for the maintainer, as a repair of a script.

## The original

Every start script creates the trigger `chestTrigger` closed and names, before each chest's `CreateObj`, the chest's cells in `TriggerPatch` records, so that no creature steps onto a chest (docs/re/movement.md, "The chests' cells").
In the start script that the 9 Underworld trees and the 2 multiplayer trees share, the patch before the chest `chest_Pilzwald_14` (object 1339) ends with the tag of its cell, `0x2A`, and no payload: the record is 12 bytes short (docs/02 §7.1, docs/re/netscript.md, "Truncated operands").
The client's handler of `0x2A` reads those 12 bytes past the record, from whatever follows it in the copy buffer.
What it reads there is not read (**hypothesis**: bytes of another record, which name no cell of the chest), so in the original that chest's cell is most likely open, where the cells of the other 21 chests of its object in those scripts are closed.

## Ours

`opensacred-convert netscript` gives the truncated payload the cell of the chest whose `CreateObj` follows the record, (1641, 911, 0), as it writes the trees (`script_repairs`): the chest's cell is closed like the other chests'.
The repair names the 11 trees, the trigger and the chest; a tree whose start script lacks such a record is left as it is, with a warning.
`opensacred-formats` still reads the record as it is.

## Why

The record is a defect of the data, not a rule of the play: a payload cut off by its record's end is read from bytes that mean nothing to it, and every other chest of the script has its patch on its own cell, as 40,010 of the patches of `chestTrigger` over the 20 trees name the cell of the object after them.
The alternative, dropping the payload, would keep the chest's cell open, the most likely outcome in the original, but would keep the one chest unlike its siblings for a reason that is only the file's.
The repair changes a script, though only one chest's cell in the Underworld and the multiplayer trees, which the game plays only when `--tree` names one; so it waits for the maintainer's decision.
