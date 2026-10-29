# The cursor's looks

Recovered from the GOG `Sacred.exe` 2.28 with Ghidra and in the running original under the driver of #448 on 2026-10-27 (#1147, a subagent of the session).
Status: **confirmed** by code unless marked otherwise; **seen** where the running original showed it (probes of the chooser and screenshots, which include the cursor; the frames stay in the scratchpad).

## Where the cursor comes from

- `cMouse` (the singleton `0x654F60`) keeps the cursor's position (`+0x04`, `+0x08`, the client's pixel of the 1024 × 768 screen), a stack of cursors (a deque at `+0x3C` to `+0x58`, its top at `+0x64`; the debug line's `MOUSESTACK`), the item carried on the cursor (`+0x68`) and the id of the object under the cursor (`+0x6C`, which the chooser writes at each frame and the labels over the objects read).
- An entry of the stack (`0x6546E0`, 0x2C bytes) holds flags (bit 0 the glow, bit 1 an item's image), an offset of the drawing (`+0x02`, `+0x04`), a width and a height where given (`+0x06`, `+0x08`), the texture (`+0x0C`) and its rectangle of texture coordinates (0, 0, 1, 1); `0x655060` pushes one, `0x655180` pops one.
- `initApp` (`0x8173FE`) pushes `MOUSE_MAIN.TGA` with no offset: the bottom of the stack, the cursor of the menus and the world's default.
  The interface's thread (`cUI_Manager_runThread`, `0x757227`) draws the top of the stack while no world is drawn.
- In the world the render thread (`cEngine_renderThreadRun`, `0x60E9C7` and `0x60EA01`) calls `cEngine::updateWorldCursor` (`0x611E70`) twice a frame: first without drawing (it picks the object, sets the hover's flags and posts the enemy's panel), then at the frame's end with drawing (the item's label and the cursor); **seen**: a probe of the chooser fired twice each frame.
- A look is a texture found by its name in `texture.pak` (`0x6569C0`, `0x657360`); the chooser pushes it with no offset for the frame, draws (`cMouse_renderCursor`, `0x655450`) and pops it, and without one draws the stack's top.

## The chooser's tests, in order (`0x611E70`)

1. It clears `cMouse +0x6C`; over an interface's window (`cUI_Manager_isCursorOverUi`, `0x75AA90`) or without the local hero (`0x603E30`) it does nothing more, so the stack's top shows (**seen**: over the bottom panel nothing was chosen).
2. The view turns the cursor into a point of the world (its virtual `+0x18`).
3. While the cursor carries an item (`0x655560(1)`) it does nothing more: the item's image, which `0x655580` pushed with the flag 2, stays the cursor.
4. **The pick** (`0x626C50`): every object drawn in the last frame left its screen rectangle in a list of the view (`+0x96B24`, entries of 28 bytes: the id, the rectangle and a second one for its label; filled as `cObject3D`s and `cObjectFX`s are drawn, `0x62E410`, and swapped in at the frame's end, `0x62FF60`).
   The rectangles under the cursor are sorted into four classes by the object's query 1 (its virtual `+0x24`): 0 a living creature hostile to the local player (`cCreature`, `0x55D3F2`, by `0x549000`), 2 an item (`cItemBase`, `0x5C70E0`) and every entry whose id has bit 31, 3 the rest (`cObject3D`'s default `0x450930`: the peaceful and the dead creatures, the chests, the doors).
   The first class that is not empty in the order 0, 1, 2, 3 (the table `0x95F934`), or 2, 1, 0, 3 while Alt is held (`0x95F944`), wins, and in it the rectangle whose centre is nearest the cursor; more than 1,000 rectangles pick nothing.
   With nothing picked and Alt held, the rectangles of the items' labels are tried (`0x6270A0`), so that the labels that Alt shows over the items on the ground (`cWorldView_drawOverheadLabels`, `0x627180`) can be clicked.
5. A picked id with bit 31: **the hand** (`mouse_action.tga`) at once (what makes such ids is not read).
6. An object: its id goes to `cMouse +0x6C`, then:
   - **A horse** (its creature record's `+0x04` is 4, `0x4266F0`) that the hero does not take for a foe: the horses' looks, below.
   - **A creature** (the category 2, 3 or `0x20`, `0x426610`), considered only where the hero stands on the ground or the first floor (its byte `+0x24` below 2) or on the creature's floor; where the local player's object is a horse with a rider (`+0x1EC`), the rider stands for the hero in these tests (**hypothesis** for the meaning).
     - In a network game (`0x182EBEC`) with Shift held over another player's hero (`+0x200` bit `0x40`) hostile neither way (`0x548FB0`): the hover's highlight alone.
     - **A living foe** (the hero takes it for one, `0x548F60`: its AI's state not 9, its action not 6, its health not 0, the sides hostile; and alive, `0x549080`): a horse whose rider (`+0x1EC`) is not the hero gives way to its rider; the enemy's panel goes to the interface (`0x75C760`) while `Settings.cfg`'s `SHOW_ENEMYINFO` is on (`0x81CDD0`; **seen**: the level, the name and the kinds' icons in a box at the bottom's centre); its float `+0x20C` is raised to at least 1.0 (**hypothesis**: its health's bar shown); it takes the hover's highlight; then the look by the hero's weapons in its two hands (`+0x1D4`, `+0x1D8`):
       a weapon of the kind 13 or 14 (`Weapon.pak +0x1E`; 14 the muskets, 13 the Dwarf's cannons) in either hand (`0x54A340`, `0x428000`) **`MOUSE_GUN.TGA`** (a pistol);
       else a weapon of the kind 7, 12, 13 or 14 (the bows and the crossbows) in either hand (`0x54A2A0`), the hero not in the Vampiress's night's form (`+0x14` bit `0x2000`), **`MOUSE_ATTACKA.TGA`** (a bow);
       else **`MOUSE_ATTACK.TGA`** (a sword).
       `MOUSE_MAGIC.TGA` is looked up while the engine's art of the right button (`+0x5A`) is below 14, but its handle is dropped (`0x6125BA` to `0x6125C6`): it never shows (**seen**: no lookup with the Seraphim's 1071 selected).
     - **Any other creature** (peaceful, dead, or no hero), by its `+0x200`, with no highlight: bit `0x2000` (the merchants) **`MOUSE_TRADER.TGA`** (a bag of money); `0x4000` (the combat masters) **`MOUSE_COMBO.TGA`**; `0x1000` (the smiths) **`MOUSE_BLACKSMITH.TGA`** (a hammer); `0x4000000` (the horse trader, `CreateNPC`'s `0x64`) **`MOUSE_HORSETRADER.TGA`**; else **`MOUSE_DIALOG04.TGA`** (a speech bubble) where `0x549920` answers above 0, the creature is in no talk (`+0x200` bit `0x10`), has no `+0x1F4` bit `0x40000` and its id is above 16 (no player's hero); else the default.
       `0x549920` answers the creature's dialogue (`+0x245`) where it has one; for a townsperson without one (the sides 3, 7, 8, 12, 14 and 15, a mode of `0x1006DCF8`, alive, hostile neither way) the dialogue of the scripts' table by its region and type (`0xAB44E8`), or in the regions 50 to 69 of its sector (`0x602290`) a line of chatter at random (`NPCUWR_*`, the answer `0xD431`; which regions those are is not read).
   - **An item that can be picked up** (`0x427920`: the categories 5 to 9, 13 to 25, 28, 29 and 33, the type's word without bit 22): as it draws, the item's name beside the cursor, 24 pixels right and 16 down, and **the hand**.
     An item that is a `cObject3D` (`0x612728`) goes by `0x44A250`: the text of its key (`+0x3C`), which a script's text name or the item maker's name sets (docs/re/items.md, "The made item's name"), where it has a key, else its type's name (`items.pak`'s key at `+0x20`, `0x428830`), in the colour of its query 2 (`cItemBase`'s `0x5C70C0` → `0x5C13C0`, the item's colour, #1273).
     Another goes by its type's name, white, red for an object of `+0x14` bit `0x20`.
     The scripts' field reader keys a name of the prefix `res:` alone (`0x4737CA`): the text's key for a name, the number for one of digits, 0 for any other name; `CreateObj`'s object keeps its first `0x01`'s key, so that an item laid by a `res:` name goes by that name's text, empty where the texts lack it (`0x672C90`), and one of another name, `NON_UNIQUE` among them, by its type's (**confirmed** by code 2026-10-28, #1360).
   - **An object of a type whose first byte has bit 5** (`0x4263C0`; the items table's kind 34, the `.grn` objects: the chests, the barrels, the doors, the shrines): **the hand** and the hover's highlight, but not for a chest (the category 4, `0x428440`) whose object has `+0x14` bit `0x4000`, emptied (**confirmed**, docs/re/containers.md; a town stash's too while its window shows).
   - Any other object: the default.
7. Nothing picked: the cell under the cursor on the hero's floor (`0x634C50`):
   - a cell of `+0x1E` bit `0x80` (set as the game runs; **hypothesis**: the scripts' spots of `TRIGGER_INSERT_MOUSEOVER`) is looked up in the script manager's table (`0xAACF80`, `0x4A6770`, records of 0x58 bytes at `0xAB45B8`): its image is drawn beside the cursor and, where its text (`+0x18`) is not empty, **the hand**;
   - else the walkable cell under the cursor (`0x616AD0`, docs/re/movement.md) of the low nibble 3 (a teleport) or 5 to 8 (the stairs), or one that the map `0xAB4498` holds: **the hand**;
   - else the default.
8. **The glow**, whatever was chosen: the engine's art of the right button (`+0x5A`, `0x51D830`) and its launch type (a spell's record `0x4D2310 +0x3C`, a weapon art's `0x4D1AF0 +0x4C`); the target is the interface's chosen object (`0x75B360`) if any, else the cell under the cursor and the creature picked; where the art is ready (`0x580DF0` on the hero's stats `+0x3A8`), `0x51D840` with the target and the launch type sets the glow.
   **Seen** with the Seraphim's 1071 (a weapon art on an enemy): no glow over a goblin on the ground while the hero stood on the monastery's first floor, the glow over the goblins outside on the hero's level, none once `arts use 1071` started its regeneration.

### The horses' looks

**Confirmed** by code, not seen (no horse near the start).
For a horse that the hero does not take for a foe, its owner the creature of the id `+0x570`:

- It may be ridden (`0x4267D0`) unless the hero is the Dwarf (the type 8), where the hero's skill 22 (`0x582450`; **hypothesis**: riding) reaches what the horse's query `0xD` asks and the horse's `+0x1F4` has none of the bits `0xF0`.
- Where the owner is none or no player's hero (`0x426650`) and the horse's query 5 is not 0, a text (`0x55C9F0`; **hypothesis**: its price) is drawn 32 pixels above the cursor.
- Not to be ridden: **`MOUSE_STEALHORSEFAIL.TGA`** where its owner is no horse trader (`+0x200` bit `0x4000000` clear) in a game of one player, else **`MOUSE_CANTRIDE.TGA`**.
- To be ridden: the horse trader's horse **`MOUSE_HORSETRADER.TGA`**; another's horse (an owner other than the local player) **`MOUSE_STEALHORSE.TGA`** (the Dwarf `MOUSE_CANTRIDE.TGA`); a horse of no owner or the hero's own **`MOUSE_RIDE.TGA`** where its query 5 is not 0, but while the hero rides (`0x54FC20`: `+0x14` bit 2 and a mount) only with Shift held; else the default.
- A horse that the hero takes for a foe is a creature as any other.

## The textures

All are 64 × 64 RGBA in `texture.pak`, the picture in the top left with its point at (0, 0), but for the map's.

| Texture | Picture | Shown |
|---|---|---|
| `MOUSE_MAIN` | the ornate arrow, its tip at (0, 0), within 32 × 32 | the default, the menus |
| `mouse_action` | an open hand, the fingertips at the top left | the items, the chests, the doors, the other objects of the kind 34, the teleports, the stairs, the scripts' spots |
| `MOUSE_ATTACK` | a sword, its tip at (0, 0) | a foe, in melee |
| `MOUSE_ATTACKA` | a bow and an arrow | a foe, a bow or a crossbow in hand |
| `MOUSE_GUN` | a flintlock pistol, its muzzle at the top left | a foe, a musket or a cannon in hand |
| `MOUSE_MAGIC` | a casting hand | never (its handle dropped) |
| `MOUSE_DIALOG04` | a speech bubble | a creature of a dialogue or of chatter |
| `MOUSE_TRADER` | a bag of money | the merchants (`0x2000`) |
| `MOUSE_BLACKSMITH` | a hammer | the smiths (`0x1000`) |
| `MOUSE_COMBO` | a figure | the combat masters (`0x4000`) |
| `MOUSE_HORSETRADER` | a horseshoe and a bag | the horse trader and his horses |
| `MOUSE_RIDE`, `MOUSE_STEALHORSE`, `MOUSE_CANTRIDE`, `MOUSE_STEALHORSEFAIL` | a horseshoe, with a hand, crossed, or both | the horses |
| `MOUSE_DIRN` to `MOUSE_DIRNW` | the arrow pointing eight ways, up to 43 × 43 | the world map's edges |
| `MOUSE_PUSHPIN` | a blue pushpin | the world map's marker |

No string or format of the executable names `mouse_dialog01` to `03`, `mouse_sell`, `mouse_stairs`, `mouse_stair_1`, `mouse_stair_2`, `mouse_windows` or `mouse_dwarf_no_ride`.
`GUI_SPELL_MOUSE.TGA` is no cursor but the icon of the changeling's mouse's form (docs/re/arts.md, the effect 36).
`MOUSE_TRADER`, `MOUSE_BLACKSMITH`, `MOUSE_COMBO` and `mouse_horsetrader` are also the world map's icons of those creatures (`0x6C8A40`, `0x6CCBD0`).

## The point, the size, the tint and the buttons

- **The point** is the texture's top left pixel for every look of the world and for `MOUSE_MAIN` (pushed with the offset 0, 0; `0x6548A0` draws the quad from the cursor plus the offset); **seen**: the arrow's tip, the sword's tip and the hand's and the bubble's top left corners at the probed pixel, within a pixel.
  The map's cursors carry offsets (`cUI_Megamap`, `0x6C7540`) so that each arrow's point is the cursor: `DIRNW` (0, 0), `DIRN` (−16, 0), `DIRNE` (−32, 0), `DIRW` (0, −16), `DIRE` (−40, −16), `DIRSW` (0, −32), `DIRS` (−16, −40), `DIRSE` (−32, −32), `PUSHPIN` (−16, −16).
- **The size**: a quad of 64 × 64 at 1:1 on the 1024 × 768 screen (`cMouse_renderCursor` passes 64, 64), the pictures in its top left 32 × 32 or less.
- **No animation**: the chooser and the drawing read no clock, and every look is one texture.
- **No tint** (`0xFFFFFFFF`) but for the glow: with the entry's bit 0 (the selected art of the right button ready and admitting what lies under the cursor, nothing carried) the picture is drawn 49 times more at the offsets −6 to +6 by 2 on both axes, white at the alpha 8/255 and added (the destination's blend `ONE`, `0x643470`'s state `0x10`): a soft white halo of about 6 pixels (**seen**).
- **The buttons**: the chooser reads none; **seen**: a left button held over a foe left the cursor's pixels as they were.
  Shift matters only for a horse while riding and for another player's hero; Alt changes the pick's order (the items first) and shows the items' labels.
- **The hover's highlight**: a foe, a ridden foe's horse and an object of the hand take `+0x14` bit `0x20000000`, which the world's drawing reads (`0x62FDE5`); **seen**: a hovered chest's front shaded a little otherwise (the effect not read); the peaceful creatures and the items take none.
- **The world map**: `cUI_Megamap` (`0x6C7920`) pushes the arrow of the screen's edge that the cursor touches (x at 0 or 1023, y at 0 or 767, the corners the diagonals), and the map scrolls that way; its pin's button (`0x6C8140`) pushes `MOUSE_PUSHPIN` until a marker is placed (**seen**).

## Seen in the running original

A Seraphim's new game, `god on`, the chooser probed (2026-10-27):

| Under the cursor | Look |
|---|---|
| the main menu, the floor | `MOUSE_MAIN`, its tip at the pixel |
| the novice of a dialogue | `MOUSE_DIALOG04` |
| a bow dropped with `item 1732` | `mouse_action`, the name beside it |
| a goblin warrior made hostile | `MOUSE_ATTACK`, the enemy's panel |
| the merchant of 50,40 (`+0x200` `0x2000`) | `MOUSE_TRADER` |
| a goblin, a bow worn (`wear 1732`) | `MOUSE_ATTACKA` |
| a goblin, a musket worn too (`wear 1849`) | `MOUSE_GUN` |
| a chest, the merchant's open door | `mouse_action` |
| the bottom panel | nothing chosen |
| the world map's top edge, its pin | `MOUSE_DIRN`, `MOUSE_PUSHPIN` |

## In `opensacred-game`

`cursors` chooses the look at each frame of a game (#1147), and `hud` puts its picture on the window as the system's cursor, the top left 32 × 32 of its texture scaled in whole steps with the display's scale, its point at the corner.
What lies under the cursor is taken as a left click takes it (`attack::from_line`, the creatures, items and doors nearest the cursor's line of sight), so that the look tells what a click does: a living foe of no dialogue the sword, the bow or the pistol by the hero's hands (`fight::Holds::ranged` and `gun`, the Vampiress's night's form the sword); a creature of a dialogue or a trader its sign by its trades, else the bubble; an item lying on the ground the hand; a door the hand; the stairs of the cell that a click sends the hero to (`pick`, `0x616AD0`) the hand; else the arrow, and the arrow outside a game, while a dialogue's window shows and over the interface.
The order of the classes follows the clicks (a creature of a dialogue before an item, as `talk` takes it first), not the pick's classes, and no Alt changes the look; since #1409 Alt held puts an item, or its label, before a foe for the click (`attack::aim`, `labels`, docs/re/items.md).
The overlay and `state` tell the look (`cursor`).
Beside the hand over an item, its name of the game's texts by its type shows 24 of the original's pixels right of the cursor and 16 down, in white, in the game's font at 14 (the original's font for it is not read; its full name with the item's modifiers comes with the items' names of the inventory, #1088).
An item that a `CreateObj` of a `res:` name laid goes by that name's text instead, and by none where the texts lack it (`npcs::TextName`, #1360), as the Seraphim's quest 15001's plant does.

The hand shows over a container that opens, not over one emptied (#1380, docs/re/containers.md).
Not followed yet: the name's colour (the item's, #1273), the hand over the teleports and the scripts' spots, the glow of an art that may be made, the enemy's panel and the hover's highlight (#1087), the horses' looks (no horses are ridden yet) and the map's arrows (#1091).
