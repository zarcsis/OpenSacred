# Platforms and subsystems: sound, input, video, files, saves, scripts

## Summary

* **Platforms:** Windows 11 x64 is the main one; Linux is second, with the same functionality; Android and Chrome (WebAssembly) come after version 1.0 and by a separate decision.
  Everything builds from one codebase on Bevy and wgpu.
* **Sound:** Kira through `bevy_kira_audio` with Symphonia decoders (pure Rust, works on all four platforms).
  Sound events are taken from the original's 175 profiles, converted to RON.
* **Input:** keyboard and mouse as in the original plus a gamepad; key remapping through `leafwing-input-manager`; touch controls only for Android.
* **Video:** the 10 WMV movies are converted to AV1 + Opus; playback through rav1d, in the browser through the HTML video element; movies can be skipped.
* **Saves and settings** use our own formats on serde; the original saves are not supported.
* **Scripts:** NetScript bytecode is converted into an intermediate representation (IR); the interpreter lives in `opensacred-rules`.
* **No networking:** no server, no multiplayer, no lobby; no Steam or GOG Galaxy integration is needed.

## 1. Platforms

| Platform | Build | Specifics | When | Agent-hours (estimate) |
|---|---|---|---|---|
| Windows 11 x64 | `x86_64-pc-windows-msvc`, Build Tools 2026 | the main one; Vulkan, DX12 as fallback; the game directory with `assets/` next to the exe | M2–M6 | included in the milestones |
| Linux x86-64 | `x86_64-unknown-linux-gnu` | Vulkan; XDG directories; builds in WSL2 or on Linux | after M6 | 5–10 |
| Android | `aarch64-linux-android` through cargo-ndk and Gradle | touch controls, a smaller streaming radius, ASTC textures, assets in app storage | by decision | 25–50 |
| Chrome (WebAssembly) | `wasm32-unknown-unknown`, wasm-bindgen, trunk | WebGPU (WebGL2 as fallback); assets over HTTP from a local server; multithreading requires COOP/COEP headers | by decision | 25–50 |

The game assets must not be made publicly available, so the web version is meant for personal use: a local server serves the player's own `assets/` to a browser on the same computer or on the home network.

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
| Decoders | Symphonia 0.6 (MP3, FLAC, Vorbis, WAV), pure Rust |
| Sound effects and speech | `sound.pak`: WAV → FLAC (lossless from PCM and IMA ADPCM), MP3 entries unchanged |
| Music | MP3 unchanged or Ogg Vorbis; streaming playback |
| Events | profiles → `sound_profiles.ron`: event → sample group → random selection rules |
| Spatial sound | panning and distance attenuation from the hero; no HRTF at the start |
| Music system | region zones, switching to combat music, event stingers; smooth transitions |
| Volumes | music, effects, voices, as in the original's settings |

Estimate: 10–20 agent-hours (M2 and M5).

## 3. Input

