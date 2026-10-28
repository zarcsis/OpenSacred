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
The scales of the nodes above a socket still place it: eleven items' sockets lie under a scaled node (docs/02 §4.2), and the daggers placed without that scale were held a hand's width off their grip, where the original's are in the fist (#814).

## The item's model

Read 2026-10-26 (#1132) in the code, and checked in the original with the driver of #448: the model loader `0x412590` and the slot's attach `0x555300` traced through a new game of each class.

- An item is drawn with the model that its type's items-table entry names at `+0x37` (`0x426250`) and the texture of its `+0x08` (`0x4262B0`), loaded by `cObject3D::grnSequenceLoad` (`0x44C330`); the draw of an object (`0x44B230`) sets that texture again as the draw's texture override where none is set (**confirmed**).
- A fixed item's entry is its root's: the items table's loader (`0x4257A0`) copies each record's parent's entry of 128 bytes into the child's, in `Weapon.pak`'s order, keeping the child's `+0x18` and `+0x20` (**confirmed**).
  So the Daemon's 7908 draws `ORCHIDEE.GRN` (7908 → 7803 → 1773) with the texture 8578, a golden clawed hand in her right hand, and the Vampiress's 7906 the long `SWORD.GRN` with 8639, on her `Bip01 R Hand`, her skeleton having no `Bone_weapon_01` (**seen** in the traces of their new games).
- The class's variant that `0x555300` asks of `0x43CA20` before it attaches an item is never one for a hero: the switch answers none for a creature type below 32 (docs/re/items.md), so `bin/Rust.bin`'s entries of the types 1 to 6 are not used for the heroes, and the types 8 and 9 have none (**confirmed**).
- The overlays `PAK\ITEMS%.2d.PAK` (`0x429190`, after the hand-made patches of `0x4293B0`) put each of their records of a category other than 0 into the slot that its own `+0x76` names, over a base entry with a logged warning, their `+0x08` and `+0x66` raised by an offset of their pak's textures; `Items03.pak` has one such record, 3999 (**confirmed**).

## Worn items and the classes' defaults

- The slots 0–7 and 14–16 (`0x401A70`), and the fist weapons 4067 to 4072 in a hand (`0x43EE30`), are worn: the item's model, with its own skinned meshes and its copy of the skeleton, is merged onto its wearer's skeleton by `GrannyWearSequence` (`0x44C980`) and drawn after the wearer with its type's texture (**confirmed**).
- The client hides none of the wearer's meshes for a worn item: no mesh's name is in the executable, and the draws draw the whole base model, then each slot's model (**confirmed**; whether `granny.dll`'s `WearSequence` hides any is not read, **hypothesis**: it does not).
- A hero's class has a default item in a slot (`0x426580`): the Seraphim 4007 in slot 0, the Gladiator 4054 in slot 2, the Battle Mage 3219 in slot 0, the Vampiress (type 6, not her night's 7) 4028 in slot 0 and the Dwarf 3018 in slot 0; the Wood Elf, the Dark Elf and the Daemon have none (**confirmed**).
  The unequip (`0x556A30`) makes the default anew in its slot whenever that slot empties, giving nothing back when the item taken off is the default itself, and the hero's loader (`0x6058A0`) fills every such empty slot after a load; so such a slot is never bare.
  These five are the templates' cosmetic items (`items.pak +0x03` bit 1).
- With nothing in a slot, a hero shows its base model's own meshes, the Daemon's one mesh holding her wings, horns and red armour.
  In a new game (**seen** in the traces): the Seraphim wears `SeraHair01.grn` (texture 7363), the Gladiator his white kilt and belt `Gladiator_belt.grn` (7204), the Battle Mage his hood `magician_cowl.grn` (7319), the Vampiress her white hair `vlady_d_hair.grn` (7372) and the Dwarf `dwarf_goggles.grn` (7178); the Daemon wears nothing but her model.
