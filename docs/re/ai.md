# AI: the creatures while no foe is near

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra 12.1.4 on 2026-10-20 (#232; the townsfolk #241).
Distances are in world units, 53.66563 a cell.
Status: **confirmed** on the code unless marked otherwise.

## Two states

A creature carries two states, each a short with a sub-state after it: its AI's (`+0xFC`, `+0xFE`) and its action's (`+0x150`, `+0x152`), which the creatures' update `0x5375C0` switches on at every frame.
It runs the AI only for a creature whose simulating client (byte `+0x39`) is the local player or none; the creature maker sets it to the local player for every id above 16, so in the single player's game every creature thinks.

- The AI's states: 0 idle (`0x542B20`), 1 (`0x53A170`), 2 a behaviour or a walk ordered (`0x4EE030`), 4 a melee fight (`0x4E0300`), 5 (in the update), 6 (`0x51CFD0`), 8 a fight at range (`0x4DBFC0`), 9 dead (`0x53E7C0`), 10 to 14 (`0x529C60`, `0x4FD0F0`, `0x4EC560`, `0x4EA890`, `0x4EAC80`).
- The actions: 0 standing (`0x544890`), 1 moving (`0x4F4770`), 2 (`0x546690`), …, 8 (`0x4FF0B0`), 10 (`0x506770`), 14 (`0x514790`).

## Dormant and awake

An idle creature (AI 0) is dormant in its sub-state 0 and looks about in 1; there is no other sleep.
Three things wake it:

- its own timer: an accumulator (`+0x208`) against its period (`+0x204`: 5 s, 2.5 s for a hostile creature of `CreateNPC`), for a creature whose flags word (`+0x1F4`) has the bits `0x3` or `0x408`; each time it fires it is jittered by −1.5 to +1.5 s, so the next comes 3.5 to 6.5 s later;
- `sdk_WAKE` (`0x59F580`), which `CreateNPC` calls for its hostile creatures, so that they look once as they are made (it skips a creature of `+0x200` bit `0x40000`), and which runs a creature's script on sight (`+0x1F4` bit `0x400000`, `CreateNPC`'s tag `0x60`) when the hero comes within 600 × 400 in the light and in a clear line;
- the hero's look about (`0x547070`, every 0.3 s over the 3 × 3 sectors about it, within about 1,875, docs/re/motions.md, "The fight stance"): it wakes the creatures not hostile to the hero at any distance of that range, and those hostile with bit 0 of their flags within 800, or 400 while the hero moves; to every other hostile one in range it gives the idle action (below), at every look in the single player's game; a creature not hostile within 91 of the hero that moves under an order of its own (its action and its AI's state both other than 0), while the hero's AI state is 0, steps aside: it walks to the first of 10 points at random within 200 of itself along each axis that it can reach by a way.

Awake (`0x542B20`), a creature farther from the hero than 1,500 along the grid's x or 1,000 along its y goes dormant again.
A peaceful one whose flags match `0x1006DCF8` takes its behaviour (AI 2, below).
A hostile one, in one case of six and where its home's cell lacks bit 2 of its flags, walks home when it is more than 200 from it along an axis, else stops; then it weighs its level against the hero's (by the balance's `BaseOffLevel`, `OffLevelProz` and `UnterExpAnteil`, 0.85; **hypothesis**: a monster far below the hero's level fights only when touched): when that test fails and the hero is out of its reach (the mean of two sizes of the types, 70 to 480, 130 by default, `0x4DBC60`), it goes dormant again and in one case of three takes the idle action; otherwise it attacks, the hero its target (`+0x104`), from up to 800.

## The idle action

`0x54EA30` acts only on a creature that stands (AI 0, action 0, `+0x152` below 100), that no one owns and whose `REFKEY` is its type; when its group fights it alerts the group instead ("Groups" below).

- In one case of ten it fidgets: half of those times with one of its model's `*_FUN_IDLE_*` clips (the slots 180 to 192, each tried with a chance of one half; 39 models have one), the other half with its attack's clip.
- Otherwise, in one case of four (22.5 % in all), it takes a step: a creature that an `Init` function or the start script made walks to a point within 150 of itself along each axis, or home in one case of four; one that an `Enter` function made (`+0x14` bit `0x80000000`, `FillSector`'s among them) walks to its place turned about the hero by −30° to +30°, keeping its distance, and its home moves there.
  The step is an order to walk somewhere (the event 4: AI 2, then the moving action), which sets no gait: the moving action walks a creature whose gait (`+0x1F8`) is 0 (**hypothesis**: one left running by a chase runs).
- A hostile creature 800 to 1,875 from the hero so steps about once a second while it stands, and its own timer adds a roll every 5 s or so; farther than 1,500 it stands.
- Its home is where it was made (`+0x2B9` its sector, `+0x2BD` and `+0x2C1` its place, `+0x2C5` its layer), set by the creature maker (`0x480D40`).

## The peaceful creatures' behaviours

A peaceful creature awake takes the behaviour of its flags (AI 2, `0x4EF150`), run again at every frame while it stands, so that its chances below are a frame's (1 in 101, about every 3.4 s at 30 frames a second).
The AI's state 2 (`0x4EE030`) passes it one flag, the first of its word in this order: 4 (it follows a partner, only while it stands), `0x100`, 8, `0x8000` (passed as 8), `0x10000000`, the animals' `0x10` to `0x80` (only while it stands), `0x4000`, `0x400`, `0x800`, `0x1000`; so the townsfolk's flags 8 and `0x400` run at every frame, moving or not, and a creature with both runs as 8.
Its distances to the hero and to the others are straight lines (`0x5370B0`, which caps them at 32,000), not along the grid's axes as the idle creatures' (`0x542B20`):

- wild animals (`0x40`, `0x80`): dormant again farther than 1,500 from the hero (the timer set by the distance); they flee at a run from the nearest creature of their sector of a class other than 4 and 6, the hero among them, within 300 (800 if it attacks), and the flyers take off; farther than 380 from home along an axis they walk back within 50 of it; and by chance, or when their cell is crowded, they walk to a point within 100 of home;
  the flight goes, along each axis of the grid, away from the one feared by a number at random between half and twice the distance from it along that axis, and across by one between minus and plus half the distance along the other (`0x443B20` gives a whole number at random between two, both included); a flyer on the ground within 200 of it takes off instead (`+0x1F4` bit `0x200000`, the run's gait 1), its target scaled by 400 (**hypothesis**: 400 away along the line from the one feared);
- farm animals (`0x10`, `0x20`): dormant again farther than 1,500; they step away from a creature other than an animal within 200 (800 if it attacks), to the point beyond themselves; and by chance, or crowded, they walk to a point within 150 of home;
- guards (`0x4000`): dormant again farther than 1,500; by night those of an odd id take a torch (type `0x1600`, in the slot 12; by day they put it away); they attack a foe within 500; else by chance one without a dialogue (below) strolls to a point within 200 of its post along each axis, from wherever it stands, and otherwise it walks back to its post from farther than 90 along an axis, so that a stroll that ends beyond 90 brings it straight back;
- the flags `0x800` and `0x1000`: the creature stands;
- the townsfolk (`0x8`, `0x400`): below, "The townsfolk".

A creature's dialogue is `+0x245`, a number that `sdk_STAMP_IDX` (`0x5498F0`) writes, from `CreateNPC` and the scripts, and that `0x549920` gives as its talk (or, without one, a line of the generic townsfolk's in one case of three); a creature with one is held by its talk: a guard does not stroll, a townsperson keeps still, a crowded creature does not part.

A creature that stands idle (AI 0, the standing action `0x544890`) and shares its cell with another, in one case of six a frame, parts: it walks to a point within 100 of its home or of itself.
The cell's test (`0x54FCB0`) counts every other creature in it, the types of the categories 2, 3 and `0x20` (`0x426610`), the hero among them, and, for a flyer or a model wider than 60 or deeper than 110, in the eight cells about it too.
The choice between home and itself measures, of each point, its y from its x, and takes the distance of the two differences (`0x545B30`), not the distance of the creature from home: between 30 and 400 it parts about home, else about itself, and past ±32,000 for either difference it does not measure and parts about itself, which the whole world but a band along its diagonal is.
A creature of `+0x1F4` bit `0x8000000` (`CreateNPC`'s tag `0x74`), of `+0x200` bit `0x10` or of the bits `0x4007000` (the traders, below), one with a dialogue (`+0x245`) or one that a script holds (`0x4A71F0`) does not part.
A creature of a behaviour that stands crowded wakes (`0x544890`, at `0x54647A`); an animal awake strolls at once where another shares its cell, as by its chance (`0x4EF150`), and the townsfolk's machine reads the cell too (at `0x4F0D44`).

The flags come from `CreateNPC`'s tags (`0x2B` 8, `0x2E` `0x10`, `0x30` `0x20`, `0x2F` and `0x31` `0x40`, `0x32` `0x80`, `0x42` `0x400`, `0x43` `0x800`, `0x44` `0x1000`, `0x46` `0x4000`, `0x4C` `0x8000`, handled as 8; docs/02 §7.3), from the creature maker's lists of types for a peaceful creature made without flags, as `FillSector` makes them (a hostile one bit 0 alone), and, for a creature of an `Enter` function still without any at its first update, 1 if it is hostile to the hero, `0x20` for class 6, `0x10000000` for the jugglers (78, 79, 82, 104), else 8.
The creature maker (`0x480D40`) gives `0x40` to the types 512 and 567 to 570 (the wolves), 513, 589 and 590 (the crows), 514, 515 and 594 to 596 (the bats), 516 (the first rabbit; not its variants 592 and 593), 530 to 533 (the deer), 536 to 538 (the bears), 540 and 541 (the bulls), 543 and 545 (the wild boars), and `0x10` to 520 to 528 (the pigs and the cows), 544 (the sheep), 550 to 558 (the horses), 560 to 563, 565 and 566 (the chickens); `CreateNPC` writes the word of its tags over them (`0x4839F6`), so that its creatures without a tag keep none.
`Creature.pak`'s class 6 holds all these animals, wild and tame, and class 4 the horses ridden.
A creature of an `Init` function or of the start script without a tag keeps none and stands.
In the Seraphim's tree the `Enter` functions' `CreateNPC`s are mostly townsfolk (596 of `0x2B`, 570 of `0x42`), 166 guards, and animals (150 of `0x30`, 131 of `0x2E`, 26 of `0x32`, 5 of `0x2F`/`0x31`); the start's novices carry `0x42` or `0x2B`, its farmers `0x2B`, its pigs `0x2E`, its chicken and rabbit `0x30`.

## The townsfolk

The townsfolk are the creatures of the flags 8 and `0x400` (`CreateNPC`'s tags `0x2B` and `0x42`, `0x4C` as 8, and the `Enter` functions' creatures without flags) and the jugglers of `0x10000000`; their branch of `0x4EF150` starts at `0x4F0A6A`.
They have no timer of their own: most of what they do ends in the AI's state 0, and they decide again when the hero's look wakes them, every 0.3 s within its range.

### Range, dialogue and the far ones

- One with a dialogue keeps still.
- Farther from the hero than 600 times the camera's zoom (`0x611400`, the world view's `+0xB60`, 0.5 to 2), in a straight line, it goes dormant, its timer set by the distance as an animal's.
  At dusk or by night, one on the ground is first put into a house there: from the nearest door within 1,500 of it (`0x4EEFA0`), 15 tries of a cell 2 to 9 cells away along each axis, either way, that a building covers (`0x6354D0`) and that it may stand on; the try farthest from the door wins, and it stands there on the building's ground floor.
  Nothing brings it out at dawn but its own day's way below, near the hero.

### Danger and the Daemon

- It is in danger when a creature of the hero's list of hostiles (`0xAD3CE0`, which the hero's look fills) is within 800 of it, the nearest such its threat, or when the nearest of that list fights, its threat then the fight's target.
- With the hero within 500 and of type 9 (the Daemon), the Daemon is near; while the hero's count `+0x2CF` is below 25 (**hypothesis**: the quests solved, which the rewards' handler `0x4AC940` counts), it is in danger too.
  On a free cell, in one frame of three, it then shows a line of text over itself by key (`DaeAh1` to `6`): red below 25; yellow in two cases of three below 50; else green in two cases of three, and it walks up to the Daemon, to within 50 to 150 of it along each axis.
