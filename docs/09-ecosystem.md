# Game versions, community and comparable projects

Plan document 09.
Scope: which Sacred Gold builds exist and which one is the baseline; what players report about the original on Windows 10/11 and how they fix it; what the community has already built (hooks, SDKs, reimplementations, lobby servers, middleware replacements) and on what terms it can be used; where players and modders gather, including Russian-speaking ones; what about twenty comparable projects teach.
Formats are in docs/02-data-formats.md, tools in docs/03-toolchain.md, language and engine in docs/04-language-and-engine.md, timelines in docs/07-roadmap-estimates.md, the legal policy in docs/08-risks-legal.md.

Claim status.
**Verified locally**: on the files of a GOG installation of Sacred Gold 2.28 (English), the project's reference build, or on strings of its executables.
**Verified from the primary source**: the README, documentation, code or commit history of a GitHub repository was read in full.
**From secondary sources**: the site (Steam Community, GOG, VOGONS, Nexus, gogdb, press) was unreachable from the research environment and only a search-results summary is known; such facts are re-checked before anything relies on them.
**Hypothesis**: an inference of this document.

## Summary

* The project works with a single build: Sacred Gold 2.28 (GOG, English) from 2006-10-13, `Sacred.exe` 11,919,360 B, SHA-256 `db9b86a4…`, DRM-free.
  The community's addresses (sacred-bild, sacred-sdk) refer to exactly this build.
* Other builds (the German and Russian retail editions, the 2009 LGP Linux port) are listed for reference; the project does not use them.
* Players report the original broken on Windows 10/11 by the D3D7 renderer, the hard-coded 1024×768, the spin limiter, the keyboard hook and focus loss, and fix it with dgVoodoo2 and resolution patches.
  The GOG build itself runs on Windows 11 without a wrapper, in a borderless 1024×768 window (§2.2); the new engine removes the other problems entirely.
* Over 2025–2026 an ecosystem of ≥10 open projects has formed around Sacred.
  For our approach the format readers matter most: open-grn and grn2glb (Granny), openheilig (formats, bytecode), sacred-sdk (opcodes, string hash).
* Borrowing rule: facts about formats and addresses we take; MIT code only as a reference, with the notice in `THIRD-PARTY-NOTICES.md` for anything borrowed; from the GPL-3 sacred-bild only names, in a local Ghidra project that is never committed; GPL, AGPL and unlicensed code never enter the converter or the game (§3.3).
* The closest analog by goal is OpenGothic: a reimplementation of a 2002 game on the original assets, essentially by a single developer, with a modern renderer.
  Our project follows the same path, but with asset conversion, in Rust and with Claude Code.

## 1. Versions and builds

### 1.1 Executables by edition

