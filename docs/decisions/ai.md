# AI

The creatures' life while no foe is near (docs/re/ai.md).

| Decision | Ours, against the original | Why | Kind | Issues |
|---|---|---|---|---|
| Parting and stepping aside simplified | Every creature counts only its own cell, not the eight about a flyer or a big model, the `+0x200` bit `0x10` and a script's hold do not keep it from parting, and a creature steps aside whatever the hero's AI state. | The models' sizes, the scripts' hold and the hero's AI states are not read into the game, and without them a few creatures part or step aside where the original's would stand. | deviation | #240, #247, #251, #639 |
| Townsfolk's traders, doors and danger | A trader's visit ends at the trader rather than in a house, and a townsperson goes into a house by the door's points as the game measures them, giving up where it stops short rather than trying another door. In danger it runs 700 from the threat, finishing its walk first, rather than into the nearest house, and the fights' targets put none in danger. | The client's table of the doors' points (`0x43B290`) is not read, and the run is what the original's townsperson does where no door is near. | deviation | #247, #248, #251, #254, #431 |
