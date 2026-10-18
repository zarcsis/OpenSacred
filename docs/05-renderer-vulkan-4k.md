# Rendering: Bevy and wgpu, Vulkan, 4K and HD assets

## Summary

* The renderer is written from scratch on **Bevy 0.20 / wgpu 30**: Vulkan by default on Windows and Linux, DX12 as the fallback on Windows, WebGPU in Chrome, Vulkan or GLES on Android. The code of the original renderer (D3D7) is not ported; only facts are taken from it: the projection, the diamond geometry, the order of passes.
* The scene stays isometric under an orthographic camera with the original's angle, but becomes a real 3D scene: **the ground is a mesh from the heightmap**, objects are sprites in world space with depth, characters are skeletal glTF models with lighting and shadows.
* **4K:** rendering at native resolution; by default the same area of the world is visible as in the original at zoom 1.0; an extended view is an option. The UI and text are scaled separately and rasterized for the actual DPI.
* The main limit of the image is the 256² ARGB4444 textures without mipmaps. **HD packs** remove it (milestone M7): offline upscaling with free models, packing into KTX2 with mipmaps, path-based substitution on top of the base assets.
* The original's bottleneck is about 9,600 draw calls per frame on the CPU (community-reported). Bevy's batching, instancing and GPU skinning remove it; the budget is 60 fps at 3840×2160 on a mid-range graphics card.

## 1. How the original renders (facts to reproduce)

| Aspect | Fact | Status |
|---|---|---|
| API | Direct3D 7 TnL HAL through the `dxDriver7` wrapper; about 17 render states and 2 texture stages | confirmed |
| Resolution | hardcoded 1024×768; the screen center 512/384 is hardcoded in 17 places in the code | confirmed |
| Projection | `ortho(±267·zoom, ±200·zoom, −600, 2500)`: 534×400 world units on 1024×768 at zoom 1; zoom 0.5–2.0 | confirmed |
| Ground | diamonds of ≈96×48 pixels at zoom 1; 18 diamonds per 256×256 texture sheet; corner heights are int8 in steps of 5 (78.4% of cells are flat) and shift the screen Y by h/zoom; corner lighting is gray values | confirmed |
| Depth | sprites get their depth from a formula of screen Y, height and the `atan 0.5` angle (2:1 dimetric) | confirmed |
| Objects | static objects and sprites are alpha-tested quads (`ALPHAREF 0x40`) | confirmed |
| Characters | Granny meshes; CPU skinning twice per frame (the model and the flat shadow) | confirmed |
| Shadows | a mesh flattened onto the ground, through the stencil; with 16-bit color there is no stencil | confirmed |
| Fog | D3D fog is not used; "fog" is a weather sprite effect | confirmed |
| Textures | ARGB4444 and A8R8G8B8 (bytes B, G, R, A), no mipmaps, no DXT | confirmed |
| Fonts | rasterized through GDI into 256² pages | confirmed |
| Frame order | tile rows top to bottom → blend layers → water and lava → static sprites → models → effects → UI | community-reported |
| Load | ≈9,600 draw calls and 8,400 texture switches per frame at zoom 2 | community-reported |

Details: the floor cell, the heights and the UV table in docs/02, §5.2–5.4; the render states from `tools/analysis/render/d3d7scan.py`.

## 2. What we change

| Original | New renderer | Why |
|---|---|---|
| Ground quads offset along screen Y | a terrain mesh from the same heights in world coordinates | correct slope lighting, shadows, no tile "jitter" |
| 1024×768 and a fixed far plane | any resolution; depth is computed from the visible area | 4K and wide screens without clipping |
| Per-vertex lighting only | Bevy lighting: directional light by time of day, point lights for spells and torches | a modern image; smoother day and night |
| Flat stencil shadows | shadow maps for 3D characters; projected quad shadows for sprites | shadows on terrain and at any color depth |
| CPU skinning | Bevy GPU skinning | takes the load off the CPU |
| Thousands of small draw calls | per-sector tile batching, sprite instancing, texture arrays | 60 fps in 4K |
| 256² GDI fonts | cosmic-text at native resolution; the original TTFs + a free Unicode font | crisp text at any scale |
| No mipmaps | mipmaps in KTX2 | no shimmering when zoomed out |

Fidelity of the image is not a goal: differences are acceptable if the game looks better and reads the same.

## 3. How the Bevy renderer is organized

### 3.1 World