| Edition | Build / fingerprint | Languages | Protection | Knowledge status | Role in the project |
|---|---|---|---|---|---|
| GOG "Sacred Gold" 2.28 (English) — **the project build** | `Sacred.exe` 11,919,360 B, SHA-256 `db9b86a44c8c1cc8c5ed9e3f427121ab8b4ed62760861e58995f10af9d971e74`, version 2.0.2.28, PE 0x452F85C7 = 2006-10-13 12:25:43 UTC, four sections `.text`/`.rdata`/`.data`/`.rsrc`, entry point RVA 0x44D964, `.text` entropy 6.57; `GameServer.exe` 0x452F8580; in the store: product 1207658688, build 50854356829483339, published 2018-01-10 | English / French / Russian depots of ≈2.8 GB each; the project is verified on the English one | none (no DRM) | files: verified locally (docs/01, §2); the stamp 0x452F85C7 matches the value sacred-bild gives for GOG ENG (from the primary source); product number, build number and depot contents: from secondary sources (gogdb) | the only supported build: the converter and the oracle check an installation against its manifest; the local Ghidra project, the oracle (#363) and experiments run on the developer's own copy of this exe as is; all addresses in the plan documents refer to it; the exe itself is never published |
| German retail Underworld/Gold 2.0 | PE 0x451BBE74 = 2006-09-28 12:22:12 UTC | German | ProtectDISC on the retail line (hypothesis, from a build path string in the 2.28 executable; the GOG file is not protected) | stamp: from the primary source (sacred-bild); build path: verified locally | not supported |
| Russian retail "Knyaz Tmy: Zolotoe Izdanie" ("Prince of Darkness: Gold Edition", Akella) 2.28 | a different binary: the animation selection table sits at 0x956E40 in ENG and at 0x955E48 in RUS | Russian | not established | from the primary source (openheilig/research) | not supported; matters for Russian-speaking players with the disc version |
| LGP Linux "Sacred Gold" (2009; patch 1.0.02, 2010) | stripped i386 ELF ≈9.16 MB, 615 dynamic imports; includes `sacredserver` | — | activation by an LGP key and account; LGP was liquidated on 2011-05-03 | from the primary source (openheilig) | not used (§1.3) |

Checking which build a contributor has (on their own copy; the script only reads the file):

```bash
uv run python -I - "$SACRED_DIR/Sacred.exe" <<'EOF'
import sys, datetime, hashlib, pefile
pe = pefile.PE(sys.argv[1], fast_load=True)
ts = pe.FILE_HEADER.TimeDateStamp
print(hex(ts), datetime.datetime.fromtimestamp(ts, datetime.timezone.utc))
# 0x452f85c7 2006-10-13 12:25:43 UTC -> Gold 2.28 ENG
# 0x451bbe74 2006-09-28 12:22:12 UTC -> German retail 2.0
print(hashlib.sha256(open(sys.argv[1], 'rb').read()).hexdigest())
# db9b86a4...71e74 -> GOG ENG 2.28, the project build
EOF
```

All addresses, signatures and analysis results in the project refer to GOG ENG 2.28 (SHA-256 `db9b86a4…`); the file is analyzed as is.

### 1.2 Patches: official and unofficial

| Version | Contents | Source / status |
|---|---|---|
| Sacred 1.8.x.6 + Sacred Plus | the last patch of the base game; Plus is a free add-on | from secondary sources (SacredWiki) |
| Underworld 2.20 → 2.24 → **2.28** | 2.28 is the last official one (November 2006, ≈46.7 MB): it merged the English versions and fixed the disappearing horse, Hell Golem resistances and gold overflow; version 2.0.2.28 in the registry | from secondary sources; version number: verified locally |
| 2.29 "Thorium" (unofficial, 2007) | a detour of the `global.res` loader and a fix for the busy-wait on focus loss; both fixes are ported into sacred-sdk | from the primary source (sacred-sdk, Acknowledgements); distribution via Discord: from secondary sources |
| 2.29.14 | the SacredUtils configurator (2018) installs 2.28.01 and 2.29.14 and checks the integrity of 2.28–2.29.14 files | from secondary sources (the author's Steam post) |
| "Inoffizieller Patch" (German) | the engine address table used in sacred-sdk (`refs_native_src.md`) | from the primary source (sacred-sdk) |
| PureHD v.26012023, ReBorn v4.6 (2025-08-05), SacredNL | mods that modify the exe/DLLs; see §3 | from secondary sources |

Two conclusions for the plan.
First, exactly 2.28 counts as "vanilla": there were no official builds after it, and save and network compatibility is needed only with it.
Second, the unofficial patches are distributed as binary patches without source; their contents are available to us only through sacred-sdk (MIT), which documented the addresses and ported two 2.29 fixes into code.
The patches themselves do not go into the repository.

### 1.3 The 2009 LGP Linux port

According to openheilig's build survey (from the primary source), the LGP port replaces the Windows platform layer (Miles → OpenAL + Vorbis; Bink/WMV → libavcodec/avformat; TinCat → libgrapple; DirectX → SDL 1.2 + X11 with fixed-function OpenGL), carries Itanium RTTI for 318 classes, and reads the same data: 8/8 `.bin` files are byte-identical to Windows, 380/380 `balance.bin` keys are present, the PAKs are the same.
So the formats are frozen across builds.
The project does not use the port: its addresses do not carry over to the GOG build, and all analysis runs on the GOG build.

## 2. Problems on Windows 10/11 and existing workarounds

### 2.1 Causes

Sacred is not a palettized DirectDraw game but a Direct3D 7 one (TnL HAL via `dxDriver7`, FVF 0x244/0x1C4, ~17 render states, 2 texture stages; confirmed by static analysis, see docs/05).
Therefore DirectDraw-only wrappers (cnc-ddraw) do not apply; only wrappers with a D3D7 frontend work.
The other root causes (confirmed by static analysis, see docs/01): the resolution is hard-coded at 1024×768 (the 512/384 center is assumed in 17 places in the code; `cDxDevices` only enumerates modes); the frame limiter is a spin on `Sleep(0)`; the low-level keyboard hook "tabblocker" swallows Win and Alt-Tab system-wide (in 2.28 only with the word `tabblocker` on the command line, and removed again at once, docs/re/window.md); without focus the game stops drawing; video goes through the obsolete DirectShow `IAMMultiMediaStream` into DirectDraw surfaces; the lobby `sacred<lang>.ascaron-net.com:7066` is dead; the system D3D7 runtime refuses render targets larger than 2,048 px (according to sacred-bild), a direct obstacle to 4K via ddraw.dll without a custom backend.

### 2.2 Symptoms and community-accepted workarounds

On the development machine (Windows 11 Pro build 26300, NVIDIA GeForce RTX 4080; 2026-10-18) the GOG installation runs without any wrapper: with `FULLSCREEN : 0` in `Settings.cfg` the game opens a borderless 1024×768 window and plays normally (verified locally, by running it).
The installation holds no wrapper DLL (`ddraw.dll`, `D3DImm.dll` or the like), only the DLLs of the GOG release, and the only compatibility layer the registry holds for its executables is `HIGHDPIAWARE` (verified locally).
That observation does not cover fullscreen mode, long sessions, Alt-Tab or the cutscenes; the symptoms below are what players report.

| Symptom | Cause | Workaround | Status |
|---|---|---|---|
| Hang on the loading screen | 32-bit color is required | enable 32-bit in Config.exe; sacred-bild forces 32 bpp; the GOG release has `GFX32 : 1` by default | cause: from secondary sources; GOG default: verified locally |
| Cutscene playback problems on modern drivers (the specific symptom is not established) | DirectShow `IAMMultiMediaStream` into a DirectDraw surface | the GOG release has `COMPAT_VIDEO : 1` by default, an alternative cutscene playback path; sacred-bild replaces the cutscene path entirely with Media Foundation | presence of the key and where it is read: verified locally; its meaning: medium-confidence hypothesis |
| Freezes and crashes on Windows 11 | not established | dgVoodoo2 (32-bit colour, centered scaling, max VRAM); `WAITRETRACE : 0` "noticeably reduces the number of crashes" | from secondary sources; the `WAITRETRACE` key exists in the exe (verified locally) |
| Black menu after Alt-Tab, intercepted Win keys | drawing only while focused; system-wide keyboard hook | sacred-bild: draw in the background, take input only in the foreground | from the primary source (sacred-bild Display.md) |
| High CPU load, micro-freezes | 60 fps limiter on `Sleep(0)` | sacred-bild: high-resolution timer | from the primary source |
| Seams between tiles with FSAA | FSAA filter | `FSAA_FILTER : DISABLE`, MSAA off in dgVoodoo | from secondary sources |
| Stretched cutscenes and loading screens on 16:9 | UI and video are designed for 1024×768 | the "Movies Fullscreen" mod (SimonPoe, 2026); sacred-bild: Media Foundation with the aspect ratio preserved | mod: from secondary sources; sacred-bild: from the primary source |
| Render target > 2,048 px impossible | system D3D7 runtime | sacred-bild works around it via D3D9Ex; dgVoodoo2 is not affected | from the primary source (sacred-bild) |
| Stretching with FreeSync in fullscreen mode | monitor/refresh rate | disable FreeSync | anecdotal, from secondary sources |

The typical set of `Settings.cfg` keys that players recommend (the keys exist in the exe: verified locally by strings; effectiveness: from secondary sources; in the GOG release `GFX32 : 1` is already set by default, `FSAA_FILTER` is present with the value 0, and `WAITRETRACE` is absent from its `Settings.cfg`: verified locally):

```text
GFX32         : 1
WAITRETRACE   : 0
FSAA_FILTER   : DISABLE
```

### 2.3 Wrappers and compatibility layers

| Tool | Version (Oct 2026) | License | Applicability to Sacred |
|---|---|---|---|
| dgVoodoo2 | 2.87.5 | closed source, free | DDraw/D3D6-7/D3D8-9/Glide3 frontends → D3D11/D3D12; the de facto standard among players; does not solve 1024×768 (upscaling only) or the CPU bottleneck; Windows only |
| DDrawCompat | 0.7.2 (pre-release) | not recorded | DirectDraw/D3D7 compatibility on Win10/11, windowed mode; an alternative to dgVoodoo2 for tracing |
| cnc-ddraw | 7.1.0.0 | MIT | **not applicable**: DirectDraw only, no Direct3D |
| DXVK | 3.1.1 | not recorded | D3D8/9/10/11 → Vulkan; works with Sacred only through the sacred-bild D3D9Ex backend: the chain "game → sacred-bild ddraw.dll → DXVK d3d9.dll → Vulkan" is documented by the author as working, including under Wine/Proton |
| sacred-bild | main, October 2026 | GPL-3.0 | ddraw.dll hook: any resolution, UI canvas, batching, GPU skinning, Media Foundation, gamepad, networking; the only solution that fixes causes rather than symptoms (§3) |
| Wine / Proton | Lutris script `sacred-gold-proton` (id 3688) | — | runs; the ProtonDB rating could not be obtained (from secondary sources) |

Sources: wrapper versions: GitHub release pages (verified from the primary source).

### 2.4 Resolution and 4K: what breaks in "vanilla + resolution patch"

PureHD and ReBorn HD get an arbitrary resolution by patching the exe (the keys `PHD_RES_SCX/SCY` in PureHD's `Settings.cfg`; the vanilla exe has no such keys, verified locally, so the exe is patched or a DLL is swapped in: hypothesis).
What breaks as a result (from the primary source, sacred-bild RE_NOTES, unless stated otherwise):

| Problem | Gist |
|---|---|
| UI does not scale | the higher the resolution, the smaller the interface (VOGONS: from secondary sources); sacred-bild makes a 1024×768 UI canvas scaled by height, with HUD anchors at the edges |
| Tile flicker above 1366×768 | from secondary sources (VOGONS, German Steam forum); players' advice: 1366×768 + dgVoodoo upscaling |
| Blend-layer buffer overflow (cap 0x6D5) | when zooming out on a large screen: a "staircase" without transitions |
| Ground extends past the far plane | the top ≈164 px at 2560×1440 and zoom 2: depths are designed for 768 px |
| Tile row traversal goes beyond the 3×3 loaded sectors | crash at 0x62B093 (for example, when leaving a town to the north) |
| Performance | CPU: ≈9,600 draw calls and 8,400 texture switches per frame at maximum zoom-out (the plan's canonical numbers); CPU skinning of each character twice per frame (model + shadow), 2.9–3.4 ms to pose 220–310 skeletons (according to sacred-bild) |
| Pathfinding | 64×64 tile grid: distant clicks on a large screen have to be "pulled in" (`click_walk` in sacred-bild, commits of 2026-10-08) |
| Screenshots | only the top-left 1024×768 is written |

That is why the new engine renders at native resolution with a configurable zoom, scales the UI separately, with the window's height, and rasterizes text at the window's resolution (docs/05 §3.5).

### 2.5 Multiplayer

The official lobby does not work; the community plays over LAN via VPN and through its own lobby servers.
For this project, multiplayer comes after version 1.0 (PLAN.md ADR-09).

## 3. Community projects

### 3.1 Summary table

| Project | Language / license | Activity (as of 2026-10-15) | What it provides | How we use it | Limitations |
|---|---|---|---|---|---|
| **kirides/sacred-bild** | C++ (CMake, vcpkg, Detours, ImGui, SDL3, KCP) / GPL-3.0 | 118+ commits, 35 of them on October 7–8, 2026; 7★ | ddraw.dll hook: a D3D9Ex backend for D3D7, any resolution, UI canvas, batching and atlases, GroundMesh, Granny GPU skinning, Media Foundation, gamepad, UDP/KCP + matchmaker; `RE_NOTES.md` (78 KB, GOG ENG addresses), `sacred_addr.h` (297 lines, addresses + signatures), `functions_eng.tsv` (7,631 functions: 7,409 FUN_, 197 named, 25 thunks; 25,337 entry points in total including EH funclets), class structures in `src/sacred/` | function names go into the local Ghidra project only and are never committed; RE_NOTES is a source of facts about rendering, threads and logic; no code is used | GPL-3.0: its code would not let ours stay MIT OR Apache-2.0; we do not need the Win32/D3D9 glue; a single author |
| **openheilig** (engine, research, tools) | GDScript (Godot 4.7 Forward+/Vulkan) + Python / engine and tools MIT; research CC BY-SA 4.0 | 235 commits, ≥70 in 90 days, the last on 2026-10-05; 2★ | readers for all formats (pak, keyx/wldx, ARGB4444, tiles, statics, mixed, items, creatures, GRN, pax, FunkCode/StartCode, global.res); two independent decoders (GDScript + Python) that must agree; a survey of seven builds; names for 120/141 opcodes; a GRN specification cross-checked on 4,991/4,993 records | the format descriptions are a basis of `opensacred-formats` (in our own words, credited); layouts from the MIT code carry the notice in `THIRD-PARTY-NOTICES.md`; the readers are a reference for cross-checking the converter; the build survey as a source of facts | the Godot engine does not carry over as code; the research is CC BY-SA 4.0: facts only, in our own words |
| **bssth/sacred-sdk** | C++14 / Lua 5.4 / Python / MIT | created in May 2026, 71 commits, active in September 2026 | a DLL proxy in the `ijl15.dll` slot; Lua → `FunkCode.bin` bytecode; a disassembler (125,236 records in 721 ms), a C++ (de)compiler with byte-exact round-trip on 20/20 bins; compiler keyword tables; `sacred_hash`; the `.pax` format; three PAK readers; texture decoding; keyx/wldx; hostility matrix; quests; Ghidra headless scripts; 27 RE specifications; ported 2.29 fixes; a catalog of ReBorn patches + `reborn_symbols.json` | MIT, as a reference: the string hash (with the notice in `THIRD-PARTY-NOTICES.md`), the AMH container, the opcode table, the NetScript disassembler as a reference for our converter (docs/02, docs/06) | addresses are for the 2.28 build |
| **karolak6612/open-grn** | C++20 / MIT | 6 commits, all on 2026-08-31; 1 PR with no response; 14★ | a `granny.dll` 1.2b reimplementation: container, poses, multi-track animation, ADSR blending, D3D skinning, textures, VFS; `grn_dump.py`, `grn_unpack.py`; 105 exports = 101 Granny + 4 internal `_Bink_*` | a reference on the format and the animation semantics (`control.cpp`: parameters 0–8) for an approximate mapping onto Bevy; its code is not used | described by its README as an AI-assisted proof of concept; tests without asserts against the original |
| **ancaria-dev/grn2glb** | C++ / MIT; VTex codec LGPL-2.1-or-later | 33 commits, 2026-09-06 → 2026-10-01; no releases | GRN parser (`grn_parser.cpp` ≈530 lines): 1,571 models, 1,009 skinned; VTex decoder (≈500 lines, derived from FFmpeg `bink.c`); C ABI | a reference for our Granny parser: tag tables and the reading order of the MIT core, with the notice in `THIRD-PARTY-NOTICES.md`; the LGPL VTex codec is not used (FFmpeg runs as an external program) | 1/1,571 animations make it into the GLB; deformation in pose is not verified |
| **maci0/openmiles** | Zig 0.16 / GPL-3.0 | 679 commits, 0.4.0 from 2026-09-29; 8★ | a drop-in `mss32.dll` for MSS v3–v9 (`-Dmss-version=6`), miniaudio + TinySoundFont, MP3/OGG/WAV, 3D, timers | a reference for `AIL_*` semantics when recreating the sound logic; no code is used | 0.x with no guarantees; tested on Europa 1400, not on Sacred; Zig is not part of our stack |
| **kryotek777/Sacred-Gold-LobbyServer** | C# / .NET 10 / AGPL-3.0 | March 2025 → April 2026, 180 commits; 47★ | TinCat2/CUSTOMDATA lobby, string obfuscation, OpenNet/ClosedNet (characters on the server, a separate `SacredClosedNet.exe`), WebAPI and a panel; a public instance | not used before multiplayer, after version 1.0; then facts only | AGPL-3 |
| **stefanwerfling/sacredbamboo** | TypeScript / GPL-3.0 | 24 commits, 2026 | all 56 `SacredMsgType` on the wire, ≈2/3 of the handlers, a game-server skeleton | not used before multiplayer, after version 1.0; then facts only | GPL-3.0 |
| **Aytackydln/SacredEngineRemake** | C# (.NET, Avalonia, DX12) / **no license** | 119 commits, the last on 2026-10-12; 7★ | a coverage report of 38 file patterns / 65 StructLayouts; the type table in the exe (VA 0x8EC328, 5,624 × 0x44); a managed Granny 1 parser + an x64 DLL backend; sector and minimap rendering, GPU particles, HUD; TinCat Frame/Codec/Checksum/Login, LAN admission | not used | no license: all rights reserved |
| **sonicmouse/SacredGameTools** | C++ (2005) / MIT (2022) | 3 commits | save format, layout of `TINCAT2.DLL` 2.0.24 (according to sacred-sdk) | historical cross-check of formats | an old TinCat version |
| **S22F5/GhidraExports** | XML | 2020 | `granny.exports`, a library definition for Ghidra (101 exports) | import into the local Ghidra project | — |
| **pnxr/sacred2-lobby-emulator** | C# (Mono, MySQL) | 34 commits, revived in 2025 | a Sacred 2 lobby (port 6800) built from intercepted traffic | the methodology only (Wireshark + "guessing the responses") | a different game |
| PureHD, ReBorn/ReBorn HD, SacredNL, Classic UI, Movies Fullscreen | closed mods and patches | PureHD v.26012023; ReBorn v4.6 (2025-08-05); SacredNL: a DLL (interview on DTF) | arbitrary resolutions (PureHD, ReBorn), balance and content (ReBorn), multiplayer fixes (SacredNL), the old UI, widescreen cutscenes | as a list of player expectations and regression scenarios; the authors are potential contributors | no source; contents are known from secondary sources and from the ReBorn patch catalog in sacred-sdk |

The community publishes derived material (TSV name exports, headers, specifications), not decompilations.
OpenSacred does the same: it commits only facts and its own code, never decompiler output (docs/08 §3).

### 3.2 What exactly is solved and what is not

**Granny 1.2b.**
The version is confirmed by the string `1.2b`/`10-4-2000` in the client's `GrannyOpenVersion` call at 0x4010D0 (not 2.x, as an initial triage assumed).
Three independent `.GRN` readers (openheilig GDScript, openheilig Python, open-grn) agree on 4,991/4,993 `models.pak` records, 1,571/1,571 meshes and 3,420/3,420 track counts with no discrepancies (from the primary source, sweep of 2026-09-01; docs/02, §4.2, lists the readers the same way); grn2glb additionally parses 1,571 models and 3,420 clips, but its cross-check against the others on the whole corpus has not been published; openheilig derived the interpolation for mode 2 (a corner-cutting quadratic B-spline, error 0.214° → 0.00004°) and cross-checked the materials with apitrace.
But no project implements the runtime that the game calls: all 54 Granny imports are used (`GetAnimationParameter` 175 sites, `SetAnimationParameter` 72, `GetBoneState` 61), parameters 0–7 form an ADSR envelope with period 10000.0 as an "eternal" loop, `AddControlledPose` is called with FLT_MAX, `AttachSequence` with mask 0x1FF, and IK is not used (verified by static analysis).
Therefore, for the new engine, models and animations are converted to glTF, and the blending semantics are carried over approximately into Bevy's animation graph (docs/02, §4.5).

**NetScript.**
Two independent groups have named the VM records: openheilig 120/141 from the keyword table in the Linux binary, sacred-sdk 125 tags from the 2.28 client; the tables agree in 123 of 141 positions.
The keyword table is also present in Sacred.exe itself (`.data`, file offset 0x54ce78, verified locally).
The shipped scripts use 113 of the 141 record types; 45 cover 95% of the campaign's records, 68 cover 99%.
The interpreter lives in the client, not in `GameServer.exe` (the server stores QuestDB and relays `TRIGGER_*`), and the community has not implemented it with any claim to accuracy: sacred-sdk executes quests and dialogs with its own runtime, openheilig with its own interpreter.
The converter translates the scripts into Luau, which the game runs through mlua (docs/06 §8).

**Networking.**
The TinCat-2 transport (28 B frame, CRC32 of the body), the lobby (56 message types, obfuscation) and the UDP announcement are described and implemented in three codebases (C#, TS, C++).
The game protocol on top of them is not described: ≥107 packet names are known from error strings, the real enum is estimated at 110–140 types; the project needs it only if multiplayer, after version 1.0, speaks the original's protocol (PLAN.md §13).

**Localization and saves.**
`sacred_hash` from sacred-sdk reproduces 946/947 numeric `res:N` references and 12,965/23,123 `global.res` ids from the shipped names (the rest are hashes of names from unshipped sources); the file is plain UTF-16LE.
The AMH container (0x100 + 16×12, 8 occupied slots, version 0x101B in all Gold templates and saves) is covered by sacred-sdk's MIT code `hero_save.cpp`; the semantics of the sections (especially 0xCD, 822,824 B of world state) are not.

### 3.3 License matrix for borrowing

OpenSacred's code is MIT OR Apache-2.0 (docs/08 §3).

| Source | License | Into the converter and the game | Condition |
|---|---|---|---|
| sacred-sdk, SacredGameTools, GhidraExports | MIT | yes, as references; anything borrowed carries the notice | `THIRD-PARTY-NOTICES.md` and a comment naming the source |
| openheilig engine/tools | MIT | yes: layouts, with the notice; the Python tools only as external cross-checks | GDScript does not carry over |
| openheilig research | CC BY-SA 4.0 | facts only, in our own words, credited | facts and numbers are not subject to license; wording is |
| grn2glb, core | MIT | yes: tag tables and the reading order, with the notice | `THIRD-PARTY-NOTICES.md` |
| grn2glb, VTex codec | LGPL-2.1-or-later | **no**: not used | FFmpeg decodes VTex as an external program |
| open-grn | MIT | **no by policy**: a reference on the format and the semantics; its code is not used | — |
| sacred-bild | GPL-3.0 | **no** | function names only, in a local Ghidra project, never committed |
| OpenMiles | GPL-3.0 | **no** | a reference on `AIL_*` semantics only |
| kryotek | AGPL-3.0 | **no** | not used before multiplayer, after version 1.0; then facts only |
| sacredbamboo | GPL-3.0 | **no** | not used before multiplayer, after version 1.0; then facts only |
| SacredEngineRemake, ptasev/Age-of-Mythology (GRN node names) | no license | **no** | not used |
| Unicorn (`unicorn-engine`) | GPL-2.0 | **no** | only in `opensacred-oracle`, a separate development-only crate, never linked into the converter or the game: GPL-2.0 cannot be combined with Apache-2.0 |
| Leaked or unlicensed code, builds or SDKs | — | **no** | never used |

Class and method names from the original's RTTI and log strings, as well as GOG ENG addresses, are facts of the executable; the documents and code comments cite them.
Names that a community project gave to functions (the sacred-bild export) stay in the local Ghidra project (PLAN.md §4.2; docs/08 §5).

## 4. Communities and channels

### 4.1 International

| Resource | Role | Verification |
|---|---|---|
| DarkMatters (darkmatters.org) | the English-speaking hub for Sacred 1/2; home of the Sacred 2 CM Patch and the SacredSDK thread | links from sacred-sdk/pnxr (primary source); the site is unreachable |
| SacredWiki (sacredwiki.org) | patches, guides | from secondary sources |
| Steam Community (AppID 12320) | compatibility, dgVoodoo, the SacredSDK guide (sharedfiles 3802870489), threads about 2.29 and ReBorn | from secondary sources; the link to the guide comes from the sacred-sdk README |
| GOG forum "sacred_series" | freezes on Win11, PureHD/1080p guides | from secondary sources |
| VOGONS | resolutions, tile flicker | from secondary sources |
| Nexus Mods "sacred" | ReBorn HD (mods/26), tweaks (mods/24), Classic UI (mods/12) | from secondary sources |
| kryotek's Discord (English + Italian) | lobby server players, testers | kryotek README (primary source) |
| German sacred-legends.de, sacred-game.de | not confirmed; the German press (PCGH, PC Games) covers Sacred 2 Remaster | from secondary sources |

The rights holder: Ascaron went into insolvency on 2009-04-14; the rights to Sacred passed to Deep Silver (Koch Media) and, after THQ Nordic acquired Koch in 2018 and an internal IP exchange in May 2020, to THQ Nordic GmbH; Sacred 2 Remaster was released on 2025-11-11 (all from secondary sources; the trademark notice is in docs/08 §6).

### 4.2 Russian-speaking community

| Fact | Status |
|---|---|
| The publisher in Russia is Akella: Sacred as "Knyaz Tmy" ("Prince of Darkness", 2004), Underworld as "Knyaz Tmy: Podzemelya Ankarii" ("Prince of Darkness: Dungeons of Ancaria", 2005), Gold as "Knyaz Tmy: Zolotoe Izdanie" ("Prince of Darkness: Gold Edition") | from secondary sources (ruwiki, StopGame, MobyGames) |
| The Russian retail Gold 2.28 is a separate exe build (addresses differ from ENG) | from the primary source (openheilig) |
| The GOG version contains a Russian depot (≈2.8 GB); the Steam version has no Russian | from secondary sources (gogdb, Steam discussions) |
| VGTimes: "Sacred ReBorn v3.0 + HD (beta)" (HD → 1366×768) | from secondary sources |
| DTF: an interview with the author of the SacredNL mod, who started in 2019 with reverse engineering, moved the work into an external DLL and is interested in file formats | from secondary sources |
| 3DNews, StopGame: coverage of Sacred 2 Remaster and game pages | from secondary sources |
| Zone of Games: no Sacred 1 thread; gameru.net: a Russian localization for Sacred 2 only; no VK/Discord groups for Sacred 1 were found | from secondary sources (three search queries) |

Consequences.
(1) Russian localization comes through the Russian GOG depot or Akella's disc version, for owners of the corresponding copies; the engine must read `global.res` in any language, and the new engine's text, drawn by Bevy (parley) at native resolution with a free Unicode font planned for what the original TTFs lack, is to remove the GDI fonts' limitation on Cyrillic.
(2) Potential contributors: the author of SacredNL, bssth (sacred-sdk), Russian-speaking players on the kryotek lobby.
(3) Documentation language: everything in this repository is written in English, so the README and the format descriptions are accessible to the international community (§4.1).

## 5. Lessons from comparable projects

### 5.1 Summary table

Dates come from GitHub metadata, CHANGELOGs and release pages (verified from the primary source) unless noted otherwise.

| Project (game, year) | Approach / era | Timeline | Rendering modernization | Legal events | Lesson for us |
|---|---|---|---|---|---|
| isle + isle-portable (LEGO Island 1997) | matching, MSVC 4.2, reccmp/ReproBit; LGPL-3 | repo 2023-04-28 → functionally complete ≈2025 → byte-exact 2026 | isle-portable: SDL3 GPU (Vulkan/Metal/D3D12), D3D9, GL, software | not known | a small binary; comparison infrastructure first; the port in a separate repository; a rendering abstraction with several backends; byte-exactness cost another ≈year after functional completeness |
| devilution → DevilutionX (Diablo 1996) | reconstruction from PS1 symbols and a debug build, VC6 SP5 + PP; Sustainable Use License | ≈4 months of reversing by one person (2018); DevilutionX 1.0 in January 2020 | SDL2; D3D11 upscaling (1.4), virtual resolutions (1.5), a custom font with Cyrillic/CJK (1.3) | — | someone else's symbols speed things up several-fold (for us: server RTTI and log strings); the first 1.5 years went to porting and stabilization, then features |
| OpenJKDF2 (Jedi Knight 1997) | functional, C, mostly one developer | 2018 → 2,694/3,169 functions (95.9% excluding the rasterizer); ≈27 functions/month | the rasterizer was not decompiled but replaced with OpenGL 3.3; SDL2 + OpenAL; 64-bit and three OSes before reaching 100% | — | restore rendering as a contract, not verbatim; hook mode is marked deprecated: the bridge is not forever |
| TR1X / TR2X → TRX (Tomb Raider 1996/97) | bridge → own exe; GPL-3 | TR1X: injection 2021-02 → own exe 2021-12 (10 months); TR2X: 2024-04 → 2025-01-01 (8 months) | OpenGL 3.3 Core; UI scaling on day 4; aspect/border/integer upscale instead of a list of resolutions (4.13) | — | the fastest "bridge → autonomy" cycle, with an experienced team; do UI scaling early |
| OpenRCT2 (RCT2 2002, asm) | bridge (DLL + import patch), GPL-3 | 2014-04 → v0.0.5 (December 2016): RCT2.EXE no longer needed (≈2.75 years) | OpenGL drawing engine in 0.0.5; "Enlarged UI" only in 0.4.12 (2024) | — | the canonical bridge; HiDPI arrived after 10 years: plan for it from the start |
| KeeperFX (Dungeon Keeper 1997) | DLL next to keeper95.exe; GPL-2 | 2008 → 1.0 (2023-11-10): the legacy code from the exe fully moved over | SDL, GUI scaling for 16:9, 4K fixes in 1.1 | — | a bridge without an exit criterion = 15 years |
| re3 / reVC (GTA III/VC 2001/02) | functional C++ (MSVC6), librw GL/D3D9 | full decompilation; Linux/macOS/Switch/Vita ports | widescreen | DMCA 2021-02-19; a lawsuit in 2021-09, dismissed with prejudice on 2023-04-03 after a settlement; the code is unavailable (HTTP 451) | — |
| re47 (Hitman C47 2000, MSVC6) | functional, solo, IDA, GOG build (retail uses SafeDisc) | 2024-09 → archived 2026-07-29; ≈15–20% in a year, 10,055 lines | — | — | a solo effort on an MSVC6 binary of ≈7k functions stalls; a DRM-free build as the static-analysis target (same for us: the GOG client) |
| D2MOO (Diablo II 2000, MSVC6 C++) | DLL by DLL, D2.Detours; MIT + a non-commercial caveat | since 2020; only D2Common.dll builds and is swapped in | none (it remained a modders' tool) | — | the original's modularity helps (for us: server/client); do not repeat contradictory license caveats |
| SpaceCadetPinball (2001) | decompilation with a public PDB, one person; MIT | 2021-01 → a cross-platform port within months | SDL2 | — | symbols → months; a "snowball" of ports after publication |
| OpenNox (Nox 2000, C) | C → Go decompilation via cxgo; GPL-3 | years of manual refinement | — | — | cross-language porting works only for C, and still takes years; there is no such path for C++ |
| fallout2-re / fallout2-ce, fallout1-ce (Watcom) | a `-re` reference (left untouched) + a `-ce` modernization; Sustainable Use License | fallout2-ce 2022-05; fallout1-ce ≈8 months later on the same engine | SDL2, `f2_res.ini` | — | two repositories: the reference and the port; a shared engine pays off (we have 258 classes shared between client and server) |
| dethrace (Carmageddon 1997, Watcom) | symbols of another build from the Splat Pack CD; reccmp; GPL-3 | 2019-10; 33% (2022-02) → 80% (2023-04) → 100% implemented; peak ≈77 functions/month | its own GL renderer was removed in 0.8 in favor of the decompiled software + 3dfx renderers; SDL3 in 0.10 | — | someone else's symbols; shrink the surface; the "GPL-3 and public domain" confusion in the README shows how not to do it |
| sm64 / oot + Ship of Harkinian (N64) | matching; port after ≈90% match | SoH: first release ≈4 months after the decompilation | DX11/GL/Metal; assets are extracted from the user's ROM | Nintendo (2020-05) took down compiled builds of the SM64 port, not the decompilation repository | do not distribute builds containing someone else's code; the assets come from the user's own copy, as with our converter |
| OpenGothic (Gothic 2 2002) | reimplementation on its own Tempest layer (Vulkan/DX12/Metal), essentially one developer; MIT | 2019 → the campaign and the add-on are completable | 3D at a configurable internal resolution, UI and text at native resolution; mesh shaders and ray query optional | — | a thin custom rendering layer; a ready recipe for 4K UI; REGoth-bs on a third-party engine died in 2020 |
| OpenMW (Morrowind 2002) | reimplementation; started in D in 2008 → C++ in December 2009; GPL-3 | 16+ years to completeness | OpenSceneGraph/OpenAL/SDL2 (per common knowledge) | — | switching languages at the start cost ≈1.5 years; a clean-room reimplementation of a large RPG takes a very long time |
| Arx Libertatis (Arx Fatalis 2002, official source released in 2011) | source port; GPL-3 | 1.0 in 2012-04 (FreeType instead of GDI text, amd64); 1.2 in 2021-07: HUD/book/cursor scaling, cutscene letterboxing, font size | GL 1.5+, DirectX backends removed | — | even with the source, UI scaling took ≈10 years as a leftover priority; the 1.2 feature list is a checklist for docs/05 |
| OpenTTD (TTD 1995, asm) | asm → C (2003–2004), then C++; the free OpenGFX/OpenSFX/OpenMSX sets (2010); GPL-2 | 0.1.0 in March 2004 | — | on Steam since 2021 | the only project free of both the original code and the original assets: unrealistic for us, except for fonts and an HD UI set |
| freeablo, openage, REGoth-bs | reimplementations "from scratch" | freeablo archived 2023-04 ("lost motivation watching devilution"); openage: 12 years without playable gameplay | openage: OpenGL, C++20 + Python | — | a reimplementation loses to decompilation once the latter gets going; architectural perfectionism without a game is a dead end |

### 5.2 Calibrating pace and timelines

* **Decompilation pace** in public projects is 27–77 functions/month per developer: OpenJKDF2 ≈27 (one person, 99 months), dethrace ≈29 on average and ≈77 in its peak 14 months with someone else's symbols, re47 solo for ≤22 months and stalled at ≈15–20%.
  For engineers with good names (RTTI, log strings, the functions shared with the server, sacred-bild's 7,631 names), ≈100–200 functions per FTE-month is plausible: a hypothesis not tested on Sacred.
* **The Sacred baseline**: ≈5,900 game functions in 3.48 MB (≈180k logical lines of C++), 410 classes; this is about twice JKDF2 (3,169 functions per the OpenJKDF2 README) and, by estimate, about an order of magnitude more than LEGO Island (the isle function count was not obtained: hypothesis).
  This calibration is for a team of people; the plan counts in Claude Code agent-hours (docs/07).
* **Bridge → standalone exe**: TR2X 8 months, TR1X 10 months, OpenRCT2 ≈2.75 years, KeeperFX 15 years.
  The spread is explained not by the size of the game but by whether there is an exit criterion and comparison infrastructure.
  The plan has no bridge: the game is written from the start as its own Rust executable.
* **Client and server on a shared engine**: fallout1-ce took ≈8 months after fallout2-ce; Sacred has 1,176 game functions and 258 classes shared; for our project this means that `GameServer.exe` is useful as an auxiliary binary during analysis, while the server itself is not reimplemented: multiplayer comes after version 1.0 and is written anew (PLAN.md ADR-09).

### 5.3 How comparable projects handled each subsystem

| Subsystem | What the comparable projects did (verified from the primary source) | Our choice (PLAN.md) |
|---|---|---|
| Window/input | SDL2 → SDL3 (isle-portable, dethrace 0.10, Arx 1.2, DevilutionX) | Bevy (winit), gamepad via gilrs |
| 3D rendering | SDL3 GPU + D3D9 + GL + software (isle-portable); Tempest Vulkan/DX12/Metal (OpenGothic); OpenGL 3.3 (TR1X, OpenJKDF2, re3/librw); DX11/GL/Metal (SoH) | Bevy on wgpu: Vulkan, DX12 |
| 2D/UI in 4K | UI and text at native resolution, separately from 3D (OpenGothic); HUD/book/cursor scaling, letterbox (Arx 1.2); aspect/border/integer upscale (TR1X 4.13); Enlarged UI + DPI (OpenRCT2); virtual resolutions (DevilutionX 1.5) | Bevy UI on the original's 768-pixel-high screen, scaled with the window's height (docs/05 §3.5); the original's visible area by default |
| Fonts | GDI → FreeType (Arx 1.0, 2012); a custom font with Cyrillic/CJK (DevilutionX 1.3) | Bevy's text (parley), the original TTFs; a free Unicode font (Noto) planned |
| Audio | OpenAL (Arx, OpenJKDF2, swars); SDL3 + miniaudio (isle-portable); SDL2_mixer (SpaceCadetPinball, KeeperFX) | Kira; FLAC masters built into Ogg Opus, decoded by libopus (#701) |
| Video | Smacker libraries (isle); most skip cutscenes | AV1 via FFmpeg during conversion, rav1d during playback; skipping |
| Networking | ZeroTier + TCP (DevilutionX 1.3); custom (OpenRCT2); absent (swars) | after version 1.0 (PLAN.md ADR-09, §13) |
| Simulation tick | DevilutionX 1.0: 20 fps → monitor refresh rate; TR1X: 60 fps as an experiment → the default | the original's world tick of 0.1 s, gathered from the frames (`frame::WorldTick`), rendering at the monitor refresh rate |
| Saves | Julius: compatibility with the original in both directions | not needed for us: our own save format (docs/06, §6) |

## 6. What we take and what we avoid

### 6.1 We take

1. **The community's names and addresses**: the sacred-bild function export, sacred-sdk's addresses and keyword tables, into the local Ghidra project.
2. **Format descriptions confirmed by two or three implementations** (PAK, keyx/wldx, TEX, GRN, FunkCode, global.res), as the basis for `opensacred-formats` and as a reference for cross-checking our converter.
3. **MIT code as a reference:** `sacred_hash` and the NetScript disassembler (sacred-sdk), the core of the Granny parser (grn2glb), the layouts of the openheilig engine; our Rust code is our own, and anything borrowed carries the notice in `THIRD-PARTY-NOTICES.md`.
   open-grn is a reference on the format only; its code is not used.
4. **OpenGothic's experience:** UI and text at native resolution regardless of the scene's scale; a modern renderer on top of the original assets; a reimplementation by one person is feasible.
5. **The experience of TR1X/TR2X and OpenRCT2:** UI scaling, widescreen and fonts in the first year, not later.
6. **Player expectations** from PureHD, ReBorn, SacredNL: arbitrary resolution without balance changes, windowed mode, gamepad, widescreen cutscenes.

### 6.2 We avoid

1. **Publishing any game bytes or derivatives**: `Sacred.exe`, assets, converted files, HD packs, dumps.
2. **Code with an incompatible or unclear license:** GPL-3 (sacred-bild, OpenMiles), AGPL-3 (kryotek), "no license" (SacredEngineRemake), leaked or unlicensed middleware SDKs.
3. **Byte-exact decompilation as a goal** (isle: another ≈year after functional completeness): we need a game, not a copy of the code.
4. **A hybrid bridge** (KeeperFX: 15 years in the bridge): without a parity requirement it is not needed.
5. **Blind reimplementation** without analyzing the binary (freeablo, openage, REGoth-bs): we recover the combat, item and script rules by analyzing the original's behavior.
6. **Postponing HiDPI, widescreen and fonts** (OpenRCT2 and Arx: about 10 years).
7. **Implying an affiliation**: OpenSacred uses the game's name nominatively, only to say what it works with; see docs/08 §6.

### 6.3 Ecosystem work by milestone

| Milestone | What we take |
|---|---|
| M1 Converter | openheilig's format descriptions, the grn2glb and open-grn readers as references, sacred-sdk's `sacred_hash` and disassembler |
| M2 Vertical slice | OpenGothic's rendering and UI recipes |
| M3–M4 Rules and quests | the sacred-bild function export and sacred-sdk's addresses, into the local Ghidra project; sacred-bild's RE_NOTES, sacred-sdk's opcode tables, openheilig's bytecode documents |
| M7 HD assets | the experience of the community's HD mods (PureHD, ReBorn HD): what players consider important |

## Open questions

1. The contents of the unofficial 2.29/2.29.14 patches beyond the two fixes ported into sacred-sdk; where they are hosted (Discord) and whether there is a changelog.
2. The authors and sources of PureHD, ReBorn HD and SacredNL; whether they are willing to participate; whether PureHD has open code.
3. The fingerprint (PE stamp, size) of Akella's Russian 2.28 build and of the Russian GOG depot: a separate exe or the ENG exe with a different `global.res`?
   The project is verified only on the English GOG depot; the answer determines whether other depots can be supported.
4. Whether to coordinate a shared GRN specification with openheilig, grn2glb and open-grn.
5. WSGF/PCGamingWiki entries for Sacred Gold, the ProtonDB rating, the state of the Lutris script: the sites were unreachable from the research environment.
6. The "Resacred" project (a C++ remake, a source of pak/keyx/iso structures according to links from sacred-sdk and openheilig): the repository was not found.
7. The meaning of the `COMPAT_VIDEO` key, which the GOG release enables by default (a medium-confidence hypothesis from static analysis: a compatibility mode for cutscene playback, §2.2): confirm it by analyzing the cutscene playback function and comparing captures on the original with and without the key (docs/01).
   What the GOG installer writes to the registry has not been checked.

## Sources

Local verification: a GOG installation of Sacred Gold 2.28 (English; `Sacred.exe` SHA-256 `db9b86a4…`), checked against `docs/manifest-gog-2.28.txt`.

External primary sources (read in the research environment on 2026-10-15; only github.com and raw.githubusercontent.com were reachable):

* https://github.com/kirides/sacred-bild (README, docs/Display.md, Rendering.md, Multiplayer.md, RE_NOTES.md, src/game/sacred_addr.h, tools/data/functions_eng.tsv, commit history)
* https://github.com/openheilig/engine ; https://github.com/openheilig/research (builds/build-survey.md, engine/tech-stack.md, formats/granny-grn.md, formats/script-bytecode.md, engine/granny-runtime-oracle.md) ; https://github.com/openheilig/tools
* https://github.com/bssth/sacred-sdk (README, ports/, re/py/, .claude/knowledge/) ; https://github.com/Aytackydln/SacredEngineRemake ; https://github.com/karolak6612/open-grn ; https://github.com/ancaria-dev/grn2glb ; https://github.com/maci0/openmiles ; https://github.com/kryotek777/Sacred-Gold-LobbyServer ; https://github.com/stefanwerfling/sacredbamboo ; https://github.com/sonicmouse/SacredGameTools ; https://github.com/S22F5/GhidraExports ; https://github.com/pnxr/sacred2-lobby-emulator
* Wrappers: https://github.com/dege-diosg/dgVoodoo2/releases ; https://github.com/narzoul/DDrawCompat/releases ; https://github.com/FunkyFr3sh/cnc-ddraw ; https://github.com/doitsujin/dxvk/releases
* Comparable projects: https://github.com/isledecomp/isle , https://github.com/isledecomp/isle-portable , https://github.com/isledecomp/reccmp ; https://github.com/diasurgical/devilutionX (docs/CHANGELOG.md) ; https://github.com/shinyquagsire23/OpenJKDF2 ; https://github.com/LostArtefacts/TRX (docs/tr1/CHANGELOG.md, docs/tr2/CHANGELOG.md) ; https://github.com/OpenRCT2/OpenRCT2 ; https://github.com/dkfans/keeperfx ; https://github.com/0danny/re47 ; https://github.com/ThePhrozenKeep/D2MOO ; https://github.com/k4zmu2a/SpaceCadetPinball ; https://github.com/opennox/opennox , https://github.com/gotranspile/cxgo ; https://github.com/alexbatalov/fallout2-re , https://github.com/alexbatalov/fallout2-ce , https://github.com/alexbatalov/fallout1-ce ; https://github.com/dethrace-labs/dethrace , https://github.com/dethrace-labs/reccmp-report ; https://github.com/n64decomp/sm64 , https://github.com/zeldaret/oot , https://github.com/HarbourMasters/Shipwright ; https://github.com/Try/OpenGothic , https://github.com/Try/Tempest ; https://github.com/OpenMW/openmw ; https://github.com/arx/ArxLibertatis ; https://github.com/OpenTTD/OpenTTD ; https://github.com/wheybags/freeablo ; https://github.com/SFTtech/openage ; https://github.com/halpz/re3 (HTTP 451)
* re3's legal events: https://github.com/github/dmca (2021/09/2021-09-29-take-two-legal-action)

Secondary sources (search-result summaries; the sites were unreachable from the environment, re-check before use): gogdb (product 1207658688), Steam Community 12320 discussions and the GOG forum "sacred_series", VOGONS, Nexus Mods "sacred" (mods/12, 24, 26), gamepressure (PureHD, ReBorn, Movies Fullscreen), SacredWiki, Lutris (3688), Phoronix (LGP 2009), Wikipedia/Shacknews (Ascaron), PocketGamer.biz/GameSpace (2020 IP exchange), Gematsu/PCGH/PC Games/3DNews (Sacred 2 Remaster), DTF (SacredNL interview, in Russian), VGTimes (in Russian), ruwiki/StopGame/MobyGames (Akella editions), TorrentFreak/Exputer (re3).