- The Underworld's sets (`daemonia_armor0N_*`, `dwarf_*_*`, `vlady_*`) are item types of their own models and class bits (`Weapon.pak +0x84`), worn by category in the slots 0–5, 15 and 16; no generic piece is remodelled for a hero's class.

## The choice of the hero's figures

The figures of the choice of the hero (docs/re/menus.md) do not wear their templates' items but a showcase of a table compiled into the client at `0x9E3248`, loaded by `cUI_NetGranny` (`0x6F3E90`; **confirmed** by code and by a trace of the screen).
The table has eight entries of `0x50` bytes in the screen's order (the Gladiator, the Seraphim, the Daemon, the Dwarf, the Wood Elf, the Battle Mage, the Dark Elf and the Vampiress; docs/02 §6.8): `+0x00` the class, `+0x04`…`+0x20` eight item types, `+0x24`…`+0x30` the figure's position and its turn in degrees, `+0x34`…`+0x3C` three floats not read, and `+0x40`…`+0x4C` the figure's window in the screen's pixels, in which the cursor lights it (**seen**).
The position is of a world of Z up, X to the screen's right and Y towards the camera; the turn turns the figure about Z, its model's front, its +X, facing the camera at 0 and turning to the screen's left as the turn grows (the orientation that `0x6F3E90` gives the figure; **confirmed** by the frames).
The figures' camera (`0x6F50B0`) has its eye 100 units from the origin, a little below it at a pitch of −0.0592 rad, looking at it with Z up, a horizontal angle of 69.7°, a vertical one of 0.75 of that, its near plane at 10 and its far one at 1000, over the whole screen; so the Gladiator's feet at (−100, −82.5, −59) stand at the pixel (101, 663) (**confirmed** by code and by the frames).
The code at `0x70C710` puts the eight types into the slots 13, 12, 1, none, 0, 4, 2 and 17, so the fourth (the boots) is never shown, and `0x6F4E60` fills an empty slot 0 or 2 with the class's default.
`0x6F3E90` loads the models without textures, wears the slots 0–7 and 14–16 and the fist weapons onto the figure, and attaches the slot 13 on the figure's own `Bone_weapon_01` where its skeleton has one, else `Bip01 R Hand`; the slot 12 on `Bip01 L Forearm` for a shield, else on `Bip01 L Hand`, by the item's `Bone_weapon_02`; and the slot 17 by the item's `Bone_spine` on `Bip01 Spine2`; the draw (`0x6F1B30`) gives each item its type's texture.
So the Seraphim's figure wears her `SeraWings01.grn` (4006, category 25) on her back, which no hero wears in a new game.

| Figure | Slot 13 | Slot 12 | Slots 1, 0, 4 and 2 | Slot 17 |
|---|---|---|---|---|
| Gladiator | 1726 `sword_2handed.GRN` | – | 4054 `Gladiator_belt.grn` in 1, and in 2 as the default | – |
| Seraphim | 4008 `SeraWindslicer.grn` | – | 4004 `SeraArmor01.grn`, 4007 `SeraHair01.grn`, 4001 `SeraGloves01.grn`, 4005 `SeraBelt01.grn` | 4006 `SeraWings01.grn` |
| Daemon | 1242 `daem_schwert01.grn` | 1246 `daem_shield01.grn` | 1220, 1222, 1221, 1219 (`Daemonia_Armor01_*`) | – |
| Dwarf | 1853 `muskette_2.grn` | – | 3009, 3012, 3011, 3008 (`Dwarf_leather_*`) | – |
| Wood Elf | – | 1743 `BOW_ELB.GRN` | – | – |
| Battle Mage | 1716 `STAFF_FIGHT.GRN` | – | 3219 `magician_cowl.grn` in 0 | – |
| Dark Elf | 1865 `weapon_Dunkelelf_04.grn` | 1862 `weapon_Dunkelelf_01.grn` | – | – |
| Vampiress | 1726 `sword_2handed.GRN` | – | 4028 `vlady_d_hair.grn` in 0 | – |