* **Sectors** are streamed in around the hero (the radius is configurable). For each sector, a terrain mesh is built at load time: vertices at the nodes of the cell grid, heights from the converted data, UVs into the tile atlas.
* **The tile atlas** is a texture array: each 256² sheet is a layer; the UVs of the 18 diamonds come from a table recovered from the code (`tools/analysis/world/uv_table.py`). Tile blend layers are a second UV set and a mask.
* **Water and lava** are a separate material with UV animation and, optionally, sky reflection.
* **Static objects** (trees, houses, decorations) are sprite quads in world space. The camera looks at a constant angle, so the quads are oriented once at load time. Depth comes from the world position, sorting goes through the depth buffer with alpha testing; translucent parts go in a separate sorted pass.

### 3.2 Characters and items

* glTF models from the converter with the Bip01 skeleton; materials are PBR with standard parameters, with no claim to accuracy.
* Animations are glTF clips; Bevy's animation graph blends walking, running, attacks and skills. Blend parameters from Granny (ADSR, durations) are carried over approximately.
* Weapons and armor are attached to bones by name; changing equipment swaps meshes without rebuilding the skeleton.

### 3.3 Effects

* The original's 178 effects are described in data (RON): emitter, textures, color and size curves, lifetime.
* The executor is GPU particles (`bevy_hanabi`) on Vulkan and WebGPU; CPU particles on WebGL2. If `bevy_hanabi` lags behind the Bevy version, we start with the CPU executor.

### 3.4 Camera and visible area

* An orthographic camera with the original's dimetric angle (`atan 0.5`).
* The base visible area is 534×400 world units at zoom 1, as in the original; at 3840×2160 it is stretched across the whole screen, keeping the proportions by height.
* Zoom is smooth, within the original's 0.5–2.0 range; the "extended view" mode shows more of the world and streams in more sectors.
* Sprite sampling modes: "smooth" (linear filtering with mipmaps) and "sharp" (an xBR- or FSR 1-style upscaling shader), while there are no HD packs yet.

### 3.5 UI and text

* A new interface in Bevy UI with a scale factor from the monitor's DPI and a user setting.
* Text: cosmic-text through Bevy; the original fonts (`ANTQS__.TTF`, `CAROLING.TTF`) + Noto (OFL) for missing characters.
* The original UI textures are used as skins with nine-slice stretching (9-slice) so that the frames do not blur.

### 3.6 What `opensacred-game` does so far (2026-10-17)

The vertical slice (`crates/opensacred-game`, Bevy 0.20) implements R1 and most of R2; where it departs from §3.1–3.4, the departure is interim:

