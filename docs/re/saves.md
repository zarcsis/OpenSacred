# Saves

What the original's save holds, and when it saves and loads (read 2026-11-03 for #1956, a part of #1095; **confirmed** by code unless marked otherwise).
The game does not load the original's saves (PLAN.md §1.1, ADR-08): this is what its own saves must give back.
The saves' window and the keys are in docs/re/window.md ("The menus that the driver clicks", the keys of `cEngine_receiveEvent`); the exported heroes `save/hero*.pax` are not saves (docs/02 §9, docs/re/menus.md).

## The file

`cEngine::save` (`0x619BF0`) writes one file, `./SAVE/GAME%.2d.PAK` by the save's slot (the strings also name `GAMEF%d.PAK` and `SAVE02.PAK`, not read), into `.\Save`, which it makes where it is missing.
It builds the whole file in memory (a `cFile_ram` of 8 MiB) and writes it at once; a save that cannot have the memory logs `---SAVING FAILED` and writes nothing.
While it saves, the game's timer stands (`0x6413E0`), and a loading screen shows `PAK\SAVEGAME.BMP` with a line for each part as it is written.

Before it saves, it compares a few of the hero's values (three numbers at its `+0x3B4`, `+0x3EE` and `+0x3FE`, its attributes' bases and bonuses, its skills' levels) with a copy kept in a write-protected page, and where they differ it marks the hero a cheater and takes those values away (`Cheater detected`); the game has no such check (its console and its commands change the hero openly).

The file begins with a header of 256 bytes, then a table of its parts of 768 bytes, then the parts:

- **the header**: the letters `AMS`, the version 27 (`---SAVING (%c%c%c) v%d #%d`), the hero's type and its creature, the campaign (2 Ancaria, 3 the Underworld, `0x182EE48`), whether the game is a network one (`0x182EBEC`), the difficulty (`0x182EE44`), the hero's level reached on each of the five difficulties, the game's calendar (its day, hour and minute), the computer's date and time, and the sums of the bytes after it;
- **a descriptor** (`0x83`): the line `Date dd.mm.yyyy Time hh:mm` of the game's calendar, for the saves' list;
- the save's **name**, 64 bytes, which the saves' list and the quick save (`QUICKSAVE`) and the autosave (`AutoSave`) give;
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
- **the picture** (`0x93`): the frame shown in the saves' list, 32 KiB;
- **the stats** (`0x9B`): the hero's player's values (`0x424E10`);
- `0x9C`: two of the interface's states (`0x75A970`, `0x75A990`), and `0x9D`, 1 KiB of zeros;
- **the hero** (`0xC3`, 556 bytes): the hero's name and the values that the saves' list and the choice of a campaign's hero show (`0x604300`).

The load (`0x61B6F0`, `---LOADING SAVEGAME.PAK`) refuses a save of another version (`savegame v%d incompatible (need version %d)`) and logs the save's language beside the game's.

So the original's save keeps the whole world as it stands: every creature and item of the object manager where it is, the sectors' states, the blood, the particle systems, the weather, the calendar and the scripts' state, beside the hero.
Which sectors the object manager and the world keep (all the world's, or those visited) is not read; that a creature killed far away stays dead after a load is a **hypothesis** to check against the running original (a creature killed, the hero far away, a save, a load, the hero back).

## When it saves

- **Save Game** in the menu over a game opens the saves' list (`UI_WND_SAVEGAME`), which saves into the row chosen, its name typed into the row (docs/re/window.md); it asks before it overwrites a save.
- **The quick save** (F9, and Control+Z): where the interface allows it (`0x75ADB0`, false while a merchant's, a smith's, a combat master's or a stash's window is open), `cEngine_receiveEvent` names the save `QUICKSAVE` (`0x618E3A`) and sets the engine's request to save (its `+0x54` bit `0x40`), which the engine's loop carries out; F8 (and Control+X) loads it outside a network game.
- **The autosave**: an event of the engine with the save's slot, its name and an autosave's flag (`0x618FA0`); where the interface does not allow a save it is refused with `AUTOSAVE [not allowed now!]`.
  The options hold an autosave's switch (`UI_CFG_AUTOSAVE`, `UI_BTN_AUTOSAVE`, `AUTOSAVE` of `Settings.cfg`, which `0x81B6F0` reads).
  The scripts post it, not a timer (#1961): the record `AutoSave` (121, `0x4AC740`) takes its number (its tag `0x16`, `0x1D` or `0x1E`) and, where the number is not 0, outside a network game (`0x182EBEC`) and with `AUTOSAVE` set, posts the engine two events, `(0xF, 3)` and then `(0xF, 2)` with the slot −1 and the number, the second 100 later (**hypothesis**: milliseconds); which file the slot −1 takes and the name it gives the save (**hypothesis**: the number's text) are not read.
  The numbers are 30000 to 30027 (texts of `global.res`); the Seraphim's tree holds 26 records, in the sections of its quests' exits (`QIS_OnExit11`, `QIS_OnExit22`, `QIS_OnExit41`, …) and a few functions of the quests 9, 11, 22, 41 to 45, 50 to 59, 61, 65, 71 to 74, 81 and 670, so that the original autosaves as the main quests are done.
  docs/06 §6's `AUTOSAVEDELAY` is no key of this release's (no string of `Sacred.exe` names it).

## For `opensacred-game`

The game saves its own format (ADR-08), and keeps what the original's save keeps, by its own means: the hero (its values, items, arts, combos, skills, effects, place), the world (the creatures alive and dead by their sectors, the items on the ground, the doors, the containers and the hidden stashes, the followers, the town stash, the explored map), the scripts' state, the calendar and the weather, the difficulty and the campaign.
#1095 is split into its pull requests on its issue.
