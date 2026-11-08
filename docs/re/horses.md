# Horses

The heroes' horses: the stables and their traders, buying, riding, fighting from the saddle and the horse left alone (#1822).
Read 2026-11-08 in Ghidra, code only, unless marked **seen**; every claim **confirmed** by code unless marked **hypothesis**.
Field offsets are of a `cCreature` unless said otherwise; "the horse" is a creature of `Creature.pak` class 4 (`0x4266F0`), "the rider" the creature on it.

## The types and the links

- The class-4 types are 201 (the saddled blood-terror), 261 (the black rider's horse) and 550 to 558, the last nine those the stables make.
- The boat 1879 (`0x757`) is taken for a mount by the same code wherever a horse is: the click, the art's event, the death's throw-off and the control; the cell test `0x4EBCA0` lets it onto the water's cells of the nibble 2 (the natures `0x90` and `0xA0`).
- `+0x570` is the horse link, both ways: on a hero or a horse trader the id of its horse, on a horse the id of its owner (the getter `0x55C630`, the creature's query `0x46`).
  Its setter `0x55C640(id, notify)`, on the new owner:
  - a horse that `this` linked before, another than `id`, is let go first: its own link cleared, its `+0x14` bit `0x10000000` set, and, where `this` holds an item in its slot 0, the old horse throws down the item of its own slot 0 (`0x4E9900(0)`; **hypothesis**: the test of the new owner's slot rather than the horse's is a defect);
  - with `id` 0 it clears `this +0x570` alone, so the horse keeps naming its former owner unless the release above runs;
  - with a class-4 `id` it links both ways, and with another changes nothing;
  - a new owner of an id below 17, a player, takes from the horse its `+0x14` bits `0x10000000` and `0x80000000`;
  - on a network, for the local player with `notify`, the event `0x15C`.
- The equipment's slot 18 is `+0x1EC` (the slots are words from `+0x1A4`): on a rider his mount, on a horse its rider.
  `sdk_EQUIP(18, id)` (`0x555E00`) writes it and sets `+0x14` bit `0x80`; `0x556A30(18)` clears both; on a network `0x11E` and `0x11D`.
- `+0x14` bit 2 marks an object out of the world, in no cell's chain: the object manager's take-out `0x5FC8B0` sets it, its put-in `0x5FCCB0` refuses an object without it and clears it.
  A rider is taken out of the world as he mounts, so a creature of `+0x14` bit 2 with a creature at `+0x1EC` is a rider and his mount (`0x54FC20`, `0x552660`, `0x5467A0`).
- While riding the rider has `+0x200` bit 4 and the horse bit 8; both take bit `0x10` during the clips of the mount and of the dismount.
- `+0x2B7` bit `0x10` is the horse's override, its trample (below), which every dismount clears on both.
- The horse's class, 1 to 9, is its byte `+0x400` (`CreateNPC`'s tag `0x90`), 1 where 0, answered by the creature's query `0xD` (`0x55D260`); its name is the text `9069 + class` (`0x4267A0`).
  The horse's query 5 answers whether it is alive (`0x549080`).
- The horse's `+0x263` takes the rider's id as the local hero or a player's follower mounts it (its use not read).

## The stables and the horse traders

- The tag parser (`0x454420`, `0x4547B0`) names the horses' tags: `0x63` `SetOwner:`, `0x64` `HorseMerchant`, `0x90` `SetHorseClass:`.
  - `0x64`, on `CreateNPC` or `SetNPCState`, makes a horse trader: `+0x200` bit `0x4000000` and the talk stamp 180,000 (docs/re/netscript.md).
  - `0x90` writes the class byte `+0x400` and raises the level to the same number (docs/re/spawning.md).
- In the Seraphim's start script a stable is a `CreateNPC` of its trader (the types 279, 288 and 652 to 656) with `0x64`, a `CreateNPC` of each horse (551 to 558) with `0x90` 1 to 3 at a `DefPos` place, an `EquipNPC` of its bridle (the items 3197 to 3204 and 3245 to 3259) and a `SetNPCState` of the horse with its level (`0x1F`, 1 to 7, sometimes none) and `0x63` naming the trader.
- `SetNPCState`'s `0x63` (in `sdk_DIALOG_APPLY`, `0x4624FC`–`0x4629D7`): an empty name does nothing; a horse with an owner is let go by it first; the name `hero` (case aside) gives the horse to the creature's controlling player, its talk stamp 0 and `+0x14` bit `0x80000` cleared.
  Any other name, a creature (`0x464BD0`), makes it the horse's trader:
  - the horse's gold grows by 1,000 times its level and is then set to a price of the class's first coefficient alone, `a·(1 + x + x²)`, `x` the level less 1 (the three loads at `0x4626A2`, `0x4626A9` and `0x4626B0` read one address; **hypothesis**: a defect, the restock and the window taking `a`, `b` and `c`);
  - the trader's `0x55C640(horse, 0)` links both, `+0x14` bit `0x80000` is set and the talk stamp is 170,000 where the trader's `+0x1F4` has bit `0x4000` (the guards' mode), else 160,000;
  - the horse joins the trader's stock, at most six.
- A trader holds one link, so each next horse of a stable lets the one before go: once built, only its last horse names the trader, the others none, all keeping their stamps and markers (**hypothesis**: by the setter, not seen).
- `CreateNPC`'s own `0x63` (`0x484E52`–`0x484FE8`) does the same but the stock and the price.
- The stock (`0xAACF80 + 0x79E8`, records of `0xBC` bytes): the trader's id; six horses' ids, their slot 1's item types (their gear), levels, classes and creature types and places (the horse's `+0x18` to `+0x24`); the count, below 6.
- The trader's update (in `0x5375C0`, `0x538199`–`0x53867D`, a creature of `+0x200` bit `0x4000000` that the local player controls, at its timer's tick) replaces each stock horse that is gone or no horse, or that the trader no longer owns and that stands 500 or more from its place: a new creature of the type at the place, its level from `0x537270`, its class, its gold the full price, linked to the trader, `+0x14` bit `0x80000`, the stamp 170,000 or 160,000, the event `0x11A`, a new item of its gear made and equipped (`sdk_OBJ_CREATE_NPOS`, `0x54B130`); on a network the events `0x15D` and `0x16F`.
  So a stable never empties; a horse sold back becomes its trader's but joins no record.