* **Camera:** orthographic, yaw 45°, pitch 30° (sin 30° = 1/2 gives the 2:1 diamond), in the original's world units (a cell is 24·√5 = 53.67 units, the edge of its diamond in pixels; 1.26 pixels a unit), scaled so that a cell's diamond is 96 × 48 pixels at zoom 1 and the view is as tall as the original's 768-pixel screen at any resolution (607 world units). The "534 × 400" of §1 and §3.4 is in the units of the models, which the original projects at 1.92 pixels a unit while its ground is drawn in 2D: so a model unit is 1.52 world units (`iso::MODEL_SCALE`), and the hero stands about 120 pixels tall at zoom 1, matching the well and the bench beside the village's citizens; the creatures' speeds take the same factor. The original's depth formula names the angle `atan 0.5` (26.6°); the 30° pitch keeps the 3D ground on the 2D diamonds exactly.
* **Ground:** per sector, every cell's tile and floor overlays are composed on the CPU into one slot of an atlas of the distinct stacks of layers (748 slots for the median sector, 13 MB) and draped over a mesh of the cells with their corner heights and light as vertex colour, unlit. A texture array of the sheets with the layers composed in a shader (§3.1) would use less memory; the CPU atlas keeps the first version simple.
* **Objects:** each sprite and miniature is a quad in the plane of the screen at the depth of its cell's centre, alpha to coverage. A sector's sprites are assembled from their pieces, one to one, when the sector is built, and packed into pages of 2,048 × 2,048 (the start's 3 × 3 sectors use 489 sprites, 71 MB), one mesh per page: filtering each piece in its own atlas left a step at every piece's edge, a line on roofs and walls once magnified, and the sample stays half a texel inside the sprite's image (`sprites.wesl`). Miniatures take their square of their type's atlas, one mesh per atlas. Sprites at one depth (one cell's, or those of the cells of one diagonal at one height) come forward a little in the original's order, by their cell's place along the screen's right and their place in the cell's chain (`sprites.wesl`, docs/re/drawing.md), the depth taken from the cell's centre relative to the point the camera looks at; the camera's depth range is 10,000–30,000 units, a thousandth of a unit a step of the depth buffer. The ground lies 5,000 units behind along the line of sight, so it never hides an object (the original draws it first); the underlay types go behind the ordinary sprites (draw lists 0 and 2, by the object's mask) and the types of list 4 in front of them (docs/02 §6.1).
* **Water and lava:** each liquid cell shows the frames of its sector's liquid material (`B_WATER`, `C_LAVA`, …), a frame over 4 × 4 cells, cycling in 2,048 ms as the original does (docs/re/liquids.md); the clear waters translucent over the bed and every surface fading at the shore until the original's blending is recovered (docs/decisions.md).
* **Streaming:** the 3 × 3 sectors around the hero's are built off the main thread, two at a time, and spawned one a frame; farther than two sectors they go.
* **Characters:** the hero (`--class`: any of the seven heroes whose models carry their own clips, the Seraphim, the Gladiator, the Battle Mage, the Wood Elf, the Dark Elf, the Daemon and the Dwarf; no base model of the Vampiress was found) as its glTF with idle, walk and run clips, starting at its start script's `StartPosition` blended by `AnimationTransitions`, at the creature's walk and run speeds (the `R` key toggles walking); the start script's NPCs with their idle clips, holding their weapons and shields as the client equips them (docs/re/equipment.md), and its objects (1,990 in the Seraphim's: barrels, chests, book stands, coffins, signposts). Each character has a soft dark ellipse for a shadow (docs/decisions.md).
* **HUD:** the hero's portrait (`gui_qbp_sera`), a health bar and whether the hero walks or runs, in the original's font, laid out on a 768-pixel-high screen scaled to the window (docs/decisions.md); the original's cursor (`MOUSE_MAIN`, the arrow in the top left 32 × 32 of its image), scaled in whole steps the same way.
* **Sound:** the hero's footsteps at the moments of the walk and run clips (docs/02 §4.1), the group of the footstep profile by the nature of the cell underfoot, by the client's own table (docs/re/sound.md); the wood tracks of the `music` profile one after another (docs/02 §10.3), as the regions' music is not recovered yet; beside it, the ambient loop that the nature underfoot calls for, by the client's own switch (docs/re/sound.md), the day's. `bevy_kira_audio` decodes a whole track into memory, so only the one playing is loaded.
* **Measured:** about 690 frames a second at 3840 × 2160 while the hero runs across sectors, 750 at 1920 × 1080 with the hero standing (`--benchmark`, music on, on a development machine with 32 logical cores, 2026-10-18); the slowest frames, 30 ms, come when sectors spawn. Until then a few frames a run took 30–150 ms, with vsync too and with the hero standing: Bevy's compute pool had 24 threads, and with the main and render threads, the audio's and the rest of the system's there were more threads ready to run than cores, so a thread that lost its core waited for the scheduler's next tick. The stalls came in multiples of 15.6 ms, Windows' tick; ordinary systems (`propagate_parent_transforms`, `prepare_lights`) took 20–30 ms in them, a thread of the process that only slept a millisecond at a time woke up to 78 ms late, and another process's such thread up to 31 ms late; the sound crackled with them. The game gives the compute pool four threads (`COMPUTE_THREADS`, #2): no stall in the benchmark's runs since, and more frames. `--benchmark` counts the process's stalls (a thread of its own woken more than 20 ms late).
* **Looking at frames:** `--screenshot FILE` saves one frame once the scene is ready, `--record DIR` the next `--record-frames` (90) as `DIR/frame-NNNN.png`, with `--target` sending the hero off meanwhile: flicker and stutter show frame by frame. A capture of the desktop does not see the window (FFmpeg's `gdigrab` gets the wallpaper behind it).

## 4. Performance budget

| Metric | Target |
|---|---|
| Frames per second at 3840×2160, zoom 2 | ≥ 60 on a mid-range graphics card |
| CPU time to prepare a frame | ≤ 4 ms |
| Draw calls per frame | hundreds, not thousands (batching) |
| VRAM for the base assets | ≤ 2 GB (only the hero's surroundings are loaded) |

Base textures: 25,535 of them at 256² in BC7 take ≈1.6 GB in total (estimate), but only the textures of visible sectors and active creatures are in memory at any one time.

## 5. HD assets (milestone M7)

1. **Selection:** ground and tiles, characters and weapons, UI, effects, in order of visibility.
2. **Upscaling:** free models: Real-ESRGAN through ncnn-vulkan (BSD-3) or waifu2x-ncnn-vulkan (MIT); the Rust option is ONNX models through the `ort` crate.
3. **Alpha channel:** ARGB4444 gives 16 transparency levels; the alpha is upscaled separately and smoothed, otherwise the edges become jagged.
4. **Ground tiles:** upscaling the whole 256² sheet, followed by a check of the diamond seams; if needed, upscaling with overlap.
5. **Packing:** KTX2, BC7 for PC, ASTC or UASTC for Android and the web, a full set of mipmaps.
6. **Hooking up:** the `assets-hd/` directory with the same relative paths; the game takes the HD version if it exists.
7. **Size:** upscaling all textures ×4 gives ≈25 GB in BC7 (estimate), so HD packs are selective and separate.

HD packs are derived from the game assets and, like the assets themselves, are not published.

## 6. Platforms

| Platform | wgpu backend | Specifics |
|---|---|---|
| Windows 11 x64 | Vulkan; DX12 as fallback | the main target; backend choice in the settings |
| Linux | Vulkan | the same functionality |
| Android | Vulkan; GLES as fallback | a smaller streaming radius; ASTC; touch controls |
| Chrome (WebAssembly) | WebGPU; WebGL2 as fallback | no GPU particles on WebGL2; sectors streamed over HTTP from a local server |

## 7. Testing

* **Screenshot tests:** a headless render of test scenes (a sector, a character, an effect) to PNG and comparison with a reference within a tolerance. **Implemented 2026-10-17** for a sector: `opensacred-game`'s `scene_tests` writes a synthetic world in the converter's formats (a patterned tile sheet, a sector with a hill, a pond and light, trees and an underlay slab), renders it through the game's terrain, sprite and camera code into an image with no window, waits until every texture is loaded and no pipeline compiles, and compares it with `crates/opensacred-game/tests/screenshots/scene.png` in blocks of 4 × 4 pixels (at most 1 % of the blocks may differ by more than 12 levels; a 2 % zoom fails). It needs no game files, so CI runs it, on Mesa's software Vulkan; the reference was rendered on a development machine's GPU, and Windows' software adapter (WARP, `WGPU_BACKEND=dx12 WGPU_FORCE_FALLBACK_ADAPTER=1`) matches it too, in 70 s instead of 1: neither differs in any block, while a 2 % zoom differs in 6.5 % of them.
* **Cross-check with the original:** reference frames of the original through dgVoodoo2 on Windows 11 in 32-bit color mode (`GFX32 : 1`, the GOG default), for visual cross-checking, with no requirement for pixel matching.
* **Debugging:** RenderDoc, the Vulkan SDK validation layers, Bevy's built-in diagnostics (frame time, number of draw calls).

## 8. Rendering milestones

| Step | Result | Agent-hours (estimate) | Plan milestone |
|---|---|---|---|
| R1 First sector | terrain mesh, tile atlas, orthographic camera, 4K | 8–15 | M2 |
| R2 Slice | sector streaming, sprite objects, hero with animations, water | 15–30 | M2 |
| R3 Full world | all object types, shadows, day and night lighting, weather, effects | 25–50 | M5 |
| R4 4K polish | UI scale, sampling modes, graphics settings | 8–15 | M6 |
| R5 HD pipeline | upscaling, KTX2, path-based substitution | 15–30 | M7 |

## Open questions

1. How 3D models get their base height on slopes, in the data or in the code (this affects placing characters on the terrain).
2. Where the image or geometry of the type 9 and 25 static templates in `Items.pak` is stored: sprites or models. Narrowed down on 2026-10-16: the names are editor labels, and there are no references to models (docs/02, §6.1).
3. Readiness of `bevy_hanabi` for Bevy 0.20.
4. How well upscaling copes with 4-bit alpha: check on a sample in M7.

## Sources

* Heights, the floor cell and the UV table: docs/02, §5.2–5.4; `tools/analysis/world/uv_table.py`. Render states: `tools/analysis/render/d3d7scan.py`.
* Community measurements and workarounds (sacred-bild), behavior at high resolutions: docs/09, §2.
* Bevy 0.20: bevy.org/news/bevy-0-20; wgpu 30.0.1: crates.io.
