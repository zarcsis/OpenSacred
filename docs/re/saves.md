# Saves

What the original's save holds, and when it saves and loads (read 2026-11-03 for #1956, a part of #1095; **confirmed** by code unless marked otherwise).
The game does not load the original's saves (PLAN.md §1.1, ADR-08): this is what its own saves must give back.
The saves' window and the keys are in docs/re/window.md ("The menus that the driver clicks", the keys of `cEngine_receiveEvent`); the exported heroes `save/hero*.pax` are not saves (docs/02 §9, docs/re/menus.md).

## The file

`cEngine::save` (`0x619BF0`) writes one file, `./SAVE/GAME%.2d.PAK` by the save's slot (the strings also name `GAMEF%d.PAK` and `SAVE02.PAK`, not read), into `.\Save`, which it makes where it is missing.
It builds the whole file in memory (a `cFile_ram` of 8 MiB) and writes it at once; a save that cannot have the memory logs `---SAVING FAILED` and writes nothing.
While it saves, the game's timer stands (`0x6413E0`), and a loading screen shows `PAK\SAVEGAME.BMP` with a line for each part as it is written.

Before it saves, it compares a few of the hero's values (three numbers at its `+0x3B4`, `+0x3EE` and `+0x3FE`, its attributes' bases and bonuses, its skills' levels) with a copy kept in a write-protected page, and where they differ it marks the hero a cheater and takes those values away (`Cheater detected`); the game has no such check (its console and its commands change the hero openly).

