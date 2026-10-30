# The character's sheet: the statistics window and the level's points

Read 2026-10-30 (#1089, #1521) in the GOG `Sacred.exe` 2.28, **confirmed** by code unless marked otherwise; the places are pixels of the 1024 × 768 screen, or of the window where it says so, and "image N" is the entry N of the interface's images' table (`0x9ECEB0`, docs/02 §6.7).
The values are docs/re/fight.md's; the points that each level gives are its "The hero's levels".

## The window (`cUI_StatisticsChar`, `UI_WND_STATS_CHAR`)

- The manager makes it among the game's windows (`0x759DE0`), and the inventory's show shows it with the inventory and the equipment (`0x6C5A50`); its constructor (`0x6A5D70`) puts it at (656, 0), 256 × 388; its input is `0x6A6110`, its draw `0x6A9F20`, its show `0x6A5FA0`, which leaves the learning mode (`+0xF84`).
- It shows the creature whose portrait in the column is under the mouse (`0x75B340`, `0x6D65D0`), else the local hero (`0x603E30`), its stats at `+0x3A8`.
- Its controls (`0x6AC470`):
  - `UI_STAT_BTN_BASE` at (242, 10), image 132 (34 × 33, a round frame with a cross), which closes the inventory's windows (`cCommand_onInvClose`, `0x756870`) and sparkles for a second at a level-up (`0x57F023`);
  - the header bar at (9, 49), 203 × 21, a toggle: image 1605 (the skills) while its flag `0x10` is clear, image 1604 (the attributes, its two gems swapped) while it is set;
  - eight "+" buttons at (232, 72 + 19i), 16 × 16, image 110 (pressed 111, which never shows: the window tests the clicks itself);
  - two backgrounds tinted `0x9FFFFFFF`: image 376 at (0, 20) stretched to 256 × 286 and image 380 at (0, 306), 256 × 256;
  - its frame: along the top eight pieces from x −4, 32 apart, image 54 at y 11, image 21 at y 17 for the six between and image 55 at y 10; down the sides ten rows of image 34 from y 28 at x −4 and 255, and at y 348 image 52 at the left and image 53 at x 251; no bottom edge.
- Its draw, the window's colour white, its font the interface's first:
  - the name, green `0xFF00EF00`, at (8, 24): the creature's key `+0x3C`, else its type's (`sdk_REFKEY`); text 1070 right in (8, 24, 188), the level `+0x56` right in (8, 24, 220);
  - separators, a pixel of `0xFFAC8A67` across at y 48, 222, 263, 305 and 365;
  - the "+" shown while attribute points (`+0x23`) or skill points (`+0x42`) wait, or while the learning mode is on (`0x6AC310`);
  - the upper panel, from y 48: over the header bar its text (1096 the skills', 1145 the attributes') centred in (26, 47, 180), a shadow of `0xA0000000` 2 pixels down and right, `0xFFFFFF80`, `0xFFFFFF10` under the mouse, and the points of the list right in (8, 48, 236) in `0xFFFFFF80`;
  - the skills (`0x6A9790`): each slot's at y 69 + 19i, the name (text 9399 + the skill) at x 8, the level (its base `+0x2C` and its bonus `+0x34`, at most 255) right in (189, y, 39), `0xFF41A4FF` where the bonus raises it, `0xFFFF6347` where the base is at the hero's level or above; the first empty slot, while skills wait to be learnt (`+0x44`), text 1099 and their count in yellow `0xFFFFFF00`;
  - the learning mode: text 1099 in yellow at (8, 48) and the skills to learn at y 67 + 19i (below);
  - the attributes (`0x6A8DF0`): each of `a0` to `a5` at y 67 + 19k, text 1090 + k and its value (the stats' words `+0x10`…) right ending at x 228, `0xFF41A4FF` where the items raise it (`+0x22` bit k); at y 181 text 1071 and the experience `+0x0C`, at y 200 text 1072 and the level's end `X(L)`;
  - the lower panel: at y 222 text 1073 and the health `current/most`; at y 241 text 1119 and `X(L)` less the experience; at y 264 text 1074 and `A` right in a box of 135, and text 1117 right in the line; at y 283 text 1075 and `D` the same, and the hero's hit chance against its last foe `⌊min(100, 100·h(A, D′))⌋%` (`0x424000`, `0x43EEE0`, `D′` the last foe's of the player's statistics, `0x424700`) right, and the last foe's portrait (`0x4345C0`) in (148, 264)–(188, 304); at y 306 text 1087 and the weapon's damage `least-most`, at y 325 text 1089 and the selected art's (`0x6A6550`, read in part), at y 344 text 1077 and the resistances' sum `Σ ⌊r_k·m_k + 0.5⌋`; at y 365 text 1082 and the gold, `%s %8d`, centred;
  - the kinds' icons beside the damages, images 205 to 208 from (154, 308) and (154, 327), and the resistances', images 209 to 212 framed, from (154, 347), each the next to the left by its width less 4, at the alpha of its share of the greatest (`0x6A7140`).
- The weapon's damage (`0x563FD0` to `0x5641B0`): each kind's least and most (the stats' `+0x86`, `+0x76`), halved as the unarmed creature's, times the kind's multipliers (`+0x96`, `+0xA6`), each raised by the survival bonus `SB` as `⌊d·SB·0.5·0.01 + d + 0.5⌋` stored as a float (`LongivityCombatFactor` 0.5), the four summed.
- The survival bonus (`0x6AA062`): `100·(1 − 1/(1 + (m − 1)/120))` of the minutes played `m`, 0 below one (`0x565D70(0, m, 120, 50)`), and 10 times the difficulty's factor more where the hero's count `+0x4F8[0x1C]` reaches 6 (**hypothesis**: a set's pieces worn).
- While a line of the lower panel is under the mouse, the upper panel shows its details instead (the last foe's panel `0x6A7390`, the weapon's kinds `0x6A7C60`, the arts' `0x6A82F0`, the resistances' `0x6A8A00`) and four more lines: the bonus of the damage or of the resistances (1152, 1153), the pace (1101), the attack speed (1100) and the survival bonus (1159); a skill or an attribute under the mouse shows its description below (9499 + the skill, `UI_ATTRIB_*`, and the skill's effect at this level and the next, `0x566070`).

## Spending the points (`0x6A6110`)

- A click on the header bar turns the upper panel between the skills and the attributes.
- A shown "+" i:
  - in the attributes' list with attribute points: `0x580E90(i)` adds 1 to the attribute's extra points (`+0x1C + i`), takes 1 from `+0x23` and makes the stats anew (`0x5796A0(0, 1)`), with no sound;
  - in the skills' list, the slot i holding a skill and skill points left: where its base is below the hero's level, (9, `0x13`) and `0x5824A0(i, 1, 1)`, which adds 1 to the base, takes 1 from `+0x42` and makes the stats anew; else (9, `0x14`) and nothing (docs/re/sound.md: neither plays);
  - else, skills to learn left, the learning mode: the list of `0x581070`, the class's `Creature.pak` skills (its 16 bytes at `+0x16`) less those held, the first eight in order; a "+" i there learns its skill (`0x581120`) into the first empty slot at the base 1, takes 1 from `+0x44` and makes the stats anew, with no sound;
  - after any, the taskbar's message 7 with 2, which stops the "+"'s glow, and with 1, which hides it, once no points of any kind remain.
- A click at the gold (x from 723, y from 368) drops gold: none below 10, all below 1,000, else a tenth, thrown as a pile 20 to 50 about the hero (`0x4AEBD0` of the code `0x7E60`: 5132 below 50, 5133 below 350, 5134 below 1,000, else 5135).
- The taskbar's "+" (`0x6E1BC0`): the message 8 shows it glowing (a level-up, `0x57EFCE`, and the hero's set-up where points wait, `0x585260`), 7 with 2 stops the glow, 7 with 1 hides it; its click opens the inventory.

## The skills

The name of the skill n is text 9399 + n and its description 9499 + n, for 1 to 33.
The classes' lists at `Creature.pak +0x16` begin with their two starting skills; the first eight offered, with only those two held (**hypothesis**: the templates hold no others): the Seraphim 12, 25, 1, 3, 4, 10, 9, 22; the Gladiator 5, 6, 24, 7, 4, 10, 11, 8; the Mage 15, 17, 16, 18, 2, 4, 3, 22; the Dark Elf 13, 27, 26, 6, 10, 8, 23, 22; the Wood Elf 14, 12, 19, 7, 25, 10, 22, 3; the Vampiress 10, 28, 25, 4, 5, 11, 9, 23; the Dwarf 29, 31, 5, 21, 11, 7, 25, 9; the Daemon 32, 12, 4, 5, 6, 3, 25, 10.
The slots' order matters: the list shows them in order, a new skill takes the first empty one, and the stats take the skills in that order (`0x5823E0`, `0x582450` the first slot that holds a skill).

## Open questions

- The fonts' heights (the "+"'s step `h + 9` with `h` the font's at `0x182EE2C`, **hypothesis** 10) and the flag `0x40000` of the headers.
- The texts of `0x566070`'s text mode; the layouts of the hovered lines' panels.
- `0x75AA30` and what `cCommand_onInvClose` does while it holds; the region (680..780, 50..70) that a click also toggles the header by.
- The templates' points and skills' levels at the start (their section `0xC7`); `0x585260`'s caller `0x54B3D0`.

## In `opensacred-game`

Since #1521 the window shows with the inventory, read only (`statistics`): its backgrounds, frame, close button and header bar as above, the name (the class's, the player's own not kept yet), the level, the skills with their levels and the points (the base alone, no items' bonuses yet) or the attributes (the hero's with its items' bonuses, `fight::Combat`, blue above its own grown by the level) with the experience and the next level's, the lower panel's lines with `A`, `D` and the hit chance against the foe the hero struck last (`statistics::LastFoe`, **hypothesis** of the original's last foe) and its portrait, the weapon's damage raised by the survival bonus of the minutes played, the resistances, the gold, the icons, and the "+" while points wait; a click on the header bar turns the upper panel.
**Seen** in the game beside the original at the Seraphim's start: the name, the level, the two skills of level 1 in tomato red, the health 119/119, 300 to the next level, `A` 28, `D` 26, 0 %, the weapon's 30-33 with its icon, the resistances' 8 with their four icons and the gold, as the original's.
Not followed yet: the spending (#1522), the combat arts' damage, the hovered lines' panels and the descriptions, the close button, the set's part of the survival bonus, and the window's showing a follower's values.