- In danger a townsperson not running drops what it does but deciding, going in and arriving at a door.

### Deciding

Deciding (its sub-state `+0xFE` 0, `0x4F0D66`) takes the evening's way at dusk, by night, in danger or with the Daemon near, when its cell is free (`0x54FCB0`), and the day's way otherwise.

- The evening's way, outdoors, in danger or in five frames of seven:
  - by night (not at dusk), in the single player's game, with its left hand empty and not of the types 71 or 512 to 600 or class 6, it takes a torch (`TORCH_SMALL_ON`, type `0x1600`, slot 12; any odd id or not);
  - it walks to the nearest door within 1,500 but the last it used (gait 1, or 2 in danger): farther than 1,000, 700 toward it, within 150 more along each axis; farther than 350, to the door's outer point; within 350 it opens the door if it is closed and walks through it, to its outer point and then its inner one on the building's floor;
  - without a door it walks to a point within 400 of itself, or, in danger with a threat, 700 away from the threat along the line, within 100 more along each axis.
- The evening's way indoors, or outdoors in two frames of seven out of danger: it puts away what its left hand holds unless it is night outdoors (any item, not only a torch), and in 46 frames of 101 walks to the farthest of 15 cells 2 to 9 cells from it under a building, on its floor.
- The day's way: outdoors by day, or indoors, it puts away what its left hand holds; indoors, out of danger, on a free cell, in five frames of seven it leaves: through its door, or the nearest, opened if closed, to the first of 15 cells within 4 or 5 of the door's cell outside the buildings; otherwise, standing, it rolls.

