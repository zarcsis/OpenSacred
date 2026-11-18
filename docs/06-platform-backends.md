# Platforms and subsystems: sound, input, video, files, saves, scripts

## Summary

* **Platforms:** Windows 11 x64 is the main one; Linux is second, with the same functionality; Android comes after version 1.0 and by a separate decision; the browser version was dropped on 2026-10-23.
  Everything builds from one codebase on Bevy and wgpu.
* **Sound:** Kira through `bevy_kira_audio`; the sound and the music as Opus in Ogg, decoded through libopus (#701).
  Sound events are taken from the original's 175 profiles, converted to RON.
* **Input:** keyboard and mouse as in the original; a gamepad after version 1.0 (M8, #1097); key remapping through `controls.toml` beside the settings (#1553); touch controls only for Android.
* **Video:** the 10 WMV movies are converted to AV1 + Opus; playback through rav1d; movies can be skipped.
  The conversion (`opensacred-convert video`, then `build`) and the playback where the original plays them (#1945, #1946) are done.
* **Saves and settings:** settings are TOML through serde and `toml_edit`; the saves are our own versioned format, postcard through serde (#1095); both are done; the original saves are not supported.
* **Scripts:** the converter translates the NetScript bytecode into Luau, which the game runs through mlua (PLAN.md ADR-13, #720).
* **Multiplayer after version 1.0** (PLAN.md ADR-09): until then no networking, no server, no lobby; no Steam or GOG Galaxy integration is needed.

## 1. Platforms

| Platform | Build | Specifics | When |
|---|---|---|---|
| Windows 11 x64 | `x86_64-pc-windows-msvc`, Build Tools 2026 | the main one; Vulkan, DX12 as fallback; the assets in `assets/` of the working directory, or, for the packaged build, in `%LOCALAPPDATA%\OpenSacred\assets`, where its first start converts them (#1495, #1498) | M2–M6 |
| Linux x86-64 | `x86_64-unknown-linux-gnu` | Vulkan; XDG directories; CI builds the workspace and runs its tests on Linux (docs/03 §5), and no release is made for it yet | M8, after version 1.0 |
| Android | `aarch64-linux-android` through cargo-ndk and Gradle | touch controls, a smaller streaming radius, ASTC textures (#702), assets in app storage | M8, by decision |

## 2. Sound

### 2.1 What the original does

* Miles Sound System 6.6g: 57 of the 62 imported `AIL_*` functions are called: 2D samples, 3D samples through the DirectSound3D provider, MP3 streams through `mssmp3.asi`, the IMA ADPCM decoder.
* `sndProfiles.pak`: 175 profiles map `SND_*` events (footsteps, hits, doors, chests, hero voices, creatures) to sample groups: 2,355 identifiers in total, 2,253 of which are present in `sound.pak`.
* `sound.pak`: 6,598 samples: 3,194 RIFF WAV (PCM and IMA ADPCM) and 3,404 MP3; the profiles do not reference the MP3s, which the client plays by name: **confirmed** by code for the heroes' lines of the fight (#709), a hypothesis for the rest (dialogue voice-over from the scripts).
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
| Spatial sound | a sound's volume and pan by its place from the hero, as the original's 2D path gives them (`sfx::placed`, #731; docs/re/sound.md, "A sound's place"); no HRTF |
| Music system | the region's music and the ambient loop in turns, the fights' music and jingles, the scripts' music, crossfaded (`music`, `opensacred_rules::music`; docs/re/sound.md) |
| Volumes | music, effects, speech, as in the original's settings (`[sound]`, the options window; `volumes`, #2341) |

Done with M5 (#1093); docs/re/sound.md, "The sources: what plays, and what `opensacred-game` plays of it", lists what is not played.

## 3. Input

* **Keyboard and mouse:** click to move, attack, the quick skill bar, the original's hotkeys as the default layout.
* **Gamepad** (new): stick movement, skills on buttons, a radial menu for the skill bar; through gilrs inside Bevy.
  `bevy_gilrs` is linked; the game reads no gamepad yet.
  It comes after version 1.0, in M8 (the maintainer's decision of 2026-11-09, #1097); its layout is the project's own, the original having none, and waits for the maintainer.
* **Remapping:** the game's own module `controls` (#1553): every action and its keys in `controls.toml` beside `settings.toml`, the defaults the original's (docs/10 §9); `leafwing-input-manager` is not used, its action states adding nothing a list of keys per action lacks.
* **Touch input:** only for Android, after the platform decision.
* Text (the hero's name, the saves' names, the console) is typed through Bevy's keyboard input, by winit; the game enables no IME.

## 4. Video

| Step | Decision |
|---|---|
| Conversion | FFmpeg as an external program: WMV → FFV1 (video) + FLAC (audio) in Matroska as the master (#698), built into AV1 (video) + Opus (audio) in WebM for the game; the masters keep the original resolution and frame rate (640×480, 25 fps), and `build` cuts the frames to their content, without the files' black bars (#2413; docs/decisions/interface.md, "Movies by their content"); the source audio is WMAv2 or WMA Pro, both decoded by FFmpeg |
| Playback | symphonia's WebM demuxer + the rav1d AV1 decoder on a thread of its own → a Bevy texture; the Opus sound decoded whole by libopus, as the game's other sound, played through Kira (#1945) |
| Controls | Escape, Space or either mouse button held stops a movie; `[movies] show` off plays none by itself, as the original's `SHOWMOVIE` (docs/re/movies.md, docs/10 §3) |

Done with M5 (#1094).

## 5. Files and assets

* **Layout:** `assets-src/` holds the converter's masters and `assets/` the game's assets built from them, in ZIP archives by kind with a loose overlay (#694, #699); `assets-hd/` holds HD packs and mods in the masters' formats with the same relative paths, which stage B builds into archives that override the base ones by path (M7).
* **Loading:** the sectors are read and built on Bevy's async compute pool, the models and images through its AssetServer; the sectors and their creatures come in as the hero approaches.
* **Names:** the converter normalizes paths to lowercase and forward slashes, so file system case does not matter.
* **Installation check:** the command `opensacred-convert verify` cross-checks `OriginalGame/` against the SHA-256 manifest of the GOG installation (`docs/manifest-gog-2.28.txt`, 313 files, excluding the files the game writes: `Settings.cfg`, `Gameserver.cfg`, `save/`, the `DefPos.bin` caches; and those nothing of the project reads, docs/02 §13), taking the Steam release's four files of its own by `docs/manifest-steam-2.28.txt` (#1490); the single conversions do not check it themselves, the player's `install` does (#1497).
* **Empty quest files:** 29 `QuestCode.bin` and `QuestPoolCode.bin` files (9 and 20) in the GOG release are zero-sized and are not in the manifest; the converter treats an empty file and a missing file alike as "no script".
* **Packaging:** the Release workflow packs the game and the converter, without assets, into the player's archive (#1500; docs/03 §3.6); the game's first start converts the player's installation into `%LOCALAPPDATA%\OpenSacred\assets` (#1498).
  A build for personal use is the exe with `assets/`, its archives, in its working directory.

## 6. Saves and settings

* **Saves:** our own format (`saves`, #1095; docs/re/saves.md): a file of postcard, `saves/<name>.sav` beside the settings' file, its header with the version, which the saves' list reads alone, then its picture, the hero, the scripts' state and the world.
  A save of another version is refused: no migration is written, and the version is raised at each change of a saved structure's layout, which a test traces from the types (#2306).
  The design's RON in debug builds and zstd compression were not built.
  The saves' window loads, saves and deletes them (`saves_window`, #1990), F9 or Control+Z saves the quick save and F8 or Control+X loads it (#1988).
  The autosave as the original's: the scripts' `AutoSave` records as the main quests are done, where `[gameplay] autosave` allows it (docs/re/saves.md, #1961, #1989); the original has no timer.
  A hero exported from a game lies in `heroes/hero<slot>.hero` beside the saves: its summary, and the hero as a save keeps it (`exports`, #2551).
* **Directories:** `%APPDATA%\OpenSacred` on Windows, `$XDG_CONFIG_HOME/OpenSacred` (`~/.config/OpenSacred`) on Linux, app storage on Android.
  Since 2026-10-19 the game takes them from the environment (`APPDATA`, `XDG_CONFIG_HOME`, `HOME`), which needs no crate; `directories` stays the option when Android needs its app storage.
  The log goes to the state directory: `%LOCALAPPDATA%\OpenSacred\logs\` on Windows, `$XDG_STATE_HOME/OpenSacred` (`~/.local/state/OpenSacred`) on Linux (docs/10 §5).
* **Settings:** TOML, `settings.toml`: its sections are `[display]`, `[graphics]`, `[interface]`, `[sound]`, `[movies]`, `[gameplay]`, `[network]` and `[debug]`, and the keys are in `controls.toml` beside it (docs/10 §3 and §9 list them).
  Done ahead of M6 on 2026-10-19 (#167): `settings.toml` with its display section, `mode` (`borderless`, the default: borderless full screen at the monitor's resolution; or `windowed`) and the window's `width` and `height` in physical pixels; F11 switches the mode and writes it at once (docs/10 §3).
  Since #169 and #170 a `[debug]` section keeps whether the overlay (F7) and the log (F12) show.
  The game writes only the key it changes, through `toml_edit`, so the user's comments, order and unknown keys stay; a file that does not read is reported, the defaults are taken, and it is never written; an unknown key is reported.
  `--settings FILE` reads and writes another file, `--windowed`, `--borderless`, `--width` and `--height` override it for one run, and `--screenshot`, `--record` and `--benchmark` run in a window of the given size.
  The original's options are carried over as keys of the same meaning, their defaults the original's: the options window's fifteen (#2340; `AUTOTRACKENEMY` and `PICKUPAUTO` among them), `SHOW_ENEMYINFO`, `WARNING_LEVEL`, `SCREEN_QUAKE`, `SHOWMOVIE` and the extros' two (docs/10 §3).
  `SHOWPOTIONS` and `UNIQUE_COLOR` have no key, `NIGHT_DARKNESS` is held at 50 and `CHAT_DELAY` at 8 s (`daynight`, `info_lines`), and the rest of `CHAT_*` waits for multiplayer, after version 1.0.
* **The original's saves** (`save/GAMEnn.PAK`) and exported heroes (`save/hero*.pax`) are not read (docs/re/saves.md).
  The hero templates `Templates/*.ptx` are converted into starter kits for character creation.

The saves came with M5 (#1095) and the options window with M6 (#1096, #2340).

## 7. Localization

* `global.res` (UTF-16LE, 23,123 strings) → Fluent (`.ftl`) with keys from the hash identifiers: every string is the message `id-<number>`, its id, as the game, its tables and its scripts name a string by that hash; the original names are known for 56% of the strings (docs/02 §8).
  The words that the game shows of its own, not the original's, are Fluent files of `opensacred-game`'s `text/`, embedded in the program (#1986).
* The GOG release carries English, French and Russian, each in a language depot of its own (the store page and gogdb): English and Russian with text and voice, French with text alone, the store marking no French voices; the Steam release carries English alone.
  The project is verified on the English installation, whose `scripts/us/global.res` the converter reads; the French and Russian depots are not checked locally (docs/09-ecosystem.md, "Open questions", 3).
  Other languages can be added as Fluent files; third-party translations are for personal use only.
* Fonts: the original TTFs (the menus use the interface font).
  The debug overlay, the log, the console and the keys' help use Bevy's embedded Fira Mono (OFL).

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

## 9. What we do not do

| Original subsystem | Decision |
|---|---|
| TinCat 2 networking, lobby, LAN announcement, `GameServer.exe` | not before version 1.0; multiplayer after it (PLAN.md ADR-09, §13) |
| Loading the original formats in the game | only through the converter |
| Compatibility with the original's saves and exported heroes | no |
| The original's Miles, amstream, IJL, libxml2, unicows, CRT and STLport | replaced by the Rust ecosystem or not needed |
| Steam and GOG Galaxy integration | not needed |
| macOS, iOS | out of scope |

## Open questions

1. Whether Android is needed: a decision after version 1.0.

Closed on 2026-10-16: the container and codecs of `sound.pak` (docs/02, §10.2), the codecs of the WMV movies (docs/02, §10.3).

## Sources

* The original's platform dependencies: docs/01, §1 and §4.5; NetScript, the string hash, AMH: docs/02, §7–9.
* Crate versions: the workspace's `Cargo.toml` (docs/04 §6.1).
