# The containers: chests, barrels, vases, coffins, graves, cabinets

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra on 2026-10-28 (#1379, a subagent of the session), and on the Seraphim's script trees by scans of their Luau translations.
Status: **confirmed** by code unless marked otherwise; the opening of a chest and a barrel seen in the running original on 2026-10-28 (below).

The containers are the `CreateObj` objects of the `items.pak` category 4 (`+0x2E`), the client's `cChest3D`: the chests, the barrels, the vases, the coffins, the graves, the cabinets, the book stands and the shrines.
A click sends the hero to one; at it the container's own trigger opens with its model's clip and sound, its `Take:` function runs and fills it, and it throws everything it holds to the hero's feet at once.
An opened container is emptied for good: nothing fills it again.

## The hero's order

A click on a container gives the hero the event 7 and the AI's state 1 (`0x53A170`) with the container as its target (`+0x104`), as on an item (docs/re/items.md, "The walk").
Its sub-state 0 chooses by the target's category (`0x426220`; the jump table `0x53E3CC` with the byte table `0x53E3DC`): 4, the containers, goes to `0x53A856`.

- A creature of `+0x200` bit 2 goes to `0x53B4BA` (not read).
- A container already emptied (`+0x14` bit `0x4000`, `0x53A863`), but a town stash (the type 5200, `0x428760`), ends the branch: the hero stands until its next order.
- Unless the creature stands within 100 of the container already (the integer square root rounded up, `0x4B2710`), it walks to the container's own point (`+0x150` 1, `+0x174` 1, the stop distance `+0x176` 20, the target `+0x158`, `+0x15C`, `+0x164` 0), not the items' point 25 aside with their stop of 50; a creature busy with another action is first put to its standing (`0x5467A0`).
- The walk's layer is taken as for the items (`0x635240`, `0x6354D0`, `0x635E50`, `0x634150`), and the AI takes its sub-state 2.
  The container's cell is closed (docs/re/movement.md, "The chests' cells"), so the way ends beside it (**hypothesis**).

The sub-state 2 (`0x53BD9D`, the doors' too) waits while the creature moves; a creature target or one on another layer (the layers 0 and 1 counting as one) goes idle (`0x54E6D0`).
Farther than 300 from the container (`0x53BE8C`), or for a container without a straight line of walkable cells to it (`0x4FCC80`, `0x53BEDC`), the order ends without opening.
Then, in a single player's game:

1. the quests' hook `0x471790(creature, container, 3, 0)` runs (not read further);
2. an emptied container, but a town stash, stops there (`0x53BF1E`);
3. the event `0x21` (a `cEvent_object` of the class 6 and the kind `0x22`; `+8` the creature's id, `+0xC` the container's) goes to the container's receive at once (`0x808E50`, `0x618090`, its virtual `+0x18`);
4. the order ends (`+0xFC`, `+0xFE` 0, `0x53C009`) and the creature turns to the container (`0x44DD90`); the hero plays no clip of its own, unlike a pick-up's;
5. the container's `Take:` function runs (`0x4A71F0` with the prefix `Take:` at `0x94E1CC`, then `0x46BF40` with the name and the container, `0x53C107` to `0x53C132`), which fills it ("The contents");
6. where the container has a trigger (`+0x34`), its state is read again (`0x634150`, the word `+0x0A`): open (bit 0), the container is sent the event 3 of the sub-kind 1 with the opener (`0x53C204`), which empties it; closed, the sub-kind 2 (`0x53C2B6`).

A network game sends the event `0x21` without an opener and its packets (`0x7DC260`, `CHEST_OPEN` and `CHEST_CLOSE`, not read).
Only the local player's click gives a container the event 7; the scripts' `ActObj` (93) and `SetObjState` turn a trigger with its clip and sound but empty nothing; that no creature but the hero opens a container is a **hypothesis**.

## The container (`cChest3D`)

The object maker makes a `cChest3D` of the category 4 (`0x451300`); its constructor (`0x5C9720`) builds a `cItemBase` (`0x5BE130(type, 2, 1, 200000)`), clears its contents (the items' references, `+0x1E4` to `+0x1EC`) and copies its type's record (`0x4340B0`, 0x82 bytes) to `+0x118`.
Its virtuals (`0x891BB0`): the draw with its shadow `0x5C97B0` (`+0x14`), the receive `0x5C9840` (`+0x18`), its save `0x5CA460` (`+0x1C`: `cItemBase`'s, then the contents' count and references), its network form `0x5CA690` (`+0x20`), and the query `0x5CA430` (`+0x24`: 10 answers `+0x14` bit `0x4000`, the others `cItemBase`'s `0x5C70C0`).

The receive passes an event first to `cItemBase`'s (`0x5BE580`) and `cObject3D`'s (`0x44BCD0`), which turns the event `0x21` into the trigger's toggle:

- with a trigger: not barred (`+0x0A` bit `0x8000` clear) and closed, it opens it (`0x415F00(1, 0)`), sets `+0x14` bit `0x2000000`, plays the model's slot 170 (`MOTION_ACTIVATE`) and the sound of the type opened (`0x695800(type, id, 1)`); open, it closes it (`0x416280(1, 0)`) with the slot 171 (`MOTION_DEACTIVATE`) and the sound group 3; barred and closed, nothing;
- without a trigger: the slot 170, or 171 where 170 is the current (`+0x64`) and the model has it, the event left `0x21`.

Then `cChest3D`'s own, for the events of the class 6 and the kind `0x22` of the id 3 or `0x21`, by the sub-kind `+0x10`:

- 1 (open), or `0x21` for a container without a trigger: the sound event `(9, 0x16)`; but for a town stash, the contents dropped where there are any (`0x5C9BA0` with the opener `+8`) and `+0x14` bit `0x4000` set, emptied; a town stash 5200 sets the bit and opens the windows `UI_WND_CHEST` with its id and `UI_WND_INVENTORY`;
- 2: the sound event `(9, 0x17)` alone;
- 3: unless emptied, the reference `+0x14` appended to the contents;
- 4: unless emptied, an item of the type `+0x14` made at the container (`0x5D57C0(type, +0x18, −1)`) and appended.

So a container is emptied or not (`+0x14` bit `0x4000`); its look open or shut is its trigger's state and its model's last clip.
The bit is cleared only by a town stash's window closing (`0x6ACE17`) and the network's `CHEST_CLOSE` (`0x7DF3E3`), and the cursor shows no hand and no highlight over a container of the category 4 that has it (`0x612843` to `0x612852`; docs/re/cursor.md).
Nothing fills an emptied container again: its `Take:` entry stays (`0x4A71F0` does not erase it), but the emptied bit stops the order before the function; the start scripts make every container but quest 1111's chest once a new game (**confirmed** on the Seraphim's tree); that the saves keep `+0x14` is a **hypothesis**.

The barrels, the vases, the coffins, the shrines, the cabinets and the book stands open the same way, one class without a break.
Their models: the chests, the cabinets, the shrines, the book stand and most coffins have the clips of the slots 170 and 171; the barrel, the vase, `cabinet_04`, `coffin_02` and the dwarves', hell's, the dryads' and the mushroom room's chests 170 alone; `chest1a`, `coffin1a`, `coffin1b`, `coffin2` and `i_chest_02` none (counted on the converted models).

The sound of the type opened comes of `0x44BCD0` → `0x695800`, the doors' path (docs/re/sound.md, "The sources"): 605 for 5200 to 5213 and 5220 (a barrel, 5207, sounds as a chest), 609 for the shrines 5214 and 5215, 610 for the vase 5218, 611 for the book stand 5222, 324 or 325 for the graves; the sound events `(9, 0x16)` and `(9, 0x17)` are not identified.

## The contents (`FillChest`)

`CreateObj` keeps its `0x41` as the name `Take:` and the function (`0x4862C3`) in the interpreter's map at `+0x358` (entries of 0x44 bytes, `0x48856A` to `0x4885EE`) and sets the object's `+0x14` bit 1.
`0x4A71F0` runs a `Take:` name that holds `Fill`, `fill`, `FILL`, `Chest`, `chest` or `CHEST` for an object of the category 4, with the container as the function's object.
So a container's `fillchest_<name>` runs as it opens (**confirmed** by code and in the running original, below; docs/re/netscript.md's quest 10260), and its `FillChest` records (78, `0x4A02A0`, the walker's case `0x475B0D`) act on the function's object where they name none (`[esp + 0x320]`, `0x4A0607`).

`FillChest` seeds `srand` (`0x849B33`, `0x4A0705`) with the sum of the hero's type `+0x10`, its id `+0x0C`, its experience `+0x3B4`, and the container's `x` `+0x1C`, `y` `+0x20` and id `+0x0C`, and from the time again at its end (`0x4A1439`): a container gives a hero the same until the hero's experience changes.
Its operands (the jump table `0x4A14C4`, the byte table `0x4A14E4`):

- `0x01`: a container by its name (`0x464BD0`; logged where none is found);
- `0x02`: a type `T`; a rune, for a hero above level 2, is replaced where `R(0, n)` passes `UpgradeDrop` (50), `n = 0x5595A0(1000)` (docs/re/netscript.md, "The rewards"): in 1 case of 26 one of the icons 5101 to 5106, or the potion 5171 where an icon lies in the world, else one of the instruments 5108 to 5112;
- `0x53`: the value `v + R(0, ⌊v/10⌋ + 1)`;
- `0x57`: a count of random items; `0x86`: a count of books; `0x9E`: the key 15;
- `0x54`, `0x55`, `0x56` and `0x6C` are read and ignored (`0x4A05E0`).

In this order (`0x4A0711` on), with `B` the eight numbers of `BalanceSpawnItem` (107, the walker's case `0x475EBE`; 15, 82, 20, 1, 40, 40, 1, 1 in all 20 trees, 25, 45, 30, 10, 50, 50, 50, 50 by default, `0x4614CB`):

1. without a value, a count of books or `T`, the value is the hero's level times 800;
2. with a value above 0, `R(0, 100) ≤ B[0]` gives nothing (16 cases in 101); the shrines and the graves (5214 to 5217, 5219, 5223 to 5226; the table `0x4A158C`) raise a creature instead, at the hero's level (`0x564D60`), near it (`0x45FF80`, not read), with `+0x1F4` bit 1 and the stance `0x52E420(1, 0xB)`: the ghost 46 in 2 cases of 3, else the noble 597 in 2 of 3 or 598;
3. else, for the local hero, a run of items (`0x4A08BE`) while `R(0, 100) ≥ B[1]` (19 cases in 101), each:
   - of a category by `R(0, 100)`: the weapons below `B[4]`, else one of 6, 13, 17, 18, 19, 21 to 25 below `B[5]`, else a rune below `B[6]`, else one of 7, 8, 9, 15, 16, 20, 29, 30, 31 below `B[7]`, else rolled again;
   - at the hero's level, 1 or 2 above where `R(0, 100) < B[3]`;
   - its type `0x5C7FA0(category, level, 100, 15, 0, 2 or 0, 1, 14)` turned to the hero's class's (`0x43CA20`), in 2 cases of 1,001 an icon 5101 to 5106 where none lies in the world;
   - made by `0x5D57C0(type, the container, −1 or 15)` at a level near the hero's held within the container's sector's levels raised by the difficulty, its quality key rolled with the luck and the time played, at most 11;
   - its price (`0x5BEB30(−1) + 8`) taken off the value, and appended (the sub-kind 3); a value of 0 or less ends the run without gold;
4. at the run's end a pile of gold (`0x4A0CE5`): 5132 for a value below 10,000, 5133 below 100,000, 5134 below 1,000,000, else 5135; its amount first `v·B[2]/100`, then, where the hero's sector is found (`0x634840`), `g = a + (b − a)·L/100` of `GoldProLevel` (20 and 2,000) with `L` the hero's level held to the sector's highest (`+0xD5`) plus `LevelKap`, and the amount `R(⌊g/2⌋, g + ⌊g/2⌋)`: the value decides whether items come first, not the gold;
5. `0x86` makes that many books, each `4100 + R(0, 202)` (a missing type passed over), in 1 case of 3 `4613 + R(0, 26)`, in 1 of 91 `5150 + R(0, 10)`;
6. `T` without a name or a value makes `T` (the sub-kind 4, `0x4A124E`);
7. `0x57` makes that many items, each of a random category (`0x43B090(0)`, the table `0x890B60`) at the level `⌊rand()/32768·(L − 1) + 1⌋` (`0x5C7FA0` with the hero's luck `0x564A60` and its type, `0x5D57C0`).

In the Seraphim's tree (counted): 1,676 records of `0x53` alone (200 in 1,352 of them, 30 to 6,500), 33 of `0x02` (21 of them the pile 5135), 7 of `0x57`, 2 of `0x54` and 2 of `0x86`; `FillSarkophag` raises a monster itself in a case of 4, and the dwarves' chests' `super_kiste` gives four `Gewinn`s and no contents.

### The throw (`0x5C9BA0`)

Each item goes into the world at the container's point (`0x5FCCB0`), is lifted (`0x44D520`), marked (`+0xB0` bit `0x8000000`) and thrown by `0x44DED0`:

- with an opener, to the opener's point `±R(20, 50)` along each axis (`0x44DED0(x, y, 0, −1000, 80, 0.9)`): the −1000 makes `0x44DFE0` fly it from where it stands to that point in 0.9 s, along an arc peaking at 80 (`80·(1 − 4(s − ½)²)`), to the hero's feet;
- without one, at `(±R(20, 50), ±R(20, 50), 100)` and turning `R(−6, 6)`.

Landed (`0x44F4D0`), the piles and the runes stand upright; gold tells the quests' manager (`0x4AEBD0`) and a rune counts in the statistics.

The local player's marked item (`+0xB0` bit `0x8000000`), once it lands, is tested from the local hero (`0x603E30`, at `0x44F62C`) (**confirmed** by code, read 2026-10-28, #1407):

- where the hero may not stand on its cell (the hero's test of a cell `0x4EBCA0`, on the item's cell position `0x6224E0` and its layer `+0x2C`), or no straight line of walkable cells leads to it from the hero (`0x4FCC80`), the item leaves the world's map (`0x5FC8B0`) and comes back into it at the hero's own place (`0x5FCCB0` with the hero's `+0x18`), its transform made again (`0x44D520`): it lies at the hero's feet;
- in a network game (`0x182EBEC`) it stays where it lies if any of the players 1 to 16 stands within 2,000 of it (the integer root), on its level, with a straight walkable line to it.

Besides the container's throw (`0x5C9E9C`), the bit is set by the jump of an item without room (`0x5592B0`, docs/re/items.md) and by a throw from a creature's place at `0x556565` (its function not read), found by a scan of the stores of `0x8000000`.

## The triggers and the locks

The `0x29` of a container's `CreateObj` is its own trigger (`+0x34`), made closed without cells; its cell is patched to `chestTrigger`, so opening the own trigger frees no cell.
In the Seraphim's tree: 1,883 containers with their own trigger and a function, the 43 town stashes 5200 (`chest_<town>`) with their own trigger and no function, 9 dwarves' chests 5195 whose `0x29` is `chestTrigger` itself with `super_kiste`, and quest 1111's chest 5201 without a trigger.
So opening a dwarves' chest opens `chestTrigger`, freeing every container's cell, until another closes it, and a second one, finding it open, closes it, runs `super_kiste` again and stays unemptied (**hypothesis**, by the code).
A container without a trigger is emptied by the event `0x21` before its function runs, which then fills nothing; quest 1111's alone, whose function fills nothing.

`SetObjState` on a container (`0x48AE90`): `0x06` and `0x07` bar and unbar its trigger (`+0x0A` bit `0x8000`); `0x0F` and `0x10` open and close it with its sound (`0x48B278`) and the slot 170 or 171, and empty nothing; in the Seraphim's tree it names one chest, `chestR1_03`, from its own fill, after its trigger opened, to no effect.
A barred container's click leaves its trigger shut and sounds `(9, 0x17)`, but its function fills it at each click (**hypothesis**: the same items by the seed); unbarred, one opening throws them all.
The test `0x81` (`IsLocked`) reads the bit `0x8000`; no record of the Seraphim's tree tests it.
Triggers of the flags `0x44` (`+4`) want a key item (`+0x0C`) or sound 331 or 332 (`0x415F00`); the containers' triggers have the flags 0.

## The town stash (5200)

A town stash always opens, its emptied bit kept only while its window shows: the AI passes over the bit for it, and its open sets the bit and shows `UI_WND_CHEST` with the inventory.
Its window closing clears the bit and sends the event `0x21` without an opener, which closes its trigger with the slot 171 and the closing sound.

## The hidden stashes (`MachVersteck`)

Read 2026-10-30 (#1478), **confirmed** by code unless marked otherwise, and seen in the running original (the driver of #448, the Seraphim's new game).

- The start function lays the stashes' places into named pools: `AddPoolPos` (108) adds a cell to the pool of its name, `Versteck_<region>` (`Versteck_uw<region>` in the underworld), and `SetVar` sets each region's count `MaxVerstecke_r<region>` (30 for the region 1).
- `MachVersteck` (140, `0x4B0C00`) takes up to five names (the tags `0x01`, `0x1D`, `0x1E`) and a count (`0x0B`): the mouse-over's text, the pool, the function and the count's variable; without a count it reads the count from the variable of the fourth name (the interpreter's variables at `+0x7550`, entries of 0x24 bytes).
- It then picks that many places one by one: of the scripts' pools' list (`+0x334`, entries of 100 bytes, the kind 2 a pool's place, its name at `+4` and its cell at `+0x44`, `+0x48`) the places of the pool, at most 100, one at random (`rand() % n`), which leaves the pool.
- Each place becomes a mouse event (`0x4AB160`, an entry of 0x58 bytes at `+0x370` and in the list at `0xAB45B8`: its cell and floor, the function's name at `+0x18`, flags at `+0x14`, the mouse-over's text), and the static object that stands on its cell (`0x634AF0` on the ground, `0x6354D0` and `0x634C50` on a floor) takes the bit `0x80` of its byte `+0x1E`; the client logs `versteck x, y, floor` for each (**seen**: 30 places about the start among some 800 lines in all).
- In the Seraphim's tree 37 records of `MachVersteck` run in its start function, each a region's pool with the function `VERSTECK` (`VERSTECKuw` in the underworld) and the text `VERSTECKMOUSEOVER`; the functions `SetzeVersteck` and `SetzeVersteckUW` lay the same by `GetPoolPos` and `MouseEvent` and no record runs them (**hypothesis**: an older way).
- Its blink (`cWorldView_renderTileRow`, `0x62CF12`): a static object of the bit `0x80` whose cell holds a mouse event (`0x4A6560`) of a function and not of the flag 1 adds an entry of the kind `0x20` with its global cell to the world view's list 3 at `+0x96AF4` (the sub-kind 1 where the event has the flag 2), after the cell's statics, so that it draws over its own object.
  `MachVersteck` gives its events the flags 2 (`0x4B0E6E`), so a stash's entry is of the sub-kind 1.
  Read 2026-10-30 (#1519, **confirmed** by code): the lists' pass (`0x62FF60`) draws such an entry at its cell's centre, `(cell + 0.5) · 53.66563` at the height 0, taken to the glitter's space of the camera (`0x6239C0`, `0x623940`), by the sub-kind 1 through `0x40A230` and by the sub-kind 0 through `0x409D00`.
  `0x40A230` draws only for the first 1,024 milliseconds of each 8,192 of the clock that the glitter takes (`0x6413E0`), every stash in step: with `s` the share of the 1,024 passed, a square of the half side `10 sin(πs)` in the glitter's units, turned `2s` radians, of `PARTICLE_SPARK07` (the textures' holder `0x401030`, its `+0xC0`) tinted `0xBB9360`, and over it the square of half its side tinted `0xBBBBBB`, both added to the frame (`0x643430`: the depth test on, its write off, the source and the destination `ONE`, the linear filter).
  `0x409D00`, which no stash takes, is the glitter's draw (docs/re/items.md, "The glitter") with the table of sixteen numbers after the glitter's (`0x8E80CC`), its sparks rising up to 25 (`|sin(3t + a[i + 6])|`) at the hue's value 125 and its grey squares' half side 2 plus `sin(t·a[i + 4] + a[i + 6])`.
  **Seen**: a pale flash about the stash's bush, in one frame of each 15 of a burst taken 0.56 s apart, so every 8.4 s or so (**estimate**), as the 8.192 s of the code.
- The cursor over it (`cEngine_updateWorldCursor`, `0x612909`): the object's bit `0x80` finds its event (`0x4A6770`), and its mouse-over text shows beside the hand (**seen**, the text of `VERSTECKMOUSEOVER`).
- A click (`0x617A5B`) sends the hero to it (the order 7 with the event's index), and as it arrives (`0x53BA29`) the event's function runs (**seen**: the log's `WorkFunktion` of `VERSTECK`, an item at the hero's feet); the stash is then gone, the cursor over it showing nothing (**seen**).
- `VERSTECK` runs `CreateObj` at the hero (`CPOS:HERO`) with `0x54` 3 and `0x79` (`0xFFFFFF`, 4) and no type, which makes a random item for the local hero (`0x4863D4`): `srand(time)`; the difficulty's bonus `d`, 0 on bronze, 1 on silver, 3 on gold, 5 on platinum, 8 on niobium; in 1 case of 4 a potion (`0x43B1A0` of the difficulty, docs/re/items.md); else in 1 of 3 a weapon (5), else in 1 of 2 an armour (6), else a category of `0x43B090(0)`, each by `0x5C7FA0(category, L + rand() % (⌊L/5⌋ + 3) + d, 0, 15, the hero's type in 1 case of 2 else 0, 2, 1, 14)`, `L` the hero's level; without a type a weapon's roll once more.
- Not read: what `0x79` gives the item maker, how the event leaves the lists once its function ran, the order 7's walk, and the underworld's pools' use.

## Seen in the running original

On 2026-10-28 (#1379, the driver of #448, the save `start-seraphim`, traces of `0x5C9840`, `0x4A02A0`, `0x5C9BA0`, `0x5D57C0` and the item maker):

- a click on the chest `chest_sera2` (5201, cell (3232, 2512) on the floor 1, four cells from the hero) walked the hero beside it and opened it, its lid up; its receive ran `fillchest_sera` with the chest's id, whose two `FillChest`s of `0x02` made the rune 5394 and the item 2149 (the item maker, `0x5D57C0`, the sub-kind 3 each), then `dropItems` threw both to points beside the hero (the log's `(−1000, 80, 0)`), where they lay;
- a click on the barrel 5207 of `chestR1_30` (cell (3225, 2522)) ran `fillchest_chestR1_30`, its `FillChest` of the value 350 making one item, thrown to the hero; the barrel stayed, open.

## Not read

What the sound events `(9, 0x16)` and `(9, 0x17)` play; `0x5D57C0`'s level and the arguments of `0x5C7FA0` here; where `0x45FF80` puts the ghost; whether the flight stops at a blocked cell; whether `0x4FCC80` passes the container's own closed cell; whether the saves keep the emptied bit; `cChest3D`'s `+0x44` (`0x5C9800`) and `0x608080`.
To see in the running original: the same chest's contents after a reload and after the hero gains experience; the share of containers that give nothing and the gold's range over many; the graves' and the shrines' ghosts; a dwarves' chest and the cells of the barrels after it; a town stash; a chest against a wall; quest 1111's chest.

## In `opensacred-game`

Since #1380 the hero opens the containers (`containers`): a left click on one that is not emptied, nor a town stash, picks the container nearest the cursor's line of sight as the doors are picked, and the hero walks where the click sends it, beside the container's closed cell; once its walk ends, or at once within 100, standing within 300 of the container it opens it: the model's `*_ACTIVATE` clip, the type's sound (the profiles 605, 609, 610 and 611, the graves' 324 or 325 at even odds), its own trigger opened (`npcs::open_container_trigger`), its `Take:` function run (`scripts::run_function`), and the container emptied, kept by its placement (`npcs::Npcs::emptied`), so that its sector places it again emptied, its clip played to its end.
The cursor shows the hand over a container that opens, none over one emptied (`cursors`).
**Seen** in the game: the Seraphim's start's `chest_sera2` opened by a click 0.75 s after it, the hero beside it, its lid up, `fillchest_sera` run.
Since #1381 the container's `FillChest`s, taken out of the records its function ran, make what it holds (`opensacred_rules::containers::fill`, `SPAWN_ITEM`): seeded by the hero's type, its experience and the container's point and placement, the value's nothing or ghost, the run of items with their prices (`Weapon.pak +0x8C`, 600 without a record) off the value, the gold, the books and the type named, a rune of which may be replaced; each thrown from the container to a point `R(20, 50)` from the hero along each axis, in 0.9 s along an arc that peaks at 80 (`drops::Flight::to_point`), a raised creature beside the hero.
**Seen** in the game: `chest_sera2` threw the rune 5394 and the item 2149, as the original's did, both landing 36 and 43 from the Seraphim.
Since #1407 the thrown items are marked as the hero's (`drops::throw_to`), and one that lands on a cell that the hero may not stand on, or without a straight line of walkable cells to it from the hero, is moved to the hero's point (`drops::fly`), where the hero picks it up; before, it lay out of the hero's reach, a click on it giving up.
Not followed yet: the town stash's window, the straight line of walkable cells that the opening wants, `FillChest`'s random items of `0x57` (the categories' table `0x890B60` is read since #1518, `opensacred_rules::drops::random_category`), the class's variant of a type (`0x43CA20`), and the item maker's own level and key for the contents (docs/decisions/items.md, "Containers' items at the hero's level").
Since #1518 the hidden stashes (`stashes`): the scripts keep the pools that `AddPoolPos` fills and the stashes that `MachVersteck` lays from them, drawn by the scripts' dice (`opensacred_rules::quests::Pools`, `Stashes`), 819 in the Seraphim's tree as the original's log lists them; each stands at its cell's centre, 40 above its ground, and is picked within 45 of the cursor's line of sight, where the cursor shows the hand and the stash's text (`cursors`); a left click sends the hero there, and standing within 300 of it the hero searches it: the stash leaves the scripts' list and its function runs (`scripts::run_function`).
A `CreateObj` of `0x54` and no type makes a random item for the hero (`opensacred_rules::drops::script_item`, with the categories' table `0x890B60` as `random_category`), made as a container's item (`drops::contained`) and thrown 20 to 50 from the hero along each axis; the command `stashes` lists those about the hero.
**Seen** in the game at the Seraphim's start: the stash in the bush at 3216, 2556 showed its text under the cursor, and a click gave the item 7089 at her feet, the stashes then 818.
Since #1519 every stash blinks as above (`stashes`), its squares brought 49 units toward the camera along its line of sight (docs/decisions/drawing.md, "Stashes' blink in front of the ground").
**Seen** in the game: the turning four-pointed spark, warm white, over the stone beside the fern of the stash at (3284, 2526), growing and shrinking.

Not followed: the static object's mark (the stash is picked at its cell's point, not by the object's picture), what the `CreateObj`'s `0x79` gives the item maker, and the order 7's walk.