### The roll

`rand() % 26` (`% 41` in a network game) at `0x4F22F0`; a juggler of `0x10000000` always rolls 9, the jugglers' types 78, 79, 82 and 104 in one case of six, any other 9 counts as 10, and a child always rolls 20.

- 0: it takes the nearest human creature for its partner (below) and follows it.
- 1: it walks to the nearest farm or wild animal of the 3 × 3 sectors (`0x4EE4E0`), and stays by it for 0 to 2 frames.
- 2: it takes a partner as for 0 and walks home.
- 3: it walks home.
- 4, 6, 7: it goes into the nearest house (below).
- 5, 8: one of `0x400`, and one of 8 in one case of three, visits a trader: the nearest creature of the 3 × 3 sectors whose `+0x200` has the bits `0x1000` (the smiths, `CreateNPC`'s tag `0x23`), `0x2000` (the merchants, `0x24`), `0x4000` (the wizards, `0x25`) or `0x4000000` (`0x64`), the second nearest if within 800 (`0x4EE660`); it walks to within 150 of it, plays its slot 177 (`*_TALK_A`) once and goes into a house.
  In the other two cases of three one of 8 looks for a field (`SetRG`'s regions of kind 1, `0xAAD2C8`, three in each tree, at the start's farmland) to work along it; the measure of the field's distance takes its cells for world units (`0x4B2350`), finds none, and it rolls again at the next frame: the branch is dead.
- 9: a juggler plays (below).
- 10 to 25: it stops, in its idle; standing within 24 of a door's point, it walks off to a point within 250 of itself.

So an ordinary townsperson: in 26 rolls one partner, one animal, one partner and home, one home, three houses, two traders (one of 8 rolls again instead in two cases of three), and 17 stops.

### Houses and doors

- The doors are the objects of the category 10 (`items.pak` 5650 to 5766, `Door_*`), which the `Enter` functions' `CreateObj` makes and the script manager keeps by sector (`0xAB4490`).
  The nearest door (`0x4EE300`) is one of its own sector within 300, else the nearest of the 3 × 3 sectors, whose trigger's word lacks bit 15 (**hypothesis**: locked); bit 0 of that word is open, and the event `0x21` to the door turns it (`0x4FC8D0`).
- Each door's type has an outer and an inner point, offsets from the door of up to 170 in world units (`0x43B290`, 94 types).
- Going in (sub-states 8, 9, 7 and 2): it seeks the door, walks to its outer point, opens it, walks to its inner point onto the building's floor, and closes the door behind it; one that stops outside tries another door farther than 150, reachable (sub-state 5), else gives up.
  Leaving (sub-state 3) closes the door behind it too.

### Partners

- A partner (`+0x251`) is the nearest creature of the 3 × 3 sectors, at any distance, of class 3 (`Creature.pak`), not of the heroes' kind, other than itself and its last partner (`+0x24D`; `0x4EE9C0`).
- Following (the flag 4, while it stands): with the hero farther than 1,500 it goes dormant; with a hostile within 300 of the hero, or its partner fighting, it flees 700 from the fight or stands; its partner farther than 900 it forgets; farther than 240 it walks to it at the partner's gait; within 149 it steps off to the first of 10 points 220 from it at a random angle that it can reach.
- Between 80 and 200 from its partner it meets it (`0x4FC490`) and forgets it: in one case of two, the adult turns to it, and the partner, idle, alive and no trader, stops and turns back; it plays its `*_TALK_A` once, or, a woman's type (`_FEM`) at 100 to 140, in one case of 11, its attack's clip (**hypothesis**: a slap).

### Children and jugglers

- The children are the types 229 to 231 and 702 to 705 (`0x43F670`); they always roll 20 (`0x4F2AD7`):
  - before a hero of type 7 (the Vampiress) or 9 (the Daemon) they flee;
  - else in one case of 13, or when the hero's cell lacks bit 1 of its flags (**hypothesis**: a settlement's ground), they play: within 800 of home a point within 400 of themselves, farther than 1,000 800 toward home, else within 400 of home, at a run; every other idle child within 600 without a dialogue follows the one that plays, at a run (`0x4EE820`);
  - else they gather about the hero: farther than 349 they follow it at a run; within 200, in one case of nine, one shows a child's line over itself and turns to it; else, in 14 cases of 15, they turn to it and stand, in one of 15 they run to a point 150 from it.
