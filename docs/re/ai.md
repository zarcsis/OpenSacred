# AI: the creatures while no foe is near

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra 12.1.4 on 2026-10-20 (#232; the townsfolk #241).
Distances are in world units, 53.66563 a cell.
Status: **confirmed** on the code unless marked otherwise.

## Two states

A creature carries two states, each a short with a sub-state after it: its AI's (`+0xFC`, `+0xFE`) and its action's (`+0x150`, `+0x152`), which the creatures' update `0x5375C0` switches on at every world tick (below).
It runs the AI only for a creature whose simulating client (byte `+0x39`) is the local player or none; the creature maker sets it to the local player for every id above 16, so in the single player's game every creature thinks.

- The AI's states: 0 idle (`0x542B20`), 1 (`0x53A170`), 2 a behaviour or a walk ordered (`0x4EE030`), 4 a melee fight (`0x4E0300`), 5 (in the update), 6 (`0x51CFD0`), 8 a fight at range (`0x4DBFC0`), 9 dead (`0x53E7C0`), 10 to 14 (`0x529C60`, `0x4FD0F0`, `0x4EC560`, `0x4EA890`, `0x4EAC80`).
- The actions: 0 standing (`0x544890`), 1 moving (`0x4F4770`), 2 (`0x546690`), …, 8 (`0x4FF0B0`), 10 (`0x506770`), 14 (`0x514790`).

## The world's tick

The creatures' update (`0x5375C0`) runs on a tick of the world, not at every frame (**confirmed** by a probe, #616).
The render loop (`cEngine::renderThread::run`, `0x60E350`) adds each frame's time to the float `0xAD6EFC`; once the sum is above 0.1 s, it calls `0x60DF00` and `0x60F130` with it and sets it to 0.
Observed: the hero's update came every 100 or 117 ms, with its argument 0.1 or 0.117, so every 6 or 7 frames at 60 a second, 108 ms on average.
The hero, two goblins and two horses changed their regeneration's accumulators at the same moments, and the hero's at each of 18 resets of `0xAD6EFC` in 2 s and at no other time.
Which call of `0x60F130` (`sdk_SACRED_KERNEL_GET`, `0x809170`) reaches the updates, and what of the creatures runs at every frame (the walk looks smooth), is open (#617).
A probe of a hostile goblin coming at the hero and striking (2026-10-23, #617): every change of its action `+0x150` and of its AI state `+0xFC` fell on a tick, at its offset 0, its strikes' starts and ends among them (a strike of 12 ticks, 1.30 s), while its position changed at every frame as it walked; the hero's position too moves at every frame, its action's changes and the mouse's orders on the ticks (docs/re/movement.md, "The hero's orders and its stop"; #640).
So the creatures decide, and take their chances, on the tick; they move and their clips play at every frame.
`opensacred-game` runs the regeneration on such a tick (`frame::WorldTick`, #616), and the creatures' decisions (#639): the AI's systems (`ai::open`, `think`, `aside`, `behave`, `pursue`) run only when the world ticks, with the tick's time for their timers, and their rolls are a tick's.

## Dormant and awake

An idle creature (AI 0) is dormant in its sub-state 0 and looks about in 1; there is no other sleep.
Three things wake it:

- its own timer: an accumulator (`+0x208`) against its period (`+0x204`: 5 s, 2.5 s for a hostile creature of `CreateNPC`), for a creature whose flags word (`+0x1F4`) has the bits `0x3` or `0x408`; each time it fires it is jittered by −1.5 to +1.5 s, so the next comes 3.5 to 6.5 s later;
- `sdk_WAKE` (`0x59F580`), which `CreateNPC` calls for its hostile creatures, so that they look once as they are made (it skips a creature of `+0x200` bit `0x40000`), and which runs a creature's script on sight (`+0x1F4` bit `0x400000`, `CreateNPC`'s tag `0x60`) when the hero comes within 600 × 400 in the light and in a clear line;
- the hero's look about (`0x547070`, every 0.3 s over the 3 × 3 sectors about it, within about 1,875, docs/re/motions.md, "The fight stance"): it wakes the creatures not hostile to the hero at any distance of that range, and those hostile with bit 0 of their flags within 800, or 400 while the hero moves; to every other hostile one in range it gives the idle action (below), at every look in the single player's game; a creature not hostile within 91 of the hero that moves under an order of its own (its action and its AI's state both other than 0), while the hero's AI state is 0, steps aside: it walks to the first of 10 points at random within 200 of itself along each axis that it can reach by a way.

Awake (`0x542B20`), a creature farther from the hero than 1,500 along the grid's x or 1,000 along its y goes dormant again.
A peaceful one whose flags match `0x1006DCF8` takes its behaviour (AI 2, below).
A hostile one, in one case of six and where its home's cell lacks bit 2 of its flags, walks home when it is more than 200 from it along an axis, else stops; then it weighs its level against the hero's (by the balance's `BaseOffLevel`, `OffLevelProz` and `UnterExpAnteil`, 0.85; a monster far below the hero's level fights only when the hero comes within its reach, docs/re/fight.md): when that test fails and the hero is out of its reach (the mean of two sizes of the types, 70 to 480, 130 by default, `0x4DBC60`), it goes dormant again and in one case of three takes the idle action; otherwise it attacks, the hero its target (`+0x104`), from up to 800.

## The idle action

`0x54EA30` acts only on a creature that stands (AI 0, action 0, `+0x152` below 100), that no one owns and whose `REFKEY` is its type; when its group fights it alerts the group instead ("Groups" below).

- In one case of ten it fidgets: half of those times with one of its model's `*_FUN_IDLE_*` clips (the slots 180 to 192, each tried with a chance of one half; 39 models have one), the other half with its attack's clip.
- Otherwise, in one case of four (22.5 % in all), it takes a step: a creature that an `Init` function or the start script made walks to a point within 150 of itself along each axis, or home in one case of four; one that an `Enter` function made (`+0x14` bit `0x80000000`, `FillSector`'s among them) walks to its place turned about the hero by −30° to +30°, keeping its distance, and its home moves there.
  The step is an order to walk somewhere (the event 4: AI 2, then the moving action), which sets no gait: the moving action walks a creature whose gait (`+0x1F8`) is 0 (**hypothesis**: one left running by a chase runs).
- A hostile creature 800 to 1,875 from the hero so steps about once a second while it stands, and its own timer adds a roll every 5 s or so; farther than 1,500 it stands.
- Its home is where it was made (`+0x2B9` its sector, `+0x2BD` and `+0x2C1` its place, `+0x2C5` its layer), set by the creature maker (`0x480D40`).

## The peaceful creatures' behaviours

A peaceful creature awake takes the behaviour of its flags (AI 2, `0x4EF150`), run again at every tick of the world's update (#616) while it stands, a guard's whatever its action: `0x4EE030` passes on an animal (the flags `0x10` to `0x80`) only while its action `+0x150` is 0, a guard always (read 2026-10-23, #608); so its chances below are a tick's, 1 in 101 about one in 11 s at the tick's 108 ms.
The AI's state 2 (`0x4EE030`) passes it one flag, the first of its word in this order: 4 (it follows a partner, only while it stands), `0x100`, 8, `0x8000` (passed as 8), `0x10000000`, the animals' `0x10` to `0x80` (only while it stands), `0x4000`, `0x400`, `0x800`, `0x1000`; so the townsfolk's flags 8 and `0x400` run at every frame, moving or not, and a creature with both runs as 8.
Its distances to the hero and to the others are straight lines (`0x5370B0`, which caps them at 32,000), not along the grid's axes as the idle creatures' (`0x542B20`):

- wild animals (`0x40`, `0x80`): dormant again farther than 1,500 from the hero (the timer set by the distance); they flee at a run from the nearest creature of their sector of a class other than 4 and 6, the hero among them, within 300 (800 if it attacks), and the flyers take off; farther than 380 from home along an axis they walk back within 50 of it; and by chance, or when their cell is crowded, they walk to a point within 100 of home;
  the flight goes, along each axis of the grid, away from the one feared by a number at random between half and twice the distance from it along that axis, and across by one between minus and plus half the distance along the other (`0x443B20` gives a whole number at random between two, both included); a flyer on the ground within 200 of it takes off instead (`+0x1F4` bit `0x200000`, the run's gait 1), its target scaled by 400 (**hypothesis**: 400 away along the line from the one feared);
- farm animals (`0x10`, `0x20`): dormant again farther than 1,500; they step away from a creature other than an animal within 200 (800 if it attacks), to the point beyond themselves; and by chance, or crowded, they walk to a point within 150 of home;
- guards (`0x4000`): dormant again farther than 1,500; by night those of an odd id take a torch (type `0x1600`, in the slot 12; by day they put it away); they attack a foe within 500; else by chance one without a dialogue (below) strolls to a point within 200 of its post along each axis, from wherever it stands, and otherwise it walks back to its post from farther than 90 along an axis, so that a stroll that ends beyond 90 brings it straight back; deciding as it walks, it turns back at the first tick beyond 90 of a stroll toward a farther point, so that it strays hardly a tick's walk beyond 90 (**confirmed** in the original 2026-10-23, #608: the garrison of 52,39's guards within 1 or 2 cells of where they stood over a minute, 5 to 10 moves; and by a probe of the guard by its post at 3388,2524 over 2 minutes, #624: eight strolls toward points up to 200 off, each turned back to the post after 0.4 to 1 s as it passed 90);
- the flags `0x800` and `0x1000`: the creature stands;
- the townsfolk (`0x8`, `0x400`): below, "The townsfolk".

A creature's dialogue is `+0x245`, a number that `sdk_STAMP_IDX` (`0x5498F0`) writes, from `CreateNPC` and the scripts, and that `0x549920` gives as its talk (or, without one, a line of the generic townsfolk's in one case of three); a creature with one is held by its talk: a guard does not stroll, a townsperson keeps still, a crowded creature does not part.

A creature that stands idle (AI 0, the standing action `0x544890`) and shares its cell with another, in one tick of six, parts: it walks to a point within 100 of its home or of itself.
The cell's test (`0x54FCB0`) counts every other creature in it, the types of the categories 2, 3 and `0x20` (`0x426610`), the hero among them, and, for a flyer or a model wider than 60 or deeper than 110, in the eight cells about it too.
The choice between home and itself measures, of each point, its y from its x, and takes the distance of the two differences (`0x545B30`), not the distance of the creature from home: between 30 and 400 it parts about home, else about itself, and past ±32,000 for either difference it does not measure and parts about itself, which the whole world but a band along its diagonal is.
A creature of `+0x1F4` bit `0x8000000` (`CreateNPC`'s tag `0x74`), of `+0x200` bit `0x10` or of the bits `0x4007000` (the traders, below), one with a dialogue (`+0x245`) or one that a script holds (`0x4A71F0`) does not part.
A creature of a behaviour that stands crowded wakes (`0x544890`, at `0x54647A`); an animal awake strolls at once where another shares its cell, as by its chance (`0x4EF150`), and the townsfolk's machine reads the cell too (at `0x4F0D44`).

The flags come from `CreateNPC`'s tags (`0x2B` 8, `0x2E` `0x10`, `0x30` `0x20`, `0x2F` and `0x31` `0x40`, `0x32` `0x80`, `0x42` `0x400`, `0x43` `0x800`, `0x44` `0x1000`, `0x46` `0x4000`, `0x4C` `0x8000`, handled as 8; docs/02 §7.3), from the creature maker's lists of types for a peaceful creature made without flags, as `FillSector` makes them (a hostile one bit 0 alone), and, for a creature of an `Enter` function still without any at its first update, 1 if it is hostile to the hero, `0x20` for class 6, `0x10000000` for the jugglers (78, 79, 82, 104), else 8.
Read 2026-10-22 (#482, `0x542B20`): the first update gives them only to a creature of id above 16, made by an `Enter` function (`+0x14` bit `0x80000000`) or of `+0x2B8` bit `0x80` (not read), and passes over one with a dialogue (`+0x245`), one with any of the `+0x200` bits `0x40077C0`, one of the heroes' class (`0x4266F0`) and the type 1879.
Those bits are the hero's `0x40` (docs/re/motions.md), `0x100` and `0x200` of `CreateNPC`'s tag `0x1A`, `0x200` of its tags `0x70`, `0x71` and `0x72` and of `sdk_JOIN` too (`0x482510`), the traders' `0x1000`, `0x2000`, `0x4000` and `0x4000000` (below), and `0x80` and `0x400` (not read).
So the merchant of 50,40 and the novice with a dialogue keep no flags and stand on their floors all day (observed in the original under the driver of #448: the merchant's `+0x200` `0x8002000`, the novice's dialogue `0x3BF`).
A fill's critter is never without: it keeps the bit `0x1000000` of its constructor (docs/re/movement.md, "The objects in the way"), so that the rabbits' variants 592 and 593, on neither list below, take no behaviour and stand, parting as the crowded do (read in the original 2026-10-22, #468: their word `0x1000000` alone; one stood 32 s and took one step of a cell, another took one-cell steps within 2 cells with the hero beside it).
The creature maker (`0x480D40`) gives `0x40` to the types 512 and 567 to 570 (the wolves), 513, 589 and 590 (the crows), 514, 515 and 594 to 596 (the bats), 516 (the first rabbit; not its variants 592 and 593), 530 to 533 (the deer), 536 to 538 (the bears), 540 and 541 (the bulls), 543 and 545 (the wild boars), and `0x10` to 520 to 528 (the pigs and the cows), 544 (the sheep), 550 to 558 (the horses), 560 to 563, 565 and 566 (the chickens); `CreateNPC` writes the word of its tags over them (`0x4839F6`), so that its creatures without a tag keep none.
`Creature.pak`'s class 6 holds all these animals, wild and tame, and class 4 the horses ridden.
A creature of an `Init` function or of the start script without a tag keeps none and stands.
In the Seraphim's tree the `Enter` functions' `CreateNPC`s are mostly townsfolk (596 of `0x2B`, 570 of `0x42`), 166 guards, and animals (150 of `0x30`, 131 of `0x2E`, 26 of `0x32`, 5 of `0x2F`/`0x31`); the start's novices carry `0x42` or `0x2B`, its farmers `0x2B`, its pigs `0x2E`, its chicken and rabbit `0x30`.

## The townsfolk

The townsfolk are the creatures of the flags 8 and `0x400` (`CreateNPC`'s tags `0x2B` and `0x42`, `0x4C` as 8, and the `Enter` functions' creatures without flags that the first update does not pass over) and the jugglers of `0x10000000`; their branch of `0x4EF150` starts at `0x4F0A6A`.
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
- Each door's type has an outer and an inner point (`0x43B290`): offsets in world units added to the door's position, its `0x04` cell's centre (docs/02 §7.3).
  For 50 types the call's flag picks one of a pair, the inner point for a creature on a building's floor and the outer for one on the ground (`0x53A170` passes whether the creature's layer is 0).
  For 44, the doors of the dungeons, the prisons, the dwarves, the gates, the fences, the cage, two castles' gates and one of `Door_NeuHof01`'s types, the call gives of two points the one nearer the creature.
  The other 23 types fall to a default that the client logs, 6 of them placed (48 doors, most of them the palisades').
  Over the 117 types the offsets' components reach 175 (median 67), and the points lie 53 to 218 from the door's position (median 110).
- The client's points against the doors (docs/02 §5.7), compared as a range and not copied (docs/08 §3): at the 470 houses' doors whose types give a pair (one of them by the default), the inner point lies 29 to 98 units onto the floor's side of the door's cell's centre and the outer 40 to 105 onto the ground's, from the 10th to the 90th percentile (medians 54 and 71).
  Along the heading's direction the inner lies −39 to +28 from the cell's centre (median −3), and the outer −52 to +11 (median −34).
  The inner stands on the building's floor at 467 of them and the outer on the ground at 453.
  At the other 595 doors of the types of two points, these lie on opposite sides of the leaf at 520, 41 to 129 from the door's cell's centre across it (median 68).
- **The rule** for the game, measured from the doors' models, their placements and the floor's grid rather than taken from the client's table (docs/02 §5.7): a door's inner point is the centre of its cell, its trigger's, moved one cell (53.67 units) along the door's normal, across its type's heading on the grid, to the side of the building's floor, and its outer point the centre moved one cell the other way, onto the ground.
  Put in cells, the inner point is the centre of the cell beside the door's (of the four that share a side with it) that lies the most along the normal on the floor's side among those walkable on the building's floor, and the outer point the same on the other side among those walkable on the ground.
  A door without a trigger's cell takes the doorway's middle for its cell's centre, its position plus the leaf's middle along the heading.
  The floor's side is the one with a cell walkable on an inside layer and not on the ground: at all 479 houses' doors the side `(u_y, −u_x)` of the heading's direction `u` on the grid, away from the viewer.
  At a door with ground or floor on both sides (the dungeons', prisons' and gates' among them), the two points are the cells beside it either way along the normal, and a creature takes the one on its own side as its outer point, as the client's types of two points do.
  Checked on the files: at all 479 the cell beside the door's along the normal is the floor's, and so is the ground's at 472, the cell one off at the other 7 (`Door_Stadt01` of 17°) being a wall's, where the cell beside on the heading's side is the ground's.
  At the 470 of the 472 whose types give a pair, the inner point lies 23 units from the client's at the median (69 at the 90th percentile) and the outer 49 (89); a cell is about the client's medians across the leaf, 54 and 71.
- Going in (sub-states 8, 9, 7 and 2): it seeks the door, walks to its outer point, opens it, walks to its inner point onto the building's floor, and closes the door behind it; one that stops outside tries another door farther than 150, reachable (sub-state 5), else gives up.
  Leaving (sub-state 3) closes the door behind it too.

### Partners

- A partner (`+0x251`) is the nearest creature of the 3 × 3 sectors, at any distance, of class 3 (`Creature.pak`), not of the heroes' kind, other than itself and its last partner (`+0x24D`; `0x4EE9C0`).
- Following (the flag 4, while it stands): with the hero farther than 1,500 it goes dormant; with a hostile within 300 of the hero, or its partner fighting, it flees 700 from the fight or stands; its partner farther than 900 it forgets; farther than 240 it walks to it at the partner's gait; within 149 it steps off to the first of 10 points 220 from it at a random angle that it can reach.
- Between 80 and 200 from its partner it meets it (`0x4FC490`) and forgets it: in one case of two, the adult turns to it, and the partner, idle, alive and no trader, stops and turns back; it plays its `*_TALK_A` once, or, a woman's type (`_FEM`) at 100 to 140, in one case of 11, its attack's clip (**hypothesis**: a slap).

### Children and jugglers

- The children are the types 229 to 231 and 702 to 705 (`0x43F670`); they always roll 20 (`0x4F2AD7`):
  - before a hero of type 7 (the Vampiress) or 9 (the Daemon) they flee;
  - else in one case of 13, or when the hero's cell lacks bit 1 of its flags (a settlement's ground: **confirmed** on the files as to the cells it covers, docs/02 §5.2), they play: within 800 of home a point within 400 of themselves, farther than 1,000 800 toward home, else within 400 of home, at a run; every other idle child within 600 without a dialogue follows the one that plays, at a run (`0x4EE820`);
  - else they gather about the hero: farther than 349 they follow it at a run; within 200, in one case of nine, one shows a child's line over itself and turns to it; else, in 14 cases of 15, they turn to it and stand, in one of 15 they run to a point 150 from it.
- A juggler with both hands empty takes one of the flute, the bell, the harp and the horn (5108 to 5111, `Instrument_*`) into its right hand at random and plays it once, the slot of its instrument (the flute 183, the horn 181, the harp 182, the bell 180, the models' `*_FUN_IDLE_*`); one of `0x10000000` plays on.

### Times

The dusk is 20:20 to 22:20 (`0x41BB50`), the night 22:20 to 03:20 (`0x41BAB0`, docs/re/daynight.md); the dawn counts as the day here.

## Groups

`+0x249` is a group's number (5,000 to 9,999 at random, kept in a map of the script manager's, `0x4A9EF0`), not a leader.
A group's creatures do not follow one another: when one of a group of an `Enter` function takes a step, every member walks by the same step (within 50 more) from its own place, its home moving with it.
When one of them fights, or decides to attack (in one case of two), the group's alarm (`0x46AE70`) sends the idle members against its target, or the hero when it is dead, or wakes them.
`+0x255` (`FillSector`'s `36:n`) serves nothing: its one reader is a branch of the alarm that no caller takes.

## Foes among the creatures

Read 2026-10-23 (#630); **confirmed** by code unless marked otherwise.
The creatures of hostile sides (docs/re/fight.md, "Sides") fight one another by two ways.

- A guard's foe: the guards' behaviour (`0x4EF150`), after its dormancy beyond 1,500 and its torch, takes the nearest creature of the 3 × 3 sectors about it (`0x4EEE40`: its own sector and the 8 that `0x634840` gives, each sector's list of `0x5FEAF0`), at any distance, straight from it (the square root rounded up), alive and to which it is hostile (`0x548F60`), the hero among them; for a guard of an id above 16 only.
  Within 500 of it (`0x5370B0`) it sends itself the event `0x100` with that creature at the event's `+0x14`, and sets `+0x261` to 4.
- The event `0x100` (`0x52FA63`): a creature of `+0x200` bit `0x10` ignores it; one with an owner (`+0x14` bit 2 clear and `+0x1EC` set) that is no hero's class sends it on to its owner; one in the AI's state 4 or 8, deciding (sub-state 0), keeps its fight; any other takes the AI's state 8 with a ranged weapon in a hand (`0x54A2A0`) or the `+0x200` bits `0x600000`, else 4, the sub-state 0, its target (`+0x104`) the event's creature.
  The group's alarm (`0x46AE70`) sends the same event.
- An idle hostile creature's target (`0x542B20`, from `0x543113` to `0x54361A`): one of the flags' bit 0 whose action is neither 3 nor 5 (one of bit 0 and of a behaviour, while the local hero lives and it is hostile to the hero, else it loses bit 0 and takes its behaviour).
  Its target is the local hero (in a network game the nearest player).
  After the walk home (one case of six, "Dormant and awake"), in one tick of three (`rand() % 3 == 2`), and at every tick for one of the side 5, of the flags' bit `0x40000` or of the type 75, it looks among the creatures of its own sector within 800 of it: one of another side that fights, the AI's state 4 or 8 with a target (`+0x104`), or the action 3 or 5 or the AI's state 6 or 11 with the action's target (`+0x158`), not farther from it than the hero; one of its own side only for the side 5 against one not of the sides 3, 5 and 6, for the flags' bit `0x40000`, or for the type 75 against one not of the types 75, 55, 99 and 109; and any of `+0x1F4` bits `0x50000`.
  Each such that is alive and to which it is hostile becomes its target in two cases of three (`rand() % 3 != 0`), the last so taken counting.
  The roll of three is skipped, and the look made, when the first creature of the hero's list `+0x39C` of `+0x1F4` bits `0x10100` is found (**hypothesis**: a summoned creature; not followed).
  Then the types 189 and 334 take the nearest of the types 188 and 245 of the hero's list of hostiles within 500, and a creature of class 15 one of class 9 within 700 in one case of two, and the other way about (not followed).
- Its target taken, the test of the levels weighs it (its owner when it has one, docs/re/fight.md, "Who fights"), and it attacks its target within 800 along a straight line as it does the hero, at the AI's state 4 or 8: a hostile creature joins a fight beside it rather than come at the hero.
- A hit does not by itself turn its defender on its attacker: the resolution (`0x56B130` to `0x56D54A`) writes neither the AI's state nor the target, and the defender's handler raises its group's alarm (`0x5337CE`, "Groups").
- Seen in the original after `load start-seraphim`, the hero at 1425,3547 (#630): the start's boss 689, a guard (`CreateNPC`'s mode `0x4000`) peaceful on the side 7, at level 13 and 733 health, in the AI's state 4 against a bear 538, which struck it in its behaviour's state 2; the other bears there, of the mode `0x4000` and hostile on the side 14, struck the hero in the AI's states 2 and 4.
- Read 2026-10-23 (#669), of those bears: their flags word is their mode alone, `0x4000`, without bit 0, for `CreateNPC` writes its tags' word (`0x4839F6`) and adds no bit for a hostility, and wakes a creature of a word with bit 0 (`0x4839FC`: any word with it, not bit 0 alone).
  The hero's look tests the hostility by the sides (`0x548F30` with the hero, at `0x5472EC`) and gives a hostile creature without bit 0 the idle action alone (`0x54EA30`, which writes no AI state), and the update's timer wakes only the words of the bits `0x3` or `0x408` (`0x5375C0`), so neither wakes them.
  Woken, a creature without bit 0, or one in the action 3 or 5 whatever its bit, takes its behaviour (`0x542B20`, the AI's state 2), whatever its side: so the bears' state 2 between their strikes, and their guards' behaviour attacks the nearest creature hostile to them within 500, the hero among them (`0x4EEE40`).
  A struck creature in a group raises its group's alarm (`0x5337CE`, `0x46AE70`): each other member of the AI's state 0 takes the struck one's target where it fights (the event `0x100`), the local hero where it is dead, and else wakes (`sdk_WAKE`).
  `sdk_WAKE`'s other callers are a dialogue's (`0x461DE6`), the hiring and the companions' (`0x54CFD6`, `0x54D0BB`, `0x52E7F8`, `0x53043B`), a partner's (`0x535FAD`), the risen dead's (`0x53ECA0`) and the giant spider's young (`0x4E58E4`).
  Open: what wakes the bears, which the game's scripts give no group (**hypothesis**: a group the game does not read, or a script).

## Open

The test of the levels in detail; the AI's states 1, 6 and 10 to 14; a formation of groups (`+0x25F` `0xD160`); whether the actions' own switches (a strike's end, a walk's stop) wait for the tick in the original as its decisions do (#617; the game ends them at the frame, and decides at the next tick).
Of the townsfolk: the meaning of `0x434070`, which spares some types a torch; a following child's catch-up (`0x4EBF20`); the hero's list at `+0x39C` that keeps a child following it; how the engine's event 6 shows the lines.

## In `opensacred-game`

The decisions named below as `minds::…` live in `opensacred-rules` (`opensacred_rules::minds`, #362); `opensacred-game`'s `ai` keeps the minds as components, finds the creatures, the doors and the cells about a creature, and carries the decisions out with the walks, the strikes, the lines and the turns.
`ai` gives each NPC a mind (`ai::Mind`, #237; `minds::Mind`): its home where it was made, its flags word, whether an `Enter` function made it and whether `CreateNPC` did, its group, its walk speed.
The hostile ones of `CreateNPC` wake as they are made and look every 2.5 s, the other hostile ones and the townsfolk every 5 s, jittered by ±1.5 s; the hero's look every 0.3 s wakes the creatures not hostile within 1,875 and the hostile ones within 800 (400 while it moves), and gives the idle action to the other hostile ones in range; an awake creature farther than 1,500 or 1,000 from the hero along the grid's axes goes dormant again.
The idle action's step walks a creature (`npcs::Walk`, at its walk speed and with its walk clip) to a point within 150, home in one case of four, or, made by an `Enter` function, turned about the hero, its home and its group's members moving with it.
An awake hostile creature, in one case of six, walks home from farther than 200; else one that takes on the hero or has it within its reach comes at it (docs/re/fight.md, "In `opensacred-game`"), and any other in one case of three takes the idle action (docs/decisions/2026-10-20-idle.md); the fidgets are left out.
The animals take their behaviours (#238): a fill's peaceful creature its type's flags (`minds::of_type`) with its constructor's critter bit (`ways::constructed`, #468), so that the rabbits' variants take none, an `Enter` function's creature without flags those of its first update; awake, a wild one runs (`npcs::Walk::run`, at its run speed and with its run clip) from the nearest creature of its sector neither of class 4 nor 6, the hero among them, within 300, walks back within 50 of home from farther than 380, and strolls within 100 of home; a farm one steps away to the point beyond itself from such a creature within 200 and strolls within 150 of home; the strolls come in one tick of 101 (#608, #639, docs/decisions/2026-10-20-animals.md).
The guards keep their posts (#239): awake, a guard strolls within 200 of its post at the same rate, and otherwise walks back to it from farther than 90 along an axis, deciding as it walks too, so that it turns back from a stroll beyond 90 (#608); the torches wait for the lights in hand (#61) (docs/decisions/2026-10-20-guards.md).
The creatures of hostile sides fight one another (#630, `minds::hostile`, docs/decisions/2026-10-23-creatures-fight.md): a guard awake, walking or standing, attacks the nearest creature of the 3 × 3 sectors about it hostile to it within 500 (`minds::Mind::guards`), its behaviour waiting while it fights; an idle hostile creature, in one tick of three, takes for its target instead of the hero each creature of its sector that fights, of another side and hostile to it, within 800 and no farther than the hero, in two cases of three (`minds::Mind::think`, `minds::Foe`), and weighs its level against the foe's; both fight their foe in melee as the hero, steered by it alone, without places about it, and leave the fight once it is dead or gone (`ai::pursue`); one with a ranged weapon takes no creature for its foe (#670).
A hostile creature of a behaviour without the flags' bit 0, the start's bears by the boss 689, takes its behaviour once woken, not the pursuit of the hero, and a guard takes the hero too for the nearest creature hostile to it (#669); a struck creature in a group raises its group's alarm, whose idle members join its fight against a creature, come at the hero where it fights the hero, and else wake (`ai::alarm`, `minds::Mind::joins`, `minds::Mind::wakes`); what wakes the start's bears, which have no group, is open (#669).
A creature of a behaviour goes dormant again farther than 1,500 from the hero in a straight line.
The creatures part and step aside (#240): an idle one standing where another creature stands, the hero among them (`crowd::Crowd::crowded`), parts in one tick of six (#639) to a point within 100 of home or of itself by the original's measure; an animal awake so crowded strolls at once; at each of the hero's looks, a creature not hostile that walks within 91 of the hero walks instead to the first of 10 points within 200 of itself it reaches (`opensacred_rules::ways::path_onto`); the cells about a big creature and the scripts' hold are left out (docs/decisions/2026-10-20-crowds.md).
A creature's dialogue is a `CreateNPC` with the operand `0x09`, a dialogue's name (**hypothesis**: the index that `0x483AFA` stamps; `minds::Mind::dialogue`), or a `SetNPCState` after it whose `0x09` names a dialogue of the table as it stands then, which `0x0A` and a name not in it take away (#578, docs/re/netscript.md, "`SetNPCState`"): it keeps a townsperson still, a guard from strolling and a crowded creature from parting.
The townsfolk decide (#247, of #255): awake within 600 times the camera's zoom of the hero, a townsperson rolls at each of the hero's looks (`minds::roll`): 1 walks to the nearest farm or wild animal of the 3 × 3 sectors but the last it went to, 2 and 3 walk home, and the other rolls as below (docs/decisions/2026-10-20-townsfolk.md); between its rolls a townsperson crowded parts as an idle creature.
The partners (#250): the roll of 0 takes the nearest creature of class 3 of the 3 × 3 sectors but the last met for its partner, 2 too and walks home; one with a partner follows it while it stands, the hero within 1,500 (`minds::follow`): farther than 900 it forgets it, farther than 240 it walks up to it, within 80 it steps off to the first of 10 points 220 from it at random that it may stand on; between 80 and 200 they meet, and an adult, in one case of two, turns to its partner, which stops and turns back, and plays its talk once (`npcs::Talks`, slot 177); else it forgets it; a woman's slap is left out (docs/decisions/2026-10-20-townsfolk.md).
The traders (#251): a `CreateNPC` with the tags `0x23`, `0x24`, `0x25` or `0x64` is a trader (`minds::Mind::trader`; `0x64` stamps its dialogue too); the rolls of 5 and 8 (8 in one case of three for one of the flag 8) send a townsperson to the trader of the 3 × 3 sectors (`minds::trader_for`), to within 150 of it (`npcs::Walk::within`), where it plays its talk once; a trader does not part, and a meeting does not stop or turn it; it does not go into a house after (docs/decisions/2026-10-20-townsfolk.md).
The children (#252) roll 20 at each look, an adult's 20 stopping (`0x4F231D`, docs above): with the hero outside the buildings (its cell without bit 1), or in one case of 13, a child runs off to play (`minds::child`) and the other idle children within 600 of it run after it; with the hero inside one, a child farther than 349 runs after the hero until it leaves the building, a nearer one turns to it, and in one case of 15 runs to a point 150 from it; the hero's shooing them is left out, their flight from the Daemon and their lines are below, and the Vampiress is not a class of the game (docs/decisions/2026-10-20-townsfolk.md).
Danger (#254): a townsperson awake and standing within 800 of a hostile of the hero's (flag bit 0, in the 3 × 3 sectors about the hero within its look's range) runs, at every tick it stands, 700 away from the nearest such along the line, within 100 more along each axis, as the original's does without a door to go in by (`ai::danger`, `minds::runs_from`), rather than going in by a door; the fights' targets put none in danger (docs/decisions/2026-10-20-townsfolk.md).
The houses (#248): the doors are the objects of category 10 (`npcs::Door`); the rolls of 4, 6 and 7 send a townsperson into the house of the nearest door within 1,500 by the door's outer and inner points (#431, `opensacred_rules::ways::DoorPoints`, the rule above): farther than 350 from the door it walks to the outer point, and standing there, or within 350 from the start, it opens the door and walks through to the inner point on the building's floor, closing the door behind it once it stands; one that stops farther than 350 from the door gives up and decides again; a door without an inner point on a floor and an outer on the ground sends it, as before, to the farthest of 15 cells 2 to 9 cells about the door that a building's ground floor holds (`ai::indoors`), by the way through the building's entrance; indoors on a free cell, it walks out at its next decision to the first of 15 cells −4 to 5 cells about the door outside the buildings (`ai::outdoors`); many houses of the world have no floors to go in to (docs/decisions/2026-10-20-townsfolk.md).
The doors open and close (#268): a townsperson going in or out by a door opens it as it sets off (`npcs::Swing`), and closes it behind it once it stands again, the door's model playing its clip `*_ACTIVATE` or `*_DEACTIVATE` once and staying so, its trigger's cells with it; a door that the hero turned keeps the hero's mark, and no townsperson closes it again (`0x44BCD0`, #372); the doors' outer and inner points (#273, "Houses and doors") take the townsfolk in (#431) and the creatures in the way to their doors (docs/re/fight.md, "In `opensacred-game`"); a townsperson going out still opens its door as it sets off and walks by the way through the entrance.
The evening (#249): at dusk and by night (`daynight::Phase`), at its looks, a townsperson on a free cell walks among the building's cells indoors, and outdoors goes into the house of the nearest door within 1,500, 700 toward it at a time from farther than 1,000, then by the door's points as the rolls of 4, 6 and 7 do (#431), or without a door wanders within 400 of itself; one that goes dormant beyond its range on the ground is put at once into a house by the nearest door within 1,500 of it (`npcs::PutAt`); the torches wait for #61.
The jugglers (#253): a juggler's roll of 9 (`npcs::Plays`) gives it, both hands empty, an instrument of the four at random in its right hand, held by its socket `Bone_weapon_01`, the name that the converter gives the instruments' own `Weapon_bone01` (#353; **hypothesis**: the client's lookup finds none in them and attaches them otherwise, `0x44C670`), and plays its slot once (`motions::Slots::slot`), on and on for one of the flag `0x10000000`.
The Daemon (#269): with the hero the Daemon (`ai::Daemon`) within 500 of a townsperson, which has solved no quest yet, the townsperson takes the evening's way at any hour, into a house or wandering, and a child runs 700 away from it; the Vampiress is not a class of the game.
The lines over the creatures (#270, `lines::Says`): a child near the hero within a building, in one case of nine, turns to it and shows one of its 35 lines at random in green; a townsperson before the Daemon, in one case of three of its looks, shows one of `DaeAh1` to `6` in red; a line shows for 3 s over the creature, centred, from the game's texts by its name (`opensacred_data::text::sacred_hash`; **hypothesis** for the time, the place and the green).
The quests' NPCs that a script sends somewhere walk there first (docs/decisions/2026-10-19-quest-npcs.md).
