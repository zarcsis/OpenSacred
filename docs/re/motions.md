# Motions: the clip a creature plays

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra 12.1.4 on 2026-10-19 (#153).
A model's motion slots are in [docs/02 §4.1](../02-data-formats.md#41-the-mdlmdt-container-and-caches) (`ModelDesc +0x70`); the converter keeps them with each clip, as `extras.slots` of its glTF animation.
The weapon kinds are in [docs/02 §6.2](../02-data-formats.md#62-creaturepak-cif-and-weaponpak-wpn), the hands in [docs/re/equipment.md](equipment.md).
Status: **confirmed** on the code unless marked otherwise.
Implemented in `opensacred-game` (`motions`, `npcs`, `crowd`, `hero`, `forms`), `opensacred-rules` (`horses`) and `opensacred-convert` (`gltf`, `models`).

## The slots of the actions

`0x5467A0` gives the motion slot that a creature plays for an action, of six: 0 standing, 1 walking, 2 running, 3 the fight stance, 4 an attack, 5 defending; for another number it gives 0.
`0x593320` then plays that slot of the creature's model, blending from the clip before; where the model has nothing in the slot, it plays the stand-in that the models' clip getter gives for an attack's slot, else nothing new ("Where the model lacks the slot" below).

A table at `0x956E40` gives each action a row of 20 slots, one for each class of what the creature holds (below):

- standing, 1 to 13 for the classes 0 to 12;
- walking, 27 to 39;
- running, 40 to 52;
- the fight stance, 14 to 26;
- defending, 53 to 65;
- an attack, the first of five, 79 + 5 × the class.

Class 13 has slots of its own (235 standing, 237 walking, 238 running, 236 the stance, 241 an attack, 239 defending), class 14 takes class 12's, and the rest of each row is 0.

For an NPC (`+0x200` bit `0x40` clear) the attack's slot is the table's for its class whether its model has it or not (`0x5467A0`'s action 4, read 2026-11-13 for #2708, **confirmed** by code): no fallback as for standing and moving; the models' clip getter then gives the clip of another slot in its place ("Where the model lacks the slot" below, read 2026-11-21 for #3303).
A grave's skeleton warrior (`grab_21` and the like: the type 32 holding the dagger 1712, class 5 `DOLCH`) has no attack of its dagger's class: its model `SKEL_WARRIOR` fills the slots 79 (`SKLW_ATTACK_BH_A`), 84 and 85 (`SKLW_ATTACK_1H_A`, `_B`), 99 and 114 of the attacks, so that the original's getter gives it the slot 84's clip for the slot 104; the game follows it since #3314 (`motions::Slots::attacks`, "Where the model lacks the slot" below).

The classes agree with the names of the clips in the slots, over the 231 converted models that have a slot 1: 0 `BH` (bare hands), 1 `1H`, 2 `2H`, 3 `2H_AXT`, 4 `STAB`, 5 `DOLCH`, 6 `WURF`, 7 `BOGEN`, 8 `2WAFFEN`, 9 `1H_KLINGE`, 10 `2H_KLINGE`, 12 `ARMBRUST`; no model has a clip named for class 11.
Most models fill few slots: a thief's idle `THFM_IDLE_BH` fills 1, 14 and 21, its walk 27, its run 40, and its `THFM_HORSE_IDLE`, a rider's idle in the saddle, 169.

## Where the model lacks the slot

`0x5467A0` tests the model's slot (`0x4117B0`) and, where it is empty:

- standing, takes slot 2 where the model has it, else 1;
- walking, takes the first of 27, 28 and 29 that the model has, as a walk (it sets the creature's gait, `+0x1F8`, to 1), then of the runs 40, 41 and 42, as a run (gait 2), else the standing slot as above;
- running, takes the runs first, as a run, then the walks, as a walk, else the standing slot;
- in the fight stance or defending, takes the standing slot of its class from the table.

A creature that walks on a run's clip so moves at its run speed, which `0x4F75F0` takes by the gait (docs/re/movement.md, "The speed").

The actions that play a slot themselves through `0x593320` (an attack's of the table, a shot's, the Seraphim's bolts' 145 and 164) take none of those stand-ins; the models' clip getter gives one of its own for an attack's slot alone (read 2026-11-21 for #3303, **confirmed** by code and **seen** in the running original the same day, the driver of #448 probing `0x593392`, `0x59374B`, `0x4DCB80`, `0x4DD855` and the shot's makers through 300 shots of three ice druids).
The reading of the same day for #3031, by which nothing played for any slot the model lacks and the druid's shot took its cast's fraction, had missed the getter's stand-in.

- **The getter** (`cGrannyModelManager::getMotion`, `0x4118B0`): a model's slot table (`ModelDesc +0x70`, docs/02 §4.1) holds 0 for a slot it lacks.
  For such a slot from 79 to 153 the getter gives the clip of the slot 89 where the model has it (`0x4117B0`), else of 84, else of 79: the first attacks of the classes 2, 1 and 0, `*_ATTACK_2H_A`, `*_ATTACK_1H_A` and `*_ATTACK_BH_A`.
  It gives no clip where the model has none of the three, and for a slot outside 79 to 153.
- **The player** (`0x593320`, its arguments the slot, a blend, the loops and a rate): it asks the getter for the slot's clip, the slot 174 trying 173 after it.
  With a clip it starts a control of it at the call, so that the clip's start on Granny's clock is the call's moment, with its loops and at the speed `GameSpeed` times the rate and the creature's time factor `+0x2C9` (`0x413610`), and blends to it over the blend divided by the rate and that factor, at most 0.2 s.
  It gives the clip in play, which blends out, the same rate (`0x413610` at `0x5935D6`, read 2026-11-23 for #3473; "The rate of the clip in play" below).
  Until the blend's end (`+0x22D` on Granny's clock) the clip in play (the control at `+0x48`) and the slot in play (`+0x64`) are the old ones, the new control waits at `+0x235` and the slot asked at `+0x241`; the creature's animation step (`0x5941A0`, which its render and its update call) then makes the new control the clip in play, `+0x64` the slot asked and `+0x241` 0.
  A play that comes while a blend runs first ends that blend at once, and one that finds no slot in play (`+0x64` 0) takes its clip into play with no blend.
  So with a stand-in the slot in play is the slot that was asked, which the model lacks, and the stand-in's own slot is kept nowhere.
  With no clip it sets the creature's `+0xB0` bit of the value 1 and returns: nothing starts, the clip in play goes on, and `+0x64` and `+0x241` stay.
- **The bit** (`+0xB0`, the value 1): the three starters of a clip alone write it, setting it where the getter gave no clip and clearing it where one started: `0x593320`, `0x593770` (**hypothesis**: a clip played beside the one in play) and the animation step's start of a queued clip (`0x594BE5`–`0x594C2D`).
  Nothing reads it: a scan of `.text` for the loads and tests of `+0xB0` finds tests of the values 2, 4, 8, `0x10` and `0x20` and of the high bits, none of the value 1 (**confirmed** by the scan).
  So it ends no action and sends no creature to its idle; an action goes on as its own states have it.
- **A queued clip**: `0x5491F0` keeps a clip to come at `+0x214` (its slot, its loops at `+0x218`, its rate at `+0x21D`, its blend over the attack speed at `+0x221`), which the animation step starts once what is left of the clip in play is at most that blend, as `0x593320` starts one, at `GameSpeed` times `+0x21D` and `+0x2C9` (`0x594BB1`, `0x413610` at `0x594BE0`: the clip it starts alone, not one already in play); the attack action, the mover and the arts' executors call it.
- **The fractions**: an action that times itself by the clip in play reads the motions' record of the slot in play (`0x4135D0` with the model, `+0x64` and an index `k`: the record whose number the model's slot table holds, its float `+0x80 + 4k`, so `+0xA0` for 8 and `+0xA4` for 9).
  For a slot the model lacks that is the record 0, `INVALID_MOTION`, whose `+0xA0` is 0.401854, `+0xA4` 0.508987 and `+0x9C` 0.6 (`tables/motions.ron`, docs/02 §4.1).
  So a stand-in clip is timed by those two fractions and not by its own record's, and its speed takes no factor of its own, 0.6 lying below the 0.8 from which `0x413610` takes one (docs/re/movement.md, "The motions' own factors"; **seen** 2026-11-23 for #3563: the Mage's shot of the slot 119, which his model lacks, read 0.6 and played at `GameSpeed` times its rate alone).

The ice druid (141, the model `ICEELVE_DRUID`, which fills 79, 84 and 89 and neither 145 nor 164) shows both cases (**seen**; the shot's own steps are docs/re/fight.md, "The archers"):

- With the effect 16 its shot asks for the slot 145 at the rate 2.1, and the getter gives `ICEF_ATTACK_2H_A`, the slot 89's clip of 3.26 s, which plays in 1.2419 s (`GameSpeed` 1.25 times 2.1), blended in over 0.119 s; the slot in play is 145 from the blend's end.
  Its bolt leaves at the first tick at which the clip has played 0.401854 of its length, which is 0.499 s after the shot began, so at the fifth tick: through 194 bolts the least fraction at a bolt's tick was 0.4025 and the greatest at a tick before one 0.3892, where the clip's own record has 0.343319.
- With the effect 17 its shot asks for the slot 164 at the rate 1.0: the getter gives nothing, the bit is set, and the clip in play stays, the druid's stance (`ICEF_FIDLE_BH`, the slot 14, its hands emptied by the effect) or its walk.
  The two bolts leave by that clip and its slot's record, whose fractions are 0: the first at the shot's first tick after its start, the second once at most 0.4 of the clip's loop in play is left (through 106 pairs the second's least fraction was 0.6056, the greatest at a tick before one 0.5982).
- Its first shot after its art's cast is as the later ones: the cast's clip (154) has ended and its stance (18 with the staff of the effect 16) or its walk is in play as the shot starts, and the bolt left 0.53 s after the shot asked for 145 (the 0.52 s **seen** 2026-11-19).
- Its walks and runs are the mover's plays (`0x4F54E6`–`0x4F5537`): the slot of `0x5467A0` for its gait (`+0x1F8`), looped 10,000 times and blended in over 0.125, at the rate of the stats' `0x585070` times its time factor `+0x2C9` over 100, 1.3 for the druid (read 2026-11-22 for #3315, **confirmed** by code and **seen**: its slots 27, 31, 40 and 44 at 1.3 from `0x4F553C`, its stances 14 and 18 at 1.0 from the standing action and the shot's end, its time factor 1).
- Its stance after a shot of the slot 164 loops in 0.656 s or in 0.5046 s: `ICEF_FIDLE_BH` (0.82 s) at the rate 1.0, or at its pace's 1.3, `0.82/(1.25·1.3)`, which its own search of a way sets after the stance's start ("The rate of the clip in play" below; read and **seen** 2026-11-23 for #3473).
  The shot's end starts the stance at the rate 1.0 and blends to it over 0.2 s; the druid, which holds no ranged weapon, then takes its target up again in its idle handler, whose search of a way to the target (`0x4F7710` at `0x544217`) sets the clip in play to `GameSpeed` times its pace (`0x4F75F0`).
  Where that search comes before the blend's end, the clip in play is still the stance before, which blends out at that rate, and the new stance plays on at 1.0; where it comes at or after the blend's end, the new stance is in play and plays at 1.3 until the next shot's end starts another, the shot of the slot 164 starting none.
  **Seen** 2026-11-23 (the driver of #448; three ice druids of level 10 made hostile on open grass 5 cells from the Seraphim at 3283,2533 under the god mode, each with the driver's effect 17, shooting for about two minutes; probes at `0x593320`, `0x4F75F0`, `0x4F7710`, `0x413610`, the blend's end `0x5944C1`, the queued start `0x594BB1` and the shot's read `0x4DD855`, each with its caller): of 651 stances read in a shot, 284 looped in 0.5046 s and 367 in 0.656 s.
  Each was started by its shot's end at the rate 1.0, and its blend ended 151 to 220 ms later (184 ms in most); the idle handler's search (returning to `0x54421C`) set the rate 1.3 through `0x4F75F0` 200 to 367 ms after the start for every fast stance, and 83 to 150 ms after it for every slow one, whose search set the stance blending out; no druid's clip started from the queue.
  So the rule held for all 651, and #3303's 106 stances (48 fast) fit it by the times of their next decisions alone, which came 0.40 to 0.60 s after the shot's end for the fast ones and 0.30 to 0.52 s for the slow (**hypothesis** for that run, whose searches were not traced); the four runs between read the clips' starts alone, where both show the rate 1.0.
  The search comes in the tick of the first look of the hero after the end's tick, one to three ticks after the end, the look of the end's own tick running before the druid's update and finding its shot still playing (docs/re/fight.md, "At range: the AI's state 8"; **seen** 2026-11-23 for #3572 through 356 shots, docs/re/fight.md, "The archers"): a search one tick after the end (100 to 134 ms) came before the blend's end and left the stance at 1.0 (150 of 152), one two ticks after it (200 to 234 ms) found the blend over and paced the stance but for six at exactly 200 ms (77 of 83), and one three ticks after it (300 to 417 ms) paced it always, a walk aside.
  So the stance plays at 1.0 exactly where the hero's look comes at the tick right after the shot's end.
- Its second bolt leaves by the stance's place in its loop at that rate (the shot's steps, docs/re/fight.md, "The archers"), and a fast stance comes with a late search, so later in the stance's loop: **seen** in the same run, at the shot's first read a fast stance had played 0.17 of its loop (the median of 284 shots), past the end of the loop before, and the second bolt waited for 0.6 of the next one, 0.32 to 0.55 s after the shot's start (0.43 s the median), the shots 0.88 s apart (the median); a slow one had played 0.66 (of 367) and the second bolt left at the next tick, 0.20 to 0.25 s after the start, the shots 0.80 s apart.
  In ticks (**seen** 2026-11-23 for #3572, docs/re/fight.md, "The archers"): the shot starts two ticks after the look that wakes the druid and its first bolt leaves at the tick after the start, so a stance left at 1.0 by a search one tick after the end has played four ticks of its 0.656 s loop at that first bolt, some 0.66 of it, and the second leaves at the next tick; a stance paced by a search two ticks after the end stood at 0.93 to 0.96 of its loop at the first bolt and one paced three ticks after it at 0.08 to 0.18, and the second bolt left two to four ticks after the first, three in most.
  The shots came 6 or 8 ticks apart after a stance at 1.0 (0.833 s the median) and 8, 9 or 11 after one at 1.3 (0.95 s), seldom 7 or 10.

Who else meets a lacking slot (the converted models' slots against the creature types' models, **confirmed** on the data 2026-11-21):

- Every play goes through the getter (the attack action `0x4E4CF0`, the arts' executors, the spells' cast, the shot), so the stand-in serves the attacks and the slots 144 to 153 of the kicks and the specials, and a cast's slot from 154 up that a model lacks plays nothing.
  Of the casters of `0x4D4E50` (docs/re/ai.md, "The creatures' spells"; read 2026-11-22 for #3315, **confirmed** on the data, the slots of the types' models by `tools/analysis/formats/grn_dump.py`'s reader against `0x522030`'s slot of each of their spells, the aid's heal and shields among them), these cast with a slot that their model lacks: the goblin mages 127 and 143 and the god 144 their plant cage (40, the slot 158); the wood elf 02 (105, `WALDELFE_DARK`, which has 154 and 155 alone) every spell, 34, 38, 40, the heal 7 and the shields 24 and 2 (157, 162, 158, 161, 162, 163); the dryad druid 216 and the priestess 311, whose models hold no cast's clip, every spell; the terror sprinter mage 212 its plant cage; Sharifa 697 her 39 (159); the god's child 145 its lightning 13 (158); and the prince's guards 187, 205 to 208, 253 to 255 and 330 to 332 their plant cage.
  Every other caster's model holds its spells' slots: the mummies, the magic skeletons, the orc shaman, the black magicians 258 to 271, the priest 312, Anducar, his demon, his guard, the demon prince, the dark elf 693, Sharifo, the ice elf, its champion and the ice druid; the dragons' 81 and the big demons' 144 and 145 lie in the getter's range.
  Such a cast starts no clip, and its spell acts at the first test of the cast's action (`0x51D150` at `0x51D515`), which `0x521D60` answers with 100, for a clip of more than 1,000 s, as the clip in play is a stance or a move looped 10,000 times (**seen** 2026-11-22 under the driver: a priestess 311 of level 10 hostile to the Vampiress asked the slot 157 at the blend 0.2 and the rate 1.2, from `0x521C14`, three times, and each time `0x521D60` answered 100 about 0.1 s later, her run 40 or her stand 1 in play).
  `opensacred-game` follows it: a cast whose slot its model lacks plays nothing and acts at once (`npcs::script_clips`, `creature_casts`).
- The slots 145 and 164 are asked for a shot of the effects 16 and 17 alone (the Seraphim's 1018 and 1019), which of the creatures the ice druid alone orders itself (docs/re/ai.md, "The creatures' spells"); of the creature types' models only the Seraphim's fill both slots (`SERAPHIM`, the types 1 and 132, and `SERAPHIM_HOLO`, 135), so the two cases above are the druid's alone in play.
- A plain shot asks for its holder's attack slot (`0x5467A0`'s action 4: 114 with a bow, 139 with a crossbow or a musket), and the makers of the creatures' weapons give no ranged weapon to a model without the slot 114, nor a crossbow or a musket to one without 139 (docs/re/spawning.md, "`MakeGroup`"), so a plain archer's model has its shot's clip.
  A weapon of the kind 13 asks for the slot 241, which the Dwarf's model alone fills and for which the getter has no stand-in; the fills' weapon pools hold no weapon of the kinds 13 and 14 (`tables/weapon_groups.ron`), and neither do the scripts' records (read 2026-11-22 for #3315, **confirmed** on the data: no `CreateNPC`'s or `EquipNPC`'s items of the 20 trees name the types of those kinds, 1849, 1853 to 1857, 7724 and 7816 to 7821).
  A hireling takes a weapon only where its model has the slot of its kind's first attack, 241 for the kind 13 (`0x6D6B90`, docs/re/ai.md, "A hireling's hands"), so that no creature asks for 241 but of a model that fills it.
- A melee strike of a class the model lacks takes the stand-in: its clip is the slot 89's, 84's or 79's, timed by the record 0, whose second fraction above 0 marks a second hit (`0x4EA1F0`) that a creature of the heroes' class alone takes, an NPC's attack leaving its step at its first hit (docs/re/fight.md, "The second hit is the heroes' class's alone"); so a grave's skeleton warrior with its dagger swings `SKLW_ATTACK_1H_A` and hits once, at the first tick past 0.401854 of it.
  **Seen** 2026-11-23 for #3314 (#3601; the driver of #448, the Dwarf at his start under the god mode, `script grab_21` making the skeleton warrior 32 with its dagger 1712 on his cell; `0x593320` traced for it, probes after the getter at `0x593392` with the slot asked and the getter's record, and in the hit's step at `0x4EA2A5` with the action's `+0x39`, the clip's length and what is left of it and the two fractions read; the strikes' resolution traced):
  - each strike asked the slot 104 (79 + 5 × the dagger's kind 5) at the rate 1.4, its attack speed 140 over 100, blended over 0.25 for the first and 0.2 for those chained after, and the getter's record named the slot 84 as the clip it gave (its fifth word), `SKLW_ATTACK_1H_A` of 1.54 s, which played in 0.88 s (`GameSpeed` 1.25 times 1.4);
  - the hit's step read the fractions 0.401854 and 0.508987, the record 0's, and not the clip's own 0.411168, and the length 0.88;
  - every clip made one strike, not two: the step ran at each tick of the clip until the tick of its first hit and not after it, the next strike's clip being asked 0.18 to 0.23 s later; in the chained clips the first hit came at 0.511 of the clip, past both fractions, in the first one at 0.473, before the second, and no tick after it brought the second hit (the cause read 2026-11-24, **confirmed** by code and **seen** on an Amazon's own clip of two fractions: an NPC's attack action leaves its step at its first hit, docs/re/fight.md, "The second hit is the heroes' class's alone").
  - So the skeleton warrior swings the stand-in at its attack speed and hits once a swing, at the first tick past 0.401854 of the clip; the chained swings came 0.63 to 0.65 s apart, longer where its stance played between two.
- No model with attack clips lacks all three of 89, 84 and 79 but those of the types 199 and 340 (`STACHELALBAUFBLUTSCHRECKE` and `STACHELALBAUFBLUTSCHRECKE01`, the slots 109 and 114 alone).
- The getter gives the slot 79's clip where a model fills 79 and neither 84 nor 89 (**confirmed** on the data 2026-11-24, the converted models' `extras.slots`): of the models with an attack's slot past 83, `VLADY_N` (79, 80, 144 to 153), `MEDUSA` (79, 99, 100), and the monsters' models whose other attacks are 80 and the kicks' and specials' 144 to 146 (`ANDUCAR_DEMON`, `BIG_DEMON`, `BLUTSCHRECKE` and its saddled one, `EITERBLUME` and `EITERBLUME01`, the three `GEHIRNFUESSLER`, the three `KILLERKRABBE`, `RINGELWUERGER`); none was staged with a weapon of a class its model lacks, and which creatures of them hold one is not read.

`opensacred-game` follows it for the Seraphim's bolts of a creature since #3303 (`npcs::BoltClip`, `opensacred_rules::minds::stand_ins`): a shot of the slot 145 that the model lacks plays the first of 89, 84 and 79 that it has at the shot's rate and looses at the record 0's fraction, and one of the slot 164 plays nothing, its first bolt leaving at the first tick after its start as its stance's fraction of 0 lets it, its second since #3562 at the first tick after it at which the stance has played 0.6 of its loop (`opensacred_rules::arts::second_bolt_due`, `npcs::strike`), and the shot ending since #3572 at the first tick after its second bolt (`opensacred_rules::arts::Bolt::ends_after`, docs/re/fight.md, "The archers").
Since #3314 an NPC's melee strike of a class its model lacks takes the first of 89, 84 and 79 that the model has (`motions::Slots::attacks`), timed by the record 0 (`npcs`' `LACKING`), and hits once a swing at its first fraction as every NPC's strike does since #3686; one whose model has none of the three plays nothing and strikes not, as before.
The hero's attack keeps its class 0's clips where its model has none of its class's (`motions::Slots::hero_attacks`), the hero's case of `0x5467A0`'s action 4 not read.

## The rate of the clip in play

Read 2026-11-23 for #3473 (**confirmed** by code, and **seen** where marked).
A clip's speed on Granny's clock is Granny's parameter 2 of its control, which `0x413610` sets (the model, the slot, the control, a rate): `GameSpeed` times the rate, times the own factor of the slot's motion where that lies in [0.8, 5) and is not 1 (`MotionDesc +0x9C`, docs/re/movement.md, "The motions' own factors": the bows' and the crossbows' of seven classes, **seen** on the Mage's bow for #3563).
Its callers:

- the clip player `0x593320`, for the clip it starts (`+0x235`) and for the clip in play that it blends from (`+0x48`, at `0x5935D6`), both at the play's rate times `+0x2C9`; `0x593770` for its own clip; the queued start (`0x594BE0`) for the clip it starts, at `+0x21D` times `+0x2C9`; the objects' player `0x44A6C0`;
- the time factor's setter `0x537160` (the effect 15, docs/re/arts.md, "The effects");
- the shot's action at `0x4DF306`, for a shooter without the effect 17's bit and without a gun: the hero's shot going on, for the slot in play (**seen** 2026-11-23 for #3563, docs/re/fight.md, "The archers");
- the gait's speed `0x4F75F0`, which sets the clip in play (`+0x48`, its slot `+0x64`), whatever that clip is, to `GameSpeed` times the pace's factor, the pace times `+0x2C9` over 100 (docs/re/movement.md, "The speed"), and leaves a clip that a blend is bringing in (`+0x235`) as it is.

`0x4F75F0` runs at every search of a way (`0x4F7710`, docs/re/movement.md, "Searching again"), in the straight mode's line (`0x4FB7E0`, which the mover calls at `0x4F559C` and `0x4F6922`), as the hero takes the walk (`0x4F50F4`) and at two of the network's events of a gait (`0x531062`, `0x531187`).
Where a move follows the search, the mover's play gives its walk or run the same rate and the clip it blends from too, so nothing shows; a creature that walked and then stands takes its stand from the walk's end at the rate 1.0 (`0x4F7192`).
Where no clip starts after the search, the clip in play keeps the pace's rate until the next start of a clip; where a blend runs at the search, the clip blending out takes it and the one coming in keeps its own.
Granny keeps a clip's own time through the change: its start on Granny's clock moves later, so that its end is where its loops end at the new speed from the place the clip had reached (**seen**: the starts of the druids' stances read in their shots lay within two frames of `T1 − (T1 − T0)/1.3`, `T0` the start and `T1` the change, 33 to 85 ms later than `T0`).

**Seen** 2026-11-23 (the run of "Where the model lacks the slot" above, the probes set for every creature):

- the ice druid's stance after its shots of the slot 164 (above): its idle handler's search, 669 of them, set the rate 1.3 on the stance in play or on the one blending out;
- the hero: the cursor's update tests the art of the right button at the cursor twice a frame (`cEngine_updateWorldCursor` calling `0x51D840` at `0x612E28`), and the launch test of an art on an enemy without a line of sight to it asks a way (`0x4F7710` at `0x51DDC5`, docs/re/arts.md, "The special weapon arts"); with the cursor at rest for some 54 s where the driver had clicked a druid standing among bushes (**hypothesis**: that druid the test's target), 5,516 such searches set the Seraphim's stance in play (the slot 15) to her pace's 1.32, about 100 a second, and none came after she was put elsewhere;
- the other creatures of the place, townsfolk and goblins, searched ways as their mover's order (`0x4F539E`, 539 times) or its search again (`0x4F642B`, 103) took them on, their walk's or run's play following in the same frame, so that no stand of theirs kept the pace's rate.

## The class of what a creature holds

`0x546F80` gives the class from the creature's two hands, the left (`+0x1D4`) and the right (`+0x1D8`), each counted when it holds a `cWeapon3D`:

- with a weapon in each hand and the left one not a shield (category 13), class 8, two weapons, unless neither has a kind, then 0;
- otherwise the kind (`Weapon.pak +0x1E`) of the right hand's weapon, else of the left's, else 0 for bare hands; a weapon without a `Weapon.pak` record has kind 0;
- a kind of 14 counts as 12 (unless its caller asks otherwise; `0x5467A0` does not).

A creature whose flag `+0x14` has bit `0x2000` takes class 0 whatever it holds.
A soldier with a sword and a kite shield is of class 1, one with a crossbow 12, a thief with a dagger 5, a dark elf with two blades 8.

## The arts' slots

A combat art's executor plays its own slot of the model for the art and the class of what the creature holds, not through `0x5467A0`'s table: the Gladiator's 201 to 222, the Seraphim's 153 and 211 to 216 (docs/re/arts.md, "The arts' clips", #509).
The other classes' executors take the same slots for their own `Rundum`, `Hardhit` and `Attacke` (219 to 225, 201 to 207, 210 to 216), 248 and 247 for the Dwarf's guns, 194 on horseback, 144 to 154 for the kicks and the Vampiress's arts, and the action 4's clip where a class has no slot of its own (docs/re/arts.md, "The other classes' clips", #510).

## The other cases

Before the table, `0x5467A0` decides some creatures' slots itself:

- creature 71, the small spider, always plays slot 27;
- a creature whose flag `+0x14` has bit 2 and whose owner (`+0x1EC`) is a creature that `0x4266F0` accepts (a rider on its horse, docs/re/horses.md) plays 169 where its model has it, else 166, the `*_HORSE_IDLE` and `*_HORSE_RIDE` clips, and 193 to 196 or 234 attacking, by its class;
- a flyer aloft (`+0x1F4` bit `0x200000`, docs/re/ai.md, "The flyers") other than creatures 190, 335 and 336 plays 179, `MOTION_FLY_LOOP`; a rider plays 169, else 166 (docs/re/horses.md);
- a standing creature whose flag `+0x200` has bit 6 clear takes the fight stance instead where its model has the stance's slot and the first value of a sorted list at `0xAD3CE4` is below 550 ("The fight stance" below);
- with that bit set, it takes the stance where its flag `+0x200` has bit `0x8000` or it is creature 2 holding class 2, and creatures 5, 6 and 9 have cases of their own;
- an attack has cases of its own: a random one of the five where the model fills more than one, and fixed slots for some creatures.

## The fight stance

Read 2026-10-20 (#155).
The list at `0xAD3CE4` is the hero's, not the standing creature's: the distances from the local player's hero to its nearest hostile creatures, and its first value the nearest's.

- `0x547070` fills it, called only by the creatures' update `0x5375C0` (at `0x53918C`), in its block that runs every period (`+0x204`: 0.3 s for the players' heroes, 5 s for the others, set by the constructor `0x52DE40`), at the first world tick that takes the timer above it, keeping what is above (docs/re/ai.md, "The world's tick", #617), and only for the creature whose id is the local player's, the hero.
- It takes the creatures (types of category 2 or 3, alive) of the hero's sector and its eight neighbours (`0x634340`) that are hostile to the hero (`0x548F30`, the hero first, or `+0x1F4` bit `0x40000`), whose list at `+0x588` is empty and whose distance is below a range: 1,500 in a network game, `(S + 45) · 25` in the single player's (`S` the float at `+8` of the timer `0x6413E0`, a smoothed rate of the frames, **hypothesis**: about 1,900 to 2,600 world units); at most 55, nearest first, the distance `√(dx² + dy²)` in world units (550 is about 10 cells).
- It keeps the count apart (`0xAD3BC8`, 0 while the hero is dead), which the AI and the scripts' getters read first; but it writes the values only when it finds a hostile, and nothing else writes or clears them, so the first value is 0 when the process starts and keeps the last distance found when none is near.
- `0x5467A0` compares that first value with 550 without the count (at `0x546A7E`).
- Bit 6 of `+0x200` is set by the constructor alone (with `0x80000`) for a creature of the heroes' class (`0x426650`, the class record's `+4` is 1) of id 1, or of id 16 or below in a network game: in the single player's game, the hero only, the NPCs of the heroes' classes not.
- Bit `0x8000` of `+0x200` is the hero's, set and cleared by `0x547070`: set while the nearest hostile is within 350, or within 1,000 when the hero holds a weapon of the kinds 7, 12, 13 or 14 (`0x427A40`, `0x427ED0`; **hypothesis**: the bows and the crossbows); with it set and the list not empty, `0x5375C0` posts the engine's event `(0x10, 2, 1)`, else `(0x10, 2, 0)`.
  The first sets the engine's mark of the stance (bit `0x80000000` of its `+0x54`) and starts the 5 s of its fight's marks again, the second does nothing (`0x613290`, read 2026-11-21 for #3067, **confirmed** by code): it plays no music, and of the sound it only keeps a strike from taking the sound manager's calm anew (docs/re/sound.md, "The calm").
  Read again 2026-11-23 for #3150 (**confirmed** by code and **seen**): the look clears the bit (`0x548062`) wherever it lists no hostile or the nearest lies beyond that range, and sets it at `0x547E6B`; the creatures' update tests it for the engine's event right after the look (`0x539250`).
  Of the stores that set the bit alone, a scan of `Sacred.exe` (the stores at `+0x200` and through the side's getter `0x4EB860`, `+0x1F0`) finds one other, the children's shoo (`0x4F2C8C`, docs/re/ai.md, "Children and jugglers"), whose bit therefore lasts until the hero's next look: **seen** cleared by it 0.083 to 0.333 s after each of 14 shoos, so that it never reaches the engine's event and the stance's clip plays only where a run of the standing action falls between the two, for a tick (7 of the 14), the defence that Shift orders taking its place at once (docs/re/fight.md, "The hero's defence").
- A standing creature's action (`+0x150` 0, `0x544890`) asks `0x5467A0` again on every update (at `0x546579`) and changes its clip when the slot changes, at once where less than 0.15 s of a clip played once is left, else blending over 0.15 s (`0x593320`).

So every standing NPC whose model has the stance's slot for what it holds takes its fight stance while a hostile creature is within about 10 cells of the hero, wherever the NPC stands, and the hero takes its own within 350 (or 1,000 with a bow).
By the code, the NPCs also take it at the start of the process, before the hero has seen a hostile, and keep it after the hero kills the last hostile near it until another is found; not checked in the game.
**Seen** 2026-10-25 (#227, check 4), the slot traced at `0x5467A0` for one creature:
- a peaceful Amazon (316, whose model has the slots 14 to 26) with empty hands stood in slot 1 and took slot 14 within 0.6 s of a goblin spawned within 550 of the hero;
- with that goblin killed, the next hostile the hero's look found lay at 990, and she went back to slot 1 within 0.6 s;
- at the Wood Elf's start, the dark elf 693 (319, holding class 1) stood in slot 2 with the nearest wolf at 653, and took slot 15 as a goblin was spawned near;
- the soldiers' model has only the slot 21 of the stances, so the guards holding class 1 keep their standing slot (the guard 23: slot 1, its attacks 84).
The start of the process, when the first value is 0, and a kill that leaves no other hostile in the look's range were not caught: the hero's first look fills the list within 0.3 s, and the start's sectors have hostiles in range.

## In `opensacred-game`

`motions` picks a clip as `0x5467A0` does for standing, walking, running and the attacks (`motions::Slots::attacks`, docs/re/fight.md), by the table and the fallbacks above, from the slots that the model's file keeps, and the class of what the creature holds as `0x546F80` gives it.
`npcs` gives each NPC of the scripts and the fills the clip it stands in and, for one that a script sends somewhere, the clip it walks with, by the class of the items in its hands (docs/re/equipment.md); one whose walk falls back to a run moves at its creature's run speed.
The hero stands, walks and runs, takes its fight stance and attacks by the class of its hands' items: the template's at the start, the chosen weapon set's since #1348 (#488, `npcs::Npcs::hero_gear`), the class's preset clips only where its model's slots cannot be read: the Seraphim with her weapon of kind 1 those of class 1.
Of the other cases, the riders' came with the horses (docs/re/horses.md): the hero in the saddle (`opensacred_rules::horses::ride_slot`, #2299) and a black rider (`npcs`, #2288) play 169, else 166, and strike from it by their class (`motions::Slots::riding_attack`, #2302); a flyer aloft plays 179 (`npcs`, #2250).
Since #2913 the small spider plays its walk's clip, the slot 27, for every action (`npcs::ClipNames::of_creature`); its gait's speed and its strike's hits on that clip are **hypotheses**, neither being read.
The fight stance follows the code (#219): `crowd::Nearest` keeps the distance from the hero to its nearest hostile creature, looked for every 0.3 s of the world's ticks (#617) in the 3 × 3 sectors about the hero within 1,875 world units (the client's range at 30 frames a second) and kept while none is found, 0 until one is; a creature is hostile there when it is alive and hostile to the hero by its side and type (`0x548F30`, `ai::Mind::hostile`, #1013), as the reading above has it; it was its flags word's bit 0 or 1 before, which a summoned follower has too.
While that distance is below 550, every standing NPC whose model has the stance's slot for what it holds plays its stance (`motions::Slots::stance`, `npcs::stance`), else its idle, the clips blending over 0.15 s; so the NPCs that have one stand in it from the start, as the client's 0 at the start of the process has them (not checked in the game).
The hero stands in its own stance (its bit `0x8000`, #230) while the last look found a hostile within 350 units, or 1,000 with a bow, a crossbow or a gun (`opensacred_rules::fight::in_fight_within`, #2914), its model's stance for what it holds, else in its class's idle, blending as its other clips do; its regeneration takes `RegInFight` by the same range.
The hero keeps its class's clips by name (`game::Class`).
Since #3562 a creature with no ranged weapon in hand whose shot ended searches a way as the hero's look wakes it, in the look's own tick since #3572 (`opensacred_rules::minds::searches_after_shot`, `ai::pursue`; "Where the model lacks the slot" above), which sets its standing clip in play to the game's speed times its pace, keeping its place in its loop, where the 0.2 s blend that started that clip is over (`npcs::SearchesWay`, `npcs::paced`, `opensacred_rules::minds::search_paces`), until its next clip starts; so the ice druid's stance loops in 0.5046 s after some of its shots of the slot 164 ("The rate of the clip in play" above).
A shot that plays no clip starts the standing clip anew at its end, at the rate 1.0, blended in over 0.2 s from the clip's second node (`npcs::Clips`), Bevy's transitions blending no node into itself.
The original's other searches of a way are followed by a walk's clip at the same rate, or the game's creatures search none where the original's would and then stand (**hypothesis**: none such seen, "The rate of the clip in play" above); the hero's stance that the cursor's launch test sped is not followed (docs/decisions/arts.md, "Cursor's art test leaves the hero's stance's rate").
