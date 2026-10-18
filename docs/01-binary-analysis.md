# Analysis of the executables

Plan document 01.
Subject: the GOG.com release of Sacred Gold 2.28 (English), the only version the project works with, as installed in the user's own `OriginalGame/` directory and checked against the SHA-256 manifest `docs/manifest-gog-2.28.txt`.
All numbers come from static analysis by the `tools/analysis/pe/*.py` and `tools/triage/pe_triage.py` scripts ([tools/README.md](../tools/README.md)).

Why this document exists in the new revision of the plan: the game is not rebuilt from recovered sources but rewritten in Rust (PLAN.md, §4).
The binary is needed as a source of rules and content semantics.
The document describes what the binary contains, how much it costs to analyze, and what does not need to be analyzed.

The status of claims is marked with these words: **confirmed** — there is an address, offset, command output or primary source; **community-reported** — taken from sacred-bild / sacred-sdk / openheilig and not checked on the file; **hypothesis** — the author's inference, needs checking.

## Summary

* The GOG release: the client `Sacred.exe` (11,919,360 B, SHA-256 `db9b86a4…71e74`, VC6 SP5 + Processor Pack, no DRM), `GameServer.exe` (1,900,544 B, not packed, RTTI intact), `Config.exe` and third-party DLLs: Granny 1.2b, Miles 6.6g, TinCat 2.0.28, IJL 1.51, libxml2, MSLU.
* The GOG client has no protection at all and is analyzed as is: 4 sections, entry point RVA 0x44D964, no traces of ProtectDISC.
* Client code: 1,462,718 instructions; after normalization **≈5,900 game functions in 3.48 MB (≈180k logical lines of C++)**, 410 vtables and 417 RTTI classes, ~1,300 `dynamic_cast`s, 2,539 EH frames; CRT, STLport 4.x, zlib 1.2.1 and D3DX7 are statically linked in.
* On the order of **2,000–3,000 functions** need analysis: rules, items, AI, the script interpreter (§8).
  Rendering, UI, sound, input, networking and the file layer are written from scratch without reading the original's code.
* `GameServer.exe` is an auxiliary binary: 1,176 game functions (227 KB) are byte-identical to the client, which helps confirm function boundaries and classes.
  The server itself is not ported: networking is out of scope.
* Behavior details useful for the reimplementation: random numbers come from the MSVC CRT LCG; object positions are integers; x87 precision in the original was not fixed, so bit-exact results are not needed (§7).

## 1. Contents of the release

The installation is the GOG release of Sacred Gold 2.28 (English) in `OriginalGame/`.
Executables and libraries:

| File | Size, B | Toolchain | Timestamp (UTC) | Packing / protection | Role | Status |
|---|---|---|---|---|---|---|
| `Sacred.exe` | 11,919,360 | VC6 SP5 (199 objects) + Processor Pack (18) + libraries, link 6.00.8447 | 2006-10-13 12:25:43 (0x452F85C7) | none (no DRM, 4 sections, entry point RVA 0x44D964); no relocs, TLS or debug-dir | client; NetScript interpreter | confirmed on file (§2) |
| `GameServer.exe` | 1,900,544 | VC6 SP5 (55) + SP6 (22) + libraries | 2006-10-13 12:24 (0x452F8580) | not packed, `.text` entropy 6.57, RTTI 259 descriptors | dedicated server | confirmed |
| `Config.exe` | 151,552 | VC7.1 (VS2003), PDB path `d:\Source\Tools\SacredConfig\Config\Release\Config.pdb` | 2005-06-22 | — | external configurator (dialog 134: fullscreen, movies, GFX32, filter, sound, compat video, logging, old lobby; there is no resolution dialog) | confirmed |
| `granny.dll` | 466,944 | RAD Granny **1.2b**, link 6.0 | 2003-11-27 | — | meshes, skeletons, animation (101 exports `_Granny*@N`, 54 imported) | confirmed by static analysis: 101 exports; version 1.2b from the `GrannyOpenVersion("1.2b", "win32", "10-4-2000")` call in the client (0x4010D0) and the `1.2b` string in the GRN files inside `models.pak` (the call's arguments: `tools/analysis/pe/granny_args.py`) |
| `mss32.dll` + `mssmp3.asi` + `mssds3d.m3d` | 374,272 + 149,504 + 56,320 | Miles Sound System 6.6g | 2004-12 | — | sound: MP3 streams, samples, 3D via DirectSound3D (62 `AIL_*` imports, 57 used) | confirmed |
| `tincat2.dll` | 335,872 | TinCat 2 Network System 2.0.28 (instance four), MSVC 7.1 | 2005-03-08 | — | network transport and lobby (4 imports: `TinCat_CreateAPI`, `TinCat_CreateTinCatValues`, `TinCat_Scramble`, `TinCat_GetBuildNumber`; the rest via a virtual interface) | confirmed |
| `ijl15.dll` | 352,256 | Intel JPEG Library 1.51 | — | — | JPEG (4 ordinals; hero portrait) | confirmed |
| `libxml2.dll` + `iconv.dll` | 770,048 + 872,448 | libxml2 (2003) | — | — | `guiconfig.xml` (12 imports, 154 calls) | confirmed |
| `unicows.dll` | 245,408 | MSLU | — | — | Unicode layer for Win9x; the MSLU loader is linked into the exe | confirmed |
| `MSVCRT.DLL`, `MSVCP60.DLL` | 323,072; 401,462 | — | — | — | **not imported by the game**; needed by `libxml2.dll` and possibly other DLLs | confirmed |

Specifics of the GOG release that matter for the converter and the engine:

* `EULA.txt` (the GOG user agreement) and `license.rtf` (the license of the publisher, Ascaron) are license agreements; they are not quoted in the project documents, and the converter ignores them.
* `Gold.jpg`, `drache.jpg` are illustrations; the game does not load them, and the converter ignores them.
* `pak/loading0.bmp`, `pak/loading1.bmp` are 1024×768, 24-bit loading screens.
* 29 `QuestCode.bin` and `QuestPoolCode.bin` files in `bin/` have zero size; the converter treats them as "no script".
* `Settings.cfg` in a clean installation: `GFX32 : 1` (32-bit color), `GFX_LIMIT128 : 1`, `COMPAT_VIDEO : 1` (§4.5), lobby `sacreduweng.ascaron-net.com` (does not work and is not needed).

The client's VERSION resource is "Sacred" 2.0, SpecialBuild=ENG.
The build path `F:\Buildserver\Lord_2_28_ProtectDisc6\Source\engine\object\balancing\balancing.cpp` gives the code name `Lord`, the number 2.28 and the branch name for ProtectDISC 6 (the protection itself was not applied).

Other known builds (community-reported): German retail 2.0 (0x451BBE74 = 2006-09-28), Russian retail Gold 2.28 (a separate binary, the addresses differ), the official 2009 LGP Linux port (stripped i386 ELF, Itanium RTTI with 318 classes; a supplementary, not a primary, source).

## 2. The GOG client

Checked on the file `OriginalGame/Sacred.exe` (`tools/triage/pe_triage.py`, pefile, `python3 -I`, read-only):

| Parameter | Value |
|---|---|
| Size | **11,919,360 B** |
| SHA-256 | `db9b86a44c8c1cc8c5ed9e3f427121ab8b4ed62760861e58995f10af9d971e74` |
| PE TimeDateStamp | 0x452F85C7 = 2006-10-13 12:25:43 UTC |
| ImageBase / AddressOfEntryPoint | 0x400000 / **RVA 0x44D964** (VA 0x84D964); the entry bytes `55 8B EC 6A FF 68 …` are VC6 `WinMainCRTStartup` |
| SizeOfImage / CheckSum | 0x196C000 / 0 |
| Data directories | only imports (16 DLLs), resources and IAT; no relocs, TLS or debug-dir; no overlay: the file ends exactly at `.rsrc` |
| Protection | **none**: no DRM; ProtectDISC, SecuROM, SafeDisc, StarForce not found (the only mention of ProtectDISC is the build path, §1) |

| Section | RVA | VirtualSize | File: offset / size | Characteristics | Entropy | SHA-256 (prefix) |
|---|---|---|---|---|---|---|
| `.text` | 0x1000 | 0x48E632 | 0x1000 / **0x48F000** (4,780,032 B) | 0x60000020 | **6.570** | `d42afcb13dd332a7` |
| `.rdata` | 0x490000 | 0x56846 | 0x490000 / 0x57000 | 0x40000040 | 4.448 | `def14aeddd2947ea` |
| `.data` | 0x4E7000 | 0xF4A794 | 0x4E7000 / 0x13D000 | 0xC0000040 | 3.669 | `74e4f886d37c46ca` |
| `.rsrc` | 0x1432000 | 0x539A18 | 0x624000 / 0x53A000 | 0x40000040 | 5.796 | `2be48cac5cb37516` |

The `tools/analysis` scanners read `.text` as is.

`Sacred.exe` and any dumps of its memory are never committed or published ([docs/08-risks-legal.md](08-risks-legal.md)).

## 3. Compiler and static libraries

### 3.1 The Rich header

Decoded with `pefile.parse_rich_header()`, comp.id values resolved via the richprint table (confirmed):

| comp.id → tool | `Sacred.exe` | `GameServer.exe` | `Config.exe` |
|---|---|---|---|
| `[C++] / [C]` VS6 SP5 (build 8966) | **199 / 13** | **55 / —** | — |
| `[C++] / [C]` VS6 SP6 (9782) | — | 9 / 13 | — |
| `[C++] / [C]` VS6 SP5 Processor Pack (9044) | 17 / 1 | 14 / 1 | — |
| `[C] / [C++]` VS6 libs (8047) — static libraries | 181 / 15 | 153 / 14 | — |
| `[LNK]` VC6 SP5+ linker / imp-exp (8447) | 8 | — | — |
| `[ASM]` MASM 6.13 / 6.14 | 41 / 6 | 43 / — | — |
| VS2002 XP SP1 DDK (9178) C / C++ | 15 / 25 | 18 | — |
| VS2003 (3077) imp / C++ | 4 / 1 | 3 / — | 95 C / 21 C++ |
| Import0 (imported functions) | 390 | 195 | 62 |

Conclusions: the client's own code is ≈230 translation units (199 + 13 SP5 + 18 PP), the server's ≈77 (55 SP5 + 22 SP6); the linker is link 6.00.8447.
The 41 asm objects are the asm part of the static CRT (`memcpy`, `strlen`, `_ftol`); the "VS2002 XP SP1 DDK" objects are, as a hypothesis, the MSLU static library (`unicows.lib`) or `d3dx.lib`.
What exactly the 18 Processor Pack objects build is not established (hypothesis: low-level math/skinning; yet there are no SSE intrinsics in the game code, see §3.3).

### 3.2 Statically linked libraries

| Library | Evidence | Consequence for the port |
|---|---|---|
| VC6 CRT (static, multithreaded) | no imports from `MSVCRT.dll` in either the client or the server; strings `Microsoft Visual C++ Runtime Library`, `R6xxx`; `rand`/`srand` with a per-thread seed via `_getptd` (§7.3) | replaced by a modern CRT/`std`; the semantics of `rand()` and `_ftol` are preserved by explicit wrappers |
| STLport 4.x (namespace `_STL`) | ≈50 RTTI classes of facets and streams; ≈209 KB of byte-identical code in the server; the exact release (4.5.x / 4.6.x) cannot be determined from RTTI | `std::` instead of `_STL::`; template instances inside game objects (≈1,130 functions, 0.56–0.6 MB) are not rewritten but tagged `TEMPLATE` semi-automatically (BSim, `_STL::` RTTI, carrying over the 607 shared with the server; §3.4) |
| zlib 1.2.1 | strings `deflate 1.2.1`/`inflate 1.2.1` in `.rdata` at 0x494338 / 0x495548 | current zlib; the stream format is compatible |
| D3DX7 (with PSGP) | strings `D3DX: (WARN) …`, `DisableD3DXPSGP`, the key `Software\Microsoft\Direct3D` at 0x49B6F0…; a 64 KB block 0x821000–0x831000 with 7,853 SSE/MMX/3DNow! instructions (matrix math implementations for Katmai/K6-2); hypothesis: the neighboring blocks 0x811000–0x841000 are the rest of D3DX7 (≈100–150 KB) | not needed: matrices/texture loading are written from scratch; needed only as a source of signatures |
| MSLU loader (`unicows`) | a table of 17 DLL names, the path `\Microsoft Shared\MSLU\` | removed |
| NA2dLib (IME) | classes `NA2dLib::CIMHandler/CW32IMHandler`, 16 KB in 0x7F1000–0x821000 | not carried over: text input via Bevy and winit |

The share of third-party code in the client's `.text`: the library block 0x7F1180–0x85AB59 (1,625 functions, 426 KB) + EH glue 0x85ACD2–0x88F628 (2,538 thunks and unwind funclets, 214 KB) + STL/CRT instances in the game area (≈1,130 functions, ≈599 KB) ≈ 1.2 MB, 25% of the section.
Correction: the client's main module (WinMain, window procedure, initialization) is linked *after* STLport and lies in 0x811000–0x818000, formally in the "tail", but it is game code.

### 3.3 Code generation style

| Trait | Observation (confirmed) | Consequence |
|---|---|---|
| Optimization | /O2-like: 93% of functions without a frame pointer, `leave` is rare (224 of 10,823), inline `rep movsd` 3,881 / `rep stosd` 1,312 (struct copying), 910 switch tables, functions aligned to 16 with `0x90` (7,104 cases; `0xCC` only in libraries built later) | — |
| EH and RTTI | `/GX`: 2,539 FuncInfo (23.5% of functions), 278 `try` blocks; `/GR`: ≈1,300 `__RTDynamicCast` | the analysis relies on the exact class hierarchy from RTTI |
| Floating point | pure x87: 58,467 FPU instructions in the client, comparisons `fcomp; fnstsw ax; test ah`, 0 `fcomi`, 1 `cmov`; conversion via the CRT `_ftol` | compiled for the default architecture (/GB or /G5); see §7 |
| SIMD | all 7,853 SSE/MMX/3DNow! instructions are in the D3DX7 PSGP block; in the game code ≤ 9 per 64 KB block (noise) | the Processor Pack objects do not use intrinsics (hypothesis: inline asm or just flags) |
| Calling conventions (client / server) | thiscall 53% / 46%; cdecl or no arguments 38% / 36%; stdcall 8% / 18% (the server has more Win32 callbacks); fastcall ≈ 0 | methods are `__thiscall`, free functions `__cdecl`, Win32/Granny/Miles callbacks `__stdcall` |
| Symbols | no PDB and no debug directory; there are 417 RTTI class names, ≈215 `Class::method` in log strings, 2,611 `.tga` names, Granny/Miles exports with `@N` | the symbol map is built automatically (§6) |

### 3.4 Consequences for the project

* The game is not rebuilt from recovered sources, so VC6, the Processor Pack and byte-matching tools are not needed.
* ≈2,750 library functions (1,625 in the library block and ≈1,130 STL/CRT instances in the game code) are not analyzed: in Ghidra they are marked as library functions by signatures and by `_STL::` names.
* Strings in the original are UTF-16 (VC6 `wchar_t` = 16 bits): this matters only to the converter of `global.res` and the templates.
* The original's C++ exceptions and SEH filter are not carried over: in Rust, errors are handled with `Result` types, and crashes by the standard panic mechanism with a report.

## 4. The client: code size and structure

### 4.1 Functions and the normalized base

| Metric | `Sacred.exe` (GOG client) | `GameServer.exe` |
|---|---|---|
| Instructions (capstone) | **1,462,718** | 434,617 |
| Direct `call` targets / number of calls | 6,202 / 97,081 | 2,732 / 20,527 |
| Virtual functions not called directly | 1,918 | 1,423 |
| Function starts (union of call targets, prologues, vtables, EH thunks) | **10,823** | **5,088** |
| Bytes in functions | 4,719,721 | 1,205,221 |
| Mean / median size, B | 436 / 115 | 237 / 61.5 |
| Functions > 4 KB | 150 | 27 |
| Largest functions | 0x67A7A0 (43,788 B), 0x52E590 (35,604), 0x7DC260 (33,069), 0x571710 (30,644), 0x5EAC70 (30,507) | 0x4C3F20 (36,064), 0x4D7290 (22,617), 0x4BB290 (15,043) |
| `switch` tables | 910 | 382 |
| Indirect `[reg+disp]` calls (virtual/COM) | 10,292 | 4,982 |

Hypothesis: the giant functions (25–44 KB) are the NetScript interpreter (`cInterpretSQW`), network packet handlers and balancing (huge `switch`es); when decompiling, they will have to be split by case branches manually.

Normalization of the base (confirmed):

| Set | Functions | Bytes | Comment |
|---|---|---|---|
| Starts in the game area 0x401000–0x7F1000 ("own") | **6,660** | 4,079,784 | including ≈1,130 STLport/CRT instances (≈599 KB) inside game translation units |
| Library block 0x7F1180–0x85AB59 | 1,625 | 425,877 | STLport 654, CRT 321, D3DX 183, IME 62 (+ ≈400 tagged as game code; hypothesis: an artifact of label propagation) |
| EH glue 0x85ACD2–0x88F628 | 2,538 | 214,060 | `mov eax, FuncInfo; jmp __CxxFrameHandler` + unwind funclets; no need to write them |
| Ghidra sacred-bild (`functions_eng.tsv`) | 7,631 | — | 7,409 `FUN_` + 197 named + 25 thunks; 6,048 in the game area |
| Ours only, game area | 1,017 | 509,266 | reachable only via vtables (637 of the 1,270 particle functions); Ghidra did not detect them |
| Ghidra only | 405 | — | lie inside our functions (merged neighbors) |
| **Combined base of own starts** | **≈7,050** | — | minus ≈1,130 STL/CRT ⇒ **≈5,900 game functions, 3.48 MB** |

Conversion to lines: at 19 B per logical line (calibrated on Box2D v2.4.1, `clang -O2`, range 14–24 B for VC6), **≈180k logical lines of C++** (≈150–260k at the range bounds, ≈2× in physical lines).
Server: ≈3,350 own functions / ≈0.8 MB.

### 4.2 Modules

Labels come from the vtable owner class and from string references, propagated to neighbors within ≤ 24 KB (the objects of one `.cpp` lie contiguously; the labeling is done by `pe_deep.py`); confirmed for direct labels, a hypothesis for propagated ones.

| Module (game area) | Functions | Bytes | > 256 B | Byte-identical to the server | What is inside |
|---|---|---|---|---|---|
| game_sim | **2,343** | **1,814 KB** | 1,039 | 572 fn / 127 KB | `cObject*`, `cCreature*`, `cItem*`, inventory, `cWorld`, `cSector`, `TypeManager`, `cObjectManager`, networking (`cGCclass`/TinCat wrappers), and also `cMSS` (≥7 fn / 73 KB) and `cGranny*` (≥33 fn / 35 KB) |
| "large unlabeled" | 304 | 528 KB | 197 | 98 / 18 KB | blocks 0x491000, 0x4F1000, 0x561000–0x571000; hypothesis: world/sectors/AI/pathfinding |
| UI | 987 | 572 KB | 501 | 54 / 7.5 KB | 98 `cUI_*` + 9 `cCommand*` (355 virtual functions / 209 KB; ≈2/3 of the code is non-virtual handlers) |
| particles | 1,270 | 345 KB | 314 | 260 / 45 KB | 178 `cParticleSystem_*`, 7 virtual slots each, bodies ≈200 B |
| events | 257 | 85 KB | 67 | 83 / 11 KB | `cEvent*`, `cKernel` |
| render | 134 | 56 KB | 56 | 12 / 2 KB | `dxDriver7`, `cDxDevices`, `cDX7LightMgr`, `cTexture*`, `cFont*` |
| core | 110 | 42 KB | 39 | 46 / 9 KB | `cEngine`, `cKernel`, `cResource*`, `cStats*` |
| fileio | 93 | 26 KB | 21 | 44 / 5.5 KB | `cFile_*` |
| script | 29 | 12 KB | 10 | 5 | `cScriptCompiler`, `cInterpretSQW`, `TypeManager` (a lower bound: the interpreter lies in game_sim/unlabeled) |
| STL/CRT instances | 1,130 | 599 KB | 481 | 607 / 143 KB | replaced by `std::` |
| **Total own** | **6,660** | **4,080 KB** | 2,726 | 1,783 / 370 KB | |

Address layout (a hypothesis based on the labels): 0x401000–0x641000: simulation, events, file layer, script (`cInterpretSQW` around 0x431000), core; 0x641000–0x651000: rendering; 0x6A1000–0x761000: UI; 0x761000–0x7D1000: particles; 0x7D1000–0x7F1000: networking/end of the game code; 0x7F1000–0x861000: STLport, IME, D3DX7 (0x811000–0x841000), CRT (0x851000); 0x811000–0x818000: the main module (WinMain).

### 4.3 Classes

| Indicator | Client | Server |
|---|---|---|
| TypeDescriptor | **417** | 259 |
| CompleteObjectLocator / vtable | **410** / 410 | 253 / 253 |
| Total vtable slots / distinct virtual functions | 3,224 / 2,027 | 1,856 / 1,463 |
| Inheritance depth 0 / 1 / 2 / 3 / 4 | 33 / 266 / 36 / 64 / 11 | 11 / 196 / 17 / 18 / 11 |
| Multiple inheritance | only the 14 STLport facets | the same 14 |
| Hierarchy roots (number of descendants) | `cParticleSystem` 178, `StringInterface` 62, `ParamCommand` 36, `cEvent` 20, `cTimerListener` 19, `cCommand` 9, `cFile` 5, `cFont` 4, `cPath` 3, `cKernelListener` 3, `cObserver` 3 | `cParticleSystem` 178, `cTimerListener` 19, `cEvent` 9, `cFile` 4, `ITinCatCallback` 3 |

Exactly one vtable per class: there is no virtual or multiple inheritance in the game code (confirmed).
178 particle classes: slot 0 is one shared body (30 B), slots 1/2/4 have their own bodies in 70–90% of the classes (median ≈200–230 B; this is the effect logic itself), slots 5/6 are shared among dozens of classes; the structure is generated from a table, the bodies are ≈5–6k lines of almost boilerplate code.

### 4.4 Exceptions and `dynamic_cast`

| | Client | Server |
|---|---|---|
| FuncInfo (magic 0x19930520) | **2,539** | 896 |
| EH thunks | 2,538 | 895 |
| Functions with `try` / `try` blocks | 276 / 278 | 134 / 136 |
| Unwind states (Σ maxState) | 20,321 | 6,026 |
| Accesses to `fs:[0]` | 8,864 | 3,405 |
| `__RTDynamicCast` calls | **≈1,300** | 237 |

Practical value: the unwind maps give the order of local objects with destructors in each function and, via member destructors, the order of class fields; these are type hints that an ordinary decompilation lacks.
The heavy use of `dynamic_cast` means that reading the code requires the exact class hierarchy (§5.3).

### 4.5 Imports and call sites

Client: 389 imports from 16 DLLs (41 `jmp [IAT]` thunks); 31 imports are loaded into a register and never called directly (`tools/analysis/pe/pe_deep.py`).

| DLL | Functions | Calls | Purpose | In the new engine |
|---|---|---|---|---|
| KERNEL32 | 123 | 657 | files, `CreateFileMappingA`/`MapViewOfFile` (mmap PAK), threads, mutexes (`ReleaseMutex` 64, `WaitForSingleObject` 55), `VirtualProtect` 72 (§7.5) | std, Bevy AssetServer |
| USER32 | ≈60 | 190 | window, message loop, `GetAsyncKeyState` 39, `ChangeDisplaySettingsA` 2, dialogs of the built-in UI editor, the `tabblocker` hook | Bevy (winit) |
| GDI32 | 19 | 94 | rasterizing TTF into 256² pages (`CreateFontIndirectA`, `TextOutA` 11, `SetTextColor` 15, `BitBlt`) | cosmic-text via Bevy |
| granny.dll | 54 | 424 | Granny 1.2b: 51 are called directly, the rest via pointers; per `granny_args.py`, the most frequent are `GrannyGetAnimationParameter` (175), `SetAnimationParameter` (72), `GetBoneState` (61), `LockSequenceForRendering` (25) | models and animations are converted to glTF; Bevy animation graph |
| mss32.dll | 62 | 237 | 57 of 62 functions: streams (`AIL_set_stream_volume_pan` 27, `AIL_pause_stream` 24), 3D samples (`AIL_set_3D_position` 15) | Kira + Symphonia |
| libxml2.dll | 12 | 154 | `xmlGetProp` 86, `xmlSetProp` 30: `guiconfig.xml` | not needed (new UI, settings in TOML) |
| IMM32 | 10 | 70 | IME (`ImmGetCompositionStringW` 21) | IME via Bevy and winit |
| tincat2.dll | 4 | 31 | `TinCat_GetBuildNumber` 19, `TinCat_Scramble` 8; the rest of the API is a virtual interface | not needed: no networking |
| WS2_32 | 14 | 21 | UDP: `recvfrom`, `select` (LAN announce/ping 2005); `connect/send` only in the server | not needed: no networking |
| ijl15.dll | 4 | 12 | JPEG | the `image` crate (converter) |
| OLEAUT32 / ole32 | 5 / 4 | 13 / 5 | Excel OLE automation (the balancing tool, 0x5E6550 ≈ 16 KB: reads sheets and writes `defaults.h`/`xls_*.h`) and COM for `amstream` | not needed |
| ADVAPI32 | 6 | 9 | registry: `Software\Microsoft\Direct3D` (PSGP), MSLU, writing a DirectPlay8 key without importing `dpnet.dll` | settings in TOML |
| DDRAW | 3 | 7 | `DirectDrawCreateEx` → QI `IID_IDirect3D7`, `IID_IDirect3DTnLHalDevice` | Bevy and wgpu (Vulkan) |
| WINMM / VERSION | 3 / 3 | 5 / 5 | `timeGetTime`, `timeBeginPeriod`, file versions | `std::time`, Bevy time |

Server: 186 functions from 10 DLLs; kernel32 366 calls, user32 274 (the server has a Win32 dialog UI: `DialogBoxParamA`, `AnimateWindow`, `Shell_NotifyIcon`), ws2_32 26 (`connect` and `send` are present), tincat2 20 (`TinCat_GetBuildNumber` 16, `TinCat_CreateAPI` 2, `TinCat_CreateTinCatValues` 2); `VirtualProtect`: 0.

Video: function 0x6A0D80 creates `CLSID_AMMultiMediaStream` → `IAMMultiMediaStream::Initialize/AddMediaStream(MSPID_PrimaryVideo, PrimaryAudio)/OpenFile`; frames are read via `IDirectDrawMediaStream` into a DirectDraw surface (callers 0x6A1590, 0x6A1780); 16 `.wmv` names in `.data` at 0x5DFDCC…; Microsoft has declared `amstream` deprecated → in the new engine the cutscenes are converted to AV1 via FFmpeg.
The `COMPAT_VIDEO` key from `Settings.cfg` (the string at VA 0x9E0074, referenced from 0x6A0F06, 0x6A0F1E, 0x6A0F24, i.e. only inside this function; confirmed on file): according to a static reading (a **medium-confidence hypothesis**), the value is taken from the configuration object 0x182EE88; an empty value keeps the original path (a 1024×512 DirectDraw surface for `IDirectDrawMediaStream`), a non-empty one (GOG sets 1 by default) enables an alternative presentation with `GetWindowRect` and D3D7 render flags via `setRenderFlag` (0x643430): a compatibility mode for showing cutscenes on modern drivers (see [06-platform-backends.md](06-platform-backends.md), §5, §7).
Dynamically loaded: `riched20.dll` (console/editor), `amstream.dll`, `snmpapi.dll` + `inetmib1.dll` (hypothesis: the MAC address as a hardware ID for the lobby), `unicows.dll`.

### 4.6 Resources

`.rsrc` 5.3 MB (`tools/analysis/pe/rsrc_dump.py`):

| Type / ID | Size | Contents | Status |
|---|---|---|---|
| `BINARY` #107 (lang 1031 de-DE) | 2,760,574 B | **an encoded copy of `scripts/us/global.res`** embedded in the executable (despite the language tag 1031), loaded by function 0x80E680 right after `.\SCRIPTS\%s\global.res`; the port reads the file `scripts/<lang>/global.res` and does not use the resource | confirmed |
| `RT_BITMAP` 124–126 | 787 KB × 3 | 1024×768, 8 bpp | hypothesis: startup/loading screens |
| `RT_BITMAP` 102, 103, 112, 113 | 85 KB × 4 | 813×35, 24 bpp | hypothesis: progress bars |
| `RT_DIALOG` 105, 106 | 4,498 / 510 B | "Attribute Editor" (96 controls: name/type, position, UV, color, tooltips, 29 flags) and "Frame Texture Settings": **the built-in `cUI_Control2` property editor**, matching the set of `cUI_*::Command*` in RTTI | confirmed |
| `RT_ICON` ×3, `RT_GROUP_ICON`, `RT_VERSION` | — | icons 64/32/16, version "Sacred 2.0" | confirmed |

There is no `RT_STRING`, no `RT_MENU`, no manifest (on Windows 10/11 the process falls under DPI virtualization) and **no resolution selection dialog**: the exe has no strings of the form `NNNNxNNN`, `cDxDevices::dxGetDeviceModes()` only enumerates the DirectDraw modes, `cDxDevices::findMode(1024, 768, …)` is called once, and 17 places in the code assume the screen center 512/384 (according to sacred-bild): the resolution is hard-coded to 1024×768, and there is nothing to migrate; the question is closed (see [docs/05-renderer-vulkan-4k.md](05-renderer-vulkan-4k.md)).

## 5. GameServer as a Rosetta stone

### 5.1 Shared code

Functions were matched by the md5 of their normalized bytes (with the `rel32` of `call/jmp` and all 32-bit operands that point into the image zeroed out); confirmed.

| Threshold | Server functions ≥ threshold | Matched | Server bytes matched |
|---|---|---|---|
| ≥ 8 B | 5,199 | 3,860 | 39.3% |
| ≥ 32 B | 3,769 | 2,533 | 39.6% |
| ≥ 128 B | 2,036 | 1,372 | 37.8% |

The canonical cut (≥ 32 B): **1,176 game functions / 227 KB** (game_sim 572 / 127 KB, particles 260 / 45 KB, events 83, unlabeled 98, UI 54, core 46, files 44, render 12, script 5) + 607 STL instances (143 KB) in the game area, plus 1,023 matches in the library tail; 2,806 / 592 KB in total.
By bytes, ≈30% of the server's game simulation matches the client literally; the remaining 70% is built from the same source with different inlining, constants or server-side branches (`cGSclass` instead of `cGCclass`, SP6 objects).

### 5.2 Client / server classes

| Group | Classes | Examples | Where |
|---|---|---|---|
| Shared | **258** | all 178 `cParticleSystem_*`; `cObject/cObject3D/cObjectFX/cObjectFRZ`; `cCreature/cCreatureHero`; `cItemBase/cItemUpgrade`; `cWeapon3D/cArmor3D/cArrow3D/cChest3D/cBsItem3D/cPotion3D/cRing3D/cScroll3D/cShield3D/cSpecial3D`; `cEvent` + 8 descendants; `cFile/cFile_disk/cFile_memoryMapped/cFile_ram`; `cTimerListener`, `cCalendar`, `cSector`, `cTinCat`, `ITinCatCallback`; ≈50 STLport classes | both |
| Server only | **1** | `cGSclass` (the counterpart of the client's `cGCclass`, 14 virtual slots, base `ITinCatCallback`) | server |
| Client only | **159** | UI: 98 `cUI_*` (incl. 36 `cUI_*::Command*` based on `ParamCommand`) + 9 `cCommand*`/`cMacroCommand`; fonts `cFont/cFontTTF/cFontTTF2/cFsFont/cUI_Ttf2`; `cDxDevice`, `dxDriver7`, `crTexture/iResource`, `cTextureMgr2`; IME `NA2dLib::*`, `cEventIME`; `cEngine`, `cKernelListener`, `cObserver/cSubject`, `cWorld/cWorldView/cWorldView0`, `cLogWindow`, `cResourceMgr`, `cScriptResource(Dlg)`; `cFile_async`, `cFile_cached`; **pathfinding `cPath/cPathAstar/cPathFloodfill`**; input/UI/console/network events; `StringInterface` (62 descendants); STLport streams and locales | client |

The server's `Class::method` log strings (21 classes / 50 methods: `cObjectManager` 11, `cKernel` 5, `TypeManager` 4, `cCreature` 4, `cInventoryEntry` 4, `cParticleSystemManager` 4 …) are all present in the client as well.

### 5.3 Hierarchy of the key classes (abridged)

Skeleton of the RTTI hierarchies (`pe_deep.py`, `class_compare.py`; the number is the count of virtual slots; "C" means client only):

```text
cTimerListener (3)
└── cObject (10) ── cObjectFX (10), cObjectFRZ (10)
    └── cObject3D (12)
        ├── cCreature (12)              [cCreatureHero (1) — a separate class with no base]
        └── cItemBase (18)
            └── cWeapon3D, cArmor3D, cArrow3D, cChest3D, cBsItem3D, cPotion3D,
                cRing3D, cScroll3D, cShield3D, cSpecial3D, cItemUpgrade (18)
cEvent (6) ── cEvent_creature, cEvent_engine, cEvent_key, cEvent_object, cEvent_trigger,
              cEventFX, cEventUI2 → cEventUI_string (7), cEventWeather
              C: cEvent_idle, cEvent_mouseMove, cEvent_mouseButton → _down/_up,
                 cEventArrow, cEventConsole, cEventIME, cEventNetwork, cEventProfile
cFile (8) ── cFile_disk, cFile_memoryMapped, cFile_ram; C: cFile_cached (9); cFile_async (3, no base)
cParticleSystem (7) ── 178 descendants cParticleSystem_<effect> (7)
ITinCatCallback (14) ── cTinCat (14); cGCclass (14, C) / cGSclass (14, server)
C: cKernelListener (1) ── cEngine (2), cUI_Manager (3)
C: StringInterface (4) ── cUI_Control2 (12) ── cUI_Button2, cUI_EditControl → cUI_DigitEdit,
      cUI_Frame, cUI_Listbox(2), cUI_Combobox, cUI_Scrollbar, cUI_Static2, cUI_StaticText64(FX),
      cUI_TabControl, cUI_TabSheet, cUI_RadioButton2
      └── cUI_Window2 (17) ── ≈60 windows: cUI_Character (18), cUI_Inventory3, cUI_Trade (20),
          cUI_StatsTab (21), cUI_NetLobby, cUI_Options (18), cUI_Megamap, cUI_Taskbar2 …
C: ParamCommand (abstract) ── 36 cUI_*::Command<Property>
C: cCommand (2) ── cCommand_Event, _MultiEvent, _exitGame, _onInvOpen/Close, _armaPlay/Stop; cMacroCommand (9)
C: cFont (15) ── cFontTTF (18), cFontTTF2 (18), cFsFont (15)
C: cPath (2) ── cPathAstar, cPathFloodfill
C: cSubject (4) ── cWorld;  cObserver (2) ── cWorldView (8) ── cWorldView0
C: iResource (9) ── crTexture;  cResourceMgr (1) ── cTextureMgr2;  dxDriver7 (1); cDxDevice (1)
C: NA2dLib::CIMHandler (9) ── CW32IMHandler (14)
```

Known object sizes from the debug logs of early builds (according to openheilig, not checked on the file): `cSectorChunk` 512, `cTrigger` 16, `cPatchIso` 32, `cGrnMtnChunk` 256, `cSoundChunk` 128, `cTimerListener` 12, `cSectorEnvironment` 256; they are unchanged across all builds; during decompilation they are cross-checked against the `operator new` argument before the constructor.

### 5.4 What the server provides and what it does not

* **Provides:** an image of the same source, four times smaller, with full RTTI: a convenient first entry point for `RecoverClassesFromRTTIScript`, OOAnalyzer/Kaiju and for building signatures; a reference for `cObject/cCreature/cItem*/cEvent*/cFile*/cWorld/cSector/TypeManager/cObjectManager/cKernel`, the item and name generators (§7.3), the network protocol (`cGSclass`, `NW_*`/`OBJECT_*` handlers), QuestDB and `TRIGGER_*` relaying.
  Names are carried over to the client via Ghidra Version Tracking / BSim using the 1,176 byte-identical functions and thousands of "similar" ones.
* **Does not provide:** the NetScript interpreter (the `%s\FunkCode.bin` loader and all handlers exist only in `Sacred.exe`: the host client executes the scripts, the server stores QuestDB; confirmed from strings), rendering, UI, sound, Granny animation, pathfinding, IME, the main loop.
  For all of these the only source is the `Sacred.exe` client.
* The server build is partly made of SP6 objects, so some shared functions differ in code generation rather than in logic; when carrying annotations over, this is normal noise.

## 6. Map of subsystems from log strings

Strings of the form `Class::method` in `.rdata/.data` are a free partial symbol map: each one ties a function in the client's `.text` (the GOG client) to a method name (`fn_context.py --str "cEngine::renderThread"`).
Client: 74 classes / 215 methods (2 junk entries); `tools/analysis/pe/sacred_strings.py` writes the full list.
A sample:

| Subsystem | Classes (methods) | Characteristic methods | What it tells us |
|---|---|---|---|
| D3D7 rendering | `dxDriver7` (21), `dxDisplay` (3), `cDxDevices` (1), `cDX7LightMgr` (1), `cDxDriver7` (1), `dx3DDriver/dxDriver` (1) | `init` (15 messages), `createDirectDraw`, `createD3DDevice`, `createZBuffer`, `enumZBufferFormatsCallback`, `setDirectDrawMode_id`, `flip`, `lockBack/lockOff`, `waitForVerticalRetrace`, `dxGetDeviceModes` | the whole device contract is one class of ≈10 KB; modes are enumerated but not selected; hardware lighting (`hwTnL`) |
| Engine and threads | `cEngine` (14), `cKernel` (5), `cUI_ManagerThread` (1) | `simThread`, `renderThread`, `receive_event`, `initGame`, `load/save`, `setFadeMode`; `cKernel::update/flushEvents/registerListener` | 4 engine threads (sim, render, UI menu, async reader) + Miles/TinCat threads; the `cKernel` event bus |
| Objects and world | `cObjectManager` (15), `cWorld` (5), `cSector` (3), `cWorldView2` (1), `cObjectFactory` (1) | `allocate/create/destroy`, `GetFreeSlot`, `loadHero/terminateHero`, `precalcSectorLookup`, `remapTrigger_load`, `getHeightChunk/getHeightInfo`, `showWorld` | an object pool with slots; sector heights; the 100×100 grid (docs/02) |
| Creatures and items | `cCreature` (7), `cCreatureHero` (3), `cInventoryEntry` (6), `cInventory` (2), `cItemBase/cItem/cItemUpgrade/cChest3D/cWeapon3D` | `equipment_equip/reset`, `inventory_putItem`, `pickupItem`, `CalcResults`, `gethitvalu`, `autoSortItems`, `fixInventory`, `toggleVisuals` (17 messages) | combat formulas and balance are concentrated here; shared with the server |
| Types and data | `TypeManager` (6), `ItemDataMgr` (2), `ItemTypeMgr` (2), `cTextureLoader` (4), `cTextureManager` (1), `cGrannyModelManager` (12) | `loadItemTypes/loadWeaponInfo/loadSoundProfiles`, `getRandomItem(Fam, Lvl, mf, wc …)`, `saveHero` (11), `generateLookup/loadLookup`, `pakReadHeader`, `createFromPak` | loaders of PAKs and `.tmp` caches; the item generator |
| Animation | `cGranny` (12), `cObject3D` (1) | `render/renderComplex/renderShadow/renderShadowFake/renderTransparent`, `bindTexture(s)`, `getControlForSequence`, `grnSequenceLoad` | CPU skinning twice per frame (model + shadow) |
| Scripts | `cScriptCompiler` (3), `cInterpretSQW` (8) | `parseStatement`, `parseResource`, `loadScriptedSequenceR`; `enterSector/exitSector/enterRegion/exitRegion/initRegion/Teleporter` | the `.txt → .bin` compiler is built into the client (keyword tables in `.data` at file offset 0x54ce78), and the interpreter is in the client too |
| Sound | `cMSS` (7) | `Check3DProvider`, `playMusic`, `playSFX` (4), `getSoundChunk`, `advanceTime` | one class of ≈73 KB on top of Miles |
| UI | `cUI_Manager` (8), `cUI_Character` (3), `cUI_Trade`, `cUI_Blacksmith`, `cUI_NetLan`, `cUI_TabControl`, `cUI_Taskbar2`, `cUI_Ttf2` … | `runThread/endThread`, `handleEsc`, `tooltipCreate`, `executeAction` (5), `tradeFinal/tradeUndoInventory`, `slotChangeRef` | the UI lives in a separate menu thread; fonts via GDI |
| Files and resources | `cFile_cached` (1), `cFile_memoryMapped` (1), `cResourceMgr` (1), `CABBCache` (1) | `async_read`, `open`, `disposeAll` | a `cFile` hierarchy with mmap and asynchronous reading |
| Networking | `NetworkManager` (1), `cObject` (`createPacket`) | `receive_event`, `createPacket () ref/type/bakRef/bakType` | a packet factory by ref/type; ≥107 packet names from error strings (enumeration estimate 110–140) |

Additional sources of names (PLAN.md, ADR-10): sacred-bild's `functions_eng.tsv` (7,631 functions, ≈145 meaningful names: `dxDriver7_*`, `cEngine_renderThreadRun/simThreadRun`, `cWorldView*`, `cTileRenderer_*`, `cFontTTF2_*`), `sacred_addr.h` (≈40 structs and globals, e.g. `g_pDxDriver`), sacred-sdk addresses: the NetScript operand reader `FUN_00472BC0`, the record walker `FUN_00475680` (123 cases), section start `FUN_0046BA90`, the `global.res` loader `FUN_0080E680` (an earlier attribution "`FUN_00472BC0` is the interpreter" is obsolete; [docs/re/netscript.md](re/netscript.md)); `FUN_0046F9B0`, once taken for a "loader": its role is not confirmed.
Rule: every imported name carries a provenance tag (`src=rtti|str|xref|bsim|tsv|llm`); names from sacred-bild are facts under GPL-3 usable only as names, not as code.

## 7. Behavior details useful for the reimplementation

Source: static analysis with `tools/analysis/pe/fpu_scan.py`, `detail_scan.py`, `srand_scan.py` and `control87_callers.py`.

| Topic | Fact | What it means for the new engine |
|---|---|---|
| Threads | simulation in `cEngine::simThread` (0x60f060), rendering in `cEngine::renderThread` (0x60e350), UI and input on the main thread | our own Bevy system schedule; the rules are single-threaded |
| x87 precision | the main thread runs at 24-bit precision (`_controlfp(_PC_24)`), the simulation and render threads at 53-bit; in the original the same function computes differently | bit-exact results are not needed; `f32`/`f64` |
| Rounding | `_ftol` truncates toward zero, 2,816 calls in the game code | when porting formulas, use `trunc`, not rounding |
| Random numbers | MSVC CRT `rand()` is the LCG `seed·214013 + 2531011` with the result `(seed >> 16) & 0x7FFF`; 1,828 calls in 228 functions; deterministic seeding in the name generator and for creatures | a modern generator can be used; the LCG can be reproduced if the same "character" of the distributions is needed |
| Positions | `u16 map, i32 x, i32 y, u8 z` in the object | an integer world grid is convenient for the new engine too |
| Hero data protection | 72 `VirtualProtect` calls guard one page holding an encrypted copy of the stats | not carried over |

## 8. What this means for the project

### 8.1 What to analyze

| Client module | Functions / size | What we do |
|---|---|---|
| Game logic (`game_sim`) | 2,343 / 1,814 KB | **analysis**: stats, skills, combat, items, trading, AI; the rules are carried over into `opensacred-rules` |
| Large unlabeled functions (world, AI) | 304 / 528 KB | **selective analysis** |
| NetScript interpreter and quests | ≈29 / 12 KB under the client label + handlers in `game_sim` | **analysis**: semantics of the 113 record types in use |
| Events (`cEvent*`) | 257 / 85 KB | **selective analysis**: which events the rules generate |
| Particles (178 classes) | 1,270 / 345 KB | **data**: effect parameters are extracted semi-automatically, the code is not carried over |
| UI | 987 / 572 KB | **not analyzed**: a new interface; only the formulas the UI displays are taken from the code |
| Rendering, D3D7, fonts | 134 / 56 KB | **not analyzed**: the facts are already collected ([docs/05-renderer-vulkan-4k.md](05-renderer-vulkan-4k.md)) |
| Sound, video, networking, files, IME | — | **not analyzed**: replaced or out of scope |
| Libraries (CRT, STLport, zlib, D3DX7) | ≈2,750 | **not analyzed** |

In total, on the order of 2,000–3,000 functions need analysis.
At a pace of 8–15 functions per agent-hour (estimate), that is 130–375 agent-hours, which are part of milestones M3 and M4 ([docs/07-roadmap-estimates.md](07-roadmap-estimates.md)).

### 8.2 How to analyze

1. A Ghidra 12.1.4 project with the GOG `Sacred.exe` and `GameServer.exe`; RTTI, class recovery, community names (sacred-bild only locally, sacred-sdk), library tagging ([docs/03-toolchain.md](03-toolchain.md), §4).
2. Claude Code works with the decompilation through an MCP server and writes per-subsystem notes into `docs/re/`.
3. For formula functions: the behavior is described in prose in `docs/re/`, implemented independently in Rust and verified against vectors from the user's own executable in the Unicorn emulator ([docs/04-language-and-engine.md](04-language-and-engine.md), §2).
4. `GameServer.exe` helps where functions are shared: boundaries, types, class names.

## Open questions

1. Which values the deterministically seeded functions 0x425ec0 (name generator) and 0x549920 (creatures) generate; decompilation is needed to reimplement the generation.
2. Whether the 77 KB of `game_sim` and 21 KB of `events` in the library block 0x7F1180–0x85AB59 are game code or a labeling artifact.
3. The semantics of the `COMPAT_VIDEO` key (a medium-confidence hypothesis, §4.5): unimportant for the new engine, of interest only for running the original.
4. The exact STLport release (4.5.3 or 4.6.x): needed only for automatic tagging of library functions.

## Sources

Scripts ([tools/README.md](../tools/README.md); run with `python3 -I` on the user's own installation):

* `tools/triage/pe_triage.py` — headers, sections with entropy, imports and exports, version (§1–2).
* `tools/analysis/pe/pe_deep.py` — functions, calling conventions, import call sites, RTTI, vtables, EH, subsystem labels (§3–4).
* `tools/analysis/pe/class_compare.py` — classes shared by the client and the server (§5.2–5.3).
* `tools/analysis/pe/fn_context.py`, `detail_scan.py`, `sacred_strings.py` — function context, targeted code reports, log strings and the `Class::method` map (§4.5, §6).
* `tools/analysis/pe/granny_args.py` — Granny call sites and arguments (§1, §4.5).
* `tools/analysis/pe/rsrc_dump.py` — the resource section (§4.6).
* `tools/analysis/pe/fpu_scan.py`, `srand_scan.py`, `control87_callers.py` — FPU control, `_ftol`, RNG, `VirtualProtect` (§7).
* `tools/analysis/scripts/kwtab.py` — the keyword tables of the script compiler in `.data` (§6).

The data formats referenced by §4–6 are in [docs/02-data-formats.md](02-data-formats.md).

External primary sources (checked on 2026-10-15):

* The Rich header comp.id table — https://raw.githubusercontent.com/dishather/richprint/master/comp_id.txt
* Wine `include/ddraw.h`, `dlls/ddraw/ddraw.c`, `dlls/ddraw/device.c` (DDSCL_* values, FPUSETUP/FPUPRESERVE behavior) — https://github.com/wine-mirror/wine
* Microsoft: `IDirectDraw7::SetCooperativeLevel`, `_control87/_controlfp`, Memory Protection Constants, `IAMMultiMediaStream` (deprecated) — the MicrosoftDocs repositories `sdk-api`, `cpp-docs`, `win32`; https://learn.microsoft.com/en-us/previous-versions/windows/desktop/api/amstream/nn-amstream-iammultimediastream
* ReactOS `ntoskrnl/ke/i386/thrdini.c` (CW 0x27F of a new thread) — https://github.com/reactos/reactos
* sacred-bild (`sacred_addr.h`, `tools/data/functions_eng.tsv`, `RE_NOTES.md`) — https://github.com/kirides/sacred-bild ; sacred-sdk — https://github.com/bssth/sacred-sdk ; openheilig research — https://github.com/openheilig/research
* reccmp — https://github.com/isledecomp/reccmp ; decomp.me compilers (`msvc6.5pp`, `msvc6.6`, wibo) — https://github.com/decompme/compilers , https://github.com/decompals/wibo
* Bytes-per-line calibration — https://github.com/erincatto/box2d (v2.4.1)