The file begins with a header of 256 bytes, then a table of its parts, 64 entries of 12 bytes (the part's id, its offset and its size; the header's `+4` holds 64), then the parts (corrected 2026-11-04 for #1990 and #1991, **confirmed** by code and seen in the original's files):

- **the header**: the letters `AMS`, the version 27 (`---SAVING (%c%c%c) v%d #%d`), the hero's type (`+0x14`, 1 the Seraphim) and a colour of it (`+0x18`, `0x43C250`), the campaign (`+0x24`: 2 Ancaria, 3 the Underworld, `0x182EE48`), whether the game is a network one (`+0x2C`, `0x182EBEC`), the difficulty (`0x182EE44`), the hero's level reached on each of the five difficulties, the game's calendar (its day, hour and minute), the computer's date and time (`+0x5C` to `+0x78`, a `SYSTEMTIME` as words of four bytes), `0xBAADF00D` at `+0x7C`, and the sums of the bytes after it;
- **a descriptor** (`0x95`, 256 bytes of UTF-16): the line `Date[dd/mm/yyyy] Time[hh.mm]` of the game's calendar and a line feed, which the saves' list shows where the header holds no computer's date;
- the save's **name** (`0x83`, 64 bytes), which the saves' window, the quick save (`QUICKSAVE`) and the autosave (its number's text) give;
- **the engine** (`0x80`): a few of its values (not read);
- **the mouse** (`0x98`): the mouse's state;
- **the triggers** (`0x81`): the world's sectors (`0x63B580`, the same function loads them: `cWorld::load() sectors: #world[%d] #saved[%d]`, the sectors of the world and those it keeps a state of; each sector's record not read);
- **the calendar** (`0x8B`): the day and the time;
- **the blood** (`0x94`): the blood's stains on the ground (`0x401660`);
- **the regions** (`0x99`, `0x61D450`): the regions' states (not read);
- `0x9A` (`0x422820`; not read: **hypothesis** the map's explored land);
- **the items** (`0x8E`): every object of the object manager (`0x602300`; `cObjectManager::save() Ref[%3d] Fam[%2d] wp[%d,%d,%d]`, each with its family and its place in the world), the creatures among them (`cCreature::save`), so the world's creatures, alive and dead, and the items lying on the ground;
- **the inventory** (`0x8D`): the hero's items (`0x5DEBC0` with the hero; `ItemDataMgr::saveHero`, each slot's item and the hero's place `wpos` and portals);
- **the particles** (`0x96`): the particle systems that live, the spells' and the arts' objects among them;
- `0x82`: the engine's sixteen players, each that is there saving itself (the hero's player among them);
- **the weather** (`0x92`, `0x41BBA0`);
- **the quests** (`0x91`): the scripts' state (`sdk_SAVELOAD_SERIAL`: the quests, their flags, the variables, the timers);
- **the picture** (`0x93`): the frame shown in the saves' window ("The picture", below), listed 0x6000 bytes long in the table but written 0x8000 long and followed by `0xDEADC0DE`;
- **the stats** (`0x9B`): the hero's player's values (`0x424E10`);
- `0x9C`: two of the interface's states (`0x75A970`, `0x75A990`), and `0x9D`, 1 KiB of zeros;
- **the hero** (`0xC3`, 556 bytes): the hero's name and the values that the saves' list and the choice of a campaign's hero show (`0x604300`).

The load (`0x61B6F0`, `---LOADING SAVEGAME.PAK`) refuses a save of another version (`savegame v%d incompatible (need version %d)`) and logs the save's language beside the game's.

So the original's save keeps the whole world as it stands: every creature and item of the object manager where it is, the sectors' states, the blood, the particle systems, the weather, the calendar and the scripts' state, beside the hero.
Which sectors the object manager and the world keep (all the world's, or those visited) is not read; that a creature killed far away stays dead after a load is a **hypothesis** to check against the running original (a creature killed, the hero far away, a save, a load, the hero back).

## When it saves

- **Save Game** in the menu over a game opens the saves' list (`UI_WND_SAVEGAME`), which saves into the row chosen, its name typed into the row (docs/re/window.md); it asks before it overwrites a save.
- **The quick save** (F9, and Control+Z): where the interface allows it (`0x75ADB0`, false while a merchant's, a smith's, a combat master's or a stash's window is open), `cEngine_receiveEvent` takes the frame for the save's picture (`cEngine_captureInternal`), names the save, sets its slot 0 (`+0x68`) and the engine's request to save (its `+0x54` bit `0x40`), which the engine's loop carries out (read 2026-11-04, #1988).
  F9 (the game's key code `0x117`) names it `QUICKSAVE` (`0x618E3A`), Control+Z (its character `0x1A`) by the text `UI_QUICKSAVE`.
  F8 (`0x116`) and Control+X (`0x18`) set the slot 0 and the request to load (`+0x54` bit `0x200`) outside a network game: the quick save is the slot 0's file, `GAME00.PAK`, which both load.
- **The autosave**: the engine's event `(0xF, 2)` with the save's slot (`+0x0C`), a number (`+0x10`), whether to take the picture's frame (`+0x14`), whether the interface must allow it (`+0x18`) and a name (`+0x20`) (`0x618FA0`): with the test asked and the interface not allowing a save it is refused with `AUTOSAVE [not allowed now!]`; a name given replaces the save's, the slot −1 takes the first free slot (`0x619B80`: the first `SAVE/GAME%.2d.PAK` from 1 that does not open, the slot 511 where 1 to 509 are all taken), and the number goes to the engine's `+0xAC` before the request to save; `(0xF, 3)` takes the picture's frame alone, and `(0xF, 1)` asks to load the slot.
  The options hold an autosave's switch (`UI_CFG_AUTOSAVE`, `UI_BTN_AUTOSAVE`, `AUTOSAVE` of `Settings.cfg`, which `0x81B6F0` reads): where the file lacks it, the function writes it 0 (`%d` of 0, `0x662DF0`) and answers yes that once, so that a fresh installation's first test autosaves and every later one, the setting 0, does not; the reference installation's file holds 0.
  The scripts post it, not a timer (#1961): the record `AutoSave` (121, `0x4AC740`) takes its number (a field of its tags `0x16`, `0x1D` or `0x1E`, the last read) and, where the number is not 0, outside a network game (`0x182EBEC`) and with `AUTOSAVE` set, posts the engine `(0xF, 3)`, the picture's frame, and then `(0xF, 2)` with the slot −1, the number, no picture, no test of the interface and no name, the second 100 later (**hypothesis**: milliseconds) (read 2026-11-04, #1989).
  `cEngine::save` (`0x619BF0`) names a save whose number is not 0 by the number's text (the game's texts by their numbers, made narrow, 63 characters at most) and clears the number, which the header keeps too: each autosave goes into a slot of its own, named by its quest's text.
  The numbers are 30000 to 30027 (texts of `global.res`); the Seraphim's tree holds 26 records, in the sections of its quests' exits (`QIS_OnExit11`, `QIS_OnExit22`, `QIS_OnExit41`, …) and a few functions of the quests 9, 11, 22, 41 to 45, 50 to 59, 61, 65, 71 to 74, 81 and 670, so that the original autosaves as the main quests are done.
  docs/06 §6's `AUTOSAVEDELAY` is no key of this release's (no string of `Sacred.exe` names it).

## The picture

Read 2026-11-04 for #1991 (**confirmed** by code; the pictures of three saves decoded and compared with the frames taken just before them in the running original, a mean difference of 0.03 to 0.1 of 255).

- `cEngine_captureInternal` (`0x613670`) locks the 1024 × 768 back buffer, copies it to the engine's `+0x1A0`, makes it ARGB4444 of alpha 15 in place (from 32 bits each channel's top four bits, `0x6609A0`; from 16 bits through 555, `0x660940`, `0x660A60`) and scales it to 128 × 96 into `+0x1A4` (`0x660AD0`, its filter on), 0x8000 bytes.
- The scale: the pixel (x, y) is each channel's whole mean of the four bits of the screen's pixels at (8x − 8, 8y − 8), (8x, 8y − 8), (8x − 8, 8y) and (8x, 8y), those left of or above the screen left out: a sparse sample rather than a box, about a picture's pixel right of and below a centred one, of the whole screen, the interface with it.
- When: the engine's `(0x13, 5, 1)` (`0x613370`), which the menu over a game sends as it opens (`0x6BB6C0`, at `0x6BB795`) and the key S before it opens the saves' window; F9 and Control+Z before the quick save; an autosave's `(0xF, 3)` before its save. A save through the saves' window takes none, so it keeps the frame from before Escape or S.
- In the file: little-endian ARGB4444 words, rows from the top, 256 bytes a row.
- The saves' window draws it through one texture of 128 × 128 (`ui_sl_rendertarget.tga`), filled before each row, as a 128 × 96 quad at a texel a pixel (`0x71DE70`, `0x71DFA0`); an empty row's is filled with `0x5555`, a translucent grey.

## For `opensacred-game`

The game saves its own format (ADR-08), and keeps what the original's save keeps, by its own means: the hero (its values, items, arts, combos, skills, effects, place), the world (the creatures alive and dead by their sectors, the items on the ground, the doors, the containers and the hidden stashes, the followers, the town stash, the explored map), the scripts' state, the calendar and the weather, the difficulty and the campaign.
#1095 is split into its pull requests on its issue.

Since #1957 the game saves the hero (`saves`): a save is postcard in `saves/<name>.sav` beside the settings' file, its header first (the version, the save's name, the hero's class, name, difficulty and level, the calendar, the computer's time), which the list reads alone, and a save of another version is refused, as the original's load refuses one.
It keeps the hero's place and floor, its experience, attributes and skills, its health, its effects, its arts, art slots and selected art, its combos, its pack (the gold, the backpack, the items worn, the quests' items, the books read), its weapon sets, its home, its statistics, the time played and the calendar.
A load starts a game of the save's hero in its sector, as a new game starts, and once the hero stands puts the saved values back after the resets of a game's start, its health at its level so that no level-up fills it, its values made anew with its effects, and the hero where it stood.

Since #1958 a save keeps the scripts after the hero: the game's seed (`seed::Game`), the scripts' state (`opensacred_rules::quests::Scripts`: the quests' flags, log and book, the variables, the dialogues, the regions run, the places of `DefPos`, the base triggers, the timers, the drops and the replacements, the kills and the items counted, the main quest's items, the pools, the hidden stashes not searched yet, the map's names), the functions marked done, the scripts' `rand()`, and what the play changed of the placements (`npcs::Saved`): the play's records of the world in the batches they were placed in, the triggers open, those a hero turned and those locked, the quests' items taken and the containers emptied.
A load's start script runs again of the save's seed, which makes the saved game's tree and placements again, as the indices of the placements in the save need; its state, functions done and `rand()` are then the save's, and its talks and texts (the new game's first window) are dropped, so that nothing of the start is made twice.
The play's records of the world are placed again batch by batch on the placements, none in the world yet: the creatures and objects they made come with their sectors, once each, with the states, groups and moves the play gave them.
A save of other scripts' assets (another conversion of the tree) may not make the same placements again.
The scripts' timers count on from the calendar put back, not across its change.
The regions' kills are not kept: the original's save does not touch them (docs/re/maps.md, "The overview map", a **hypothesis**), and a loaded game's counts start full.
Since #1959 a save keeps the world after the scripts (`saves::World`, the saves' version 3).
Its creatures are what the sectors keep of those that went with them (`npcs::Kept`: the kept placements where they stood with their health or that they were killed, the fills' living creatures by their sectors, the sectors that kept a time and the region that filled last), the creatures of the sectors in the world gone into it as an unload of their sectors would keep them, so that one mechanism serves both.
A load's sectors put each back as they come in: where it stood with its health, a killed one not at all, and a sector whose fills' creatures it keeps fills nothing anew.
The placements of the living kept creatures go with them as the play left them (a weapon chosen anew, the death's functions left, a `RegionChange`'s type), by their indices, which the load's start makes again.
A creature killed far away stays dead after a load, as the save holds it; whether the original's does is the **hypothesis** above.
The hero's followers (hired, summoned, called or raised) are kept apart: each what it was made of (its kept placement, its fill's or its type), where it stood with its health, its flags word, side and stand in the others' way as its hiring left them, its hands' items with their values, the items the hero gave it and its effects; they come back beside the hero once it stands, their placements kept out of their sectors.
The save keeps the drops lying, and flying with their flights, but the scripts' items, which their placements lay again; the town stash's items; the merchants' stock, its merchant known by its placement rather than its entity, which a load does not keep; the land explored, the sectors entered and the world map's pin; and the weather, its timers of real time kept from the save's time and put on the load's.
Not kept: the creatures' script queues (`npcs::ScriptQueue`) and a `CreateObj` waiting in one, what the creatures were doing (a pursuit, a walk under way), the creatures that the sectors' `Enter` functions and the waters make again at each coming, and the corpses.
Since #1988 F9 and Control+Z save the game as `QUICKSAVE` where the interface allows a save (`saves::allowed`), and F8 and Control+X load it, two actions of `controls.toml`; since #1989 the quick save is the slot 0's file, `saves/GAME00.sav`, its name inside it, as the original's.
Since #1989 a script's `AutoSave` saves the game where `[gameplay] autosave` of `settings.toml` is on, off by default (docs/decisions/interface.md, "Autosave off from the first test"): into the first free slot from `GAME01`, named by its number's text, whatever windows show.
A save's file and its name are apart: the console's `save NAME` writes `NAME.sav`, its name the same.
The load of the hero's stats (`0x585260`, its reading branch) tells the taskbar its "+" as the points leave it, the message 8 (shown, glowing) where attribute or skill points or skills to learn wait, else 7 with 1 (hidden); since #1992 a loaded game's hero sets the "+" so (`points::Plus::loaded`).
Since #1991 a save keeps a picture after its header (the saves' version 4; a save of the version 3 still reads, without one): the window's frame as the menu over a game opens, which shows the menu a frame later, and right before a quick save and an autosave, which wait for it (30 frames at most, then save with the last taken), sampled as the original does of the frame's middle of its screen's shape (`save_picture`; docs/decisions/interface.md, "Save's picture of the screen's middle"), kept as the original's ARGB4444 words.