A figure plays its model's motion of the slot that the table of `0x956E40` gives (`0x6F4C58`; docs/re/motions.md): of the standing's row by the class of what it holds, the right hand's weapon's kind (`Weapon.pak +0x1E`), 8 for a weapon in each hand but a shield, else the left hand's; the Vampiress and the Dark Elf of the fight stance's row, from slot 14 (**confirmed** by code and by a trace of the screen: the Gladiator's slot 3, the Seraphim's and the Daemon's 2, the Dwarf's 13, the Wood Elf's 8, the Battle Mage's 5, the Dark Elf's 22 and the Vampiress's 16).
The motions turn the figures as they play: the Dwarf's turns him from the camera to the screen's right and back, the Wood Elf's swings her bow from across her body towards the camera (**seen**).

## In `opensacred-game`

`npcs` gives each NPC of the scripts the items of its record, and each of the fills those of its `MakeGroup` rule (docs/re/spawning.md), in the client's hands: the item's model goes onto the NPC's bone with the item's socket at that bone, by the socket's position and orientation from the model's root, and wears its item's texture (`items.pak +0x08`).
The position is by the whole chain of the model's nodes, their scales included, and the orientation by the product of their rotations, which is the rotation of the whole chain in every item (`npcs::Chain`, #814).
The weapons' kinds come from `Weapon.pak` by the item's slot, which is the weapon record's id (**hypothesis**, docs/02 §6.2).
A fixed item, a record of `Weapon.pak` without an `items.pak` slot of its own (the heroes' templates' weapons, 7900 to 7909), takes its parent's slot (`Weapon.pak +0x24`, followed up to eight steps), its model, texture and category, as the client copies the parent's definition into the child; its kind stays its own (#488).
The hero holds the items of its template (docs/02 §9) that go in a hand, as an NPC's go, their models on its bones alike (#488): the Seraphim her weapon 7901, the model of its parent 1724, in her right hand, as the original's does from the start of a new game.
The hero wears its template's items that it does not hold and that go on no bone of its own, the classes' cosmetic items, since #1131 (`npcs::worn_gear`): each item's model goes under the hero, and its skinned meshes take the hero's bones of their joints' names as their joints (`npcs::bind_worn`), with its type's texture, so that the Seraphim's hair, the Gladiator's kilt and belt, the Battle Mage's hood, the Vampiress's hair and the Dwarf's goggles show and move with them.
Since #1307 the hero wears the items put on in the equipment window (`equipment::dress`, `hero::Hero::dress`): those of the slots 0 to 7 and 14 to 16 on its skeleton as its template's are, the amulets of 6 and 7 among them since #1334, the wings of the slot 17 on its back; each takes off its template's item worn in its slot, the first slot that takes that item's category, so that a helmet takes off the Seraphim's hair and a belt the Gladiator's, and the default comes back as the slot empties, as `0x556A30` makes it anew (**seen** 2026-10-28: the Seraphim's black helmet 3005, armour 3003 and wings 3111 worn in both games alike, her hair under the helmet in neither); the rings (the slots 8 to 11) are not drawn yet (**seen** 2026-10-28, #1334: the Seraphim's brooch 4776 on her chest in both games), nor the wings' glow of the original (#1309).
The choice of the hero's figures stand, wear and move as the original's since #1130: `opensacred-convert tables` writes the table of `0x9E3248` into `tables/figures.ron`, and `choice` stands each figure at its place and turn under the original's camera, in its items by their slots and its class's default (`npcs::figure_gear`; the Seraphim's wings on her back), playing the motion of its slot, and lights it while the cursor is in its window.
An item is attached four frames after its holder's scene is ready (`npcs::SETTLES`): Bevy's spawner may spawn a scene's instance anew within its first two frames, despawning the holder's bones with the items on them, and an item's scene whose spawn is still queued then panics on its despawned root; a new game started at once after the last one was ready did so in about a third of the runs once the hero held a weapon, and in none of 16 with the wait (#488).
