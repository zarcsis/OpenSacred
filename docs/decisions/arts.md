# Combat arts

The heroes' combat arts and spells (docs/re/arts.md).

| Decision | Ours, against the original | Why | Kind | Issues |
|---|---|---|---|---|
| Combat master's button lit where a press makes the combo | The combat master's button lights, and its total shows in the affordable yellow, only where the gold is above the cost, as its press asks (`combos::affordable`, `makes`), where the original's lights them with the gold equal to the cost too, a press then making nothing (`0x6D26D0`, `0x6D22A0`; seen). | The window tells whether a press makes the combo; the price and the press stay the original's (#1737's rule). | repair | #1540, #1743 |
| Daemon's blast edge and level | A creature beyond the blast's radius `R` takes no strike, where the original's unsigned `R − d` gives it a killing blow, and the knockback's chance takes the charge's level for the object's level word. | The unsigned difference is a defect that the factor's form, falling to 0 at `R`, does not ask for, and the level word has no value of its own on that way. | deviation | #760 |
| Fireball's start and level flight | The Mage's fireball leaves from where he stands and flies level at the bolts' height along the line on the ground, where the original's flies along a direction in three dimensions whose height's part shrinks at each frame. | Where the original's fireball starts and at what height is not read. | deviation | #754, #872 |
| Spells page's key cleared with its binding | A key F1 to F6 pressed again over the art that holds it takes its label off and the hero's binding with it (`Arts::unbind`), where the original's takes it off the page's list alone, so that the key still names nothing until the page's next showing puts the label and the binding back (`0x6C1F30`). | The label tells which key makes the art (#1737's rule). | repair | #1536, #1743 |
| Seraphim's jump rate at least 1 | The Seraphim's jump plays its clip at a rate of at least 1, where the original's rate is 0 from 495 to 500 and holds her in the art. | A clip that stands still plays as a defect of the original rather than a rule, and the rate beside those six units is 1. | deviation | #754 |
