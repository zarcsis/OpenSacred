# The Underworld campaign's start and its heroes

Read 2026-11-08 for #1675 in `Sacred.exe` (GOG 2.28) and in the running original under the driver of #448.
Each claim is **confirmed** (read in the code, or seen in the running original) or a **hypothesis**; texts are named by their keys.
The choice of the hero's screen itself is in docs/re/menus.md, "The choice of the hero"; the heroes' file format in docs/02-data-formats.md §9.

## The heroes of `save/hero*.pax`

### The eight slots

- The choice's screen (`cUI_Character`, one object for both campaigns) reads eight slots, `SAVE\HERO%.2d.PAX` for 0 to 7 (`0x70C4A0`, the format `%s%s%.2d.%s` at `0x9E357C`), each through the hero loader in its summary's mode (`0x61D0F0` without a whole load), puts each hero found on its pedestal (`0x70C7E0`) and clears the others (`0x6F4E20`); it answers how many it found (**confirmed**).
- A pedestal keeps the file's 556-byte summary, the section `0xC3` (`0x6F4E60` copies `0x8B` words to its `+0x2FC`): its first word the level, its second the class, the hero's creature type (1 the Seraphim, 2 the Gladiator, 3 the Battle Mage, 4 the Dark Elf, 5 the Wood Elf, 6 the Vampiress, 8 the Dwarf, 9 the Daemon) (**confirmed**).
- Each pedestal is labelled with the hero's name and level, a free one `UI_SAVE_EMPTY`'s word (**seen**; below, "The pedestals").
  The slot 0 is the front left pedestal, the slot 1 the one behind it, the slot 6 the Dwarf's (on the right, in the middle) and the slot 7 the Daemon's (on the right, in front) (**seen** by the files the export wrote); the slots 2 to 5 among the other labels were not read.

### The pedestals

Read in the code (**confirmed**) and on the frames of the run (**seen**):

