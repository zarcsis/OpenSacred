# Data formats and assets

Plan document 02.
Describes all containers and data files of Sacred Gold 2.28 (GOG, English), how far each has been parsed as of 2026-10-20, and what each is converted into.
The game does not read the original formats: the `opensacred-convert` converter translates them once into open formats, and the new engine works only with those (PLAN.md, §4 and §7).

The level of confidence is marked with these words: **confirmed on file** (parsed by a script on a local copy), **confirmed by static analysis** (client code in Ghidra or via the `tools/analysis` scripts), **community-reported** (not checked on the file), **hypothesis**.

## Summary

- Most game containers (≈20 files in `pak/` and `World/`, ≈1.9 GiB of the ≈2.55 GiB release) share one layout: a 0x100 B header, a `count × 12` B directory `(type, offset, size)`, then arrays of fixed-size records written straight from memory.
  `sectors.keyx`, `Creature.pak`, `Weapon.pak` and `MOTIONS.PAK` have the header without a directory, their records right after it, and `Triggers.PAK` has one directory entry for the whole array (§1, §5.5).
  Confirmed on file for all containers.
- Textures: 25,535 records, 22,932 of them 256×256; format 4 = ARGB4444 + zlib, format 6 = uncompressed A8R8G8B8 (bytes B, G, R, A); no mipmaps, DXT or color key; 10,591 ground atlases `MIX*.444`.
  Converted to PNG (master); KTX2 with mipmaps comes with the HD packs (M7, docs/05 §5).
- Models: `models.pak` and `models03.pak` = 1,574 models + 3,423 animations (1,572 + 3,421 in `models.pak`), each a complete Granny **1.2b** file.
  The format has been solved by three independent community readers; the MDL container, the GRN container, the skeleton, the meshes and their materials are confirmed on our files (§4.1–4.2); converted to glTF 2.0 with skeletons and clips.
  The original's animation blending semantics are carried over approximately, via Bevy's animation graph.
- World: a 100×100 grid of sectors (6,050 exist); a sector is 64×64 cells of 32 B each with corner heights (step 5, 78.4% of cells are flat).
  Converted into binary sectors; the new renderer builds real terrain from the heights.
- NetScript: 141 record types, 113 of which occur in the 20 script directories, 45 cover 95% of records; the interpreter is in the client.
  Converted into an intermediate representation (IR) for the interpreter in `opensacred-rules`.
- `global.res`: UTF-16LE, 23,123 strings with id = `sacred_hash(name)`; the formula is verified.
  Converted to Fluent.
- Original saves are not supported; the hero templates are converted with their sections whole, into starting presets once their layouts are recovered (§9).
- Sound: `sound.pak` holds 6,598 samples, 3,194 RIFF WAV (PCM and IMA ADPCM) and 3,404 MP3; confirmed on file.
- Not established: the semantics of some cell and static object fields (Open questions 1 and 2).

## 1. General layout of PAK containers

Confirmed on file for all containers in the §2 table.

| Element | Layout |
|---|---|
| Header | 0x100 B: `char magic[3]`, `u8 version`, then u32 fields (counters, sizes of the source paks), the rest zeros; in TXL/MDT/TRG the rest is 0xCC, i.e. the header is also an `fwrite` of a struct |
| Directory | from 0x100: `count × { u32 type; u32 offset; u32 size; }`, `offset` is absolute from the start of the file; entry 0 is usually empty (`INVALID_*`, index 0 = "no object"); none in `sectors.keyx`, `Creature.pak`, `Weapon.pak` and `MOTIONS.PAK`, whose records start at 0x100, and one entry for the whole array in `Triggers.PAK` |
| Data | an array of fixed-size records (CIF, WPN, MHP, ISO, ITM, SPF, TRG, WLK, Static, Floor) or "record header + payload" (TEX 80 B, MDL 1,194/256 B) |
| Compression | zlib only (`78 9C` in TEX, `78 DA` in AMH, `78 01/9C/DA` in wldx); no LZ/RLE occurs |
| Strings | ASCII/cp1252 in fixed `char[32]`/`char[64]` fields (German internal names with `ä`); UTF-16LE only in `global.res` and in the hero summary |
| Alignment | structs are packed (`#pragma pack(1)`): u16/float at odd offsets (TEX: w/h at +0x20, size at +0x25; ITM: name at +0x37, float at +0x57) |
| Indexing | object id = ordinal number of the record in the container; other paks, scripts and saves use these indices |
| `*.TMP` caches | `Texture.TMP`, `Models.tmp`, `MOTIONS.PAK` store the source pak's size in the header for invalidation (e.g. 0x334C72F3 = 860,648,179 = size of `texture.pak`) and are written **to the game directory** |

Sketch of the common reader (the same for all containers; record semantics are a separate type per format):

```cpp
#pragma pack(push, 1)
struct PakHeader { char magic[3]; uint8_t version; uint32_t fields[63]; };   // 0x100 B
struct PakDirEntry { uint32_t type, offset, size; };                          // 12 B
#pragma pack(pop)
static_assert(sizeof(PakHeader) == 0x100 && sizeof(PakDirEntry) == 12);
// Loader: mmap the file → check magic/version → span<PakDirEntry>(0x100, fields[0]) →
// for each entry span<uint8_t>(offset, size) → reinterpret as the format's Disk* struct.
```

Why this matters: the on-disk structures are dumps of `cObjectShared`, `cTrigger`, `cSectorChunk` and similar objects, so their layouts are visible both in the data and in the code; according to openheilig (not re-verified), the sizes have not changed since the 2001 prototype.
Parsing lives only in `opensacred-formats`: past that point the converter writes ordinary `serde` types, and the new engine has neither pointers nor packed structs.

## 2. Container summary table

Record sizes are canonical numbers (confirmed by the file size dividing without remainder).
The scripts in the last column are under `tools/analysis/`.

| File | Magic/version | Records × size | Purpose | Parsing status | Checked by |
|---|---|---|---|---|---|
| `pak/texture.pak` (860 MB) + `texture03.pak` | `TEX` 3 | 25,535 (+3); 80 B record header + data | textures | format confirmed, including the fmt 6 channels | `formats/tex_decode.py`, `formats/tex_fmt6.py`; §3.1 |
| `pak/Texture.TMP` | `TXL` 1 | 25,538 × 80 | `cTextureManager` directory cache | confirmed; not needed by the port | §3.2 |
| `pak/models.pak` (434 MB) + `models03.pak` | `MDL` 3 | 1,572 + 2 × ModelDesc 1,194 B and 3,421 + 2 × MotionDesc 256 B, each followed by a GRN file | Granny 1.2b models and animations | container and GRN container confirmed on every record; GRN contents: §4 | `formats/grn_dump.py`; §4.1–4.2 |
| `pak/Models.tmp` | `MDT` 2 | 1,574 × 1,194 + 3,423 × 256 | descriptor cache | confirmed | `formats/grn_dump.py`, `formats/mhp_vs_mdt.py`; §4.1 |
| `pak/MOTIONS.PAK` | `MHP` 1 | 3,423 × 198 | motion parameters (= `MotionDesc[0x42:0x100]`) | structure confirmed; the attack's hit fractions confirmed by code (§4.1), the other floats a hypothesis | `formats/tables_dump.py`, `formats/mhp_vs_mdt.py`; §4.1 |
| `pak/tiles.pak` | `ISO` 3 | 90,132 × 64 | tile definitions: name `isoNNNN.tga`, texture index, diamond number 0..17 | confirmed; diamond geometry from the code (§5.4) | `world/world_dump.py`; §5.5 |
| `pak/mixed.pak` (14 MB) | `MIX` 0 | 32,096 records: 16 B + nPieces × 64 B | the sprites of the static objects, assembled from pieces of atlases | confirmed on every record (§3.3) | `formats/mixed_dump.py`; §3.3 |
| `pak/Items.pak` (= `Items03.pak`) | `ITM` 5 | 32,768 × 128 (17,408 occupied) | templates of all objects (creatures, statics, FX, interactive objects) | medium–high | `formats/mixed_dump.py`; §6.1 |
| `pak/Creature.pak` | `CIF` 0 | 474 × 86 | base creature stats | field map confirmed on every record (§6.2) | `formats/tables_dump.py`; §6.2 |
| `pak/Weapon.pak` | `WPN` 8 | 4,883 × 258 (+ 4,883 × 64 of zeros) | weapon and armor types | medium | `formats/tables_dump.py`; §6.2 |
| `pak/sndProfiles.pak` | `SPF` 1 | 8,192 × 184 (175 occupied, 2,355 sample ids) | "event × variants" sound profiles | confirmed | `formats/sound_dump.py`, `formats/spf_names.py`; §10.1 |
| `pak/sound.pak` (392 MB) | `SND` 1 | 50,000 × {flags, offset, size}, 6,598 occupied | samples: RIFF WAV (PCM, IMA ADPCM) and MP3 | confirmed (`snd_ls.py`) | `formats/snd_ls.py`, `formats/sound_dump.py`; §10.2 |
| `World/sectors.keyx` | `WLK` 5 | 6,050 × 768; header `6050, 100, 100, −18432, 112152` | sector keys, bounds, offsets into wldx | structure confirmed; some fields are a hypothesis | `world/world_dump.py`; §5.1 |
| `World/sectors.wldx` (64 MB) | `WLD` 5 | 6,050 zlib blocks | sector data: the 32 B name, 64×64 cells of 32 B from `+0x20`, then the layers (variable) | confirmed (all blocks decompressed); cell fields: §5.2 | `world/world_dump.py`; §5.2 |
| `World/Static.PAK` (79 MB) | `OBJ` 1 | 1,038,014 × (12 B directory + 64 B) | static instances (`cObjectShared`) in cell chains | confirmed on every record (§5.5) | `world/world_dump.py`; §5.5 |
| `World/Floor.PAK` (188 MB) | `OBJ` 1 | 6,713,136 × (12 B directory + 16 B) | floor overlays (`cPatchIso`) in cell chains | confirmed on every record (§5.5) | `world/world_dump.py`; §5.5 |
| `World/Triggers.PAK` | `TRG` 1 | 2,268 × 16 (1,246 occupied) | triggers → 20-bit reference (hypothesis: a Static.PAK index) | confirmed | `formats/tables_dump.py`; §5.5 |
| `Templates/Hero*.ptx`, `save/hero*.pax` | `AMH` 0x1B (u16 version 0x101B) | 0x100 + 16 × 12 table + sections | hero templates and saves | container confirmed; section fields partially | `formats/amh_dump.py`; §9 |
| `scripts/us/global.res` | `SZ` 0 | 23,123 × 16 + UTF-16LE pool | localized strings | confirmed; the id hash is known | `formats/global_res_dump.py`, `formats/global_res_hash.py`; §8 |
| `bin/**/{FunkCode,StartCode,QuestCode}.bin` | — | stream of `(u16 op, u16 len, operands)` | NetScript bytecode, 20 directories | encoding confirmed (parsed without remainder); semantics per sacred-sdk/openheilig; 9 Underworld `QuestCode.bin` files are empty (0 B, §13) | `scripts/netscript_census.py`, `scripts/netscript_dump.py`; §7 |
| `bin/**/{DefPos,Vectoren,QuestPoolCode}.bin` | — | Vectoren: three counted tables (84 B function blocks, 292 B quests, regions); DefPos: 100 B positions and 80 B dialog entries; QuestPoolCode is empty (0 B) | Vectoren: the function blocks of `FunkCode.bin`, the quests and the regions; DefPos: a cache the client rebuilds | Vectoren confirmed on every file; DefPos skipped (§7.1) | `scripts/netscript_dump.py`; §7.1 |
| `bin/*.bin` (11 tables: Balance, World, World2, sets, wpmod, wea, treppe, static10_18, merc, MultiStart, Rust) | none (static10_18: `map`) | see §6.3 | balance tables compiled from `scripts/*.txt` | medium | `formats/bin_tables_dump.py`; §6.3 |
| `font/*.ttf`, `pak/*.bmp`, `movie/*.wmv`, `mp3/*.mp3` | standard | 2 TTF; 6 BMP 1024×768, 24-bit; 10 WMV (113.3 MB; WMV3 640×480, WMAv2 or WMA Pro); 173 MP3 (399.5 MB) | fonts, loading screens, cutscenes, music | standard formats | `ffprobe`; §10.3, §11 |

Separately: the `BINARY #107` resource in `Sacred.exe` (lang 1031, 2,760,574 B, loaded by function 0x80E680 right after `.\SCRIPTS\%s\global.res`) is an encoded copy of `scripts/us/global.res` embedded in the executable; the port reads the file `scripts/<lang>/global.res` and does not need the resource (docs/01 §4.6; §8).

## 3. Textures

### 3.1 TEX: texture.pak and texture03.pak

Confirmed on file and by a decoder (`tools/analysis/formats/tex_decode.py`, `tex_fmt6.py`).

| Record field (80 B, packed) | Meaning |
|---|---|
| `+0x00 char name[32]` | a `*.TGA` or `MIXnnnn.444` name; arbitrary case; 7 names occupy all 32 bytes without a NUL and are truncated (`….T`, `….TG`). Names are not unique: 25 occur twice, 6 of them with different pixels (`DRYADE_SCOUT_{ARMS,HANDS,HEAD,LEGS}.TGA`, `DAEMONIA_1024.TGA`, `DWARF.TGA`), so the texture identifier is the record number |
| `+0x20 u16 width`, `+0x22 u16 height` | 256×256: 22,932; 128²: 919; 64²: 724; 512²: 205; 1024²: 21; maximum 1024. Counted over both paks, as in `Texture.TMP`: `texture.pak` itself has 918 at 128², the remaining one is `CAB.TGA` from `texture03.pak` |
| `+0x24 u8 format` | 4: 16-bit ARGB4444, zlib (25,307 records); 6: 32-bit, uncompressed (227); 0: empty (1) |
| `+0x25 u32 dataSize` | for fmt 4, the size of the zlib stream; in `.444` atlases the field holds the uncompressed size 131,072 (the actual stream is 25–45 KB between neighboring offsets); for fmt 6, `w·h·4`. In all 25,534 records it equals the directory's `size` field, i.e. the 80-byte header is not included in `size`. In 14,716 fmt 4 records other than atlases the stream ends exactly at `dataSize`; in the last three atlases `offset + 80 + dataSize` runs past the end of the file. So for fmt 4 the field is only an upper bound: the zlib stream defines its own length |
| `+0x29 u64 packedOffset` | `(offset << 8) \| pakIndex`; 0 in the pak, filled in only in `Texture.TMP` |
| `+0x50` | data |

Format 4 is ARGB4444 (`D3DFMT_A4R4G4B4`, u16 LE: A in the high nibble): confirmed by gradient statistics and visually on `CAB.TGA`, `KROMBACHER.TGA`, `AJ_TEST.TGA`; matches sacred-sdk and sacred-bild.
The stream decompresses to exactly `w·h·2` bytes: **there are no mip levels in the file**, and the engine does not create them (no `MIPFILTER`/`MIPMAPLODBIAS` in the client, confirmed by static analysis).
The `DXT` strings and the `GFX32` key in the exe concern surface format negotiation when uploading to D3D7, not the files.
Format 6 (227 records: the menu and hero selection screens `GUI_OPTION*`, `GUI_CHAR*` and their `UW` variants as 256×256 tiles, 1024×1024 `UI_*.TGA` UI canvases, `FX_HORIZON.TGA` etc.) is `D3DFMT_A8R8G8B8`: u32 LE, bytes B, G, R, A in memory, as in the TGA itself; rows top to bottom; uncompressed, `dataSize = w·h·4`.
Confirmed on file 2026-10-16 by the `tools/analysis/formats/tex_fmt6.py` script: byte 3 is alpha (77% of values are 255 and 11% are zeros, versus 4–7% for bytes 0–2); the `GUI_OPTION1…12` tiles, assembled 4×3 into the 1024×768 main menu screen, give natural colors only in BGRA order (in RGBA the stone comes out light blue and the foliage blue).

The directory is sorted by offset; records are packed back to back with no gaps (confirmed on file 2026-10-16).

Conversion: each record is decompressed (zlib) and converted to RGBA8 → PNG as the master copy; KTX2 with mipmaps (BC7 for PC, ASTC or UASTC for Android and the web) comes with the HD packs (M7, docs/05 §5).
In total ≈1.6 Gpix, i.e. ≈3.2 GB of uncompressed 16-bit data; ≈1.6 GB in BC7 (estimate).
Format 6 is converted to RGBA8 by swapping bytes B and R.

Implemented 2026-10-16: `opensacred_formats::tex` and `opensacred-convert textures` write `assets/textures/<pak>/<number>-<name>.png`.
The number is the five-digit record number, the name is lowercase without `.tga`, with characters outside `[a-z0-9._-]` replaced by `_`.
This yields 25,536 PNGs (1.2 GB) in 16.5 s on 32 threads.
The pixels match `tex_decode.py` on a sample of 97 fmt 4 records (41 sizes, 22 atlases, all names without a NUL, the last record) and `tex_fmt6.py` on 48 menu screen tiles; the share of alpha 255 and 0 in fmt 6 matches the script output (77.4% and 11.2%).

One menu tile's alpha is faint, **confirmed** on the converted PNG 2026-10-20 (#352): record 347, `GUI_UW_OPTION12.TGA` (format 6, 256 × 256), one of the twelve tiles of the Underworld main menu, has 39,076 texels of alpha 195 to 255 over its body (14 of them 255), where its neighbours' bodies are 255, besides 8,658 of alpha 0, 9,042 of 1 and an edge of 2 to 194; drawn over the menu's animated water, its rocks show the water through them (**community-reported**: DarkMatters file 770, "Main Menu Graphics FIX").
`opensacred-convert textures` repairs that one record as it writes it: its alphas are scaled by 255/195, held at 255, so its body is opaque and its edge keeps its ramp (docs/decisions/2026-10-20-menu-tile-alpha.md); the lavas' frames are repaired too (§5.2).

The texture fixes of "Sacred Gold: Miscellaneous Fixes" (**community-reported**: PCGamingWiki file 3994, packaged by Narzoul, the fixes credited to Alfchaval, Trecasim and Onion; checked for #358 on 2026-10-21) are described in public only as minor texture errors, mainly of the lava and the main menu.
They are the three lavas' streaks of §5.2 (#347) and the faint tile above (#352): of the 24 tiles of the two main menus (`GUI_OPTION1`–`12`, `GUI_UW_OPTION1`–`12`, records 332 to 355), record 347 is the only one almost without an opaque texel (14 of 65,536 at alpha 255, the others 23 to 100 per cent; **confirmed** on the converted files).
Its other texture fixes are not named in what is public, so none can be identified without its files; the armours' reduced resistance glow it bundles (credited to PhanD00M) is a change of taste, not a repair.

### 3.2 Texture.TMP

`TXL` v1: a 0x118 B header + 25,538 × 80 B, the same record headers but with `packedOffset` filled in; 25,534 records with `pakIndex` 0 matched the `texture.pak` directory by (offset, size), two with `pakIndex` 4 come from `texture03.pak`.
Confirmed on file.
This is the `cTextureManager` cache, sorted by name; the new engine builds the index from the directories itself and **does not write to the game directory**.

### 3.3 MIX ground atlases and mixed.pak

- `texture.pak` already contains 10,591 pre-baked textures `MIX0.444 … MIX9999.444` (fmt 4).
  Confirmed on file.
  Hypothesis based on three inspected atlases (`MIX100`, `MIX5000`, `MIX9000`, 2026-10-16): these are not ground but packed fragments of static objects: tree crowns, roofs, rocks, logs on a transparent background.
- `mixed.pak` (`MIX` v0, 32,096 records, directory type 99): **the sprites of the static objects**, not blended ground tiles as an earlier reading guessed.
  Confirmed on every record 2026-10-17 by `tools/analysis/formats/mixed_dump.py` (layout per openheilig, engine `formats/mixed.gd`, MIT): record 0 is 16 B, `u32 99`, the file size − 16, `u32 16` and the fill `0xCCCCCCCC`; a sprite is `u32 n`, `u16 width`, `u16 height`, `i16 dx, dy` (meaning unknown; openheilig found that adding them misplaces the sprite), the fill `0xCCCCCCCC`, then `n` pieces of 64 B: `char atlas[32]` (`MIXnnn.444`), `u32` texture.pak record (10,585 atlases of 256 × 256), `u16 x1, y1, x0, y0` (the piece in sprite pixels), `u32 0`, `f32 u0, v0, u1, v1` (the piece in the atlas as fractions of its size).
  Every piece is as large in the atlas as in the sprite and falls on whole pixels; 62,406 of the 209,956 pieces (blocks of up to 64 pixels) run past the sprite's width or height.
  The width and height count from the pieces' smallest corner (the negation of `dx, dy`, §6.1): past them the pieces hold only texels of alpha 0, in every one of the 16,255 sprites with pieces, while 2,122 sprites have texels of some alpha past the width and height counted from their origin.
  Confirmed 2026-10-18 by `mixed_dump.py --alpha`, which decodes the atlases' alpha from `texture.pak`.
  16,255 sprites have pieces, 15,840 none.
  16 more bytes follow the last record (an empty sprite header, 30 × 61, outside the directory).
  An object type of Items.pak names its sprite at `+0x10` (§6.1), and a static object of Static.PAK puts the sprite's top left corner at its position (§5.5).
  Implemented 2026-10-17: `opensacred_formats::mixed`, matching `mixed_dump.py` on every record.

## 4. Models and animations

### 4.1 The MDL/MDT container and caches

Confirmed on file 2026-10-16 for every record by `tools/analysis/formats/grn_dump.py`.
Implemented the same day: `opensacred_formats::mdl`, cross-checked against the script record by record.

- **Header** (`MDL` v3): `count, nModels, nMotions, sizeof(ModelDesc) = 1194, sizeof(MotionDesc) = 256, X`, with `count = nModels + nMotions`.
  `models.pak`: 4,993 = 1,572 + 3,421, X = 13,268.
  `models03.pak`: 4 = 2 + 2 (`INVALID_MODEL`, `KROMBACHER.GRN`, `INVALID_MOTION`, `BO02_ACTIVATE.GRN`), X = 19,897.
  The meaning of X is unknown; the `Models.tmp` and `MOTIONS.PAK` headers carry the same 13,268.
- **Directory:** all models (type 0x40) first, then all motions (type 0x41).
  In each pak the first model and the first motion are the placeholders `INVALID_MODEL` and `INVALID_MOTION` with `dataSize = 0`.
- **Record** = descriptor + GRN file.
  Records lie back to back in directory order up to the end of the file.
  The directory `size` is `1194 + 2·dataSize` for a model and `256 + dataSize` for a motion; why a model's data counts twice is not known (hypothesis: the loader's buffer for the file plus a working copy).
- **Shared descriptor start:** `char name[32]; u32 0xFF; u32 dataOffset; u32 dataSize; u16 flags`.
  `dataOffset` = record offset + descriptor size: the GRN file starts right after the descriptor.
  `flags` is 3 for every model, 5 for every motion and 0 for the placeholders (meaning unknown).
  One name has 33 characters, `VAMP_WEAPON_SWORDLONG_2HANDED.GRN` (record 1546); its last character and NUL overwrite the `0xFF` word, and the reader cuts the name to 32 characters, as with TEX names (§3.1).
