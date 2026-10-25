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