- The restocked horse's level (`0x537270`, read 2026-11-08 for #2320, **confirmed** by code): the local hero's level held at least at the lowest of the sector at the stall's place raised by `OffLevel` (`0xAD4C18`) and at most at its highest raised by `LevelKap` (`0xAD4E1C`), by the difficulty's column, else the hero's plus `rand() & 1`; the hero's level where the sector has no lowest or no highest, and 1,000 without a local hero; on a network the players' levels are weighed first (not followed).

## The price and the horse data

- The balance's image holds three floats a class from `+0x830` (`0xAD4CC0`), the class `c` at `+0x830 + 12·(c − 1)`: `a` 1,000·`c`, `b` 100 and `c₂` 2 in the file.
  The text parser's `BasisPferd<n>:` stores the row `n` one class higher (`0x5EE8E5`), for the unshipped text balance alone.
- The base price (`0x426920`): for a class 1 to 9, `⌊a + x·(b + c₂·x)⌋` with `x = max(L − 1, 1)`; for another class `(5·L + 50)·20`.
  So the class 1 costs 1,102 at the levels 1 and 2 and 2,062 at the level 10.
- The asking price (`0x6BEB90`): `P − ⌊0.005·s·P⌋`, `s` the hero's skill 21, trading (`0x582450(0x15)`), but at least `⌊P/2⌋`, by an unsigned compare.
- Selling pays `⌊0.66·asking price⌋`; healing at a trader costs `⌊⌊price/most⌋·(most − shown health)·0.25⌋`, at most a quarter of the price.
- The horse data, `Balance.bin`'s tail table 4 (`HorseData:`, the parser `0x5F1581`, the map `0xAD57D4` by the class, seven words from the node's `+0x14`):
  - `w0` the horse's pace, the stats' `+0x40` (`0x579A31`–`0x579AD1`, its attack speeds `+0x3C` 140 and `+0x3E` 120);
  - `w1` the trample's most against one creature and `w2` its damage, their bases (`0x552840`–`0x552B48`);
  - `w3` and `w4` the parameters of the curve of the health bonus by the horse's level;
  - `w5` the trample's regeneration, `w6` the riding skill needed.

| Class | w0 | w1 | w2 | w3 | w4 | w5 | w6 |
|---|---|---|---|---|---|---|---|
| 1 | 140 | 5 | 25 | 5 | 15 | 55 | 0 |
| 2 | 155 | 8 | 40 | 8 | 24 | 55 | 0 |
| 3 | 170 | 10 | 60 | 10 | 30 | 55 | 0 |
| 4 | 220 | 8 | 40 | 7 | 21 | 55 | 0 |
| 5 | 200 | 12 | 75 | 11 | 33 | 55 | 0 |
| 6 | 180 | 16 | 90 | 14 | 42 | 70 | 0 |
| 7 | 145 | 18 | 90 | 12 | 30 | 70 | 0 |
| 8 | 160 | 25 | 150 | 20 | 50 | 70 | 10 |
| 9 | 175 | 30 | 180 | 25 | 62 | 80 | 25 |

- The riding test `0x4267D0(skill 22, class, type)`: never the Dwarf (type 8); else `w6` at most the hero's skill 22 (`0x582450(0x16)`, its base and the items' bonus, at most 255); a class of no row needs 0.

## The horse window

- A talk is the talked-to creature's action 7 (`0x52AB70`); a stamp of 100,000 or more calls `0x529930` instead of a dialogue (`0x52BA16`), which refuses a creature ridden or riding and else opens `UI_WND_HORSE` (`cUI_Horse`, `creatureHorseDlg()`): on the horse for a class-4 creature, on the trader for another.
  The stamps 160,000 and 170,000 are the stable horses', not the hires' (docs/re/netscript.md); `0x499E90` gives a talk of 160,000 or more the icon 4.
- The window (`0x6BD2B0`) stands at (256, 204), 512 × 360, with three buttons of 22 rows at its foot (`0x6BED90`): heal at x 10, 120 wide, the text 9066; deal at the middle less 70, 140 wide, 9050; leave at the width less 120, 100 wide, 9053; and the owner's portrait.
- Its states (`0x6BE6E0`):

| State | Opened on | When | Buttons | Shows |
|---|---|---|---|---|
| 16 | either | the hero is the Dwarf | leave (1028) | `UI_DWARF_CANTRIDE` |
| 3 | the trader | the hero has no horse | leave (1028) | T9063 |
| 4 | the trader | the hero's horse lies beyond 1,000 | leave (1028) | T9061 |
| 1 | the trader | the hero's horse within 1,000 | heal, deal (T9051, sell), leave | the horse's sheet, the selling price, T9068 and the healing price |
| 5 | a horse | the hero may not ride it | leave (1028) | the horse's sheet |
| 2 | a horse | of no owner, a trader's or a hero's | deal (T9050, buy, hidden where the gold falls short), leave | the sheet, the price red where it cannot be paid |
| 6 | a horse | another NPC's, the hero's class may steal | deal (T9056, steal), leave | the class's text |
| 7 | a horse | another NPC's, the class may not | leave | the class's refusal |