* **Keyboard and mouse:** click to move, attack, the quick skill bar, the original's hotkeys as the default layout.
* **Gamepad** (new): stick movement, skills on buttons, a radial menu for the skill bar; through gilrs inside Bevy.
* **Remapping:** `leafwing-input-manager` 0.22; the layout is stored in the settings.
* **Touch input:** only for Android, after the platform decision.
* IME for text input (the hero's name), by means of Bevy and winit.

Estimate: 6–12 agent-hours.

## 4. Video

| Step | Decision |
|---|---|
| Conversion | FFmpeg as an external program: WMV → AV1 (video) + Opus (audio) in WebM; the original resolution and frame rate (640×480, 25 fps); the source audio is WMAv2 or WMA Pro, both decoded by FFmpeg |
| Playback | a WebM demuxer (`matroska-demuxer`) + the rav1d AV1 decoder → a Bevy texture; audio through Kira |
| Browser | the HTML video element over the canvas |
| Controls | skip with a key; a "do not show movies" setting |

Estimate: 6–12 agent-hours (M5).
Movies are not needed to complete the game, so this is a low priority.

## 5. Files and assets

* **Layout:** `assets/` holds the base assets from the converter; `assets-hd/` holds HD packs with the same relative paths.
  The game looks for a file in the HD directory first.
* **Loading:** Bevy's AssetServer, asynchronously; sectors and creatures are streamed in as the hero approaches.
* **Names:** the converter normalizes paths to lowercase and forward slashes, so file system case does not matter.
* **Installation check:** the converter cross-checks `OriginalGame/` against the SHA-256 manifest of the GOG installation (`docs/manifest-gog-2.28.txt`, 319 files, excluding the files the game writes: `Settings.cfg`, `Gameserver.cfg`, `save/`, the `DefPos.bin` caches) and refuses to work on an unknown build unless the force flag is given.
* **Empty quest files:** 29 `QuestCode.bin` and `QuestPoolCode.bin` files (9 and 20) in the GOG release are zero-sized and are not in the manifest; the converter treats an empty file and a missing file alike as "no script".
* **Packaging a build for personal use:** the exe + `assets/` (+ `assets-hd/`) in one directory; later, an optional asset archive.

## 6. Saves and settings

* **Saves:** our own format: a `serde` structure with a version number and migrations.
  RON in debug (human-readable), postcard with zstd compression in release.
  Autosave on a timer, like `AUTOSAVEDELAY` in the original.
* **Directories:** through the `directories` crate: `%APPDATA%\OpenSacred` on Windows, XDG on Linux, app storage on Android, IndexedDB in the browser.
* **Settings:** TOML: graphics (resolution, window mode, backend, zoom, UI scale, HD packs), sound (volumes), controls (layouts), gameplay.
  The original's gameplay options are carried over by meaning: `AUTOTRACKENEMY`, `SHOWPOTIONS`, `PICKUPAUTO`, `NIGHT_DARKNESS`, `UNIQUE_COLOR`, `SHOW_ENEMYINFO`; `CHAT_*` are not needed (no networking).
* **The original saves** (`.pax`) are not supported.
  The hero templates `Templates/*.ptx` are converted into starter kits for character creation.

Estimate: 8–15 agent-hours (M6).

## 7. Localization

* `global.res` (UTF-16LE, 23,123 strings) → Fluent (`.ftl`) with keys from the hash identifiers; the original names are known for 56% of the strings, and the rest get a key of the form `id-<number>`.
* English is the only language of the GOG release.
  Other languages can be added as Fluent files; third-party translations are for personal use only.
* Fonts: the original TTFs + Noto (OFL) for Cyrillic and other characters.

Estimate: 3–6 agent-hours (M1 and M5).

## 8. NetScript scripts

### 8.1 What is known

* 20 script directories: `bin/TYPE_NPC_*` (8 classes) and `bin/NetScriptCamp` are the Sacred campaign; `bin/Addon/*` is Underworld (9 identical trees); `bin/NetScript` is multiplayer (not needed).
* Records of the form "length u16, type u16, data"; the type space is 141, of which 113 occur in the release; 45 types cover 95% of the campaign records, 68 cover 99%.
* The interpreter is in the client; the compiler's keyword tables are in the `.data` section of `Sacred.exe` and give names for 132 types.
* Details: `docs/02-data-formats.md`, §7, and `docs/re/netscript.md`.

### 8.2 Implementation

1. **Converter:** a disassembler in `opensacred-formats` → IR (RON): quests, states, conditions, actions, references to strings and objects; plus a readable listing for review.
2. **Record type semantics** are recovered by analyzing the interpreter in Ghidra and described in `docs/re/netscript.md`, starting with the most frequent ones; each type gets a test on real records.
3. **The interpreter** in `opensacred-rules`: `match` on the record type over the IR; quest state is part of the save.
4. **Modding** (optional, after 1.0): the ability to write new quests in Rhai on top of the same API.

Estimate: 40–80 agent-hours (M1 and M4); the main source of content uncertainty.

## 9. What we do not do

| Original subsystem | Decision |
|---|---|
| TinCat 2 networking, lobby, LAN announcement, `GameServer.exe` | out of scope |
| Loading the original formats in the game | only through the converter |
| Compatibility with `.pax` saves | no |
| The original's Miles, amstream, IJL, libxml2, unicows, CRT and STLport | replaced by the Rust ecosystem or not needed |
| Steam and GOG Galaxy integration | not needed |
| macOS, iOS | out of scope |

## Open questions

1. Whether Android and the browser are needed: a decision after version 1.0.

Closed on 2026-10-16: the container and codecs of `sound.pak` (docs/02, §10.2), the codecs of the WMV movies (docs/02, §10.3).

## Sources

* The original's platform dependencies: docs/01, §1 and §4.5; NetScript, the string hash, AMH: docs/02, §7–9.
* Crate versions: crates.io as of 2026-10-16.
