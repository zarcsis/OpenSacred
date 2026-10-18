# Equipping a creature: the hands and the sockets

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra 12.1.4 on 2026-10-18.
The item categories and the weapon kinds are in [docs/02 §6.1](../02-data-formats.md#61-itemspak-itm-v5-the-object-template-database) and [§6.2](../02-data-formats.md#62-creaturepak-cif-and-weaponpak-wpn); the `CreateNPC` record is in [§7.3](../02-data-formats.md#73-the-opcode-space-141-and-113).
Status: **confirmed** on the code unless marked otherwise.

## The slots

A creature has 19 equipment slots, a word each from `+0x1A4` (slot 12 at `+0x1D4`, slot 13 at `+0x1D8`).
`0x401A70` sorts them in two: slots 0–7 and 14–16 are worn (merged into the creature's skeleton), slots 8–13, 17 and 18 are attached to one of its bones.
Slot 12 is the left hand, slot 13 the right; by the bones below, slots 8–11 are the rings' fingers, 17 the back and 18 a horse.

## The hand an item goes in

`0x54ACC0` picks an item's slot from its category (`items.pak +0x2E`) and, for a weapon, its kind (`+0x1E` of its `Weapon.pak` record):

- a shield (category 13) goes in the left hand;
- a weapon (category 5) of kind 7 (the bows) or 12 (the crossbows) goes in the left hand; any other weapon, and one without a `Weapon.pak` record, in the right;
- the other categories go to the other slots (rings, amulets, armour), or none.

`0x54B130` equips an item into that slot when it is empty.
When the slot is the right hand and it is taken, a weapon goes in the left hand if that is empty (a second blade).
Otherwise the item is not equipped: a left-hand item whose hand is taken does not move to the right.

The `CreateNPC` handler (`0x482510`) keeps the record's `0x02` operands after the first, the creature, in their order, and once the creature is made equips each of them so (`0x54B130`).
The start script's soldiers carry a sword and a kite shield, or a crossbow; its dark elves two blades.

## The bones

`0x555300` attaches an item of a bone slot by two bones: the item's socket and the creature's bone that takes it.

- **The item's socket**: `Bone_weapon_02` in the left hand, `Bone_weapon_01` in the right and in the other slots, `Bone_spine` on the back (slot 17), `Bip01` on a horse (slot 18).
  The names come from a table of 32-byte strings at `0x8E7200` (`0x401000`).
- **The creature's bone** (`0x401AE0`, by the slot and the item's category): a weapon's is the hand, `Bip01 L Hand` or `Bip01 R Hand`, but the creature's own `Bone_weapon_02` or `Bone_weapon_01` when its skeleton has one (the heroes' skeletons do, under their hands; on the right, not for a creature whose model's name holds `_OGRE`); a shield's is the forearm, `Bip01 L Forearm` or `Bip01 R Forearm` (`0x44C670` tries the spelling `Bip01 L ForeArm` when the first is missing); a ring's (category 8) is `Bip01 L Finger31` or `Bip01 R Finger31`; an amulet's (category 20) `Bip01 Neck`; the back's (category 25, the Seraphim's wings) `Bip01 Spine2`; a horse's `horse_bone`.
- A left-hand item that is not a ranged weapon and whose model lacks `Bone_weapon_02` goes to the right hand when that is empty.
- The dwarf (creature 8) holds its left-hand item by its own `Shildbone` when its skeleton has one.

`0x44C670` then looks both bones up by name and calls Granny's `AttachSequence` with the item's socket, the creature's bone, a mask of `0x3F` (0 for a horse) and `0x1FF`.

**Hypothesis**: the mask `0x3F` attaches the socket's position and orientation, not its scale.
The dark elves' blades (`weapon_Dunkelelf_*.grn`) have sockets of scale 0.23 and meshes as long as a sword's; attached with the scale, they stood four times too long.

## In `opensacred-game`

`npcs` gives each start script's NPC the items of its record in the client's hands: the item's model goes onto the NPC's bone with the item's socket at that bone, by the socket's position and orientation from the model's root, and wears its item's texture (`items.pak +0x08`).
The weapons' kinds come from `Weapon.pak` by the item's slot, which is the weapon record's id (**hypothesis**, docs/02 §6.2).
The heroes have no equipment yet.