- The steal (`0x426990`): the Gladiator, the Dark Elf and the Vampiress may (the texts 9080, 9083, 9084); the Seraphim, the Mage and the Wood Elf refuse (9081, 9085, 9082), the Daemon with no text; the states 6 and 7 play the hero's sound (9, `0x20`).
- The sheet (`0x6BD3F0`) lists the owner's name, the class's name, the level, the pace (`0x585070`), the trample's time, damage and regeneration, the health bonus, the resistances, the riding needed against the hero's skill 22 and the price; it hides the deal where the hero may not ride a horse he does not own, and the heal where its price or the hero's gold is 0.
  Read again 2026-11-08 for #2300 (**confirmed** by code): the panel is untextured black at the alpha 159 over (256, 204)–(768, 568), with two 1-pixel separators of `0xFFAC8A67` at the y 236 and 544; every text is the interface's font 3; the title, the owner's name text (`sdk_REFKEY`), is centred with its top at 208 and drawn on the sheet alone; the portrait, 96 × 128 at (272, 268), is the trader's opened on, else the horse's owner's; the rows' labels stand at the x 381 and their values centred on 684, the rows 24 apart from the y 238: the class (`T(9069)` and `T(9069 + class)`), the level (1070), the pace (1098), the trample's three (`UI_TT_OVERRIDE…`), the health bonus, the resistances, the local hero's own, in two rows of two by their images 209 to 212, and the riding (9421) as `needed/skill`, which turns red where the riding test fails (the class's name stays white); the healing's row (9068, red) at 486 on the trader's sheet and the price's (9067) at 510, yellow, red where it passes the gold, the sale's two thirds on the trader's sheet.
  The buttons are plain texts centred in their rects, yellow under the pointer: the healing (266, 544, 120 × 22), the deal (442, 544, 140 × 22) and the leave (648, 544, 100 × 22); the states without a sheet show one justified text in (381, 238), 350 × 296.
  It closes by its leave and by any deal alone (no distance test; Esc and Enter do nothing in its own handlers); its events (9, `0x20`) to (9, `0x24`) play nothing, the gold its coin (7100).
  Its defects: once a state without a sheet showed, the leave reads 1028 for the rest of the run; a horse without an owner keeps the title and the portrait of the window's last opening (**seen**: the first horse of the stable at 3180,2788 under another stable's dealer); a trader's horse of no creature keeps the last state.
  The cursor over a horse no hero owns draws the same facts as one text 32 pixels above it, the horse's description `0x55C9F0`.
- The buttons (`0x6BE400`):
  - heal: the sound (9, `0x24`), the horse's health and shown health to its most, the price paid;
  - sell: the sound (9, `0x22`), the hero lets the horse go; a living horse becomes the trader's, its stamp 160,000 and `+0x14` bit `0x80000`, the hero paid; a dead one is lost unpaid;
  - steal: the sound (9, `0x21`), the hero takes the horse, its stamp 0, and the effect 19 for 25 s on the hero (what it does not read);
  - buy: the sound (9, `0x23`), the hero takes the horse, its stamp 0, the price paid.
  No creature is made: the bought horse is the stable's own, where it stands, with its level, class, gold and gear.

## Mounting

- A left click on a horse (`0x6172C0`) sends the hero the creature event 7, the AI's state 1 (`0x53A170`) with the horse as its target; the cursor is `MOUSE_RIDE` or its kin (docs/re/cursor.md).
- The tests (its sub-state 0):
  - a target ridden already (`+0x200` bit 8) gives way to the clicker's own slot 18 where it has one;
  - a rider clicking his own horse, which stands, and his `+0x152` below 100, both take `+0x200` bit `0x10` and he goes to the dismount (the sub-state 5); another horse stops him;
  - else the horse must stand, walk or run, lie within 1,879, be no other player's, be alive, and its word `+0x94` must not name an entry of the table `0xAAD29C` whose byte `+0x10` lacks the bits `0x3C` (not read);
  - the riding test above, and none of the horse's `+0x1F4` bits `0xF0`; a failure plays the sound (9, `0x26`) and stops;
  - the point to mount from, by the horse (`0x59A3E0`), on no cell of the nibbles 3 or 5 to 10 (teleports, stairs, entrances and exits).
- The approach (the sub-state 4): the clicker walks to the point; a player whose point has not moved in more than 3 tries nudges the horse aside, the event 4 to a point within ±50 of it.
  Within 25 (a player) or 120 (an NPC): both take `+0x200` bit `0x10`, the clicker bit 4 too, is taken out of the world, put on the point, turned to its heading, and plays the slot 165 at the rate 1.0.
  With `Settings.cfg`'s `PICKUPANIM` off (`0x81A690`; on by default) there is no clip and the mount ends at once, as does the dismount.
  An NPC whose horse lies beyond 261, no living hero hostile to it, mounts at once.
- At the clip's end, the horse alive and free: the owners linked both ways, the rider out of the world, `sdk_EQUIP(18)` each to the other, and the rider's bone `Bip01` attached to the horse's `horse_bone` (`0x555300(18)`, Granny's `AttachSequence`); the rider takes the horse's sector, place and floor and plays the slot 169 where his model has it, else 166, at once.
  For the local hero the engine's event of kind 1 with the horse's id passes the control to the horse (**hypothesis** for its handler); then the sound (9, `0x84`, 1, the horse), the stats made again (`0x5796A0`), the horse's bit 8 set.
