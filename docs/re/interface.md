# The interface over the world: the bar and the portraits' column

Read 2026-10-27 (#1087) in the GOG `Sacred.exe` 2.28 (the taskbar `cUI_Taskbar2`, the portraits' column `cUI_Mercenary`, their buttons and statics, the interface's images' table) and measured on three frames of the running original at 1024 × 768 (a new Seraphim at the start, under fog, rain and snow; the frames stay in the scratchpad).
Status: **confirmed** by code unless marked otherwise; "measured" means a composite of the atlases at the places below was compared pixel by pixel with a frame.

All places are pixels of the 1024 × 768 screen.
`cUI_Manager_createGameWindows` (`0x759AF0`) makes `UI_WND_TASKBAR` at (0, 676, 1024, 92) and `UI_WND_MERC` at (932, 0, 92, 676); a control made with its window as the parent is placed relative to the window's corner, and the potions' belt, the clips and the taskbar's "+" are written in screen coordinates directly (both **measured**).
"Image N" is the entry N of the interface's images' table at `0x9ECEB0` (docs/02 §11).

## The images' table at run time

`0x761580` prepares every entry once, after the textures load (**confirmed**):

- It adds 1 to the right and bottom edges (so they become exclusive), loads the TGA named at `+0x08` and keeps its handle at `+0x28`.
  So the name is 32 characters (`+0x08` to `+0x27`), not 36.
- `+0x4C` and `+0x50` become the width and height, `right − left + 1` and `bottom − top + 1` of the table's numbers.
- The rectangle becomes texture coordinates over a 256-pixel atlas: left/256, top/256, (right + 1)/256 + 1/512, (bottom + 1)/256 + 1/512.
- `+0x3C`, `+0x40`, `+0x44`, `+0x48` are two anchor points in atlas pixels; each value not 0 is made relative to the rectangle's left or top.
  An entry whose `+0x04` is 9 gets both anchors at its centre (width/2, height/2) instead; the buttons and icons are of this kind.
  The bar's pieces use the first anchor's y as the piece's distance from its top to the bar's line (below).

Controls (`cUI_Static2`, `cUI_Button2`) keep their picture as a texture handle and four texture coordinates in the order left, right, top, bottom (`+0x54`, `+0x58`, `+0x5C`, `+0x60`), a button a second set at `+0x64` to `+0x70`.

## The bar (`cUI_Taskbar2`)

Built by `0x6DF590` with `0x6E4DF0` (the controls) and `0x6E0070` → `0x6E01F0` (the slots and the rail).
`n` below is the number of slots a side, 1 to 5, by the hero's level (`+0x3FE`) through the table at `0x95703C` (`0x57F890`): 1 below level 2, 2 from level 2, 3 from 8, 4 from 16, 5 from 30.
The bar checks the level whenever the mouse is tested against it (`0x6DFD60`) and rebuilds the slots and the rail when `n` changes.

### Elements

| Element | Rectangle (screen) | Images by state | What it does |
|---|---|---|---|
| Disc of day and night | an ellipse of 110 × 55 centred on (513, 710.5) | `GUI_daynightdisc.TGA` turned by the clock, `GUI_daynightalpha.TGA` over it unturned (below) | the time of day; tooltip text 9915 over (477, 680, 76 × 20) (`UI_TB_DAYNIGHT`) |
| Disc's housing (`UI_STATIC`) | (397, 680), 231 × 87 | image 11, `GUI_main_03` (0, 169)–(230, 255) | the oval frame about the disc and the bar's centre, drawn over the disc (**measured**) |
| Rail, left end | (362 − 66n, 747), 103 × 20 | image 5, `GUI_main_03` (0, 56)–(102, 75) | decoration (**measured** for n = 1: x 296) |
| Rail, left middle | from the end's right edge to x 397, one piece after another | image 6 (0, 76)–(99, 93) at y 749 or image 7 (0, 94)–(99, 110) at y 750, each piece chosen by `rand() & 1` | decoration; none for n = 1 |
| Rail, right end | (570 + 64n, 747), 105 × 20 | image 8, `GUI_main_03` (0, 111)–(104, 130) | decoration (**measured** for n = 1: x 634) |
| Rail, right middle | from x 570 + 64n − 64 leftwards while x > 532, each piece at x then x −= its width | image 9 (0, 131)–(99, 149) or 10 (0, 150)–(99, 168) at y 748, by `rand() & 1` | decoration; one piece at x 570 for n = 1 (the frames' match is poor, **hypothesis** that it is image 9) |
| Weapon slots (left), i < n | (394 + 66(i − n), 691), 64 × 64 | background image 515 (`GUI_spell03` (192, 192)–(255, 255), a pink orb) for the selected set, 635 (`GUI_spell10` (192, 192)–(255, 255), a beige orb) for the others; ring 103 (`GUI_main_01` (83, 169)–(146, 232), gold) selected, 102 ((19, 169)–(82, 232), dark iron) not; while an item is dragged over it, halves 531 and 532 (`GUI_spell04` (192–223 and 224–255, 192–255), green) selected or 549 and 550 (`GUI_spell05`, pale green) not | the weapon set: the two hands' items drawn as 3D models in the orb; click selects; tooltip text 9906; **measured** (the background and ring at (328, 691)) |
| Weapon set's damage icons | from (slot x + 4, slot y + 57), 16–17 × 16 each, each next one 4 pixels less than its width along (the direction by a flag, not read) | images 205–208 (`GUI_main_06` (192, 240)–(255, 255): physical, fire, magic, poison), or framed 209–212 ((192, 204)–(255, 219)) | the set's kinds of damage, each drawn at an alpha of the sum of the two items' bytes for that kind (`0x5C8D70`), only while `TASKBAR_ICONS` is 1 (the installation's `Settings.cfg` has 1); **measured** image 205 at (332, 748) |
| Art slots (right), i < n | (640 + 66i, 691), 64 × 64 | the art's three images (below); without an art 479 (`GUI_spell01` (192, 192)–(255, 255), pale blue) selected or 635 not, 533 (`GUI_spell04`, green) while the cursor holds a spell, or a weapon art in the edit mode, wherever it is (**seen**; read 2026-10-30 for #1090, `0x6E3430`), 569 (`GUI_spell06`, purple) for the Vampiress at night (`+0x14` bit `0x2000`); ring 103/102 as the left | the combat art or combo of the right mouse button; click selects; tooltip text 9912 and the art's description; **measured** (Attacke's 620 and the gold ring at (640, 691)) |
| Inventory (`UI_TB_INVENTORY`) | (399, 705), 34 × 34 | image 87 (`GUI_main_01` (102, 0)–(135, 33)), down 88 ((102, 34)–(135, 67)) | toggles the inventory (`cCommand_onInvOpen`/`onInvClose`); tooltip 9907; **measured** |
| Options (`UI_TB_OPTIONS`) | (434, 729), 34 × 34 | 89 ((136, 0), a gear), down 90 ((136, 34)) | toggles `UI_WND_ESCMENU`; tooltip 9908; **measured** |
| Map (`UI_TB_MAP`) | (558, 729), 34 × 34 | 91 ((170, 0), a grid), down 92 ((170, 34)) | toggles `UI_WND_MEGAMAP`; tooltip 9910; **measured** |
| Book (`UI_TB_BOOK`) | (594, 705), 34 × 34 | 93 ((204, 0), a book), down 94 ((204, 34)) | toggles `UI_WND_QUESTBOOK`; tooltip 9911; **measured** |
| Collect (`UI_TB_COLLECT`), single player | (496, 743), 36 × 26 | 134 (`GUI_main_01` (184, 230)–(219, 255)), down 135 ((220, 230)–(255, 255)) | sends the hero the creature event `0x113`, which sends it collecting the items about it (the AI's state 12, docs/re/items.md, "The automatic pick-up"; read 2026-10-28, #1408); tooltip 9909; **measured** |
| Net info (`UI_TB_NETINFO`), network game | (496, 743), 34 × 25 | 140 ((185, 203)–(218, 227)), down 141 ((149, 230)–(182, 254)) | toggles `UI_WND_NETINFO`; tooltip 9917 |
| Potions' belt, 5 buttons | (435, 667), (465, 653), (497, 649), (530, 653), (560, 667), 32 × 32 each | filled: 176 (`GUI_main_05` (129, 65)–(160, 96), red), 179 ((129, 164), purple), 177 ((129, 98), blue), 178 ((129, 131), green), 180 ((129, 197), yellow), slot 1 to 5; empty: 175 ((195, 65)–(226, 96), a grey flask) | a slot shows its colour while `0x5DA8C0(slot, 0)` is not 0, else the empty flask; a click sends the engine event `0x17` with the slot (1–5) and which button, the left drinking its potion (seen, #1311); tooltip the item's description; only while `SHOWPOTIONS` is 1; **measured** (slot 1 red, slots 2–5 empty) |
| Belt's clips | (424, 690) 16 × 18, (586, 690) 18 × 18 | images 39 and 40 (`GUI_main_03` (240, 201), (238, 183)) | fully transparent in this `GUI_main_03` (**measured**): they draw nothing |
| Compass needle | the 18 × 58 sword turned about (512, 710) | image 101 (`GUI_main_01` (0, 169)–(17, 226)) | points at the marker 0 (below); tooltip text 9916 over (477, 700, 76 × 35) (`UI_TB_COMPASS`) |
| Compass pins | 17 × 16, on ellipses about (512, 710) | image 136 ((133, 137)–(149, 152), white arrowhead, blue dot), image 137 ((133, 121)–(149, 136), red dot) | the markers 1 to 3 (below) |
| Points "+" | (898, 10), 34 × 34 | image 81 (`GUI_main_01` (0, 0)–(33, 33)) in both states | shown with a pulsing glow while the hero has points to spend; a click opens `UI_WND_INVENTORY`; hidden at a new game (**seen**: absent in the frames) |

The bar shows no text of its own: no level, no numbers, no font; the only words are the tooltips (`cUI_Manager` tooltips, styles `0x209`, `0xB`, `0x8B`), which belong to the tooltip work.
There is no health or mana orb: Sacred has no mana, and the health is the ring of the portrait (below).
The left orb is the weapon set (the left button's attack), the right orb the selected combat art (the right button).

### The disc of day and night (`0x6E3E00`)

- The clock's minutes `t` (hours × 60 + minutes of `cCalendar`, `+0x20` and `+0x22`) give the angle θ = t · 0.004363323 radians (a turn a day, 0.25° a minute).
- One quad, corners (±27.5, ±27.5) with texture coordinates 0 to 1 over the whole 128 × 128 texture; a corner (x, y) is turned by θ and then drawn at (513 + 2(x cos θ − y sin θ), 710.5 + (y cos θ + x sin θ)): an ellipse of 110 × 55 whose picture turns clockwise on the screen as the day goes.
- `GUI_daynightdisc.TGA` (a ring of night sky with the moon at its top and day sky with the sun at its bottom) is drawn turned; then `GUI_daynightalpha.TGA` (a bronze ring of red runes open at the top) over it on the same ellipse unturned, so only the top of the turning sky shows through the opening.
- At 12:00 the sun stands at the top (**measured**: the three frames fit the clock's angle within 2° at 12:16, 12:24 and 14:32, the fog frame 331 s after the rain frame at 24 game seconds a second).
- Both are drawn with the filter linear (`0x643430(0x800, 1)`: magnification and minification `D3DTFG_LINEAR` on stages 0 and 1) and blended by their alpha (**hypothesis** for the blend, the default 2D state).
- `GUI_daynightglow.TGA` is loaded into `+0xAB0` and never read by the taskbar's code (a byte scan of `0x6DF590`–`0x6E6700`).

### The compass (`0x6E3F70`)

- Each frame it asks four map markers, `sdk_MARKER_RESOLVE(k, &x, &y, &colour)` for k = 0 to 3, each giving a direction (**hypothesis**: from the hero to the marker), and takes φ = atan2(x, y) + π/2.
- Marker 0 (red): the sword (image 101) turned by φ about its middle at (512, 710).
- Marker 1 (green): the pin 136 turned by φ at (512 + 30 sin φ, 710 − 15 cos φ), on an inner ellipse of 30 × 15.
- Marker 2 (blue): the pin 137 turned by φ at (512 + 58 sin φ, 710 − 29 cos φ), on the outer ellipse of 58 × 29.
- Marker 3 (white): blinks every 300 ms of real time between the outer pin 137 and the inner pin 136.
- All in the texture of image 136 (`GUI_main_01`), filter linear, vertex colour white.
- A marker newly set (the taskbar's message `0x34` with its number k) starts a "sonar" ring: `FX_SONAREFFECT.TGA` tinted `0xDDCCAA` at (512, 711), from radius r = 10 out to r + 40, r growing 600 pixels a second until 500, its alpha falling (**hypothesis** for the alpha's formula), with the sound event (9, `0x46`); four can run at once.

### The markers (`sdk_MARKER_RESOLVE`, `0x4A5980`)

Read 2026-10-30 (#1462), **confirmed** by code unless marked otherwise.

- The quest manager's log is a vector of entries of 0x174 bytes (`+0x424`, `+0x428`): `+0x0` the kind (3 a main quest, 4 a side quest), `+0x4` a state (2 as the quest is entered), `+0x8` the quest's number, `+0x10` and `+0x14` the compass's target, `+0x20` its radius.
  A quest's entry (`0x46C160`) appends one and makes it the player's newest main quest (`+0x3A4` by the player, a number from 1 to 99) or newest side quest (`+0x3A8`).
- The target: `0xEEEEEEEE` and a name's hash, an object not yet found, which the next call looks up by name and turns into `−1` and the object's id; `−1` and an object's id, the object's place each frame; else the cell `(+0x10, +0x14)` (`sdk_BUILD_POS`); `0, 0` none.
- Marker 0, the needle: the newest main quest's target, none for a number above 100 or a state from 100 (**hypothesis**: a quest finished).
- Marker 1, the inner pin: the newest side quest's target, none for a number below 100, from 9000 to 9499 or a state from 100; within the entry's radius of the hero the pin fades, its alpha `d + 255 − r` at the distance `d` (the integer root of the squares, 32,000 where either axis is beyond it), nothing nearer than `r − 255`.
- Marker 2, the outer pin: a point that the world map's window (`cUI_Megamap`, `0x6C8140`) stores at `+0x7674` through `0x4A63B0` (**hypothesis**: the player's own marker on the map); `0x4A63E0` reads it.
- Marker 3, the blinking pin: a creature that `GroupIsDead` (24, `0x48C860`) stores at `+0x7704` as it finds the group's nearest living member within 25,000 world units, for 60 s of the game's clock (`+0x7718`, `+0x771C`); a group found dead clears it.
- The direction is the target's place less the hero's, turned by 45° (`fcos`, `fsin` of π/4), the screen's axes of the isometric view.
  Where the hero and the target lie on different sides of a dungeon's mouth (`0x634840`, `0x6238C0`), it points at the way between them instead (`0x4A5420`, `0x4A4E20`, not read); quest 61 with three variables of its own takes a fixed place (`0x4790C66`).
- With its last argument set it teleports the hero to the marker instead (`sdk_ENGINE_TP`, its debug use from `0x615550`).

`QuestKompassPos` (63, `0x49A4B0`) and `QuestKompassObj` (64, `0x49AC80`) set a quest's compass:

- A name `rad:N` is the radius, 1,200 world units without one.
- `QuestKompassPos`: a name is a `DefPos` (`+0x334`, entries of 100 bytes) and a second one a variable whose value is the quest's number; its numbers are the cell's x and y where no name gave them, then the quest's number; raw coordinates (`0x20`) the cell; a cell of a negative coordinate is refused.
- `QuestKompassObj`: its name is the object's, and its quest the number.
- Without a quest's number, either takes the running function's quest (the manager's `+0x18`, read at `0x49A4D8` and `0x49ACA7`).
- On a quest in the log, the entry takes the compass, and where the quest is a main one, or the player's newest main or side quest, and its target changed, the taskbar gets the message `0x34`, the sonar.
- On a quest not in the log, or none (a record outside a quest's functions), the compass waits (`0xAB782C`) for the next quest entered, which takes it with the message `0x34` for a main quest; an entry clears it at its end, whether it entered a quest or not.
- In the Seraphim's tree: 3,330 such records, 2,083 positions (1,153 a `DefPos` and a quest, the rest numbers, some without the quest's, as quest 9's `OnEnter`) and 1,247 objects (1,112 with a quest, 135 of the running function's); no `rad:` name.

### The slots' drawing (`0x6E3430` right, `0x6E29E0` left, `0x6DFBF0` rings)

- A selected slot's ring is image 103 (gold), the others 102 (dark), on both sides; the left's selection is `0x5D9BA0(0)`, the right's `0x5D9BA0(1)`.
- An art or a combo in a right slot is drawn from its three images (the art record's `+0x30`, `+0x34`, `+0x38`, docs/re/arts.md; a combo's `0x4D2B30` record `+0x24` to `+0x2C`): `+0x30` grey (not ready), `+0x34` pale (ready), `+0x38` vivid (ready and selected); Attacke's are 618, 619, 620 (`GUI_spell10` (0, 128), (0, 64), (0, 0)).
- Ready and selected: the vivid image whole.
- Otherwise the icon is split across a line: the top `g = ⌊(1 − s)·64⌋` rows from the grey image, the rest from the pale one, `s` the share regenerated (`0x580D90`; a combo's regenerated time over its time, held at 1, `0x581A20`); so the pale picture fills up from the bottom as the art regenerates, and a ready art not selected is all pale.
- Riding (`0x54FC20`) with an art that may not be used on horseback, the slot shows the horse's images 632, 631, 630 (`GUI_spell10` (0, 192), (64, 192), (128, 192): orange, brown, grey) in place of the art's, `s` being `1 − [+0x4E8]/[+0x4EC]` of the hero (**hypothesis**: the horse's own timer).
- Each right slot then gets the burn (below), then its ring.
- A weapon slot draws its background, its ring, the two items' models (offset ±8 pixels when both hands hold one of `0x427A40`'s kind), then the damage icons.
  Read 2026-10-28 (#1347, **confirmed** by code and seen): each model goes through the grid's draw (`0x40E9C0`, the scale 1.0, its type's backpack's turn) at the slot's corner plus its type's offsets (`0x434480`): its `Weapon.pak` record's `+0x1C` and `+0x1D` (its cells across and down) times 16 less its signed bytes `+0x14` and `+0x15`, and 2 and 5.
  `0x427A40` holds for the category 5 (`items.pak +0x2E`), the weapons: the left hand's 8 pixels to the right and the right hand's 8 to the left.
  The left hand's is drawn first and the right hand's over it (seen: the Vampiress's sword over her shield), and a long weapon overhangs its orb (the Seraphim's sword from 690 to 755).
  The selected set's slot also keeps the set and the hands as one at each draw (`0x6E29E0`), unless the sets are locked (`0x5D8FD0`): with both hands empty (the hero's slots 12 and 13, `0x556F50`) it puts the set's items into them (`0x5D8970`), else it writes the hands' items into the set's halves where they differ (`0x5D8730`); #1348 follows it.

### The burn (`FX_BUTTONBURN.TGA`, `0x6E4CA0`)

- The taskbar's message `0x35` starts it (`+0xAB8` set, the start time kept).
  Its one sender is the Vampiress's change of form (`0x557040`, read 2026-10-30, #1464; found by a scan of the code that names `UI_WND_TASKBAR` for the message's id): her type set to 6 by day or 7 by night, her clips, her values and her arts' regenerations made anew, it posts `0x35` where she is the local player's hero, as her arts change with her form; its callers are her creature's event handler (`0x53079C`, `0x5307AA`) and `0x55D608`.
- 16 frames at 16 a second from the 4 × 4 grid of the 256 × 256 texture, frame f at (f mod 4, f div 4) · 64; after the 16th it stops.
- It is drawn over every visible art slot at once, after the icon and before the ring, blended by alpha (`0x643430(8, 0)` and `(0x10, 0)`: `SRCBLEND` `SRCALPHA`, `DESTBLEND` `INVSRCALPHA`).

### The buttons (`cUI_Button2`, `0x72B700` draw, `0x72B560` input)

- The taskbar's buttons are toggles: a left click inside plays the button's sound (its `+0x18`), sets the flag `0x10` and runs its first command (show the window, the UI event `0x11`), or clears it and runs the second (hide, `0x12`).
- Without `0x10` the button draws its `+0x64` image (the first row of `GUI_main_01`, lit); with it the `+0x54` image (the second row, darker, pressed in), so a button stays down while its window is open.
- No hover image; the hover shows the tooltip.
- The flags `0x20000` (1 s) and `0x20000000` (4 s) draw a sparkle: a glow of radius 30 cos(πt/2) and 20 random sparks; the potions' buttons get `0x20000` from the message `0x10`, which a potion used up sends with its belt kind (`0x5CAA60`, docs/re/inventory.md, §7), and an emptied slot passes it to its empty flask.
  The sparkle (read 2026-10-30, #1272, **confirmed** by code): its time `t` (`+0xB8`) grows by the frame's time, and the flag is cleared and `t` set to 0 once it reaches 1 (4 for `0x20000000`).
  Each frame `0x40B980` draws at the button's centre a square of `PARTICLE_SPARK04.TGA` of the half side 30 cos(πt/2), turned 2 (1 − t) radians clockwise and tinted `0xBB9360`, and over it one of `PARTICLE_SPARK07.TGA` of half that side tinted `0xBBBBBB`, both added onto the screen (`SRCBLEND` and `DESTBLEND` `ONE`, the flags 8 and `0x10` of `0x643430`); then 20 sparks the same, of the half side 9 sin(πt) turned a right angle, each at a point of the button's rectangle drawn at random every frame (`rand()`).
  The two textures are `cGranny`'s (`0x401030`, its `+0xBC` and `+0xC0`); the grid's sparkles are drawn the same way (docs/re/inventory.md, §7).
- The glow `0x732220(a)` (the "+" and the portrait's ring) draws the control again 49 times shifted by −6 to +6 pixels in steps of 2, added onto the screen (`DESTBLEND` `ONE`) at an alpha of ⌊12.75 a⌋/255 (16 times at alpha 16/255 on a display below 32 bits).

### Draw order (`0x6E3D10`)

1. The disc and its alpha ring.
2. The rail's pieces (`+0x248` list).
3. The right slots (icons, burn, rings).
4. The left slots (backgrounds, rings, item models, damage icons).
5. A control at `+0x160` (not read).
6. The children: the disc's housing (image 11), then the buttons.
7. The compass's needle, pins and sonar rings.
8. The belt and its clips, while `SHOWPOTIONS` is 1 and a hero exists (`0x6E3C30`).
9. The tooltips of the belt and the slots (`0x6E4560`).
10. The points' "+" (`+0x18C`) and, while its flag `0x400000` is set, its glow at a = (sin 6T + 1) · 0.3, T the real time in seconds.

### Input (`0x6E1BC0`, `0x6DFD60`)

- The bar takes the mouse over x 384 − 80n to 640 + 80n, y 676 to 768, so a click there does not reach the world.
- A click on a right slot selects it (`0x6E1A20`); while the combat arts' window has put the bar in its edit mode (the message 2), a click drops the art or combo the cursor holds into the slot, and a click on an empty or the selected slot calls `0x5D96C0` (not read).
  Read 2026-10-30 for #1090 (#1529, **confirmed** by code and **seen**): the slots are the inventory entry's records at `+0xC90` (docs/re/inventory.md, §1), the selection its `+0xD58`; `0x6E1A20` turns the rings (102 the old, 103 the new) and `0x5D9860` sends the engine the record's kind and id, which the right button then makes (`cEngine +0x58`, `+0x5A`, `0x619286`); the right button on a slot selects it, the left outside the edit mode too; in the edit mode, which follows the inventory (its show sends the message 2 with 1, its hide with 0), an art, spell or combo on the cursor goes into the slot, which is selected, the cursor emptied, without a sound, and with the cursor empty an empty or the selected slot is cleared (`0x5D96C0`), then selected; nothing comes back from a slot onto the cursor; the inventory's hide lets go of an art on the cursor.
- A click on a left slot selects that weapon set (`0x6E1690`); in the edit mode it puts the dragged item into the half clicked (`0x6E14C0`).
  Read 2026-10-28 (#1348, **confirmed** by code): a set of no slot shown, or one of no slot, chooses nothing; else the old slot takes the plain orb and ring and the new one the selected, the hero is sent the creature event `0x114` with the set and its halves' items (not read), the hands' items are taken off (`0x556A30` of the slots 12 and 13) unless the sets are locked, the set is selected (`0x5D9860`), and its half 0's item is put into the slot 12 and its half 1's into the slot 13 (`sdk_EQUIP`); where the class of what the hero holds changes (`0x546F80`), the hero is stopped (`0x54E6D0(1, 0)`).
  **Seen** the same day with the driver: the Seraphim at the level 2, the key 2 chose the empty second set and her slot 13 emptied, her sword gone from her hand; the key 1 put it back.
- Keys `1` to `5` select the weapon sets 1 to 5 and, while `COMBINE_SLOTS` is on (its default, absent from the installation's `Settings.cfg`), also the art slots 1 to 5; keys `6` to `9` and `0` select the art slots 1 to 5.
- Taskbar messages (UI event 5, kind `0x2B`): 1 sets `n`; 2 the edit mode; 3, 4 and 6 put a spell, a weapon art or a combo into the selected art slot; 5 equips a weapon (with sounds (9, 3), (9, 4), (9, 7) or (8, 3)); 7 with 1 hides the "+" and with 2 stops its glow (`0x581120`, `0x580E90` send it once no points remain; docs/re/character.md); 8 shows it; 9 a toggle of `0x5D8FB0`, the inventory entry's `+0x1162` bit 0 set by the value 0 and cleared by any other, which refuses every put into a weapon set (docs/re/inventory.md, §5); `0x10` the sparkle of a belt slot; kind `0x34` a compass marker's sonar; kind `0x35` the burn.
- The windows' own keys (I, M, Esc …) are not the taskbar's: not read here.

## The portraits' column (`cUI_Mercenary`)

Built by `0x6D6430` and `0x6D8250`: five entries of `0x194` bytes, the hero first and up to four followers (the hero's list `+0x39C`, those with a master `+0x251` or the bits 4 or `0x100` of `+0x1F4`), entry k at y0 = 16 + 132k.

| Element | Rectangle (screen) | Images | What it does |
|---|---|---|---|
| Ring, health | (932, y0), 96 × 108 | image 42 (`GUI_main_02` (96, 0)–(191, 107), grey) over the top `g` rows, image 41 ((0, 0)–(95, 107), red) below | health share f = `+0x4D8`/`+0x4D4`, held at 0; g = ⌊(108 − 11)(1 − f) + 11⌋, so the red drains from the top and the clasp's 11 rows are always the grey's; **measured** for the hero |
| Low health | the red part | the glow `0x732220` of the red part as drawn, at a = (sin 4T + 1) · 0.5, T the real time in seconds | while f < 0.25: called on the red control (`+0x250`) right after its draw and before the portrait's, so the portrait lies over it (read 2026-10-29, #1463, `0x6D73D0`; **seen**) |
| Hero's alarm | a ring about (975, 70) | `FX_SONAREFFECT.TGA` tinted `0xFF0000` at the alpha ⌊(1 − r/500) · 255⌋, from the radius r out to r + 40 in 90 straight pieces of 4°, the texture's u 0 at the outer edge and 1 at the inner and its v 0, blended by its alpha (`0x75FC70`); r from 10, drawn and then grown by 600 a second, the ring ended once r passes 500 | once as the hero's f falls below the warning level from at or above it at the frame before (`+0x938`); the level is `Settings.cfg`'s `WARNING_LEVEL` times 0.01, 0.25 without it (`0x81D2E0`, 25 in the installation) (read 2026-10-29, #1463, `0x6D70F0`, `0x6D73D0`; **seen**) |
| Portrait | (941, y0 + 18), 64 × 74 | by creature type (`0x435D30`): the heroes 1 Seraphim 1255, 2 Gladiator 1257, 3 Mage 1256, 4 Dark Elf 1260, 5 Wood Elf 1261, 6 and 7 Vampiress 1259 (`GUI_PORTRAIT00` (0, 0), (128, 0), (64, 0), (64, 71), (128, 71), (0, 71), 64 × 71, 1261 65 wide), 8 Dwarf 1438 and 9 Daemon 1437 (`GUI_PORTRAIT15` (192, 0), (128, 0)); followers by the same switch, which gives 373 types a picture and the others none (docs/02 §6.7) | the 64 × 71 picture stretched to 64 × 74 (**measured**); hover: tooltip of text 9960 with the health and its most, and a `<notify>` event `0x2D` to `UI_WND_MERCENARY`; right click on the hero's calls `0x6D60E0` (not read) |
| Bonuses "+" (hero only) | (1000, 34), 16 × 16 | image 110 (`GUI_main_01` (240, 0)–(255, 15)) | hover: a tooltip at (16, 16) listing the hero's bonuses (`0x55E5E0`, the list at `0xAD4218`); **measured** |
| Experience bar (hero only) | frame (940, y0 + 106) 73 × 10; fill (940, y0 + 108) | frame image 66 (`GUI_main_02` (74, 123)–(146, 132), dim green beads); fill image 67 ((0, 119)–(73, 125), bright green) | fill width ⌊73 · e⌋, e the experience into the level over the level's span (`0x57F7B0`, `0x57F6E0`, docs/re/fight.md), its texture cut to match, so it grows from the left; hover: text 9961 with the two numbers; the message (5, `0x29`, 2) for a creature flashes it with the glow from 1 fading over a second; **measured** (empty at level 1) |
| Mount's bar | frame (940, y0 + 117) 72 × 15; fill the same | image 77 ((75, 160)–(146, 174), blue horseshoes), fill image 78 ((75, 145)–(146, 159), gold horseshoes) cut as the experience | while the creature has a horse (`0x55C630`): the horse's health share; hover: the horse's description (**hypothesis** that 77 is the frame) |
| Horse's button | (986, y0 + 70), 34 × 34 | image 365 (`GUI_main_15` (33, 216)–(66, 249), a horseshoe) | while riding a hero's horse: left click mounts within 250 (creature event 7) or calls the horse (`0x116`); its tooltip 200 pixels right of it |

The ring is drawn before the portrait: its middle is a glass of grey 204 at the alpha 85, and over the portrait it would pale the face by about a third toward grey, which the frames do not show (**measured** 2026-10-27: the face's mean colour 172, 155, 137 in the frames, the portrait's own; 182, 170, 158 with the ring over it).
The portrait is blended by its alpha (its background is black at the alpha 0 to 0.47), so the world shows through about the head (**seen** in the rain's frame).

The other images of `GUI_main_02` beside them (yellow beads 65, red frame 68 and beads 69) are not used by these functions.

## The tooltip (`cUI_Tooltip`)

Read 2026-10-30 (#1273), **confirmed** by code unless marked otherwise; the item's lines are docs/re/inventory.md, §9.

- The manager's tooltip (`UI_WND_TOOLTIP`) is shown by `0x75B390(x, y, text, item, style, raise)`: it takes the style's bits (`0x6E8860`), the corner `(x, y)` (`+0x24`, `+0x28`), the text (`0x6E6EA0`) and the item's id (`0x6E81C0`, `+0x21D0`), whose sockets it draws under the text (`0x5C0A70`).
- The style's bits (`+0x154`): 1 the tooltip goes as the control under the cursor changes (`0xCD7A50`); 2 `(x, y)` is its top left, else its centre; 8 it is kept 16 pixels within the 1024 × 768 screen; `0x20` it stands at the screen's centre.
  The grid's items take the style `0xB` (`0x6C1480`).
- Its layout (`0x6E7AF0`): its width the text's, measured in its font (`+0x158`), and 8; its height the lines times the font's height of `A` and 4; with sockets, 40 more high and at least 40 a socket and 16 wide; an image of its own (`+0x21DC`) widens it by the image's width and 16.
- Its draw (`0x6E76C0`): the window's panel and border (`cUI_Window2_render`, `0x7277B0`), then the text from 4 pixels in.
- Its panel is black at the alpha of a half (**measured** 2026-10-30 on the original's frames with the tooltip and without, `with ≈ 0.5 · without` in each channel).
- Its border is eight `UI_STATIC_BORDER`s made once (`0x727B30`), for a window of `W × H`:
  - the image 21 (`gui_main_03` (224, 0), 32 × 6) along the top from (4, −4) and the bottom from (4, H − 3), `W − 4` long;
  - the image 34 (`gui_main_03` (120, 112), 6 × 32) down the left at −4 and the right at `W`, `H` long;
  - the image 55 (`gui_main_02` (114, 180), 38 × 18) at the corners: the top right's at (W − 24, −11) as stored, the top left's at (−13, −11) turned about across, the bottom right's at (W − 24, H − 8) turned about along, the bottom left's at (−13, H − 8) both.
- **Measured** on a potion's tooltip at 1024 × 768: its lines 20 pixels apart, its name's letters 165 pixels wide and 16 high from 4 pixels in.

## What changes with the hero's state

- Level: the slots a side (1, 2, 3, 4, 5 at 1, 2, 8, 16, 30) and with them the rail's length and the bar's mouse area.
- Health: the ring's red share, the glow below a quarter, the red alarm once below the warning level.
- Experience: the bar under the portrait.
- The clock: the disc's angle.
- Markers: the needle and the pins; a new marker's sonar.
- Arts: each art slot's split by its regeneration, the vivid picture for the selected ready art, the burn on the message `0x35`.
- Riding, the Vampiress's night form, points to spend, potions on the belt, the selected weapon set and art.

## Implementing it: an order

1. The static frame at the start of a game, which is most of the look: the rail (ends 5 and 8, middle pieces chosen once), the housing 11, the five buttons in their lit images, the belt's flasks, the two slot orbs with their rings, the portrait's ring, portrait, "+" and experience frame.
   Every place above was measured on the frames, so a screenshot beside the original's should match within a pixel.
2. The disc: two textured quads on the ellipse, the clock's angle; then the health ring's split and the experience fill.
3. The art slot's icon from the selected art's three images and its regeneration split; the weapon slot's selected background and the damage icon (a 3D model of the held weapon can wait: draw nothing or the item's flat icon).
4. Input: the bar's mouse area (keep clicks off the world), the toggles' down images, keys 1–0 and slot clicks once the arts and weapon sets have a selection in `opensacred-game`.
5. Later, with their windows: the inventory, options, map and book buttons open #1088, the Esc menu, #1091's map and quest log; the belt's potions and their use with #1088 or #1090; the right slots' edit mode and drag and drop with #1090 (the combat arts' window); the points "+" with #1089 (the character sheet); the tooltips, the compass's markers (quests, #1091), the burn, the sonars, the followers' portraits and the mount's bar when followers and horses come.

Done by 2026-10-28: items 1 to 3, the sets' keys and slots of item 4 (#1348), and item 5's inventory button (#1267) and belt (#1311).

## Open questions

- What the log entry's state `+0x4` holds after the quest's end, which hides its markers from 100.
- The ways between a dungeon and the world that the markers take across its mouth (`0x4A5420`, `0x4A4E20`).
- The right rail's middle piece in the frames matches neither image 9 nor 10 well at x 570 (its chain's knot at 641–660 fits image 9; the wavy line left of it does not show).
- `0x6D60E0`; the control at `+0x160`.
- How the bar scales at the original's other resolutions (the windows' rectangles are fixed at 1024 × 768 here).
- Whether the compass's sonars fade with the radius as the hero's alarm does, ⌊(1 − r/500) · 255⌋.

## In `opensacred-game`

`bar` lays the bar and the hero's entry of the column out from the interface's images' table (`tables/ui_images.ron`, #1160) at the places above, in the interface's units of the 768-pixel-high screen (`hud`, `UiScale`), the bar centred along the bottom of a wider window and the column at its right edge (#1087):

- the disc of day and night turned by the clock, its rings sampled without their smaller levels so that the runes stay sharp as the ellipse shrinks them (the original's textures have one level);
- the rail's ends and middle pieces for the slots a side, its middle pieces chosen at random once as the slots are made;
- the slots, 1 to 5 a side by the hero's level: the weapon sets' at the left, the first selected, and the arts' at the right, the first holding the selected art (`making::Selected`) split by its regeneration, vivid when ready;
- the housing, the five buttons in their lit images, and the belt's five empty flasks;
- the health's ring split by the hero's health, the portrait by its type over the ring, the bonuses' "+", and the experience's bar filled by the experience into the level;
- clicks over the bar's area (x 384 − 80n to 640 + 80n, y from 676) and over the hero's portrait do not reach the world, the set `frame::Control` held back there; the pieces are the interface under the cursor, which shows the arrow (`cursors`).

The belt's flasks are lit by the potions of their kinds since #1311 (`belt`).
The left slots draw their weapon sets' items since #1347 (`sets`), as above: each set's two models, the right hand's over the left's, by a camera of their own into a picture over the bar.
Since #1348 a click on a left slot, the cursor empty, or the keys 1 to 5 choose its set: the slot takes the selected orb and ring, the hero's hands its items, its arms made anew with them (`arsenal::Maker`: their models, the clips of their class, its values and its weapons' sounds), and a change of their class stops the hero; the event `0x114` and the keys' choice of the art slots (`COMBINE_SLOTS`) not followed.
Since #1349 the inventory open is the left slots' edit mode: a click takes an item out of a set or puts the held one in, and the halves that would take the held item glow under the ring (`sets`, docs/re/inventory.md, §5); the hero's skill of two weapons, which lets a second one-handed weapon into the left hand, not followed (none of the templates has it).
Since #1408 a click on the Collect button sends the hero collecting the items about it (`pickup`, docs/re/items.md, "The automatic pick-up").
Since #1463 the red part of the ring glows while the hero's health is below a quarter of its most, and the red alarm spreads about the portrait once as it falls below the warning level (`low_health`): the glow is a material of the interface that adds the 49 copies of the part as drawn onto the frame, 6 pixels about it and under the portrait, and the alarm one that lays the texture's row across a ring, a circle rather than the original's 90 pieces (a third of a pixel apart at most, at the radius 540); the level is the settings' `[interface] warning_level` (docs/10), 25 as the installation's.
Since #1464 the art slots burn as the Vampiress's form changes (`art_burn`): the 16 frames of `FX_BUTTONBURN` at 16 a second of real time over every visible art slot, over its icon and under its ring.
**Seen** in the game: her changeling 1022 at the level 16 burnt her four art slots.
Since #1465 the portraits come from the column's own table (`portraits.ron`'s `column`, docs/02 §6.7), and up to four of the hero's followers (`followers`) have their entries below the hero's, 132 pixels apart: each its ring split by its health as the hero's and its portrait by its type, none for a type without one.
Those shown keep their places as others come, the newcomers after them, and a follower's entry goes as it dies or is let go; clicks over the entries do not reach the world.
**Seen** in the game: the Vampiress's wolf (588) called at her level 16 took the entry under hers, its ring grey down to its 80th row at 60 of its 214, and a second call's wolf took the first's entry as the first went.
Not followed for them: the order of the hero's list `+0x39C` (not read), their rings' glow, the tooltip and the mount's bar.
Since #1272 a potion used up sparkles on its kind's slot of the belt as above (`sparkles`), its sparks drawn anew every frame by the game's own dice.
Since #1522 the points' "+" shows and glows after a level-up, its glow stopped by a point spent in the statistics window and hidden once no points remain, and a click on it opens the inventory (`points`, docs/re/character.md): its glow is the ring's material at a = (sin 6T + 1) · 0.3, and it keeps at (898, 10) by the column, left of it at the right edge of a wider window, where the original has one screen.
Since #1273 the grid's items have their tooltips in the window above (`tooltip`): the panel, the border's eight pieces, the text 4 pixels in from its corner at the item's right edge and 16 and its top and 16, kept 16 pixels within the screen, its font at 20 pixels so that a line's letters are as wide as the original's (a potion's name 167 pixels to 165; its letters 3 pixels higher), the lines 20 apart.

Since #1529 the art slots hold their arts (`art_slots`): a new game's first holds the hero's first art, the selected one's art is the right button's, a click selects a slot or, in the edit mode while the inventory shows, takes the art on the cursor or clears the slot, the keys 6 to 9 and 0, and 1 to 5 with the weapon sets, select the slots; each slot draws its art split by its regeneration, an empty one its orb (479 selected, 635 not, 533 while the cursor holds a spell or, in the edit mode, a weapon art), its ring 103 selected and 102 not; an art on the cursor shows its vivid picture there.
**Seen** in the game beside the original's frames: 1070 taken from the spells page with every empty slot green, dropped on the second slot, which took the gold ring; the selected slot cleared to its pale blue orb; the key 6 selecting the first.
Since #1530 a slot holds a combo too, from the combos page (`combos_page`, docs/re/arts.md): its images 801 to 812 split by its share regenerated, no slot green while the cursor holds one; a slot that holds a combo runs it with the right button since #1531 (`combos`, docs/re/arts.md, "A combo's run").
Since #1536 the keys F1 to F6 put the arts bound to them into the selected slot (`spells_page`, docs/re/arts.md, "The spells page").
Not followed for them: the Vampiress's second bank and a slot's tooltip.

Not followed yet: the other buttons' windows and the pressed images, the damage icons in the left slots, the glows, the sparkle of the flag `0x20000000`, the tooltips and the mount's bar.

Since #1462 the scripts keep the quests' compasses (`opensacred_rules::quests`: `Compass`, `Target`, `Quests::set_compass`, `Quests::marker`, `side_alpha`): `QuestKompassPos` and `QuestKompassObj` as above, of the running function's quest without a number, a compass waiting for the next quest entered, the newest main and side quest, and the new markers for the sonar (`Quests::take_fresh`).
`compass` draws them on the bar: the needle (image 101) turned about (512, 710) toward the newest main quest's target, the inner pin (image 136) on its ellipse toward the newest side quest's, fading within its radius, the angle φ clockwise from up of the target's place less the hero's as the camera shows them; a cell's target is its centre, a named one the creature of that name in the scripts (`npcs::ScriptName`) where it stands.
A new marker starts one of four sonar rings at (512, 711), `FX_SONAREFFECT` tinted `0xDDCCAA` as the hero's alarm spreads (`low_health::Sonar`), with the sound 7027.
**Seen** in the game: at the Seraphim's start the needle points at the novice as she walks to the benches, and turns as the hero moves; `quest trigger 9` turned it at quest 9's target with a sonar.
Since #1569 the outer pin (image 137 on its ellipse of 58 × 29) points at the world map's pin (docs/re/maps.md, "The pin").
Not followed: the blinking pin of `GroupIsDead`'s creature, objects that only a `CreateObj` names, and the way across a dungeon's mouth.