- A saved hero stands on the pedestal of its slot at the place and turn of the figures' table's entry of that index (`0x70C7E0` passes `0x9E326C + 0x50 · slot`, the entry's `+0x24`…`+0x30`, docs/re/equipment.md, "The choice of the hero's figures"), so the slot 0 stands where the Ancaria campaign's Gladiator does and the slots 6 and 7 where its Dark Elf and Vampiress do (**seen**: the exported Seraphim on the Gladiator's pedestal, the Dwarf and the Daemon on the two right ones).
- Its figure wears the items of its summary's equipment (docs/02 §9), and its class's default in the slot 0 or 2 where that is empty (`0x6F4E60`, `0x426580`); it is dark until the cursor is in its window, the table entry's, or it is chosen, as the Ancaria campaign's figures (**seen**).
- A free pedestal's word (`0x6F15A0`) is centred in its window's middle, white, `0xFFFFFF80` while the cursor is in the window or the pedestal is chosen (**seen**: the pedestal of a deleted hero stays chosen and yellow).
- A saved hero's label (`0x6F15A0`) is a box as wide as its window, from two lines above the window's foot to 5 pixels below it, black at the alpha `0x9F`, `0x7F000068` (dark blue) while the hero is chosen; in it, in the font 2 (Carolingia 12), a line every 19 pixels (the font's height of `A`), the hero's name centred, then `"%s: %d "` of `1070`'s word ("Level") and the level, followed by the medal of the highest difficulty won as an inline image (`\gFFFFFFFF%08x`), the interface's images 414 to 418 of `GUI_main_icons.TGA` (bronze's to niobium's, 16 × 16 at (137, 18) and 17 apart), the line centred with it; both lines in the class's colour (`0x43C250`: the Dwarf's `0x00FFFF`, the Daemon's `0xFF6C00`, the Seraphim's `0x87CEFA`).
  The medal is bronze's for a hero that won nothing; past niobium's, none.
  A third line, `UI_DIFFICULTY_HARDCORE` in red, follows for a hero of the hardcore mode (the summary's byte `0xF3`) where the screen's `+0x53D` is set (not read further); in a network game the level's line has no medal.

### When the original writes one

- **Export**, the menu over a game's item (`0x6BBBD0` posts the window `UI_CHARACTER`), opens the choice's screen in its export mode: its show (`0x70E8A0`) sets the mode 5 while a game runs (`0xAD6EF8` not 0), else the mode 7 in a single player's Underworld campaign, else the mode 0, the classes (**confirmed**).
  The export's screen shows the pedestals with the saved heroes and the free ones, a title naming the export as local, Back and Export (`UI_EXPORT`, on Start Game's button `+0x398`), greyed until a pedestal is chosen; any pedestal may be chosen (**seen**).
- Export (`0x70EC50` in the mode 5) on a free pedestal runs the action 6 (`0x70E502`): it formats `Save\Hero%.2d.pax` (`0x95E4D4`) with the slot, writes the hero there (`0x61CD00`) and shows the mode 5 again, the new hero on its pedestal (**confirmed**; **seen**: a new game's level-1 Seraphim exported to the slot 0 wrote `save/Hero00.pax`, 19,094 bytes, standing on the front left pedestal).
  The file has the shipped heroes' eight sections (`0xC7`, `0xCA`, `0xCB`, `0xC8`, `0xC3`, `0xC4`, `0xCD` of 822,824 bytes, `0xCE`), and `tools/analysis/formats/amh_dump.py --saves` reads it (**confirmed**); its name is written in mixed case, `Hero00.pax`, where the shipped ones are `hero06.pax` (**seen**).
- On a taken pedestal the action 9 (`0x70D71E`) sets the mode 6, the question `UI_MSG_OVERWRITE_CHAR` with OK and Cancel; OK (the answer 2, `0x70EE60`) runs the action 6, and Cancel goes back to the mode before (`+0x394`), writing nothing (**confirmed**; **seen**).
- Back goes back to the menu over the game (**seen**).
- No other path of a single player's game writes a hero's file: the writer `0x61CD00` has two callers, the export's action 6 and `0x7F38E0`, which runs only in a network game (`0x182EBEC`) and writes the network's hero into its profile's slot (**confirmed**).
  So the end of a campaign exports nothing: `UI_CHARACTER` is posted only by the main menu's two campaigns' items (`0x713670`), the menu over a game and the screen's own Back (`0x70D6E0`), and no script record opens it (**confirmed** by the references to the name).
- The cheat console's `loadplayer [N]` (`0x614230`, not in a network game) makes the object 2 again and loads `Save\Hero%.2d.pax` of N, 6 where N is not given (**confirmed** by code; not run).

### Import and Delete

- The Underworld's choice has no Import: the mode 7 relabels the choice's second button (`+0x3A0`) `UI_NET_PLAYER_NEW` (`0x70D25C`), and a click on it does nothing while the campaign (`0x182EE48`) is the Underworld's (`0x70F0DE`) (**confirmed**; **seen**: no such button shows).
- **Import** is the Ancaria campaign's: its button (`UI_IMPORT`) is enabled only where `save/` holds a hero (`0x70CEF6`, the count of `0x70C4A0`) and switches the choice to the mode 7, the saved heroes on the pedestals, the button relabelled to return to the classes (**confirmed**; **seen**).
  There any saved hero may be chosen, of any level, and Start Game runs the same `IMPORT_DO` as the Underworld's, in the Ancaria campaign (**seen** up to the choice; the imported game's start a **hypothesis** by code); Back from there goes back to the classes (**seen**).
- **Delete**: the erase button (`+0x3B4`, a red cross at the name field's right end, about (687, 586)) shows in the modes 4 and 7 while a pedestal is chosen (`0x70CA30`); its click asks `UI_CHAR_ERASE_CONFIRM` with OK and Cancel (`0x70EE60`), and OK (`0x720FDC`) calls `0x7F0CB0` with the slot + 1, which formats `Save\Hero%02d.pax`, sets the file's attributes normal and deletes it, then posts the profile's event that fills the pedestals again (**confirmed**); in a network game with a server the server is asked instead (**confirmed**, not run).
- After a deletion the pedestal shows itself free, but the panel keeps the deleted hero's name, its difficulty and Start Game lit until another pedestal is chosen (**seen** in both campaigns; what Start Game then does was not tried).

## Start Game in the Underworld campaign

### The choice

- The main menu's Underworld campaign sets the campaign `0x182EE48` to 1 (`UI_LABEL_NEWADDON`), the Ancaria campaign to 0 (`UI_LABEL_NEWGAME`) (`0x7127B7`, `0x7127DE`; **confirmed**); docs/re/saves.md reads a save's campaign at `+0x24` as 2 and 3 (**hypothesis**: the global plus 2).
- The screen opens in the mode 7 (`0x70E8A0`), its water without a reflection (`UI_CHAR_REF.TGA` in the modes 0 to 2 alone), the pedestals filled from `save/` (`0x70C6A0` with 5) (**confirmed**).
- **A hero below level 25 is not chosen**: in the mode 7 of a single player's Underworld campaign a pedestal's click chooses it only where its level is above 24 (`0x70CA30`: the pedestal's `+0x2FC` as a word against `0x18`) (**confirmed**); a click on a lower hero shows a red title, a level of 25 needed, and chooses nothing, Start Game greyed (**seen**: an exported level-1 Seraphim); a click on a free pedestal does nothing (**seen**).
  The Ancaria campaign's import has no such bound (**confirmed**, **seen**).
- A chosen hero shows its name in the name field (the summary's), the erase button and the difficulty's bar, from bronze up to one past the highest difficulty the hero won (`0x6F4F20` over the summary's bytes `0xED` to `0xF2`, the pedestal's `+0x3E9` to `+0x3EE`; docs/re/fight.md) (**confirmed**): the shipped Dwarf's and Daemon's six bytes are 0, so each is offered bronze and silver (**seen**, **confirmed** in the files), and so was an exported level-25 Seraphim (**seen**).

### `IMPORT_DO` (`0x70DC20`)

In this order (**confirmed** by code):

1. It reads the name field (`+0x3BC`) (**hypothesis**: a name typed does not rename the hero, whose own the file holds).
2. It sets the profile's class (`0x7D84A0`, `0x7D8550`) to the pedestal's and its `+0x14` to 1.
3. It sets the difficulty `0x182EE44` from the bar (`0x730960`), and leaves the hardcore flags `0x182EE50` and `0x182EE51` alone, which a new game's `SINGLE_GO` clears; the hero's loader sets `0x182EE50` from the summary (`0x6058A0`) (**hypothesis**: the hardcore mode comes with the hero).
4. It posts the UI event `0x28` with the class, hides the screen, and posts `cCommand_armaPlay` (`0x896FF4`) with the word 1 and the long `0x80000000 | (slot + 1) << 16` (`SINGLE_GO` posts 0 and 0) (**seen** in the log: `0x80070000`, the slot 6, the Dwarf's).

### The engine's start

- `initGame` (`0x60DB60`) runs as for a new game: outside a network game and with a class of 1 to 12 it makes the hero of the class's template (the table at `0xAD47B0`, the index class − 1, class − 2 for 8 and 9) and equips its weapon (**confirmed**; **seen**).
- Then, the command's slot + 1 not 0, it formats `Save\Hero%.2d.pax` with the slot (`0x60BE76`) and loads the whole file over that hero (`0x61D0F0` → `0x6058A0`) (**confirmed**; **seen**: every item kept its id).
- The script tree loads after the hero (`0x46F9B0`): in the campaign 1 outside a network game `bin\Addon\%s` of the hero type's name (`0x43CED0`; the type 7 taken as 6), so `bin\Addon\TYPE_NPC_ZWERG` for the Dwarf; in a network game `Bin\Addon\NetScript` (**confirmed**).
  The nine Underworld trees are alike (docs/02 §7.2), so the class changes nothing but the start script's own tests of the class.

### What the start script sets

From `bin/Addon/TYPE_NPC_ZWERG/StartCode.bin` and the run's log:

- **The place**: one `StartPosition`, the cell (6265, 3864) with no floor (**confirmed** in the tree); the Dwarf stood there, the sector (97, 60), on the ground, its home there (**seen**); the place of the hero's file is not used.
- **The quests**: the script sets up the quests 9511, 9512, 9514, 9518, 9520, 7601, 7612, 7613, 7614, 7616, 7617, 7618, 7623, 7630 and 9900, runs `QIS_Trigger11101`, and by the hero's type sets up 7609 and 7625 for the Dwarf, the Wood Elf, the Seraphim, the Battle Mage and the Gladiator, 7605, 7624 and 7626 for the Dark Elf and the Vampiress, and 9513 for the Daemon too (**confirmed** in the tree; **seen** for the Dwarf in that order).
  The log book is empty at the start: the quests are set up and none taken (**seen**).
- Three cells west of the start a peaceful creature of the type 224, of the dialogue 618, with the icon of a new quest over it, and a 271 beside it (**seen**).
- The clock read 08:02 (**seen**; who sets it was not read); no first window and no tutorial opened (**seen**).

### What the hero's file carries

The loader `0x6058A0` reads every section onto the hero made from the template (**confirmed** by code), and the Dwarf's start matched `save/hero06.pax` (**seen**):

- **The creature `0xC7`**, through the hero's own load (its virtual table's `+0x1C`, `0x54B3D0`): the level 29, the experience 1,800,000 and the gold 977,384, the health full, the attributes and their bases, the skills with their points and the skills to learn, the combat arts and the spells at the file's levels, bonuses and times, the combos, and the equipment's slots by the items' ids (**seen**; the layout in docs/02 §9, "The creature", #2361).
- **The inventory `0xC8` and the containers `0xCA`, `0xCB`**: the file's 52 items with their ids, 15 worn, the weapon sets and the backpack's 33 in its grid (**seen**; the sets a **hypothesis**), a healing potion in the belt (**seen**); the template's own weapon is gone (**hypothesis**: the load replaces it).
- **The summary `0xC3`**: the levels the hero reached on each difficulty (`0x425290`, `0x424D30`) and the hardcore flag.
- **`0xC4`** (64 bytes) is read only for the profile's own hero, into the player's record (`0x424670`, `0x4246C0`, `0x424C40`, `0x424C50`, `0x424C60`, `0x423F20`) (**confirmed** that it is read; not read further).
- **`0xCD`**, 822,824 bytes, is read where the file's version byte is above `0x19` and the world exists: a count and, an entry each, 8 bytes and `0x80` more applied to a sector (`0x634840`, `0x622BA0`) (**confirmed** that it is read; **hypothesis**: a sector's state, such as the land explored).
- **`0xCE`**, 100 bytes, goes to the player's record's `+0x5724` (**confirmed**; not read further).

Not the file's (**confirmed**): the difficulty, the bar's; the place and the home, the start script's; the quests, the log book and the scripts' state, the Underworld tree's start run anew; the campaign, the Underworld's whatever the hero played before.

## The choice without a saved hero

- **Confirmed** by code: the screen finds no file (`0x70C4A0` answers 0) and every pedestal is free; a click chooses none (its level 0 is below 25), so Start Game stays greyed and Back (`0x70D6E0`'s action 0 in the campaign 1) goes back to the main menu.
  The only way to fill it is to export a hero from a game of either campaign.
- In the Ancaria campaign's choice Import is disabled where `save/` holds no hero (its flag 4 cleared, `0x70CF12`; **hypothesis**: drawn greyed, as Start Game is by the same flag).
- Not seen with no file at all: the shipped `hero06.pax` and `hero07.pax` were not moved.

## For `opensacred-game`

- The Underworld's heroes are of level 25 or more; the game needs an export of a hero of its own (its format its own, ADR-08) and an import that keeps the hero's values, arts, skills, items and gold and the levels it reached by difficulty, and starts the place, the quests and the scripts at the Underworld tree's start (#2358, #1677).
- The shipped `hero06.pax` and `hero07.pax` are a fresh installation's only Underworld heroes; carrying them into a game needs the `0xC7` layout (docs/02 §9) or a conversion of their values (#1677).
- Since #1676 `opensacred-convert heroes` converts the two, and the game's choice (`choice`) stands them on their pedestals in their summaries' items, labels them as above, chooses one of level 25 or more (the red title of a lower one waits for the export, #2358), puts its name in the field, offers it the difficulties up to one past the highest it won, and lights Start Game; it shows no erase button (docs/decisions/interface.md, "The installation's Underworld heroes").
- Since #2360 Start Game starts the game of the chosen hero's class, name and difficulty in the class's tree of the add-on (`Class::underworld_tree`, `addon-type_npc_<class>`), whose start script places the hero at (6265, 3864) with the creatures 224 and 271 beside it and sets up its quests, no first window opening and the log book empty, as the original's start (seen 2026-11-09 with the Dwarf: the place and the creatures alike, `compare.py`); the hero is its class's template until its file's values, skills, arts and items come (#2362).
- Since #2362 the saved hero's game (`--saved SLOT`, which Start Game sets) carries its file's level, experience, gold, attribute points spent and left, skills with their bases, skill points and skills to learn once the hero stands (`carry`), its values made anew at its level and its health full, and starts with its file's combat arts: the statistics window shows the Dwarf's level, skills, points, the experience to his next level and his gold as the original's (seen 2026-11-09); since #2362's second part his items too, in place of his template's: those of his equipment's slots worn in them but the hands', his backpack's at their cells, his weapon sets with the one his hands' slots hold selected, each of its record's values at its template's scale and with its sockets' items, and his health full once their values are in it; his health, attack speed, skills and the items' bonus to his first skill, his backpack and his sets are the original's (seen 2026-11-09).
  Since #2370's first part the bar's art slots too, its first bank's arts in the slots (a combo's as `0x200` and its index) and the slot the file selects, whose art is the right button's: the Dwarf's four weapon arts with 1082 selected, as the original's bar and statistics window show them (seen 2026-11-09).
  Since #2370's second part his items' effects 845 and 848 count as the original's (docs/re/items.md, "The bonus effects that the code reads"): his weapon arts' bonuses, his attack 240 and his defence 179 are the original's (seen 2026-11-09).
  Since #2375 his arts' regeneration times, since #2391 his damage's and resistances' multipliers (the items' attributes in their attributes' parts) and since #2392 his made items' names are the original's too (seen 2026-11-09).
  His combos are empty in both shipped heroes, so a combo's carry waits for a hero that holds one, which the export makes (#2358).