- **Seen** 2026-11-08 in the original, beside the Seraphim in the starting chapel: a click on a horse that the driver spawned mounted her at once, on its floor too; the portraits' column showed the horse's button and the mount's bar.

## Getting off and being thrown off

- Shift with a left click on one's own horse standing sends the rider the event `0x101` (taken as 7, `0x6172C0`), the dismount (the sub-state 5): the control back for the local player, the attachment undone, the rider put down by the horse (`0x59A3E0`) or at his own place where that cell may not be walked on, on the horse's floor; slot 18 emptied on both, the override ended, the bits cleared, the stats made again; with `PICKUPANIM` the rider plays 167, or 168 below 1 health, then the sound (9, `0x84`, 0, the horse).
- Thrown off:
  - the horse's death, as it plays the slot 173, throws its rider down beside it, standing where alive (`0x53ECA0`, `0x551E40`);
  - the rider's death detaches him, slot 18 emptied on both, and he plays 168, else 173;
  - a player's hero knocked down (the strike's resolution `0x56B130` at `0x56E3B9`: a blow of at least its health from an attacker more than 4 or 5 levels above, `rand() & 1`) is first thrown off;
  - a lethal blow on a rider (`0x56F8DB`) leaves him 1 health and kills his horse instead;
  - a script's `Morph` of a ridden horse.
- The creatures' update (`0x5375C0`) puts a rider whose horse no longer exists back into the world, and one whose horse is dead on his feet, the owners unlinked (the horse lost), the local player's interface told.

## The control and the gait

