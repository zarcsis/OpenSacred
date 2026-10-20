# AI: the creatures while no foe is near

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra 12.1.4 on 2026-10-20 (#232).
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
- the hero's look about (`0x547070`, every 0.3 s over the 3 × 3 sectors about it, within about 1,875, docs/re/motions.md, "The fight stance"): it wakes the creatures not hostile to the hero at any distance of that range, and those hostile with bit 0 of their flags within 800, or 400 while the hero moves; to every other hostile one in range it gives the idle action (below), at every look in the single player's game; a creature not hostile within 91 of a hero walking under an order steps aside to a point within 200 of itself it can reach (10 tries).

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
Its distances to the hero and to the others are straight lines (`0x5370B0`, which caps them at 32,000), not along the grid's axes as the idle creatures' (`0x542B20`):

- wild animals (`0x40`, `0x80`): dormant again farther than 1,500 from the hero (the timer set by the distance); they flee at a run from the nearest creature of their sector of a class other than 4 and 6, the hero among them, within 300 (800 if it attacks), and the flyers take off; farther than 380 from home along an axis they walk back within 50 of it; and by chance, or when their cell is crowded, they walk to a point within 100 of home;
  the flight goes, along each axis of the grid, away from the one feared by a number at random between half and twice the distance from it along that axis, and across by one between minus and plus half the distance along the other (`0x443B20` gives a whole number at random between two, both included); a flyer on the ground within 200 of it takes off instead (`+0x1F4` bit `0x200000`, the run's gait 1), its target scaled by 400 (**hypothesis**: 400 away along the line from the one feared);
- farm animals (`0x10`, `0x20`): dormant again farther than 1,500; they step away from a creature other than an animal within 200 (800 if it attacks), to the point beyond themselves; and by chance, or crowded, they walk to a point within 150 of home;
- guards (`0x4000`): dormant again farther than 1,500; by night those of an odd id take a torch (type `0x1600`, in the slot 12; by day they put it away); they attack a foe within 500; else by chance one without a partner strolls to a point within 200 of its post along each axis, from wherever it stands, and otherwise it walks back to its post from farther than 90 along an axis, so that a stroll that ends beyond 90 brings it straight back;
- the flags `0x800` and `0x1000`: the creature stands;
- the townsfolk (`0x8`, `0x400`): while it stands, a roll of 26 sends it to follow or talk to a partner (`+0x251`), to an object near, home, to the sector's nearest waypoint of a road (`0xAAD2C8`), to juggle (the jugglers), to wait, or to stop; children play within 800 of home; at dusk and by night they carry torches and go into the houses through their doors (not read in detail; **hypothesis**: those far from the hero are put in a house).

Any creature that stands idle shares its cell with another, in one case of six a frame (`0x54FCB0`), walks to a point within 100 of its home or itself (**hypothesis** for which).

The flags come from `CreateNPC`'s tags (`0x2B` 8, `0x2E` `0x10`, `0x30` `0x20`, `0x2F` and `0x31` `0x40`, `0x32` `0x80`, `0x42` `0x400`, `0x43` `0x800`, `0x44` `0x1000`, `0x46` `0x4000`, `0x4C` `0x8000`, handled as 8; docs/02 §7.3), from the creature maker's lists of types for a peaceful creature made without flags, as `FillSector` makes them (a hostile one bit 0 alone), and, for a creature of an `Enter` function still without any at its first update, 1 if it is hostile to the hero, `0x20` for class 6, `0x10000000` for the jugglers (78, 79, 82, 104), else 8.
The creature maker (`0x480D40`) gives `0x40` to the types 512 and 567 to 570 (the wolves), 513, 589 and 590 (the crows), 514, 515 and 594 to 596 (the bats), 516 (the first rabbit; not its variants 592 and 593), 530 to 533 (the deer), 536 to 538 (the bears), 540 and 541 (the bulls), 543 and 545 (the wild boars), and `0x10` to 520 to 528 (the pigs and the cows), 544 (the sheep), 550 to 558 (the horses), 560 to 563, 565 and 566 (the chickens); `CreateNPC` writes the word of its tags over them (`0x4839F6`), so that its creatures without a tag keep none.
`Creature.pak`'s class 6 holds all these animals, wild and tame, and class 4 the horses ridden.
A creature of an `Init` function or of the start script without a tag keeps none and stands.
In the Seraphim's tree the `Enter` functions' `CreateNPC`s are mostly townsfolk (596 of `0x2B`, 570 of `0x42`), 166 guards, and animals (150 of `0x30`, 131 of `0x2E`, 26 of `0x32`, 5 of `0x2F`/`0x31`); the start's novices carry `0x42` or `0x2B`, its farmers `0x2B`, its pigs `0x2E`, its chicken and rabbit `0x30`.

## Groups

`+0x249` is a group's number (5,000 to 9,999 at random, kept in a map of the script manager's, `0x4A9EF0`), not a leader.
A group's creatures do not follow one another: when one of a group of an `Enter` function takes a step, every member walks by the same step (within 50 more) from its own place, its home moving with it.
When one of them fights, or decides to attack (in one case of two), the group's alarm (`0x46AE70`) sends the idle members against its target, or the hero when it is dead, or wakes them.
`+0x255` (`FillSector`'s `36:n`) serves nothing: its one reader is a branch of the alarm that no caller takes.

## Open

The test of the levels in detail; the AI's states 1, 6 and 10 to 14; the townsfolk's times, waypoints and houses; a formation of groups (`+0x25F` `0xD160`); the original's frame rate, which scales every chance taken at a frame.

## In `opensacred-game`

`ai` gives each NPC a mind (`ai::Mind`, #237): its home where it was made, its flags word, whether an `Enter` function made it and whether `CreateNPC` did, its group, its walk speed.
The hostile ones of `CreateNPC` wake as they are made and look every 2.5 s, the other hostile ones and the townsfolk every 5 s, jittered by ±1.5 s; the hero's look every 0.3 s wakes the creatures not hostile within 1,875 and the hostile ones within 800 (400 while it moves), and gives the idle action to the other hostile ones in range; an awake creature farther than 1,500 or 1,000 from the hero along the grid's axes goes dormant again.
The idle action's step walks a creature (`npcs::Walk`, at its walk speed and with its walk clip) to a point within 150, home in one case of four, or, made by an `Enter` function, turned about the hero, its home and its group's members moving with it.
The fight is not followed: an awake hostile creature, in one case of six, walks home from farther than 200, else in one case of three takes the idle action (docs/decisions/2026-10-20-idle.md); the fidgets are left out.
The animals take their behaviours (#238): a fill's peaceful creature its type's flags (`ai::of_type`), an `Enter` function's creature without flags those of its first update; awake, a wild one runs (`npcs::Walk::run`, at its run speed and with its run clip) from the nearest creature of its sector neither of class 4 nor 6, the hero among them, within 300, walks back within 50 of home from farther than 380, and strolls within 100 of home; a farm one steps away to the point beyond itself from such a creature within 200 and strolls within 150 of home; the strolls come as a rate, 30 in 101 a second (docs/decisions/2026-10-20-animals.md).
The guards keep their posts (#239): awake, a guard strolls within 200 of its post at the same rate, and otherwise walks back to it from farther than 90 along an axis; the torches wait for the lights in hand (#61), the attacks for the fight (docs/decisions/2026-10-20-guards.md).
A creature of a behaviour goes dormant again farther than 1,500 from the hero in a straight line.
The crowded cells and the hero's way wait for #240, the townsfolk for #241.
The quests' NPCs that a script sends somewhere walk there first (docs/decisions/2026-10-19-quest-npcs.md).