- **ModelDesc** (1,194 B): `+0x70` holds 256 u32 motion slots.
  A slot is the ordinal of a motion among the motions of the same pak (record `nModels + ordinal`); 0 is none, and slot 0 is always 0.
  `models.pak` fills 5,110 slots, the largest ordinal is 3,420.
  The slot number is the game's motion enum (openheilig; **confirmed** by code 2026-10-19, [docs/re/motions.md](re/motions.md)): the client plays a creature's action by slot, standing 1–13, the fight stance 14–26, walking 27–39, running 40–52, defending 53–65 and attacking from 79, by the class of what the creature holds, and a rider's idle 169.
  `+0x470` holds three equal floats, the model's own scale (**confirmed** by code 2026-10-21, #443): 1.0 in 1,538 of 1,574 models; 0.33 for the four gold piles `GOLDCOINS_01`, `_02`, `_05` and `_10`, 0.43 for `MEDUSA`, 0.5 for the six `UPGRADE_*` runes, 5.0 for `HQ_DEMONHORN_01`, and 1.1 for the placeholder `INVALID_MODEL` and 23 armours and helmets (`AMAZONE_*`, `ELVE_MAGICAN_*`, `ELVE_SORCESS_*`, `WARRIOR_ARMOUR_*`).
    `cGrannyModelManager::getModel` (`0x411F50`) opens a model with `GrannyOpenModel` and gives it once to `GrannyTransformModel`, with no translation and the diagonal `+0x470`, minus `+0x474`, `+0x478`: Granny applies the scale to the model's data as it loads it, and a model named exactly `RING.GRN`, which the paks do not have, gets 0.1 more on each axis.
    `cGrannyModelManager::getMotion` (`0x4118B0`) gives a motion it opens for a model's slot to `GrannyTransformAnimation` with the same diagonal from that model's descriptor, so a clip is scaled with the model that plays it (**hypothesis**: the motion cache is keyed by the motion's number, so a motion shared by models of different scales may keep the first one's).
    The world's draw adds no scale of its own (docs/re/drawing.md, "The models' scale").
    The minus on the middle axis is the same on every model; on the screen it does not show: the models of the original and of our glTF, whose frame is a rotation (§4.5), have one handedness (**confirmed** in the original 2026-10-22 with its driver, #445).
    The guard 26 (`wache2_rg1`, type 295) at his post at (3347, 2535), at noon at zoom 0.5, faces the viewer and a little to the left in the original and in `opensacred-game` alike, and holds his sword in his right hand, on the screen's left, his cloak and his belt's buckle falling the same way; a mirrored model would hold it, by the same right hand's bone, on the screen's right.
    So the client's view or its turn takes the mirror back out (not read: which of the two), and the converter's frame needs none.
    At 0.33 a gold coin, a disc 14.76 units across in its file, is 4.9 units across, and `GOLDCOINS_01`, 29.39 × 29.36 units on the ground in its file, is 9.70 × 9.69.
    `opensacred_formats::mdl` reads it (`Record::scale`), cross-checked against `grn_dump.py`'s `scale` column, and `opensacred-convert models` applies it (§4.5).
  `+0x47C u32` and `+0x4A8 u16` hold the MSVC fill `0xCC` in the pak; the cache fills them in.
- **MotionDesc** (256 B): `+0x2E u16` holds `0xCCCC` in the pak; the parameter block `+0x42…+0x100` is described with `MOTIONS.PAK` below.
- **`Models.tmp`** (`MDT` v2) = 0x118 + the descriptors of `models.pak` and `models03.pak` in that order: 1,574 ModelDesc, then 3,423 MotionDesc (exactly the file size).
  The header: `0, size(models.pak), 13268, 1574, 3423, size(models.pak + models03.pak)`.
  The cached descriptors match the pak ones except for: the global number (ModelDesc `+0x4A8`, MotionDesc `+0x2E`); the pak index at ModelDesc `+0x47C` (0 for `models.pak`, 4 for `models03.pak`, like `pakIndex` in `Texture.TMP`, §3.2); the MotionDesc parameter block `+0x80…+0x100` (filled from `MOTIONS.PAK`); and the motion slots, where pak ordinals become global numbers and custom motions fill extra slots (`KROMBACHER.GRN` slots 169 and 170 point to `BO02_ACTIVATE.GRN`, global motion 3,422).
  The new engine builds the same index from the pak directories and does not write to the game directory.

`MOTIONS.PAK` (`MHP` v1, 3,423 × 198 B; **checked on every record 2026-10-17** by `tables_dump.py`: `u16 index; u16 0; u16 index; u16 v` (v = 0 but in one record), zeros to `+0x46`, `f32[4]` at `+0x46`, zeros, `f32[9]` at `+0x62`, zeros, `u16` at `+0xBE` (0 but in two), zeros; the parameter blocks of the motions in `models.pak` itself are all zero, so this file is their only source; `opensacred-convert tables` writes them with the motion names to `tables/motions.ron`) is a byte-for-byte copy of `MotionDesc[0x42:0x100]` for all 3,423 records (confirmed on file by `tools/analysis/formats/mhp_vs_mdt.py`).
It is the output of `cGrannyModelManager::saveMotionParams()`: motion parameters compiled from `CUSTOMMOTION.TXT`, which is not shipped.
The float table (0.005–1.4) correlates with the motion type: WALK/RUN use `+0x80/+0x84` (402 of 495), ATTACK/SPECIAL use up to 8 values at `+0xA0…+0xBC` (827 of 989 attacks).
A record's offset `o` is `MotionDesc`'s `o + 0x3A` (its first 8 bytes its own): `f32[4]` at `+0x46` is `MotionDesc +0x80`, `f32[9]` at `+0x62` is `+0x9C`, so that the attack's hit fractions `+0xA0` and `+0xA4` are `floats_62[1]` and `[2]` of `tables/motions.ron` (**confirmed** by code 2026-10-20, `0x4EA1F0`, docs/re/fight.md).
Hypothesis for the other values: normalized event times (footsteps) for sound and particles.
Supporting it (2026-10-17, `tables_dump.py` and `tables/motions.ron`): the run and walk clips carry two values at block `+0x46`/`+0x4A` half a cycle apart (`SERA_RUN_1H` 0.5 and 0.000001, `SERA_WALK_1H` 0.63 and 0.06, `GLAD_RUN_1H` 0.81 and 0.31), the two footsteps of a cycle; `opensacred-game` plays its footsteps there.

### 4.2 The .GRN format: what is already known

The file format has been recovered by three independent readers (openheilig GDScript, openheilig Python and open-grn `grn_unpack.py`), which agree on the whole corpus: 4,991/4,993 `models.pak` records, 1,571/1,571 meshes and material→texture tuples, 3,420/3,420 track counts (community-reported).
grn2glb (C++) additionally parses 1,571 models and 3,420 clips, but no cross-check against the other readers on the whole corpus has been published.
RAD's vendor converter `grn2gr2.dll` crashes on the Sacred dialect, and there is no RAD documentation for the 1.x format.

**The container, confirmed on file 2026-10-16** by `tools/analysis/formats/grn_dump.py` on all 4,993 non-empty records of both paks; the layout follows the openheilig description (`formats/granny-grn.md`, CC BY-SA 4.0), restated here as facts measured on our files.
Offsets are from the start of the GRN file, i.e. from the record's `dataOffset` (§4.1).

- `+0x00`: a 64-byte signature, the same in every file.
- `+0x40`: the root header, 32 B: tag `0xCA5E0000`, section count 3, CRC-32 of the section table, 0, the size from `+0x40` to the end of the file, 12 zero bytes.
- `+0x60`: the section table, 3 × 20 B `{tag, 0, offset, CRC-32 of the section bytes, 0}`: `0xCA5E0102` at `0x9C`, `0xCA5E0103` at `0x1B8`, `0xCA5E0101` near the end.
  A section runs to the next one, the last to the end of the file.
  The CRC is the standard CRC-32 (the zlib polynomial); it holds in all 14,979 sections and 4,993 section tables, so a reader can verify every file.
- Each section is a chunk tree: `u32` node count, 12 unused bytes (zeros or stale pointers), then the nodes `{u32 tag, u32 offset from the section start, u32 number of descendants}` in pre-order.
  A node's own data runs from its offset to the next node's offset; the first node's data follows the node table; the last node is the terminator `0xCA5EFFFF` at the end of the section.
- Section `0102` always holds six nodes (`0200`, `1003` with `1001` → `1002` and `1000`, the terminator) and section `0101` only the terminator; the model or the animation is in section `0103`.
- Implemented 2026-10-16: `opensacred_formats::grn` reads the container of a whole file and verifies both CRC levels; `mdl::Record::parse_grn` reads the GRN file of a record in place, so errors name the record and the offset in the pak.
  On the game files all 4,993 files parse, and every tree matches `grn_dump.py`: the node counts per section and a fingerprint, CRC-32 of `(tag, own data length, descendants)` of every node in order.
- **The object section and the skeleton, confirmed on file 2026-10-16** by `grn_dump.py` on all 4,993 files; the tag names follow grn2glb (`src/core/grn_types.h`, MIT), the name chain its `grn_parser.cpp`.
  The top level of section `0103` is always `0200 0F03 0A01 0B01 0602 0507 0304 0D01 0C01 0E01 1205`.
  - `0200`, the string table: `u32 count, u32 length`, then `count` NUL-terminated strings that fill `length` bytes, string 0 empty; the node pads them to 4 bytes.
  - `0F03`, data extensions: `0F00` objects (8 B, always `1, 1`), each with one `0F05` of `0F01` properties (`u32` key string), each with one `0F06` of `0F02` values `{f32, u32 string}`.
    Every object has `__ObjectName`; most also have `__FileName` (the artist's `.max` path) and `__Description`.
    The float is 0 everywhere except 4 properties in motions whose several values pair times with marker texts (hypothesis: 3ds Max note tracks; a possible source of animation events).
  - `0B01`, transform channels: `0B00`, each with one `0F04`, the `u32` number of its object from 1.
  - `0507` → one `0505` skeleton → one `0508` → `0506` bones of 68 B: `u32 parent; f32 translation[3]; f32 rotation[4]` (quaternion `x, y, z, w`, unit within 1e-5) `; f32 scale_shear[9]`.
    Bone 0 is `__Root`, its parent field 0 points at itself; every other bone's parent precedes it, so one pass from the root composes the bind pose.
    The file has no inverse bind matrices.
  - Bone names: a bone record carries none.
    `0C01` → one `0C00` form → `0C07` → `0C08` (`u32` skeleton number 1) → `0C02`, a `u32` channel number from 1 per bone (a permutation of the channels), and an empty `0C05`.
    A bone's name is the `__ObjectName` of the object of channel `0C02[bone]`.
    The channel order differs from the bone order (in `SERAPHIM.GRN` it is close to the object order), so pairing channels with bones by position gives wrong names.
  - Census: one skeleton and one form per file; bones: 70,242 in models, 258,760 in motions.
    138 files repeat bone names (32 models, 106 motions; helpers such as `Dummy01` and `null_*` in dwarf armour), so a name does not identify a bone.
    115 bone names have bytes outside printable ASCII (Latin-1 umlauts and acute accents, tabs in one name), none in 0x80–0x9F, so Latin-1 and Windows-1252 agree.
    The forearms have two spellings: 176 files (the dwarf's and the gladiator's rigs and armour pieces, creatures such as Medusa, model 1249, and the Woodgod, 1300 and 1301) name them `Bip01 L ForeArm` and `Bip01 R ForeArm`, the rest `Bip01 L Forearm` and `Bip01 R Forearm`, and no skeleton has both (confirmed on the converted files); the client looks for `Forearm` and tries `ForeArm` when it is missing (`0x44C670`, [docs/re/equipment.md](re/equipment.md)).
    The converter writes both as `Forearm`, in the nodes and in the binding of the clips, a repair of the data (#355, docs/decisions/2026-10-20-forearm-spelling.md).
    The four instruments, `Instrument_Floete`, `…_Glocke`, `…_Harfe` and `…_Horn` (models 0746 to 0749), the only files that spell their socket `Weapon_bone01` or `Weapon_Bone01`, have no `Bone_weapon_01`, the right hand's socket of every other item (**confirmed** on the converted files); the converter names it `Bone_weapon_01` (#353, docs/decisions/2026-10-20-instrument-socket.md).
    Scale/shear differs from the identity by more than 1e-4 in 22,622 bones with a diagonal matrix (6,399 in models) and 1,175 with a general one (356 in models): the glTF export needs node scale, and for the general ones a decomposition or a matrix.
    Every scale/shear matrix is symmetric within 1e-6 of its largest value, so whether the 9 values are rows or columns makes no difference.
  - Implemented 2026-10-16: `opensacred_formats::grn::Objects` (strings, objects, channels) and `grn::Skeleton`; on the game files every skeleton reads and matches `grn_dump.py` by a fingerprint, CRC-32 over the bones of the name, a NUL, the parent field and the 16 floats.
- **Meshes, the form's bindings, textures, materials and render passes, confirmed on file 2026-10-16** by `grn_dump.py` on all 4,993 files; the tag names follow grn2glb, the vertex lists, the weights and the pass sources were measured on our files.
  A reference to an object (`0F04`), a mesh (`0C03`), a material or a texture is numbered from 1; indices into a mesh's own lists and a pass's form mesh from 0.
  Counts are for model files; those for the 23 motion files with meshes follow in parentheses where given.
  - `0602` → `0601` meshes, each with `0604` → one `0603` → `0801` positions and `0802` normals (`f32[3]` each) and `0804` → 0–2 `0803` texture-coordinate sets (`u32` 3, then `f32[3]` entries); then `0702` weights, `0901` triangles and `0F04`, the object whose `__ObjectName` names the mesh.
    17 meshes have no `0604`, no triangles and no weights: two in each of six `VLADY_SA1_*` files, five in `3_KEULE.GRN`.
  - Each attribute has its own indices, as in 3ds Max: a `0901` triangle is 24 B, `u32 position[3], u32 normal[3]`, and the texture coordinates of its corners come from the passes.
    The counts differ: 412,698 positions, 510,181 normals and 706,145 triangles in 2,555 meshes.
    Z is up, and 99.4% of the triangles (701,324 of 705,773 with a non-zero test) run counter-clockwise around the sum of their corner normals in a right-handed frame, as glTF expects.
    868 normals (1,027) are `(0, 0, 0)`, and 2,476 corners (2,927) in 144 meshes (28) use them; every other normal is unit within 1e-3.
  - `0702`: `u32` vertex count, `u32` highest local bone, `u32` most influences on a vertex, then per vertex `u32 n` and `n × {u32 local bone, f32 weight}`.
    The count is the position count, or 0 with a zero header for a rigid mesh.
    A vertex has 1–7 influences, its bones distinct, its weights positive and summing to 1 within 1e-5, in no fixed order.
    1,679 meshes in 1,010 files are skinned, 859 rigid; 16 meshes (none in motions) have vertices with more than 4 influences, 60 vertices in all.
  - The form binds meshes to bones: `0C00` → `0C06` → `0C03` form meshes, a `u32` mesh number from 1, each mesh once and in an order of their own (the `GLADIATOR` 3-cycle openheilig reports); each holds an empty `0C04` and `0C09` → `0C0A` entries of 32 B, `u32 bone; f32 radius; f32 min[3]; f32 max[3]`.
    Local bone *k* of the weights is entry *k*; a rigid mesh has one entry, the bone it follows.
    The radius and the box bound the positions weighted on that bone in the bone's bind-pose frame: they match within 1e-3 (relative) in 22,457 of the 22,474 entries with such positions, and within 2.2e-3 in the rest (`SNAKE_01.GRN`).
    So the positions are in the model frame of the bind pose, and the bind pose of a bone is `parent · (T · R · SS)` composed from the root, whatever the shear.
    532 entries have no weighted position but non-zero bounds (unexplained).
  - `0304` → `0301` textures: `0305` → one `0303` `{u32 width, u32 height, u32 format}` with the payload, then `0F04`, whose `__ObjectName` names the texture and `__FileName` holds the artist's file.
    Formats (names per grn2glb): 7, an external image without payload (1,923; 21); 4 and 5, VTex opaque and with alpha, a payload from `BIKi` (104 and 41; 22 and 88); 0 and 1, raw RGBX and RGBA, `width · height · 4` bytes (6 and 9; none).
    There is no DXT format 8.
  - **Embedded images, confirmed on file 2026-10-17** on all 270.
    A VTex payload is a complete Bink 1 file, revision `i`, of one frame: the standard 44-byte header (`BIKi`, file size − 8, 1 frame, the largest frame size, 1 frame again, width, height, rate 1/1, flags 0 or `0x100000` for alpha, no audio tracks), the frame table `(53, file size)` (the frame starts at 52 and is a key frame) and the frame; FFmpeg's Bink decoder reads all 255 (`yuva420p` with alpha).
    A raw texture's rows run from the top, its bytes R, G, B and A or padding.
    106 of the images are one colour: all 9 raw RGBA ones and 97 VTex streams of at most 256 bytes, among them the eight 252-byte textures of each of the 11 special attack motions that carry meshes.
  - The game binds every texture of a Granny file by name: `cGranny::bindTextures` looks up `"%s.TGA"` in `texture.pak` and logs `Texture [%s] not in PAK! IGNORED!!!` on a miss (community-reported, openheilig `formats/granny-grn.md`).
    244 of the 270 embedded textures have an image of their name there; the decoded VTex images match those twins in layout and orientation (by eye on `CROW` and `CHICKEN`; of the 114 opaque VTex with a twin, 113 differ least from it unflipped).
    What the game shows for the other 26 (18 VTex, among them the bodies of `CITIZEN_FEM.GRN`, `FARMER_MAL.GRN` and `ZOMBIE.GRN`, and 8 raw RGBA placeholders of weapons) is not known.
  - `0D01` → `0D00` materials without own data: a texture material holds `0D03` `{0, u32 texture from 1, 1}` and a `0F04` (2,083; 131), a color material `0D02` `f32 rgba[4]` and an optional `0F04` (181; 144).
    In every file each texture belongs to exactly one material, and the material order is not the texture order (`GLADIATOR.GRN` takes textures 2, 6, 1, 3, 4, 5, as openheilig found).
  - `0E01` → one `0E00` model (`u32` 1) → `0E07`, present only with meshes → `0E02` passes of 8 B, `u32 form mesh from 0; u32 material from 1`.
    A pass holds `0E03` with two sources, the color and the texture coordinates, each either `0E05`, a constant `f32 rgb[3]`, or `0E04`, a `u32` set number of the mesh; then `0E06`, a `u32` count and per triangle a `u32` triangle of the mesh with three `u32` set indices for each `0E04` source, corner by corner and within a corner source by source.
    Patterns: white and set 0 (2,715 passes), white and black for a mesh without sets (174), set 0 and set 1 (2: `SERABFG.GRN`, whose set 0 is all `(1, 1, 1)`, and `EDLST_RUND_GESCHL_KLEIN.GRN`).
    The second set of the other 874 meshes with two sets (48) is used by no pass; its meaning is unknown.
  - The passes of a mesh claim each of its triangles exactly once, in any interleaving (openheilig's skin and boot example); the 17 empty meshes have no passes.
    openheilig's two files whose group "declares a single triangle" are the two-set passes above, read with a one-set stride.
  - 2,357 meshes are drawn with texture materials only, 181 with color materials only, none with both.
    Color-only meshes are mostly helper geometry (the `Bone01`… meshes of `SNAKE_01.GRN`, the six `Bip01 Ponytail03_*` of `ELVE_SORCESS.GRN`); 308 of their 326 passes use the color `(0.682, 0.729, 0.796, 1)`.
    openheilig's trace shows the game not drawing those six meshes of `ELVE_SORCESS.GRN` (community-reported); hypothesis: the game draws texture passes only.
  - Implemented 2026-10-16: `opensacred_formats::grn::Model` (`Grn::model`) resolves the file's references to indices and rejects a file that breaks the layout above: an index out of range, a mesh bound by no form mesh or by two, a triangle that no pass draws or that two passes draw, a rigid mesh with more than one bone.
    On the game files every model reads, the counts above hold, and every record matches `grn_dump.py` by two fingerprints that `--records` writes, `mesh_crc` (geometry, weights and bone entries) and `material_crc` (textures, materials and passes).
- **Animation tracks, confirmed on file 2026-10-16** by `grn_dump.py` on all 4,993 files; the record layout follows openheilig (`formats/granny-grn.md`), the counts and checks were measured on our files.
  - `1205` → one `1200` → `1201` and `1203`, all three without own data in every file; `1203` → `1204` tracks.
    There is no clip length field: a clip lasts until its last key.
  - A track starts with `u32` channel (the 1-based transform channel, which the form's `0C02` gives exactly one bone), `u32` 0 and `u32` layout: 1 split (255,493 tracks), 0 interleaved (3,069).
  - Split: a 52-byte header: the layout words, then `u32` interpolation modes of the translation, the rotation and the scale/shear, `u32` key counts nt, nq, ns (at least 1 each), `u32` time selectors, always 0, 1, 2, and a zero word.
    Then the times of the three curves (`f32`), then the keys: `f32[3]` translations, `f32[4]` quaternions `x, y, z, w`, `f32[9]` scale/shear matrices; the record is exactly `52 + 16·nt + 20·nq + 40·ns` bytes.
    Modes: translation and rotation are 2 in every track; scale/shear is 0 in 1,262 tracks (with equal keys within 1e-6 in all but 4), 1 in 171,412, 2 in 82,815; mode 3 never occurs.
    Translation curves have at least 3 keys, rotation curves at least 3, scale/shear curves at least 2.
  - Interleaved: frames of 68 B after the 12-byte header, `f32` time, translation, rotation, scale/shear.
    3,013 tracks in 28 motions (26 of them interleaved throughout) carry 283,758 frames at 50, 30 or 60 per second; the 56 tracks of `CITIZEN_MAL.GRN`, a model, have no frames.
  - In every curve the times rise strictly from 0, in seconds.
    In a motion all curves end at the same time, except in `FX_E_IDLE_BH.GRN` and `FX_G_IDLE_BH.GRN`, whose bones each have a split and an interleaved track.
    Clips last 0.15–80.9 s, median 1.86 s.
  - 3,415 motions have exactly one track per bone.
    Rotation keys are control points, not unit quaternions: 614,981 of 6,639,940 are off unit length by more than 1e-3, and openheilig found that normalizing them before blending moves the pose by 0.2° (community-reported).
    627 rotation keys lie in the other hemisphere than the previous key, 1,318 than the key two before.
    70,347 scale/shear keys are off the identity, 7,183 off the diagonal; all are symmetric.
  - Implemented 2026-10-16: `opensacred_formats::grn::Animation` (`Grn::animation`); `Bone::channel` names the bone of a track.
    On the game files every animation reads, the counts above hold, and every record matches `grn_dump.py` by the fingerprint `animation_crc` that `--records` writes.
  - Interpolation (community-reported: openheilig derived it from the corpus and matched the game's own poses within 0.005°, `formats/granny-grn.md`).
    Mode 0 holds the key at or before the time, mode 1 blends neighbouring keys linearly, mode 2 is a quadratic B-spline whose keys are control points.
    On the span from key j to key j + 1, with the times ta ≤ tb < tc ≤ td of keys j − 1, j, j + 1, j + 2 (indices clamped to the curve): `f1 = (t − tb)/(tc − tb)`, `f2 = (t − ta)/(tc − ta)`, `g = f1 + f2 − f1·f2`, `h = f1·(t − tb)/(td − tb)`, and keys j − 1, j, j + 1 weigh `1 − g`, `g − h`, `h`.
    For evenly spaced keys this is the uniform quadratic B-spline; the curve starts at the first key with zero slope and ends at the last key, and every span is a quadratic polynomial in time.
    A rotation turns keys j − 1 and j into the hemisphere of key j + 1, blends, then normalizes.
    Where the turn of one span differs from the next one's, the curve jumps at the key between them: at 1,242 keys of the converted files (a motion counted once per file that carries it).
  - Facing: with its clips a model keeps the facing of its bind pose.
    `SERAPHIM.GRN` faces +X in the bind pose and in her idle, walk, run and attack clips (Blender renders, 2026-10-16), and she walks and runs in place: the tracks carry no forward motion of the root.
- Census of section contents: bones (`0506`) are in every file, 70,242 in model files (up to 354 per file).
  Meshes (`0601`) are in 1,568 of 1,572 model files, 2,555 in total; the four without meshes are `CAST02`, `FX_DWARF_KRIEGSRAUSCH`, `NQ08_2_HERBS02_01` and `WINDSTRAUCH`.
  23 motion files carry meshes as well: `GLADIATOR.GRN` and `WIZARD.GRN` are whole models filed as motions, the other 21 are special attacks and hit clips.
  Per-bone tracks (`1204`) are in 3,417 of 3,421 motion files, 258,502 in total; the four without tracks are `GLADIATOR.GRN`, `WIZARD.GRN`, `SD01_ACTIVATE.GRN` and `WILB_DYING_C.GRN`, which matches the four track-less clips openheilig reports.
  Two model files carry tracks: `WINDSTRAUCH.GRN` (4) and `CITIZEN_MAL.GRN` (56).

| Format element | What is known | Confidence |
|---|---|---|
| Container | 64-byte signature, root header, a table of 3 sections with CRC-32, chunk trees `{tag, offset, descendants}` with tags 0xCA5E0000…0xCA5EFFFF (layout below) | confirmed on every record |
| Skeleton | 68-byte bones, names through the form's bone channels and DataExtension `__ObjectName`, the bind pose is assembled by composing local transforms (there is no inverse-bind in the file); `__Root`, `Bip01 *`, `Bone_weapon_01/02` | confirmed on every record (layout, parents, names, the bind pose against the mesh bounds); scale/shear is symmetric |
| Meshes | `0601`: positions, normals and 0–2 texture-coordinate sets, each indexed on its own; triangles `0901` index positions and normals | confirmed on every record |
| Weights | vertex-major lists `0702`, 1–7 influences (above 4 in 16 meshes); form meshes `0C03` = 1-based mesh number with `0C09` → `0C0A` = local bone → skeleton bone with bounds; 1,010 files with a skinned mesh | confirmed on every record |
| Materials | `0D03 +4`: 1-based texture reference, or a `0D02` color; passes `0E02` with two sources `0E03` and triangles `0E06` carrying the corners' texture coordinates; name → `"%s.TGA"` in `texture.pak`, overridden by the index from `Items.pak +0x08` | confirmed on every record (layout); the texture lookup is community-reported |
| Clips | 52-byte header `1204`, `+8`: 0 = interleaved (12 + 68·N), 1 = split; 3 interpolation modes, time selectors always 0, 1, 2; every track of every file reads | confirmed on every record |
| Interpolation mode 2 | corner-cutting quadratic B-spline, keys = control points, **do not normalize** (error 0.214° → 0.00004° in openheilig) | medium–high (cross-checked against native poses < 0.005°) |
| Scale/shear | part of the pose: `T·R·SS`, then the parent | confirmed for the bind pose on every record (mesh bounds); for animated poses by openheilig's native oracle |
| Embedded VTex textures (`BIKi`) | 104 + 41 in model files, 22 + 88 in motion files; grn2glb decoder (≈500 lines, derived from FFmpeg `bink.c`, LGPL-2.1+); no DXT | confirmed on every record (counts); the decoder per grn2glb |
| Chunks without contents | `0C04` and `0C05` are always empty | confirmed on every record |

### 4.3 Runtime contract: what exactly Sacred.exe calls

The shipped `granny.dll` is a 2003-11-27 build with **101** exports (105 in open-grn = 101 + 4 internal `_Bink_*`; discrepancy resolved).
Confirmed by static analysis (`tools/analysis/pe/granny_args.py`): all **54** imports have call sites (an earlier count of 51 came from a linear sweep that missed `mov reg,[iat]; call reg`).
Key groups:

| Group | Functions (call sites) | What this requires from our own implementation |
|---|---|---|
| Animation parameters | `GetAnimationParameter` (175), `SetAnimationParameter` (72); Set indices 0–7, Get 0–3; the game sets param 3 = 10,000.0 ("eternal" loop), 4 = 1.0, 5 ∈ {0,1}, 6 = 10.0 or a variable, 7 ∈ {0,1} | ADSR envelope of control weights (start/end/scale/period/sustain/attack/release): the indices are confirmed by the binary, the formula is not |
| Bone state | `GetBoneState` (61; flags 1 and 3), `GetBoneTreeState` (2; 0x91, 0x40 = bbox), `GetBoneIndexFromName` (43) | world/local matrices, bbox |
| Rendering | `LockSequenceForRendering`/`GetRenderingStatesLeft`/`LockNextRenderingState`/`Unlock*` (25 each; flags 0x39 model, 0x21 shadow, 0x29/0x31/0x3B…) | "hand over the deformed vertex/normal/UV streams + indices + texture cookie": the IRenderer boundary |
| Controls | `AddControlledAnimation`, `AddControlledRebasedAnimation` (retarget onto another model), `SetSequenceAccumulating(1)` right after creation, `Remove/ReleaseControlledAnimation` | clip blending, root-motion accumulation |
| Poses | `OpenPose/ClosePose/SetPoseBoneName/SetPoseBoneScaleShearM` (5 each; bones `Bip01 Head`, `L/R UpperArm`, `L/R Thigh`), `AddControlledPose` with time FLT_MAX | a persistent pose on top of the animation; hypothesis: variation of creature proportions |
| Equipment | `WearSequence` (4), `StopWearingSequence`, `AttachSequence` (3; dof mask 0x1FF, 0x3F at 2 sites), `DetachSequence` (18) | clothing by bone names; weapons on the `Bone_weapon_0x` socket. **Confirmed** by code 2026-10-18: a held item's socket, `Bone_weapon_02` in the left hand and `Bone_weapon_01` in the right, goes onto the creature's hand, or its own socket of that name where its skeleton has one, and a shield's onto the forearm, with the mask `0x3F` ([docs/re/equipment.md](re/equipment.md)) |
| Other | `AdvanceTime` (once per frame), `SetFilesytem` (VFS on top of `models.pak`), `AllowTextureFormat`, `LockSequenceForRayIntersection` (picking), `OpenVersion("1.2b","win32","10-4-2000")` | scene clock, loader, picking against the skinned mesh |

IK is not imported, so it is not needed.
The original does skinning on the CPU twice per frame (model + shadow), which is a performance bottleneck (docs/05).

### 4.4 What community projects can do and what nobody can

| Need | openheilig (MIT/CC BY-SA) | grn2glb (MIT + LGPL VTex) | open-grn (MIT, drop-in DLL) | SacredEngineRemake (no license) |
|---|---|---|---|---|
| Container, skeleton, weights, materials, clips | yes, cross-checked on the corpus and against an oracle (apitrace/GDB on LGP Linux) | yes (1,571 models parse; 1 animation makes it into the GLB, deformation not checked) | yes | yes (managed parser) |
| Semantics of interpolation mode 2 | derived and cross-checked | no (raw ints) | its own blend, not cross-checked numerically | linear/slerp (wrong) |
| Controls/ADSR/period/accumulation | no (hard clip switching) | no | emulation (`control.cpp`, 9 parameters, linear ramps), **tests without asserts** | no |
| Wear/Attach/pose control | partially | no | partially (formulas not confirmed) | partially |
| VTex decoder | no (names only) | yes (LGPL-2.1+) | stub | no |

In short: the format is solved, **the runtime has been solved by nobody**. open-grn is a reference on the format and a list of hypotheses about the runtime; its code is not used, and the engine does not need the original DLL's internal layouts.

### 4.5 Conversion to glTF

* **A Granny 1.2b parser in Rust** in `opensacred-formats`, based on open format descriptions (the openheilig specification, CC BY-SA 4.0, as a source of facts) and using the MIT code of grn2glb and open-grn as a reference; anything borrowed is credited.
* **Export:** meshes, the Bip01 skeleton, weights, materials and texture references → `.glb`; animations → glTF clips with the same bone names, so that one clip fits all models with that skeleton.
* **Models, implemented 2026-10-16:** `opensacred-convert models` writes every record of both paks to `assets/models/<pak>/<index>-<name>.glb`: 1,572 models and 3,421 motions in 2.5 seconds, 1.4 GB; the conversion is `crates/opensacred-convert/src/gltf.rs`.
  A model's file carries the motions of its slots as animations, a motion's file its skeleton, its meshes if any and its own animation.
  - The skeleton becomes a node tree in bone order; every mesh with a drawn pass becomes a skinned mesh at the scene's top level with one primitive per pass, in pass order.
    A bone whose scale/shear is not diagonal, in the bind pose or in a track that plays on it (1,202 bones), becomes two nodes, since glTF scales along a node's own axes only: with `SS = U · D · Uᵀ` (`U` a rotation, `D` diagonal) an outer node `<name> scale/shear` holds the translation, the rotation `R · U` and the scale `D`, and the bone's own node under it the rotation `Uᵀ`; together they are `T · R · SS` exactly.
    A glTF vertex is a distinct corner (position, normal, texture coordinates, color); the weights follow the position.
    A rigid mesh is skinned to its only bone with weight 1.
    The skin of a mesh lists its form mesh's bones, so `JOINTS_0` holds the local bones of §4.2 unchanged.
  - Frame: the models are Z-up, as exported from 3ds Max, and every position, normal and transform turns by −90° about X, `(x, y, z) → (x, z, −y)`: a rotation, so the winding stays counter-clockwise, and Max's front (−Y) becomes glTF's front (+Z).
  - Scale (2026-10-21, #443): a model's own scale (§4.1) multiplies, about its origin, the meshes' positions and the bounds of their bones, the bind pose's translations and the translations of the clips of its slots, as `GrannyTransformModel` and `GrannyTransformAnimation` do; the bones' turns and scale/shear stay, and the inverse bind matrices, computed from the bind pose, follow.
    A motion's own file keeps the scale 1, the client scaling a motion by the model that plays it.
    Facing in the bind pose is not uniform: of the 875 models with a biped's feet (`Bip01 L/R Foot → Toe0`), 719 face −Y, 148 face +X (among them `SERAPHIM.GRN`, `GLADIATOR.GRN` and the armour sets), 4 face +Y and 4 −X; in the game the animations turn them (to be checked with the clips).
    Units are kept: a hero is about 74 units tall.
  - Textures: a texture refers to the PNG of `opensacred-convert textures` by URI, found as the game finds it (`"%s.TGA"` without regard to case, of several records with the name the last; community-reported), whether the file embeds an image or not.
    On the game files this picks the records openheilig observed in the game (`DWARF` → 7986, `DAEMONIA_1024` → 7974, `DRYADE_SCOUT_ARMS` → 6890).
    No texture name contains a dot.
    V is not flipped: in Blender the textures sit as in the game.
    682 texture materials stay without an image: names that are not in `texture.pak` and have no embedded image, mostly equipment, which the game textures by the index of `Items.pak +0x08` (community-reported).
  - Materials: a texture material is a glTF `baseColorTexture`, a color material a `baseColorFactor`, both with metallic 0 and roughness 1 for now.
    A color set (only `EDLST_RUND_GESCHL_KLEIN.GRN` has one that is not white) becomes `COLOR_0`; constant white colors are left out.
  - Where glTF cannot hold the original (`docs/decisions/`): zero normals take their triangle's normal (2,476 corners); vertices with more than 4 influences keep the largest 4, renormalized (60 vertices); meshes drawn only with unnamed color materials, helper geometry, are left out (179).
  - Verification: glTF-Validator 2.0.0-dev.3.10 finds no error and no warning in all 4,993 files (infos: empty leaf nodes, external URIs, unused objects, two images with non-power-of-two sizes).
    `SERAPHIM.GRN` draws as the game does in openheilig's trace: six primitives of 144, 358, 612, 390, 280 and 270 triangles with the hair, body, arms, legs, head and boots textures, 2,054 triangles in all; a test checks this and runs the validator.
    Renders in Blender 5.2 of `SERAPHIM`, `GLADIATOR`, `DWARF`, `WOLF` (sheared bones), `BEAR`, `ELVE_SORCESS` (helper meshes left out), armour, gloves, a sword and a gem show the meshes whole and the textures in place.
* **Animations, implemented 2026-10-16** (`crates/opensacred-convert/src/animation.rs`): a clip becomes a glTF animation named after its motion record without `.GRN`; in a model's file its `extras.slots` lists the motion slots that play it.
  A track plays on the model's bone with the same chain of names from the root, else on the only bone with its name; tracks of bones the model lacks are left out of that file (34,062, all helpers: `Bip01 Footsteps`, camera and light targets, `*Nub` ends; no model–motion pair binds less than half of its tracks).
  - A quadratic curve becomes a glTF cubic spline with a keyframe at every key: each span is a quadratic polynomial, so the values and the slopes on both sides of each key carry a translation or a scale curve exactly.
    A rotation keyframe holds the normalized curve and its slope; where the spline strays from the original by more than 0.001°, a span gets keyframes in its middle (9% more keyframes).
    A test checks every curve of the game: translations within a few f32 ulps, rotations within 0.0025° (curves with a jump excepted).
    A linear curve becomes a linear sampler, a mode 0 curve a step sampler, interleaved frames linear samplers; a curve whose keys are all equal two keyframes at its ends.
  - A track whose scale/shear has shear (1,532) is sampled at its keys and at least every 1/60 s: the outer node's rotation and scale and the bone's own rotation, with the eigenvectors kept continuous from sample to sample.
  - A clip gives every bone that another clip of the same file moves constant channels of its bind pose (21,384 bones), because Granny poses unmoved bones in their bind pose and an engine keeps what the last clip left.
  - Results: 7,185 animations, 507,802 bound tracks, 21.1 million keyframes.
    Renders in Blender 5.2 of the clips of `SERAPHIM` (idle, walk, run, attack), `WOLF` (walk, attack, idle; sheared), `DWARF` (attack) and `BEAR` (walk) show natural motion; before the two-node shear the wolf's attack tore its head and neck apart.
* **Embedded textures, implemented 2026-10-17** (`crates/opensacred-convert/src/embedded.rs`): a texture of any format refers to the image of `texture.pak` with its name, as the game binds it (244 of the 270 embedded ones); the other 26 are decoded and stored in the `.glb` as PNG images: raw RGBX and RGBA as they are, VTex by FFmpeg run as an external program (`FFMPEG`, else `ffmpeg` from `PATH`), which turns YUV into RGB by BT.601 with the limited range, as grn2glb does.
  No Bink code is in the converter; grn2glb's decoder (LGPL-2.1+, from FFmpeg) was not needed.
  A test decodes all 270 embedded images; Blender renders of `CITIZEN_FEM`, `ZOMBIE` (embedded) and `BARBARIAN` (bound by name) show them textured.
* **The original's animation runtime** (ADSR envelopes, period, root-motion accumulation, `AddControlledPose` for the head and arms) is not part of the files and is carried over approximately: into a Bevy animation graph with blend durations from `MOTIONS.PAK`; exactness is not required.
* **Verification:** glTF-Validator on all files and inspection in Blender (done, above); a screenshot test in the game.
* **Estimate:** models 15–30 agent-hours, animations 10–20 (milestones M1 and M2).

### 4.6 Defects the community reports

Checked for #358 on the converted files and the GOG 2.28 paks on 2026-10-21.
The reports are **community-reported** (Sacred ReBorn 4.2 and 4.4, "BUGFIXES (SACRED)"), and they name no files, so the records below are those the data points to.
The measures: holes as boundary edges of a mesh whose corners are welded by position, sorted into its connected surfaces; degenerate and flipped triangles; each vertex's bone against the nearest bone in the bind pose; the edges' stretch over the model's own clips, sampled; and, for an item a hero wears, its vertices placed by the hero's bones of the same names times the item's inverse bind matrices, over the hero's clips (a wear by bone names, Granny's `WearSequence`, §4.3; **hypothesis** that the client wears these items so).

- **The dwarf's combat gauntlets shown wrongly**: **confirmed** on the data that the gauntlets do not fit the dwarf's skeleton, in two ways.
  The combat gauntlets are the weapons of items 4067 to 4072 (category 5), models 600 to 605 (`GLAD_GLOVES_*.GRN`), a mesh per hand skinned to `Bip01 L/R Forearm` and `Hand` (a few positions to `Finger0`), 14 or 15 positions of each on the forearm; no armour switch gives the dwarf a model of its own (`tables/armour_switches.ron`).
  Their files, like the arm gauntlets 606 to 608 and the fighting gloves 587 and 1439, spell the forearms `Forearm`, and `DWARF.GRN` (722) is the only hero skeleton that spells them `ForeArm` (§4.2), so a wear by names finds no forearm on the dwarf.
  The gauntlets' own rig has a forearm 11.06 units long, the gladiator 9.75 and the dwarf 8.53: worn in the bind pose, edges from the cuff to the hand shrink to 0.56 of their length on the gladiator and to 0.34 on the dwarf, and the dwarf's forearm, 9 to 10 units across against the gladiator's 6 to 7, is wider than the cuff (6 to 7).
  The converter already writes both spellings as `Forearm` (#355), so the gauntlets bind on the dwarf in `opensacred-game`; the fit would need meshes reshaped for the dwarf, not a narrow repair (**hypothesis** that ReBorn did so).
- **One amulet shown wrongly**: **confirmed** on the data for `DWARF_AM_ERDE.GRN` (792), the only one of the 150 amulet models (`*_AM_*`, 17 for the daemon and 19 for each other class) whose mesh follows a bone that its class's hero skeleton lacks.
  Its mesh `Amulett_erd` (46 positions) is rigid on a bone `Amulett_erd` straight under `__Root`, where the other 18 dwarf amulets follow `Bip01 Spine3`; worn by names, it finds no bone on the dwarf.
  Its skeleton is also the only one of the 19 without the `Root` that `DWARF.GRN` has, which the other 18 have to the bit (a translation of (−0.042, −0.335, 0) under `__Root`, a turn of 1.5e-5°); its other bones equal theirs, but `Bip01` hangs straight under `__Root` with the transform it has under `Root`, so the file's bind pose lies 0.34 units off theirs (**confirmed** on the files).
  Its mesh lies where theirs lie, not where its bind pose moved: the map of the elf's fire amulet (812) or the gladiator's (831) onto the dwarf's (793), each in its file's `Bip01 Spine3` frame, takes that class's earth amulet (811, 830) to within 0.14 and 0.09 units of 792's mesh in the frame of the siblings' `Bip01 Spine3`, and to 0.46 and 0.42 in the frame of 792's own.
  The converter repairs it (#398, docs/decisions/2026-10-21-earth-amulet.md): it gives the skeleton the siblings' `Root` as the parent of `Bip01` and binds the 46 positions rigidly to `Bip01 Spine3`, positions unchanged.
  The inverse bind matrix of the amulet's joint is then its siblings' within 1.5e-6, and worn on the dwarf's bind pose by names, its centre lies 0.06 units from the fire amulet's (by the file's own bind pose it would lie 0.36 units away).
  `DWARF_AM_SERAPHIM.GRN` (798) follows `Dummy01`, a helper under `Bip01 Head` that the dwarf has at the same place, so it moves with the head rather than the chest (**hypothesis** whether that shows).
- **The cannons shown wrongly**: **confirmed** on the data for `Dwarf_leather_cannon.grn` (1415, item 3015).
  Five of the 43 positions of its strap `shoulder_leather01` (3, 17, 26, 27 and 32), at shoulder height, are wholly on `Bip01 Pelvis`, 17 to 23 units below them, among positions on `Bip01 L Clavicle` and `Bip01 Neck`: the nearest position on another bone lies 1.1 to 4.2 units from each, on `Bip01 L Clavicle` for three and on `Bip01 Neck` for two, and 10 of the 13 sides from them to other bones' positions run to `Bip01 L Clavicle`.
  Worn on the dwarf, 49 of the 204 sides of the strap's 68 triangles stretch more than twice in the dwarf's clips, up to 22 times in `DWAR_ATTACK_SPECIAL01`.
  The converter repairs it (#399, docs/decisions/2026-10-21-cannon-strap.md): it weights those five positions wholly on `Bip01 L Clavicle`, the positions unchanged.
  Measured with 16 samples a clip (which find the 49 sides and 23 times before), 32 sides then stretch more than twice, up to 13 times in `DWAR_SPECIAL_HHIT_2H_AXT` and 8.8 in `DWAR_ATTACK_1H_A`, all between the strap's positions on `Bip01 Neck` and on `Bip01 L Clavicle` (**confirmed** on the converted files).
  That is the strap's own split between the two bones, like the seams of the dwarf's own mesh, whose sides stretch up to 7.2 times in `DWAR_RUN_2H_AXT` and `DWAR_ATTACK_1H_A`; smoothing it would be a new weighting, not a repair of the data.
  The other two cannons, `DWARF_CANNON.GRN` (723, items 1700 and 3016) and `dwarf_platearmor_cannon.grn` (725, item 3095), hang on the helpers `null_cannon_base`, `null_cannon_mover` and `null_rucksack`, whose bind pose under `Bip01 Spine3` is the dwarf's to 0.01 unit and 0.1°, and none of their edges stretches past 3.1 times (723's rucksack in `DWAR_DYING_A`); 725 spells the forearm `Forearm`, but none of its meshes uses it.
  What else the community saw wrong on the cannons is not in the data (**hypothesis**).
- **A small hole in the blacksmith's model**: **not in the geometry**, **confirmed** on the data.
  The four meshes of `blacksmith.grn` (1305) equal those of `WARRIOR_BLONDE.GRN` (434) and `WARRIOR_BROWN.GRN` (435) in positions, triangles, texture coordinates and weights, and those of `VALOR.GRN` (1338) and `VALORGHOST.GRN` (432) in all but the positions, which face another way.
  Their boundaries are openings of the design (the neck, the waist and the legs' ends that another mesh covers, two-triangle straps on the boots, the boots' cuffs) and two slits under 0.03 units wide (the right boot's rim against the trouser leg and a seam at the front of the waist) with the same weights on both sides, which no clip opens.
  The blacksmith differs only by its textures (8201 to 8207), and the one with transparent texels is the apron's, record 8202 (`blacksmith_belt`): two areas of alpha 0 lie under 16 of the 154 triangles of `warrior_cloth` (406 of the 3,900 texels it covers), the flaps hanging below the belt, where the other warriors' record 6825 (`warrior_belt`) is opaque.
  Valor's belt (8329) has transparent texels under 46 triangles too, its flaps' tips among them, a design of ragged flaps; on the blacksmith the areas cut through the flaps' middle, and a render of the converted model with the texture's alpha (Blender, 2026-10-21) shows the front flap mostly gone, a few slivers left.
  That is the hole, **confirmed** in the running original 2026-10-22 (#516): it tests a model's texels by their alpha, as the sprites', and shows the blacksmith's front flap cut to a strip under the belt (docs/re/drawing.md, "The models").
  The texture's colour runs on unbroken under the two strokes (1,176 texels of alpha 0, the rest 255; the strokes' mean colour equals the opaque texels'), as though they were painted into the alpha alone (2026-10-22, #401).
  The converter repairs it (docs/decisions/2026-10-22-blacksmith-apron.md): `opensacred-convert textures` writes record 8202 opaque, its colours unchanged; a render of the model with its textures' alpha tested at `0x40` (Blender, 2026-10-22) then shows the front flap whole down to the thighs, where the original's texture leaves a strip under the belt.
  `opensacred-game` drew every model opaque whatever its textures' alpha before #516, so it showed no hole before the repair either; it tests the alpha since.
- **The Blooded Terror mount broken**: **not found in the data**.
  The models are `Blutschrecke.grn` (1187, the untamed creature 200), `BlutschreckeSattel.grn` (1188, the saddled 201) and its riders `StachelalbaufBlutschrecke.grn` (1292, creature 199) and `…01.grn` (1293, 340); the saddled mesh is the untamed one's 1,244 triangles plus the saddle's 188, and the riders' add the rider's 1,785.
  Their boundaries are the design's, and their clips' tracks find every bone but helpers such as the `…Nub` ends.
  No vertex of the two mounts lies far from its bone; in the riders' models the saddle's grip, on the mount's root `Bone01`, lies by the rider's left hand, as a held grip would.
  The saddled body's texture differs from the untamed one's by the saddle's straps painted on it.
  What the community saw broken is a **hypothesis** to be checked in the original.
- **A defect at the start of the large dragons' tails**: **confirmed** on the data as the weights of the tail's root.
  The eight dragons (1193 to 1200, `dragon*.grn`) share one mesh `Dragon` (3,120 vertices, 3,064 triangles) with the same positions, triangles, texture coordinates and weights, mostly one bone per vertex.
  At the tail's root, the vertices of its sides and underside are wholly on `Bip01 L Thigh` or `Bip01 R Thigh` next to vertices wholly on `Bip01 Tail`, such as the edge from (±21.5, 88.5, −88.2) to (±20.8, 92.2, −92.2), 5.5 units long.
  In each of the three sets of clips (`DRGN_`, `DRGI_`, `DRGU_`) these are the most stretched edges of the whole mesh: 4.7 times in the walk, where the next elsewhere reach 2.6, and 10.5 times in the dying clip.
  The surface is closed there: the body's only boundaries are slits along the four legs and the mouth's opening, and the mesh's 16 three-vertex loops are the open bases of spikes on the head and the tail's last segments.
  The converter repairs it (#400, docs/decisions/2026-10-21-dragon-tails.md) by blending the tail into the thighs over two rings of the mesh's 1,634 positions.
  The six positions wholly on a thigh that share a side with a position wholly on `Bip01 Tail` take 2/3 of `Bip01 Tail` and 1/3 of their thigh, and the six wholly on a thigh that share a side with those take 1/3 and 2/3.
  The rings hold four positions of the left side and two of the right each, since the right's mirror image of the left's position 225, wholly on the tail, is half on its thigh already.
  Measured with 48 samples a clip, the sides from the twelve positions then stretch at most 2.26 times in the walk and 3.94 in the dying clip (4.66 and 10.63 before), below the most stretched sides of the rest of the mesh in the same clips (2.57, and 4.54 at the belly's seam between `Bip01 Spine` and a thigh), and at most 3.94 over all the clips, where the rest reach 7.32 (the crest of the head in the first attack); the three sets of clips give the same numbers (**confirmed** on the converted files).
  One ring alone leaves 4.3 to 6.5 times in the dying clip, with 2/3, 1/2 or all of the tail; taking the right side's mirror images of the left's positions as well gives the same peaks with more sides stretched past three times (9 against 5).
  Which weights the community's fix gave is not known (**hypothesis** that this is the defect it reports).

`opensacred-convert models` repairs the named records as it writes them, each repair a `bug` of its own and recorded as a decision (CLAUDE.md, "Code conventions"), and counts them in its summary; a named record whose data is not what its repair expects is written as it is, with a warning.
The earth amulet, the cannon's strap and the dragons' tails are repaired; the other repairs above are not made yet.

## 5. World

### 5.1 The sector grid and sectors.keyx

`WLK` v5, header `6050, 100, 100, −18432, 112152`; 6,050 × 768 B (confirmed on file by `tools/analysis/world/world_dump.py`).
Sector key: `+0x20 u32 sectorIndex = y·100 + x`, `+0x24 u32` the sector's id (Static.PAK `+0x0C` refers to it; 2579 for the starting sector (50, 39)), `+0x3C…+0x48` bounds in world units with a step of 3,434 (sector = 3,434 units, cell ≈ 53.7 units; exactly 24·√5 = 53.66563, the edge of a cell's diamond in screen pixels, as `CreateNPC` (`0x482E9D`) puts a creature at `(cell + 0.5) · 53.66563`; the bounds are that times 64, truncated), `+0x50` chunk table `(offset, size, type)` × 3: `(o, 32, 1)`, `(o + 32, 131072, 6)`, `(o + 131104, n, 0)`, at the offsets of an uncompressed world file with a 0x100 header, `+0xEC u32 offsetInWldx`, `+0xF0 u32 csize`, `+0x108 u32 usize`; `+0x1C0…` parameters and `+0x290…` environment (colors, the numbers 25/30/100; hypothesis: `cSectorEnvironment`); `+0x2C0 u8` the sector's area (56 values; the region music's handler turns it into a region, docs/re/sound.md) and `+0x2D8 u32` the sound id of the sector's music (`MUSIC_WOOD01` in the starting sector; 0 in 1,892 sectors; confirmed 2026-10-18 by the client's region music, docs/re/sound.md, and on every record by `world_dump.py`), both in `world.ron` since then; `+0x1ED u8` a bit mask and from `+0x1EE` four entries of 28 B, the sector a dungeon when an entry whose bit is set starts with the u16 4 (the client's dungeon test `0x622E20`, docs/re/daynight.md; 2,428 sectors, among them 541 of the 562 of the dungeons' music; hypothesis: the record the client reads from `+0x1E9`), in `world.ron` as `dungeon` since 2026-10-18; `+0x2E0` and `+0x2E1` the liquid materials (0..13, §5.2) of the sector's `WATERA` (`0x90`) and `WATERB` (`0xA0`) cells (openheilig named them the materials of the water and the lava cells; 0..13 in every record): measured 2026-10-17 over all sectors, the `0x90` cells take a lava material of the first slot in 77 sectors (C_LAVA, E_LAVA: 93,000 cells) and the `0xA0` cells mostly a water of the second (B_WATER, G_WATER, D_WATER; C_LAVA in 3,160 cells), so the two natures are two liquid slots, not water and lava; the drawing pass confirms it ([docs/re/liquids.md](re/liquids.md)).
The 100×100 occupancy map shows a continent and separate dungeon "islands".
The original keeps 3×3 sectors loaded (`cSector/cSectorChunk`); walking rows of tiles beyond this window is a known source of crashes at high resolutions (according to sacred-bild).

### 5.2 A sector in sectors.wldx: the 32 B floor cell

**Confirmed on file 2026-10-17** by `tools/analysis/world/world_dump.py` on every sector, correcting the earlier reading: a decompressed sector is the three chunks of keyx, **the 32 B name** `"Sector x,y"` with a NUL, filled with MSVC's `0xCD` (275 sectors carry another sector's name or none, so the keyx key is the position), **the 4,096 cells of 32 B from `+0x20`**, row by row (`cellIndex = r·64 + c`; 24,780,800 cells in the game), and from `+0x20020` the third chunk of `n` items up to the inflated size (292 B in the sectors with one item).
**The third chunk is the sector's layers**, confirmed on every sector 2026-10-18 by `world_dump.py` (and by the client's cell lookup, docs/re/movement.md): `n` (keyx `+0x6C`) headers of 36 B, `u32 x, u32 y` the global cell of the layer's first cell, `u16 width, u16 height`, `u32 kind` (6 in all), `u32` the offset of its cells in the stream, `u32` their size (`width · height · 32`), 12 zero bytes; the first header is the ground's, the sector's own cells, at the sector's origin with size 0; then the cells of the other layers, each right after the last, and 256 bytes of unknown use.
The 1,870 other layers (in 361 sectors, up to 57 in one) are the buildings' insides: grids within their sector (up to 56 × 57 cells) of the cells' layout with only `+0x18…+0x1F` set (the second heights, 10, 15, 20, … in them, which the creatures stand on inside; the offsets; the flags, 0 or 4 a door; the ground byte, the walls 1 or 2 in its low nibble) but two cells' first static.
In the start sector the monastery's inside is a layer of 34 × 30 cells from (3213, 2502); in sector 50,40 a house's of 12 × 12 from (3249, 2568), whose door lies at its door on the ground.
An earlier reading took the cells from `+0x120`, eight cells late: with the cells from `+0x20` the shared corners of neighbouring cells hold the same height everywhere but 2,082 of 48,787,200 corners (ledges and edits), against 98.8% of the non-zero corners from `+0x120`, and the cells' chains reach every record of Static.PAK and Floor.PAK (§5.5).
Cell fields (the meanings per openheilig `formats/world-sectors.md` where marked; the early statistics per `tools/analysis/world/sector_stats.py`, which reads the cells eight cells off):

| Offset | Type | Value | Use in code | Confidence |
|---|---|---|---|---|
| +0x00 | u32 | tile: index into `tiles.pak` (90,132 tiles, so above 16 bits in 5% of cells); the tile names its `texture.pak` sheet and diamond 0..17 | `renderTileRow` 0x62AE90 | confirmed on every cell (range) |
| +0x04 | u32 | the first static of the cell's chain in Static.PAK, 0 none (openheilig) | — | confirmed on every cell (§5.5) |
| +0x08 | u32 | 0 in every cell: at run time the head of the cell's chain of dynamic objects, the id of the first, each object's `+0x30` the next's (**confirmed** by code 2026-10-19, docs/re/movement.md, "The objects in the way"; openheilig took it for a list removed before release) | the world pass draws the chain (`0x62C306`); the test of a step reads it (`0x4F7E10`) | confirmed on every cell (0) and by code (use) |
| +0x0C | u32 | the first floor overlay of the cell's chain in Floor.PAK, 0 none | copy of the quad into the layer list (cap 0x6D5) | confirmed on every cell (§5.5) |
| +0x10…+0x13 | int8[4] | **corner heights** in the order left, top, right, bottom; multiples of 5, range −128…+125; all zeros in 78.4% of cells | `v.y -= h / zoom` | confirmed by code and statistics |
| +0x14…+0x17 | u8[4] | **corner light** L,T,R,B: 255 (93.3%), 235, 215, … step 20 | gray diffuse `0xFF·L·L·L` | confirmed |
| +0x18…+0x1B | int8[4] | 0 (91.5%), ±1…±6 per corner: a second corner height, the height a creature stands at, in steps of 2.5 units of the models' height (below) | not read in quad filling; `0x636520`, a creature's height; `0x639080`, the footstep's nature on a liquid | confirmed by code 2026-10-19 |
| +0x1C, +0x1D | int8, int8 | 0 (99.6%), ±1…±6: the offsets to the cell of the building's anchor, whose chain holds the building's object (openheilig's "offsets of the parent object"; **confirmed** by code 2026-10-19, #104, docs/re/movement.md) | `movsx` → `fild` in the object pass; `0x6354D0`, the building of a position | confirmed by code |
| +0x1E | u8 | bit 0 structure, bit 1 a settlement's ground (openheilig's "room interior"; below), bit 2 door (openheilig); no higher bit in any cell. In the starting sector 510 cells have bits 0 and 1 (the monastery's footprint, which the hero can walk into), 63 bit 0 alone (its edges, a few single cells), none bit 2; so the bits are no walkability map. The client posts an engine event when the hero's cell gains or loses bit 1 (`0x594011`: `0x6770E0(9, 0xCC, …)` with 1 or 0, unless the hero's sector is a dungeon, `0x63D180`); not for the roof, which the entrances and exits of the low nibble of `+0x1F` switch (docs/re/movement.md, confirmed by code 2026-10-18), and what the event does is not followed. Bit 1 covers a settlement's ground, not a room's (**confirmed** on the sector files 2026-10-20, #374): all 4,096 cells of 52,39 (the garrison by the guarded bridge), 3,195 of 53,40 (the eastern town's square, streets and yards), 3,993 of 48,35 (Hedgenton), and in the start sector only the monastery's footprint, all with bit 0; the village of 50,40 has none. The spawning keeps the monsters off it (docs/re/spawning.md) and the children play only off it (docs/re/ai.md). **Confirmed** by code 2026-10-18 (`0x636C10`, the client's test of a cell for walking, docs/re/movement.md): bit 3 makes a cell unwalkable; a cell with bit 2, a door, is walkable by its trigger's state (bit 0 of the word at `+0x0A` of the building states' table), and by the low nibble below where the trigger has none; in sector 50,40, three such cells are the door of a house and the gate of the cemetery | bit tests | bits confirmed on every cell |
| +0x1F | u8 | high nibble = the cell's **nature** (table below); low nibble 1 (64%), 0 (25%), 2 (11%), rarely 3, 5, 7, 9, 10, 15. **Confirmed** by code 2026-10-18 (`0x636C10`): a low nibble of 1 or 2 makes the cell unwalkable. By the start sectors' maps, 1 marks the mass of a building (the monastery, the houses of sector 50,40: the whole footprint but the door) and some walls, 2 water (the pond), fences, walls and the cemetery's railing. **Confirmed** by code 2026-10-18 (`0x5FCCB0`, docs/re/movement.md): an object that steps onto a cell changes its layer by the nibble, 9 an entrance onto a building's inside (973 cells of the ground, all of a building), 10 an exit onto the ground (2,545), 5 to 8 one or two layers up or down (in the inside layers: 214, 237, 3 and 22), 3 a teleport (17 on the ground) | `& 0xF0 ∈ {0x90, 0xA0}` → the separate liquid pass; `& 0xF ∈ {1, 2}` → not walkable; `& 0xF` 3, 5…10 → a change of layer | names confirmed by code, liquids confirmed; walking and the changes of layer confirmed by code; 15 unknown |

Correction to an earlier reading: the `+0x10` bytes are not "blend weights" (0xFB = −5 = height −5 px); this is exactly where the heightmap was found.

**Natures.**
A debug line of the client (`Nature[%s]`, next to `RegionID[%d]` and `sector[%d,%d] layers[%d]`) names the nature byte through a switch at `0x6CD702` (`byte − 0x10`, byte table `0x6CFFA4`, jump table `0x6CFF64`); a byte with a low nibble other than 0 prints `UNKNOWN`.
Confirmed by code 2026-10-17; the shares are the high nibbles of all 24.8 million cells, and the start sectors match them by eye (meadows `GRASS`, the darker patches `WALD`, paved paths `STONEROAD`, the monastery's floors `IND-STONE`, the pond and the river `WATERA`, a field `KIESROAD`):

| Nibble | Name | Cells | | Nibble | Name | Cells |
|---|---|---|---|---|---|---|
| 0 | (none) | 2.5% | | 8 | `DUNGEON` | 13.3% |
| 1 | `DESERT` | 4.3% | | 9 | `WATERA` | 7.7% |
| 2 | `ARCTIC` | 1.8% | | 10 | `WATERB` | 0.08% |
| 3 | `WALD` (forest) | 13.1% | | 11 | `STONEROAD` | 8.1% |
| 4 | `GRASS` | 16.5% | | 12 | `KIESROAD` (gravel road) | 4.2% |
| 5 | `SUMPF` (swamp) | 0.01% | | 13 | `IND-STONE` (indoor stone) | 24.4% |
| 6 | `LAVA` | 0.25% | | 14 | `IND-WOOD` (indoor wood) | 0.7% |
| 7 | `BRIDGE` | 3 per 100,000 | | 15 | `KIES` (gravel) | 3.2% |

The footstep profiles' groups are another list: 0 environment, 1 grass, 2 water, 3 snow, 4 swamp, 5 stone, 6 sand, 7 wood (the client's `SND_*` names, [docs/re/sound.md](re/sound.md)); the client plays a footstep's group by the nature (a switch at `0x68EDDA`: the deserts and gravels sand, the stone roads, floors, dungeons, bridges and lava stone, `IND-WOOD` wood, the rest grass), and it also picks the ambient loop and the footprint texture by the nature ([docs/re/sound.md](re/sound.md)).
Under a liquid the first corner heights are the bed (−10 in most of the start's pond) and the second (`+0x18`) mostly 0 or a little below (−1, −2).
The surface lies flat at height 0 and takes its corners' alphas from the bed's heights (**confirmed** by code 2026-10-19, `0x62DD00`, docs/re/liquids.md); the liquids' pass does not read the second heights.

**The second heights.**
The client stands a creature on the second corner heights, not on the first (**confirmed** by code 2026-10-19, [docs/re/movement.md](re/movement.md), "The height"): a step of them is 2.5 units of the models' height, 4.29 pixels at zoom 1, where a step of the first is a pixel.
Measured on the sector files 2026-10-19 (#141): of the 6,043,081 walkable cells of the ground off the liquids, 75.7% have second heights within 2.5 pixels of their first on average over the corners and 99.2% within 12.5; the rest are bridges and walkways above the drawn ground (up to 535 pixels) and a few hollows.
Of the 194,981 walkable liquid cells, 12,473 have second heights above 0 that differ from their first, bridges over the water, and the others mostly 0 or a little below, the water's surface rather than its bed.
The start's bridge to the pond's island is two rows of walkable cells, (3225…3236, 2546…2547), over the water of the natures `BRIDGE` and `WATERA` with the low nibble 0 and on the landing by the road `IND-WOOD`; its corners' second heights run 8, 5, 3, 6, 12, 17, 18, 17, 17, 17, 12, 6, 3, 0 from the island to the road, up to 77 pixels above the bed's −10.
Of the 187,787 walkable cells of the inside layers, 87.4% have second heights within 2.5 pixels of the ground's first heights there.

**Liquid materials.**
The client fills a table of 14 materials at `0x4189DF` (records of 0xD8 B from `this + 0x91E8`: the frames' texture handles, `+0xC8` the frame count, `+0xCC` a byte, `+0xD0` an int, `+0xD4` a byte), each from a series of `texture.pak` frames `<SERIES>%.2d.TGA` (128 × 128, opaque but for three lavas, tileable); a sector's two slots (§5.1) index it.
Confirmed by code 2026-10-17; the meaning of `+0xCC` and `+0xD0` by code 2026-10-19 (#103, docs/re/liquids.md):

| Material | Series | Frames | `+0xCC` | `+0xD0` | `+0xD4` |
|---|---|---|---|---|---|
| 0, 1 | `B_WATER` | 50 | 1 | −12 | 0 |
| 2, 3 | `C_WATER`, `D_WATER` | 50 | 1 | −12 | 0 |
| 4, 5 | `A_LAVA`, `B_LAVA` | 50 | 0 | −255 | 1 |
| 6, 7 | `C_LAVA`, `A_SCHWEFEL` (sulphur) | 20 | 0 | −255 | 1 |
| 8 | `D_LAVA` | 50 | 0 | −255 | 1 |
| 9 | `E_WATER` (murky, a swamp's) | 50 | 0 | −255 | 0 |
| 10 | `F_WATER` | 50 | 1 | −24 | 0 |
| 11 | `G_WATER` | 50 | 1 | −12 | 0 |
| 12 | `E_LAVA` | 50 | 0 | −255 | 1 |
| 13 | `B_WATER` | 50 | 0 | −12 | 0 |

`+0xD0` is the factor of a surface's corner's alpha: the bed's height there times it, held to 0…255, so a water of −12 is opaque from a bed 22 pixels deep and a lava of −255 over any bed below 0.
`+0xCC` gives a clear water its second layer at `DETAILLEVEL` 2, added over the bed under its surface (#118).
Hypothesis: `+0xD4` marks the glowing ones, the lavas and the sulphur; what the client does with it is not read.
The `CAUST00`… frames (64 × 64, grey) are loaded by the world view's constructor and never bound; the second layer shows the water's own frames (**confirmed** by code 2026-10-19, docs/re/liquids.md).
How the client picks a cell's material, the frame of the moment (every material cycles in 2,048 ms) and the mapping (a frame over 4 × 4 cells) is in [docs/re/liquids.md](re/liquids.md).

Three lavas' frames hold texels that are not opaque, **confirmed** on every frame 2026-10-20 (#347): all 50 of `A_LAVA` (600 texels, row 0, columns 109–127), 26 of the 50 of `D_LAVA` (141, row 0, columns 114–127) and all 50 of `E_LAVA` (1,394: row 0, columns 64–127, and one in row 2 of each frame).
Their values look like uninitialised memory of the tool that made them (ARGB4444 `0x0CCC`, `0xCC0C`, `0xC310`, …: grey with alpha 0, purple with alpha 204, others with alphas 17 to 204), and the surfaces' blending by the texture's alpha (docs/re/liquids.md) shows them as grey and purple streaks along one edge of every 4 × 4 cells; every other texel of the liquids' frames is opaque, and `opensacred_formats::tex` decodes them as they are (it matches `tex_decode.py`).
`opensacred-convert textures` repairs these frames as it writes them: every texel that is not opaque takes the colour of the nearest opaque texel of its column, the one above on a tie (docs/decisions/2026-10-20-lava-streaks.md).

### 5.3 Heights and the render contract

Confirmed by static analysis: the data is a full heightfield with shared vertices (bottom(r,c) = left(r,c+1) = right(r+1,c) = top(r+1,c+1) in 99.77–99.97% of cases; the remainder are ledges and manual edits), but the engine draws a cell as a pretransformed quad, FVF 0x244 (XYZRHW + DIFFUSE + 2 UV, 36 B/vertex), with immediates `z = 0.9f`, `rhw = 1.0f`, vertex offsets (−48.2, 0), (0, −24.2), (0, 24.2), (48.2, 0)/zoom and each vertex's screen Y shifted by `−h/zoom` (1 height unit = 1 px at zoom 1).
The ground enters the Z-buffer as the plane z = 0.9; the "manual" depth of building sprites is given by the formula `depth = (2·zoom·(200 − screenY·400/768) − height/sin(atan 0.5) + bias) / 3100 + 0.002` on the same normalized scale as the orthographic projection of the models, `ortho(±267·zoom, ±200·zoom, −600, 2500)`.
The camera angle is 26.565°; when moving to "real" 3D ground, the height in pixels is converted to the world vertical as `h / cos 26.565° ≈ 1.118·h` (hypothesis).
There is no D3D fog anywhere.
Cell (r, c) lies on screen at `(48·(c − r), 24·(c + r))` plus the frame offset.
This reconciles two earlier readings ("real 3D geometry" and "flat quads"): the only geometry under the orthographic projection is the Granny models and their stencil shadows; the ground is a heightfield in the data and flat diamonds in the frame.

A creature stands on the second corner heights (`+0x18`), not on the drawn ground's, which only the two tile renderers read (**confirmed** by code 2026-10-19, [docs/re/movement.md](re/movement.md), "The height"; §5.2).

### 5.4 The UV table of the 18 diamonds

Taken from the `cWorldView` constructor (0x624671–0x6247A1; confirmed by static analysis, `tools/analysis/world/uv_table.py`; checked on real sheets by a render, §5.6).
For row `i = 0..8`, column `j = 0..1`, diamond number `cell = 2i + j`, offset `S = (i odd ? 52 : 0)`:

```
left   (v0): u = (104j + S +  2.0)/256 + 0.002   v = (25i + 23.5)/256 + 0.002
top    (v1): u = (104j + S + 50.0)/256 + 0.002   v = (25i +  0.5)/256 + 0.002
bottom (v2): u = (104j + S + 50.0)/256           v = (25i + 48.0)/256 + 0.002
right  (v3): u = (104j + S + 97.5)/256 + 0.002   v = (25i + 23.5)/256
```

A diamond is 95.5 × 47.5 texels (96.4 × 48.4 on screen at zoom 1), the row step is **25** texels (not 24), the column step is 104, odd rows are shifted by 52; the diamonds nest in a "lattice" without overlaps, with margins of ≈ 8 texels in X and 2 in Y.
The ground is filtered linearly (the state `0x800`) only while the zoom (the world view's float `+0xB60`, 0.5 to 2, §5.3) is over 1.2, in the wider views, where a texel covers less than 5/6 of a pixel, and by point at 1.2 and closer (**confirmed** by code 2026-10-20, #343: `0x62AE90` sets the state at the head of each row, `0x62AF32`; docs/re/drawing.md, "Blending"); an earlier reading of "zoom < 1.2" had the comparison reversed.
This closes an earlier hypothesis of "9 half-rows with a step of 24".

### 5.5 Static.PAK, Floor.PAK, Triggers.PAK

- **Static.PAK** (`OBJ` v1, type 134): 0x100 + directory 1,038,014 × 12 + 64 B records back to back (matches the file size exactly).
  **Confirmed on file 2026-10-17** (layout per openheilig and Resacred's `rs_file.h` as openheilig quotes it): record 0 is all zeros; `+0x00 u32` the record's own index, `+0x04 u32` the object's type (an Items.pak record), `+0x08 u32` flags, `+0x0C u16` the sector's id (keyx `+0x24`), **`+0x0E i32 x`, `+0x12 i32 y`**: the position in isometric screen pixels, `48·(cx − cy)` and `24·(cx + cy)` for the global cell `(cx, cy)` (openheilig; what an earlier reading took for `i16 x, z, y` with a "layer" `z` of −5…4 are the two halves of `x`), `+0x17 u32` the first child (0 or the fill `0xCDCDCDCD` for none), `+0x1B u32` the parent, `+0x1F u32` the next object of the chain; `+0x27 u32` a trigger (openheilig; non-zero in 3,007 objects; for a building, **confirmed** by code, the index of its record of `Triggers.PAK` in the world's table, docs/re/drawing.md, "Buildings: masks and layers") and `+0x2B u8` the mask (openheilig; 0 or one bit of 1 to 16, 1 in 542,635 objects; checked on every record 2026-10-17 by `world_dump.py`).
  **Confirmed by code 2026-10-19** (#104, docs/re/movement.md): a static of flag `0x10` is a building, the first such in its anchor cell's chain, and its children are its floors from floor 1 up; a floor's `+0x2D u8` is the index from 1 of the inside layer of its sector that holds its grid (1,245 buildings, 1,868 floors, checked on every sector by `world_dump.py`); a static of flag `0x80` in the chain of a cell of the nibble 3 is a teleport, its `+0x23 i16`, `+0x25 i16` the global cell it leads to and its `+0x2D` the floor there (none in the files).
  A building whose object has flag `0x400` is open to the sky, its upper floors' objects drawn and picked whatever its state (**confirmed** by code 2026-10-20, #149; 253 buildings, the castles' curtain walls and crenellated towers; docs/re/drawing.md, "Buildings: masks and layers"); the world format (version 8, #203) carries it with each building.
  The world format (version 6) carries each sector's buildings, their anchors and floors.
  A static's `+0x33 u8` is, for a floor of a building and for a sprite of a type of bit `0x200` (an upper storey's floor), the floor's height in steps of 28 units of the models' height, 0 for floor 1 (**confirmed** by code for the sprites, `0x62FF60`, `0x6288A0`; docs/re/drawing.md, "Depth"); the world format (version 7, #120) carries it with each building's floors and each such sprite.
  A cell's `+0x04` is the head of a chain linked by `+0x1F`; 1,246 parents hold 1,870 children in chains of their own; the chains are linked lists (no record linked twice, a linked record never a head, no two cells with one head), and together they reach every record but record 0 and 2 orphans.
- **Floor.PAK** (`OBJ` v1, type 135): 0x100 + directory 6,713,136 × 12 + 16 B records.
  **Confirmed on file 2026-10-17**: record 0 is all zeros; `+0x00 u32` the own index, `+0x04 u32` the art tile in the low 17 bits and the mask tile in the high 15 (0 none; openheilig: drawn as one quad, the art tile's colour with the mask tile's alpha), `+0x08 u32` 0 or the fill `0xCDCDCDCD`, `+0x0C u32` the next overlay; a cell's `+0x0C` heads the chain, and the chains reach every record but record 0, each once.
- **tiles.pak** (`ISO` v3): 90,132 × 64 B, **confirmed on file 2026-10-17**: `char name[32]` (the source `isoNNN.tga`), `u32` texture.pak record, `u32` diamond = index % 18, `u32 0`, `u32 65536`, 16 zero bytes; 18 consecutive tiles share a name and a texture (5,008 sheets).
- **Triggers.PAK** (`TRG` v1): 2,268 × 16 B (1,246 occupied): `u32 id, u16 16, u16 lo, u16 hi (0..15), u16 1, u32 0` → a 20-bit value `hi<<16|lo` in the range 6,102…1,035,146; hypothesis: an object index in Static.PAK.
  **Confirmed** by code 2026-10-18: the client loads the records as they are into the world's table of triggers (`+0x268` of the world, `0x638570`), and the word at `+0x0A`, 1 in the file, is the trigger's state: a door's open bit (bit 0) and a building's state, a mask of one bit (docs/re/drawing.md, docs/re/movement.md).
  **Confirmed** by code 2026-10-21 (#383): the word at `+0x04`, 16 in the file, takes bit `0x2000` at run time when a hero (an id of 16 or below) opens or closes the door (`0x44BCD0`), and only a door so marked draws the monsters that no straight line leads to their target (docs/re/fight.md, "Doors in the way"); bit 15 of the state keeps a door from opening (`0x415F00`, `0x44BCD0`).
  Triggers are named in scripts (`trigger210`, `QIS_Trigger9511`).
  **Confirmed on every record 2026-10-17** by `tools/analysis/formats/tables_dump.py`: the header count is 2,268, and a one-entry directory `{u32 136, u32 0x10C, u32 36,288}` at 0x100 precedes the records; an empty slot is 16 zero bytes.
  `opensacred_formats::tables::triggers`; `opensacred-convert tables` writes `tables/triggers.ron`.

### 5.6 Consequences for the loader and the renderer

1. The converter reads a sector by `(x, y)`: keyx → wldx zlib block (≤ 500 KB) → 4,096 cells, whose chains name their statics and floor overlays.
   Implemented 2026-10-17: `opensacred_formats::world` (`Keyx`, `Sector`, `Tiles`, `Statics`, `Floors`); on the game files every sector and chain reads and matches `world_dump.py` sector by sector (the CRC-32 of the cells rebuilt from their fields, of the third chunk and of the chained records), and since 2026-10-18 the inside layers (`Sector::layers`, with the CRC-32 of their places, sizes and cells); the world format (version 4) carries each layer's place, size and its cells' second heights, offsets, flags and ground byte.
2. Heights stay per cell corner, as the original stores them (shared corners agree but at 2,082 of 48.8 million); the new renderer builds the terrain mesh from them.
3. **Implemented 2026-10-17:** `opensacred-convert world` writes `assets/world/world.ron` (the 6,050 sectors with their ids and files, and the 5,008 tile sheets as PNGs of `opensacred-convert textures`) and one `sectors/<x>-<y>.sector` per sector: the `opensacred_data::world::Sector` of its cells (tile, floor overlays as art and mask tiles, heights, light, the second heights, the offsets, flags, ground) and its static objects (type, flags, position, record, children), postcard-encoded in a zlib stream.
   All 6,050 sectors in 2.2 s, 74 MB; a test reads the starting sector back.
   A Python render of the ground of the starting sector (50, 39) from the tile sheets with the UV table of §5.4 and the overlays shows seamless grass, stone paths, the village square and a field: the table holds on real sheets.
   An overlay without a mask tile draws through the alpha of its own art tile; with the mask's alpha instead, its transparent texels show black.
   The same render with the sprites of the statics (Items.pak `+0x10` → mixed.pak, each at its top left corner, painted by `y`) shows the starting monastery, the pond with its pavilion and bridge, trees, fences, a flower garden and the field with crops: the placement holds.
   `opensacred-convert world` also writes `sprites.bin` (the 16,255 sprites as pieces of 10,631 atlases, pixel rectangles) and gives every static object its type's kind, its sprite or, for a miniature, its atlas and square; the game assembles each sector's sprites from the atlases' PNGs when it builds the sector (docs/05 §3.6), rather than the converter writing 3.7 GB of assembled sprites.
4. The surface type from `+0x1F` (water, lava) and the corner light are carried over as vertex and material attributes.
5. The original's limits (0x6D5 blend layers, 0x252 batcher indices, the 3×3 sector window) are not carried over into the new code.

### 5.7 The doors

Measured 2026-10-21 (#273) on the files, the converted models and the Seraphim's add-on tree (`addon-type_npc_seraphim`; each of the four trees checked places 1,130 doors) by scratch scripts; the client's own outer and inner points (`0x43B290`) are compared with them in docs/re/ai.md, "Houses and doors", as a range only (docs/08 §3).

- **Which objects are doors** (**confirmed** on the files).
  Every door is a script's object, a `CreateObj` of a type of category 10 (`items.pak` 5650 to 5766, §6.1): no record of Static.PAK has such a type.
  The tree places 1,130 doors, each with a world position in `0x20` (§7.3) and none with a heading, each naming a trigger; 1,125 of the triggers are patched (`TriggerPatch`) on one cell each, the door's cell, and 5 on none.
  The merchant's door of sector 50,40 is one of them: type 5672 (`Door_Pub01.GRN`), its trigger on the cell (3256, 2578), an entrance of the ground (flags 5, nibble 9, §5.2).
  The ground has 1,761 cells of flag bit 2, each holding one static `BLocker_Object` (type 1032, of kind 1, flags `0x200`): 1,125 are the doors' cells, and of the other 636, which no door of the tree names, 449 lie in dungeon sectors (**hypothesis**: doorways without a leaf, such as the second cell of the cemetery's gate of 50,40, (3222, 2590), beside its door's).
  Of the inside layers' 491 cells of bit 2, 483 lie on a door's cell.
- **The leaf** (**confirmed** on the converted models, the 29 of `Door_*.GRN` that the types name, in glTF's frame with y up).
  The rest pose is the closed one (the clip `*_DEACTIVATE` ends within 4.4 model units of it), and the leaf lies in the model's x–y plane, 3 to 13 model units thick along z, 40 to 253 wide and 41 to 168 high (61 to 383 and 62 to 255 world units at the models' scale).
  A single leaf hangs on x = 0 and spans to −x (`Door_Hof*`, `NeuHof*`, `Stadt01`, `NeuStadt01`, `DarkCity01`, `Villa*`, `Pub01`, `Prison01`, `Dungeon01`, `Stronghold01`, `Gate01`, `Gatter02`, `Pali01`, `Cage01`, `Zaun01`) or to +x (`Gatter01`, `Pali02`); the gates and double doors (`Desertgate01`, `Dwarf01`, `Dwarf02`, `Schmied01`, `Gitter01`, `Stronghold02`, `Waldburg01`) are centred on x = 0.
  Opening (`*_ACTIVATE`), a single leaf and the desert gate's swing about their hinges to the model's +z, the dwarves' doors to −z; `Gitter01` rises, `Stronghold02` and `Waldburg01` slide along −x, and `Schmied01` keeps its bounds.
- **The heading** (**confirmed** on the files).
  A door's type carries it, the float at `+0x57` (§6.1): 28° or 333° for most of the 117 types, 17° or 347° for `Stadt01`, 17° or 346° for `DarkCity01`, 0° for `Gitter01`.
  The client turns a model's x along its direction (docs/re/drawing.md, "The models' turn"), so the leaf runs along the heading's direction `u` on the grid (§7.3): 28° is within 2° of the grid's +x, 333° within 1° of −y, 17° 13.5° off +x toward −y, 347° and 346° 20° and 18.5° off −y toward +x, 0° the diagonal between +x and −y.
  The door's normal on the ground lies across the heading, `(−u_y, u_x)` or its opposite.
  The doorway's middle is the door's position plus the leaf's middle along `u` (half the leaf's width back from the hinge for most): from the `0x20` position it falls in the door's cell for 873 of the 1,125 doors, against 66 with the model's x across the heading, 63 with the leaf mirrored along it, and 772 from the `0x04` cell's centre.
  The cell's centre lies −25 to +25 units from it along the leaf and −21 to +16 across it, from the 10th to the 90th percentile.
- **The sides** (**confirmed** on the files).
  One cell (53.67 units) from the door's cell's centre along the normal, 472 of the 1,125 doors have a building's floor on one side, a cell walkable on an inside layer (§5.2) and not on the ground, and on the other a cell walkable on the ground: the houses' doors (`Door_Hof*`, `NeuHof*`, `Stadt01`, `NeuStadt01`, `DarkCity01`, `Pub01`, `Villa*`, `Schmied01`, `Stronghold01`), whose cells are all entrances (nibble 9).
  At all 472 the floor lies on the side `(u_y, −u_x)`, away from the viewer: up and to the right on the screen for 28° (the grid's −y), up and to the left for 333° (−x); the doors stand in the walls that face the viewer.
  628 doors have the ground on both sides (the dungeons', prisons', gates', fences', palisades', the cage's and the castles' and the dwarves' gates), 7 more a cell walkable on both levels on one side, 4 a floor on both sides, and at 14 the cell one off on a side is a wall's.
  7 of those are `Stadt01`'s of 17°, with the floor across the leaf on the side `(u_y, −u_x)` and the ground in the cell beside the door's on the heading's side: houses' doors too, 479 in all.
- **The open leaf's side.**
  `opensacred-game` turns a model's x along its heading and so its +z onto `(−u_y, u_x)` (`hero::facing`): the houses' doors there swing onto the ground, outward.
  The original turns them the same way (**confirmed** in the original 2026-10-22 with its driver, #430): the merchant's door of 50,40 at (3256, 2578), opened by a click, swings out onto the ground, hinged at its right jamb, as the game's does; the `Door_Stadt01` of (272, 2379) on floor 2 of its building (#459) swings into its room, toward the viewer, hinged at its right jamb too.

## 6. Items, creatures, weapons, balance

### 6.1 Items.pak (ITM v5): the object template database

**How the original draws the statics** (community-reported: openheilig's reading of the Linux and Windows clients, its engine `view/sector_view.gd`, MIT; used by `opensacred-game` 2026-10-17, where the starting sector renders as expected with it):

* A static with flags `& 0x290` is not drawn with the world's art (Win `0x0062AE90`).
* A sprite goes down with its top-left corner at the static's position; `mixed.pak`'s per-sprite `dx, dy` is the negation of the pieces' smallest corner and is not applied.
* A miniature (static flag `0x20`) is a square of its type's atlas (`items.pak +0x04`, 256 × 256): without flag `0x40`, `+0x2E, +0x2F` are its x, y and `+0x30` its size in atlas pixels; with it, `+0x2E, +0x2F` are the columns and rows of a grid of frames, `+0x30` a rotation about the position in 1/256 turns, `+0x31` a timing divisor and `+0x32` the frames.
* The objects go into five draw lists, drawn in order: the type's flags (`items.pak +0x00` as a u32) with bit `0x4` send a static to list 0 if it is a miniature or its mask (`Static.PAK +0x2B`) is 1, else to list 2; bit `0x800000` sends it to list 4; the rest go to list 3; the statics' shadows are list 1 (**confirmed** by code 2026-10-19, docs/re/drawing.md, "The statics' shadows").
  Within a list the cells are visited by diagonals (`cx + cy`, then `cx`), each cell's chain in order.
  In the starting sector 442 objects go to list 0 (186 miniatures, 256 flowers and small things of mask 1), 5 to list 2 and 3 to list 4 (pieces of the monastery's chapel, `KLOSTER_KAPELLE01_0U1_11`–`13`).
  The world format (version 3) carries the type's flags and the mask with every object for this.
  Read off the client's draw loop (`0x62AE90`) 2026-10-17: it skips the objects of flags `& 0x290` (`0x62B980`), and adds an object to a further list when its type has bit `0x10000` (benches and chairs: `0x10009`), the object lacks flag `0x800`, its mask (`+0x2B`, read as a word) is 1 and `DETAILLEVEL` is 1 or more (`0x62BA03`): the objects that cast a shadow, list 1 (**confirmed** 2026-10-19, #100).
  The world format (version 5) carries every static's shadow with it, its type's fields below for a type of bit `0x10000`: in the starting sector 267 statics.

**Converted 2026-10-17** with every field (`opensacred_formats::items::Record`, the name at `+0x37` padded with zeros in every used slot) into `tables/items.ron` by `opensacred-convert tables`, together with the overlay `PAK/Items03.pak`: the same layout with one used slot, 3999 (`Krombacher.grn`, a product placement; the slot is empty in `Items.pak`, so the overlay adds a type; openheilig reads the client's `Pak\Items00.pak`…`Items15.pak` as a 16-slot overlay array).
The 32,767 empty slots of `Items03.pak` are not zero, but the client reads only used slots (directory type 2).

32,768 slots × 128 B, 17,408 occupied (directory type 2); index = global id, used by Static.PAK, scripts and saves.
Confirmed on file (`tools/analysis/formats/mixed_dump.py`):

| Field | Meaning |
|---|---|
| `+0x00 u8 type` | 0: `.grn` upgrades (slots 8000+); 1: teleports/blockers; 2: creatures and equipment `.GRN` (2,485); 6: FX; 8/12: MiniObjTex; 9: world statics (9,010, names without an extension); 13: interactive objects (doors, buttons, bridges; 737); 25: trees/walls (4,533); 29: floors; 34: `.grn` upgrades (543) |
| `+0x01 u8 class`, `+0x02 u8 subclass` | for creatures 1 = hero/NPC, 5 = equipment, etc. |
| `+0x04 u32` | for the miniature types 8 and 12 (`MiniObjTex*`), the texture.pak atlas their statics cut squares from (openheilig; every miniature static's type has one) |
| `+0x10 u32 sprite` | the `mixed.pak` record with the type's sprite, 0 none (openheilig; confirmed in range on every record, and by the render of the starting sector, §5.6); equal to the own index only in some records |
| `+0x14 u32` | the type's size for the arrows' hits, 50 where it is 0 (`0x428CE0`; **confirmed** by code 2026-10-21, #383, docs/re/fight.md, "The archers"): an arrow hits an object nearer its path than the sum of the two sizes; on file 0 for the arrows and the heroes, 45 for the wolf, 25 for the goblin warrior |
| `+0x18 u16` | the client's index of the type's `Weapon.pak` record (`0x434030`); 0 in every record on file, so the client fills it (**hypothesis**: by the weapon record whose id is the slot, §6.2) |
| `+0x20 u32 id` | own index for types 2/34 |
| `+0x2E u8 category` | for the type-2 items, what the item is and where it goes (**confirmed** by code 2026-10-18, `0x54ACC0`, `0x401AE0`; the names by the models): 1 gibs and stones, 3 creatures, 5 weapons (240), 6 armour, 8 rings, 10 doors (the slots 5650 to 5766, `Door_*`, which the townsfolk go through, docs/re/ai.md), 12 effects, 13 shields (42), 15 glyphs and quest rings (the earlier "runes" was a guess; the combat arts' runes are 28), 16 books and scrolls, 17 helmets, 18 shoes, 19 belts, 20 amulets, 21 shoulders, 22 arms, 23 legs, 24 gloves, 25 the Seraphim's wings, 27 arrows, 29 bridles, 33 the dwarf's cannons. The `.grn` items of type 34 use it too (**confirmed** by code 2026-10-21, #404, docs/re/items.md, "Drops", `0x428110`, `0x43AF50`, `0x5598B0`): 9 the potions and bottles (28 types), 15 the quest items (`qi_gibs_*`), the iron bars and the gold piles 5132–5135 (`GoldCoins_01`, `_02`, `_05`, `_10.grn`), 28 the combat arts' runes (140 types, `upgrade_*.grn`); 26 is the blacksmith's upgrades of type 0 (slots 8000–8047). A shield goes in the left hand, a weapon in the hand its kind gives (docs/re/equipment.md). The object factory (`0x451300`) makes an object's class by its type's category (**confirmed** by code 2026-10-19): a creature of 2, 3 and `0x20`, a `cChest3D` of 4 (the chests, the barrels, the vases, the coffins, the cabinets, the book stands and the shrines of the start scripts, of kind 34), a `cSpecial3D` of 15 (the signposts); a `cChest3D` casts its model's shadow, a `cSpecial3D` none (docs/re/drawing.md, "The models' shadows") |
| `+0x32 u16` | 120/180/135 for heroes. **Confirmed** by code 2026-10-18 (`0x62FF60`, docs/re/daynight.md): for a type of flag `0x20000` (`+0x00` as a u32), the radius of its light by night, in pixels of the screen at zoom 1; 54 types have the flag: the heroes, some creatures, fires and spells, the torches `TORCH_SMALL_ON`/`LARGE_ON`, three miniatures and the invisible statics `SimpleLightTiny`, `Small`, `Medium`, `Big` (220, 300, 450, 600) |
| `+0x37 char name[32]` | `SERAPHIM.GRN`, `Tree17`, `FX_BLOODPUDDLE`, building mesh names |
| `+0x5B u16`, `+0x5D i16`, `+0x5F i16`, `+0x63 u8`, `+0x64 u16` | the shadow of a type of bit `0x10000` (**confirmed** by code 2026-10-19, `0x631EB0`, docs/re/drawing.md): its cell of the atlas `SHADOW_TREE00` (`texture.pak` record 14942, 16 × 16 cells of 32 × 32 texels, `16 · row + column`), half the offset of the static's foot from its sprite's top-left corner, the shape (0 a rectangle, else leaning right) and the size `s` (the quad `2s` wide and `s` tall). 2,043 types have the bit (1,412 of type 9, 623 of type 25, 8 of type 13), 1,635 leaning and 408 rectangles, one of size 0 (`Dunkelbaum21`), 137 distinct cells up to 143 (`tools/analysis/formats/mixed_dump.py`). Examples: `Tree17` (sprite 381 × 426) cell 2, foot (190, 440), `s` 240, leaning; `Bench 2` (132 × 89) cell 74, foot (78, 100), `s` 80, a rectangle |
| `+0x57 float` | the angle on the screen, in degrees, that an object of the type faces as it is made (**confirmed** by code 2026-10-19: the object manager sends it as the event `0x40`, `0x5FB530`, `0x44BCD0`; docs/re/drawing.md, "The models' turn"): −30.0 for 71 types of type 34 (the chests, barrels and book stands of the start scripts), 28.0, 333.0, 17.0 and others for 118 more of type 34, 90.0 for the heroes, 45.0 for the shields, 0 for the rest; `opensacred-convert tables` keeps it split, in `byte_57` and the low three bytes of `words_58[0]` (`Item::angle`) |
| `+0x70 u32 modelIdx` | model index in `Models.tmp` (≤ 1,571): a cache of `.GRN` name resolution |
| `+0x08` | the `texture.pak` record of the model's texture, 0 for none (community-reported as an override for the Granny material). **Confirmed** on the data 2026-10-18 for the models of the Seraphim's start script: of the 81 item models with one material it names the model's own texture for 32, another for 38, a variant of the skin (`HORSE.GRN` `horse01`…`horse05`, the wolves, bears, citizens, thieves), and the only texture of the 11 whose material has none (`chest.grn`, `vase_01.grn`, `coffin_01.grn`, `SOLDIER.GRN`); it is 0 for the models of several materials. `opensacred-game` draws an item's model with it (**hypothesis**: so does the client) |

Indices 1…9 are the hero classes (1 SERAPHIM, 2 GLADIATOR, 3 MAGICIAN, 4 DARKELVE, 5 ELVE_SORCERESS, 6/7 VLADY_D/N, 8 dwarf, 9 Daemonia); the same numbers are the class field in AMH (§9).
**Closed 2026-10-17:** the image of the type 9/25 statics with extensionless names (`Tree17`, `Busch 1`, editor labels) is the `mixed.pak` sprite that `+0x10` names (§3.3).
Confirmed on every record by `mixed_dump.py`: the directory has 32,768 slots of 128 B back to back, type 2 for the 17,408 used ones and 0 for the 15,360 empty ones, which are all zeros but slots 0 and 8191 (stray bytes); used types: 9,010 of type 9, 4,533 of type 25, 2,485 of type 2, 737 of type 13, 543 of type 34, 38 of type 12, 11 of type 29, 8 of type 8, 5 of type 6, 2 of type 1, 36 of type 0 (counted again 2026-10-20: 17,408 in all); the names are Latin-1 (`Brücke`).
Implemented 2026-10-17: `opensacred_formats::items` (type, class, subclass, the miniature atlas, the sprite, the name), matching the prototype on every record.
Of the 1,038,011 placed statics, 586,002 draw a sprite and 445,943 are miniatures (flag `0x20`): squares of their type's atlas, which Static.PAK `+0x2E…+0x32` cut (x, y, size; with flag `0x40` columns, rows, rotation, timing, frames; openheilig).

### 6.2 Creature.pak (CIF) and Weapon.pak (WPN)

**Checked on every record 2026-10-17** by `tables_dump.py` and converted with every field by `opensacred-convert tables` (`tables/creatures.ron`, `tables/weapons.ron`):

- **The CIF field map**, from the client's own dump of the table (openheilig, `creature-pak.md`, read for facts): `u32 id` (also the `items.pak` slot of the model), `u8 class` (1–15), `u8 0`, `u8 flags` (0x01 flies, 0x02 big, 0x10 no shadow, 0x20 ghost, 0x40, 0x80; no other bit is set in any record), `u8 0`, `u16 experience[2]` (a + level × b), `u8 base[6]`, `u8 0, 0`, `u8 skills[2]`, `u8 skills_extra[16]`, `u16 speed[2]` (walk at `+0x26`, run at `+0x28`: `0x4F75F0` takes them by the gait, confirmed 2026-10-18, docs/re/movement.md), six bonuses `{u8 kind, u8 target}`, `u8 bonus_value[6]`, `u8 bonus_class[6]`, `u8 damping[4][5]`: 86 bytes, every one placed.
  The earlier reading put a `u16` id at `+0`; it is a `u32`.
- **The fields' roles in the fight**, **confirmed** by code 2026-10-20 (#281, [docs/re/fight.md](re/fight.md)): `base` are the six attributes, which the level raises by a tenth each; `skills` and the first six of `skills_extra` are the skill slots; a bonus's `kind` (1–36) picks what it gives from what (the kinds 1–7: per cent of an attribute or of the level), its `target` (1–8) a kind's damage or resistance, `bonus_class` the opponents' class it holds against (0 every one); `damping` is per cent of each kind's damage (physical, fire, magic, poison) taken off on each difficulty.
  Only the kinds 1, 2 and 5 occur; no bonus has a class.
- **The heroes' types**, **confirmed** 2026-10-20 (#298): the records 1–9 are of class 1, and a record's id is also the `items.pak` slot of its model: 1 `SERAPHIM.GRN`, 2 `GLADIATOR.GRN`, 3 `MAGICIAN.GRN`, 4 `DARKELVE.GRN`, 5 `ELVE_SORCERESS.GRN`, 6 `VLADY_D.GRN`, 7 `VLADY_N.GRN`, 8 `dwarf.grn`, 9 `Daemonia.grn`; the eight templates' types (§9) are 1–6, 8 and 9 (**hypothesis**: 7 is the vampiress's night form), and the hero's loader `0x585C60` picks the row of the table of `A` and `D` by its stats' type (`+0x08`).
  The heroes walk at 70 and run at 180, but the Daemon (9) at 220 and the record 7 at 80 and 200.
- **The weapon kind**, `u8 +0x1E` (the byte 30 below): 1 swords and axes, 4 staves and polearms, 5 daggers and short swords, 7 bows, 10 the dark elves' blades, 12 crossbows, 14 muskets, among others; 0 for every type that is no weapon.
  The client picks a creature's clips by the kind of what it holds (**confirmed** by code 2026-10-19, [docs/re/motions.md](re/motions.md)).
  The client holds a weapon of kind 7 or 12 in the left hand, any other in the right (`0x54ACC0`, **confirmed** by code 2026-10-18).
  **Hypothesis**: a weapon record belongs to the `items.pak` slot equal to its id (`+0x80`): 2,383 of the 4,883 ids are item slots, and the kinds they give agree with the items' models (`BOW_LONG.GRN` 7, `BOW_HEAVYARM.GRN` 12, `muskette_1.grn` 14, `SHIELD_KITE.GRN` 0).
- **WPN**: the floats at `+0x00…+0x10` and `+0x18` are finite in every record, the name `+0x28` takes 88 bytes padded with zeros (up to 48 characters, Latin-1), `u32 +0x24` is the parent type (openheilig: the client copies the parent's item definition into the child at load), the reserve after the table is zero.
  The bytes from `+0x84` are kept as 63 u16 words until their meaning is known.
  `+0x80…+0x101` holds an item's 130-byte record in the layout that the client's code reads (2026-10-20, #357; the layout **confirmed** by code and fitting every record's bonus slots, the copy into the item a **hypothesis**; docs/re/items.md, "Defects the community reports").
  The record's name is the `global.res` string whose id is the hash of its id in decimal, as a numeric `res:N` (§8): 4,317 of the 4,883 ids have one (**confirmed** on the data 2026-10-20).
- **CIF v0**, 474 × 86 B: the earlier reading (a `u16` id, four base attributes at `+0x0C`, a list of 17 bytes at `+0x14`, resistances at `+0x36`) is superseded by the field map above.
  There are no names: they are in `global.res`.
  sacred-sdk publishes a table of 474 creature types (MIT), SacredUtils contains a `creature.pak` editor (Apache-2.0): sources for cross-checking the fields.
- **WPN v8**, 4,883 × 258 B (the array is followed by 4,883 × 64 zero bytes of reserve): float `+0x00/+0x10/+0x18`, `u8 +0x1C (1..4)`, `+0x1D (1..5)`, `+0x22 (1..3)`, `char name @+0x28` (German internal names for ~200 unique items), `u32 id @+0x80` (925…8,047, unique, monotonic), mask `u8 +0x84`, subtype `+0x85`, flags `+0x86`, `u32 +0x88`, `u32 +0x94`, `u16 +0x98`; byte 30 is the weapon kind, by which the client picks the clips (above).
  The UI names come from `global.res` (hypothesis).
  Medium confidence; an estimate of 5–10 days for the semantics.

### 6.3 bin/*.bin and the scripts/*.txt sources

The tables are compiled by the compiler built into the exe (`cScriptCompiler`) from texts that are **not shipped** (`Scripts\Balance.txt`, `scripts\sets.txt`, `treppe.txt`, `waffenmod.txt`, `Rustungenswitch.txt`, `SCRIPTS\CREATURE.TXT`, `CUSTOMMOTION.TXT`); the source names are present in the exe (confirmed from strings).
Confirmed on file (`tools/analysis/formats/bin_tables_dump.py`):

| File | Size | Structure | Purpose |
|---|---|---|---|
| `Balance.bin` | 24,328 | `u32 4896`, the size in bytes of what follows: an image of the client's balance variables at `0xAD4490`–`0xAD57AF`, 1,224 words, f32 or i32 by key. **Confirmed** 2026-10-18, counted again 2026-10-19 (#71, `tools/analysis/pe/balance_keys.py`): the balance text's parser (`Scripts\Balance.txt`, `key=value`, the chain around `0x5EAC70`) tests 380 keys (`push` of the key's name, `call 0x5E3700`), and 354 of them store their value into the image, each to a word of its own, with `fstp` (276 floats) or `mov` (78 integers), 24 of them a row of values through a register (`mov esi, <the row>`, its first word the key's variable: `ProzAW`, `ProzHP`, `ProzDmg`, `ProzRes`, `ProzExp`, `OffLevel`, `LevelKap`, `OffLevelProz`, `BaseOffLevel` and others, rows of six by the difficulty, docs/re/fight.md; #277); the other 26 store elsewhere (`SKILL_`, `MOVES`, `HorseData:`, …). A key that ends in `:` heads a row of values, of which it names the first. The 354 span exactly the 4,896 bytes, from `BalanceDmg` (`+0x000`, 1.0) to `DaemonDamageBonus` (`+0x131C`, 2.0), and each holds a value of its kind (`GameSpeed` `+0x6FC` 1.25, `TransSize` `+0x960` 190.0, `TransValu` `+0x964` 50.0, `skilllearn2` `+0x30C` 6); the earlier count of 301 missed some. The earlier reading of 4,896 words was wrong. `opensacred-convert tables` writes `balance.ron` with the 354 variables and `GoldProLevel`'s second value (below) by name and kind (`opensacred_formats::bin_tables::BALANCE_KEYS`), the image's 1,224 words as stored, and the tables after it (below). Then 19,428 bytes (4,857 u32, 1,253 of them 0xFFFFFFFF, "not set") of another layout, starting 33, 0, 56: tables of their own, below | balance constants; the variables' names are the parser's keys, and are also printed by the exe itself (`defaults.h`, `xls_*.h` on Ctrl+K) or shown by the SacredUtils/SacredMagician editors |
| `World.bin` | 46,264 | `u32 3855` and 3,855 rows of 3 × i32: the first `(−1, −1, −1)`, then mostly `(i, 3392 + 64·(i − 1), 0)` (not all: the last is `(3854, 4096, 6336)`) | a table of id → offset of 64-byte blocks (hypothesis) |
| `World2.bin` | 32,772 | `u32 32768` + 32,768 bytes (11,823 nonzero) | a 256×128 byte map (hypothesis) |
| `sets.bin` | 7,396 | `u32 66` + 66 × 112 B: the first all 0xCC, the others `{u32 items[10]; u32 hash; u8 bytes[4]; 64 zero bytes}`. **Confirmed** by code 2026-10-20 (#357, the loader `0x42C3E0`): the hash is the key of the set's name (its low 31 bits the name's `global.res` id), `bytes[0]` the count of pieces and `bytes[1]` the set's number, equal to its slot (all three on all 65 sets), which the loader writes to each piece's `Weapon.pak +0x23`; the set bonuses are not in the file but compiled into the client (`0x571710`, docs/re/items.md) | item sets |
| `wpmod.bin` | 147,796 | `u32 572` + 572 × `{u32 words[53]; u32 k; k × u32[6]}` (the client's loader `0x42F8CE`) | weapon modifiers: the templates of the items' values, the words' meanings in docs/re/items.md (#294) |
| `wea.bin` | 4,648 | 256 lists `{u32 k; u32 ids[k]}` to the end (the leading 10 is the first list's count) | weapon groups |
| `treppe.bin` | 19,952 | 1,247 × 4 u32 | "stairs": quadruples of object/position ids |
| `static10_18.bin` | 41,440 | `map\0`, u32 2574, zeros to 0x100, 2,574 × `u32[4]`: two (old, new) pairs, the second 0xCCCCCCCC twice when unset (8 rows) | remap of static ids v1.0 → v1.8 |
| `merc.bin`, `Addon/merc.bin` | 1,872 | 117 × `(0, x, y, type)` | mercenary positions |
| `MultiStart.bin` | 768 | 48 × `(0, x≈174118, y≈153134, 0)` | multiplayer start points |
| `Rust.bin` | 4,392 | `u32 85` + 85 × `{u32 k; k × (u32, u32)}` | armor switching |

**Confirmed to the last byte of every file 2026-10-17** by `tools/analysis/formats/bin_tables_dump.py` (the rows above as corrected); `opensacred_formats::bin_tables` reads them and `opensacred-convert tables` writes each to its RON file under `tables/`, every word kept.
`merc.bin` and `Addon/merc.bin` are identical.

**The tail of `Balance.bin`**, **confirmed** by code 2026-10-21 (#404; the reader `0x5E5C50`, which `initApp` calls with `.\Bin\Balance.bin`) and on the file to its byte 22,048 by `tools/analysis/formats/bin_tables_dump.py` (#413); docs/re/items.md, "Drops":

1. `u32 n` (33) and `n` × `{u32 key; u32 m; m × 12 bytes}`, keys 0 to 32, into a map at `0xAD57B0`; records of three words, which hold −1 or effect numbers (801 to 820 and others); not followed.
2. `u32 n` (18) and `n` × `{u32 key; u32 weight}` into `0xAD57BC`: the drops' weights of the quality keys 0 to 17, which `0x5CF4B0` copies into the list at `0xAD4438` (7, 9, 0, 3, 5, 6, 8, 10, 12, 13, 14, 15, 15, 15, 15, 15, 300, 544).
3. `u32 n` (16) and `n` × `{u32 key; u32 value}` into `0xAD57C8`: the item maker's quality key to the value of the quality pass `0x5D2E40` (0 to 7 the same, then 9, 11, 13, 15, 18, 21, 25 and 25).
4. `u32 n` (9) and `n` × `{u32 key; u32 words[7]}`, keys 1 to 9, into `0xAD57D4` (the first `(140, 5, 25, 5, 15, 55, 0)`); not followed.
5. `u32 n` (5) and `n` × `{u32 effect; f32 a, b, c}` into the list at `0xAD57E0`, the parameters of a chance by an effect's value `v`, `(1 − a/(b + v))·100 + 0.5` per cent at most `c` (`0x590570`, `0x5F23A0`; 100, 100 and 90 for an effect not listed): 861, 863 and 859 `(100, 100, 98)`, 862 `(300, 301, 75)`, 868 `(500, 502, 25)`.
6. Then lists into `0xAD57EC` and further, to the end of the file: 570 words, starting 6, 3, 7, 7984; not read.

`opensacred_formats::bin_tables` reads the five tables and the rest (`Tail`), and `opensacred-convert tables` writes them into `balance.ron` (#413): the second as `quality_weights`, the third as `quality_passes`, the fifth as `effect_chances`, the first and the fourth as words (`tail_1`, `tail_4`), and the 570 words after them as `rest`.
`GoldProLevel` holds two values (the parser at `0x5EE79B` reads one after the key's colon and one after a comma): `a` at `+0x8AC` (20) and `b` at `+0x8B0` (2,000), which `opensacred_formats::bin_tables::BALANCE_KEYS` names `GoldProLevel[1]` (#413; `tools/analysis/pe/balance_keys.py` finds it as the only second store of a key that heads no row); the drops' gold is `a + ⌊(L − 1)·(b − a)/100⌋` at the level `L` (docs/re/items.md).
`DropSonst` (`+0x8A8`, 1,000) is stored by the parser and read by no code.

GameServer.exe requires `Creature.pak, Weapon.pak, Items.pak, Items03.pak, Balance.bin, wpmod.bin, World.bin, World2.bin, Wea.bin` (from the server's strings); this is the minimal data set for a headless server.
The LGP Linux port: 8/8 `.bin` files match Windows byte for byte, 380/380 balance keys are present (according to openheilig), so the tables are identical in all builds.

### 6.4 Links between the tables

```
Items.pak[id] ──type 2/34──▶ Models.tmp[modelIdx] ──▶ models.pak (GRN) ──▶ texture.pak (name + ".TGA")
      │                            │
      │                            └──▶ MOTIONS.PAK / MotionDesc (clip parameters)
      ├──type 9/13/25──▶ Static.PAK[templateId] ──+0x27──▶ Triggers.PAK (back: a 20-bit reference, hypothesis)
      ├──▶ Creature.pak[creatureId] (stats)       ├──▶ Weapon.pak[id] (weapons/armor)
      └──▶ sndProfiles.pak[slot] ──▶ sound.pak[sampleId]
NetScript (CreateNPC/CreateObj/FillChest …) ──▶ Items.pak, DefPos/Vectoren, global.res (res:NAME → sacred_hash)
AMH save ──▶ Items.pak (inventory), classes 1..9, QuestDB (section 0xCD)
```

### 6.5 The combat arts' tables in Sacred.exe

**Confirmed on every record 2026-10-23** by `tools/analysis/formats/arts_dump.py` (#505); what each field means and the code that reads it are in [docs/re/arts.md](re/arts.md) (#500).
The combat arts are in no data file: two tables compiled into `Sacred.exe` hold them, the defaults of a balancing spreadsheet (`xls_magictypes.h`, `xls_smovetypes.h`), as the table of the sound ids' names is compiled into it (§10.2).
Both lie in `.data`, which the GOG 2.28 file stores at its virtual addresses less the image base `0x400000`; another build would hold them elsewhere, so the converter reads them only from the manifest's `Sacred.exe` (§13).
Each table opens with an empty record 0, the id 0 with the icon `INVALID.TGA`, which the code does not search; the second table ends where the first starts.
A value by the level `L` is a pair `(a, b)`, `a + L·b`.

- **The spells**, the first table, at VA `0x952A38` (file offset `0x552A38`): the record 0 and 70 records of `0x94` B, to `0x955344`.
  `{u16 id; u16 0; u32 name; u32 description; char icon[32]; u32 0; u32 sprites[3]; u16 launch; u16 regeneration[2]; u16 effects[13][2]; u16 values[3][2]; u16 caps[3]; u16 element; u16 0; u32 classes; u8 flags; u8 0[3]}`.
  The ids are 1 to 72 but 54 and 68, mostly in order: 13 and 14, and 47 and 48, are swapped, and from 54 on the ids run one or two ahead of the records.
  The icon is a texture's name, ASCII with zeros after it, and the `u32` after it its texture's handle at run time, 0 in the file; `flags` holds bits 0 to 2 alone.
- **The weapon arts**, the second table, at VA `0x94FDB0` (file offset `0x54FDB0`): the record 0 and 94 records of `0x78` B, to `0x952A38`.
  `{u16 id; u16 0; u32 name; u32 description; char icon[32]; u32 0; u32 sprites[3]; u32 +0x3C; f32 regeneration[2]; i32 distance; u8 launch; u8 0[3]; f32 factor[2]; f32 +0x58[2]; i32 +0x60[2]; i32 +0x68[2]; u32 classes; u8 flags; u8 0[3]}`, the first `0x3C` B as a spell's.
  The ids are 1000 to 1103 but 1011, 1055, 1056, 1077, 1081, 1088, 1090, 1093, 1095 and 1096, mostly in order; `+0x3C` is 5 to 120, read by no code found.
- **The texts**: `name` and `description` are text indices, and the short description's index is the long one's plus 600; an index names the `global.res` string whose id is the hash of the index in decimal, as a numeric `res:N` (§8).
  Every name and short description is a string of `global.res`, and every long description but those of the eleven spells 43 to 53, whose index is 0 (**confirmed** on the data 2026-10-23).

`opensacred_formats::arts` reads both tables, the addresses mapped through the section table (`opensacred_formats::pe`), and matches the prototype on every field of every record; `opensacred-convert tables` writes them into `tables/arts.ron` (`opensacred_data::tables::Arts`) with their fields named and the text indices as `global.res` ids, after it checks `Sacred.exe` against the manifest.

## 7. NetScript scripts

### 7.1 Record encoding

Confirmed on file: all `FunkCode/StartCode/QuestCode.bin` files in the 20 directories are parsed without remainder (2,203,278 records in total and 113 types by `tools/analysis/scripts/netscript_census.py`, remainder 0 in all files, including the empty ones).
A record = `u16 code; u16 len` (len includes the 4 header bytes), followed by operands: a tag byte and the payload the tag selects.
**Confirmed on every record 2026-10-17** by `tools/analysis/scripts/netscript_dump.py`, with the tag table read off the client's operand reader (`FUN_00472BC0`, 40 handlers; the table and the handlers are in [docs/re/netscript.md](re/netscript.md)): 66 tags and 0x71 are zero-width markers; the others carry a u8, u16, u16 × 2 or × 4, i32, i32 × 2, × 3 or × 4, a C string (e.g. `0x01/0x09/0x1E`: position names, `res:1024`, dialog and scene names), two strings, an i32 and a string (with an optional expression for 7 tags), a vector or a position name after `0xFFFFFFFE` (`0x04`), an i32 or a variable name after `0xFFED2979` (`0x0B`), a byte and an optional string, or a variable reference `9F EF BE ED FE name`.
Tags 0x00, 0x17, 0x18, 0x21, 0x22, 0x76, 0x94 and 0xA2–0xFF end the record with an error; none occurs.
With this table every record of the 20 trees consumes its body exactly, except: type 40, whose body is an 80-byte struct `{i32 −1; char name[64]; i32 a; i32 b; i32 0}` (16,460 records), type 41, whose body is a u16 from 1 to 5 (4,950), and 11 records of type 50 whose last tag 0x2A has no payload (the client reads 12 bytes past the record).
Implemented 2026-10-17: `opensacred_formats::netscript` (`Script`), matching the prototype on every operand of the 51 non-empty files.
There are no jumps by offset: branching goes by names and indices (`Vectoren.bin`), state is quest flags, variables, triggers.
Terminology: in this document "opcode" = record type (u16), corresponding to sacred-sdk's "tag"; the operand marker is called "tag" by openheilig and "opcode" by sacred-sdk.

Auxiliary files, **confirmed 2026-10-17** on the client's loader (`0x46FBxx`–`0x47021E`) and on all 20 trees by `netscript_dump.py`:

* `Vectoren.bin`: three counted tables back to back to the end of the file.
  (1) `u32 n` and n records of 84 B `{char name[64]; i32 offset; i32 length; i32 quest; i32 region; i32 0}`: the **function blocks of `FunkCode.bin`**, contiguous from offset 0, each starting and ending at a record, together covering the whole file (23,520 in the Sacred campaign tree `NetScriptCamp`, 17,208 in Underworld, 18,769 in MP; the first has no name and no length; `quest` is −1 or the id of a quest of table 2, `region` 0 or, in the Sacred campaign trees, 1–23).
  (2) `u32 n` and n records of 292 B `{u32 id; char name[64]; 192 zero bytes; i32 values[8]}`, the quests (603 / 190 / 286; the values hold function indices, hypothesis).
  (3) `u32 n` and n regions `{u32 id; char name[64]; i32 values[5]; 3 × (u32 k; u32 list[k]); u32 m; m × (u32 k; u32 list[k]; i32 values[4]; u8 tail[8])}`: 22 in the Sacred campaign trees (`Region1` …), none elsewhere.
  Implemented: `opensacred_formats::netscript::Vectoren`.
  The earlier reading (`"QI"` + 18,768 records) took the first count's bytes for a signature.
* `DefPos.bin` is a **cache** the client writes: on load it reads the first u32 and takes the file only if it is `0x4D2` (`0x46FC5C`); otherwise it rebuilds the tables from the scripts and writes `{u32 0x4D2; u32 n; n × 100 B position; u32 m; m × 80 B dialog entry; u32 k; k × 76 B}`.
  The shipped files have no `0x4D2` and no third table (`{u32 n; n × {u32 0; char name[64]; i32 x; i32 y; i32 −1; i32 −1; 16 × 0xCC}; u32 m; m × 80 B}`, the names padded with the stack fill 0xCC), so the 2.28 client never reads them and rebuilds them; their dialog tables hold 1 + 618k entries (k = 1…8) in the Underworld trees, which have 618 type-40 records each: the table grew by one copy per rebuild on the developers' machines (hypothesis).
  The converter skips them, and the manifest leaves them out (§13).

Other auxiliary files: `QuestCode.bin`: 1–3 records of starting flags (the Sacred campaign and the MP trees), empty in the 9 Underworld trees (`bin/Addon/NetScriptCamp`, `bin/Addon/TYPE_NPC_*`); `QuestPoolCode.bin` is empty in all 20 directories.

Empty quest files: 29 files (9 `QuestCode.bin` + 20 `QuestPoolCode.bin`) in the GOG release are 0 B in size; the converter treats them as sections without records.

### 7.2 Twenty directories

| Tree | FunkCode: bytes / records / types | StartCode: bytes / records / types | What it is |
|---|---|---|---|
| `bin/TYPE_NPC_<8 classes>` + `bin/NetScriptCamp` (= GLADIATOR byte for byte) | 3,960,242–3,967,103 / 124,931–125,236 / 102–103 | 575,477–578,356 / 15,408–15,494 / 32–34 | **Sacred campaign**; the classes differ in one block (starting position, first quest), similarity 98.5–99.8% |
| `bin/Addon/TYPE_NPC_*` + `bin/Addon/NetScriptCamp` | 2,504,248 / 66,969 / 77 | 600,760 / 16,073 / 30 | **Underworld campaign**; all 9 trees are identical, a single starting position |
| `bin/NetScript`, `bin/Addon/NetScript` | 2,774,680 (679) / 78,733 / 87 | 640,517 / 16,962 / 29 | multiplayer/"free play" |

Correction to an earlier reading: `NetScriptCamp` is the Sacred campaign; Underworld lives in `bin/Addon/*`.
For the port it is enough to load the directory by hero class (as the original does: `Bin\NetScript%s`, `bin\Addon\%s`); storing "one bytecode + 8 patches" is an option.

### 7.3 The opcode space: 141 and 113

141 = the size of the dispatcher table (0x00–0x8C).
**113** types occur in the shipped scripts (112 in the Sacred campaign, 87 in Underworld, 96 in MP); 28 never occur, and for 16 of them the client has no `case` at all; 42 and 62 (`NOP`) occur and are swallowed by the default branch.
Coverage of campaign records: 45 types give 95%, 68 give 99%.
**Names, confirmed 2026-10-17:** 135 of the 141 types have a keyword, read by `netscript_dump.py` off the five "keyword → type" resolvers of the compiler in the client's `.text` (61 + 60 statements, 7 + 6 quest directives, 3 dialog commands; docs/re/netscript.md); the strings themselves lie in `.data` at file offset 0x54CE78 (`tools/analysis/scripts/kwtab.py`).
`DelNPC` and `DelOBJ` both compile to 55.
Without a keyword: 0, 39–42, 47; of these 39 (binds an id to a trigger name, 12,146 records), 40 (the 80-byte dialog table entry, 16,460), 41 (a block selector 1–5, 4,950) and 42 (a no-op in the walker, 4,950) occur.
Corrections to the earlier reading: 43, 44 and 111 are the dialog commands `ExitDlg`, `DelDlg` and `ToBegin` (not a guard bracket, "the dialog dispatcher" and "end of section"), and 122/123 are the quest sections `SolveTime:` and `Strafen:`, which settles the sacred-sdk/openheilig discrepancy.

**`CreateNPC` operands, read off the data 2026-10-17** (hypothesis; `opensacred-game` places the start script's NPCs with it and they stand in sensible places): the first `0x02` is the creature, which is also the `items.pak` slot of its model (295, 288, 297 → `SOLDIER.GRN`, 555 → `HORSE.GRN`, 652 → `CITIZEN_MAL.GRN`), the further `0x02` its equipment (1729 `SWORD.GRN`, 1731 `BOW_HEAVYARM.GRN`, 1200 `SHIELD_KITE.GRN`; **confirmed** by code 2026-10-18: the handler keeps them in order and equips each once the creature is made, docs/re/equipment.md); `0x04` its position, the name of a `DefPos` record (`DefPos 01:name 0B:x 0B:y`, a global cell) or a global cell `(x, y, 0)`; `0x03` a heading in degrees (45, 90, 225); `0x01` its name as a text id.
The Seraphim tree's start script creates 319 NPCs.
**Confirmed** by code 2026-10-20 (#157, the handler `0x482510` and its jump tables at `0x485B58` and `0x485A68`): the operand-less tags make the creature's flags word (`+0x1F4`, written at `0x4839F6`): its low 16 bits one mode, the last of `0x12` (1), `0x13` (2), `0x14` (0), `0x2B` (8), `0x2E` (`0x10`), `0x30` (`0x20`), `0x2F` and `0x31` (`0x40`), `0x32` (`0x80`), `0x42` (`0x400`), `0x43` (`0x800`), `0x44` (`0x1000`), `0x46` (`0x4000`), `0x4C` (`0x8000`); `0x45`, `0x50` and `0x74` add `0x2000`, `0x10000` and `0x8000000`, `0x85` and `0x8A` clear the mode and set `0x2000000` or `0x10000000`; `0x08` makes the creature hostile and `0x0E` not, by its side rather than its flags word (docs/re/fight.md, "Sides"; #580), while the modes 1 and 2 put it in the hero's way (docs/re/movement.md, "The objects in the way").
  In the Seraphim tree, the guards of the start carry `0E 46`, the monsters `08 12` (2,411 records), the captain `hauptmann3` `0E 46 13`.
Confirmed by code 2026-10-17 (the handler `0x482510`, through the walker's jump table `0x4784C0`): a cell becomes a position at `(cell + 0.5) · 53.66563`; tag `0x03` is the heading, a random one of 0…360 (`rand() % 361`) without it, which `0x44DDC0` turns into a direction: `(cos, sin)` of `heading · π/180` on the screen, 0° to the right and 90° down, taken back into the grid's axes through the inverse of the cells' projection (`0x622660`, from the constants 53.66563, 48 and 24 at `0x892048`: the direction `(cos + 2 sin, 2 sin − cos)` on the grid; corrected 2026-10-19, #159 and #161: the grid's axes were read before, a quarter turn off for 45°), so the model shows at that angle on the screen (docs/re/drawing.md, "The models' turn").
`CreateObj` (type 8; 1,990 records in the Seraphim's start script: 596 barrels, about 1,000 chests, book stands, vases, coffins, signposts) has the same `0x02` (the `items.pak` slot of its model), `0x04` (a cell `(x, y, 0 or 1)`) and `0x03` (a heading), and names a trigger in `0x29` (`chestR1_09`) and its contents in `0x41` (`fillchest_chestR1_09`, a `FillChest` record); hypothesis by the same reading, `opensacred-game` places the models with it.
  **Confirmed** by code 2026-10-19 (`0x485C10`, the walker's case 8): the handler turns the object by its heading only when it is above 0; without one the object faces the angle of its `items.pak +0x57`, which it takes as it is made (§6.1).
  **Confirmed** by code 2026-10-21 (#273, `0x485C10`): `0x20` is a world position `(x, y, z)` in world units, which the handler keeps negated; with a cell in `0x04` the object's position is the cell's centre, `(cell + 0.5) · 53.66563` (`0x487130`), and `0x20` takes its place only for an effect (category 12, `0x4280B0`), or without a cell.
  For an object with a model the handler then sets the floats at `+0xCC…+0xD4` from `0x20` and the object's flag `0x800000`: **confirmed** by code and in the running original 2026-10-22 (#459), they are where the model is drawn, `0x20` moved by `1.92 · h` world units along x and along y for the object's height `h` at its cell's centre on its layer (in units of the models' height), and `h` (docs/re/drawing.md, "The models").
  5,730 of the 8,266 `CreateObj` records of the Seraphim's add-on tree carry it, all 1,130 doors among them; 1,026 of those with a cell lie farther than 38 units (half a cell's diagonal) from its centre, up to 810.
  Of the doors 188 do, the dwarves' (59) and the fences' (33) the most of them, the farthest a castle's door on a floor of layer 2 (748).
  The third number repeats the cell's layer in all 5,703 records that carry both, so it is no height.
  A raised doorway's `0x20` lies on the ground up the screen from it, √(3/2) units along −x and −y for a unit of the doorway's height above the ground's heights at `0x20` (**confirmed** on the files and the game's heights 2026-10-21, #438).
  In the base tree the doors of layer 2 do so: two `Door_Stadt01` of cells (272, 2379) and (1744, 2503), on a floor 263 units up, lie 455 units from the cells of their triggers, beside their own, and the castle's `Door_Stronghold01` of (2365, 1991), on a floor 439 up, lies 748 from its cell, its trigger patching none.
  So does the keep's `Door_Stronghold01` of (2345, 2014) on layer 0, its doorway a landing atop a stair 116 units above the ground at its `0x20`, 208 units from its trigger's cell (2344, 2014).
  Taken along the line of sight from the ground's heights at `0x20` to their doorways' heights, the leaves' middles (§5.7) of the first two lie 12 and 13 units from their triggers' cells' centres and the third's 14; of the 1,125 doors whose trigger patches one cell, 1,100 lie within a cell of it so against 1,090 at `0x20` on their doorways' heights, none more than 6 units farther.
  The eight `Door_Villa02` lie 1.4 to 1.7 cells from their triggers' cells by the leaves' middles at `0x20`, seven of them on porches 28 to 40 units above the ground there; in the game the leaf of the eighth, (1750, 3559), with no such height, shows in its arched doorway at `0x20` (by a frame's difference with the leaf and without), so that the model's middle, not its place, misses the trigger's cell.
  **Confirmed** 2026-10-22 (#459): the client draws the model where `0x20` at height 0 shows, at the object's height on its level, never reading the ground's height at `0x20`, and puts the object in the draw list of its `0x04` cell on its layer (docs/re/drawing.md, "The models"); the two `Door_Stadt01` of layer 2 so stand centred in their doorways, drawn after their cells' back walls, as the running original shows them.
  `opensacred-game` shows an object's model where its `0x20` at height 0 shows, standing it along the line of sight at its cell's height on its level, and keeps the cell's centre as the object's position (#429, #438, #459; docs/decisions/2026-10-21-raised-doors.md): the cell that a chest takes in the creatures' way, the doors' distances, the doors that the creatures choose and the model's place among the sprites go by the cell, the model and its shadow by where it stands.
  The 2,817 `CreateNPC` records of the tree that carry a `0x20` with a cell all lie within 38 units of its centre; the game places the NPCs at their cells' centres.
**The spawning records**, **confirmed** by code and on the trees 2026-10-20 (#215, docs/re/spawning.md), all in the sectors' functions `Sector<x><yyy>Init` and `…Enter`:
`SpawnValues` (type 100) `0B:density 0B:lowest 0B:highest`, the sector's density in percent (50 in all 11,498 records) and its creatures' levels (1–45, 4–80);
`FillSector` (type 51, 5,309 records, all in `Enter` functions) `36:n` (100 for animals, 310 for monsters), `34:(sx, sy)` the record's own sector, `35:(a, b, 4096)` `a` to `b` creatures per 4,096 cells (0–16), `33:(lowest, highest)` (overridden by `SpawnValues`), then up to 100 `19:(type, weight, hostile)` entries (14,000 in all, 251 types, weights 0–100); `08` and `0E` make every entry hostile or peaceful;
`MakeGroup` (type 115, 22,762 records) an equipment rule for the creatures that `FillSector` makes: up to 20 `02` types, `87`, `88`, `89` each `(pool, chance %)` of an item from the start script's `Waffenpool` pools (type 114, `0B:pool 02:item`: each record adds its last item to its pool 0–254, duplicates kept; 906 records, 114 pools), `8B` a byte, `1D`, `1E` or `16` a name, up to ten `67` spells.
`SetDrop` (type 137, 1,204 records, all `02 0B 02`), **confirmed** by code and on the trees 2026-10-21 (#404, the walker's handler `0x4AFFF0`, docs/re/items.md, "Drops"): a creature type, a chance in per cent and an item type, the two types in either order (956 records name a quest item of category 15, 168 a potion or bottle, 40 a ring and 40 a book); the chances are 0 (602 records, which remove a drop), 75 (346), 80 (132), 100 (71), 50 (33) and 65 (20).
`StartPosition` (type 45, one per hero tree) is the hero's start cell, `0B:x 0B:y` (the Seraphim's `3236, 2511`, in the monastery's chapel; the Gladiator's `3790, 349`; the Daemon's `352, 1722`); hypothesis by the places, which match the heroes' starts.

The most frequent types (per campaign tree): 115 `MakeGroup` 22,762; 100 `SpawnValues` 11,498; 8 `CreateObj` 8,778; 1 `CreateNPC` 6,433; 66 `ELSEIF` 5,401; 51 `FillSector` 5,309; 67 `SetVar` 5,233; 53 `QuestBook` 4,514; 26 `Text` 4,323; 50 `TriggerPatch` 3,720; 60 `SetButton` 3,218; 48 `CreateTrigger` 3,100; 23 `DefPos` 3,022; 58 `IF` 2,764.

### 7.4 Control flow semantics

According to sacred-sdk (the walker `FUN_00475680`, 123 `case`s) and, independently, openheilig: on a false condition `IF` (0x3A) scans forward by record lengths to `ELSE` (0x3B)/`ELSEIF` (0x42); `ELSE` skips exactly one record; "guard" conditions (`QuestDone`, `GroupIsDead`, …) abort the whole section on a false result; 0x2B/0x00 is the end; guard 0x29 makes records be skipped up to 0x2A.
The interpreter must **refuse** an unknown type rather than skip it, otherwise the body of an `IF` will run unconditionally.

### 7.5 Where the interpreter is and where the compiler is

Confirmed from the strings of both exes: the loader (`%s\FunkCode.bin`) and all handler strings (`CreateOBJ`, `GetPoolPos`, `IsNotVarBit`, …) exist only in Sacred.exe; GameServer.exe has only `SelfTriggerQuest%d`, `TRIGGER_INSERT_QUEST`, `NW_TRIGGER_UPDATE_QUESTDB`.
The scripts are executed by the **host client**; the server stores QuestDB and relays `TRIGGER_*` packets.
A new game runs its tree's start script through, which sets quests up (`SetUpQuest`) and enters some (`TriggerQuest`), running their `QIS_OnEnter<n>` functions (**confirmed** by code 2026-10-19, docs/re/netscript.md, "Quests").
This refutes an earlier guess ("the VM on the server is the best entry point") and means the VM's semantics have to be recovered from the client.
Addresses according to sacred-sdk: the operand reader `FUN_00472BC0` (a table of 256 markers in 40 groups), the walker `FUN_00475680`, section start `FUN_0046BA90`.
The source compiler (`cScriptCompiler::parseStatement/parseResource/loadScriptedSequenceR`, `cInterpretSQW`, error messages with line numbers, keyword and directive tables `ToDoBloecke:`/`Delay:`/`Belohnungen:`/`Dialog:`) is built into the exe: the grammar can be recovered by decompilation, which opens up quest modding from text.

### 7.6 Conversion and the interpreter: what is needed, and the estimate

1. A disassembler in Rust with a "no remainder" check on all 20 trees: done 2026-10-17 (`opensacred_formats::netscript`, §7.1).
2. IR: done 2026-10-17 as a faithful one (`opensacred_data::netscript`, `opensacred-convert netscript`): one postcard file per tree with the start script, the quest flags, `FunkCode.bin` split into its named function blocks by `Vectoren.bin`, the quests and the regions; the records keep their types and operand tags.
   `--listing` writes a readable listing of each tree.
   Grouping into quests, states, conditions and actions waits for the record semantics (step 3).
3. Record type semantics from the interpreter code in Ghidra, starting with the most frequent: 45 types give 95% of campaign records, 68 give 99%.
   For each type, a test on real records.
4. The interpreter in `opensacred-rules`: a `match` on the record type; the 28 types absent from the release are not implemented.
5. Estimate: converter and IR 15–30 agent-hours (M1); interpreter and semantics 25–50 agent-hours (M4).

## 8. Localization: global.res and the hash

**Confirmed on every record 2026-10-17** by `tools/analysis/formats/global_res_dump.py`, correcting the earlier reading: the file is 2,760,574 B: `"SZ\0\0"`, then **23,123** records of 16 B `{u32 id; u32 offset; u32 flags; u32 length}` sorted by id (unique), then a pool of **UTF-16LE** strings without terminators, one right after another, from the end of the records to the end of the file.
An offset counts from byte 4, the start of the records; the length is in bytes.
`flags` is 0 but in 4 records (1, 1, 3, 5; meaning unknown).
The earlier reading counted 23,122 strings and read the fields with a shift (the sdk model `{d0, id, offset, pad}`, `d0[k]` the length of slot k−1): it lost the last record.
Every string decodes as UTF-16 without unpaired surrogates; none holds a line break or a brace, 48 start and 1,343 end with spaces.
Binary search by id (`FUN_0080F5E0`).
The path is `.\SCRIPTS\%s\global.res` by the `LANGUAGE` key (the GOG installation has only `us`, `Settings.cfg` has `LANGUAGE : US`; the GOG product has separate French and Russian language depots, not checked locally); the save stores "save language"/"game language".
Implemented 2026-10-17: `opensacred_formats::text` (`GlobalRes`, `sacred_hash`), matching the prototype on every record; `opensacred-convert text` writes `text/en-US/global.ftl`, one message `id-N` per string with id `N` (the spaces at the ends of a string kept as string literals, which Fluent would trim), and a test reads every message back with `fluent-syntax` as the string of `global.res`.
The file itself is not encoded: the encoding that community notes mention belongs to the copy embedded in the exe as the `BINARY #107` resource (loader `FUN_0080E680`), which the port does not read (§2; docs/01 §4.6).

All ids are `sacred_hash(name) & 0x7fffffff`; numeric references `res:N` are hashed as the string `str(N)` (`res:1` → 49, `res:1024` → 71,320,567): 946 of the 947 numeric references in the scripts resolve, 12,965 of the 23,123 ids are reproduced from names present in the release; the rest are hashes of names from unshipped sources (not needed by the port).
The formula of the client's `FUN_0080E780` (credited to sacred-sdk, MIT; confirmed by rebuilding all slots with `tools/analysis/formats/global_res_hash.py`): starting from `h = 0`, for each byte `c` of the name, with ASCII `a`–`z` upper-cased, `h = (h·113 + c) mod 999,999,991`, where `h·113 + c` wraps to 32 bits and is taken as a signed 32-bit integer, and the remainder has the sign of the dividend (C's `%`); the id is `h & 0x7fffffff`.
The port is `opensacred_data::text::sacred_hash` (`crates/opensacred-data/src/text.rs`), the key of the open format's texts, which `opensacred_formats::text` re-exports and the game looks its lines up by (#270).
A "mathematical" version without the 32-bit overflow and the signed remainder gives different values (checked by brute force).
For the engine: an in-memory index, UTF-16LE → UTF-8 at load time, byte comparisons wherever gameplay depends on them (inventory sort order), a `mods:/<lang>/global.res` overlay and CSV/PO export/import by a tool; font fallbacks for Cyrillic/CJK: §11 and docs/06.
Ready-made code: sacred-sdk (`sacred_hash.h`, a `global.res` generator), SacredUtils (Apache-2.0, decompiler/compiler).
Estimate: < 1 day with the ready-made code.

**Defects the community reports**, checked for #358 on `scripts/us/global.res`, the converted `text/en-US/global.ftl` and the tables on 2026-10-21; a string is named here by its id, the message `id-N` of `global.ftl`, never by its wording:

- **The orc god's name left untranslated** (**community-reported**): **confirmed**.
  Ten messages carry one German word, the name of a god that the goblins' and the orcs' lines invoke: alone in nine and in a compound in one.
  They are ids 63580866, 63580868, 567832966, 588918616, 640731051, 724553692, 724553694, 990141306, 1478592224 and 1478592228.
  No string of the file holds an English form of the name.
- **Missing names and lines**, which ReBorn restored in every language after Sacred NL (**community-reported**): two kinds show in the data (**confirmed**), whether they are those ReBorn restored is a **hypothesis**.
  Names: a creature type's name is the string whose id is the hash of its number in decimal, as a weapon's (§6.2; type 37 is id 5818), and 25 of the 474 types have none: the empty type 0, 146, 164 to 167, 182, 183 (`Anducar.grn`, `Anducar_Demon.grn`), 222 to 224, 328, 356, 357, 361 to 365, 367, 368, 599 (`WOLF.GRN`), 706 (`NOBLE_FEM.GRN`), 1879 and 6114; and 566 of the 4,883 `Weapon.pak` records have none (§6.2), 448 of them with an `items.pak` slot.
  Lines: of the `res:` keys the scripts name whole, not built at run time with `+VAR(…)`, four quest-log keys of `QuestBook` records (type 53) resolve to no string, `HQ_6_2_1_Log_preQuest`, `NQ_7607_LOG_HEADER`, `NQ_LOG_QLOSE` and `R09NQ01_Log_Qsieg`, besides `res:174242` (type 22), the one numeric reference that does not resolve (above).
  What the game shows for a missing name or line is not read.
- **Generic names of the arcade's "gamebox" items** (**community-reported**): **confirmed**.
  The eleven items 5150 to 5160 (models 953 to 963, `gamebox_*.grn`, a model each) are named by ids 77105259 to 77105268 and 77105372, which all hold the same single word, the records' `Weapon.pak` internal name as well.
- The unused hero voice lines, some German (**community-reported**, DarkMatters topic 72808), are cut content, not a defect.

**Proposal: repairing the texts without carrying them** (#358; a proposal that awaits the maintainer's decision, nothing of it is implemented).
`opensacred-convert text` would apply a table of repairs as it writes `global.ftl`, each a decision of its own (`docs/decisions/`), since it changes what the player reads.
Every entry names its string by id and carries the SHA-256 of the original string it was written for, so it applies only to that text and is skipped with a warning on another one; the table carries ids, offsets, lengths and hashes, never the game's wording.
Its sources, in the order of preference:

1. **The same file**: a string takes another id's string whole, or a span of it (id, offset and length in UTF-16 units) replaces a span of its own.
   Nothing of the game is carried, but it repairs only what the right text already exists for elsewhere in the file; none of the reports above is such a case.
2. **Another language the user has installed**: an id the English file lacks takes the same id of `scripts/<lang>/global.res` from the user's other installation (the GOG product has French and Russian depots, not checked locally).
   It fills a gap in another language, and that file is outside the manifest, so its strings cannot be pinned by hash in advance.
3. **Words of the project's own**: what no installed file holds (the god's English name, names for the eleven gameboxes, the missing names and log lines) would come as English text written for OpenSacred and kept in the table, never ReBorn's or another release's wording.
   A span to replace is named by its id, offset, length and the hash of the original span, never by the original word; each such text is a choice of translation.
4. **A text overlay the user supplies** (a patched `global.res` or a file of `id = text` lines from a mod they own), read after the original: the repository carries nothing, but the result depends on the user's copy and its terms.

The limits: a repair holds only for the exact text of 2.28 that its hash names; a span replaced inside a sentence leaves the grammar around it as it is; and a text taken from another language or written by the project is no longer the original's.
Speech is not text: the goblins' shouts (§10.2) would need other samples, not a table of strings.

## 9. Saves and AMH hero templates

Confirmed on 10 files (8 `Templates/Hero*.ptx`, 2 `save/hero*.pax`; `tools/analysis/formats/amh_dump.py`).
0x100 header: `"AMH", 0x1B` (u16 @3 = **0x101B**, "UW 2.21, OpenMethod 2" per sacred-sdk/charmodif; in all 10 files), `u32 0x10` @4, `u32 0x22C` @8 (summary size), `1` @0x1C, `9` @0x20, `2` @0x24, `20000` @0x28, zeros elsewhere (corrected 2026-10-17: the earlier reading put the last four words 4 bytes later).
Table @0x100: 16 slots × `{u32 tag; u32 off; u32 size}`; data from 0x1C0 (= sacred-sdk's "0x1C0 header").
8 slots are occupied (0–5, 8, 9):

| Slot | Tag | Size (template / save) | Storage | Contents |
|---|---|---|---|---|
| 0 | 0xC7 (199) | 1,881 / 2,013–2,079 | zlib | `cCreatureHero` dump (starts with 0xFEEDF00D; per sacred-sdk: class @0x3DD, experience @0x3E1, gold @0x41B, level @0x42B; not checked byte by byte); the hero's combat arts from byte 1,227 (docs/re/arts.md, "The heroes' starting arts") |
| 1 | 0xCA (202) | 4,444 | zlib | the hero's backpack: an inventory container (below), 20 × 8 cells |
| 2 | 0xCB (203) | 4,444 | zlib | a second inventory container, 20 × 10 cells, of the reference `0x11`, empty in all 10 files |
| 3 | 0xC8 (200) | 2,528 / 21,148–23,216 | zlib | inventory: a block per item (below; 5 to 7 in a template, 52/58 in saves) |
| 4 | 0xC3 (195) | 556 | uncompressed | summary: `u32 level`, `u32 classId` (1..9, see §6.1), `wchar name[32]`, statistics (kills, gold, time) |
| 5 | 0xC4 (196) | 64 on disk (556 in the table) | uncompressed | mini block |
| 8 | 0xCD (205) | 822,824 | zlib | world/quest/chest state (98.6% zeros; markers 0xB00B/0xDEAD; templates differ in ~60 bytes, saves in ~240) |
| 9 | 0xCE (206) | 100 | uncompressed | zeros (there is no room for a checksum) |

Compressed sections: a 32-byte prefix `u32 0xBAADC0DE, u32 csize, 24 × 0` + zlib (`78 DA`); inflate yields exactly `size`.
**Confirmed on every section 2026-10-17** by `tools/analysis/formats/amh_dump.py`: the sections follow the table in slot order, back to back, the last to the end of the file; a stored section is `size` bytes on disk but for 0xC4 (64 bytes for a size of 556).
The 8 templates are of 8 different classes (1–6, 8, 9), level 1, without a name; the 2 saves are level-29 heroes.

**The inventory containers** `0xCA` and `0xCB`, **confirmed** 2026-10-19 (#133) by the client's reader (`0x5DDA30`, which the hero's loader `0x6058A0` calls for both, `0xCB` for the reference `0x11`) and on every section of the 10 files, read back to their exact 4,444 bytes: six u16, the container's number (1 for the backpack, 17 = `0x11` for the second), flags (16 in every backpack, 0 or 4 in the second container; `+0x1162` of the object), two more (the second 392 in the saves' backpacks, 0 in the templates', 20 in every second container), the grid's width and height (20 × 8 and 20 × 10); `0x400` bytes (zeros in all 10); the grid, `0xC00` bytes of 256 cells of 12 bytes, row by row, width × height of them used, a cell's first u32 an item reference (0 none); `0x78` bytes, five records of 24 bytes of two references each; 200 bytes; two u32; then the quest items, a u32 count and 8 bytes each (the client logs it as `NumQuestItem`), and the scrolls, a u32 count and 4 bytes each (`NumScrolls`), both 0 in all 10 files.
The backpack of a template holds 4 items, one cell each (the three healing potions and the large torch of `Hero01.ptx`'s inventory), the saves' 33 and 39; the second container is empty in all 10.
A cell names its item by the item's object id, a `u32` at its start (19 to 22 in four templates, 18 to 21 in three, 20 to 23 in `Hero07.ptx`), followed by the item's type as a `u16` 8 bytes in (5171 the potions, 5633 the torch).

**The inventory** `0xC8`, **confirmed** 2026-10-22 (#486) on the 8 templates (`amh_dump.py --items`) and by the original's driver (#448): after a `u32` (30 in every template), a block per item the hero carries, in its backpack or on it.
A block holds the item's type as a `u32`, the marker `0xFEEDF00D`, the object's id as a `u16` 10 bytes after the marker, the type again as a `u32` 14 bytes after it, and the item's record, `0x82` bytes 252 bytes after the marker, which begins with the type too; the record's layout is that of `Weapon.pak +0x80…+0x101` (docs/re/items.md, "Defects the community reports"), and a block runs to the next block's type, 398 to 486 bytes.
The templates hold 46 items: the backpack's four each, and 14 that the backpack does not name, which the hero wears and holds, one to three a template (one in `Hero02.ptx`, `Hero04.ptx` and `Hero06.ptx`, three in `Hero07.ptx`, two in the others).
Every template's hero holds a weapon, the Vampiress's a shield too, and five wear an item more, a cosmetic model whose values do not count (`items.pak +0x03` bit 1; the hair of two, the Dwarf's goggles, the Mage's cowl, the Gladiator's belt; docs/re/items.md); the records equal their types' `Weapon.pak +0x80…+0x101` but for the Gladiator's belt, 4054, of level 27 in the template (the words 8 and 10 from `+4`).
In the original's new game the Seraphim wears the template's two by their ids, 17 in the equipment slot 0 and 18, the weapon, in slot 13, the right hand (docs/re/equipment.md), and the item maker makes neither.
The Seraphim's weapon is the record 7901: physical damage 21 to 23, `A` 5 and `D` 2, and two bonus slots (the effects 811 and 812, of the values 14 and 2).
The earlier names, the skill table and the combat arts, were wrong.
The saves ship with the GOG installation, but the game rewrites `save/` and deletes them on request, so they are not in the manifest (§13): the converter reads only the templates, and `amh_dump.py --saves` checks the saves of a clean installation.
Implemented: `opensacred_formats::amh` (`Amh::items`, `Amh::backpack`, `Amh::arts`); `opensacred-convert heroes` writes each template with its sections inflated to `heroes/` and indexes level, class, the items the hero wears, their records' words from `+4`, and the combat arts of its creature section (#506) in `heroes.ron`.
The sections stay whole until their layout is recovered.
There is no encryption and there are no checksums (the "export" `.PAK` saves do have 48 bytes of checksums according to gonX, but the game does not check them).
The 304×360 hero portrait is written as a JPEG via IJL (according to sacred-bild).
The −0x48 delta for Sacred Plus saves (version 0x1007) is not needed for Gold: Gold 2.28 writes only the 0x101B layout.

**The game's saves**, `save/GAMEnn.PAK`, which the menu's Save Game writes (the list shows them in the order of `nn`), are another container: they begin `"AMS", 0x1B`, and the name typed in the list is a C string at `0x500` (observed 2026-10-21 on four saves the original wrote, #450; `<empty>probe1` when the name was typed after the field's `<empty>`); the rest is not read.

For the new engine: original `.pax` saves are not supported (by design).
The `Templates/Hero*.ptx` templates are converted into postcard hero files with their sections inflated and whole, indexed by level, class, the items the hero wears and its combat arts in `heroes.ron`; the rest of the starting presets (the stats) waits for the layout of 0xC7.
The section layouts above are needed only for this and as a source of knowledge about the hero fields.

## 10. Sound, music, video

### 10.1 sndProfiles.pak (SPF)

8,192 slots × 184 B, 175 occupied: `char name[32]` (`music`, `footsteps_hero_male`, `weapon_bow`, `gladiator_snd_1`, …), `u32 1`, then 74 `u16`: **10 zeros and 8 groups of 8 sample ids** (confirmed on every record 2026-10-17 by `tools/analysis/formats/sound_dump.py`); a group holds its random variants repeated in turn to fill its 8 slots (3 variants: `a b c a b c a b`), or nothing; the directory entries have type 35, and a slot without a name is all zeros; 2,355 distinct ids in total, in the range 100…21,613.
Slot allocation (confirmed on file, `tools/analysis/formats/spf_names.py`): 1 `music` (64 ids), 2 `jingle_extension`, 10 `atmospot_*`, 100–114 general (death, rain, 13 kinds of footsteps), 500–516 weapons, 600–611 objects, 1000–1016 hero voices (24 ids each), 2002–2146 creatures and NPCs.
The group indices within a record are named by the client's `SND_*` table, eight per kind of profile (creatures, weapons, footsteps, objects, music, generic; confirmed by code 2026-10-17, [docs/re/sound.md](re/sound.md)): a profile = an "event × variants" matrix.
The variant selection and priority logic from the `cMSS` layer is carried over into the new engine's sound system (Kira; docs/06, §2).

### 10.2 sound.pak

Confirmed on file 2026-10-16 by the `tools/analysis/formats/snd_ls.py` script; matches sacred-bild's data and extends it.
The original (`cMSS`) holds a single `FILE*` and reads samples on demand (according to sacred-bild).

- **Container** `SND` v1: a 0x100 B header (`u32 count = 50000`), a directory of 50,000 × `{u32 flags, u32 offset, u32 size}`; 6,598 slots are occupied (ids 2…21,616), sample id = slot number.
  The data follows contiguously right after the directory (from 600,256) to the end of the file, with no gaps, overlaps or shared data.
- **`flags`**: 32 = RIFF WAV (3,194 records), 33 = MP3 without a container (3,404).
  There are no other values.
- **WAV**: 1,825 PCM (1,772 are 16-bit mono 44.1 kHz; 35 stereo 44.1 kHz; 15 mono 22.05 kHz; one record each of stereo 22.05 kHz, mono 16 kHz and 8-bit 11.025 kHz) and 1,369 IMA ADPCM (tag 0x11, 4-bit; 1,342 are mono 44.1 kHz with `blockAlign` 1,024 and 2,041 samples per block; the rest have `blockAlign` 256, 512 or 2,048).
  In all of them the RIFF size matches the record size.
  Besides `fmt ` and `data`, the chunks `fact`, `PAD `, `LIST`/`INFO`, `smpl`, `inst` occur; the reader skips unknown ones.
- **MP3**: MPEG-1 Layer III, mono, 44.1 kHz; 48 kbit/s (2,546), 64 (766), 128 (72), 96 (20).
  3,331 records start directly with the sync word; 73 have one frame's worth of zeros (208 or 417 B) before the first frame, so the decoder must search for the sync word.
  `opensacred_formats::sound` gives an MP3 record as stored, with the count of those zeros; `opensacred-convert sound` drops them as it writes the sample, a repair of the data (#354, docs/decisions/2026-10-20-mp3-leading-zeros.md).
- **Duration**: PCM ≈0.7 h, ADPCM ≈0.8 h, MP3 ≈4.4 h (estimated from the bitrate of the first frame).
- **Link to the profiles** (§10.1): of the 2,355 profile ids, 2,253 are in the pak, all of them WAV; of the 102 missing, 88 are the music files of `mp3/` (§10.3) that the profiles `music`, `jingle_extension` and `atmospot_waterfall` name, and 14 of `soldier_snd` (`SOUND_FX_THIEF_FLEE*`, `…FOLLOW*`) have no sound at all (the original apparently skips them silently; hypothesis).
  The profiles reference none of the 3,404 MP3s (ids 16,300…21,616) and none of 941 WAVs.
  Hypothesis: the MP3s are dialog and quest voice-over triggered by scripts; this would also explain the string `stereo mp3 on 3d channel!` in `playSFX`.

- **Names**: `Sacred.exe` names the sound ids in a table in `.data` at VA `0x964870` (file offset `0x564870` in GOG 2.28): 6,869 records of 68 B `{u32 id; char name[64]}`, ASCII and zeros after the name, from id 0 `SOUND_FX_INVALID` to the last whose name starts with `SOUND_FX_`; the ids are not sorted, every name is distinct, and two ids have two names each (13,024 and 13,402).
  Every sample of the pak and every profile id is named (`SOUND_FX_ANIMAL_BAT01`, `SOUND_FX_FOOTSTEP_…`, `SOUND_FX_MUSIC_WOOD01`).
  Confirmed 2026-10-17 by `sound_dump.py --names`.
- **Goblins shout German phrases in the English version** (**community-reported**, TV Tropes; checked for #358 on 2026-10-21): the goblins' profile is slot 2031, `gobliins`, whose eight groups, `APPROACH` to `DEATH` ([docs/re/sound.md](re/sound.md)), name 49 samples, `SOUND_FX_ORC03_*` (ids 13350 to 13399 but 13386, which is `ORC02_RFLEE06`); five more, `ORC03_GROUP01` to `05` (13400 to 13402, 13419, 13420), are in no profile.
  No name of the 6,869 carries a language, the pak holds one sample per id, and none of the 54 has its data equal to another sample's, so the data shows no English counterpart (**confirmed**); whether these samples are German cannot be told from their names (**hypothesis**: they are, as reported).
  The sets' uses (counted on the profiles 2026-10-22, #402): `ORC01` is the champion orc's (`orc_champ`, 2029), `ORC04` the big orc's (`orc_big`, 2034), `ORC05` the ogres' (2030, 2045) and another orc's (2046), so that a goblin given one of them would speak with that creature's voice.
  `ORC02` (13300 to 13386, 51 samples, all in `sound.pak`) has the groups of `ORC03`, `RFLEE` among them, which no other set has; 45 of its samples are in no profile, its six `RFLEE` in the big orc's group 6 (`FLEE`).
  Its timbre does not suit the goblins (the maintainer's hearing), and no other set is theirs: the goblins keep the original's samples, as the English 2.28 plays them, and the data needs no repair (the maintainer's choice, 2026-10-22, #402).

Consequence for the converter: PCM and IMA ADPCM → FLAC, MP3 records are copied as is, like the music (§10.3).
**Implemented 2026-10-17:** `opensacred_formats::sound` (`SoundPak`, `Profiles`) reads every sample and profile and matches `sound_dump.py` on every record; `opensacred-convert sound` writes the 3,194 WAVs as FLAC, encoded by FFmpeg as an external program with bit-exact flags (lossless: a test decodes a PCM and an IMA ADPCM sample through FFmpeg from the original and from the FLAC and compares the PCM; a rerun writes the same bytes), copies the 3,404 MP3 samples, without the zero bytes before their first frame (a repair since #354; until then the reader dropped them), and the 173 music files (from their first frame since #367, §10.3), and writes `sound/sound.ron` with the samples, the 175 profiles (8 groups of variants each), the music and the 6,869 names of `Sacred.exe` (`opensacred_formats::sound::names`, matching `sound_dump.py --names`): 754 MB in 18 s.

### 10.3 MP3 music and WMV video

- `mp3/`: 173 files, 399.5 MB (`MUSIC_*`, `JINGLE_*`, `ATMOSPOT_*`, `Meet*`); the original plays them as Miles streams via `mssmp3.asi` (`AIL_open_stream` with `.\MP3\<name>.mp3`; `AIL_set_stream_volume_pan` is the most frequent AIL function, 27 calls).
  In the new engine Symphonia decodes the MP3s, and Kira provides several simultaneous streams with smooth transitions (hypothesis based on the `MUSIC%2d handle=…` log).
  154 of the files hold bytes before their first frame (**confirmed** on every file 2026-10-20, #367): 152 zeros (835 bytes in 128 files, 417 in 16, 522 in 8), and `JINGLE_FIGHT_EASY14.mp3` and `…EASY15.mp3` the name of the jingle before them (`…EASY13.mp3`, `…EASY14.mp3`) then zeros, 1,347 and 1,859 bytes in all; 19 start with the sync word.
  The original's decoder searches for the sync word; `opensacred-convert sound` writes each file from its first frame (`opensacred_formats::sound::mp3_first_frame`: the first valid MPEG-1 Layer III header whose frame ends the file or is followed by another), a repair of the data (docs/decisions/2026-10-20-music-junk.md).
  A music file has a sound id (§10.2) by its name: `MUSIC_WOOD01.mp3` is `SOUND_FX_MUSIC_WOOD01`, id 6,560; 171 of the 173 files match (not `ATMOSPOT_WATERFALL`, `ATMO_VILLAGE_SIEGE_MILITARY`), and `sound.ron` records the id of each.
  The `music` profile names the music by these ids: groups 0 and 1 `MUSIC_WOOD01`–`03`, group 2 `MUSIC_FIGHT01`–`04`, 3 `JINGLE_DANGER01`–`08`, 4 `JINGLE_FIGHT_NORMAL01`–`08`, 5 `…HARD01`–`08`, 6 `JINGLE_FIGHT_OVER01`–`08`, 7 `JINGLE_FIGHT_EASY01`–`08` (confirmed 2026-10-17); a sector's music is its keyx `+0x2D8` (§5.1), and group 0 (`PEACE`) plays when a track ends in peace by day in a sector without music of its own (**confirmed** by code 2026-10-18, [docs/re/sound.md](re/sound.md), "The region's music"); hypothesis: the script command `PlayMusic` (type 127, a name and a flag: `ATMO_WOOD` with 0, `SOUND_FX_MUSIC_FIGHT07` with 1) plays the rest.
  The ambient loops (`ATMO_*`) follow the nature of the cell under the hero, by a switch in the client ([docs/re/sound.md](re/sound.md)).
- `movie/`: 10 WMV, 113.3 MB (Intro 26.2; Extro 33.3; introuw 21.2; extrouw 10.4; act1–4; ascaron; oem); the exe lists 16 names (act5–8, PUBLISHER, Take2 are not shipped, so a missing file must be skipped silently).
  The player is DirectShow Multimedia Streaming (`CLSID_AMMultiMediaStream`, `IAMMultiMediaStream`, `IDirectDrawMediaStream` → DirectDraw surface); the Excel ProgID/IDispatch in the exe is a balancing tool, not video.
  Codecs checked with `ffprobe` 2026-10-16: the video of all cutscenes is WMV3, 640×480, 25 fps (`oem.wmv`: 30); the stereo audio is WMAv2 44.1 kHz in `Intro`, `Extro`, `act1`, `ascaron`, `oem` and WMA Pro in `act2–4` (44.1 kHz), `introuw`, `extrouw` (48 kHz).
  FFmpeg decodes both audio codecs.
  Total duration ≈7.9 min.
  On a 4K screen a cutscene is stretched 4.5 times, so upscaling `Intro` and `Extro` in M7 makes sense.
  The `COMPAT_VIDEO` key (GOG sets it to 1) affects only the cutscene display path in the original (a medium-confidence hypothesis from static analysis, docs/01 §4.5); it does not affect file reading.
  In the new engine the cutscenes are converted to AV1 + Opus and played via rav1d (docs/06, §4); they can be skipped.
  **Implemented 2026-10-17:** `opensacred-convert video` encodes the 10 WMVs with FFmpeg as an external program (SVT-AV1 at CRF 30, preset 6; libopus at 128 kbit/s) into `video/<name>.webm` and lists them in `video/video.ron`; a test checks that FFmpeg reads a converted cutscene as AV1 and Opus.
  With several threads per cutscene, SVT-AV1 output varies from run to run; the converter therefore encodes each cutscene on one thread (`-svtav1-params lp=1`, the cutscenes in parallel; 100 s for all 10 instead of 23 s) and asks the WebM muxer for no random ids (`-fflags +bitexact`), so two runs give identical files (checked 2026-10-17).
- All paks, WMV, MP3, TTF, BMP are the rights holder's content: the converter reads them from the user's own installation, verifying it against the SHA-256 manifest; neither the source nor the converted files are published.

## 11. Fonts and UI assets

- Fonts: `font/ANTQS__.TTF` ("AntiquaSSK", 63 KB) and `font/CAROLING.TTF` ("Carolingia", 39 KB); the original rasterizes them via GDI into 256² pages.
  The new engine uses the same TTFs directly via cosmic-text, plus Noto (OFL) for missing glyphs.
- UI bitmaps: 1024×1024 `UI_*.TGA` in format 6 (A8R8G8B8, §3.1), the cursor `MOUSE_MAIN.TGA`; the `cUI_*` widget layout is defined in code, there are no layout files.
  The new interface is designed from scratch; the original's textures are used as skins.
- Loading screens: `pak/loading0/1.bmp`, `loadgame.bmp`, `savegame.bmp`, `LoadingUW00/01.bmp`: BMP 1024×768, 24-bit, uncompressed, rows from the bottom; three files carry 2 bytes after the pixels.
  **Implemented 2026-10-17:** `opensacred_formats::bmp` decodes them, `opensacred-convert ui` writes `ui/screens/*.png` (a test compares every pixel with FFmpeg's decoding) and copies the two fonts to `ui/fonts/` and `credits.txt`, `credits2.txt` (ASCII with CRLF) to `ui/texts/`.

## 12. Conversion: targets, order, checks

### 12.1 Targets per format

| Source | Target | Check |
|---|---|---|
| TEX (`texture*.pak`) | PNG RGBA8 (`textures/<pak>/`); KTX2 with mipmaps with the HD packs (M7); `Texture.TMP` is a cache and is skipped | pixel cross-check against the `tools/analysis/formats/tex_decode.py` prototype |
| Tiles and MIX atlases | PNG atlases + the UV table of the 18 diamonds | rendering a test sector |
| MDL + GRN (`models*.pak`) | `.glb`: meshes, skeletons, weights, materials, clips; `Models.tmp` is a cache and is skipped | glTF-Validator, Blender, reference poses |
| Sectors (`sectors.wldx`, `sectors.keyx`), `Floor.PAK`, `Static.PAK`, `tiles.pak`, `mixed.pak` | `world/world.ron`, one file per sector (`sectors/<x>-<y>.sector`: postcard in a zlib stream, format version 8) and `sprites.bin` | loading all 6,050 sectors |
| `Items.pak`, `Items03.pak`, `Creature.pak`, `Weapon.pak`, `MOTIONS.PAK`, `Triggers.PAK`, `bin/*.bin`, the combat arts of `Sacred.exe` | RON tables with a typed schema under `tables/` | comparing the fields with the source |
| `global.res` | Fluent (`.ftl`) | 23,123 strings in place |
| NetScript | one postcard file per tree with a `netscript.ron` index; listings on request | disassembler without remainder on the 20 trees |
| `sndProfiles.pak`, `sound.pak` | profiles in RON (`sound.ron`); WAV (PCM, IMA ADPCM) → FLAC; MP3 records unchanged | durations and spot listening |
| MP3 | unchanged | playback |
| WMV | AV1 + Opus in WebM via FFmpeg | playback |
| TTF, BMP | TTF unchanged; BMP → PNG | viewing |
| `Templates/*.ptx` | postcard hero files with the sections whole and `heroes.ron` with the items worn and the combat arts; the rest of the starting presets later | reading back |

### 12.2 Structure

* `opensacred-formats`: reading the original formats (safe slice parsing, no `unsafe`).
* `opensacred-data`: open format types shared by the converter and the game.
* `opensacred-convert`: a CLI with the subcommands `verify`, `textures`, `models`, `world`, `sound`, `text`, `video`, `ui`, `netscript`, `tables`, `heroes`, `all` and `coverage`; parallel (rayon), deterministic.
  Each full run of a command records `manifest/<command>.ron` under the output directory (the SHA-256 of the converter, of the installation files it read and of the files it wrote), and `all` skips a command whose record still holds.
* Outputs go to `assets/` (not committed).
  HD packs go to `assets-hd/` with the same relative paths.

### 12.3 Order

The order follows the vertical slice (PLAN.md, §9): textures and tiles → models and animations of the hero and of the starting region's creatures → the starting region's sectors → sound → the remaining tables, texts, scripts → the whole world → video.

### 12.4 Tests

* Synthetic mini files in the repository, for CI.
* Tests on the real installation are marked and run only locally.
* The run manifests (§12.2) hold the SHA-256 of every output, so a changed output shows up in them.
* The Python prototypes in `tools/analysis/` serve as the reference when porting the parsing logic to Rust.

### 12.5 Estimate

The whole converter: 80–160 agent-hours (milestone M1); the breakdown by format is in `docs/07-roadmap-estimates.md`, M1.

## 13. Specifics of the GOG release

* 29 `QuestCode.bin` and `QuestPoolCode.bin` files in the GOG installation have zero size, meaning "no script"; the converter treats an empty file and a missing file the same way.
* `Settings.cfg` in a clean GOG installation: `GFX32 : 1`, `GFX_LIMIT128 : 1`, `COMPAT_VIDEO : 1`, `GFXSTARTUP : LOADING0.BMP`, volumes 64, `VIOLENCE : 0`.
  The game rewrites the file on launch (adds `AUTOSAVE`, `NETWORK_*` and other keys, changes `FULLSCREEN`, `VIOLENCE`), so the file is not in the manifest.
  For the new engine the values matter only as a hint for default settings.
* `EULA.txt` (the GOG user agreement), `license.rtf` (the publisher's license), `Gold.jpg`, `drache.jpg` are not game data; the converter skips them.
* The SHA-256 manifest is `docs/manifest-gog-2.28.txt`: 319 game files (≈2.54 GiB), only those the game never writes.
  Not in it: `Settings.cfg` and `Gameserver.cfg`, which the game rewrites on launch; the saves of `save/`, among them the two level-29 heroes that ship with the installation, which the game lists for import and deletes on request; the 20 `DefPos.bin` caches, which the client rebuilds (§7.1); the empty script files and the non-game files above.
  Removed from the manifest on 2026-10-17: after a launch from the installation and the two shipped heroes deleted in the game, the remaining 319 files still matched; `sha256sum` format, paths relative to the installation root.
  Check: `cd OriginalGame && sha256sum -c ../docs/manifest-gog-2.28.txt`.
  As of 2026-10-16 the manifest matches a clean GOG 2.28 installation (395 files: 377 game files plus 18 GOG and installer files, such as PDF manuals, `unins*`, `goggame-*`).
  The converter ignores the files outside the manifest.

## Open questions

Closed and not in the list: where the screen resolution is stored (nowhere: it is hard-coded to 1024×768) and the purpose of the `BINARY #107` resource (an embedded copy of `global.res`, docs/01 §4.6), both while preparing the plan; the channel order of format 6 (§3.1), the container and codecs of `sound.pak` (§10.2) and the codecs of the WMV cutscenes (§10.3), on 2026-10-16; the UV table of the 18 diamonds on real sheets (§5.6) and the image of the type 9/25 statics (the `mixed.pak` sprite, §6.1), on 2026-10-17; the cell's low nibbles 3 and 5 to 10 (§5.2), on 2026-10-18; how the models get their height on hills (the second heights, §5.3), on 2026-10-19; where the client draws a `CreateObj`'s model and in which cell's list it puts the object (`0x20` at height 0 moved by the object's height, its `0x04` cell's list, §7.3), on 2026-10-22.

1. The value 15 of the cell's low nibble of `+0x1F`; the 256 bytes that end a sector's stream (§5.2); the cell statistics of `sector_stats.py`, which reads the cells eight late, are to be measured again where they matter.
2. The meaning of the Static.PAK flags other than those of §5.5 and §6.1 (`0x10`, `0x20`, `0x40`, `0x80`, `0x290`, `0x400`, `0x800`) and of its bytes `+0x16` and `+0x34…+0x3F`.
   Closed on 2026-10-17: the position (screen pixels at `+0x0E`/`+0x12`, community-reported; the "`z`" was the high half of `x`) and how Floor.PAK and Static.PAK attach to the sectors (the cells' chains, §5.5).
3. Granny animation blending parameters (ADSR envelopes, period, the events of the MotionDesc float table other than the attack's hit fractions, §4.1); needed only for the approximate transfer into the Bevy animation graph.
4. Semantics of the NetScript record types without a keyword (39; the fields `a`, `b` of type 40; the five cases of type 41; 42), the single unresolvable reference `res:174242`.
   Closed on 2026-10-17: the operand tag widths of the Windows build and the names of 43, 44, 111, 122, 123 (§7.1, §7.3).
5. Hero template fields (section 0xC7) to the extent needed for the starting presets.
   Closed on 2026-10-22: the inventory's blocks of section 0xC8, the items the hero wears and holds (§9, #486).
6. The meaning of the X fields in the TEX (9,406/27,100), MDL (13,268 in `models.pak`, also in the `Models.tmp` and `MOTIONS.PAK` headers; 19,897 in `models03.pak`), CIF (0x3C3D5) and WPN (0x1A3717) headers; probably CRCs/timestamps.
7. How the 719 of the 875 bipeds that face −Y in their bind pose come to face their way (§4.5): the client turns a model's `x` along its direction (docs/re/drawing.md, "The models' turn"), which suits the 148 that face +X, among them `SERAPHIM.GRN`.
8. What the ground's 636 cells of flag bit 2 that no door names are (§5.7).
   Closed on 2026-10-22: the side the client turns a model's +z to, the doors' leaves swinging onto the ground as the game's do (§5.7, #430).

## Sources

Scripts ([tools/README.md](../tools/README.md); run with `python3 -I` on the user's own installation; paths under `tools/analysis/`):

- `formats/tex_decode.py`, `formats/tex_fmt6.py` — TEX formats 4 and 6 (§3.1).
- `formats/grn_dump.py`, `formats/mhp_probe.py`, `formats/mhp_vs_mdt.py` — MDL/MDT, GRN, MOTIONS.PAK (§4.1–4.2).
- `formats/mixed_dump.py` — `items.pak` and `mixed.pak` (§3.3, §6.1).
- `world/world_dump.py`, `world/sector_stats.py`, `world/static_stats.py`, `world/uv_table.py` — keyx, wldx, tiles, Static.PAK, Floor.PAK, the UV table (§5).
- `formats/tables_dump.py`, `formats/bin_tables_dump.py` — Creature, Weapon, MOTIONS, Triggers, Items03 and the `bin/` tables (§5.5, §6).
- `formats/arts_dump.py` — the combat arts' tables in `Sacred.exe` (§6.5).
- `scripts/netscript_census.py`, `scripts/netscript_dump.py`, `scripts/kwtab.py` — NetScript (§7).
- `formats/global_res_dump.py`, `formats/global_res_hash.py` — `global.res` and the hash (§8).
- `formats/amh_dump.py` — AMH templates, and saves with `--saves` (§9).
- `formats/snd_ls.py`, `formats/sound_dump.py`, `formats/spf_names.py` — `sound.pak`, `sndProfiles.pak`, the sound names (§10).
- `pe/granny_args.py` — the Granny call sites (§4.3).

Plan documents this one relies on: docs/01-binary-analysis.md (§4.5–4.6: `COMPAT_VIDEO`, `BINARY #107`), docs/05-renderer-vulkan-4k.md, docs/06-platform-backends.md.

External projects and documents this one relies on (without copying their content):

- kirides/sacred-bild (GPL-3.0) — `docs/RE_NOTES.md`, `src/game/sacred_addr.h`: https://github.com/kirides/sacred-bild
- openheilig/research (CC BY-SA 4.0), /engine, /tools (MIT) — `formats/granny-grn.md`, `formats/script-bytecode.md`, `formats/pak-containers.md`: https://github.com/openheilig/research
- bssth/sacred-sdk (MIT) — `ports/engine/sacred_hash.h`, `ports/herosave/hero_save.h`, `re/py/funkcode_tags.py`, `lua_bake_opcodes.inc`: https://github.com/bssth/sacred-sdk
- ancaria-dev/grn2glb (MIT; VTex codec LGPL-2.1+): https://github.com/ancaria-dev/grn2glb ; karolak6612/open-grn (MIT): https://github.com/karolak6612/open-grn
- Aytackydln/SacredEngineRemake (no license; not used): https://github.com/Aytackydln/SacredEngineRemake
- MairwunNx/SacredUtils (Apache-2.0, Balance/Creature/Weapon/global.res editors): https://github.com/MairwunNx/SacredUtils ; gonX/sacred-savepak-system: https://github.com/gonX/sacred-savepak-system
- wine `include/d3d.h`, `include/d3dtypes.h` (D3D7 constants for the render contract): https://raw.githubusercontent.com/wine-mirror/wine/master/include/d3dtypes.h
- FFmpeg `libavcodec/bink.c` (origin of the VTex codec): https://raw.githubusercontent.com/FFmpeg/FFmpeg/master/libavcodec/bink.c