- The engine's controlled object (`+0x13C`) is the horse while riding: a click on the ground moves the horse (the event 4, `0x617030`), a click on an object sends the rider the event 7, and the right button's art sends the horse the event `0x105`.
- The moving action (`0x4F4770`) hands a rider's movement (`+0x150`, 21 words) and his gait (`+0x1F8`) to his horse, the local player's or one of no master, and he stands; a follower's horse walks to the master once farther than 270, to within 200, at the master's gait.
- The cell update (`0x5941A0`) copies a ridden horse's sector, place and floor into its rider at each move.
- The horse walks and runs by its own `Creature.pak` speeds (`+0x26`, `+0x28`) times its pace, `w0` of its class, times the game's speed (`0x4F75F0`); Shift makes a run a walk, as for the hero on foot.
- The rider plays 169, else 166, whatever the horse does (`0x5467A0`); the horse its own standing, walking and running slots, holding nothing; the rider sits on its `horse_bone`.
- A horse whose rider is a player is never taken away for its distance from the hero (`0x5375C0`).
- The motion slots of riding (the names' table from `0x8E9DF0`): 165 the mount, 166 the ride, 167 the dismount, 168 the death on horseback, 169 the idle on horseback, 193 and 194 the one-handed attacks, 195 and 196 the bow's, 197 to 200 the spells', 234 the crossbow's.

## The cells and the buildings

- The cell test `0x4EBCA0` has no case for a horse.
- The step test `0x4F7E10`: a ridden horse, any horse of an NPC's id, or a creature of `+0x1F4` bits `0xF0` may not step onto a cell of the nibbles 3 (a teleport) and 5 to 8 (stairs).
- The put-in `0x5FCCB0` moves no horse between floors: an entrance (the nibble 9) takes in no horse, a teleport moves none, and the stairs' change of floor passes over it; so a rider stays on the ground and enters no building.
- A ridden horse has an id above 16, so every living creature in its way stops it, the townsfolk and the soldiers too, but while it tramples.
- `sdk_ENGINE_TP` and `Teleport` (`0x54D9D0`, `0x491D40`) move the horse, which carries its rider, and refuse a dungeon's sector.

## Fighting from horseback

- The rider's attack slot (`0x5467A0`, the action 4): a crossbow (the classes 12 and 14) 234; a bow (7 and 13) 195, else 196; anything else 194, else 193.
  He fights from his own state 4 (`0x4E0300`), not turning himself and waiting while his horse walks; the horse never fights.
  The reach is the mean of the two types' (`0x4DBC60`), the horses' 130.
- A struck horse with a rider sends the event `0x100` on to its rider, the forwarding testing that the creature at its `+0x1EC` is no horse (docs/re/ai.md); a hostile's horse under the cursor gives way to its rider (docs/re/cursor.md).
- The black rider's horse (261) circles the local hero with a bow in its rider's hands ("The black riders" below; the mark 10 is the black rider's alone).
- Each class's executor plays a `Hardhit` on horseback by the rider's 194, blended in over 0.2 at the rate 1.0, the rider turned to its target first (`0x4DBAD0`), where on foot it plays its own clip (the Gladiator's `0x4FF0B0` at `0x502B01`, the Seraphim's `0x5038F0` at `0x505EC6`, the Wood Elf's `0x50AC00` at `0x50C35D`, read 2026-11-08; the Dark Elf's, the Vampiress's and the Dwarf's push the same slot, blend and rate); the executors test the rider at their start (`+0x14` bit 2 with a mount).
- The arts: the event `0x105` reaches the horse, whose handler (`0x52FEEB` on) takes the rider's art and asks `0x4D1BE0` (a weapon art) or `0x4D25A0` (a spell) with whether the rider holds a ranged weapon; allowed (no flag bit 0, or bit 1 with a bow or crossbow), the event goes on to the rider; forbidden, the art's slot makes the horse's override.
  Such a slot shows the horse's images and its tooltip the horse's description; an art's tooltip tells `UI_HORSE_CANTCAST` or `UI_HORSE_ONLYWITHBOW` by the same flags (docs/re/arts.md).
- The override, the trample, starts where the horse's timer has run out (`1 − [+0x4E8]/[+0x4EC]` at least 1) and it holds no effect 24: the effect 24 (`0x582D50`) of the value `0x552B90`, the factor `0x552D70` and the seconds `0x552EF0`, and the timers `+0x4E8` and `+0x4EC` of the horse and the rider set to `0x552F60`; the adder sets `+0x2B7` bit `0x10` and makes an object 842 at the horse (the FX event `0x42`); the horse's death ends it.
  With `s` the rider's skill 22, `c = 0` below 1, else `1 + 98·(1 − 1/((s − 1)/100 + 1))` (`0x565D70(1, s, 100, 50)`), `L_h` and `L_r` the horse's and the rider's levels (the constants at `0x891230`, `0x890670`, `0x890D9C`, `0x890CB0`, `0x890C88`):
  - the damage, `⌊w2·(min(L_h + s, ⌊1.25·L_r + 0.5⌋) + 9)·0.1 + 0.5⌋·(1 + 0.01·c)^1.5`;
  - the most a creature takes, `⌊w1·(L_h + 9)·0.1 + 0.5⌋·(1 + 0.01·c)`;
  - the seconds, `(1 + 0.01·c)·HorseOverrideTime` (10, `0xAD5704`);
  - the regeneration, `w5/(1 + 0.015·s)`, 20 without a horse or a rider.
- The trample (`0x5941A0` from `0x596E48`), at each move of a horse of `+0x2B7` bit `0x10` holding the effect 24, its rider a player, takes each of the local hero's near hostiles (`0xAD3CE0`) within two thirds of the reach while the effect's value lasts:
  - a big creature (`0x426CB0`) against a horse below the level 50 and of a class below 8 stops the horse, takes one strike and ends the pass;
  - any other not down already is knocked down (the creature event 3 with 50, `+0x24D` 50, the slot 173, its timer held 3 s, `+0x200` bit `0x40000`), with an object 842 at its neck and another at the horse;
  - the horse strikes it for a fixed amount, the lesser of the effect's value and its factor (`0x4EA3F0`, `0x597B0C`), and the health taken is taken off the value, which spent ends the effect.
  Holding the bit without the effect, it strikes the local hero within two thirds of the reach, its target word naming itself (`0x597132`; **hypothesis**: a defect; who has the bit so not read).
- A shot from horseback divides its range factor by 3, 2, 1.6 or 1 by the struck one's skills 7 and 22 (docs/re/fight.md, "A strike's resolution").
- While riding, the rider's most health (`0x5658F0` at `0x565D05`) is times `1 + 0.01·0x565D70(w3, L_h, 100, w4)·min(1, (L_h + 9)/(L_r + 9))` (`0x5527F0`, `0x5526C0`), rounded; the mount's slot 0, its bridle, counts in the rider's items' bonuses (docs/re/sets.md).
  The share is 0 for a rider of no player's (an id above 16), whose most stays; the stats made again at the mount and at the dismount keep the current health, held at the most (**seen** 2026-11-08: the Seraphim of the level 30 at 403 of 992 mounted a horse of the level 1 and rose by her regeneration toward 1,005; at 401 of 1,005 she got off at 403 of 992).

## The black riders

Read 2026-11-08 for #2288 in the code (**confirmed** where not marked), and seen in the original at 48,22 the same day.

### The pair's making

- Only `FillSector` (`0x47C610`) makes a rider: right after the ranks' rolls, a creature of the type 261 that the creature maker made takes a rider in the branch `0x47E863`–`0x47EFA5`; a `CreateNPC` of 261 (in the trees, as `res:19589` at 1543,1190) is a riderless horse.
- The creature maker destroys the items it was given for a 261, so the horse holds none.
- The rider, a 256, is made without a place (`sdk_OBJ_CREATE_NPOS`) and never put into the world; its model is loaded, and the horse's where it was not.
- The two are linked as a mount (`0x55C640` both ways at `+0x570`, slot 18 at `+0x1EC` both ways with `+0x14` bit `0x80`), the rider's `Bip01` attached to the horse's `horse_bone` (`0x555300(18)`), and the rider takes the horse's sector, place and floor.
- The branch sets neither the rider's `+0x200` bit 4 nor the horse's bit 8 nor the horse's `+0x263`, and plays no mount's clip or sound; every copy of a horse's place into its rider (the cell update `0x596D7D`, `0x59879F`, the trample, the teleport) tests the horse's bit 8, which only the mount's end, two network handlers and the model's load of a horse that already has a rider (`0x5559E0`) set.
  **Seen**: the horse of a woken pair came beside the hero while its rider's cell stayed where it stood, the rider striking from the saddle.
- The rider's flags word `+0x1F4` is exactly `0x80001` (hostile and a boss), the horse's 0; both are put on the side 5, the undead's.
- The rider's level is the run's `L` through the level's setter `0x564D60(L, 1, 0)`, with a boss's hold and the boss's health factor (**seen**: at the level 31, 7,206 health, its horse 1,579); the horse's is `L`, one or two more on a dungeon's sector, or the champion's marker 1 where its rank's roll made it one; its class byte stays 0, so its class is 1 and its pace 140.
- The rider's weapon by `rand() % 2`: 0 the magic short bow 1811 (a bow), 1 the tribal axe 1778, made by the item maker `0x5D5080` at the rider's level on bronze's scale with the key 0 and held (`0x54B130`); the rider takes no `MakeGroup` item, spell, name or group.
- A sector's `Enter` that makes its kept creatures again makes a kept 261's rider from the record after it, where that record is of the type 256 (`0x493E2B`–`0x494221`): at its kept place, with its kept flags, side, level and items, out of the world and linked as above.
- The fills of 261: 35,38, 48,22 and 66,38, all hostile; 256 alone, a rider on foot, is in 16 fills more.

### Who steers

- The rider runs the whole AI and its moves go to its horse: the moving action (`0x4F4770`) of a creature of `+0x14` bit 2 with a class-4 mount hands its movement block and its gait to the horse and stands; its own speeds (50, 50) are used only on foot.
- The horse runs its idle state alone (its flags word 0, its class 4): at every tick, a 261 whose rider has the mark 10, stands and is idle gives the rider its own sector, place and floor and wakes it (`0x54443E`–`0x5444AC`).
- Neither strolls (`0x54EA30` returns for a rider and for any 261), the rider flees at no low health (the mark 10 is not among the specials' fleeing marks) and never gets off by itself.
- The mark 10's remount on foot (`0x4D5923`–`0x4D5A93`) keeps the horse of the near hostiles' list whose distance is above the best so far from 10,000, and then mounts only where it is below 1,024, so it never mounts (**hypothesis**: a defect, the nearest within 1,024 meant).
- The mark 10 is the black rider's alone (`0x43C540` gives it to 256 and no other).
- With a bow, the moving horse goes to the local hero's place plus the vector from the hero turned by +45° for an odd id or −45° for an even one, lengthened to five times the reach where shorter than four times, within 600 of the hero, else to the hero (`0x4F4A85`–`0x4F4C4D`); a standing one sends itself a move to the same point turned the other way and not lengthened, at any distance (`0x5455A4`–`0x5457BB`), running.

### How they fight

- With a bow (the AI's state 8): within 800 of its target the rider shoots from the saddle at once (the slot 195), at twice its clip's rate for the type 256 (`0x4DCDCC`), never backing off; the shot leaves it idle, and its horse wakes it again.
- With the axe (the AI's state 4): it waits while its horse moves, then strikes by the slot 194, not turning itself; the reach is the mean of the two sizes, 130.
- The charge, one decision in eleven of a riding mark 10 (`0x4D5A98`, `rand() % 11` above 9): it runs at its target for at most 20 updates until within the reach less 15, strikes it in passing with no clip (`0x4EA3F0`) with an object 842 at it (the value 1002), and rides away to points 400 from the target along its heading turned ±30° by a coin, up to 10 legs (sub-state 12); 300 or more away, in one case of three it charges again, adding the effect 24 of the value 0 for 10 s to itself and its horse (the floops, the horse passing through creatures; its trample strikes nothing, being no player's).
- The horse never strikes; a struck horse sends the event `0x100` on to its rider (docs/re/ai.md); the hero's cursor over the horse gives way to its rider (docs/re/cursor.md).

### Deaths

- A lethal blow on any rider leaves it 1 health and fells its horse (`0x56F8DB`–`0x56F98C`), posting no kill (**hypothesis**: so the horse gives no experience and no drop).
- The dying horse throws its rider off at the point by it (`0x53ECA0`), who fights on foot at 50, never mounting again.
- The rider's death on foot is an undead's last (class 5's case 7 of `0x532E3C`, docs/re/fight.md): no dying clip, the dust of 781's variant 4, a boss's reward (`30 + 90·E` experience, twice); on horseback, by no strike, its model would take 173, having no 168.
- The horse left alone keeps the side 5 and no flags: it stands where it was, a foe for the cursor.

### Open

- Whether anything in play sets the fill's pair's `+0x200` bits 8 and 4 (the seen horse came apart from its rider's place).
- Whether anything posts the kill of a horse felled in its rider's place.
- The rider's own heading while attached, which the charge's legs read, and the 842's value 1002.

## The horse's button, the call and the horse left alone

- The portraits' column (`cUI_Mercenary`) shows the horse's button while a hero owns a horse, ridden or not (`0x6D7A5C`–`0x6D7AE0`), its tooltip the control's text, and under the portrait the mount's bar, its health's share.
  Its left click (`0x6D6C70`): within 250 of the horse the event 7, the ride order; farther the event `0x116`, the call; its right click takes the item of the horse's slot 0 into the backpack.
- The call (`0x530D68`): a living horse of an id above 16, the hero on the ground (`+0x24` 0) and outside the places of `0x63D260` (**hypothesis**: the dungeons); where the horse was updated within the last two ticks, lies within 1,200 and a way leads to the hero (`0x4F7710`), it walks to the hero's place plus `R(0, 50)` along each axis; else it is put beside him (`0x4EBF20`); then the sound (9, `0x25`) and the hero goes to mount it.
- Nothing makes an owned horse follow its owner; the first AI update passes over every class-4 creature (docs/re/ai.md), so a horse stands where it was left (**hypothesis**: by the absence of a reader).
- A horse is unregistered from its sector beyond 6,900 from the local hero as any NPC (`0x537943`–`0x537AB7`), but one ridden by a player.
- The idle action (`0x544890`) of a creature whose `+0x1EC` names a player not in the saddle empties slot 18 on both and clears their bits (**hypothesis**: only a horse reaches it).

## Health, harm and death

- A healing potion drunk by a rider heals his horse by the same share (`0x5CAC1E`); the trader's heal makes it whole.
- A horse with a rider is a foe of whoever is hostile to its rider (`0x423580`); an unridden horse keeps the horses' side 4, which the hostile sides take for a friend, so monsters leave it alone; a purchase changes no side.
- A ridden horse dead throws its rider off and both lose the link: the horse is lost.
  An unridden owned horse that dies keeps the link (**hypothesis**: no code clears it), and the call refuses a dead horse.
- The hero's death dismounts him (`0x552050`), the link kept.

## Saves, loads and teleports

- The hero's save appends his horse to his own objects after his 19 slots (`0x604C85`–`0x604D06`); his load (`ItemDataMgr::loadHero`, `0x6058A0`) restores it unmounted and linked to him again; where it stands then is not read.
- `sdk_ENGINE_TP` moves a mounted hero's horse instead of him (docs/re/netscript.md); `Teleport` puts a creature onto a dungeon's cell only where neither its `+0x1EC` nor its `+0x570` names a player (`0x4921BE`–`0x49225B`).
- The script condition `0x40` (`0x4987B0`) counts the hero's horse among his followers.

## Sounds

- The mount's failure (9, `0x26`), the call (9, `0x25`), the window's buttons (9, `0x21` to `0x24`) and the steal's (9, `0x20`), their samples in docs/re/sound.md.
  Read 2026-11-08 for #1180 (**confirmed** by code): the call's case (`0x67C1B0`) plays one of 7012, 7013 and 7014 by `rand() % 3`, at the sample's own rate; the failure's (`0x67C246`) the hero's class's `SOUND_FX_NOTMYHORSE01_*` by name through the table `0x684C34` (the Seraphim's `SERA`, the Gladiator's `GLAD`, the Mage's `MAGE`, the Dark Elf's `DELF`, the Wood Elf's `HELF`, the Vampiress's `VAMP` in both forms, the Dwarf's `DWARF`, the Daemon's `DEMON`), of which the installation holds no `SERA` and no `DWARF`, so that those two say nothing; the window's (9, `0x20`) to (9, `0x24`) have no case and play nothing.
- (9, `0x84`, the flag, the horse) sets the sound manager's `+0xAF44` (1 riding) and `+0xAF46` (the horse) at the mount, the dismount and the deaths (`0x67EE1F`), which only `0x68C1A6` reads, where the horse's type's name stands in for the hero's in a match of names against the manager's prefixes (**hypothesis**: of the creatures' sounds; not followed), not the footsteps; the horse's own steps are its type's `NATURE_HORSE` profile 105.

## Open

- The engine event of kind 1's handler, and whether the camera follows the controlled horse.
- `0x52D7A0` and `0x52D610`, which the creatures' update calls for a rider and his horse.
- The bridle's slot: `EquipNPC` (`0x54ACC0`) puts a category-29 item into slot 1 and the restock copies slot 1, while the inventory's bridle use and the button's right click take slot 0.
- The effect 19 of a theft, and whether a theft angers the guards.
- The word `+0x94` and the table `0xAAD29C` that refuse a horse; `+0x263` and `+0x13C` 1234.
- Where a horse stands after a load, and what becomes of an unregistered one.
- The horse models' own slots and the rider models' slots 165 to 169 and 193 to 234, on the files.

## In `opensacred-game`

The stables' horses stand (#541) and the traders' tags are read (#658).
Since #2299 the hero rides (`opensacred_rules::horses`, the module `riding`):
- a click on a horse (`riding::Horse`, of its class by `CreateNPC`'s tag `0x90`, 1 where none), within 1,879 and where the riding test passes (`horses::rides`, the horse data's `w6` against the hero's skill 22), sends the hero to the point by it (`horses::place_by`, `horses::point_by`; the start bone's for the Seraphim, the Wood Elf and the Vampiress), the point following the horse as it moves; within 25 the hero turns to its heading and plays its slot 165, the horse held from then on, out of the way (`riding::Ridden`);
- riding (`riding::Seat::Riding`), the hero plays its slot 169, else 166, its model's root shifted at each frame so that its `Bip01` stands on the horse's `horse_bone`; it goes at the horse's `Creature.pak` speeds times its class's pace times the game's speed, every creature stopping it, by no cell of the low nibbles 3 and 5 to 10 (`terrain::Heights::barred_to_riders`); the horse goes under it, playing its walk or run (`npcs::Steered`);
- Shift with a click on its horse, or the command `ride`, gets it off at the point by the horse with its slot 167; the horse dead, or the hero dying, throws it off at once at that point, without the clip, and a horse gone leaves it where it stands.
- A lethal blow on the riding hero leaves it 1 health and fells its horse instead, with no killer, no experience and no drop (`fight::resolve`, `riding::fell`), whose death throws it off (#2299).
- A horse mounted is the hero's own (`riding::Owned`, #2301), which no sector keeps any longer: it stays in the world wherever it is left; its death under the hero loses it, one left alone keeps the link.
  While the hero owns it, ridden or not, the portraits' column shows its button and the mount's bar of its health's share (`bar::mount_pieces`); the button's left click mounts it within 250, or gets a rider off, and from farther calls it, the hero on foot on the ground: within 1,200 it walks to the hero's place and `R(0, 50)` along each axis, farther it is put there, and the hero goes to mount it (`riding::press_horse`).
  The button's own text shows over it, and the horse's description over the mount's bar (`riding::description`, as `0x55C9F0` writes it: the class's name, the price, the level, the pace, the trample's seconds, damage and regeneration, the health bonus, the hero's resistances and the riding needed; `opensacred_rules::horses::{Prices, trample, health_bonus}`), the same as the original's for the Seraphim of the level 30 on a horse of the level 1 (**seen**).
  A save keeps it apart, as a follower, and a load puts it back where it stood, unmounted and the hero's own (`npcs::mount_of`, `npcs::mount_again`, docs/re/saves.md).
  Since #1180 the horse carried under its rider steps as a creature that walks, by its walk's or run's clip (`steps`, its `NATURE_HORSE` profile 105), the call whistles and a horse refused says the class's line.
  Since #2300 a talk to a stable's horse (`stables::Stabled`, its `0x63`'s trader) or to a horse trader opens the horse window (`stables`): the horse's sheet with its purchase at the asking price (`horses::asking`), which makes it the hero's own, or, at a trader, the hero's horse's sheet with its sale for two thirds of it (`horses::paid_for`), which makes it the trader's stable's again, and its healing (`horses::healing`); the Dwarf's, the horseless' and the far horse's texts; each stable's horse shows its own trader (docs/decisions/interface.md, "Horse window of its own state").
  Since #2320 a stall that a purchase empties its trader fills again (`stables::restock`) once the horse bought is gone, dead or 500 or more from it and the trader is in the world: a new horse of its type at its place, of its class, its level by `opensacred_rules::fight::Rules::restock_level`, the trader's stock; the new horse's gear and gold are not made, and the stalls are not saved.
  Since #2302 the riding hero strikes and shoots from the saddle by its model's slot of its weapon's class (`motions::Slots::riding_attack`: 234; 195, else 196; 194, else 193), never its foot attacks, as far as the mean of its target's size and the horses' 130 reaches (`opensacred_rules::horses::SIZE`), not turning to its target; the horse does not fight.
  Since #2302 the riding hero's most health rises by its horse (`riding::Saddle`, `opensacred_rules::horses::raised_health`), made again as it mounts and as it gets off by whatever way, its health kept and held at the most, and at each other making of it (the level-up, what it holds, the night's form, a point spent, a load): the Seraphim of the level 30 from 992 to 1,005 on a horse of the level 1, as in the original (**seen**).
  Since #2302 the riding hero makes from the saddle the arts that its records allow on horseback (`HeroArts::on_horseback`, `opensacred_rules::arts::on_horseback`), the others starting the horse's trample: its spells by the rider's 199 or 197 (`opensacred_rules::spells::riding_slot`), its `Hardhit` by the rider's 194 at the rate 1.0 (`making`; the rider's turn to its target, `0x4DBAD0`, not followed), and it does not turn to its target.
  Since #2302 an art that may not be made on horseback, made from the saddle (the right button, the console's `arts make`), starts the horse's trample (`trample`) where the rider's timer of its regeneration has run out and none runs: for `opensacred_rules::horses::trample`'s seconds the horse holds its damage in all and its most of a strike (`trample::Trampling`), the timer runs its regeneration (`trample::Timer`), and no creature stops the rider's steps; at each move of the horse, at a tick of the world, each of the hero's near hostiles within two thirds of the reach of the two (`horses::tramples`) is struck by a sure strike of the lesser of what is left and the most of a strike, its physical damage (`horses::trample_strike`), the health it takes taken off what is left as it is resolved, which spent ends it; a big creature against a light horse (`horses::stops_trample`) stops the hero and ends the pass, any other not knocked already is knocked back 50 (`making::Knocked`) and falls by its slot 173 at the rate 1.0; the rider's getting off or the horse's death ends it.
  The strike is the rider's, whose kill and experience it is (**hypothesis**: the horse's owner's share of a kill), and within one pass the strikes not yet resolved hold back as much as they may take (**estimate**: the original resolves each before the next); each knocked creature bursts the Gladiator's green sparks at its neck and the horse at its own for each it strikes (842 of the sight 1014, `hit_sparks`); since #2329 the horse's floops (842 of the sight 1077) spread red tongues flat about its feet while the trample runs (`floops`, `opensacred_rules::particles::HorseFloops`), its light not made.
  Riding, an art slot of a combo or of an art not made on horseback shows the horse's pictures filled by the trample's timer, and its tooltip the horse's description (`bar::slot_icon`, `riding::HorseSheet`, docs/re/interface.md).
  Since #2311 a knock-down throws the rider off its horse at once, before it falls (`riding::throw_off`, docs/re/fight.md, "A knock-down").
  Since #2288 a fill's black horse brings its rider (`npcs::black_rider`): a 256 of the undead's side, a hostile boss at the run's level held as a boss's, holding the magic short bow or the tribal axe by a coin, seated on its horse as the hero is (`npcs::seat_rider`, `riding::Seat`, the horse out of the way, `riding::Ridden`); the rider's AI moves it at its horse's speeds and its horse is carried under it (`npcs::carry_riders`), it rides, stands and strikes by its riding clips (`ClipNames::riding`: 166, 169, the riding attack, 168 else 173), the hero's pointer gives way from the horse to it, a lethal blow fells its horse (`fight::resolve`), whose death throws it off on foot beside it, made again with its health, its clips and its speeds its own; a sector that comes back seats a kept rider on the kept horse of its cell (docs/decisions/spawning.md, "Black riders carried by their horses").
  Since #2288 a riding archer shoots from the saddle within 800 at once, never backing off nor switching to melee, a black rider's shot at twice its clip's rate, and between its shots its horse takes it round the hero (`opensacred_rules::minds::Mind::circles`: the vector from the hero turned ±45° by the parity of its id, lengthened to five times the reach where shorter than four times; **estimate** for the shots and the legs taking turns, the original's horse moving while its rider shoots); farther it comes on.
  Not yet: the control handed to the horse as the original hands it (the hero steers, its horse carried), the theft of another NPC's horse (the states 6 and 7, the effect 19) (#2320), the description's other format while the trample runs, the button's right click (the bridle, whose slot is open below) (#2301), a struck horse's event passed on to an NPC rider (#2288); of the black riders, the waiting for the horse to stand, the charge and its ride away, the rider's last death as an undead's dust and its weapon made at its level (#2288).
