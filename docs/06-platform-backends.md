# Platforms and subsystems: sound, input, video, files, saves, scripts

## Summary

* **Platforms:** Windows 11 x64 is the main one; Linux is second, with the same functionality; Android and Chrome (WebAssembly) come after version 1.0 and by a separate decision.
  Everything builds from one codebase on Bevy and wgpu.
* **Sound:** Kira through `bevy_kira_audio` with Symphonia decoders (pure Rust, works on all four platforms).
  Sound events are taken from the original's 175 profiles, converted to RON.
* **Input:** keyboard and mouse as in the original plus a gamepad; key remapping through `leafwing-input-manager`; touch controls only for Android.
* **Video:** the 10 WMV movies are converted to AV1 + Opus; playback through rav1d, in the browser through the HTML video element; movies can be skipped.
  The conversion is done (`opensacred-convert video`); playback comes in M5.
* **Saves and settings:** settings are TOML through serde and `toml_edit` (done); saves will be our own format on serde (M6); the original saves are not supported.
* **Scripts:** NetScript bytecode is converted into an intermediate representation (IR); the interpreter will live in `opensacred-rules`.
  Until then `opensacred-game` places the NPCs and objects of the start script and of the sectors' `Init`, `Enter` and `FillSector` records.
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
| Decoders | Symphonia 0.6, FLAC and MP3 (the converter's outputs), pure Rust |
| Sound effects and speech | `sound.pak`: WAV → FLAC (lossless from PCM and IMA ADPCM), MP3 entries unchanged |
| Music | MP3 unchanged; `bevy_kira_audio` does not stream, so only the track playing is loaded (≈85 MB decoded for four minutes) |
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
| Conversion | FFmpeg as an external program: WMV → AV1 (video) + Opus (audio) in WebM; the original resolution and frame rate (640×480, 25 fps); the source audio is WMAv2 or WMA Pro, both decoded by FFmpeg |
| Playback | a WebM demuxer (`matroska-demuxer`) + the rav1d AV1 decoder → a Bevy texture; audio through Kira |
| Browser | the HTML video element over the canvas |
| Controls | skip with a key; a "do not show movies" setting |

Estimate: 6–12 agent-hours (M5).
Movies are not needed to complete the game, so this is a low priority.

## 5. Files and assets

* **Layout:** `assets/` holds the base assets from the converter; `assets-hd/` holds HD packs with the same relative paths.
  The game will look for a file in the HD directory first (M7); nothing reads `assets-hd/` yet.
* **Loading:** the sectors are read and built on Bevy's async compute pool, the models and images through its AssetServer; the sectors and their creatures come in as the hero approaches.
* **Names:** the converter normalizes paths to lowercase and forward slashes, so file system case does not matter.
* **Installation check:** the command `opensacred-convert verify` cross-checks `OriginalGame/` against the SHA-256 manifest of the GOG installation (`docs/manifest-gog-2.28.txt`, 319 files, excluding the files the game writes: `Settings.cfg`, `Gameserver.cfg`, `save/`, the `DefPos.bin` caches); the conversions do not check it themselves.
* **Empty quest files:** 29 `QuestCode.bin` and `QuestPoolCode.bin` files (9 and 20) in the GOG release are zero-sized and are not in the manifest; the converter treats an empty file and a missing file alike as "no script".
* **Packaging a build for personal use:** the exe + `assets/` (+ `assets-hd/`) in one directory; later, an optional asset archive.

## 6. Saves and settings

* **Saves:** our own format: a `serde` structure with a version number and migrations.
  RON in debug (human-readable), postcard with zstd compression in release.
  Autosave on a timer, like `AUTOSAVEDELAY` in the original.
* **Directories:** `%APPDATA%\OpenSacred` on Windows, `$XDG_CONFIG_HOME/OpenSacred` (`~/.config/OpenSacred`) on Linux, app storage on Android, IndexedDB in the browser.
  Since 2026-10-19 the game takes them from the environment (`APPDATA`, `XDG_CONFIG_HOME`, `HOME`), which needs no crate; `directories` stays the option when Android needs its app storage.
  The log goes to the state directory: `%LOCALAPPDATA%\OpenSacred\logs\` on Windows, `$XDG_STATE_HOME/OpenSacred` (`~/.local/state/OpenSacred`) on Linux (docs/10 §5).
* **Settings:** TOML: graphics (resolution, window mode, backend, zoom, UI scale, HD packs), sound (volumes), controls (layouts), gameplay.
  Done ahead of M6 on 2026-10-19 (#167): `settings.toml` with its display section, `mode` (`borderless`, the default: borderless full screen at the monitor's resolution; or `windowed`) and the window's `width` and `height` in physical pixels; F11 switches the mode and writes it at once (docs/10 §3).
  Since #169 and #170 a `[debug]` section keeps whether the overlay (F3) and the log (F4) show.
  The game writes only the key it changes, through `toml_edit`, so the user's comments, order and unknown keys stay; a file that does not read is reported, the defaults are taken, and it is never written; an unknown key is reported.
  `--settings FILE` reads and writes another file, `--windowed`, `--borderless`, `--width` and `--height` override it for one run, and `--screenshot`, `--record` and `--benchmark` run in a window of the given size.
  The original's gameplay options are carried over by meaning: `AUTOTRACKENEMY`, `SHOWPOTIONS`, `PICKUPAUTO`, `NIGHT_DARKNESS`, `UNIQUE_COLOR`, `SHOW_ENEMYINFO`; `CHAT_*` are not needed (no networking).
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

* 20 script directories: `bin/TYPE_NPC_*` (8 classes) and `bin/NetScriptCamp` are the Sacred campaign; `bin/Addon/*` is Underworld (9 identical trees); `bin/NetScript` is multiplayer (not needed).
* Records of the form "length u16, type u16, data"; the type space is 141, of which 113 occur in the release; 45 types cover 95% of the campaign records, 68 cover 99%.
* The interpreter is in the client; the compiler's keyword resolvers in `.text`, with their strings in `.data`, name 135 of the 141 types.
* Details: `docs/02-data-formats.md`, §7, and `docs/re/netscript.md`.

### 8.2 Implementation

1. **Converter:** done 2026-10-17: the disassembler in `opensacred-formats` → a faithful IR, one postcard file per tree (`netscript/<tree>.nsc`), with a readable listing on request (`--listing`).
   Grouping into quests, states, conditions and actions waits for the record semantics (docs/02 §7.6).
2. **Record type semantics** are recovered by analyzing the interpreter in Ghidra and described in `docs/re/netscript.md`, starting with the most frequent ones; each type gets a test on real records.
3. **The interpreter** in `opensacred-rules`: `match` on the record type over the IR; quest state is part of the save.
   Until then `opensacred-game` reads the start script's and the sectors' `CreateNPC` and `CreateObj` records itself.
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
