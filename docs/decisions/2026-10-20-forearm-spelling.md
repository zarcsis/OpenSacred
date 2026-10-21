# The forearms' two spellings

Models (the converter), 2026-10-20.

## The original

176 model and motion files name the forearms `Bip01 L ForeArm` and `Bip01 R ForeArm`, the others `Bip01 L Forearm` and `Bip01 R Forearm` (docs/02 §4); the client looks a shield's bone up as `Forearm` and tries `ForeArm` when that is missing (`0x44C670`, docs/re/equipment.md).

## Ours

`opensacred-convert models` writes both spellings as `Forearm`, in the nodes of every file and in the binding of the clips to them (#355), so the game looks for one name.

## Why

The repair of the data belongs to the converter, not to the game (CLAUDE.md, "Code conventions"), and a clip played on another file's skeleton finds its bones by the chain of their names, which one spelling keeps alike.
No skeleton has both spellings, so no two bones take one name; what the player sees changes only where a shield now finds its forearm.
