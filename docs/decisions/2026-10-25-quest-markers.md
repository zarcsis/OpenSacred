# Quest 61's marker and Jacquaire's spawn follow the quests

Scripts (`opensacred-convert netscript`), 2026-10-25 (#958, #959); the maintainer's decision.

## The original

- **61 (the IceCreek Pass):** taking the element of the air ends `NimmtElementLuft_2` by pointing the compass at the 5.7 dungeon's mouth by Frostgard. That overrides the compass that `ElementAbfrage` points at the mage once the four elements are held. The dungeon's entrance by Frostgard (`OMO-1tpreinHQ_5.7.4`) has no branch for the elements of the air and the fire both held (bits 6 and 9 of `61`). So the marker keeps pointing back at Frostgard's side until the hero leaves by 5.7.1 or 5.7.2 (#958; that this is the report is a hypothesis, a strong fit).
- **9006 to 9011 (Jacquaire):** Jacquaire 17523 is made in Braverock only by the trigger that 9006's end places on a 6 × 6 box in region 6. A hero who leaves another way never makes him, and quests 9007 and 9011 wait on him (#959).

## Ours

`opensacred-convert netscript` mends the records as it writes the trees (`script_repairs`):
- `NimmtElementLuft_2` points the compass at the mouth only where bit 12 of `61` is clear, the four elements not yet held, in 9 trees.
- `OMO-1tpreinHQ_5.7.4` gains a branch for bits 6 and 9 both set, which points the compass at the way on, the cell where the hero comes in by the entrance 5.7.1 (5536, 2291), in 20 trees.
- `rg6_goodie1_start2` makes Jacquaire as 9006's step ends, before it places the box, and the box's function no longer makes him, in 11 trees. A second `CreateNPC` of his name would only teleport him back from where 9011 takes him.

## Why

The compass points the wrong way for a hero who took the air last, and the marker is the only guide through the dungeon.
Jacquaire's spawn hangs on one box of one road, so two quests may never come.
The repairs change where the marker points and when Jacquaire appears: at once, in Braverock, rather than once the box is crossed.
