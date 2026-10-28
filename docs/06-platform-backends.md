# Platforms and subsystems: sound, input, video, files, saves, scripts

## Summary

* **Platforms:** Windows 11 x64 is the main one; Linux is second, with the same functionality; Android comes after version 1.0 and by a separate decision; the browser version was dropped on 2026-10-23.
  Everything builds from one codebase on Bevy and wgpu.
* **Sound:** Kira through `bevy_kira_audio`; the sound and the music as Opus in Ogg, decoded through libopus (#701).
  Sound events are taken from the original's 175 profiles, converted to RON.
* **Input:** keyboard and mouse as in the original plus a gamepad; key remapping through `leafwing-input-manager`; touch controls only for Android.
* **Video:** the 10 WMV movies are converted to AV1 + Opus; playback through rav1d; movies can be skipped.
  The conversion is done (`opensacred-convert video`); playback comes in M5.
* **Saves and settings:** settings are TOML through serde and `toml_edit` (done); saves will be our own format on serde (M6); the original saves are not supported.
* **Scripts:** the converter translates the NetScript bytecode into Luau, which the game runs through mlua (PLAN.md ADR-13, #720).
* **Multiplayer after version 1.0** (PLAN.md ADR-09): until then no networking, no server, no lobby; no Steam or GOG Galaxy integration is needed.

## 1. Platforms

| Platform | Build | Specifics | When | Agent-hours (estimate) |
|---|---|---|---|---|
| Windows 11 x64 | `x86_64-pc-windows-msvc`, Build Tools 2026 | the main one; Vulkan, DX12 as fallback; the game directory with `assets/` next to the exe | M2–M6 | included in the milestones |
| Linux x86-64 | `x86_64-unknown-linux-gnu` | Vulkan; XDG directories; builds in WSL2 or on Linux | after M6 | 5–10 |
| Android | `aarch64-linux-android` through cargo-ndk and Gradle | touch controls, a smaller streaming radius, ASTC textures, assets in app storage | by decision | 25–50 |

## 2. Sound

### 2.1 What the original does

* Miles Sound System 6.6g: 57 of the 62 imported `AIL_*` functions are called: 2D samples, 3D samples through the DirectSound3D provider, MP3 streams through `mssmp3.asi`, the IMA ADPCM decoder.
* `sndProfiles.pak`: 175 profiles map `SND_*` events (footsteps, hits, doors, chests, hero voices, creatures) to sample groups: 2,355 identifiers in total, 2,253 of which are present in `sound.pak`.
* `sound.pak`: 6,598 samples: 3,194 RIFF WAV (PCM and IMA ADPCM) and 3,404 MP3; the profiles do not reference the MP3s (hypothesis: dialogue voice-over from the scripts).
  Details: docs/02, §10.2.
* Music: 173 MP3s in `mp3/`: background ambiences `ATMO_*`, region music `MUSIC_*`, short `JINGLE_*` stingers for combat, danger and quests.
* Details: docs/02, §10; the sound logic recovered so far: `docs/re/sound.md`.

### 2.2 New implementation

| Part | Decision |
|---|---|
| Sound engine | Kira 0.12 through `bevy_kira_audio` 0.27 |
| Decoders | Symphonia 0.6's Ogg demuxer and libopus (#701), the one C library of the sound, decode the game's Opus; the converter decodes the WAVs and MP3s through FFmpeg |
| Sound effects and speech | `sound.pak`: WAV → FLAC masters (lossless from PCM and IMA ADPCM), the MP3 entries decoded into FLAC masters (#697); the game's Opus in Ogg built from them |
| Music | FLAC masters decoded from the MP3s, the game's Opus in Ogg; `bevy_kira_audio` does not stream, so only the track playing is loaded (≈85 MB decoded for four minutes) |
| Events | profiles → `sound/sound.ron`, beside the samples, the music and the original's sound names: event → sample group → random selection rules |
| Spatial sound | panning and distance attenuation from the hero; no HRTF at the start |
| Music system | region zones, switching to combat music, event stingers; smooth transitions |
| Volumes | music, effects, voices, as in the original's settings |

Estimate: 10–20 agent-hours (M2 and M5).

## 3. Input

* **Keyboard and mouse:** click to move, attack, the quick skill bar, the original's hotkeys as the default layout.
* **Gamepad** (new): stick movement, skills on buttons, a radial menu for the skill bar; through gilrs inside Bevy.
  `bevy_gilrs` is linked; the game reads no gamepad yet.
* **Remapping:** `leafwing-input-manager` 0.22; the layout is stored in the settings.
* **Touch input:** only for Android, after the platform decision.
* IME for text input (the hero's name), by means of Bevy and winit.

Estimate: 6–12 agent-hours.

## 4. Video

| Step | Decision |
|---|---|
| Conversion | FFmpeg as an external program: WMV → FFV1 (video) + FLAC (audio) in Matroska as the master (#698), built into AV1 (video) + Opus (audio) in WebM for the game; the original resolution and frame rate (640×480, 25 fps); the source audio is WMAv2 or WMA Pro, both decoded by FFmpeg |
| Playback | a WebM demuxer (`matroska-demuxer`) + the rav1d AV1 decoder → a Bevy texture; audio through Kira |
| Controls | skip with a key; a "do not show movies" setting |

Estimate: 6–12 agent-hours (M5).
Movies are not needed to complete the game, so this is a low priority.

## 5. Files and assets

* **Layout:** `assets-src/` holds the converter's masters and `assets/` the game's assets built from them, in ZIP archives by kind with a loose overlay (#694, #699); `assets-hd/` holds HD packs and mods in the masters' formats with the same relative paths, which stage B builds into archives that override the base ones by path (M7).
* **Loading:** the sectors are read and built on Bevy's async compute pool, the models and images through its AssetServer; the sectors and their creatures come in as the hero approaches.
* **Names:** the converter normalizes paths to lowercase and forward slashes, so file system case does not matter.
* **Installation check:** the command `opensacred-convert verify` cross-checks `OriginalGame/` against the SHA-256 manifest of the GOG installation (`docs/manifest-gog-2.28.txt`, 319 files, excluding the files the game writes: `Settings.cfg`, `Gameserver.cfg`, `save/`, the `DefPos.bin` caches); the conversions do not check it themselves.
* **Empty quest files:** 29 `QuestCode.bin` and `QuestPoolCode.bin` files (9 and 20) in the GOG release are zero-sized and are not in the manifest; the converter treats an empty file and a missing file alike as "no script".
* **Packaging a build for personal use:** the exe + `assets/`, its archives, in one directory.

## 6. Saves and settings

* **Saves:** our own format: a `serde` structure with a version number and migrations.
  RON in debug (human-readable), postcard with zstd compression in release.
  Autosave on a timer, like `AUTOSAVEDELAY` in the original.
* **Directories:** `%APPDATA%\OpenSacred` on Windows, `$XDG_CONFIG_HOME/OpenSacred` (`~/.config/OpenSacred`) on Linux, app storage on Android.
  Since 2026-10-19 the game takes them from the environment (`APPDATA`, `XDG_CONFIG_HOME`, `HOME`), which needs no crate; `directories` stays the option when Android needs its app storage.
  The log goes to the state directory: `%LOCALAPPDATA%\OpenSacred\logs\` on Windows, `$XDG_STATE_HOME/OpenSacred` (`~/.local/state/OpenSacred`) on Linux (docs/10 §5).
* **Settings:** TOML: graphics (resolution, window mode, backend, zoom, UI scale, HD packs), sound (volumes), controls (layouts), gameplay.
  Done ahead of M6 on 2026-10-19 (#167): `settings.toml` with its display section, `mode` (`borderless`, the default: borderless full screen at the monitor's resolution; or `windowed`) and the window's `width` and `height` in physical pixels; F11 switches the mode and writes it at once (docs/10 §3).
  Since #169 and #170 a `[debug]` section keeps whether the overlay (F3) and the log (F4) show.
  The game writes only the key it changes, through `toml_edit`, so the user's comments, order and unknown keys stay; a file that does not read is reported, the defaults are taken, and it is never written; an unknown key is reported.
  `--settings FILE` reads and writes another file, `--windowed`, `--borderless`, `--width` and `--height` override it for one run, and `--screenshot`, `--record` and `--benchmark` run in a window of the given size.
  The original's gameplay options are carried over by meaning: `AUTOTRACKENEMY`, `SHOWPOTIONS`, `PICKUPAUTO`, `NIGHT_DARKNESS`, `UNIQUE_COLOR`, `SHOW_ENEMYINFO`; `CHAT_*` wait for multiplayer, after version 1.0.
* **The original saves** (`.pax`) are not supported.
  The hero templates `Templates/*.ptx` are converted into starter kits for character creation.

Estimate: 8–15 agent-hours (M6).

## 7. Localization

* `global.res` (UTF-16LE, 23,123 strings) → Fluent (`.ftl`) with keys from the hash identifiers; the original names are known for 56% of the strings, and the rest get a key of the form `id-<number>`.
* English is the only language of the GOG release.
  Other languages can be added as Fluent files; third-party translations are for personal use only.
* Fonts: the original TTFs (the menus use the interface font) + Noto (OFL) for Cyrillic and other characters, later.
  The debug overlay, the log, the console and the keys' help use Bevy's embedded Fira Mono (OFL).

Estimate: 3–6 agent-hours (M1 and M5).

## 8. NetScript scripts

### 8.1 What is known

* 20 script directories: `bin/TYPE_NPC_*` (8 classes) and `bin/NetScriptCamp` are the Sacred campaign; `bin/Addon/*` is Underworld (9 identical trees); `bin/NetScript` and `bin/Addon/NetScript` are multiplayer, converted with the rest and run once multiplayer comes, after version 1.0.
* Records of the form "length u16, type u16, data"; the type space is 141, of which 113 occur in the release; 45 types cover 95% of the campaign records, 68 cover 99%.
* The interpreter is in the client; the compiler's keyword resolvers in `.text`, with their strings in `.data`, name 135 of the 141 types.
* Details: `docs/02-data-formats.md`, §7, and `docs/re/netscript.md`.

### 8.2 Implementation

1. **Converter:** done 2026-10-17: the disassembler in `opensacred-formats` → a faithful IR, one postcard file per tree (`netscript/<tree>.nsc`), with a readable listing on request (`--listing`).
2. **Control flow and record type semantics** are recovered by analyzing the client in Ghidra (the control flow first, #722) and described in `docs/re/netscript.md`, starting with the most frequent types; each type gets a test on real records.
3. **The translator** in `opensacred-convert` (#723): the IR → Luau sources, a file per quest, sector, region and start script, and their bytecode; a record becomes a call of the scripts' API.
   With the converter's two stages (#694) the sources are the masters and the bytecode the built assets.
4. **The runtime and the API:** `opensacred-scripts` (#725) runs the bytecode through mlua with Luau, sandboxed, a call bounded by an interrupt; the quests' state, the variables and the dialogues' table are `opensacred-rules`' (#724) and part of the save; `opensacred-game` implements the API (#726).
5. **Modding** (after 1.0): new quests in Luau on the same API, checked against a definition file of it.

Estimate: 40–80 agent-hours (M1 and M4); the main source of content uncertainty.

## 9. What we do not do

| Original subsystem | Decision |
|---|---|
| TinCat 2 networking, lobby, LAN announcement, `GameServer.exe` | not before version 1.0; multiplayer after it (PLAN.md ADR-09, §13) |
| Loading the original formats in the game | only through the converter |
| Compatibility with `.pax` saves | no |
| The original's Miles, amstream, IJL, libxml2, unicows, CRT and STLport | replaced by the Rust ecosystem or not needed |
| Steam and GOG Galaxy integration | not needed |
| macOS, iOS | out of scope |

## Open questions

1. Whether Android is needed: a decision after version 1.0.

Closed on 2026-10-16: the container and codecs of `sound.pak` (docs/02, §10.2), the codecs of the WMV movies (docs/02, §10.3).

## Sources

* The original's platform dependencies: docs/01, §1 and §4.5; NetScript, the string hash, AMH: docs/02, §7–9.
* Crate versions: crates.io as of 2026-10-16.
