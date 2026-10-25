# NetScript: the reader of the original

Recovered from the GOG `Sacred.exe` 2.28 with capstone on 2026-10-17 (`tools/analysis/scripts/netscript_dump.py` checks the dispatch tables and reads the keyword resolvers; the handler widths below were read off their code).
The record format is in [docs/02 §7](../02-data-formats.md#7-netscript-scripts); this document describes how the client reads it.
Status: **confirmed** on the code and on all 2,203,278 records of the 20 trees, unless marked otherwise.

## Record walker `FUN_00475680`

- The walker copies a record into a stack buffer before it handles it.
  Records longer than 500 bytes (0x1F4) take another path, which was not followed.
  The longest record in the release is 169 bytes.
- It dispatches on the record type through a 141-entry jump table at `0x4784C0` (bound check `cmp eax, 0x8C`).
  18 entries go to the default case `0x4784B3`, which returns 1 and continues the section.
- Type 40 copies the 80-byte body as is into a table of 0x50-byte entries at `this+0x755C`.
- Type 41 reads the body as a u16 and accepts 1..5, which it dispatches through a jump table at `0x478768`; type 42 ends the block it opens ("Control flow" below).
- Type 43 returns 0 and ends the section.
- Every other record reads its operands through the operand reader, one call per operand.

## Control flow

Read 2026-10-24 (#722) in the walker and the function runner `FUN_0046BA90`, **confirmed** by code; the forms counted on all 20 trees by `tools/analysis/scripts/netscript_flow.py`.

- **One record a call.** The walker reads the record at the cursor (its type and its length, a `u16` each), moves the cursor past it and handles it; it answers 1 to go on, 0 to stop the function, 2 and 3 (below).
  The runner (`WorkFunktion`) reads the function's bytes (`Vectoren.bin`'s offset and length) and calls the walker until the cursor reaches the function's end, when the function **ran to its end** and the runner answers 1, or until the walker answers 0, when it answers 0.
  The walker's 2 marks the function done (`+0x50` of its entry in the table of 84-byte functions) and answers 1, so that the runner never runs it again; 3 starts the function again from its first record.
  The runner runs a function only while its entry is not done and, for a function of a quest (`+0x48` of its entry 0 or more), only while that quest is in the quest store with its flag 1; it does not run functions whose names begin with `InitRg` unless they name `Pools` or `_DQ_Quest_`, nor those beginning with `DQTrgRegion` or `DQStartRegion`.
- **No offsets.** No record or operand carries an offset to jump to: the conditional records below move the cursor by the records' lengths alone, so that a record put into a function moves nothing else (sacred-sdk's report of shifted jump offsets, docs/02 §7.4, is not the walker's).
- **`IF` (58).** Its condition (`0x4987B0`) holding, the next record runs.
  False, the walker reads on, record by record by their lengths, and stops at the first `ELSEIF` (66) whose condition holds, or at the first `ELSE` (59): the record after it runs next.
  It counts no inner `IF`: an `IF` within a branch shares the first `ELSEIF` or `ELSE` after it.
  A record of type 0 or above 140, or the function's end, before either, logs `IF mit offenem Ende` and answers 0.
- **`ELSEIF` (66)** reached in the flow, its branch having run: the walker reads on to the first `ELSE` and moves past the one record after it; without an `ELSE`, `ELSEIF mit offenem Ende` and 0.
- **`ELSE` (59)** reached in the flow moves past the one record after it: an `ELSE`'s branch is that one record.
- **Blocks, 41 and 42.** Type 41 sets the walker's skip (`0xAB7898`); while it is set every record is passed over but 42, which clears it.
  41 clears it again at once where its selector's flag holds: of the entry of the interpreter's quest table (`this+0x31C`, entries of `0x34` bytes, the flags at `+0x10`) that the creature's `+0x94` names, without its flag `0x20`, the selector 1 to 5 tests the flag 1, 2, 4, 8 or `0x10`; for 1 and 2, where the creature is in the list `+0x39C` of `0x603E30`'s object and the flag holds, the flag first becomes 4 (**hypothesis**: a quest given and taken by the speaker).
  So a block runs by the state of the creature's quest, as a dialogue's section.
- **The guards.** `GroupIsDead` (24) and `DelDlg` (44) answer 0 on a false result, `ExitDlg` (43) and a record of type 0 always, so that the function stops; `QuestDone` to `QuestNotInProgress` (10 to 13) do too, but none of them occurs in the release.
  `DeleteFunktionBlock` (14) marks done each function that its string operands name, or with the operand `0x0B` the function running, and answers 0; without such an operand it answers 2, the function done.
  `ToBegin` (111) answers 3.
- **The forms in the release** (the census, 2026-10-24): every `IF` (31,463) reaches an `ELSE` but 2, a chain of `ELSEIF`s in 20,015 of them, and every `ELSEIF` reaches an `ELSE`; the 2 are `IF`s at the end of the function `OMO-1teleportTrigger13_rg1` of two trees, before one record, so that a false condition answers 0 (a guard).
  In 227 chains an `IF` stands in the last branch, after an `ELSEIF`, and shares the chain's `ELSE`: `IF a; A; ELSEIF b; B; IF c; C; ELSE; x` runs A for a, B and C for b and c, B and x for b alone, and x alone otherwise.
  In 31 chains the record after the `ELSE` is an `IF` (`… ELSE IF c; R; ELSE x`): with an earlier branch run, the `ELSE` moves past the `IF` alone, so that R runs whatever c, and the second `ELSE` past x; with none run, the `IF` decides between R and x.
  In 11 chains the record after an `ELSE` is another `ELSE` (`IF d; r; ELSE; ELSE; s`): with d the second `ELSE` is passed over with r's branch and s runs; without d the first `ELSE`'s branch is the second, which moves past s.
  The 4,950 blocks of 41 each end at a 42, none within another, none with an `IF` chain open at its 42; type 43 occurs 4,981 times, 24 915, 44 566, 14 352, 111 1,125.

## Operand reader `FUN_00472BC0`

**Calling convention.**
`__thiscall reader(this, u32 *cursor, record *rec)`.
`*cursor` is the offset of the next tag from the start of the record, header included.
The reader returns 0 when `*cursor >= rec->length`.

**Dispatch.**
The tag byte `t` dispatches in two steps:

- `t - 1` above 0xA0 jumps to the error exit `0x475504`, which logs an error and returns 0.
  This covers tag 0 and every tag from 0xA2 up.
- Otherwise the byte table at `0x4755D4` selects one of the 40 handlers of the jump table at `0x475534`.

**Results.**
Each handler stores the value in the reader's slots: numbers at `this+0xA860…`, strings at `this+0xA460…`.
It then advances `*cursor` past the tag and the payload, logs the operand and returns the tag.

**Handlers.**
The table lists every handler with its cursor advance, payload first, then the tag byte.
"str" is a NUL-terminated string.

| Handler | Tags | Payload |
|---|---|---|
| `0x474C47` (default) | 66 tags: 06 07 08 0E 0F 10 12 13 14 1A 1B 23–27 2B–2F 30 31 32 39 3F 42–46 4C 4E 4F 50 51 58–5C 61 62 64 65 66 70 72 74 78 7B 7C 80 85 8A 8D 8E 91 96–9A 9E A0 A1 | none |
| `0x474C52` | 71 | none: the code tests the byte before the cursor for 1, which is the tag itself |
| `0x4747F3`, `0x473A11` | 28; 8B | u8 |
| `0x474DA2`, `0x473A58` | 03 0A 6B 6C 9B 9C; 93 | u16 |
| `0x4739D2` | 84 | u16 × 2 |
| `0x47478C` | 15 | u16 × 4 |
| `0x473A27` | 02 | i32; logged through `0x43CED0` as a name |
| `0x474A46`, `0x474A1E`, `0x474F90`, `0x473F54`, `0x473A86` | 11 36 75; 1C 53–57 86; 1D (logged as `res:%d`); 3B 73; 7E 7F 8C 90 | i32 |
| `0x473FBC`, `0x47402C` | 33 87 88 89; 34 79 | i32 × 2 |
| `0x473F69`, `0x474065`, `0x473FF5` | 19 (the first logged as a name); 20 2A 4D; 35 | i32 × 3 |
| `0x474731` | 3D | i32 × 4 |
| `0x472C4D`, `0x474823`, `0x474DCD`, `0x4751C4`, `0x473DF1`, `0x4738E8`, `0x473A99`, `0x473257` | 01 29 63 68 69 6A 9D; 05 09 41 60 83; 16; 1E; 3E 77 81 82; 40 95; 47 52 7D 8F; 67 | str; several handlers resolve `res:` names, `CPOS:` positions and `ECS_*` spell names |
| `0x473607` | 3A | str, str; the second string's length is measured from its second byte, so an empty second string would desynchronize the cursor (none occurs) |
| `0x4740AF` | 92 | str, str |
| `0x473AF2`, `0x474A6E` | 6F; 7A | i32, str |
| `0x473C48` | 48 49 4A 5D 5E 6D 6E | i32, str; then, if the i32 is negative and the next byte is not 0, a second str: an expression (`A+VAR(B)`) whose value, looked up in the variables at `this+0x7550`, replaces the i32 |
| `0x474181`, `0x47463F` | 04 0C 0D; 3C | i32 × 3, or u32 0xFFFFFFFE and a str: a position name (`CPOS:hero`, `CPOS:RES:…`) |
| `0x47487C` | 0B 38 5F | i32, or u32 0xFFED2979 and a str: a variable name looked up in `this+0x7550` |
| `0x474C4F` | 37 4B | u8, and a str when it is 1 |
| `0x4749AC` | 1F | i32, or a variable reference in its place |
| `0x4754A7` | 9F | a variable reference starting at the tag itself |
| `0x475504` (error exit) | 17 18 21 22 76 94, and 00 and A2–FF by the bound check | ends the record; no record of the release contains one |

**Variable references.**
`FUN_00453970` reads the bytes `9F EF BE ED FE` followed by a name string and looks the name up in the variable vector at `this+0x7550` (entries of 0x24 bytes: `char name[32]`, `i32 value`).
It moves the cursor past the name whether or not the name is found.

- If the name is not found, the handler of tag 0x1F reads 4 more bytes as the number.
  The data relies on every reference resolving (**hypothesis**: the script defines its variables before using them).
- The handler of tag 0x9F reads 4 bytes at the tag itself when the reference does not resolve.

**Truncated operands.**
A payload that runs past the record end is read from whatever follows the record in the copy buffer.
In the release, 11 records of type 50 (`TriggerPatch`) end with tag 0x2A and no payload; each names `chestTrigger`, one in the `StartCode.bin` of each of the 9 Underworld trees and of both multiplayer trees.
The handler of 0x2A reads 12 bytes past the record.

## Keyword resolvers of the compiler

The script compiler built into the client maps keywords to record types in five functions.
Each function fills two parallel stack arrays: names ending in a null pointer, and codes.
It compares the token with each name through `0x451AD0` and returns `codes[i]`.

| Function | Names → codes | Keywords |
|---|---|---|
| `0x451BE0` | `esp+0x0C` → `esp+0x104` | 61 statements (`AddExp` … `MachVersteck`) |
| `0x452370` | `esp+0x14` → `esp+0x108` | 60 statements (`CreateNPC` … `GiveSkill`, `IF`, `ELSE`) |
| `0x452130` | `esp+0x0C` → `esp+0x2C` | 7 quest directives: `MaxOffen:` 27, `Delay:` 28, `Rettungsmission:` 29, `ToDoBloecke:` 30, `Schatzsuche:` 34, `Toete_NPC` 101, `Begleite` 102 |
| `0x452200` | `esp+0x0C` → `esp+0x28` | 6 quest sections: `Text:` 26, `Dialog:` 31, `Belohnungen:` 32, `HideTmpToDo:` 33, `SolveTime:` 122, `Strafen:` 123 |
| `0x4522C0` | `esp+0x0C` → `esp+0x1C` | 3 dialog commands: `ExitDlg` 43, `DelDlg` 44, `ToBegin` 111 |

**Coverage.**
Together the resolvers name 135 of the 141 types (`opensacred_formats::netscript::KEYWORDS`).
`DelNPC` and `DelOBJ` both compile to 55.
Types 0, 39, 40, 41, 42 and 47 have no keyword: the compiler emits them itself.
Types 39–42 occur in the release.

**Corrections to the earlier readings:**

- 43, 44 and 111 are dialog commands.
  Earlier readings had 43 as a guard bracket, 44 as "the dialog dispatcher" and 111 as "end of section".
- 122 and 123 are `SolveTime:` and `Strafen:`.

## Operand signatures

The tags each record type carries, over the 2,203,278 records of the 20 trees (`netscript_dump.py --signatures`, 2026-10-17): the 30 commonest types, which hold 88.3 % of the records, with the number of distinct tag sequences and the two commonest (`-` for none, the count in brackets).
Types 39 and 40 are no tag streams (above).
What the tags mean in each type is the work of M4; `CreateNPC` is read in docs/02 §7.3.

| Type | Name | Records | Sequences | Commonest |
|---|---|---|---|---|
| 115 | MakeGroup | 455,240 | 38 | `02 02 87 88 89 (90060)`; `02 02 02 87 88 89 (70660)` |
| 100 | SpawnValues | 229,960 | 1 | `0b 0b 0b (229960)` |
| 8 | CreateObj | 170,401 | 78 | `01 02 04 20 7e (91447)`; `01 02 29 04 20 (22640)` |
| 51 | FillSector | 106,180 | 8 | `36 35 19 (34480)`; `36 34 35 33 19 19 19 19 (28500)` |
| 1 | CreateNPC | 99,560 | 577 | `01 02 2b 04 20 (12320)`; `01 02 42 04 20 (11480)` |
| 50 | TriggerPatch | 77,586 | 6 | `01 2a (68395)`; `01 2a 2a (7931)` |
| 67 | SetVar | 67,037 | 2 | `01 0b (56593)`; `01 01 (10444)` |
| 48 | CreateTrigger | 64,890 | 3 | `01 28 27 (63333)`; `01 28 2c (1337)` |
| 66 | ELSEIF | 60,624 | 57 | `7d (14256)`; `48 (12366)` |
| 26 | Text | 51,253 | 2 | `1e (40473)`; `7c 1e (10780)` |
| 53 | QuestBook | 51,041 | 2 | `0b 0b 01 (51030)`; `0b 0b (11)` |
| 108 | AddPoolPos | 45,231 | 2 | `01 0b 0b (44682)`; `01 0b 0b 0b (549)` |
| 60 | SetButton | 40,307 | 1 | `01 01 (40307)` |
| 23 | DefPos | 38,774 | 4 | `01 0b 0b (29590)`; `01 01 (3132)` |
| 78 | FillChest | 37,329 | 5 | `53 (36556)`; `02 (585)` |
| 94 | MouseEvent | 35,208 | 16 | `1e 04 41 10 (26306)`; `1e 04 0c 0d 41 10 (3459)` |
| 3 | SetNPCState | 33,167 | 186 | `01 09 (6895)`; `01 0a (3980)` |
| 58 | IF | 31,463 | 50 | `48 (6422)`; `49 (5111)` |
| 59 | ELSE | 31,256 | 2 | `- (31245)`; `4a 12 11 01 (11)` |
| 62 | NOP | 29,734 | 1 | `- (29734)` |
| 22 | CallFunktion | 28,551 | 3 | `01 (27835)`; `01 01 (705)` |
| 63 | QuestKompassPos | 23,606 | 5 | `01 0b (10876)`; `0b 0b 0b (9786)` |
| 114 | Waffenpool | 18,120 | 1 | `0b 02 (18120)` |
| 86 | SetIcon | 18,082 | 2 | `01 0b (15738)`; `0b (2344)` |
| 64 | QuestKompassObj | 17,564 | 2 | `01 0b (15794)`; `01 (1770)` |
| 20 | TriggerQuest | 16,978 | 1 | `0b (16978)` |
| 118 | SetClientVars | 16,830 | 2 | `0b 0b (10494)`; `0b 0b 0b (6336)` |
| 31 | Dialog | 16,830 | 1 | `1c 16 (16830)` |
| 122 | SolveTime | 16,830 | 1 | `1c 38 (16830)` |
| 32 | Belohnungen | 16,830 | 1 | `1c 0b 0b (16830)` |

## Quests

Read off the client with Ghidra 2026-10-19 (#117), **confirmed** by code unless marked.

- The quests are records of 0x124 bytes in a vector at `0xAABF18` (index 0 unused), the first u32 the quest's number.
  As it loads a tree (`0x465690`), the client finds each quest's section functions by name in the function table (`0xAAB708`, entries of 0x54 bytes: the name, `+0x40` the offset, `+0x44` the size, `+0x48` the owning quest, `+0x50` done; `0x460590` finds one) and keeps their indices: `+0x10C` `QIS_Trigger<n>`, `+0x110` `QIS_OnEnter<n>`, `+0x114` `QIS_OnSetUp<n>`, `+0x118` `QIS_OnExit<n>`, `+0x11C` `QIS_OnLose<n>` (the compiler `0x45A370` names a quest's sections so).
  `+0x120` holds the quest's flags: 1 set up, 4 entered, 2 finished, 8 while it is set up.
- `0x46BA90` runs a function by its index: none for the index 0, −1 or one out of range, nor for a function whose quest is not set up; it answers whether the body ran to its end, so an empty body runs through.
- `SetUpQuest` (21, `0x48F7F0`, `0x46DB20`) sets a quest up once: the flags 1 and 8, its `OnSetUp`, its trigger (the answer ignored), then the flag 8 off; it does not enter it.
- `TriggerQuest` (20, `0x48F030`) needs the flag 1, runs the quest's trigger, and if that runs through enters the quest (`0x46C160`): unless the flag 4 is set, a quest log entry, the flag 4, then the quest's `OnEnter`.
- `ExitQuest` (15) and `LoseQuest` (54) (`0x48EE20`, `0x46CBE0`) need the flag 4 and not 2, run `OnExit` or `OnLose`, and set the flag 2; `UnTriggerQuest` (77, `0x48E600`) takes the flag 4 off and the log entry out.
- A new game runs its tree's start script through, record by record (`0x46F9B0`, `0x470AC0`, from `0x60C2DC`); a saved game's load does not.
  The Seraphim's start script sets quest 1 up and triggers it at once, and its `QIS_Trigger1` is empty: the world starts with quest 1 entered and its `QIS_OnEnter1` run, which creates the novice (`CreateNPC`, creature 679, at the `DefPos` `novizin1`) and sends her to the chapel's benches (`NPC_Goto` to (3237, 2514, 0)).
  The other heroes' start scripts trigger a quest 1 of their own the same way, but the Daemon's, which triggers none; the Underworld trees' start scripts trigger none.
- `NPC_Goto` (72, `0x49E210`) takes the NPC's name, then a cell (`0x04`, through `0x6224B0`) or raw coordinates (`0x20`), and an optional flag (`0x66`): it does not move the creature, but posts it an event of walking there (a `cEvent_creature`, id 4), starting a motion first; `Teleport` (46, `0x491D40`) places the creature at once.
- `QuestKompassObj` (64, `0x49AC80`) points the quest compass at the named object: the quest log alone.

`opensacred-rules` keeps this state apart from Luau and from the engine (#724, `opensacred_rules::quests`): the quests' flags and the transitions above, each naming the section functions it runs for the scripts' runtime to run and taking its answer; the variables by name, kept to their first 31 bytes; and the dialogues' table below, its first entry doubled; all of it serializable for the saves (`quests::Scripts`).
`opensacred-game` follows the quests' start in its own code until #726 moves it onto this (docs/decisions/2026-10-19-quest-npcs.md).

## The community's quest defects

Read 2026-10-25 (#356) by a subagent of the session on the converter's Luau of every tree and four handlers of the client; **confirmed** by code and on the scripts unless marked otherwise.
The quests are named by their numbers, the community's names beside them only to identify them; the day counts that their strings state were read on the English `global.res` and are given as numbers.

### `SetDrop`'s one entry per creature

- `SetDrop` (137, `0x4AFFF0`) keeps at most one entry per creature type in its list.
  With two types, the creature is the one whose category (`+0x2E`) is 2, 3 or 32 (`0x426610`).
  It finds the list's entry by the creature alone, and with a chance of 0 removes it, else overwrites its chance and its item; only where no entry names the creature does it append one, and a chance of 0 then appends an entry of chance 0, which drops nothing.
- The quests' collecting lists (`SetOnCollect`, `+0x7620`, `0x4AAAE0`) are kept by item, so each quest still counts its own item; only the drop is shared.
- Each quest of the pairs below gives, in its `QIS_OnEnter<n>`, a `SetOnCollect` for its item and a `SetDrop` of its creature and item, and in the function that its count runs (`ot_<n>` or `oc_<n>`) the same `SetDrop` with the chance 0.
  So of two quests that name one creature type, the one entered later overwrites the earlier one's entry, whose item stops dropping; and as the later one reaches its count, its chance 0 removes the creature's entry, so the earlier one's item never drops again and that quest cannot be finished by drops.
- Over all 20 trees exactly five creature types are named by `SetDrop`s of more than one quest, the five pairs that the community reports:

| Quests | Creature types | Items and chances |
|---|---|---|
| 15015 (Stolen Iron Bars), 10259 (Collect Orc Rings) | 38 | 6005 at 75 %, 4931 at 65 % |
| 15052 (Ornate Staff of Rituals), 15028 (Books of Faith) | 312 | 6025 at 75 %, 5162 at 75 % |
| 15067 (Antidote), 15072 (Special Flavor) | 99 | 1355 at 75 %, 1331 at 75 % (the item named first, the types swapped by the handler) |
| 7618 (A Brother in Need), 7620 (New Potions) | 178 to 181 | 5142 at 80 %, 1330 at 80 % (four records each) |
| 15080 (The Epidemic), 7630 (Salvation of the Doomed) | 94 | 1366 at 75 %, 4937 at 50 % |

The first three pairs are in all 20 trees, the fourth in the ten Underworld trees and `bin/NetScript`; quest 15080 is in both campaigns' trees and 7630 only in the Underworld's, so the last pair clashes there alone.
- The defect is the scripts' data, two quests on one creature type, meeting the handler's one entry per creature, which the client makes so; whether the designers expected one quest at a time is a **hypothesis**.
  No operand of the data keeps both items: a repair of the data could only make the drops come one after the other, and a list keyed by the creature and the item would be the VM's, a departure from the client.

### `SetTimer`'s minutes and the calendar

- `SetTimer` (56, `0x4982D0`; the walker's case at `0x475A2C`) takes a function's name (`01`), the timer's id (`0B`, at least 0) and a time above 0 (`38`), and stores the id, the time and the moment it is set, `((((year·12 + month)·30 + day)·24 + hour)·60 + minute)` of the clock; a timer of the same id is replaced, which starts it anew (`0x417710`).
- At each minute that the clock completes (`0x416F00`; not at the minute that ends a day), the first timer whose time is at most the clock's count less its moment runs its function, unless the function's quest is finished (the flag 2), and every timer of its id is removed.
  So `38` is in minutes of the game's clock: 1,440 a day, 4,320 three days, 10,080 seven (a day of the game an hour of play is docs/re/daynight.md's **hypothesis**).
- The count takes months of 30 days and years of 12, but the date's carry (`0x416DA0`) makes eleven months of 29 days and the fifth of 34.
  So the count leaps a day ahead at the end of each other month, falls four days behind at the end of the fifth, and 31 days ahead at the end of the year: a timer across such an end runs a day early, four days late, or at once (not yet seen in the running original).
- The `IF` tags `0x49` and `0x4A` test that a variable's bit is set or clear, both false where the variable is missing (`0x4987B0`).

The quests whose time the community reports:

- **1202 (Magical Thistle)**: `QIS_OnEnter1202` sets `zeit1202` for 4,320, three days; it sets bit 4 of the variable `1202`, takes the target NPC's health to 0, writes the failure in the log and loses the quest, and the giver's dialogue at bit 2 deletes the timer.
  The giver's start dialogue (`R12NQ02_NPC_Auftrag_Qstart`) states four days, the log's start (`R12NQ02_Log_Qstart`) and the giver's dialogue while the quest is open (`R12NQ02_NPC_Auftrag_Qoffen`) three: the timer agrees with two of the three, and the report is the start dialogue's number.
  The same in the 11 trees that hold the quest.
- **1803 (Kalim-Khor)**: `QIS_OnEnter1803` sets `zeitum1803` for 4,320 where bit 9 of `1803` is set, which `take1803` sets as the item 6068 is picked up before the quest is taken, and for 10,080, seven days, where it is clear, the usual way from the giver; `take1803` does not start it again once the quest is open, and `exit1803` deletes it.
  Only the giver's start dialogue (`R18NQ03_NPC_Auftrag_Qstart`) states a number, three days, so the usual way lasts seven days where three are stated; the report's "about 8" being the seven read loosely, or the count's leap at a month's end, is a **hypothesis**.
  The same in the 11 trees that hold the quest.
- **1102 (Compromising Situation)**: `QIS_OnEnter1102` sets `Zeitum1102` for 1,440, one day, which writes the failure in the log, sets bit 3 and loses the quest; `extquest1102` deletes it.
  The giver's start dialogue (`R11aNQ02_NPC_Auftrag_Qstart`) states one day, so the data does not explain a limit of three days never announced, nor can the count's drift, at most four days at the fifth month's end.
  The quest beside it, 1101 (Blackrobes), has a limit that no string of it announces (`QIS_OnEnter1101`, `Zeitum1101` for 1,440); that the report confuses the two, or that a save and a load set the date back, is a **hypothesis**, and the original is to be checked: 1102 taken, the date noted, and `Zeitum1102` seen to run.

### The handlers that these quests rely on

Read 2026-10-25 (#356, a second pass of a subagent of the session, on the Luau of the 20 trees, their bodies compared across the trees, and the handlers below); **confirmed** by code unless marked otherwise.

- An `IF`'s tags (`0x4987B0`) are tested in their order, all of them to hold: `0x48` holds where a variable equals the number, `0x6D` where it is greater and `0x6E` where it is less, each false where the variable is missing.
- `0x3A` takes a creature (`HERO` for the local hero) and an object's name, and holds where the object is in that creature's inventory (its 256 slots, compared by the object's key).
- `0x40` holds where the named creature is in the hero's list of followers (`+0x39C`), or is the creature that `0x55C630` answers (not read).
- `QuestBook` (53, `0x496080`) takes the text (`0x01`), the quest's number and the line's index (the two `0x0B`s), and finds the quest's entry in the log (the vector at `+0x424`, entries of 0x174 bytes, the number at `+8`): an index divisible by 10 sets the entry's title (`+0x24`), another writes the text into the first free one of its ten lines (`+0x28`).
  It tests none of the quest's flags, so a line still goes into the log of a quest finished or lost, and nowhere for a quest never entered, whose entry its entering makes (`0x46C160`); an eleventh line is dropped.
- `SetTimer` keeps the last `0x0B` as the id and `0x38` as the time, neither set before the tags are read (`0x4982D0`), so a record without `0x38` carries whatever time the stack held.

### Moorbrook

- **9009 (Whisky Supplier)**: `QIS_OnEnter9009` creates the barrel 17542 (item 6081), whose function `rg22_goodie1_start` (`0x41`) sets bit 25 of `goodies`, and the win is the target's branch on that bit.
  The target's dialogue, `Dialog:rg22_goodie1_ziel`, tests `0x3A` of the barrel 17713 first, which quest 2203 gives as its reward (`exit2203`), and offers to buy it; while the hero carries it the branch of bit 25 is never reached, and selling or drinking it frees it (#935).
- **2203 (Pure Water)**: `Region22Init` creates its six objects (item 6081, `0x41` `obj2203`) and its hostile target 17260 (`0x05` `tot2203`).
  `obj2203` counts the variable `2303a` whatever the quest's state, setting bit 3 of `2203a` at a count of 6, or bit 3 of `2203`, the win, where bit 3 of `2203b` is set; `tot2203` tests bit 3 of `2203a` first, which sets the win, ahead of the branch that triggers the quest where the target is beaten before it is taken.
  So with the objects taken before the quest and the target killed after them, the giver offers the win with the quest never entered: its `QuestBook` lines go nowhere, `ExitQuest` does nothing, and `QIS_OnExit2203` never runs (#936; that the objects can be taken first is unconditioned in the data, not seen in play).
- The count `2303a` is also set by `Dialog:ziel2303` (bit 2), a dialogue of R23NQ03, which has no quest file: if `SetVarBit` adds 4 to a count below 4 (**hypothesis**: by its name, not read), one or two objects suffice, or the count passes 6 without `obj2203`'s equality holding, and the quest cannot be won (#943).
- The same in the 11 trees that hold the two quests.

### The log contradicting itself

- **1303 (Goods Messenger)**: `zeit1303`, set for 4,320 minutes by `QIS_OnEnter1303`, writes the failure line and then `NQ_Log_Qend`, the generic completion line, and sets bit 3, on which the giver's branch writes `NQ_Log_Qfail` and loses the quest.
  The target 17177 keeps its dialogue `ziel1303`, which tests nothing: after the failure it still writes `NQ_Log_Qend`, and its button `exit1303` gives the reward and exits the quest that the timer did not lose (#937).
- **7619 (Death to the Dead)**: `QIS_OnEnter7619` hires its follower 19545 and gives him the death function `od_threepwood_7619`, which writes the failure line, shows the failure and loses the quest, and which nothing takes away.
  `od_7619` runs at each death of the group 761901 and, the whole group dead, shows the completion while the quest stays open until the follower's button `btn_ok_threepwood_7619_c`: his death between the two loses the quest after the completion, and after the button writes the failure line after the win, `LoseQuest` doing nothing to the finished quest (#938; his death as a follower being likely is a **hypothesis**, what `0x39` does to a follower not read).
  The same in the 10 Underworld trees and `bin/NetScript`.
- **5005 (Pony Express)**: at the last leg the mage 17277 takes the dialogue `dlg_zwischenpe`, whose branch on bit 5 runs `exitpe`; `losepe`, which `timepe` starts 5 minutes after a leg's time runs out, writes the failure lines, loses the quest and removes the stations 17763 to 17766, but does not give the mage his dialogue back, which `exitpe` does (`NPC_Dialog_Magician`).
  So after a failure on the last leg the mage still offers `exitpe`: the win line, `NQ_Log_Qend` and the reward, `ExitQuest` doing nothing to the lost quest (#939).
  The same in all 20 trees.
- **10255 (Arachnophobia)**: each of its seven spider areas spawns a spider and calls `LoseQuest_10255`, which writes the line `DQ_10255_LOG_LOSE2` and gives the giver the dialogue `dlg_10255_Lost`, but has no `LoseQuest`, so the quest stays entered; its restart (`btn_accept_10255_REstart`) and its win (`btn_ok_10255_sieg`, which exits it) write no line.
  So after one failure the log's last line stays the failure line, even once the quest is won (#940; that the report means this line is a **hypothesis**); under its siblings' names (`DQ_<n>_LOG_SIEG`, as 10258 and 10261 write) the file has no line of its win or restart.
  The same in all 20 trees.
- **1401 (Ancarian Snow)**: `QIS_OnEnter1401` sets `zeit1401` (the id 1401) for 1,440 minutes, whose one record writes `NQ_Log_Qend`, and only `OMO1401tgilde1401` deletes it: the trigger `tgilde1401` with `0x40` naming 17180, the hero entering the area with him among its followers.
  So a day after the start without that, the log says the quest is completed while it stays open, with no bit, reward or `ExitQuest`; the follower's death (`tot1401`) writes the failure line and leaves the timer running (#941).
  `tot1401`'s `SetTimer` (`hinterhalt1401`) carries two `0x0B`s, 1401 and 4320, and no `0x38`: the id 4320 with a time never set, where 4320 meant as the time, which would have replaced `zeit1401` at his death, is a **hypothesis**.
  The same in the 11 trees.
- **1001 (Captain Pigface)**: `QIS_OnEnter1001` writes `R10NQ01_Log_Qziel01`, the line of the kill, and points the compass at the target 17041 where bit 20 of `1001` is clear (`0x4A`), as it is while the target's item 6046 is not taken (`take1001` sets it): a quest taken the usual way logs the kill at once while the target lives, and one triggered by the item's taking gets no line of the kill (#942).
  The same in the 11 trees.
- In the 11 trees, quest 1401's soldier 17181 names the death function `attack1401`, which no tree defines, while `attacke1401` is defined and named by nothing; the 13 smugglers of `Region13Init` (the groups 14011 and 14012) name `0x60` `schmugler1402`, while the function is `schmuggler1402` (#944).

Not read: how the log window shows a quest as completed or failed, by its lines or by its state; what `0x39` does to a follower; when an object's `0x41` function runs (**hypothesis**: as it is picked up); and none of these was seen in the running original.

### The NPCs that later events remove

Read 2026-10-25 (#356, a third pass of a subagent of the session, on the Luau of the 20 trees, their bodies compared across the trees, and the handlers below); **confirmed** by code and on the scripts unless marked otherwise.

- `SetGroupState` (73, `0x49E760`; the walker's case at `0x475ADB`) with `0x51` posts each creature of the group that its `0x11` names (`0x4C4E40`) the event `0xC` and destroys it (`sdk_DESTROY`) where `0x549080` holds (not read); `DelNPC` (55, `0x497F80`) destroys the creature that its `0x01` names and takes the name out.
  Whether either runs the creature's death functions (`0x05`) is not read.
- `RegionChange` (116, `0x4ABB60`, applied by `0x4ABFE0`) takes pairs of types (`0x02`, from and to) and a number 0 to 49 (`0x69`), which it compares with the byte `+0xD7` of the creature's sector (`0x622D50`), the area (keyx `+0x2C0`, docs/re/sound.md's **hypothesis**).
  It skips an object of another area, a trader (`+0x200` bits `0x4007000`), or one of `+0x2B7` bit `0x08`, which `0x6B` above 0 sets; the others take the pair's second type, the record's flags word (keeping `0x2000`), and with `0x08` a hostile side (2, or 14 where `+0x1F0` is 7) or with `0x0E` a peaceful one (3, or 7 with the guards' mode `0x4000`), their items of the pairs' types swapped too.
  It reads no `0x6B` of its own, so the `6B:0` of quest 55's three records does nothing.
- **301 (Orcs!)**: `Region3Init` creates its farmer 17033 and his daughter 17032 with four cows in the group 3012, in area 3, whose dialogues hold the win once its orcs (the group 301) are dead.
  `Init55`, the start of quest 55 (the button of `Dialog:DlgPrinzValor55`), removes the groups 3012 and 301 with `SetGroupState` `0x51`, testing nothing of 301: an open 301 whose orcs are beaten but whose farmers are not yet spoken to stays entered and cannot be won; with its orcs alive its timer still loses it (#946).
  It removes quest 302's cattle (the group 302) and robbers (3021) too, but not its giver 17029, and 302's ending runs through its death functions `tot302` and `tot2_302`, so an open 302 stays entered unless the removal runs them (**hypothesis**, not read).
  That the community's "start of act 2" means quest 55's start is a **hypothesis**.
- **501 (Nucquam Astralis)**: `Region5Init` creates its Lady 17070 and Lord 17071 in the group 501, in area 11, through whom both its wins go.
  `NimmtDaemonenhorn`, the end of quest 55 (`Dialog:Cinematic2`), removes the groups 501, 551 and 50013, testing nothing of 501; its ghost 17069 stays, but offers the win only once the group 501 is dead (`tot501`), so an open 501 cannot be won after quest 55 unless the removal runs `tot501` (#947; not read).
- **1702 (Robber Baron)**: `Region17Init` creates its target 17215 hostile (creature 296, with the death function `tot1702`, which drops the item 6065 that the win needs), in area 31.
  `BadToGood`, which only quest 72's `ScriptAnimShareef` calls (The Forge of the Ancestors), has a `RegionChange` of area 31 with the pair (296, 297) and `0x0E`, which makes him a creature 297 of the peaceful side, testing nothing of 1702, so that the item can no longer be won by fighting him (#948).
  This holds where `Region17Init` ran before the forge (when a region's `Init` runs is not read); of the named `CreateNPC`s of the Seraphim's tree he is the only quest target of area 31 so changed, and `BadToGood` makes the bounty targets 18708 and 18712 of `Kopfgeldauswahl` peaceful too (that it blocks an open bounty is a **hypothesis**).
- **670 (Glubba-Jar)**: `Init81` takes the dialogue of Glubba 18308 away and his health to 0 only where bit 1 of `670` is clear, which only `QIS_OnEnter670` sets, so he dies only where 670 was never entered: the data leaves no quest taken unfinishable, and the report fits a quest declined or never offered (**hypothesis**).
  But `QIS_OnEnter670` gives him the death function `lose670`, which is empty, so his death as a follower leaves 670 entered with no way to finish (**hypothesis**: that `0x6B` 0 lets him die), and `QIS_OnLose670` writes the completion line on a loss (#950).
- **1202 (Magical Thistle)**: `ok1_602`, the button of quest 602's `Dialog:ziel602` once its knight 17136 is beaten, walks 17136 once, and then names 17163 in its three `NPC_Goto`s, its `Partikel` and its call of `ritterweg_602`, which deletes 17163 (`DelNPC`).
  17163 is quest 1202's target (`Region13Init`, in area 22, some 1,470 cells from the knight), whose dialogue holds 1202's win and then a trade, and nothing in 602 tests 1202: beating the knight before 1202's end deletes its target, and the quest stays entered, neither won nor lost, once its giver has deleted its timer; after its end, the trader is gone (#949).
  That 17163 is 17136 with two digits swapped is a **hypothesis**: the function's name and its first `NPC_Goto`, which names the knight, support it.
- The functions above are in the 9 trees of the Sacred campaign (`Init55`, `NimmtDaemonenhorn`, `ScriptAnimShareef`, `Init81`) and the side quests in 11 (those and `netscript`, `addon-netscript`).
- The group number 5001 is named by quest 501's 13 ghosts, quest 5001's camp and quest 5002's test, attack and removal, so 501's ghosts would count in 5002's test and fall to its removal if the groups' numbers are global (**hypothesis**, `0x4C4E40` not read); and `QIS_OnEnter501` creates again the four ghosts of the group 551 that `Region5Init` made, so `exx501` waits for eight (#951).

Not read: when a region's `Init` runs, whether a removal or a `DelNPC` runs the death functions, what `0x549080` and the event `0xC` do; none of this was seen in the running original.

### Order traps

Read 2026-10-25 (#356, a third pass of a subagent of the session, on the Luau of the 20 trees, their bodies compared across the trees, and the handlers below; the community's guide read once for the traps' details, nothing taken from it); **confirmed** by code and on the scripts unless marked otherwise.

- `SetObjState` (2, `0x48AE90`) resolves its `0x01` name through the scripts' map of names (`+0x7668`, `0x464BD0`), which answers the id kept for the name without testing that its object still exists, and else scans the objects by their keys.
  For an object found, `0x07` clears bit `0x8000` of its trigger's state (the object's `+0x34`, `0x634150`, the word at `+0x0A`) and `0x06` sets it; `0x0F` and `0x10` set and clear bit 0.
  `CreateTrigger`'s `0x2C` (`0x496520`) clears bit 0 and sets `0x8000`, and `0x2D` clears `0x8000`: a trigger made with `0x2C` is closed and locked.
- On a name that the map does not hold, `SetObjState` keeps the state in a list by name, the first kept and a later one dropped; on a name whose id has no object any more it does nothing, neither applying nor keeping the state.
  That `CreateObj` applies a kept state as it makes the object is a **hypothesis**, which the quests' usual course requires.
- `CreateObj` (`0x485C10`) sets bit `0x80000000` of an object's `+0x14` while an `Enter` runs (the interpreter's byte `+0x12`), and the sector's exit (`0x4954C0`) destroys such objects (`sdk_DESTROY` from `0x495E7B`), whose cleanup (`0x463A70`) erases the key from the map of names only where `+0x14` bits `0x90000000` are clear (`0x463AE4`).
  So a door that an `Enter` made keeps its name, mapped to an id that no longer exists, once its sector has left the 3 × 3 (**hypothesis**: the exit's own calls on the map by the key, from `0x495C7D`, not read).
- `sdk_ENGINE_TP` (`0x54D9D0`) does not place a creature of `+0x14` bit 2 whose slot 18 (`+0x1EC`, `0x556F50`) holds a horse (`0x4266F0`) or the type 1879: it teleports that instead and answers 1, whatever the horse's teleport answered (`0x54DBAC` to `0x54DC34`).
  The horse's teleport answers 0 and moves nothing into a dungeon's sector (`0x63D1C0`, `0x622E20`), else the horse brings its rider; `Teleport` (46, `0x491D40`) tries other cells only on an answer of 0, so a mounted hero sent into a dungeon stays where he is, and nothing is logged.
  That `+0x14` bit 2 marks the players' heroes is a **hypothesis**; slot 18 holding the mount is **confirmed**, the dismount `0x551E40` emptying it.

The quests:

- **2411 (Scroll of Mhic'Dar-Mon)**: its door `R24NQ11_Tür` (item 5700) is made by `Sector38094Enter` on the trigger 519, which `Sector38094Init` makes locked, and unlocked by name by `QIS_OnEnter2411`, at the giver in sector 54,56, and by `OMO-1Goodie_02_rein`, the cave's entrance, where bit 1 of `2411` is set and bit 2 clear; the scroll behind it is made by `QIS_OnEnter2411` whatever the order.
  The quest taken first, its unlocks meet a name never seen and are kept for the door that the next `Enter` makes; the cave entered first, the name is kept for a door already destroyed, both unlocks are lost, and the next door is locked (#952; a **hypothesis** where the door's `Enter` runs after the records of the function that teleports, as the 3 × 3 is walked when the view's centre changes, docs/re/spawning.md).
- **2201 (Spider Monster of Moorbrook)**: the same pattern: its door `R22_NQ1_Tür1` (item 5707) is made by `Sector94054Enter` in the dungeon R22.1b on the trigger 1099 that `Sector94054Init` locks, and its only unlock is the bit-11 branch of `postchange2201`, as the item 6079 is taken from the giver's corpse, on the path that answers the target's dialogue with `change2201`; the path of `nochange2201` is won at the giver without the dungeon, as the community reports declining to avoid the trap (#952).
- In the Seraphim's tree the scripts unlock by name 38 objects that an `Enter` makes; each meets the same reading where its unlock can run after its sector was entered and left, and only 2411 and 2201 are reported.
- **2002 (Panem et Circensis)**: the start script sets bit 0 of `2002`, and the building's exit `OMO-1tpraus20.2c` runs `monster2002` where bit 30 is clear and bit 0 set, then sets bit 30 and clears bit 0, testing nothing of the quest.
  Before the quest is taken `2002a` is missing, so `monster2_2002` makes no creature, and Vladimir's `Dialog:auftrag2002` then holds no branch, bits 0 to 4 all clear; only `ja2002` (bit 2) and `neu2002` (quest 5001's dialogue, where bit 1 is set) set bit 0 again, so the quest can never be taken (#953; not seen in the running original).
- **10254 (Abducted)**: `QIS_OnSetUp10254` makes the note (item 4098) with the function `ot_Brief_10254` and no `0x61` or `0x8E`, unlike its sibling 10257's note; the function tests nothing, so each run makes the giver anew with its start dialogue, and accepting makes the pirates.
  The giver's death function loses the quest only once it is entered (`LoseQuest` needs the flag 4), so a giver killed before is replaced by picking the note up again, which the community reports as the way out rather than a trap; the function running at each pickup is **community-reported**, and a second giver of the same name while the first lives a **hypothesis**.
- **1404 (Rescue of a Noble Lady)**: `QIS_OnEnter1404` gives Zaphod 17190 the death function `tot1404`, which makes the key 17193 (item 6061) at `CPOS:res:17190`, and its `take1404` unlocks the two doors of `Sector38030Enter` in the same sector, so 2411's pattern does not apply; the data does not depend on where he dies.
  That the key lands on a cell or floor out of reach, or is not made, where the corpse lies on the veranda is a **hypothesis** about `CreateObj`'s placement at a creature's cell; the original is to be checked.
- **5003 (H'Runga'zz)**: `preritual5003` sets the trigger `omo_ritualstart5003` and takes the shaman's dialogue away; `OMO5003omo_ritualstart5003` deletes the trigger, teleports `HERO` to `pos_zielwaldgott5003` in sector 15,85, a dungeon by `world.ron`, makes the god there and writes the log's line.
  A mounted hero is not moved, yet the teleport answers success, the trigger is gone, and only the god's death function gives the shaman's dialogue back, so the quest is stuck (#954; that the trigger fires for a mounted hero is **community-reported**).
  `Dialog:dlg_sucher5003`'s decline button names `ablehnen5000`, which no tree defines, while `ablehnen5003` exists and is named by nothing, so a declined seeker keeps bit 2 of `5003`, and a quest later taken from the shaman takes the seeker's branch (#955; that a button naming a missing function does nothing is a **hypothesis**).
- 2411, 2201, 2002 and 1404 are the same in the 11 trees that hold them; 5003, 10254 and the sectors' functions above in all 20.

To check in the original: 2411's cave entered before the quest and the trigger 519's state; the same for 2201's dungeon before `change2201`; Zaphod killed on the veranda and his key's cell and layer; 5003's altar stepped on mounted.

### The log's quest items (10260)

Read 2026-10-25 (#356, a fourth pass of a subagent of the session, on the Luau of the 20 trees, their bodies compared across the trees, and the handlers below); **confirmed** by code and on the scripts unless marked otherwise.

- The local hero's inventory entry keeps a list of main quest items, pairs of an item type and a key of 8 bytes each, which only a reset (`0x5D8520`, from `0x55D260`) empties; `0x6B0E00` walks it and shows an entry of the key 0 or −1 by its item type's name (**hypothesis**: the log's page of quest items).
- `cInventoryEntry::addMainQuestItem` (`0x5D8280`) appends a pair unless an entry already has its type, or any entry has the key 0, and otherwise only logs that the item is there already: once an entry of the key 0 is in the list, no later quest item is ever added (that the add meant to compare with the new key, as its sibling `0x5D8490` does, is a **hypothesis**).
- Every caller but one passes the object's key from `sdk_REFKEY` (`0x44A1C0`), the key of its script name (`+0x3C`), else its item type's name key (`+0x20`, `0x428830`): the pickup of a quest item (`0x558080`, at `0x558267` and `0x55923C`) and the collecting lists (`0x4AAAE0`, at `0x4AAEFB`), each destroying the picked object after the add, so that a picked quest item lives only in the list.
- The exception is `CreateObj` (8, `0x485C10`): with `0x61` or `0x62`, an item type and no `0x8E`, it makes no object and gives the item straight to the local hero's list (`0x4868F6`), with the key of the record's first `0x01` name, or 0 where it names none (`0x4868DB` to `0x4868EA`), never the type's name key.
  With a `0x41` it then runs that function (the name kept behind the prefix `Take:`), unless it runs inside that very function, where it makes the object in the world instead; with `0x8E` it makes the object in the world, marked as a quest item (`+0x14` bit `0x800`); inside a function whose quest is finished or not set up it does nothing (the running function's index `0x94E07C`, which `0x46BA90` sets and resets).
- `0x3A` (`0x4987B0`) holds where the hero's list holds the named key, where the name maps to an existing object whose type is in the list or any entry of the list has the key 0 (`0x498860`, `0x4988B4`), or where the creature's 256 slots hold an object of that key.
- **10260 (The Riddle in the Desert)**: `CreateVasen_10260` makes six vases; the right one's function `fillchest_chestQJ_03` gives the item 4609 with `0x61` and no `0x01`, so the entry (4609, 0) enters the hero's list, and the other five lose the quest, so the entry comes with the win alone (that a chest's `0x41` runs as it is opened is a **hypothesis**).
  From then on no quest item of any quest reaches the list, the community's "the log stops recording quest items" (#957).
  The record is identical in all 20 trees and the only one of its kind: of the 1,982 `CreateObj`s of `0x61` or `0x62`, 42 name nothing, 20 of them this one and 22 those of 2501 and 502, which carry `0x8E` and so take their type's name key as they are picked up (that their types have a name key is a **hypothesis**).
- The quests that break test their items with `0x3A`:
  - **154 (Where is the Blacksmith?)**: `drop154` makes 17630 (item 1749, `0x41` `take154`, `0x61`, `0x8E`); picked up, `take154` runs, but the add is refused and the object destroyed, so `Dialog:ziel154` never holds `0x3A` of 17630 and the quest cannot be finished.
  - **9011 (Jacquaire's Revenge)**: its giver waits for bit 5 of `154`, which only `exit154` sets, and its own item 17684 is refused the same way.
  - **402 (Whispers in the Wind)** and **601 (The Painting)**: `take402` and `takeit601` give 6029 and 6033 with `0x61`, which never enter the list, so their givers' `0x3A` branches are never reached.
  - **204 (The Book of Wisdom)**, which the community does not report, ends without its reward: `exit204` gives the class's item only where `0x3A` of 17640 holds.
  - In the Seraphim's tree these four names are the only ones that a `0x61` record makes and `0x3A` tests; the quests and their dialogues are the same in the 11 trees that hold them.
  - **9006 (The Unknown Warrior)** has no item of its own, and its successor 9007's items 17525 to 17528 are world objects counted by their own function; but with an entry of the key 0, `0x3A` of each holds while it merely lies in the world, so its giver's dialogue takes an item's branch early and deletes with `DelOBJ` an item never taken, which is then never counted (**hypothesis**: that this is the community's "last stage", which its guide does not tie to the riddle, and that `DelOBJ` destroys an object lying in the world).
- Raven Rock 1.3b reports fixing it in the interpreter, the fix applying as an old save loads (**community-reported**); that the list is saved with the hero is a **hypothesis** that fits it.
- So the defect is the interpreter's, set off by one record of the data: a repair of the data names the record (`res:4609`, as 402 and 601 name their items) in all 20 trees; a repair of the VM falls back to the type's name key on the `0x61` path, refuses only a repeated type or key, and keeps an entry of the key 0 from answering `0x3A`, which also mends a state that already holds one.

Not read: the engine's event that `CreateObj` posts before the add (`cEvent_engine` 6), how the log window draws the list, the pickup's other branch (`0x55923C`) beyond its add and destroy; to check in the original: 10260 won through the right vase, then any quest item picked up, the client's log expected to say that it has the item already.

### Spawns and markers

Read 2026-10-25 (#356, a fourth pass of a subagent of the session, on the Luau of the 20 trees, their bodies compared across the trees, and the handlers below); **confirmed** by code and on the scripts unless marked otherwise.

- `CreateNPC` (`0x482510`) fails and logs where the record has no creature or no cell (`0x485A35`); for a name of its `0x01` that the map of names holds (`0x464BD0`, at `0x4830E4`) it makes nothing new, logs the name as given twice and teleports that creature to the record's cell, making a new one only where that creature is gone or has `+0x14` bits `0x30`.
  Into a sector not loaded (its `+0x30` bit 16 clear) it makes the creature with no place (`sdk_OBJ_CREATE_NPOS`) and then places it (`0x5FEE50`); during an `Enter` an unnamed creature that is not hostile is made only where the byte `0x182EE78` is set (`0x482DF0`); a new creature's type is replaced through a list per area at `+0x1050` (0xAC bytes each, `0x482F41`; **hypothesis**: filled by `ReplaceSpawn` or `RegionChange`); and the creature 86 at (3275, 4108) on layer 2 loses the hostility of its `0x08` (just after `0x482DA2`).
- `CreateObj` handles a name already held the other way round: it logs a warning, destroys the older object (`0x4860E7`) and makes the new one.
- `RndVar` (105, `0x49D450`) sets its variable to `rand() % (high − low + 1) + low`; `DefPos` (23, `0x478780`) keeps the name and up to four numbers, the first two x and y, a name defined again replacing its entry; `SetBaseTrigger` (4, `0x4819A0`) orders the two corners itself, takes the layer from the third number of `0x0C` (0 to 16, else 0), and makes a box of one cell without `0x0D`.
- `GroupIsDead` (24, `0x48C860`) answers dead for a group it does not find, skips members gone or out of the group, and answers alive at the first living member wherever it stands, keeping the nearest within 25,000 at `+0x7704` (its use not read).
- A region's `Enter` (`0x492C90`) and `Exit` (`0x493040`) take the region from the hero's sector through its area by the table at `0x95CBFC` (the area 39 the region 23, 15 the region 24, 10 the region 6), as sectors enter (`0x4939F0`) and leave (`0x4954C0`) the 3 × 3, an `Enter` once per entry (its region's byte `+5`, cleared by the `Exit`); the regions' cache of names (`0x4C6F60`) holds two functions' indices, both run (**hypothesis**: how the converter's `Region<n>Enter#2` duplicates run).
- **9512 (Extraordinary Demon)**: `QIS_OnSetUp9512`, from the start script of all 20 trees, rolls `pool_9512` from 0 to 3 and places the demon 19502 at one of four cells of area 51; nothing points at it before its death, the compass set only by `QIS_OnEnter9512` once its dropped scroll is used, so in three games of four it stands elsewhere than in another (**hypothesis**: that this is the report, or its death at other creatures' hands leaving the scroll and no demon); the data is consistent.
- **7622 (Remembrance)**: `QIS_OnEnter7622` makes its target at (3223, 4731), two sectors from the giver, among creatures of its own type that `Sector50073Enter` fills, and points the compass there, testing nothing; the data does not explain the report.
- **2404 (Evacuation)**: `QIS_OnEnter2404` makes nine hostile creatures in the groups 24041 to 24043 within the giver's sectors, each with the death function `sieg2_2404`, the win needing all three groups dead; since `GroupIsDead` counts a creature never made as dead, one that failed to spawn cannot block the quest, but one alive anywhere else does, which fits a creature that follows a defender out of the camp; where both 2402 and 2403 have bit 4 set, `flucht2404` runs at once and the groups never walk in.
  The data does not explain an undead that never spawns.
- **52 (chapter 2's water bottle)**: the bottle 18261 (item 6103, `0x62`, `0x8E`) is made only by `OMO52ZombiesBeiSterbSoldat`, as the hero first enters the start script's box (2738, 4260)–(2797, 4322) whatever the quests' states; `PferdTot52`, which would make it again at the horse's corpse, and `Dialog:PferdTot52` are named by no record, though the start script lists the dialogue, so a pass through the box before chapter 2 makes the bottle then and nothing makes it again (#960; that this is the report is not shown).
- **61 (the IceCreek Pass marker)**: its compass is moved only by the 5.7 dungeon's teleports (`OMO-1tpreinHQ_5.7.*`, `OMO-1tprausHQ_5.7.*`) and by `NimmtElementLuft_2`, which sets it to the 5.7.4 mouth by Frostgard (1093, 1750) after `ElementAbfrage`, overriding its compass once the four elements are held (**hypothesis**: one compass a quest, shared by `QuestKompassPos` and `QuestKompassObj`).
  Entering 5.7.4 with bits 6 and 9 of `61` set matches none of its branches, and the exits 5.7.3 and 5.7.4 set the same cell again, so the marker points back at Frostgard's side until the hero leaves by 5.7.1 or 5.7.2 (#958; that this is the report is a **hypothesis**, a strong fit).
- **Jacquaire in Braverock (9006)**: Jacquaire 17523 is made at `auftrag_goodie5` (in Braverock) only by `OMO9006rg6_goodie1_waldrand`, where bit 11 of `goodies` is set, the trigger that `rg6_goodie1_start2` places on a 6 × 6 box in region 6 as it exits 9006; a hero who leaves by a portal or another road never makes him, and 9007 and 9011 wait on him (#959).
  The trigger is no region's border, though `Region23Enter`, which teleports him onward and sets 9011 up once 9007 is done, is one; that the box lies on one road of several is a **hypothesis**.
- Named by no record: the debug functions `goodie5` and `setuptest`; quest 45's `region_change` and 9000's `replace_spawn` name `0x69` 15, the oasis's area, which was not checked.

To check in the original: 7622's target at its cell; 9512's demon at the four cells over several new games; 2404's groups with `creatures` once the quest stalls; the bottle after an early pass through 52's box; Jacquaire after leaving region 6 without crossing the box.

Not read: whether a save keeps the date and the timers, the drift seen in the running original, and a test by which a repaired drop could know that its partner quest is open (these quests set no bits of their own).
The repairs each change a quest's course, so each is a `bug` whose decision waits for the maintainer: the shared drops (#930), the timers of 1202 (#931) and 1803 (#932), the calendar's drift (#933), and the quests above (#935 to #944, #946 to #955, #957 to #960).

## `SetNPCState`

Read 2026-10-23 (#578); **confirmed** by code unless marked otherwise.
`SetNPCState` (type 3, the handler `0x48BB40`) reads its tags into a record of its own, a later tag overwriting what an earlier one of its kind wrote, and hands it to `0x461540`, which changes the creature that its `01` names (the local player's for the name it keeps for the hero; nothing without a name or a number).
In the order of `0x461540`:

- The operand-less tags make a flags word as `CreateNPC`'s do (docs/02 §7.3), but each mode replaces the word's low 16 bits, so that a mode after `0x45` takes its `0x2000` away.
  A word other than 0 replaces the creature's (`+0x1F4`) but for the bits `0x6682304`, which stay (`0x2000` and a boss's `0x80000` among them); the mode `0x800` (`0x43`) given to a trader (`+0x200` bits `0x4007000`) takes those bits away and is dropped itself.
- `0x75` is the creature's quest, the word `+0x96` (`0x450E20`), which `ExitQuest`, `LoseQuest` and the quests' hook `0x471790` compare with a quest's number.
- `0x4D` `(x, y, floor)` is its home (`+0x2B9`, docs/re/ai.md), at the cell's centre; the scripts give one to the hero too (**hypothesis**: where it rises again).
- `0x08` and `0x0E`, the last of them, make it hostile or peaceful by its side (docs/re/fight.md, "Sides"), and its followers with it: `0x08` puts one that is not hostile to the hero already, or an animal, on the side 2, or 14 where the record's word has the guards' mode `0x4000` or its side is 7; `0x0E` puts one that is hostile to the hero on the side 3, or 7 so.
  The test is `0x549000` against the creature's controlling player (`+0x39`, the local player's id in a game of one), where `CreateNPC` asks `0x548F60` of the hero, to the same answer.
- `0x03` turns it as `CreateNPC`'s does (`0x44DDC0`), and a word with the mode 1 wakes it (`sdk_WAKE`).
- `0x09` names a dialogue and `0x0A` gives one by its number: the walker's table of dialogues (`this+0x755C`, filled as the start script's 1,021 entries of type 40 run, the first entry added twice, `0x476A05`) is searched for the first entry of that name, case aside (`0x859690`), or whose `+0` holds that number.
  An index above 0 becomes the creature's dialogue (`+0x245`, `sdk_STAMP_IDX`), the entry's `+0x4C` taking the creature's id; a name not found yet, the index 0 or a number not found take the creature's dialogue away.
  The entries' `+0` is −1 in every one, and every `0x0A` of the scripts is `0A:0` (11,121 records): `0x0A` takes the dialogue away.
- `0x73` writes `+0x25F` (the mark, docs/re/fight.md), `0x67` `+0x2B6` and `+0x261`, and `0x6B` clears `+0x2B7` bit `0x08` for 0 and sets it above 0 (the strikes' resolution and the deaths read it with `+0x251`, not read further).
- `0x05` keeps a function's name for the creature in the script manager's list at `+0x358` (`0x464330` takes its old ones out first), as `CreateNPC`'s `0x05` does: the function its death runs (**hypothesis**, by the scripts' names: `od_5002`, the escorts' `…_LOSE_…`); `0x60` adds one too and sets `+0x1F4` bit `0x400000`.
- `0x1F` is its level, 1000 a boss (docs/re/spawning.md, "`SetNPCState`'s level"); `0x36` writes `+0x255`, which serves nothing (docs/re/ai.md).
- `0x23`, `0x24` and `0x25` make it a trader (`+0x200` bits `0x1000`, `0x2000` and `0x4000`), each in the script manager's list of traders, and `0x64` too (`0x4000000`) with a dialogue stamped (`0x2BF20`).
- `0x63` names a trader whose stock of horses the creature joins (the list at `+0x79E8`, up to six horses each, at their levels), its dialogue taken away.
- `0x11` puts it in a group (`+0x249`, `0x4A9EF0`), and `0xA1` sets `+0x14` bit `0x200000`.
  `CreateNPC`'s `0x11` does too, its number the group's (`0x483A46`, #669), and `opensacred-game` reads it there.
- `0x37`, `0x4B` and `0x71` hire it (`sdk_HIRE`, docs/re/movement.md), the hero's follower for the number 0; `0x37` and `0x71` also clear its `+0x1F4` bit `0x100` and set `0x4`, and `0x71` marks it in its quest's list (`+0x31C`); `0x39` and `0x65` call the quests' hook `0x471790` instead and tell the hero's client (an event `0x10B`).
- Last its stats are computed again (`0x5796A0`).

Not read further: `0x7F`, `0x80`, `0x84`, `0x8C`, `0x90` and `0x85`'s blood lines.
In the Seraphim's tree, of the records that `opensacred-game` runs (the start script, the quests' `OnSetUp` and entered `OnEnter` functions), 66 of the start script and one of an `OnSetUp` are `SetNPCState`s, and those that name a creature made before carry `0x1F` 62 times (the stables' horses), `0x63` 59, `0x09` once (the captain of 52,39, `auftrag22`) and `0x6B` once.
**Confirmed** in the original (2026-10-23, the Seraphim's start under the driver of #448): the captain's dialogue is 958, the place from 1 of `auftrag22` among the start script's dialogues, which the table's doubled first entry gives.

In `opensacred-game`, `npcs::set_states` gives the creature that a `SetNPCState` names after its `CreateNPC` the record's level, its dialogue by the table as it stands then, its flags word, its side by its hostility (`opensacred_rules::minds::stated_side`), its trades and its home (#578); the captain lists with its dialogue as in the original.
Its quest, its death's functions and `0x6B` are #656, the followers #657, the horses' trader and the rest #658.

## Differences from the openheilig table

openheilig's tag table (MIT; derived from a Linux build with addresses `0x08xxxxxx`) differs from the GOG client in eight places:

- 0x1F carries 4 bytes, not 3, or a variable reference.
- 0x9F is a variable reference, not 3 bytes.
- 0x92 carries two strings.
- 0x37 and 0x4B carry a byte and an optional string.
- 0x71 is a marker.
- 0x0B, 0x38 and 0x5F can carry a name after the sentinel 0xFFED2979.
- 0x6F carries an i32 before its string.
- In openheilig, an unconsumed tail ends a record at a 0x00 tag.
  With the GOG table, every record of the release consumes its body exactly.
