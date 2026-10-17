# NetScript: the reader of the original

Recovered from the GOG `Sacred.exe` 2.28 with capstone on 2026-10-17 (`tools/analysis/scripts/netscript_dump.py` checks the dispatch tables and reads the keyword resolvers; the handler widths below were read off their code). The record format is in [docs/02 §7](../02-data-formats.md#7-netscript-scripts); this document describes how the client reads it. Status: **confirmed** on the code and on all 2,203,278 records of the 20 trees, unless marked otherwise.

## Record walker `FUN_00475680`

- The walker copies a record into a stack buffer before it handles it. Records longer than 500 bytes (0x1F4) take another path, which was not followed. The longest record in the release is 169 bytes.
- It dispatches on the record type through a 141-entry jump table at `0x4784C0` (bound check `cmp eax, 0x8C`). 18 entries go to the default case `0x4784B3`, which returns 1 and continues the section.
- Type 40 copies the 80-byte body as is into a table of 0x50-byte entries at `this+0x755C`.
- Type 41 reads the body as a u16 and accepts 1..5, which it dispatches through a jump table at `0x478768`. Before that it checks the state at `this+0x94` and an entry of the table at `this+0x31C`. What the five cases do is not analyzed yet.
- Type 42 goes to the default case.
- Type 43 returns 0 and ends the section.
- Every other record reads its operands through the operand reader, one call per operand.

## Operand reader `FUN_00472BC0`

**Calling convention.** `__thiscall reader(this, u32 *cursor, record *rec)`. `*cursor` is the offset of the next tag from the start of the record, header included. The reader returns 0 when `*cursor >= rec->length`.

**Dispatch.** The tag byte `t` dispatches in two steps:

- `t - 1` above 0xA0 jumps to the error exit `0x475504`, which logs an error and returns 0. This covers tag 0 and every tag from 0xA2 up.
- Otherwise the byte table at `0x4755D4` selects one of the 40 handlers of the jump table at `0x475534`.

**Results.** Each handler stores the value in the reader's slots: numbers at `this+0xA860…`, strings at `this+0xA460…`. It then advances `*cursor` past the tag and the payload, logs the operand and returns the tag.

**Handlers.** The table lists every handler with its cursor advance, payload first, then the tag byte. "str" is a NUL-terminated string.

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

**Variable references.** `FUN_00453970` reads the bytes `9F EF BE ED FE` followed by a name string and looks the name up in the variable vector at `this+0x7550` (entries of 0x24 bytes: `char name[32]`, `i32 value`). It moves the cursor past the name whether or not the name is found.

- If the name is not found, the handler of tag 0x1F reads 4 more bytes as the number. The data relies on every reference resolving (**hypothesis**: the script defines its variables before using them).
- The handler of tag 0x9F reads 4 bytes at the tag itself when the reference does not resolve.

**Truncated operands.** A payload that runs past the record end is read from whatever follows the record in the copy buffer. In the release, 11 records of type 50 (`TriggerPatch`) end with tag 0x2A and no payload; each names `chestTrigger`, one in the `StartCode.bin` of each of the 9 Underworld trees and of both multiplayer trees. The handler of 0x2A reads 12 bytes past the record.

## Keyword resolvers of the compiler

The script compiler built into the client maps keywords to record types in five functions. Each function fills two parallel stack arrays: names ending in a null pointer, and codes. It compares the token with each name through `0x451AD0` and returns `codes[i]`.

| Function | Names → codes | Keywords |
|---|---|---|
| `0x451BE0` | `esp+0x0C` → `esp+0x104` | 61 statements (`AddExp` … `MachVersteck`) |
| `0x452370` | `esp+0x14` → `esp+0x108` | 60 statements (`CreateNPC` … `GiveSkill`, `IF`, `ELSE`) |
| `0x452130` | `esp+0x0C` → `esp+0x2C` | 7 quest directives: `MaxOffen:` 27, `Delay:` 28, `Rettungsmission:` 29, `ToDoBloecke:` 30, `Schatzsuche:` 34, `Toete_NPC` 101, `Begleite` 102 |
| `0x452200` | `esp+0x0C` → `esp+0x28` | 6 quest sections: `Text:` 26, `Dialog:` 31, `Belohnungen:` 32, `HideTmpToDo:` 33, `SolveTime:` 122, `Strafen:` 123 |
| `0x4522C0` | `esp+0x0C` → `esp+0x1C` | 3 dialog commands: `ExitDlg` 43, `DelDlg` 44, `ToBegin` 111 |

**Coverage.** Together the resolvers name 135 of the 141 types (`opensacred_formats::netscript::KEYWORDS`). `DelNPC` and `DelOBJ` both compile to 55. Types 0, 39, 40, 41, 42 and 47 have no keyword: the compiler emits them itself. Types 39–42 occur in the release.

**Corrections to the earlier readings:**

- 43, 44 and 111 are dialog commands. Earlier readings had 43 as a guard bracket, 44 as "the dialog dispatcher" and 111 as "end of section".
- 122 and 123 are `SolveTime:` and `Strafen:`.

## Operand signatures

The tags each record type carries, over the 2,203,278 records of the 20 trees (`netscript_dump.py --signatures`, 2026-10-17): the 30 commonest types, which hold 88.3 % of the records, with the number of distinct tag sequences and the two commonest (`-` for none, the count in brackets). Types 39 and 40 are no tag streams (above). What the tags mean in each type is the work of M4; `CreateNPC` is read in docs/02 §7.3.

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

## Differences from the openheilig table

openheilig's tag table (MIT; derived from a Linux build with addresses `0x08xxxxxx`) differs from the GOG client in eight places:

- 0x1F carries 4 bytes, not 3, or a variable reference.
- 0x9F is a variable reference, not 3 bytes.
- 0x92 carries two strings.
- 0x37 and 0x4B carry a byte and an optional string.
- 0x71 is a marker.
- 0x0B, 0x38 and 0x5F can carry a name after the sentinel 0xFFED2979.
- 0x6F carries an i32 before its string.
- In openheilig, an unconsumed tail ends a record at a 0x00 tag. With the GOG table, every record of the release consumes its body exactly.
