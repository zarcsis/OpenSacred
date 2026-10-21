# The instruments' socket

Models (the converter), 2026-10-20.

## The original

The four instruments that the jugglers play, `Instrument_Floete`, `…_Glocke`, `…_Harfe` and `…_Horn` (models 0746 to 0749), name their socket `Weapon_bone01` or `Weapon_Bone01`, where every other item has `Bone_weapon_01`, the right hand's socket that the client looks up (`0x44C670`, docs/re/equipment.md); how the client attaches them all the same is a **hypothesis** (docs/re/ai.md).

## Ours

`opensacred-convert models` names that bone `Bone_weapon_01` in those four files (#353), and the game attaches an instrument by the right hand's socket like any other item.

## Why

The repair of the data belongs to the converter, not to the game, which until then fell back to the instruments' spelling (CLAUDE.md, "Code conventions").
The guess of how the instrument sits in the hand does not change, nor does the play.
