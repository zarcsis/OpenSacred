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
  The runner runs a function only while its entry is not done and, for a function of a quest (`+0x48` of its entry 0 or more), only while that quest is in the quest store with its flag 1; it refuses, answering that they did not run, the functions whose names begin with `InitRg`, case aside, and hold `Pools` or `(DQ_Quest)`, as written, and those beginning with `DQTrgRegion` or `DQStartRegion`, case aside (`0x46BB72` to `0x46BC0A`; corrected 2026-10-26, #1051, where this said the `InitRg` test the wrong way round).
  These are the functions that would start the dynamic quests: `DQStartRegion<n>` checks a region's quest pool, `DQTrgRegion<n>` makes its givers, `InitRg<n>Pools` fills its pools; the Seraphim's tree calls them 244 times, and `opensacred_scripts::refused` refuses them as the original does.
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
- **The guards.** `GroupIsDead` (24) answers 0 on a false result, `ExitDlg` (43), `DelDlg` (44, after it takes the creature's dialogue away; "Dialogues" below) and a record of type 0 always, so that the function stops; `QuestDone` to `QuestNotInProgress` (10 to 13) do too, but none of them occurs in the release.
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
The converter gives that payload the cell of the chest whose `CreateObj` follows the record (docs/02 §7.1, docs/decisions/netscript.md, "Chest 1339's cut patch given its cell").

**A creature's place.**
Read 2026-10-26 (#1046): a position's name that begins with `CPOS:` the reader resolves as it reads the record (`0x474222`, `0x474295`), case aside.
`CPOS:hero` takes the local player's hero's cell and floor (`+0x1C`, `+0x20`, `+0x24`), which `0x45FF80` moves, for a hero on the ground, to a free point about it where another object stands on it or no way leads there.
`CPOS:RES:<key>` takes the cell and floor of the creature that the scripts' map of names holds for that key (`0x464BD0`), and another `CPOS:` name an object's; a creature gone leaves the position as it was.
In the Seraphim's tree 774 records of `CreateNPC` and `CreateObj` are placed so, 654 in the quests' functions.
`opensacred-game` puts such a record on that creature's cell and floor as it runs, the hero's for `CPOS:hero` (`npcs::locate`); the free point about the hero is not followed.

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
  Read again 2026-10-27 (#1242): the record walker hands `0x48EE20` the running function's quest (its entry's `+0x48`, from `0x46BA90`), and `ExitQuest` and `LoseQuest` share it, the second with a flag of the loss; each of its operands names a quest by its number (`0x0B`) or its name (`0x01`, case aside) in the quests' vector, and where the record has none, or none of them is found at an index other than 0, it ends the running function's quest instead.
  So the scripts' common `ExitQuest` and `LoseQuest` of no operand (about 5,200 and 3,250 across the converted trees, against 2,700 and 4,800 of a number, none of a name) end the quest of the function they stand in.
- A new game runs its tree's start script through, record by record (`0x46F9B0`, `0x470AC0`, from `0x60C2DC`); a saved game's load does not.
  The Seraphim's start script sets quest 1 up and triggers it at once, and its `QIS_Trigger1` is empty: the world starts with quest 1 entered and its `QIS_OnEnter1` run, which creates the novice (`CreateNPC`, creature 679, at the `DefPos` `novizin1`) and sends her to the chapel's benches (`NPC_Goto` to (3237, 2514, 0)).
  The other heroes' start scripts trigger a quest 1 of their own the same way, but the Daemon's, which triggers none; the Underworld trees' start scripts trigger none.
- `NPC_Goto` (72, `0x49E210`) takes the NPC's name, then a cell (`0x04`, through `0x6224B0`) or raw coordinates (`0x20`), and an optional flag (`0x66`): it does not move the creature, but posts it an event of walking there (a `cEvent_creature`, id 4), starting a motion first; `Teleport` (46, `0x491D40`) places the creature at once.
- `QuestKompassObj` (64, `0x49AC80`) points the quest compass at the named object: the quest log alone.

`opensacred-rules` keeps this state apart from Luau and from the engine (#724, `opensacred_rules::quests`): the quests' flags and the transitions above, each naming the section functions it runs for the scripts' runtime to run and taking its answer; the variables by name, kept to their first 31 bytes and found case aside (since #1054; the start's `Regionchange` and the quests' `RegionChange` are one); and the dialogues' table below, its first entry doubled; all of it serializable for the saves (`quests::Scripts`).
`opensacred-game` runs the start script and the sectors' functions in Luau on this state since #726 (`scripts`, docs/decisions/netscript.md, "NPCs sent by scripts walk the hero's way"): the transitions run their sections in the middle of their records through the runtime (`opensacred_scripts::Handled::Run`, `quests::Quests::step`), `ExitQuest`, `LoseQuest` and `UnTriggerQuest` too since #1242 (a quest of a number found by the quests' files of the tree, `quests/<n>.luau`, **hypothesis** for a quest that has none), and the variables and the tests on them follow "Variables and groups" below.

## Variables and groups

Read off the client with Ghidra 2026-10-25 (#943, #951) by a subagent of the session; **confirmed** by code unless marked.

- The scripts' variables are a vector of the interpreter (`+0x7550`, entries of 0x24 bytes), found by name without regard to case (`0x859690`) and of equal length.
- `SetVarBit` (68, `0x49B840`) ORs `1 << (n mod 32)` into the variable for each `0x0B`, bits counted from 0; a missing variable is created with the mask.
  The name `HeroQBit` sets a hero's quest bit of the difficulty instead (`0x424DA0`); a second `0x01` names either a creature (`res:`) or a variable whose value is the bit.
- `UnsetVarBit` (69, `0x49C160`) clears the bit of its last `0x0B`, and does nothing to a missing variable.
- `IncVar` (75, `0x49C930`) adds the last `0x0B`, or a named variable's value, keeping the old value where the sum is smaller (a negative increment, an overflow); a missing variable is created with the increment.
  It also counts in one of 150 counters (`+0x7750`) for a name that does not start with `atmo_rg`, by a number parsed from the name's eighth byte (its use not read).
- The `IF` operands: `0x48` holds where the variable exists and equals the number (a name starting with `LOC_RG` looked up in another table, `+0x334`), `0x49` where the bit is set and `0x4A` where it is clear, both false for a missing variable.
- The groups of creatures are one map of the global interpreter (`0xAACF80`, `+0x7510`), keyed by the group's number alone, each a list of object ids: no quest, area or function enters the key.
  `CreateNPC`'s `0x11` adds the creature to its number's group (`0x4A9EF0`) and keeps the number in the creature (`+0x249`); a destroyed creature leaves its group (`0x463A70`).
  During an `Enter`, creatures of some types also join a group keyed `0xFFFF0000` with the creature's word `+0x18` (**hypothesis**: its sector), which never meets the scripts' numbers.
- `GroupIsDead` (24, `0x48C860`) answers alive at the first member that lives (`0x549080`) and lacks `+0x1F4` bit `0x2000000`, wherever it stands, and takes the others out of the group.
- `SetGroupState` (73, `0x49E760`) takes out of the group the members of `+0x14` bit `0x10`; with `0x51` it makes an object of type `0x30C` at each other creature's place, posts it the event `0xC` (**hypothesis**: the vanishing effect) and destroys the creature where it lives, wherever it is.
- `Attack` (92, `0x4A2B40`) looks both of its groups up in the same map.

Not read: `DecVar` (76, `0x49CEC0`); when the map of groups is emptied, and whether a save keeps it.
Destroying a creature runs no death's function ("The deaths' functions" below).

## Base triggers

Read off the client with Ghidra and capstone 2026-10-25 (#1002), **confirmed** by code unless marked otherwise.

- `SetBaseTrigger` (4, `0x4819A0`) takes the trigger's name (`01`), a corner (`0C`, or `04` in 6 records: a cell or a `DefPos` name, its third number the layer, held to 0..16) and the other corner (`0D`), a box of one cell without it; the corners are ordered.
  `0x63C970` splits the box at the sectors' edges and appends each part to its sector's list (`+0x204`, entries of 0x4C bytes: `x0`, `y0`, the layer, `x1` and `y1` as `u16`, the name from `+0xC`; `0x622E80`).
- `DelBaseTrigger` (5, `0x4817F0`) with the name alone takes that name's boxes out of every sector, and with corners out of the sector of the lower corner and its eight neighbours (`0x63CB10`); each box goes as the sector's last box is moved into its place, the names compared without regard to case (`0x622FA0`).
- The trigger of a cell (`0x6230C0`) is the first box of the cell's sector that holds it on its layer, the layers 0 and 1 counted as 0.
- Type 39 (`0x4918B0`), which has no keyword, names a trigger (`01`) and the index of a function (`0B`), which it adds to the cache of the triggers' functions (`0xAB44AC`, `0x4636C0`), sent to the clients in a game of the network.
  The start script of every tree holds them, 656 in the Seraphim's; in each of the 20 trees each name they give has the one function `OMO<quest><name>` (the compiler's `OMO%d%s`, `0x45C192`), whatever the case, two of them where the function's name is written twice (`rg24g3_start` of the Seraphim's).
- As a creature's cell changes in its update (`0x5941A0`, at `0x597E2C`) and it is the local player's hero, or a horse whose rider it is, the trigger of its new cell and layer runs: each function of the cache's entry of its name in order, the quests' hook `0x471790` called with the event 5 first for a function of a quest, then the function by the runner `0x46BA90`, which runs a function of a quest only while the quest is set up; a name without an entry runs `OMO-1<name>`, and without that a function `OMO`, which no tree defines.
  Seen in the original 2026-10-26 with #448's driver: a walk of the Seraphim's over 3086, 3085 and 3084 of the row 2549 looked each cell up (`0x6230C0`), and on 3084,2549 `OMO-1tpreinR16_MH_1` ran and teleported her to (3619, 5076); the driver's `cell` onto 3047,2892 and 3084,2549, triggers' cells, looked nothing up, so a teleport runs no trigger.
- `Teleport` (46, `0x491D40`) takes the creature (`01`; `hero`, or none, the local player's hero), a heading (`03`), and a cell (`04`, `0C` or `0D`: a cell or a `DefPos` name, its third number the layer) or a world point (`20`).
  `sdk_ENGINE_TP` (`0x54D9D0`) puts it there; where that fails, it tries 15 cells at random within 3 cells on the layer, then 25 within 6 on the layer and the two above it; then it sets the creature's flag `0x40000000` and turns it to the heading.
  A mounted hero's branch is in "Order traps" below.
- The hero's step onto a cell of a trigger that has functions ends its motion right after a teleport (`0x593CE0`, with the flag `0x40000000`).

Not read: the quests' hook of the event 5, and what the hero's list at `+0x39C` holds there.

`opensacred-game` runs them since #1002 (`scripts`): the scripts' state keeps the places of `DefPos` and the boxes, the runtime lives on through the play, and as the hero's cell changes on its way (not by a teleport) the trigger of its cell runs its functions, found by their names; their `Teleport`s of the hero put it on the cell and layer they name (docs/decisions/netscript.md, "Hero's script teleports land on their cell").
Since #1014 the `CreateNPC`s and `CreateObj`s that the play's functions run (these, the timers' and the quests' sections) place their creatures and objects as the start's are placed, with the places of their `DefPos`s and their `SetNPCState`s on the creatures they make: at once where their sectors are in the world, else as they come in, kept as an `Init`'s, a creature taking its level as it is made; walking into quest 52's box by (2738, 4290) makes its zombies and its bottle at once, as the original's did (#960).
In the Seraphim's tree the hero so goes into the caves and the dungeons by their entrances and out by their exits: into 2.3 by 3047,2892 to 4086,83 and out by 4083,83 to 3042,2892.

## `SetHP`

Read off the client with Ghidra 2026-10-26 (#1001), **confirmed** by code unless marked otherwise.

- `SetHP` (52, the walker's case `0x4758F7`, `0x48E280`) takes a creature's name (its last `01`; `hero`, without regard to case, or none the local player's hero) and a number (its last `0B`), held to 0..100.
- Where the creature's most health is below 1 it is first set to `(level + 1)·100`; its health then becomes `most·n/100`, truncated, and a creature that has health and is not alive (`0x549080`) stops its motion; its byte `+0x4CF` is set (not read).
- So quest 201's horse 18007 (creature 551, `Region2Init`'s `SetHP` of 50) stands at 37 of its 75, as the original's driver showed (#585), and a `SetHP` of 0 leaves a creature without health: the quests kill their targets so (1202's `QIS_OnEnter1202` and its timer).
- `opensacred-game` follows it since #1001 (`scripts`, `npcs`): the start's, the regions' and the sectors' `SetHP`s give the placement they name its share as it is made, and those of the play the creatures of that name in the world, or the hero, a creature left without health dying as by no player's hand (it drops nothing; that the original's drops nothing is a **hypothesis**).

## The deaths' functions

Read 2026-10-26 (#656) by the static reading of `Sacred.exe`; **confirmed** by code unless marked otherwise.

- The script manager (`0xAACF80`) keeps one list at `+0x358` of entries of 68 bytes, a creature's id and a function's name of 64 bytes, at most 60,000 (`0x464330` logs a broken list beyond).
  Its names of the prefixes `Take:`, `Use:` and `See:` are an item's taking, use and sight (the items' handlers `0x558080`); the others are the creatures' death's functions.
- `CreateNPC` adds its `0x05`'s name, the death's function, with the creature's id (`0x4B9740`), and its `0x60`'s as `See:` and the name, a sight function, setting `+0x1F4` bit `0x400000` (its reader not read): both copy the string at `0x94E1D4` before the name (`0x482510`'s case `0x60`, `SetNPCState`'s reader `0x48BB40` alike; #1025).
- `SetNPCState`'s `0x05` first takes out the creature's entries that a death takes out (below; `0x464330(id, 0, 1)` at `0x461FBC`), then adds its name; its `0x60` adds its `See:` entry and sets `+0x1F4` bit `0x400000` (`0x46204B`).
  The `0x05`s and `0x60`s of the Seraphim's tree: 757 and 17 in `CreateNPC`s, 371 and 2 in `SetNPCState`s, and 11 and one in `SetGroupState`s; 13 `SetOnKill`s and one `SetOnCollect` name a function by `0x05` too (#1021); three of the functions they name are defined by no tree (`Zombies_OnSee`, `Zombies_OnSee2`, `tot1602`), none differs only by its case.
  What runs a sight function (**hypothesis**: `sdk_WAKE`, one of the runner's callers, as the creature first sees the hero) is not read, and the game does not run them.
- A death runs one of them, at the AI's state 9 (`0x53E7C0`, at `0x53E915`) and in the creature's events' handler (at `0x52F769`), which run the same steps:
  - the first entry of the creature's id whose name has none of the items' prefixes (`0x4A71F0` with an empty name), else nothing;
  - the creature's entries of no `:` or of `See:` taken out, but those starting with `wegw` (`0x464330(id, 1, 1)`), the second and later logged as an `OnDeath` twice;
  - the function of that name, its length from 1 to 63, found in the functions' table by its name case aside (`0x46BF40`, `0x859690`) and run by the runner (`0x46BA90`), a quest's function only while its quest is set up ("Quests" above);
  - then, the creature's health above 0, it rises (`0x53E460(1)`) and no more is done; else the quests' hook takes the death (`0x471790` with the event 4), which serves only the dynamic quests and so does nothing in the release ("The dynamic quests").
- The run comes after the creature's drops and its killer's experience, which its events' handler gives as it dies (docs/re/items.md, "Drops"; docs/re/fight.md).
  The death's `GroupIsDead` finds the creature dead already, its health 0 (`0x549080`), so a group's function runs on at its last member's death.
- In the Seraphim's tree, 32 of the functions so named give their own creature health again (`SetHP` above 0: a target beaten rather than killed, quest 2203's `tot2203` at 50, a hireling's `rg2_nachhause` at 100 after its `Teleport` home), and so it rises.
- A creature destroyed, by `SetGroupState`'s `0x51` (`0x49E760`, at `0x49F741`) or as it is removed (`0x463A70`), has its entries taken out without their functions run: neither calls the runner `0x46BF40`, whose callers are the timers (`0x416F00`), the deaths above, the items' handlers, the AI's state 1 (`0x53A170`), `sdk_WAKE` and a few of the engine's events.

`opensacred-game` follows it since #656 (`opensacred_rules::quests::DeathFunctions`, `npcs::OnDeath`, `scripts::deaths`): the start's, the sectors' and the play's `CreateNPC`s and `SetNPCState`s give the creature they name its functions, its sight functions as `See:` entries since #1025, kept with its placement as it goes with its sector; a death, at any hand, runs the first death's function through the scripts' runner by its name case aside, the world's effects of the run carried out, and a creature that its function gives health rises at that health before it is laid down.
`GroupIsDead` answers in the play (`scripts::living_groups`): a group lives while a creature of it lives in the world, but the raised dead, or away from it, made by the start or the play, or gone with its sector alive (`npcs::Npcs::living_groups`); a sector's `Init` makes its creatures only as its sector comes in, and at the start, before the world, the test is not answered, as before.
In the game the Seraphim's start's monsters of the group 10 each run quest 9's `tot1` as they fall, and the seventh's sets its timer `Tutorial09`.

## The rewards: `AddExp`, `AddGold` and `Gewinn`

Read 2026-10-27 (#1162); **confirmed** by code unless marked otherwise.
The trees hold 1,023 `AddExp` (17), 1,473 `AddGold` (18) and 5,710 `Gewinn` (125), mostly in the quests' functions of their delivery.

- Each acts only outside a function whose quest is finished (`+0x120` bit 2) or not set up (bit 1 clear), by the running function's index (`0x94E07C`) and its quest (the trigger names' `+0x48`, the quests' store `0xAABF18`), as `CreateObj` does (`0x48D480`, `0x48DA40`, `0x4AC940`).
- `AddExp` (`0x48D480`) and `AddGold` (`0x48DA40`) name a creature by `0x01`, `HERO` (case aside) the local hero, and take a number by `0x0B`; a record without a name acts on the function's creature (the walker's argument).
  `AddExp` adds the points through the creature's `addExperience` (`0x549520`), with no difficulty's factor.
  `AddGold` adds the amount through the gold's handler (`0x57FD80`), less than 0 too, and sends the creature the engine's event 6, a line over it of the amount and the game's text 1004 (the word for gold), `0xFFFFFF80` for a gain and `0xFFFF4040` for a loss; **seen**: `script F_Gold` on bronze gave the Seraphim 1,000 gold and showed the amount and the word in light yellow over her head.
  Of the trees' records, 659 `AddGold` and 231 `AddExp` name `HERO`, and the others no one: the prices of services (`AddGold` of −100 to −2,000) among them.
- `Gewinn` (`0x4AC940`) reads up to three numbers from the tags `0x0B` and `0x9F` in their order: its flags, a value `v` and a step `s` held to 0 to 5.
  Of the trees' 5,666 `Gewinn`s, the orders are `0B 0B 0B` (4,606), `0B 9F 9F` (1,000), `9F 9F 9F` (20) and `0B 9F 0B` (40, a variable's value between two numbers, as quest 10259's level); counted on the listings 2026-10-27.
  - Each counts the hero's quests (`+0x2CF`) and a set's count (`+0x2CD`); once the latter passes `R(1, 56)` it starts again and the hero drops a piece of a set not 33 to 38 whose least level is at most the hero's, 6 at least, at random (`0x559BF0`).
  - The flag 1 gives gold, `(G[s]·v + 100)·(H/2 + 1)` for the hero's level `H`, `G` = 5, 8, 12, 20, 4, 25, through the gold's handler and with the line of `AddGold`.
  - The flag 2 gives experience: the middle of the hero's sector's levels `m` (`(lowest + highest)/2` of its world record's `+0xD3` and `+0xD5`, 0 without one) makes `⌊(m + 2)·(E[s]·v + 100)/2⌋`, `E` = 10, 8, 12, 8, 4, 15, times the difficulty's factor, 1, 2, 10, 20 or 30, truncated, times the decay of a high hero (`0x57E050`), truncated, through `addExperience`.
  - The flags 4, 8, `0x10` and `0x20` choose a type each, of a category at random among 7, 8, 9, 15, 16, 20, 29, 30 and 31, among 5 and 13, among 6, 13, 17, 18, 19 and 21 to 25, and of 8, by the drops' type (`0x5C7FA0`, of the hero's class, the weapon kind any) at `v + R(0, 3)`, in that order, each taking the place of the one before: one item at most (**confirmed** by code and in the running original 2026-10-28, #1386: six runs of `dq_belohnung` made one item each, by one call of the item maker `0x5D5080`).
  - Their quality (`0x4AD461`): `S` the step's number of 50, 100, 80, 200, 500 and 800, `F` the difficulty's items' factor of 1.0, 1.4, 1.9, 2.9 and 4.2; a key rolled on the drops' weights as a death's pass rolls it, with the bonus `⌊S·F/10⌋` (at most 700) and the factor `g = 10·((1 − ItemThrowFactor)·S/400 + ItemThrowFactor)`; the item maker's `16 − key` for the keys 2 to 15, else 0.
  - The flag `0x40`, or no type made, takes a rune of the hero's class instead (`0x45F410`: one of a list of 12 to 21 runes of each hero type, drawn again up to five times while it is no rune), and counts it among the class's runes given (the player's `+0x5710`, `0x424690`).
  - The item is made 40 units before the hero along its facing (`0x450680`), on its layer (`sdk_CREATE`), at a level of the sector's levels raised by the difficulty and of the hero's (**hypothesis** for that level, not read in full; seen 2026-10-28 at the hero's level 1).
- The end of a side quest (`ExitQuest`, `0x46CBE0`, of a quest numbered 100 or more, at `0x46D362`) may give the hero a rune of its class (`0x45FC60`, **confirmed** by code and in the running original 2026-10-28, #1386: quest 15001's exit made a rune after its `OnExit`):
  - `r = 0x5595A0(1000, 1)`: the hero's runes carried `n` (in its backpack and its storage) and its arts' levels `a`, against the class's runes given `c`, set to `n + a` where it is 0 or above 10,000, and where `n + a` falls below it `a` counts as `2c/3` and `n` as `c/3`; `x = (⌊n/2⌋ + a)/H/p`, `p` the runes per level of the hero's type 1 to 7 (`RunenProLevel_*`) and 1 for the Dwarf and the Daemon, `H` its level; `x⁵·1000`, at most 32,767, whole;
  - the rune comes where `rand() % (r + 1)` falls below `UpgradeDrop` (50), made 40 before the hero as the reward's item, and counted among the class's runes given.
  - So a young hero of few arts gains a rune with nearly every side quest: the Seraphim of level 1 with one art at level 1 has `r = 46`, every roll below 50.
- `addExperience` (`0x57E160`) sends a player's creature the engine's event 6 of each positive gain, a line of the points and the game's text 1071 (the word for experience) in `0xFFFFFF20`, whatever the gain's cause: the kills', `AddExp`'s and `Gewinn`'s; **seen** 2026-10-28 (the maintainer's frame): a reward's gold line and its experience line one under the other over the hero.

`opensacred-game` gives the hero the gold, the experience and the item of the three records (`rewards`, `opensacred_rules::rewards`, #1162, #1386): `scripts` keeps the records of a finished quest's function out and reads their variables' values as they run, and `rewards` carries them out before `npcs` takes the records, the line of the gold over the hero, the item laid 40 before it rising from 60 over the ground as an item dropped off the cursor, made at the hero's level; a record without a name acts on the hero (**hypothesis**: the function's creature is the hero in the functions that pay).
A side quest won rolls its rune (`rewards::QuestWon`, `opensacred_rules::drops::Rules::quest_rune_range`), the count of the class's runes given kept for the session; and every experience the hero gains shows its line under the lines over it, the gains of one frame together (`rewards::experience_line`, `lines::Says::push`).
`Gewinn`'s sets' pieces by the quests' count, and a reward to another creature, are not followed yet.

## `SetGroupState`, `DelNPC`, `SetOnKill` and `SetOnCollect`

Read 2026-10-26 (#1021) by the static reading of `Sacred.exe`; **confirmed** by code unless marked otherwise; none of it seen in the running original yet.

**`SetGroupState`** (73, `0x49E760`):

- Its tags (the switch at `0x49E82D`; a tag without a case, such as quest 9010's `0x37` and `0x39`, is skipped): `0x11` the group; `0x08` and `0x0E` hostile and peaceful; `0x05` a death's function; `0x60` a sight function, kept as `See:` and the name; `0x09` a dialogue by name and `0x0A` by number, looked up at once in the dialogues' table as `SetNPCState`'s are; `0x1F` the level; `0x36` a value for `+0x255`; `0x6B` the `+0x2B7` bit 8; `0x75` the quest word; `0x01` a key; `0x51` the removal.
- The flags tags build one word from 0 as `SetNPCState`'s do (`opensacred_rules::ways::state_word`), without `0x45`.
- With a group above 0 it walks the group's ids from the first (a list of more than 1,000 is emptied instead, `0x49FB11`); an id that is no creature in the world is passed over and stays in the group (**hypothesis**: a creature of a sector not loaded); outside `0x51` a dead member is changed like a living one.
- A member destroyed already (`+0x14` bit `0x10`) is taken out of the group, and the handler then takes the list's last id out too (`0x49ED0F` to `0x49ED35`), which keeps its `+0x249` and is not changed.
- With `0x51`, at each other member an object 780 (`FX_PUZZLE_SOLVED`) is made and sent the FX event `0xC`, and a member alive (`0x549080`) is destroyed (`sdk_DESTROY(id, 1, 0, 1)`, at `0x49FAF0`), which takes it out of the group, the walk starting again; a dead one keeps its body; no other tag applies and no function runs.
- Without `0x51`, for each member in this order:
  - a word other than 0 replaces `+0x1F4` but for its bits `0x6682304`, and the word so written is kept for the next member (`0x49F0ED`), so that each later member also takes the kept bits of the members before it;
  - `0x01`'s number becomes the key `+0x3C` of a member whose `sdk_REFKEY` answers its type (**hypothesis**; two records in the Seraphim's tree);
  - `0x08` puts a member not hostile to its controlling player on the side 2, or 14 where the word has the guards' mode `0x4000` or its side is 7, and `0x0E` a hostile one on the side 3, or 7 so, with no animals' case (`0x52E420`); a side changed makes an object 856 (`FX_GENERIC`) at the member, sent the FX event `0x50` with a colour (**hypothesis**: a flash), and either tag clears `+0x200` bit `0x40`;
  - `0x60` adds its `See:` entry and sets `0x400000` in the kept word, not the member's, so that the bit reaches only the later members (the one record, 1401's `schmuggler1402` for the group 14011);
  - `0x75` sets the quest word;
  - the dialogue is always written: an index above 0 by `0x09` or `0x0A` becomes the member's (`sdk_STAMP_IDX`), and with neither, a name not found or `0A:0` it is taken away, so that 64 of the Seraphim's tree's 98 `SetGroupState`s take their members' dialogues away;
  - `0x1F` 1000 makes it a boss (`+0x1F4` bit `0x80000`, `0x549500(1, 0)`), another level above 0 goes to `0x564D60(L, 0, 0)`; `0x36` writes `+0x255`, `0x6B` the `+0x2B7` bit 8;
  - `0x05` takes out the member's death's and sight functions, but those starting with `wegw` (`0x464330(id, 1, 1)`, at `0x49F741`), and adds its name;
  - last a network event in a network game; it wakes no member, adds none to a group, and runs no function.

**`DelNPC`** (55, `0x497F80`): each `0x01`, its number or its name resolved, is looked up in the map of names, which gives at most one object; a creature in an art's action (`+0xFC` 10) is stopped and marked vanishing, an item held is taken out of the hero's slots or inventory, and any other object is destroyed where its name (the resource name of its key, `0x44A200`) equals the record's string case aside, or its key lies within 8,500 to 8,700 (**hypothesis** for which of the tree's 311 `DelNPC`s pass the test); then the name is erased from the map; it makes no effect object, and the destruction runs no function (`0x463A70`).

**`SetOnKill`** (135, `0x4AF190`) and **`SetOnCollect`** (136, `0x4AF8C0`):

- Each reads a type (`0x02`), a function's name (`0x05` or `0x41`, at most 64 bytes) and a count (`0x0B`), the first of each, and does nothing without a type and a count; their lists of 76-byte entries (the type, the count left, the count asked, the name) are at `+0x7614` and `+0x7620`.
- A type already listed takes the new count as both counts and the new name if any; a count of 0 takes it out; a new type is listed only with a name.
- A kill counts the dead creature's type (`0x4AA630`) in its events' handler at its death after its drops (`0x534795`), whoever killed it, the hero, a follower or a creature, but not a death without a killer (`0x534F60`) nor one of class 5 whose most health is not 0 (`0x5337AB`): an undead that rises again (docs/re/fight.md, "The undead that rise again"; no `SetOnKill` of the Seraphim's tree names a type of class 5), and at a sudden death with a killer (`0x52F453`), and by the hero's look's kill (`0x5475F0`, not read further).
- The type's first entry counts down; above 0 the engine's event `0xB` carries a line of the type's name and the counts (**hypothesis**: shown on the screen); at 0 the function, found by its name case aside (`0x460590`), runs through the runner (`0x46BA90`, a quest's only while it is set up) and the entry is taken out.
- A pickup counts the item's type (`0x4AAAE0`, from `cCreature::pickupItem` at `0x558175`), but an item of `+0x14` bit `0x4000`: first every entry whose type is a creature's takes the item of that creature's `SetDrop` (`+0x762C`), then the item's entry counts down as a kill's does, and an item counted is kept among the main quest's items and destroyed (docs/re/items.md, "What becomes of the item").
- The Seraphim's tree has 13 `SetOnKill`s and 17 `SetOnCollect`s.

`opensacred-game` follows them since #1021:

- `SetGroupState` (`npcs::Npcs::regroup`, `npcs::GroupState`): `0x51` removes the group's living creatures in the world, nothing dropped and no function run, and keeps those away from it from being placed again; else each member, in the world and away from it, takes the word (`opensacred_rules::ways::grouped_word`), the side (`opensacred_rules::minds::grouped_side`), the dialogue, the sight and death's functions, and away from the world the level.
  Not followed: the kept bits carried from member to member, `0x60`'s bit and the list's second id taken out (docs/decisions/netscript.md, "Group states without carried bits"); the objects 780 and 856 and their events; the key, the quest word, `+0x255`, `+0x2B7` and `+0x200` bit `0x40`; the level of a member in the world.
- `DelNPC`: the creatures of the name, in the world and away from it, removed as `0x51` removes them, whatever their resource name; the objects of the name are not followed.
- `SetOnKill` and `SetOnCollect` (`opensacred_rules::quests::Counts`, `scripts::Live::kill`, `scripts::Live::collect`): a death that is a kill counts its type, but a creature's that its death's function raises, a death at nobody's hands (a `SetHP` of 0, a summoned follower replaced, a lifetime's end: `npcs::Dies::unkilled`) and one of class 5; an item the hero takes counts its type, and one counted goes from the pack among the main quest's items (`pickup::Pack::quest`); the engine's lines of the counts are not followed.

In the game, quest 9's `trigger1`, the novice's button, made the start's group 10 hostile and awake again (`npcs: the scripts set the group 10`).

## Dialogues

Read 2026-10-26 (#1024) by the static reading of `Sacred.exe` (a subagent of the session); **confirmed** by code unless marked otherwise, the counts on the Seraphim's tree.
**Seen** the same day in the running original (the driver of #448, the Seraphim's start): a left click on the novice of the chapel, and on a sister indoors from 7 cells off, sent the hero running to stand about 2 cells from her, where a window opened at the screen's middle with her portrait at its left, her name in light yellow at its top, her text's paragraphs and her buttons in green at its bottom, one centred, two apart; a button closed it at once, and the novice's started quest 1 (a chapter's title shown, the novice walking off).

### The talk's start

- The hero's AI state 1 (`0x53A170`, the left click's event 7) takes the talk for a creature of the type categories 2 or 3 (`0x426220`) whose talk (`0x549920`, below) is above 0 or which trades (`+0x200 & 0x7000`): the AI's state 10 (`0x529C60`), the creature at `+0x100`.
  A creature alive, hostile to the hero (`0x548F30`) and of no talk is attacked instead, so that a hostile one with a dialogue is talked to.
  A hostile creature alive within the two creatures' reach and 15 of the first of the hero's hostiles (`0xAD3CE0`) leaves the hero idle instead (**hypothesis**: that `0xAD3CE4` is its distance).
- The state 10's first step: farther than 1.5 times the two creatures' reach (`0x4DBC60`, the mean of their sizes) the hero walks to the creature's place, stopping within the reach, and the creature ends what it does (`0x54E6D0`) and has its AI's timer put back 5 s (`+0x208`), so that it waits; nearer, or once the walk ends, the hero takes the talk's action 7 (`0x52AB70`) toward it; a trader without a talk opens its windows instead (`UI_WND_BLACKSMITH`, `UI_WND_MERCHANT`, `UI_WND_MASTER` for `0x1000`, `0x2000`, `0x4000`).
- The hero's action 7: on foot it plays its model's slot 177 (`*_TALK_A`, the blend 0.25), or its standing clip without it, turns to the creature and calls the quests' hook (`0x471790(hero, creature, 1, 0)`, "The dynamic quests"); unless the creature is in a dialogue already (`+0x200` bit `0x80`) it posts it the event `0x10C`, and sets its own `+0x200` bit `0x10`.
- The event `0x10C` (`0x52E590`, at `0x5304EE`) gives the creature the AI's state 10 toward the hero: within 1.5 times the reach it takes its own action 7, the same clip and a turn to the hero; farther, once it stands, it drops the talk, so that it never walks to the hero.
  Its action 7 runs the dialogue for a talk from 1 to 200,000, and opens the trade windows for a talk of 0 or above 200,000; its idle AI waits while the talk lasts.
- A creature in a talk (`+0x200` bit `0x10`) shows no dialogue cursor (`MOUSE_DIALOG04.TGA`, `cEngine_updateWorldCursor` at `0x612685`, which wants a talk above 0, no `+0x1F4` bit `0x40000` and an id above 16), and a strike at a talking creature is ignored (`0x56B130`).

### The dialogue's run

- The creature sets `+0x200` bit `0x80` and finds its function (`0x46B480`): `Dialog:<name>` of its talk's entry among the functions, case aside (`0x859690`), else `<name>`, else the entry's `+0x44` (the type-40 body's first value, **hypothesis**: a function's index).
- It becomes the speaker (`0x465590`: the script manager's `+0x75C0`, which the operand name `DLGNPC` names), and the five button slots are emptied (`+0x9A00`, `+0x9A14`); the function runs through the runner (`0x46BA90(function, creature, 0)`), its records collecting the window's contents.
- `Text` (26): its `0x1E` key names the `global.res` string of its `sacred_hash` (`0x80EAA0`), pushed on the list of texts (`+0x99F4`), the creature's `+0x200` bit `0x400` set.
  Without `0x7C` the record first empties the list, so that a `Text` replaces the text; with `0x7C` (902 of the 4,303 `Text`s, before the key in the bytecode) it adds to it; no `Dialog:` function's first `Text` has it.
  Nothing stands between two texts: the paragraphs are the strings' own line breaks; a string's `<sound=…/>` tag would name a sound for the window's opening, and no string of the release has one.
- `SetButton` (60, `0x499BA0`): its first `0x01` is the label, its second the function's name; a label of `res:<digits>` (case aside; 3,194 of the 3,201) keeps its number, whose string is `sacred_hash` of its decimal, so that `res:1024` is the string of `"1024"`; one of fewer than 2 characters adds nothing.
  It fills the first of five slots, a sixth dropped, and the window shows the first four (`0x6E7260`), each with the function found by its name case aside as the window is made (`0x460590`); a number of 0 shows no button.
- `PlaySound` (104, `0x4A9730`) plays the sound its `0x01` names at once (the client's table `0x964870`), so as the window opens: the dialogues' voice-overs, under their texts' keys (`sound.pak`'s `SOUND_FX_<key>` samples).
  It plays one as a line by name (`0x693FE0`), the sound manager's one at a time (`+0xB7E0`, docs/re/sound.md), so that a voice-over stops the line by name that plays, the hero's own lines' too, and the window's close stops it (`0x693FB0`; **seen** 2026-10-27, #1234: closing the novice's window at the Seraphim's start called it, and closing her first, which gives the quest, played another line by name in its place).
- `ExitDlg` (43) only stops the function (`0x476387`), and the window shows what came before it.
- `DelDlg` (44, `0x477CA7`) takes the creature's talk away (its entry's `+0x4C` cleared, `sdk_STAMP_IDX(0, 1)`, `+0x14` bit `0x80000` cleared) and stops the function, whatever came before; what came before still shows.
- `ToBegin` (111) runs the function again from its start without emptying the buttons; no `Dialog:` function holds it (91 other functions do).
- After the run, with a text, the creature makes the window (`cUI_Manager` at `0x756F40`, a `cUI_Tooltip` `UI_WND_TOOLTIP`, `0x75AF90`) at the point (512, 384) of the 1,024 × 768 screen with the size 640 × 512 (**hypothesis**: the point is its centre, as seen); without a text the talk ends, the function's records having run all the same.

### The window and its buttons

- The name is the string of the creature's key, `CreateNPC`'s `0x01` (`sdk_REFKEY`, `0x44A1C0`: `res:17095` the string of `"17095"`), else its type's name; drawn first in the colour `0xFFFFFF80`, then two line breaks and the texts in their order (`0x6E70D0`).
- The portrait is an image of the interface chosen by the creature's type through a table of the client (`0x4345C0`, the type 1 the image 964), an index into the interface's images (`0x9ECEB0`, entries of 0x54 bytes: the index, a TGA's name and a rectangle, as 964's 96 × 128 of `GUI_portraitX34.TGA`), drawn 8 pixels in from the window's corner (`0x6E76C0`).
- A click on a button (`0x6E74E0`) hides the window, plays the interface's sound `0x5D`, and `0x499DA0` ends the talk of every player's hero (the event `0x10C` of the end) before it runs the button's function with no creature (`0x46BA90(index, 0, 0)`), the speaker still the dialogue's.
  So the function's own `Text`s and `SetButton`s open no window: a conversation goes on only where the function begins another talk (`Popup`, `NPC_TalkTo`) or gives the creature its next dialogue for the next click (`SetNPCState`'s `0x09`), as the novice's does.
- Without buttons the window closes at the talk's next tick on Enter or a mouse button held (`0x655920`); two other methods of the window run the first or the second button's function (**hypothesis**: keys).
- The talk's end clears the bits `0x80` and `0x10` and destroys the window; the hero, the first button chosen, calls the quests' hook (`0x471790(hero, creature, 0, 0)`).

### The talk's own end, and the world meanwhile

- The creature's talk ends once its partner leaves the talk or stands farther than 1,500 (`0x5370B0`), and after 31 ticks where it opened no window; the hero's ends once the creature's window is gone or the creature leaves the talk.
- Nothing pauses: the talk's steps are the creatures' ticks of the world (**hypothesis**, to see in the original: the world runs on, and a click beside the window reaches the hero's orders, ending the talk).

### Other records

- `Popup` (95, `0x4A7760`) gives the local hero the named dialogue and a talk with itself (`0x54CBB0(hero, 2)`): its function runs with the hero, the window showing no name and the image of the entry's `+0x48` (the fixed images `0x400` to `0x42A`, `0x457` the hero's class's portrait); then the hero's dialogue is taken away (241 functions, none a `Dialog:`).
- `NPC_TalkTo` (71, `0x49DCF0`) gives its first `0x01` the dialogue `0x09` and a talk toward its second (`HERO` the local hero), which takes its action 7 toward the first (4 records).
  It wants both names and a dialogue of the table: of the Seraphim's 4 records, the 3 of one name do nothing, and the fourth, quest 2002's `nein2002`, its giver's "no", has its giver begin the talk again (read 2026-10-26, #1037).
  The creature's last dialogue's entry loses it (`+0x4C` 0) and the dialogue's entry takes it, as a stamp does.
- `Dialog` (31, `0x48F9E0`) gives the function's creature a dialogue by its `0x16` or `0x01` name or its `0x0B` number (1,870 records, in the dynamic quests' `ToDo` functions).
- `SetIcon` (86, `0x4A1A50`) writes the entry's `+0x48`, of the first entry of its `0x01` name, case aside, or, without one, of the running dialogue (`+0xA458`), its `0x0B` number; a name the table does not hold is logged.
  The unnamed ones stand in the `Dialog:` functions (213 of the 214 in the Seraphim's tree).
- `InfoPlayer` (132, `0x4AE350`) gathers its operands' values in order, a `0x01`'s (and `0x16`'s, `0x1D`'s, `0x1E`'s) a text's id, a `0x0B`'s a number, and takes the first for its mode: 0 and 1 post the sound manager's events `(9, 0xC8)` and `(9, 0xC9)`, which play nothing, and 2 the event `(9, 0xC9)` with 7009 through the music once (`0x1B61`, an id without a music file; **hypothesis**: silent) (read again 2026-10-27, docs/re/sound.md, "The sources"), each then posting the engine's event `(0xB, 0x10, 3)` with the second value, its text (**hypothesis**: the text across the screen, as the chapters' titles show in orange); 3 plays the sound 7000 and posts `(0xB, 1, 3)`; 5, 6, 7, 10 (an image `witz*.bmp`), 23 to 27 and 100 to 107 do other things, not read further.
  In the Seraphim's tree its modes are 1 (233 records), 0 (116), and 5, 6, 7, 23, 27 and 100 to 103 a few times each.
- `SetRgnDialog` (106, `0x465280`) keeps a creature type (`0x02`), a dialogue by name (`0x09`) or number (`0x0A`) and a region (`0x0B`, 0 for none) in the list `+0x7568`, replacing one of the same type and region; the Seraphim's start script holds all 128, none with a region.
  A creature's talk (`0x549920`) is its `+0x245` where that is above 0; else, alive, without `+0x200` bit `0x40`, of the class `+0x1F0` 3, 7, 8, 12, 14 or 15, with a bit of `+0x1F4 & 0x1006DCF8` and not `0x40000`, and hostile neither way with its controlling player, it is the first entry of its type whose region is its place's (`0x602290`) or 0, found anew at each query.
  In the regions 50 to 69 a creature of neither gets a generic line (`NPCUWR_…`) at random at each query by its type and the chapter's flags, its talk 54,321 running `Dialog:NPC_UW_DLG`, the line in place of its first text.

### In `opensacred-game`

The game follows the talks since #1028 (`npcs::Talk`, `scripts::Live::talk`, `talk`):

- A creature's dialogue is its `CreateNPC`'s `0x09`, as its `SetNPCState`s' and `SetGroupState`s' change it (#658, #1021) and a dialogue's `DelDlg` takes it away; without one, its type's of `SetRgnDialog` where it is of the sides and flags above and hostile neither way, found anew at each click (`npcs::Npcs::region_talk`); `Dialog` and the Underworld's generic lines are not followed.
- A left click on a creature of a dialogue alive near the cursor's line of sight, hostile or not, sends the hero to it on its level, heading for it again as it moves, and the hero's attack passes it over; within 1.5 times their reach the hero stops, both turn to each other and the creature plays its talk (`npcs::Talks`) and stands still while the talk lasts.
- Its `Dialog:<name>` runs as the runner runs a function, a quest's only while the quest is set up: its `Text`s replace or, with `0x7C`, add to the window's texts, its `SetButton`s give five buttons, its `PlaySound`s play the voice-overs as the window opens, `ExitDlg` and `DelDlg` stop it.
- A window at the screen's middle shows the creature's name (its key's string), its texts, their `<n>` breaking their lines and their colours' tags left out, and its first four buttons; a button closes it, ends the talk and runs its function, found by its name case aside; one without buttons closes on Enter or a click; and the talk ends where the two stand farther than 1,500 apart or the creature is gone; a dialogue without a text shows no window; while it shows, the hero's clicks on the world are held back (docs/decisions/interface.md, "Dialogue window holds back world clicks"), and its look waits for the interface's frames (#1087).
- Since #1033 a `Popup` begins the hero's own talk of its dialogue, its window without a name, once no window shows (`talk::popups`), and an `InfoPlayer` of the modes 0 to 3 shows its text in orange across the screen's upper part for 5 s, in place of the one before (`talk::notices`; **hypothesis** for the place and the time, the engine's event not read); its sounds and its other modes are not followed.
- Since #1032 the window shows the creature's portrait at its left, its type's part of a texture of the interface (`tables/portraits.ron`, docs/02 §6.7, `talk::Portraits`); the hero's own talk shows none.
- Since #1039 a creature of a dialogue or a trade shows its icon over its head (`icons`, `opensacred_rules::quests::icon`, below); the townsfolk's icons under Alt are not followed.
- Since #1037 an `NPC_TalkTo` whose second name is the hero gives the creature of its first the dialogue, and the hero speaks to it as a click on it does (`scripts::Begins::With`, `talk::popups`); a talk between two creatures is not followed.
  In the game, quest 2002's `nein2002` sent the hero to its giver, whose window opened.
- `Dialog` runs only in the dynamic quests' `ToDo`s, which the release never runs ("The dynamic quests" below), and the game follows it as the release does, doing nothing; the trade windows are not followed.

In the game the Seraphim's start's novice showed her text and her button, as the original's, and the button ran `exit01`, which gave her her next dialogue, the one the next talk showed, and showed the first act's title over the scene as the original's does.
Since #1039 her "?!" pulses over her head as the original's does.

### The talks' binds and `SelfTriggerQuest<n>`

Read 2026-10-27 for #1250 in `Sacred.exe` and on the converted trees, code only; **confirmed** by code unless marked otherwise.

- The compiler gives every quest a function `SelfTriggerQuest<n>` of one record, `TriggerQuest n` (`0x45C73D`–`0x45C7DC`; `SelfTriggerQuestPool<n>` for a pool's quest), so that each tree holds one for each of its quests (601 in the Seraphim's).
- Two records bind such a function, both handled before the walker's dispatch (`0x47578D`–`0x4757DE`) and only while the walker's flag is set, their entries of the jump table `0x4784C0` doing nothing: `TalkTo` (6, `sdk_DIALOG_CHECK`, `0x491170`), which takes a creature's name (`0x01`, a `res:` name as its shown string) and a dialogue by name (`0x09`) or number (`0x0A`), and `MoveOver` (16, `0x4915A0`), which takes a base trigger's name.
  The flag is the script manager's byte `+0x14`, which only the new game's walk of its start script sets (`0x470AC1`, cleared at `0x470BCB`) and which `SetUpQuest` hands its sections (`0x48F8A6`, `0x46DB20`): so they would bind only in the set-up sections that the start script runs (**hypothesis** that no other writer of the byte exists).
- `TalkTo`'s bind (`sdk_TALKBIND_FIRE_FUNNEL`, `0x463240`) gives the creature the dialogue's entry (its `+0x4C`, `sdk_STAMP_IDX`, the creature's `+0x14` bit `0x80000`) and adds the index of `SelfTriggerQuest<quest>`, the running function's quest, under the key `Talk <name> Dlg <entry>` to the script manager's map at `+0x752C` (`0xAB44AC`), the same map that the start's records of type 39 fill with the base triggers' functions; `MoveOver` adds it under the trigger's name (`0x4636C0`).
- A talked-to creature's talk, at its end (its talk's step 6, `0x52AB70`, `0x52C364`–`0x52C5CD`, after its window is gone and a button's function ran), runs the first function under its key with the creature (`0x52C503`), the key taken out and put back only where the function did not run to its end; the talk's start runs none (`0x52BDB7`).
- No record of the types 6, 16 or 47 stands in any of the 20 trees (every `FunkCode`, `StartCode`, `QuestCode` and `QuestPoolCode`), so the release binds nothing and never runs a `SelfTriggerQuest<n>`: they are the compiler's leftovers.
  Every side quest of the trees has a `TriggerQuest` outside it; of the Seraphim's 221 campaign side quests, 210 are entered by a dialogue's button's function (`trigger<n>`, `btn_accept_<n>_start`, `trgquest<n>`, 44 of them by another function too), 10 in their own `QIS_OnSetUp<n>` (5081 to 5089, 9522) and one by its base trigger (301); her other 352 are the dynamic quests 2600 to 3056, which never run.
- `opensacred-game` keeps no binds and runs no `SelfTriggerQuest<n>`, as the release; the side quests are entered by the buttons' functions it runs (#1028).

### The icons over the creatures

Read 2026-10-26 (#1039) by the static reading of `Sacred.exe`; **confirmed** by code unless marked otherwise, and **seen** the same day in the running original.

- `cCreature_render` (`0x599F2E`) draws an icon over a creature whose `+0x14` has the bit `0x80000`, or whose type's flags (`0x428460`, the types' table `0xAAB5E4`) have the bit 19, which no type of `Items.pak` has.
  The bit `0x80000` is set by every record that gives a creature a dialogue (`CreateNPC`'s `0x09` at `0x483AFF`, `SetNPCState`'s at `0x461F16`, `Dialog` at `0x48FD61`, `SetGroupState` at `0x49F6BB`), by the traders' tags of both (`0x484326` to `0x4843BB`, `0x462400`) and by the scripts' hires (`0x484FB9`, `0x4626FC`), and cleared by `DelDlg` and a stamp of no talk (`sdk_STAMP_IDX`, `0x5498F0`).
  Without it, while Alt is held, a creature of the townsfolk's flags (`+0x1F4 & 0x1006DCF8`) whose activity (`+0x261`, through `0x959830`) is 1 to `0x5A` shows that activity's icon instead.
- The icon (`0x499E90(talk, creature)`): 4 for a talk of 160,000 or more (the stamps 180,000 of the trader of the tag `0x64` and 160,000 or 170,000 of the hires); else a trade's, `0x20` for the smiths (`+0x200` bit `0x1000`), `0x21` the merchants (`0x2000`), `0x22` the wizards (`0x4000`) and `0x23` the trader of `0x64` (`0x4000000`); else its dialogue's entry's `+0x48`, the type-40 body's second value as the `SetIcon`s leave it; else 0.
- `0x4090D0` draws nothing for 8, 13 and 126, and else picks the image by a switch: 0, 1, 4 and 5 `NPC_DIALOG_01`, `02`, `05` and `07`, tinted `0xFFFFFF00`, 10, 11, 14 and 15 the same, tinted `0xFF3F3F3F`, `0x20` to `0x23` `npc_dialog_smith`, `trader`, `combo` and `trader`, the townsfolk's `sim_*` from `0x40` to `0x5A` and from `0x80` to `0x83`, and any other `NPC_DIALOG_02` tinted `0xFFFF0000`.
  It draws a square from −10 to 10 across and from 20 to 40 up, in the models' units, in the frame of the display's matrix (`+0x250`) at the creature's bone `Bip01 Head`, twice: the image with the tint, then the image again, added in a grey of `(sin(t · 0.005) + 1) · 127` at `t` milliseconds and 100 more for each of the creature's ids.
- **Seen**: the start's novice (her `auftrag10`, the icon 0) and the townsperson of `auftrag210` (the icon 10) south of the chapel both showed a "?!" whose highlights went from about a third of the image's values to its full values and back in about 1.3 s, with no yellow and no dimming; so the tints do not show (**hypothesis**: the device's lighting takes the vertices' colours' place and lights the square, which has no normals, by its ambient alone), and the game draws the image at a third of its values (**estimate**) and the pulse over it.

## The dynamic quests

Read 2026-10-26 (#1020, #1037) by the static reading of `Sacred.exe` (two subagents of the session); **confirmed** by code unless marked otherwise, none of it seen in the running original yet.
The engine keeps a system of dynamic quests, offered by givers that it picks at random among a region's creatures, and in the release none of it ever runs.

### The templates and the table

- A quest's template, a "pool", is an entry of 136 bytes at `0xAAC738`: a number at `+0`, a name at `+4`, the most quests open at `+0x4C` and the count open at `+0x50`, headers of 36 bytes at `+0x58`, and lists of functions: the `ToDo`s at `+0x64`, the successes at `+0x70` and the failures at `+0x7C`.
- Only the script compiler's `QuestPool:` (`0x45A370`, which nothing but itself calls) and a save's load (`0x46772A`) fill the templates, and every tree's `QuestPoolCode.bin` is empty, so the release has none.
  The walker passes over `MaxOffen`, `Rettungsmission`, `ToDoBloecke`, `Schatzsuche`, `Toete_NPC` and `Begleite` (27, 29, 30, 34, 101, 102) as doing nothing.
- The quests made from them stand in the script manager's table at `+0x31C`, records of 52 bytes:
  - `+0` the kind, from the header's `+0xC` (**hypothesis** by the flows below: 1 bring items, 2 bring a creature to the giver, 5 kill, 6 take a hired creature to a trigger);
  - `+1` to `+3` the pool, the header and the `ToDo`;
  - `+4` and `+5` the success's and the failure's functions (`Belohnungen`, 32, `0x490BE0`; `Strafen`, 123, `0x491090`), `+8` the count needed, `+9` 1 where the quest is reported to its giver and 0 where it is solved on the spot;
  - `+0x10` the state: 1 offered, 2 taken, 4 solved, 8 failed, `0x10` failing while the giver's talk runs, `0x20` closed, `0x40` all its targets dead, `0x80` its items in hand;
  - `+0x14` not 0 while it runs, `+0x18` its time limit (`SolveTime`, 122), `+0x1C` its list of entries, `+0x28` the `ToDo`s that `HideTmpToDo` (33, `0x4630A0`) hides.
- An entry of the list (44 bytes) holds an object's id at `+0`, its flags at `+4`, the record's index at `+8`, and 8 words that `SetClientVars` and `GetClientVars` (118, 119) keep.
  Its flags: 1 the giver; 4 a creature to bring (`CreateNPC`'s `0x1A`); 8 an item (`CreateObj`); `0x10` that creature joined; `0x20` the one to report to (`0x70`); `0x40` a hired creature (`0x71`, `SetNPCState`'s `0x71` too, `0x462AC2`); `0x80` a target to kill with every member of its group (`0x72`).
  A creature belongs to its record by its `+0x94`, which is −1 for none (`0x44A40B`); its quest's number, which `SetNPCState`'s `0x75` writes at `+0x96` (`0x450E20`), is compared where a trigger reports.

### The maker and the `ToDo`s

- `CheckForQuestPool` (25, `0x490A30`) names a template by its `0x01` name or its `0x0B` number and calls `0x46EDD0`, which first destroys the givers of the quests offered and never taken (**hypothesis**), and then, while the count open is below the most, picks givers at random by weight among the template's creatures alive and in no quest (`+0x200` without the bit `0x200`).
  A template's creatures are those that `CreateNPC`'s `0x15` (the pool, a low and a high, a weight) registered under it (the map `+0x340`, `0x483B16`).
- For each giver, `0x46DFC0` picks a header of the giver's range and a `ToDo` not hidden, adds a record with the giver's entry (flags `0x21`), joins the giver to it, logs `CreateDynamicQuest`, and runs the `ToDo` with the giver for its creature (`0x46E7E0`); the giver takes the bit `0x200` where the `ToDo` ran to its end, and for the kinds 1, 2 and 5 the `ToDo` runs again while it adds no entry.
- A `ToDo` is named `ToDo:-1.<m>` by the compiler (`0x45BA71`), `<m>` its block's number in `ToDoBloecke:`; the runner finds it through the template, not by its name.
  It gives its giver the offer's dialogue with `Dialog` (31, `0x48F9E0`), which wants the function's creature (the walker's fourth argument, `0x4756BB`) and does nothing without one:
  by a name (`0x01`, `0x16`) it takes the first entry of the dialogues' table of that name, case aside, and by a number (`0x0B`) the first entry of that number; the creature's former entry loses it, the found entry (above 0) takes its id, its talk becomes the entry (`sdk_STAMP_IDX`) and its `+0x14` bit `0x80000` is set, or cleared for the index 0; a name or number not found is logged and clears the bit alone.

### The hook and the end

- The quests' hook `0x471790(actor, id, event, arg)` takes the creature's record by its `+0x94` and returns at once for −1.
  Its events: 0 a talk's end on its first button (`0x52C631`), 1 a talk's start (`0x52BDB7`), 2 an item taken into the inventory or a follower let go (`0x462BD9`), 3 the hero's AI, which changes nothing, 4 a death (`0x52F7D7`, `0x53E95F`), 5 a base trigger, once for each of the hero's followers with the function's quest (`0x598106`), 6 a creature's timer (`0x41717B`).
- By its entries' flags and the record's kind it moves the state: a target's death passes the entry to the next living member of its group and, with none left, marks `0x40`, solving a quest of `+9` 0; an item taken solves a quest of kind 1, or marks `0x80`; a creature to bring that joins at a talk's end takes the state 2; a hired creature's trigger with its quest's number solves the kind 6; at the talk's start with the one to report to, the kind 2 with enough of its creatures in the party releases them to the giver and is solved, the kind 1 with enough items takes them and is solved, the kind 5 with `0x40` is solved; an offer's talk's end starts its time limit (1,440 to 4,320), and its timer's end runs the giver's talk with the state `0x10` and fails the quest.
- Its end (`0x471000`) runs the success's function, or the reward `0x4AC940` without one, or the failure's, and closes the record (`0x20`): the giver loses its talk and its `+0x94`, and takes `+0x200` bit `0x20000000` for the bit `0x200`.
- The table's other readers: the blocks of type 41 (`0x477C10`), `ExitQuest` and `LoseQuest` (`0x46CBE0`), the hero's AI state 1 (`0x53AE9A`), `0x46A910`, which joins again the items and creatures of the quests in progress, the saves (`sdk_SAVELOAD_SERIAL`) and the network's handlers; the quest log and the compass read none of it.

### None of it runs in the release

- With no template, `CheckForQuestPool` finds nothing; besides, all 22 of the Seraphim's tree stand in `DQStartRegion<n>`, and the givers (`DQ<n>_AUFTRAGGEBER1` to `4`) are made only by `DQTrgRegion<n>`, the functions that the runner refuses by their names ("Control flow", #1051).
- So the table stays empty, every creature's `+0x94` stays −1, the hook returns at once, and no `ToDo` and no `Dialog` ever runs (**hypothesis** to see in the original: the count of the table's records, `([0xAAD2A0] − [0xAAD29C]) / 0x34`, stays 0 through the play, and the log never shows `CreateDynamicQuest`).
- The Seraphim's tree holds them all the same: 1,870 `ToDo` functions (85 blocks, one copy per each of the 22 pools), each with one `Dialog` by `0x16` naming one of 110 offers of five kinds (`DQ<pool>_BRINGE_NPC`, `_BRINGE_ITEM`, `_TOETE_NPC`, `_TOETE_ITEM`, `_ESCORT_NPC`), 748 `CreateNPC`s of `0x72` and 506 of `0x1A` in them, and in the quests 2600 to 3056, which name the givers, 704 `SetNPCState`s of `0x75` and 88 of `0x71`.
- `opensacred-game` follows none of it, as the release runs none of it: `Dialog` and `CheckForQuestPool` do nothing, the table and the hook are not kept, and `SetNPCState`'s `0x75` is left out.

Not read: the headers' `+8` (**hypothesis**: a level) and the bytes `+6` and `+7` of a template's header; what becomes of the givers not picked (`0x46F240`); `0x45FC60`; whether `0x45A370` is reached through a pointer that Ghidra did not mark.

## `ReplaceSpawn` and `RegionChange`: the world's changes

Read 2026-10-26 (#55) by the static reading of `Sacred.exe` (a subagent of the session) and on the Seraphim's tree; **confirmed** by code unless marked otherwise, none of it seen in the running original yet.
In both records `0x69`'s number is an area (a sector's `+0xD7`, keyx `+0x2C0`), not a region.

### `ReplaceSpawn`: the fills of an area

- `ReplaceSpawn` (96, the walker's case `0x475BA4`, `0x4A79D0`) appends an entry of `0x3C` bytes to a vector of the interpreter (`+0x394`): a sector by `0x04` (`+0`, `+2`), `0x69`'s area (`+4`, a string read as a number), up to four `0x02` types replaced (`+8`), up to four `0x19` replacements (a type, a weight and a hostile flag, `+0x18`, `+0x28`, `+0x30`), the flags word of the same tags as `RegionChange`'s, `0x46` giving `0x4000` (`+0x34`), and the side, 1 with `0x08`, else 0 (`+0x38`).
  A record of exactly two `0x02`s and no `0x19` is a pair, the first type replaced by the second, keeping the line's weight and hostile flag; any other record without a `0x19` stores nothing (`0x4A7BC8`).
- Before it appends, it overwrites every entry of the same sector or area with the same types, and then appends all the same, so that a function run twice leaves its entries twice.
  It changes no creature that exists.
- Its one reader is the fill (`FillRegion` 35 and `FillSector` 51, `0x47C610`), and only while an `Enter` runs (the interpreter's byte `+0x12`, `0x47CF39`): an entry applies where its sector is the fill's or its area the fill's sector's, in the entries' order, each of the fill's `0x19` lines of a listed type taking the entry's type, its weight and hostile flag where the entry gives them, and the run its side and flags word (docs/re/spawning.md); a later entry can swap back what an earlier one swapped.

### `RegionChange`: the creatures of an area, and its later ones

- `RegionChange` (116, `0x4ABB60`) changes the existing creatures of its area ("The NPCs that later events remove" below), and its applier (`0x4ABFE0`, the areas 0 to 49) also keeps its pairs in a table of 50 lists of `0xAC` bytes (`+0x1048`), the flags word in each new type's high half.
  Within one function a later record of an area replaces the earlier one's list; one that follows a `call` (the runner sets the running function's index −1 as any function ends, `0x46BDAD`, `0x46BF20`) adds its pairs to it, with no bound on the list's 20 slots.
  Read again 2026-10-26 (#1063): the applier adds where the record's index equals the running function's (`0x94E07C`) or that is −1, and the record's index is `0x94E094`, which nothing writes and the image holds at −1, so that a record adds exactly while no function runs, and replaces while one runs (`0x46BD0D` sets the index as a function begins), whatever function left the list: `BadToGood` replaces `GoodToBad`'s lists.
  A list never added to before takes the record whole; one added to keeps its first record's flags word and side, each pair its own record's flags.
  The record's flags word is its tags' bits added up (`0x12` 1, `0x46` `0x4000` and the rest of `CreateNPC`'s modes, `0x45`, `0x50`, `0x74`, `0x85`), `0x14` clearing the low half; its side 1 with `0x08`, 0 with `0x0E`, else −1; its area 0 without a `0x69`.
- Its one reader is `CreateNPC` (`0x482F41`), for a record that makes no trader and carries `0x6B` of 0, whose value is −1 in a record without the tag (seen in the original, #1102: a probe of the test at `0x482F28` read −1 for the soldiers of area 22's `Enter`s): a creature of a type that the list of its cell's area holds takes the first pair's new type, and the pair's flags word where it is not 0; it keeps the record's side.
  The pair's flags are the low half of its record's word, and replace the low half of the `CreateNPC`'s, their top bit spread over the high half (`SAR 0x10`); the `CreateNPC`'s side then follows from its own tags and the new type as ever.
- Neither reader reads the other's list; both are saved with the game (`0x465690`), and only the interpreter's reset empties them (`0x461040`): no record removes an entry, and `BadToGood`'s `RegionChange` variable set to 0 is the music's (docs/re/sound.md), not this table.

### In the Seraphim's tree

- 351 `ReplaceSpawn`s in 15 functions, 327 of them pairs; 24 of one type store nothing (`GoodToBad`'s five of area 6, `belohnung2411`'s and `belohnung2412`'s seven each, `untote_goodie_wueste`'s five), none carries `0x04` or `0x19`, and two have no tag (`Dialog:DlgHologram50`'s and `ElementAbfrage`'s), which make their runs peaceful with the flags word 0 (**hypothesis**: slips of the scripts, their siblings carrying `0x08` and `0x46`).
  Repaired: both take their siblings' `0x08` and `0x46` (docs/decisions/netscript.md, "Area 2's fills turned back after quest 72").
- 34 `RegionChange`s in 7 functions: `GoodToBad` 14 over 13 areas (4 to 8, 10, 11, 20 to 24, 31), `BadToGood` 13 over the same, `Init55` 3 in area 3, `Init45`, `WillbursTot45`, `feuerrune61` (whose pairs keep each type and only make the creatures hostile) and `untote_goodie_wueste`.
- `GoodToBad` (from `WillbursTot45`, quest 45) turns the soldiers 279, 286, 288, 295 and 297 of its 13 areas into the hostile 276, 277, 278, 287 and 296 where they stand, but its lists turn none of their later `CreateNPC`s, which carry no `0x6B` of 0 (the tree's one record of `0x6B` 0 is `Help11`'s, of the type 700, which no list holds, and 718 carry 1), and it swaps the same types in the fills of the areas 2, 4, 5, 7, 11 and 24; `BadToGood` (quest 72) turns them back and reverses the lists and the fills of the areas 4 to 7, 11 and 24, so that the area 2's fills stay swapped and the area 6's hostile soldiers turn good though `GoodToBad` never swapped them there (the original's behaviour).
  Repaired: `BadToGood` turns area 2's fills back too, and the area 6's stay as the original has them (docs/decisions/netscript.md, "Area 2's fills turned back after quest 72").
- `Init58` (quest 58) makes the fills of the areas 1 and 10 the hostile undead (the skeleton warrior 32 and the types 33, 34, 44, 48, 64, 66, 80 and 81, in place of 23 types) in the guards' mode, for good.
- `opensacred-game` follows the `ReplaceSpawn`s in the fills of the sectors that come in anew (#55, docs/re/spawning.md), and the `RegionChange` lists in the later `CreateNPC`s of a `0x6B` of 0 (#1063, #1102): the scripts keep the lists by area (`opensacred_rules::quests::AreaTypes`, the host told as each function that runs begins and ends), a play's `CreateNPC` takes the pair of its cell's area as the lists stood when it ran, and an `Enter`'s creature, made again at each coming, the pair of the lists as they stand then (`npcs::Makers::retype`), its model, clips, speeds, values, flags word and side those of the new type; a `SetNPCState`'s side given to such a creature before is not given again.
  In the original, after the driver's `script GoodToBad` at 37,32 (#1100) and a trip to 52,39 and back, the `Enter`s of area 22 made the soldiers 279, 286, 288 and 297 again of their own types, peaceful, the side 7 and the guards' mode `0x4000`, the turned ones destroyed as their sector left (#1103); only `RegionChange`'s change of the creatures that exist turns them, hostile with its record's `0x08`.
  `RegionChange`'s change of the creatures that exist is followed too (#1064): the record, in its order among the play's records, changes the creatures it reaches away from the world, the placements of the start, the `Init`s and the play where they went or were made and the fills' living ones that went with their sectors (`npcs::Npcs::add`), and then those in the world where they stand (`npcs::region_changes`), each made again of its new type in its place with its share of health, its group, its dialogue and its death's functions (`npcs::Makers::change`, `opensacred_rules::quests::AreaChange`); its values are made anew of the new type (**hypothesis**: the original keeps the creature's until they are made again), and the hero's followers and the creatures of no placement (the console's, the summons) are not changed.
  In the game, `script GoodToBad` at 37,32 turned the twelve soldiers of area 22 in the world into 276, 277, 278 and 296 of the side 14, hostile; after a trip to 52,39 and back, the soldier of an `Init` stayed so, and those of the `Enter`s came again of their own types, peaceful, as in the original (above).

Not read: what `0x182EBEC` is, by which `ReplaceSpawn` posts its entry over the network (`0x1CF`); whether a fill's bare `0x02` lines are replaced; whether `RegionChange` reaches the creatures kept for the sectors beyond the 3 × 3 about the hero (`+0x5A0`, **hypothesis**: they come back unchanged).

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
- `opensacred-game` keeps the scripts' drops by the creature and the item since #930 (docs/decisions/netscript.md, "Scripts' drops per creature and item"): with 15015 and 10259 entered, the creatures of the type 38 drop 6005 and 4931 each at its own chance, both from some.
  No operand of the data keeps both items: a repair of the data could only make the drops come one after the other, and a list keyed by the creature and the item would be the VM's, a departure from the client.

### `SetTimer`'s minutes and the calendar

- `SetTimer` (56, `0x4982D0`; the walker's case at `0x475A2C`) takes a function's name (`01`), the timer's id (`0B`, at least 0) and a time above 0 (`38`), and stores the id, the time and the moment it is set, `((((year·12 + month)·30 + day)·24 + hour)·60 + minute)` of the clock; a timer of the same id is replaced, which starts it anew (`0x417710`).
- At each minute that the clock completes (`0x416F00`; not at the minute that ends a day), the first timer whose time is at most the clock's count less its moment runs its function, unless the function's quest is finished (the flag 2), and every timer of its id is removed after it ran, so that a function that sets its own timer again loses it.
  The scan then starts again from the list's first, so every timer due runs in that minute (read again 2026-10-26, #933).
  The function runs by its name (`0x46BF40`), so only while its quest is set up (`0x46BA90`); an id of `0x10000000` or more names a creature, with which the function runs, after the quests' hook `0x471790` with the event 6.
- `DelTimer` (57, `0x4985E0`, `0x4177E0`) takes the first timer of its id (`0B`) out, the list's last moved into its place.
- `opensacred-game` keeps the timers since #933 (`scripts`, `opensacred_rules::quests::Timers`), counting the game's minutes elapsed since the new game rather than the calendar (docs/decisions/netscript.md, "Scripts' timers in elapsed minutes"), the rest as above; a `SetTimer` without its time makes no timer.
  So `38` is in minutes of the game's clock: 1,440 a day, 4,320 three days, 10,080 seven (a day of the game an hour of play is docs/re/daynight.md's **hypothesis**).
- The count takes months of 30 days and years of 12, but the date's carry (`0x416DA0`) makes eleven months of 29 days and the fifth of 34.
  So the count leaps a day ahead at the end of each other month, falls four days behind at the end of the fifth, and 31 days ahead at the end of the year: a timer across such an end runs a day early, four days late, or at once (not yet seen in the running original).
- The `IF` tags `0x49` and `0x4A` test that a variable's bit is set or clear, both false where the variable is missing (`0x4987B0`).

The quests whose time the community reports:

- **1202 (Magical Thistle)**: `QIS_OnEnter1202` sets `zeit1202` for 4,320, three days; it sets bit 4 of the variable `1202`, takes the target NPC's health to 0, writes the failure in the log and loses the quest, and the giver's dialogue at bit 2 deletes the timer.
  The giver's start dialogue (`R12NQ02_NPC_Auftrag_Qstart`) states four days, the log's start (`R12NQ02_Log_Qstart`) and the giver's dialogue while the quest is open (`R12NQ02_NPC_Auftrag_Qoffen`) three: the timer agrees with two of the three, and the report is the start dialogue's number.
  The same in the 11 trees that hold the quest.
  Repaired: the timer runs four days (docs/decisions/netscript.md, "Quests 1202 and 1803 timed as stated").
- **1803 (Kalim-Khor)**: `QIS_OnEnter1803` sets `zeitum1803` for 4,320 where bit 9 of `1803` is set, which `take1803` sets as the item 6068 is picked up before the quest is taken, and for 10,080, seven days, where it is clear, the usual way from the giver; `take1803` does not start it again once the quest is open, and `exit1803` deletes it.
  Only the giver's start dialogue (`R18NQ03_NPC_Auftrag_Qstart`) states a number, three days, so the usual way lasts seven days where three are stated; the report's "about 8" being the seven read loosely, or the count's leap at a month's end, is a **hypothesis**.
  The same in the 11 trees that hold the quest.
  Repaired: the second timer runs three days (docs/decisions/netscript.md, "Quests 1202 and 1803 timed as stated").
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
  Repaired: the dialogue tests the win first (docs/decisions/netscript.md, "Crossing quests left able to end").
- **2203 (Pure Water)**: `Region22Init` creates its six objects (item 6081, `0x41` `obj2203`) and its hostile target 17260 (`0x05` `tot2203`).
  `obj2203` counts the variable `2303a` whatever the quest's state, setting bit 3 of `2203a` at a count of 6, or bit 3 of `2203`, the win, where bit 3 of `2203b` is set; `tot2203` tests bit 3 of `2203a` first, which sets the win, ahead of the branch that triggers the quest where the target is beaten before it is taken.
  So with the objects taken before the quest and the target killed after them, the giver offers the win with the quest never entered: its `QuestBook` lines go nowhere, `ExitQuest` does nothing, and `QIS_OnExit2203` never runs (#936; that the objects can be taken first is unconditioned in the data, not seen in play).
  Repaired: the branch enters the quest first (docs/decisions/netscript.md, "Quests 2203, 7619 and 5005 end once").
- The count `2303a` is also set by `Dialog:ziel2303` (bit 2), a dialogue of R23NQ03, which has no quest file, once: the same branch sets bit 2 of `2303`, which sends the dialogue's next showing elsewhere.
  `SetVarBit` ORs the bit in ("Variables and groups"), so with k of the six objects taken as it runs, the count becomes 4 or 5 for k = 0 or 1 and `obj2203`'s `== 6` holds at the second object; 6 or 7 for k = 2 or 3, with no test run, so that the quest cannot be won; and nothing changes for k = 4 to 6 (#943).
  Nothing reads that bit: the five records of `2303a` in each tree that holds the quest are the start script's `SetVar 0`, `obj2203`'s `IncVar` and two tests, and this `SetVarBit`.
  Repaired: the dialogue's write goes (docs/decisions/netscript.md, "Quests 2203 and 501 kept apart").
- The same in the 11 trees that hold the two quests.

### The log contradicting itself

- **1303 (Goods Messenger)**: `zeit1303`, set for 4,320 minutes by `QIS_OnEnter1303`, writes the failure line and then `NQ_Log_Qend`, the generic completion line, and sets bit 3, on which the giver's branch writes `NQ_Log_Qfail` and loses the quest.
  The target 17177 keeps its dialogue `ziel1303`, which tests nothing: after the failure it still writes `NQ_Log_Qend`, and its button `exit1303` gives the reward and exits the quest that the timer did not lose (#937).
  Repaired: the timer writes no completion line and takes the target's dialogue away (docs/decisions/netscript.md, "Five quests' log records mended").
- **7619 (Death to the Dead)**: `QIS_OnEnter7619` hires its follower 19545 and gives him the death function `od_threepwood_7619`, which writes the failure line, shows the failure and loses the quest, and which nothing takes away.
  `od_7619` runs at each death of the group 761901 and, the whole group dead, shows the completion while the quest stays open until the follower's button `btn_ok_threepwood_7619_c`: his death between the two loses the quest after the completion, and after the button writes the failure line after the win, `LoseQuest` doing nothing to the finished quest (#938; his death as a follower being likely is a **hypothesis**, what `0x39` does to a follower not read).
  Repaired: the group's death gives him a death function with no records (docs/decisions/netscript.md, "Quests 2203, 7619 and 5005 end once").
  The same in the 10 Underworld trees and `bin/NetScript`.
- **5005 (Pony Express)**: at the last leg the mage 17277 takes the dialogue `dlg_zwischenpe`, whose branch on bit 5 runs `exitpe`; `losepe`, which `timepe` starts 5 minutes after a leg's time runs out, writes the failure lines, loses the quest and removes the stations 17763 to 17766, but does not give the mage his dialogue back, which `exitpe` does (`NPC_Dialog_Magician`).
  So after a failure on the last leg the mage still offers `exitpe`: the win line, `NQ_Log_Qend` and the reward, `ExitQuest` doing nothing to the lost quest (#939).
  Repaired: `losepe` gives the mage his dialogue back (docs/decisions/netscript.md, "Quests 2203, 7619 and 5005 end once").
  The same in all 20 trees.
- **10255 (Arachnophobia)**: each of its seven spider areas spawns a spider and calls `LoseQuest_10255`, which writes the line `DQ_10255_LOG_LOSE2` and gives the giver the dialogue `dlg_10255_Lost`, but has no `LoseQuest`, so the quest stays entered; its restart (`btn_accept_10255_REstart`) and its win (`btn_ok_10255_sieg`, which exits it) write no line.
  So after one failure the log's last line stays the failure line, even once the quest is won (#940; that the report means this line is a **hypothesis**); under its siblings' names (`DQ_<n>_LOG_SIEG`, as 10258 and 10261 write) the file has no line of its win or restart.
  Repaired: the restart writes the quest's task again (docs/decisions/netscript.md, "Five quests' log records mended").
  The same in all 20 trees.
- **1401 (Ancarian Snow)**: `QIS_OnEnter1401` sets `zeit1401` (the id 1401) for 1,440 minutes, whose one record writes `NQ_Log_Qend`, and only `OMO1401tgilde1401` deletes it: the trigger `tgilde1401` with `0x40` naming 17180, the hero entering the area with him among its followers.
  So a day after the start without that, the log says the quest is completed while it stays open, with no bit, reward or `ExitQuest`; the follower's death (`tot1401`) writes the failure line and leaves the timer running (#941).
  Repaired: the timer writes nothing (docs/decisions/netscript.md, "Five quests' log records mended").
  `tot1401`'s `SetTimer` (`hinterhalt1401`) carries two `0x0B`s, 1401 and 4320, and no `0x38`: the id 4320 with a time never set, where 4320 meant as the time, which would have replaced `zeit1401` at his death, is a **hypothesis**.
  The same in the 11 trees.
- **1001 (Captain Pigface)**: `QIS_OnEnter1001` writes `R10NQ01_Log_Qziel01`, the line of the kill, and points the compass at the target 17041 where bit 20 of `1001` is clear (`0x4A`), as it is while the target's item 6046 is not taken (`take1001` sets it): a quest taken the usual way logs the kill at once while the target lives, and one triggered by the item's taking gets no line of the kill (#942).
  Repaired: the line is written in the other branch (docs/decisions/netscript.md, "Five quests' log records mended").
  The same in the 11 trees.
- In the 11 trees, quest 1401's soldier 17181 names the death function `attack1401`, which no tree defines, while `attacke1401` is defined and named by nothing; the 13 smugglers of `Region13Init` (the groups 14011 and 14012) name `0x60` `schmugler1402`, while the function is `schmuggler1402` (#944).
  Repaired: the records name the defined functions (docs/decisions/netscript.md, "Misspelt function names and a swapped id").

Not read: how the log window shows a quest as completed or failed, by its lines or by its state; and what `0x39` does to a follower; neither was seen in the running original.
An object's `0x41` function runs as it is picked up ("The log's quest items" below): **seen** 2026-10-27 (#1256), the Seraphim's quest 15001's plant picked up, its `od_15001` gave its giver the dialogue `DLG_15001_ZIEL` (the giver's `+0x245` 181 to 183) and its "!".

### The NPCs that later events remove

Read 2026-10-25 (#356, a third pass of a subagent of the session, on the Luau of the 20 trees, their bodies compared across the trees, and the handlers below); **confirmed** by code and on the scripts unless marked otherwise.

- `SetGroupState` (73, `0x49E760`; the walker's case at `0x475ADB`) with `0x51` destroys every living creature of the group that its `0x11` names, wherever it is ("Variables and groups"); `DelNPC` (55, `0x497F80`) destroys the creature that its `0x01` names and takes the name out.
  Neither runs the creature's death's functions ("The deaths' functions", #656).
- `RegionChange` (116, `0x4ABB60`, applied by `0x4ABFE0`) takes pairs of types (`0x02`, from and to) and a number 0 to 49 (`0x69`), which it compares with the byte `+0xD7` of the creature's sector (`0x622D50`), the area (keyx `+0x2C0`, docs/re/sound.md's **hypothesis**).
  It skips an object of another area, a trader (`+0x200` bits `0x4007000`), or one of `+0x2B7` bit `0x08`, which `0x6B` above 0 sets; the others take the pair's second type, the record's flags word (keeping `0x2000`), and with `0x08` a hostile side (2, or 14 where `+0x1F0` is 7) or with `0x0E` a peaceful one (3, or 7 with the guards' mode `0x4000`), their items of the pairs' types swapped too.
  It reads no `0x6B` of its own, so the `6B:0` of quest 55's three records does nothing.
  Read again 2026-10-26 (#1064): it walks every object of the world's list (`0x5FE6D0`, `0x5FE000`), and for each the pairs in order, so that a pair whose new type is not 0 turns the type the object has then, a later pair the type an earlier one gave; an object that is no creature takes the type alone.
  At each turn of a creature, a record's flags word other than 0 replaces the creature's (its `0x2000` kept), the side follows only where the creature's hostility to the hero differs from the record's, and each of its 19 items' slots (`0x556F50`) has every pair in order turn the item's type.
- **301 (Orcs!)**: `Region3Init` creates its farmer 17033 and his daughter 17032 with four cows in the group 3012, in area 3, whose dialogues hold the win once its orcs (the group 301) are dead.
  `Init55`, the start of quest 55 (the button of `Dialog:DlgPrinzValor55`), removes the groups 3012 and 301 with `SetGroupState` `0x51`, testing nothing of 301: an open 301 whose orcs are beaten but whose farmers are not yet spoken to stays entered and cannot be won; with its orcs alive its timer still loses it (#946).
  Repaired: `Init55` loses 301 and 302 first (docs/decisions/netscript.md, "Crossing quests left able to end").
  It removes quest 302's cattle (the group 302) and robbers (3021) too, but not its giver 17029, and 302's ending runs through its death functions `tot302` and `tot2_302`, so an open 302 stays entered, for the removal runs none of them (#656).
  That the community's "start of act 2" means quest 55's start is a **hypothesis**.
- **501 (Nucquam Astralis)**: `Region5Init` creates its Lady 17070 and Lord 17071 in the group 501, in area 11, through whom both its wins go.
  `NimmtDaemonenhorn`, the end of quest 55 (`Dialog:Cinematic2`), removes the groups 501, 551 and 50013, testing nothing of 501; its ghost 17069 stays, but offers the win only once the group 501 is dead (`tot501`), so an open 501 cannot be won after quest 55, for the removal runs no `tot501` (#947, #656).
  Repaired: `NimmtDaemonenhorn` loses 501 first, and a new `QIS_OnLose501` writes the failure (docs/decisions/netscript.md, "Crossing quests left able to end").
- **1702 (Robber Baron)**: `Region17Init` creates its target 17215 hostile (creature 296, with the death function `tot1702`, which drops the item 6065 that the win needs), in area 31.
  `BadToGood`, which only quest 72's `ScriptAnimShareef` calls (The Forge of the Ancestors), has a `RegionChange` of area 31 with the pair (296, 297) and `0x0E`, which makes him a creature 297 of the peaceful side, testing nothing of 1702, so that the item can no longer be won by fighting him (#948).
  This holds where `Region17Init` ran before the forge, as the first sector of the region 17 came near (docs/re/spawning.md, "When the sectors are filled"); of the named `CreateNPC`s of the Seraphim's tree he is the only quest target of area 31 so changed, and `BadToGood` makes the bounty targets 18708 and 18712 of `Kopfgeldauswahl` peaceful too (that it blocks an open bounty is a **hypothesis**).
  Repaired: after `BadToGood`, 17215 takes back his hostile side (docs/decisions/netscript.md, "Crossing quests left able to end").
- **670 (Glubba-Jar)**: `Init81` takes the dialogue of Glubba 18308 away and his health to 0 only where bit 1 of `670` is clear, which only `QIS_OnEnter670` sets, so he dies only where 670 was never entered: the data leaves no quest taken unfinishable, and the report fits a quest declined or never offered (**hypothesis**).
  But `QIS_OnEnter670` gives him the death function `lose670`, which is empty, so his death as a follower leaves 670 entered with no way to finish (**hypothesis**: that `0x6B` 0 lets him die), and `QIS_OnLose670` writes the completion line on a loss (#950).
  Repaired: his death loses the quest, and the loss writes the failure line (docs/decisions/netscript.md, "Five quests' log records mended").
- **1202 (Magical Thistle)**: `ok1_602`, the button of quest 602's `Dialog:ziel602` once its knight 17136 is beaten, walks 17136 once, and then names 17163 in its three `NPC_Goto`s, its `Partikel` and its call of `ritterweg_602`, which deletes 17163 (`DelNPC`).
  17163 is quest 1202's target (`Region13Init`, in area 22, some 1,470 cells from the knight), whose dialogue holds 1202's win and then a trade, and nothing in 602 tests 1202: beating the knight before 1202's end deletes its target, and the quest stays entered, neither won nor lost, once its giver has deleted its timer; after its end, the trader is gone (#949).
  Repaired: the six operands name the knight (docs/decisions/netscript.md, "Misspelt function names and a swapped id").
  That 17163 is 17136 with two digits swapped is a **hypothesis**: the function's name and its first `NPC_Goto`, which names the knight, support it.
- The functions above are in the 9 trees of the Sacred campaign (`Init55`, `NimmtDaemonenhorn`, `ScriptAnimShareef`, `Init81`) and the side quests in 11 (those and `netscript`, `addon-netscript`).
- The group number 5001 is named by quest 501's 13 ghosts (`QIS_OnEnter501`), quest 5002's camp, the seven NPCs 17691 to 17697 that `Region20Enter` and `Region21Enter` make, and quest 5002's test (`od_5002`), attacks (`timer2_5002`, `OMO5002Hinterhalt5002`) and removal (`schatzsucher5002`).
  The groups are global by number ("Variables and groups"), so 5002's test answers alive while any of 501's ghosts lives, anywhere, its removal destroys them, and its attacks send group 5002 at them; nothing else names the group, so 501 itself never counts them.
  `QIS_OnEnter501` also creates again the four ghosts of the group 551 that `Region5Init` made, so `exx501` waits for eight (#951).
  Repaired: the 13 ghosts take the group 50014 of their own (docs/decisions/netscript.md, "Quests 2203 and 501 kept apart").

Not read: what `0x549080` and the event `0xC` do; none of this was seen in the running original.
A removal runs no death's function (#656); that a `DelNPC` runs none either is a **hypothesis** (it destroys the creature, `0x463A70`).

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
- `opensacred-game` follows `SetObjState`'s bar (`0x06`, `0x07`) and open bit (`0x0F`, `0x10`) since #952 (`npcs`): the object's name finds its trigger among the scripts' `CreateObj`s (`0x29`), and the state goes to the trigger whether the object is in the world or gone with its sector, its door turning where it stands and the next door of that name made in that state (docs/decisions/netscript.md, "Object states kept for a gone object's name"); with quest 2411 entered away from 38,94, its door's trigger 519 is unbarred.
- **2002 (Panem et Circensis)**: the start script sets bit 0 of `2002`, and the building's exit `OMO-1tpraus20.2c` runs `monster2002` where bit 30 is clear and bit 0 set, then sets bit 30 and clears bit 0, testing nothing of the quest.
  Before the quest is taken `2002a` is missing, so `monster2_2002` makes no creature, and Vladimir's `Dialog:auftrag2002` then holds no branch, bits 0 to 4 all clear; only `ja2002` (bit 2) and `neu2002` (quest 5001's dialogue, where bit 1 is set) set bit 0 again, so the quest can never be taken (#953; not seen in the running original).
  Repaired: the exit runs `monster2002` only once the quest is entered (docs/decisions/netscript.md, "Crossing quests left able to end").
- **10254 (Abducted)**: `QIS_OnSetUp10254` makes the note (item 4098) with the function `ot_Brief_10254` and no `0x61` or `0x8E`, unlike its sibling 10257's note; the function tests nothing, so each run makes the giver anew with its start dialogue, and accepting makes the pirates.
  The giver's death function loses the quest only once it is entered (`LoseQuest` needs the flag 4), so a giver killed before is replaced by picking the note up again, which the community reports as the way out rather than a trap; the function running at each pickup is **community-reported**, and a second giver of the same name while the first lives a **hypothesis**.
- **1404 (Rescue of a Noble Lady)**: `QIS_OnEnter1404` gives Zaphod 17190 the death function `tot1404`, which makes the key 17193 (item 6061) at `CPOS:res:17190`, and its `take1404` unlocks the two doors of `Sector38030Enter` in the same sector, so 2411's pattern does not apply; the data does not depend on where he dies.
  That the key lands on a cell or floor out of reach, or is not made, where the corpse lies on the veranda is a **hypothesis** about `CreateObj`'s placement at a creature's cell; the original is to be checked.
- **5003 (H'Runga'zz)**: `preritual5003` sets the trigger `omo_ritualstart5003` and takes the shaman's dialogue away; `OMO5003omo_ritualstart5003` deletes the trigger, teleports `HERO` to `pos_zielwaldgott5003` in sector 15,85, a dungeon by `world.ron`, makes the god there and writes the log's line.
  A mounted hero is not moved, yet the teleport answers success, the trigger is gone, and only the god's death function gives the shaman's dialogue back, so the quest is stuck (#954; that the trigger fires for a mounted hero is **community-reported**).
  `Dialog:dlg_sucher5003`'s decline button names `ablehnen5000`, which no tree defines, while `ablehnen5003` exists and is named by nothing, so a declined seeker keeps bit 2 of `5003`, and a quest later taken from the shaman takes the seeker's branch (#955; that a button naming a missing function does nothing is a **hypothesis**).
  Repaired: the button names `ablehnen5003` (docs/decisions/netscript.md, "Misspelt function names and a swapped id").
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
  Repaired in the data: the record names `res:4609` (docs/decisions/netscript.md, "Quest 10260's vase item named").

The game follows it since #1256: a `CreateObj` of `0x8E` and a type that may be picked up lays a quest's item that the hero picks up as a drop, which goes among the quest's items and runs its `0x41` function (`pickup`, `npcs::QuestItem`); one of `0x61` or `0x62` without `0x8E` gives its item there straight and runs its function, or, inside that very function, lays its object (`scripts`).
Since #1257 the scripts keep the hero's list of the main quest's items (`opensacred_rules::quests::QuestItems`): a quest's item picked up enters it by its `CreateObj`'s first name (`npcs::Placement`'s key), else by its type's key, an item given straight by the record's first name, else the key 0; an add is refused where an entry has its type or any has the key 0, as the original's; and `0x3A` holds where the hero's list holds the named key, or any entry has the key 0, never for another creature.
**Seen** in the game (#1257): quest 204's book picked up by its `take204` enters the list as `res:17640`, and `exit204` takes the Seraphim's branch.
A `CreateObj` of an item's type without `0x8E` (and without `0x61` or `0x62`) lays an item that the hero picks up as any lying item, not placed again once taken (**seen** in the original, `beweise209`'s two items at the hero's feet, one clicked into the backpack; #1296): quest 204's reward 5570 lies at the hero as `exit204` runs.
Not followed: an object in the world whose type is in the list (`0x498860`), and the creature's 256 slots (the hero's inventory, where quest 2203's barrel 17713 stays).

An item laid by a `CreateObj` of a `res:` name goes by that name's text under the cursor rather than by its type's (docs/re/cursor.md, "The chooser's tests"; #1360).
Not read: the engine's event that `CreateObj` posts before the add (`cEvent_engine` 6), how the log window draws the list, the pickup's other branch (`0x55923C`) beyond its add and destroy, and what draws the glitter over a quest's item lying in the world (seen, #1256; #1382); to check in the original: 10260 won through the right vase, then any quest item picked up, the client's log expected to say that it has the item already.

### Spawns and markers

Read 2026-10-25 (#356, a fourth pass of a subagent of the session, on the Luau of the 20 trees, their bodies compared across the trees, and the handlers below); **confirmed** by code and on the scripts unless marked otherwise.

- `CreateNPC` (`0x482510`) fails and logs where the record has no creature or no cell (`0x485A35`); for a name of its `0x01` that the map of names holds (`0x464BD0`, at `0x4830E4`) it makes nothing new, logs the name as given twice and teleports that creature to the record's cell, making a new one only where that creature is gone or has `+0x14` bits `0x30`.
  Into a sector not loaded (its `+0x30` bit 16 clear) it makes the creature with no place (`sdk_OBJ_CREATE_NPOS`) and then places it (`0x5FEE50`); during an `Enter` an unnamed creature that is not hostile is made only where the byte `0x182EE78` is set (`0x482DF0`); a new creature's type is replaced through a list per area at `+0x1050` (0xAC bytes each, `0x482F41`; the lists that `RegionChange` keeps, "`ReplaceSpawn` and `RegionChange`" above); and the creature 86 at (3275, 4108) on layer 2 loses the hostility of its `0x08` (just after `0x482DA2`).
- `CreateObj` handles a name already held the other way round: it logs a warning, destroys the older object (`0x4860E7`) and makes the new one.
- `RndVar` (105, `0x49D450`) sets its variable to `rand() % (high − low + 1) + low`; `DefPos` (23, `0x478780`) keeps the name and up to four numbers, the first two x and y, a name defined again replacing its entry; `SetBaseTrigger` (4, `0x4819A0`) orders the two corners itself, takes the layer from the third number of `0x0C` (0 to 16, else 0), and makes a box of one cell without `0x0D`.
- `GroupIsDead` (24, `0x48C860`) answers dead for a group it does not find, skips members gone or out of the group, and answers alive at the first living member wherever it stands, keeping the nearest within 25,000 at `+0x7704` (its use not read).
- A region's `Enter` (`0x492C90`) and `Exit` (`0x493040`) take the region from the hero's sector through its area by the table at `0x95CBFC` (the area 39 the region 23, 15 the region 24, 10 the region 6), as sectors enter (`0x4939F0`) and leave (`0x4954C0`) the 3 × 3, an `Enter` once per entry (its region's byte `+5`, cleared by the `Exit`); the regions' cache of names (`0x4C6F60`) holds two functions' indices, both run (**hypothesis**: how the converter's `Region<n>Enter#2` duplicates run).
- **9512 (Extraordinary Demon)**: `QIS_OnSetUp9512`, from the start script of all 20 trees, rolls `pool_9512` from 0 to 3 and places the demon 19502 at one of four cells of area 51; nothing points at it before its death, the compass set only by `QIS_OnEnter9512` once its dropped scroll is used, so in three games of four it stands elsewhere than in another (**hypothesis**: that this is the report, or its death at other creatures' hands leaving the scroll and no demon); the data is consistent.
- **7622 (Remembrance)**: `QIS_OnEnter7622` makes its target at (3223, 4731), two sectors from the giver, among creatures of its own type that `Sector50073Enter` fills, and points the compass there, testing nothing; the data does not explain the report.
- **2404 (Evacuation)**: `QIS_OnEnter2404` makes nine hostile creatures in the groups 24041 to 24043 within the giver's sectors, each with the death function `sieg2_2404`, the win needing all three groups dead; since `GroupIsDead` counts a creature never made as dead, one that failed to spawn cannot block the quest, but one alive anywhere else does, which fits a creature that follows a defender out of the camp; where both 2402 and 2403 have bit 4 set, `flucht2404` runs at once and the groups never walk in.
  The data does not explain an undead that never spawns.
- **52 (chapter 2's water bottle)**: the bottle 18261 (item 6103, `0x62`, `0x8E`) is made only by `OMO52ZombiesBeiSterbSoldat`, as the hero first enters the start script's box (2738, 4260)–(2797, 4322) whatever the quests' states; `PferdTot52`, which would make it again at the horse's corpse, and `Dialog:PferdTot52` are named by no record, though the start script lists the dialogue, so a pass through the box before chapter 2 makes the bottle then and nothing makes it again (#960; that this is the report is not shown).
  Seen in the original 2026-10-25 (#960), the Seraphim's new game in chapter 1: a walk into the box ran the function at once, the bottle lay on 2775,4295 with its sparkle's two timers running in turn every 2.5 s, and zombies came.
  Made by a function, not by an `Enter`, it is not destroyed with its sector, so it lies there from the first pass until it is taken: an early pass does not lose it, and the data does not explain the report.
  Taken while bit 2 of `52` is clear, `NimmtWasserflasche52` calls `Init52`, which exits 51 and triggers 52, so that the bottle taken in chapter 1 may start 52 early (**hypothesis**, by the data: the pickup was not reached).
- **61 (the IceCreek Pass marker)**: its compass is moved only by the 5.7 dungeon's teleports (`OMO-1tpreinHQ_5.7.*`, `OMO-1tprausHQ_5.7.*`) and by `NimmtElementLuft_2`, which sets it to the 5.7.4 mouth by Frostgard (1093, 1750) after `ElementAbfrage`, overriding its compass once the four elements are held (**hypothesis**: one compass a quest, shared by `QuestKompassPos` and `QuestKompassObj`).
  Entering 5.7.4 with bits 6 and 9 of `61` set matches none of its branches, and the exits 5.7.3 and 5.7.4 set the same cell again, so the marker points back at Frostgard's side until the hero leaves by 5.7.1 or 5.7.2 (#958; that this is the report is a **hypothesis**, a strong fit).
  Repaired: the air's compass yields to the four elements', and the entrance 5.7.4 points the way on for bits 6 and 9 (docs/decisions/netscript.md, "Quest 61's compass and quest 9006's spawn").
- **Jacquaire in Braverock (9006)**: Jacquaire 17523 is made at `auftrag_goodie5` (in Braverock) only by `OMO9006rg6_goodie1_waldrand`, where bit 11 of `goodies` is set, the trigger that `rg6_goodie1_start2` places on a 6 × 6 box in region 6 as it exits 9006; a hero who leaves by a portal or another road never makes him, and 9007 and 9011 wait on him (#959).
  The trigger is no region's border, though `Region23Enter`, which teleports him onward and sets 9011 up once 9007 is done, is one; that the box lies on one road of several is a **hypothesis**.
  Repaired: 9006's end makes him, and the box no longer does (docs/decisions/netscript.md, "Quest 61's compass and quest 9006's spawn").
- Named by no record: the debug functions `goodie5` and `setuptest`; quest 45's `region_change` and 9000's `replace_spawn` name `0x69` 15, the oasis's area, which was not checked.

To check in the original: 7622's target at its cell; 9512's demon at the four cells over several new games; 2404's groups with `creatures` once the quest stalls; the bottle taken in chapter 1; Jacquaire after leaving region 6 without crossing the box.

Not read: whether a save keeps the date and the timers, the drift seen in the running original, and a test by which a repaired drop could know that its partner quest is open (these quests set no bits of their own).
The repairs each change a quest's course, so each is a `bug` whose decision is the maintainer's: the shared drops (#930), the timers of 1202 (#931) and 1803 (#932), the calendar's drift (#933), and the quests above (#935 to #944, #946 to #955, #957 to #960).
The maintainer chose them on 2026-10-25, as their issues say: `opensacred-convert netscript` makes those of the data as it writes the trees (`script_repairs`, docs/02 §7.1), each recorded as a decision; those of the runtime (#930, #933, #952, #954) are decisions for #726; #943 and #951 were read and repaired by the converter (#971), and #960 proved no defect of the data (#970).

## `SetNPCState`

Read 2026-10-23 (#578); **confirmed** by code unless marked otherwise.
`SetNPCState` (type 3, the handler `0x48BB40`) reads its tags into a record of its own, a later tag overwriting what an earlier one of its kind wrote, and hands it to `0x461540`, which changes the creature that its `01` names (the local player's for the name it keeps for the hero; nothing without a name or a number).
In the order of `0x461540`:

- The operand-less tags make a flags word as `CreateNPC`'s do (docs/02 §7.3), but each mode replaces the word's low 16 bits, so that a mode after `0x45` takes its `0x2000` away.
  A word other than 0 replaces the creature's (`+0x1F4`) but for the bits `0x6682304`, which stay (`0x2000` and a boss's `0x80000` among them); the mode `0x800` (`0x43`) given to a trader (`+0x200` bits `0x4007000`) takes those bits away and is dropped itself.
- `0x75` is the creature's quest, the word `+0x96` (`0x450E20`), which `ExitQuest`, `LoseQuest` and the quests' hook `0x471790` compare with a quest's number.
- `0x4D` `(x, y, floor)` is its home (`+0x2B9`, docs/re/ai.md), at the cell's centre; the scripts give one to the hero too, where it rises after its death (**confirmed** 2026-10-25, docs/re/fight.md, "The hero's death"), as `StartPosition` (45, `0x491B30`) also sets it.
- `0x08` and `0x0E`, the last of them, make it hostile or peaceful by its side (docs/re/fight.md, "Sides"), and its followers with it: `0x08` puts one that is not hostile to the hero already, or an animal, on the side 2, or 14 where the record's word has the guards' mode `0x4000` or its side is 7; `0x0E` puts one that is hostile to the hero on the side 3, or 7 so.
  The test is `0x549000` against the creature's controlling player (`+0x39`, the local player's id in a game of one), where `CreateNPC` asks `0x548F60` of the hero, to the same answer.
- `0x03` turns it as `CreateNPC`'s does (`0x44DDC0`), and a word with the mode 1 wakes it (`sdk_WAKE`).
- `0x09` names a dialogue and `0x0A` gives one by its number: the walker's table of dialogues (`this+0x755C`, filled as the start script's 1,021 entries of type 40 run, the first entry added twice, `0x476A05`) is searched for the first entry of that name, case aside (`0x859690`), or whose `+0` holds that number.
  An index above 0 becomes the creature's dialogue (`+0x245`, `sdk_STAMP_IDX`), the entry's `+0x4C` taking the creature's id; a name not found yet, the index 0 or a number not found take the creature's dialogue away.
  The entries' `+0` is −1 in every one, and every `0x0A` of the scripts is `0A:0` (11,121 records): `0x0A` takes the dialogue away.
- `0x73` writes `+0x25F` (the mark, docs/re/fight.md), `0x67` `+0x2B6` and `+0x261`, and `0x6B` clears `+0x2B7` bit `0x08` for 0 and sets it above 0 (the strikes' resolution and the deaths read it with `+0x251`, not read further).
- `0x05` keeps a function's name for the creature in the script manager's list at `+0x358`, its old death's and sight functions taken out first: the function its death runs; `0x60` adds a sight function, `See:` and its name, and sets `+0x1F4` bit `0x400000` ("The deaths' functions", **confirmed** by code 2026-10-26).
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
Its death's functions are followed since #656 ("The deaths' functions"), and since #658 its heading (`0x03`, turned as `CreateNPC`'s), its group (`0x11`, which `GroupIsDead` then counts it in) and a horse's joining a trader's stock (`0x63`), which takes its dialogue away.
Since #657 its `0x37`, `0x4B` and `0x71` hire the creature as the hero's follower and its `0x39` and `0x65` let it go, as `CreateNPC`'s `0x37`, `0x4B` and `0x71` hire too (docs/re/ai.md, "The scripts' hired followers" and "In `opensacred-game`").
The play's `SetNPCState`s give these to a creature made before them too, its placement and the creature in the world (`npcs::Npcs::restate`).
Its quest (`0x75`) serves only the dynamic quests' table, which the release never fills ("The dynamic quests"), and is left out; `0x6B`, by which a dead follower rises, and `0x67`'s healer are not followed.
Not followed, and why: the stock itself, for the game has no trade yet; `0x36`, which serves nothing; `0x73`'s mark of 64, the game taking a creature's mark by its type (`opensacred_rules::minds::marked`), and no `SetNPCState` of the Seraphim's tree carrying it; `0x90`'s byte `+0x400` and `0xA1`'s `+0x14` bit `0x200000`, whose readers are not read; `0x7F`, `0x80`, `0x84`, `0x8C` and `0x85`'s effect, not read.
In the Seraphim's tree the `SetNPCState`s carry `0x03` 4 times, `0x11` 3 times and `0x63` 59 times (the stables' horses, of no dialogue already).

## The anim mode and the creatures' queues

Read 2026-10-27 (#1220) in `Sacred.exe` and seen in the original under the driver of #448 (the novice's way out of the chapel after her first dialogue, `QIS_OnEnter9` of the Seraphim's tree, run by `exit01`); **confirmed** by code unless marked otherwise.

`SetAnimMode` (88) runs nothing itself: it turns on the "anim" bit of the script manager's mode word (`0xAACF80 + 0xA45C`, bit 1), under which the records that name a creature wait their turn in that creature's own queue instead of acting.
`SetCinemaMode` (134) turns on the same bit and the "cinema" bit 2, and stops the local player's hero where its queue is empty.
`SetPlayMode` (89) with no creature turns both bits off at once, waiting for no queue; naming a creature while the mode is on, it queues an end marker instead, and the mode ends as that creature's queue reaches it (`0x4A4040`, `0x59E926`).
Every `SetPlayMode` of the Seraphim's tree (76) names none, so its mode always ends within the same function's run.
Both post the engine's event `0x13` of subtype 3 with 1 or 0, which the interface manager takes as a flag (`0x757840`'s case 5; **hypothesis**: the interface locked while it holds, faded back after a mode of a second or more); turned on and off within one frame it changes nothing on the screen (seen).

- The records that queue, each testing the anim bit and adding an entry of 68 bytes to the creature's vector `+0x588` (its first word the entry's kind): `NPC_Goto` (72, kind 1, `0x49E210`), `Wait` (90, kind 2), `PlayAnim` (91, kind 3), `Attack` (92, kind 4), `SetPlayMode` with a creature (kind 5), `ActObj` (93, kind 6), `SetVarBit` and `UnsetVarBit` naming a creature (kinds 7 and 8), `Teleport` (46, kind 9, `0x491D40`), `Morph` (79, kind 10), `CreateObj` (8, kind 11), `CallFunktion` with a creature (22, kind 12), `Partikel` (99, kind 13) and `PlaySound` with a creature (104, kind 14).
  Every other record acts at once, the mode on or not: `SetNPCState`, `SetTimer`, `Popup`, `GroupGoto` and `NPC_TalkTo` among those of the Seraphim's spans.
  The kind 14 is `PlaySound` (104, `0x4A9730`) naming a creature, not the type 100 (`SpawnValues`, `0x4A9670`, which never queues), so a `PlaySound` acts at once only where it names no creature (read for #1276).
- The creature's update (`0x5375C0`) calls the queue's step (`0x59A840`) while its queue is not empty, once a tick of the world (about 0.1 s, seen), which takes the first entry alone (a switch on its kind, `0x59F390`); an entry done is removed and the rest shift down, a queue of more than 500 is emptied, and the queue is saved with the creature (`0x54B3D0`).
  While it is not empty, the creature's idle handler (`0x542B20`, at `0x542D9E`) and its standing action (`0x544890`) leave their own behaviours; a fight goes on.
- A walk (kind 1, `0x59A885`): while the creature fights or strikes it waits; moving, it is steered on to its target; standing, it ends once the creature stands within 40 of the target along each axis (less than a cell) and on the walk's layer, else it is ordered again, at a run (`+0x1F8` 2; at a walk with the record's `0x66`), and its count goes up, the creature's place noted as the count starts.
  Past 5 orders the walk is dropped where the creature has not moved since, and counted anew where it has: the only time-out.
  The record's layer 0 is the creature's layer as the record runs (`0x49E210`).
- A teleport (kind 9, `0x59EF1A`) puts the creature on its cell and layer as written, 0 the ground, turned to its heading, at once.
- A wait (kind 2, `0x59CAA5`) holds until its condition holds: while the mode is on (`0x58`, the creature stopped meanwhile), while or until another creature fights, lives or stands idle (`0x59` to `0x5C`, the other may be itself), until a variable's bit is set or clear (`0x5D`, `0x5E`), or for a time in seconds, in hundredths where negative (`0x5F`).
- A clip (kind 3, `0x59E151`) waits while a clip of the scripts plays, then starts its own and is done at once: a walk after it does not wait for it, a second clip does.
- The player is not held by the anim mode: the hero ran at a click while the novice's queue ran (seen); the cinema bit alone refuses the player's input on the world (`0x6172C0` at `0x6172ED`) and stops the hero at each tick (`0x539613`), in the Daemon's two functions of the Seraphim's tree.
- Seen: the novice ran from the chapel's floor 1 to its entrance cell 3237,2519 (0.3 s), stood, was put on the exit cell 3237,2520 on the ground at the next tick, and ran 4 cells down to the steps at 3237,2524 (0.85 s), at her type's run speed (276 for 170 × 1.25 × 1.3); her second walk, its layer the floor 1 she stood on as the record ran, never counted as arrived and was dropped 2.4 s later, as she stood on the steps.

In the Seraphim's tree 65 functions hold 74 spans, 72 of `SetAnimMode` and 2 of `SetCinemaMode`, each closed by a `SetPlayMode` of no creature in the same function; in them 84 `NPC_Goto`s, 43 `PlayAnim`s, 41 `CallFunktion`s, 14 `Wait`s, 13 `Teleport`s and fewer of the other records.

### The other kinds, the waits and the cinema

Read 2026-10-28 for #1276 in `Sacred.exe` by a subagent of the session, code only; **confirmed** by code unless marked otherwise.

- **The frame.** Every case of the step ends in one of two tails: `0x59F376` keeps the entry (it waits) and `0x59F2AA` removes it (it is done); after a removal that leaves entries the creature's AI state and sub-state are set to 0, unless the state is 6 or 11, its action left as it is.
  So a creature takes at most one entry a tick, and an entry done at once lets the next start at the next tick.
  The kind 0 is removed at once; the kinds 3, 5, 6, 7, 8, 10, 11, 12, 13 and 14 are done at once; 1, 2, 4 and 9 may wait.
- The records' handlers through the walker's table `0x4784C0`: `SetPlayMode` 89 `0x4A4040`, `Wait` 90 `0x4A1F20`, `PlayAnim` 91 `0x4A2550`, `Attack` 92 `0x4A2B40`, `ActObj` 93 `0x4A4310`, `Partikel` 99 `0x4A8BB0`, `PlaySound` 104 `0x4A9730`, `CallFunktion` 22 `0x4813D0`.
- **`PlayAnim`** (kind 3, the step `0x59E151`): the first `0x01` the creature (`HERO` the local hero), a second `0x01` another object; the first `0x0B` the slot, a second `0x0B` a count, 1 by default; the name `schmiede` opens the smith's window instead (`UI_WND_BLACKSMITH`); nothing without a name, a creature or a slot above 0.
  Outside the mode the creature is sent the event `0x108` with the slot and the count (its handler not found; **hypothesis**: the same order, the count the clip's loops).
  In the mode the entry waits while the creature stands (the action 0) in the sub-states `0x64` to `0x66` (a one-off clip ordered or playing: the scripts', the instrument's of docs/re/ai.md, the pick-up's alike), then sets the action 0, the sub-state `0x65`, the slot at `+0x154` and its loops at `+0x158` to 1, and is done: a queued clip plays once whatever its count; a creature walking does not make it wait, its walk overwritten.
  The standing action plays it (`0x544B6A`) through `0x593320` blended in over 0.25 s at the creature's attack speed (`0x584FE0`) times `+0x2C9` over 100 (times 1.8 for the slot 172, docs/re/items.md); a creature with no clip at all starts it unblended at 1.0; then the sub-state `0x66`.
  It ends (`0x544C58`) by Granny's times of the creature's animation, which stop with the world: a slot outside 53 to 65 at its end, the standing slot (`0x5467A0(0)`) blended in over 0.05 s, looped, at 1.0 (`0x544F06`); the slots 53 to 65 (the defending row) 0.2 s before their end, blending to standing over 0.15 s (`0x545204`); a clip with more than 150 s left (50 s for 53 to 65) is ended at once.
  While the sub-state is 100 or more the idle handler leaves the creature be, so that idle behaviours do not cut the clip once the queue is empty.
  The slot 255 makes the creature carry the object of the second name in its right hand (`+0x1D8`, `Bip01 Pelvis` to `Bip01 R Hand` by `0x44C670`, the slot 175 looped) or put it down again (the Underworld's `nimm_vilya` alone).
- **`CallFunktion`** naming a creature (kind 12, the step `0x59CE5B`): the first `0x01` the function, the second the creature; without the anim bit or a creature it runs at once (`0x46BA90`); in the mode a creature not in the world runs nothing, else the function is looked up as the record runs (its index at `+4`, 0 for an empty name, −1 for none) and run by the step with the creature as the walker's object, done the same tick, waiting for nothing.
  So in the `audiomeldungen` functions the line plays as its creature's queue reaches it, after any walk or clip queued before it.
- **`Wait`** (kind 2, the step `0x59CAA5`): the first `0x01` the creature whose queue takes it, the second the other (`HERO` allowed); one of the tags `0x58` to `0x5F` the condition; outside the mode it does nothing (quest 72's `Wait` after its `SetPlayMode` is a no-op).
  The entry keeps the condition (`+4`), the other's id (`+8`) and a value (`+0xC`): for `0x5D` and `0x5E` the variable's index (found as the record runs; a variable missing queues nothing) and the bit's number shifted by 16; for `0x5F` a deadline of the real clock (`0x6413E0`, the frame's performance counter `0x641520`), not the world's ticks.
  The conditions (the table `0x59F538`), any of `0x59` to `0x5C` done where the other is gone: `0x58` while the anim bit holds; `0x59` while the other fights (its AI state 4 or 8 with a target `+0x104`, or its action 3 or 5 or AI state 6 or 11 with a target `+0x158`); `0x5A` while it lives (`0x549080`); `0x5B` while it stands idle; `0x5C` until it stands idle (the action 0 and the AI state 0, their sub-states at most 1, so that a script's clip, `0x65` or `0x66`, is waited for too); `0x5D` while the variable's bit is set; `0x5E` while it is clear; `0x5F` a number `n` above 0 until the clock's whole seconds pass the record's whole second and `n`, below 0 until its hundredths pass the record's and `|n|`.
  For `0x58` to `0x5C` the waiting creature takes `+0x200` bit `0x10` (it takes no foes' event `0x100` and does not part, docs/re/ai.md) and is stopped (`0x54E6D0(1, 0)`) at each tick its AI state is not 0, the bit cleared as the entry ends; for `0x5D` to `0x5F` it is neither.
- **`Attack`** (kind 4, the step `0x59E3D6`): `0x01` the attacker and a target, `0x11` groups, `0x04` a cell or `0x20` a world point, `0x67` an art or a spell by name (`+0xA880`); outside the mode the attacker is stopped, made hostile (`+0x1F0` 13, `+0x4CF` 1) and sent the event `0x100`, or `0x105` with an art.
  In the mode the entry ends at once where the target is dead or not hostile to the attacker (`0x549000`); with an art it sends `0x105` once and waits while the attacker's AI state is 6 or 11 or its action 4; without one it sends `0x100` at each tick the attacker does not target it and waits until the target dies.
  Its test of the art is unsigned, so an `Attack` without `0x67` (−1) takes the art's path with 65535 (**hypothesis**: ended at the next tick); a group attacking a named target puts its entries in the target's queue, which never end.
- **The others, done at once**: `SetPlayMode` naming a creature (kind 5, `0x59E926`) queued only while the bit holds and the creature is found, the mode then ended at its turn (none in the 20 trees); `ActObj` (kind 6, `0x59E9D5`), a creature and an object, the tags `0x07`, `0x0F`, `0x26` and `0x2D` opening (the default) and `0x06`, `0x10`, `0x27` and `0x2C` closing, a mount (`0x4266F0`) mounted or left through the creature's event 7, a door's trigger set or cleared with its slot 170 or 171 (quest 1602); `SetVarBit` and `UnsetVarBit` naming a creature (kinds 7 and 8); `Morph` (kind 10, the type replaced and the creature made again); `CreateObj` whose second `0x01` names a creature (kind 11, `0x486CB2`, made by `0x5FBA40` at its turn, a colour's tag queueing a kind 13 after it; quest 72's five objects); `Partikel` naming a creature (kind 13, an FX event `0x50` in its colour at the creature); `PlaySound` naming a creature (kind 14, played once by `0x693510` at the sample's rate and the volume 1.0, where without a creature it goes through `0x693FE0`, the lines by name, docs/re/sound.md).
- A teleport (kind 9) also waits while its cell's sector or region is not loaded (`0x59F094`, `0x59F0B4`), and moves the creature's followers and its mount with it (`0x51CE60`, `0x556F50(0x12)`).
- **The modes.** `SetAnimMode` (`0x47738B`) and `SetCinemaMode` (`0x47740E`) post the event `0x13` of the subtype 3 with 1 and set the bit 1, or the bits 1 and 2, of `0xAB73DC`; `SetCinemaMode` then stops the local hero (`0x54E6D0(1, 0)`) where its queue is empty.
  The anim bit is read only by the 14 queuing handlers and the wait `0x58`; the cinema bit only at `0x6172ED`, where the engine's mouse handler does nothing (no move, attack, talk, art or pick-up from the mouse), and at `0x539613`, where the local hero whose queue is empty and whose AI state is neither 0 nor 10 is stopped at each tick (a byte scan of `.text`).
  The interface (`0x757840`'s case 5) notes the game's clock with 1 and sends its input to one window alone (**hypothesis**), and with 0 returns at once, or faded where a second or more passed.
  In the eleven trees of the Sacred campaign every span closes in its own run, so the per-tick stop, the mouse's refusal and the interface's flag never show there, the stop at `SetCinemaMode` and the queues alone outliving the run; the Underworld's and the multiplayer trees keep the mode on (`hq15_close_gate`, `hq26_close_gate`, `Dialog:vilya_dankt_held`).
- **`NPC_Goto`'s creature** (`0x49E210`): each `0x01` replaces the one before, so the record walks the creature of its last name; it does nothing without a name or without a cell or a point (`0x04`, `0x20`), the target's x read as −1 (probed at `0x49E39D`, 2026-10-28, #1280).
  Outside the mode it sends the creature the walk's event at once, at a run or at a walk with `0x66`.
- **The Daemon's start** (**seen** 2026-10-28, #1280, its first window pressed through the driver's `ui press`): the first window's OK runs `Tutorial_Trigger` and its Skip Tutorial `btn_Deamonstart`, which runs `TT_KEINEMEHR`, stops the hero for the cinema (`0x54E6D0(hero, 1, 0)`, traced) and walks nobody.
  Its `NPC_Goto` names `HERO` and then `res:Anducar01` and carries no cell, so the handler takes Anducar with no target and queues nothing; `Daemonin_Tutorial`'s names Anducar and then `HERO`, again with no cell.
  No queue's step ran (`0x59A840`), the hero stood on its cell 352,1722 and Anducar on his for the 14 s watched; the earlier hypothesis that the hero walks to Anducar is refuted.
- **The hero's own queue** (**seen** 2026-10-28, #1280): the local hero's update steps its queue as a creature's.
  `annehmen2003` (the accept button of `Dialog:ziel2003`, every hero's tree), the one span of the Sacred campaign that moves the hero, queues its `NPC_Goto` of `HERO` to 3986,1813 and a `Teleport` of no creature to 3432,5455.
  Run by the driver's `script` on the Seraphim's start, the hero's queue stepped once a tick (38 steps in 4.2 s), its walk ordered it at a run and set its gait to running (`+0x1F8` 2, kept after), it ran from 3236,2511 to 3227,2517 out of its floor's way and stood, the walk was dropped after its orders without moving, and the teleport then put it on 3432,5455.
  So a `Teleport` of no creature, the local hero's, waits in the hero's queue under the mode.
- Not read: the event `0x108`'s handler, whether the queue's step runs before or after the action's in `0x5375C0`, the creatures' order in a tick, the walker's object in a called function's records, a script's clip's rate per creature type, the art's path of an `Attack` with no `0x67`, and whether a death empties a queue.

#1232 follows these in its sub-issues: the queue of every kind and `PlayAnim` (#1277), `CallFunktion` naming a creature (#1278), `Wait` (#1279), the cinema and the hero's own queue (#1280) and the rest (#1281).

In `opensacred-game` since #1220 (`npcs::ScriptQueue`, `npcs::take_steps`): the play's `NPC_Goto`s and `Teleport`s between a `SetAnimMode` or a `SetCinemaMode` and a `SetPlayMode` wait in their creature's queue, one step at each tick of the world, the creature's idle thought and behaviours left meanwhile (`ai`); a walk runs and ends within 40 along each axis on its level, or is dropped after 5 orders standing still; a teleport puts the creature at once.
A walk to a cell of no floor goes onto the creature's level as it sets out rather than as the record ran, so that the novice's walk after her teleport down ends on the steps rather than being dropped there.
Since #1277 a `PlayAnim` waits its turn in the queue as well: it waits while a one-off clip of the creature plays (the scripts' or a juggler's instrument's), then starts its clip and is done, the creature's walk dropped (`npcs::ScriptClip`); the clip of its slot is found among its model's motions and added to its animation graph the first time, blended in over 0.25 s at its attack speed's hundredth, and ends at its end, or 0.2 s before it for the slots 53 to 65, the creature standing again; meanwhile its idle behaviours and its stance leave it be.
Outside the mode a `PlayAnim` plays its clip at once, its count of times (**hypothesis** for the event `0x108`).
`Attack` and `ActObj` take their turns in its queue, one a tick, so that the records after them wait as long as in the original; what they do is #1281's.
Since #1279 a `Wait` holds its creature's queue while its condition holds (`npcs::Until`): while another creature fights, while it lives, while or until it stands idle (no walk, no fight, no scripts' clip and no instrument; the hero idle where it goes nowhere), each done where the other is gone; for a number of seconds of the real clock from the record's whole second, of hundredths where negative; the waiting creature stopped at each tick meanwhile for the conditions on another creature.
The mode's own condition (`0x58`) is done at once, as no span of the Sacred campaign keeps the mode past its run, and the variables' bits (`0x5D`, `0x5E`, in no span of the Seraphim's tree) are not followed and done at once; the waiting creature's `+0x200` bit `0x10` (it takes no foes' events) is not followed either.
Since #1278 a `CallFunktion` naming a creature under the anim mode waits its turn too: `opensacred-scripts` asks its host before it runs a call (`Host::defer`), and the game hands such a call to the world as a record of the function and the creature (`scripts::CALL_FUNKTION`), which waits in that creature's queue; at its turn the function runs, found by its name, its records handed to the world as a play's run (`scripts::run_due`), done the same tick; a creature not in the world runs nothing, and outside the mode or without a creature the call runs at once as before.
The mode is followed within a run, from a `SetAnimMode` or a `SetCinemaMode` to a `SetPlayMode` naming no creature, as every span of the Sacred campaign's trees closes in its own run.
Seen in the original and the game (#1277): `OMO-1malorka_dance` after `Dialog:malorka_werbung` makes the five dancers go through their clips one after another in both.
Since #1280 the hero has a queue of its own (`npcs::take_hero_steps`): under the mode an `NPC_Goto` of the hero and a `Teleport` of the hero or of no creature wait in it, the walk ordering the hero to its target at a run and setting the player's gait to running, ended within 40 along each axis on its level or dropped after five orders without moving, the teleport putting the hero as the scripts' `Teleport` puts a player; outside the mode an `NPC_Goto` of the hero sends it at once.
An `NPC_Goto` walks the creature of its last name, and a `SetCinemaMode` stops a hero whose queue is empty.
Seen in both games: the Daemon's `btn_Deamonstart` leaves the hero and Anducar where they stand, and `annehmen2003` from open grass sends the hero running toward its far target, its teleport waiting behind the walk.
Not followed yet (#1232, #1281): what `Attack` and `ActObj` do in their turns, the reset of the AI's state after an entry, a `SetPlayMode` naming a creature (it ends the mode at once), `NPC_Goto`'s walk with `0x66`, the cinema mode's per-tick stop of the hero and its refusal of the mouse, the interface's flag, and the queues saved with the creatures.
The first window of a new game, the Daemon's Skip Tutorial among its buttons, is not shown in `opensacred-game` yet.

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