- A juggler with both hands empty takes one of the flute, the bell, the harp and the horn (5108 to 5111, `Instrument_*`) into its right hand at random and plays it once, the slot of its instrument (the flute 183, the horn 181, the harp 182, the bell 180, the models' `*_FUN_IDLE_*`); one of `0x10000000` plays on.

### Times

The dusk is 20:20 to 22:20 (`0x41BB50`), the night 22:20 to 03:20 (`0x41BAB0`, docs/re/daynight.md); the dawn counts as the day here.

## Groups

`+0x249` is a group's number (5,000 to 9,999 at random, kept in a map of the script manager's, `0x4A9EF0`), not a leader.
A group's creatures do not follow one another: when one of a group of an `Enter` function takes a step, every member walks by the same step (within 50 more) from its own place, its home moving with it.
When one of them fights, or decides to attack (in one case of two), the group's alarm (`0x46AE70`) sends the idle members against its target, or the hero when it is dead, or wakes them.
`+0x255` (`FillSector`'s `36:n`) serves nothing: its one reader is a branch of the alarm that no caller takes.

## Open

The test of the levels in detail; the AI's states 1, 6 and 10 to 14; a formation of groups (`+0x25F` `0xD160`); the original's frame rate, which scales every chance taken at a frame.
Of the townsfolk: the meaning of `0x434070`, which spares some types a torch; a following child's catch-up (`0x4EBF20`); the hero's list at `+0x39C` that keeps a child following it; how the engine's event 6 shows the lines.

## In `opensacred-game`

`ai` gives each NPC a mind (`ai::Mind`, #237): its home where it was made, its flags word, whether an `Enter` function made it and whether `CreateNPC` did, its group, its walk speed.
The hostile ones of `CreateNPC` wake as they are made and look every 2.5 s, the other hostile ones and the townsfolk every 5 s, jittered by ±1.5 s; the hero's look every 0.3 s wakes the creatures not hostile within 1,875 and the hostile ones within 800 (400 while it moves), and gives the idle action to the other hostile ones in range; an awake creature farther than 1,500 or 1,000 from the hero along the grid's axes goes dormant again.
The idle action's step walks a creature (`npcs::Walk`, at its walk speed and with its walk clip) to a point within 150, home in one case of four, or, made by an `Enter` function, turned about the hero, its home and its group's members moving with it.
The fight is not followed: an awake hostile creature, in one case of six, walks home from farther than 200, else in one case of three takes the idle action (docs/decisions/2026-10-20-idle.md); the fidgets are left out.
The animals take their behaviours (#238): a fill's peaceful creature its type's flags (`ai::of_type`), an `Enter` function's creature without flags those of its first update; awake, a wild one runs (`npcs::Walk::run`, at its run speed and with its run clip) from the nearest creature of its sector neither of class 4 nor 6, the hero among them, within 300, walks back within 50 of home from farther than 380, and strolls within 100 of home; a farm one steps away to the point beyond itself from such a creature within 200 and strolls within 150 of home; the strolls come as a rate, 30 in 101 a second (docs/decisions/2026-10-20-animals.md).
The guards keep their posts (#239): awake, a guard strolls within 200 of its post at the same rate, and otherwise walks back to it from farther than 90 along an axis; the torches wait for the lights in hand (#61), the attacks for the fight (docs/decisions/2026-10-20-guards.md).
A creature of a behaviour goes dormant again farther than 1,500 from the hero in a straight line.
The creatures part and step aside (#240): an idle one standing where another creature stands, the hero among them (`crowd::Crowd::crowded`), parts at a rate of 5 a second to a point within 100 of home or of itself by the original's measure; an animal awake so crowded strolls at once; at each of the hero's looks, a creature not hostile that walks within 91 of the hero walks instead to the first of 10 points within 200 of itself it reaches (`paths::path_onto`); the cells about a big creature and the scripts' hold are left out (docs/decisions/2026-10-20-crowds.md).
A creature's dialogue is a `CreateNPC` with the operand `0x09`, a dialogue's name (**hypothesis**: the index that `0x483AFA` stamps; `ai::Mind::dialogue`): it keeps a townsperson still, a guard from strolling and a crowded creature from parting.
The townsfolk decide (#247, of #255): awake within 600 times the camera's zoom of the hero, a townsperson rolls at each of the hero's looks (`ai::roll`): 1 walks to the nearest farm or wild animal of the 3 × 3 sectors but the last it went to, 2 and 3 walk home; the partners, the houses, the traders, the jugglers' instruments and the children's play wait for their features and stop meanwhile, and so do the evening's way and danger (docs/decisions/2026-10-20-townsfolk.md); between its rolls a townsperson crowded parts as an idle creature.
The partners (#250): the roll of 0 takes the nearest creature of class 3 of the 3 × 3 sectors but the last met for its partner, 2 too and walks home; one with a partner follows it while it stands, the hero within 1,500 (`ai::follow`): farther than 900 it forgets it, farther than 240 it walks up to it, within 80 it steps off to the first of 10 points 220 from it at random that it may stand on; between 80 and 200 they meet, and an adult, in one case of two, turns to its partner, which stops and turns back, and plays its talk once (`npcs::Talks`, slot 177); else it forgets it; a woman's slap is left out (docs/decisions/2026-10-20-townsfolk.md).
The traders (#251): a `CreateNPC` with the tags `0x23`, `0x24`, `0x25` or `0x64` is a trader (`ai::Mind::trader`; `0x64` stamps its dialogue too); the rolls of 5 and 8 (8 in one case of three for one of the flag 8) send a townsperson to the trader of the 3 × 3 sectors (`ai::trader_for`), to within 150 of it (`npcs::Walk::within`), where it plays its talk once; a trader does not part, and a meeting does not stop or turn it; going into a house after waits for #248.
The quests' NPCs that a script sends somewhere walk there first (docs/decisions/2026-10-19-quest-npcs.md).
