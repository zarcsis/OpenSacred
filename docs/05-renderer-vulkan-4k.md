# Rendering: Bevy and wgpu, Vulkan, 4K and HD assets

## Summary

* The renderer is written from scratch on **Bevy 0.20 / wgpu 30**: Vulkan by default on Windows and Linux, DX12 as the fallback on Windows, WebGPU in Chrome, Vulkan or GLES on Android.
  The code of the original renderer (D3D7) is not ported; only facts are taken from it: the projection, the diamond geometry, the order of passes.
* The scene stays isometric under an orthographic camera with the original's angle, but becomes a real 3D scene: **the ground is a mesh from the heightmap**, objects are sprites in world space with depth, characters are skeletal glTF models with lighting and shadows.
* **4K:** rendering at native resolution; by default the same area of the world is visible as in the original at zoom 1.0; an extended view is an option.
  The UI and text are scaled separately from the scene, with the window's height, and rasterized at the window's resolution (§3.5).
* The main limit of the image is the 256² ARGB4444 textures without mipmaps.
  **HD packs** remove it (milestone M7): offline upscaling with free models, packing into KTX2 with mipmaps, path-based substitution on top of the base assets.
* The original's bottleneck is about 9,600 draw calls per frame on the CPU (community-reported).
  A few large meshes (a sector's ground, a page of sprites) and Bevy's GPU skinning remove it; the budget is 60 fps at 3840×2160 on a mid-range graphics card.

## 1. How the original renders (facts to reproduce)

| Aspect | Fact | Status |
|---|---|---|
| API | Direct3D 7 TnL HAL through the `dxDriver7` wrapper; about 17 render states and 2 texture stages | confirmed |
| Resolution | hardcoded 1024×768; the screen center 512/384 is hardcoded in 17 places in the code | confirmed |
| Projection | `ortho(±267·zoom, ±200·zoom, −600, 2500)`: 534×400 world units on 1024×768 at zoom 1; zoom 0.5–2.0 | confirmed |
| Ground | diamonds of ≈96×48 pixels at zoom 1; 18 diamonds per 256×256 texture sheet; corner heights are int8 in steps of 5 (78.4% of cells are flat) and shift the screen Y by h/zoom; corner lighting is gray values | confirmed |
| Depth | sprites get their depth from a formula of screen Y, height and the `atan 0.5` angle (2:1 dimetric) | confirmed |
| Objects | static objects and sprites are quads, point-filtered, blended by their alpha (`SRCALPHA`, `INVSRCALPHA`) after an alpha test (`ALPHAREF 0x40`, `GREATER`); the miniatures without the test (docs/re/drawing.md) | confirmed |
| Characters | Granny meshes; CPU skinning twice per frame (the model and the flat shadow) | confirmed |
| Shadows | a mesh flattened onto the ground, through the stencil; with 16-bit color there is no stencil | confirmed |
| Fog | D3D fog is not used; "fog" is a weather sprite effect | confirmed |
| Textures | ARGB4444 and A8R8G8B8 (bytes B, G, R, A), no mipmaps, no DXT | confirmed |
| Fonts | rasterized through GDI into 256² pages | confirmed |
| Frame order | tile rows top to bottom → blend layers → water and lava → static sprites → models → effects → UI | community-reported |
| Load | ≈9,600 draw calls and 8,400 texture switches per frame at zoom 2, the widest view | community-reported |

Details: the floor cell, the heights and the UV table in docs/02, §5.2–5.4; the render states from `tools/analysis/render/d3d7scan.py`.

## 2. What we change

| Original | New renderer | Why |
|---|---|---|
| Ground quads offset along screen Y | a terrain mesh from the same heights in world coordinates, unlit, with the original's corner light as its vertex colour | no tile "jitter"; one depth for the ground, the sprites and the models |
| 1024×768 and a fixed far plane | any resolution; depth is computed from the visible area | 4K and wide screens without clipping |
| Per-vertex lighting only | the original's light and night colours, multiplied over the drawn frame in a fullscreen pass (docs/decisions/2026-10-18-night.md); a fixed directional light and an ambient light for the models | the original's look by day and by night |
| Flat stencil shadows | the same flattened meshes for the models, the depth test in place of the stencil (docs/decisions/2026-10-19-model-shadows.md); the statics' shadows from the original's shadow atlas | Bevy's main pass has no stencil |
| CPU skinning | Bevy GPU skinning | takes the load off the CPU |
| Thousands of small draw calls | one ground mesh and atlas per sector, one sprite mesh per page of 2,048 × 2,048 | 60 fps in 4K |
| 256² GDI fonts | Bevy's text (parley) at native resolution; the original TTFs; a free Unicode font is planned | crisp text at any scale |
| No mipmaps | mipmaps in KTX2 with the HD packs (M7); none yet: the art is filtered linearly without them (docs/decisions/2026-10-19-linear-filter.md) | no shimmering when zoomed out |

Fidelity of the image is not a goal: differences are acceptable if the game looks better and reads the same.
Where the game draws as the original does instead, a file of docs/decisions/ says why (§3.6).

## 3. How the Bevy renderer is organized

### 3.1 World

* **Sectors** are streamed in around the hero (the radius is configurable).
  For each sector, a terrain mesh is built at load time: vertices at the nodes of the cell grid, heights from the converted data, UVs into the tile atlas.
* **The tile atlas** is a texture array: each 256² sheet is a layer; the UVs of the 18 diamonds come from a table recovered from the code (`tools/analysis/world/uv_table.py`).
  Tile blend layers are a second UV set and a mask.
* **Water and lava** are a separate material with UV animation and, optionally, sky reflection.
* **Static objects** (trees, houses, decorations) are sprite quads in world space.
  The camera looks at a constant angle, so the quads are oriented once at load time.
  Depth comes from the world position, sorting goes through the depth buffer with alpha testing; translucent parts go in a separate sorted pass.

### 3.2 Characters and items

* glTF models from the converter with the Bip01 skeleton; materials are PBR with standard parameters, with no claim to accuracy.
* Animations are glTF clips; Bevy's animation graph blends walking, running, attacks and skills.
  Blend parameters from Granny (ADSR, durations) are carried over approximately.
* Weapons and armor are attached to bones by name; changing equipment swaps meshes without rebuilding the skeleton.

### 3.3 Effects

* The original's 178 effects are described in data (RON): emitter, textures, color and size curves, lifetime.
* The executor is GPU particles (`bevy_hanabi`) on Vulkan and WebGPU; CPU particles on WebGL2.
  If `bevy_hanabi` lags behind the Bevy version, we start with the CPU executor.

### 3.4 Camera and visible area

* An orthographic camera with the original's dimetric angle (`atan 0.5`).
* The base visible area is the original's 768-pixel-high screen at zoom 1, 607 world units tall (534×400 in the units of the models, §3.6); at 3840×2160 it fills the whole screen, keeping the proportions by height.
* Zoom spans the original's range of views (0.5–2.0, §1); the "extended view" mode shows more of the world and streams in more sectors.
* The ground and the sprites are filtered linearly at any zoom, each texel weighted by its alpha (docs/decisions/2026-10-19-linear-filter.md); a point filter and a sharp bilinear one were tried and dropped (#124, #136).

### 3.5 UI and text

* A new interface in Bevy UI, laid out on the original's 768-pixel-high screen and scaled with the window's height (`hud.rs`), whatever the desktop's scaling: the window's scale factor is fixed at 1 (`main.rs`).
  A user setting for the scale is planned.
* Text: Bevy's text (parley) at native resolution.
  The original fonts (`ANTQS__.TTF`, `CAROLING.TTF`) are converted by `opensacred-convert ui`; the game sets its words over the world in `ANTQS__.TTF` (`game::FONT`), and its debug panels and console in Bevy's default font.
  A free Unicode font (Noto, OFL) for missing characters is planned.
* The original UI textures are used as skins with nine-slice stretching (9-slice) so that the frames do not blur.

### 3.6 What `opensacred-game` does so far (2026-10-20)

The vertical slice (`crates/opensacred-game`, Bevy 0.20) implements R1 and R2, and of R3 the shadows and day and night.
Where it departs from §2 and §3.1–3.4, it mostly draws as the original does, on purpose, and a file of docs/decisions/ says why (the sprites' depth and alpha, the liquids, the night, the models' shadows, the linear filter); a departure that is interim says so below.

* **Camera:** orthographic, yaw 45°, pitch 30° (sin 30° = 1/2 gives the 2:1 diamond), in the original's world units (a cell is 24·√5 = 53.67 units, the edge of its diamond in pixels; 1.26 pixels a unit), scaled so that a cell's diamond is 96 × 48 pixels at zoom 1 and the view is as tall as the original's 768-pixel screen at any resolution (607 world units).
  The "534 × 400" of §1 and §3.4 is in the units of the models, which the original projects at 1.92 pixels a unit while its ground is drawn in 2D: so a model unit is 1.52 world units (`iso::MODEL_SCALE`), and the hero stands about 120 pixels tall at zoom 1, matching the well and the bench beside the village's citizens; the creatures' speeds take the same factor.
  The original's depth formula names the angle `atan 0.5` (26.6°); the 30° pitch keeps the 3D ground on the 2D diamonds exactly.
  The zoom is the original's number, the camera's scale (`iso::ZOOM`), as the original's zoom scales its projection `ortho(±267·zoom, ±200·zoom, …)` (§1, docs/02 §5.3): zoom 2 shows twice the world, the widest view, and 0.5 half of it (#330: the game's number was the inverse).
  The wheel steps the zoom by a tenth a notch, turned forward bringing the world closer.
* **Colour:** no display transform: the camera has `Tonemapping::None` and no debanding, so the art's texels reach the screen as they are, as in the original (Bevy's default TonyMcMapface turned 250 into 193 and 224 into 181, and took 6 % of the chroma; #28).
  The lit models looked the same either way, so the lights stay.
  The frame holds sRGB values, in a target of `Rgba8Unorm`, and the blending takes them as they are, as the original's Direct3D 7 device blends its frame's 8-bit values (`srgb`, #109): Bevy's own sRGB compositing, which Bevy grants to 2D cameras alone, granted to the 3D camera, since every material of the game encodes its colour last (`post.wesl`, `standard.wesl`) and the HUD gives Bevy's interface its colours and images in those values; Bevy's last pass decodes the frame onto the window.
  The sRGB target before blended in linear light, so a translucent dark texel darkened less than in the original: black at half its alpha over a grey of 128 left 92 rather than 64.
* **Ground:** per sector, every cell's tile and floor overlays are composed on the CPU into one slot of an atlas of the distinct stacks of layers (748 slots for the median sector, 13 MB) and draped over a mesh of the cells with their corner heights and light as vertex colour, unlit.
  A material of its own (`terrain::GroundMaterial`, `ground.wesl`) filters the ground as the one image the original's frame makes of the diamonds: the texture coordinate is the flat screen position in the sector, and each of the four texels around a fragment comes from the slot of the cell it lies in (a 64 × 64 table of the cells' slots), so that the cells' edges blend as their insides do; filtering each cell's slot alone left a step at every edge, a grid of diamonds once magnified (across the edges the steps were 4 times those inside at 2.8 pixels a texel, against 1.3 now).
  It samples the atlas as stored and multiplies the texel by the light in sRGB values, then decodes the product, as the original multiplies the 8-bit values (docs/02 §5.3); a `StandardMaterial` multiplies its vertex colour in linear light, which left shaded ground too light (a light of 128 kept 73 % of the brightness instead of 50 %).
  A texture array of the sheets with the layers composed in a shader (§3.1) would use less memory; the CPU atlas keeps the first version simple.
* **Objects:** each sprite and miniature is a quad in the plane of the screen at the depth of its cell's centre at height 0, whatever the cell's height, as the original orders its lists by the cells alone (#112), drawn in two passes (docs/decisions/2026-10-18-sprite-alpha.md): its opaque texels through the depth buffer, then the others blended by their alpha, after the original's alpha test for the sprites, the blended things in layers in the original's order (`blended`, docs/re/drawing.md).
  The shader filters the four texels around a sample linearly, each colour weighted by its alpha, so that the transparent texels, black in the atlases, do not darken the edges.
  A sector's sprites are assembled from their pieces, one to one, when the sector is built, and packed into pages of 2,048 × 2,048 (the start's 3 × 3 sectors use 489 sprites), one mesh per page: filtering each piece in its own atlas left a step at every piece's edge, a line on roofs and walls once magnified, and the samples keep to the sprite's own texels (`sprites.wesl`).
  A sprite's image is its width and height from its pieces' smallest corner, past which its pieces are transparent (docs/02 §3.3): where the art is cut at its edge to go on in another sprite, the edge's texels reach the edge, rather than fading into the transparent texels past it, which let what lay behind the two sprites show as a line between them (#35).
  The shader places a sprite's corners on screen from the original's pixels, whole numbers that an `f32` holds exactly across the world, rather than from their world positions, which each sprite works out from its own cell's anchor and which round apart by a sixteenth of a unit at the world's coordinates: where two floors met at one pixel of the original, a column of pixels at zoom 0.5, the closest view, fell in neither and showed what lay under them (#113).
  Miniatures take their square of their type's atlas, one mesh per atlas.
  The trees, the walls of type 25 and the floors of type 29 drawn after the hero fade about it as the original's do, from 50 out of 255 at the middle of its figure to whole 190 pixels off (docs/re/drawing.md, "The trees about the heroes").
  The statics' shadows are quads of the cells of the original's shadow atlas (`SHADOW_TREE00`) at each static's foot, a rectangle or leaning right, drawn by the sprites' shader in its blended pass, list 1 between lists 0 and 2 and the furniture's with their sprites, fading out by night (docs/re/drawing.md, "The statics' shadows").
  Sprites at one depth (one cell's, or those of the cells of one diagonal) come forward a little in the original's order, by their cell's place along the screen's right and their place in the cell's chain (`sprites.wesl`, docs/re/drawing.md), the depth taken from the cell's centre relative to the point the camera looks at; the camera's depth range is 10,000–30,000 units, a thousandth of a unit a step of the depth buffer.
  The ground lies 5,000 units behind along the line of sight, so it never hides an object (the original draws it first); the underlay types go behind the ordinary sprites (draw lists 0 and 2, by the object's mask) and the types of list 4 in front of them (docs/02 §6.1).
  The liquids' surfaces lie between the ground and list 0, and the miniatures of the liquid cells under them, as the original draws those before the surfaces (docs/re/drawing.md).
  The models (the hero, the NPCs and their weapons, the start script's objects) take their cell's place too, after the cell's statics, with a hundredth of their own depth (`models.wesl`, docs/re/drawing.md): a sprite covers a model whole or not at all, as in the original, which draws them in its lists without depth.
* **Water and lava:** each liquid cell shows the frames of its sector's liquid material (`B_WATER`, `C_LAVA`, …), a frame over 4 × 4 cells, cycling in 2,048 ms as the original does, each cell's surface flat at height 0 and each corner as opaque as the bed is deep there, the bed's height times the material's factor, and under the clear waters' surfaces their second layer, the frames again over 2 × 2 cells added to the frame, brighter over the deeper bed and gone by night (docs/re/liquids.md, docs/decisions/2026-10-17-liquids.md).
* **Streaming:** the 3 × 3 sectors around the hero's are built off the main thread, two at a time, and spawned one a frame; farther than two sectors they go.
* **Characters:** the hero (`--class`: any of the seven heroes whose models carry their own clips, the Seraphim, the Gladiator, the Battle Mage, the Wood Elf, the Dark Elf, the Daemon and the Dwarf; no base model of the Vampiress was found) as its glTF with idle, walk and run clips, starting at its start script's `StartPosition` blended by `AnimationTransitions`, at the creature's walk and run speeds (the `R` key toggles walking), round the cells it may not walk on by the client's rule and search, and through the doors onto the buildings' inside layers (docs/re/movement.md); the start script's NPCs in the clips the client plays standing by the class of what they hold (docs/re/motions.md), holding their weapons and shields as the client equips them (docs/re/equipment.md), and its objects (1,990 in the Seraphim's: barrels, chests, book stands, coffins, signposts); the NPCs of the quests the start enters, the Seraphim's novice walking into the chapel as the first quest sends her (docs/decisions/2026-10-19-quest-npcs.md).
  The hero, the NPCs and the chests, barrels and the like of the start script cast the original's shadows, their meshes flattened onto the ground at their height, leaning up and to the right (docs/re/drawing.md, "The models' shadows"; docs/decisions/2026-10-19-model-shadows.md).
* **Creatures:** the creatures of the sectors' `Init` and `Enter` functions and of their `FillSector` records (docs/re/spawning.md), their life while no foe is near (docs/re/ai.md), and the fight between them and the hero (docs/re/fight.md); the buildings' doors open and close (#274).
  The damage numbers (**hypothesis** for their look: the original's is not read) and the creatures' lines (#270) are interface text over the world, in `ANTQS__.TTF` (`fight.rs`, `lines.rs`).
* **HUD:** none of the hero until M5 (docs/decisions/2026-10-17-hud.md); the interface is laid out on a 768-pixel-high screen scaled to the window's height (`hud.rs`), and the original's cursor (`MOUSE_MAIN`, the arrow in the top left 32 × 32 of its image) is scaled in whole steps the same way.
  The menus, the pause, the debug overlay, the log, the console and the keys' help are Bevy UI on the same screen (docs/10).
* **Day and night:** the original's clock (08:00 at the start, a day in an hour) and phases, and the night's colour laid over the frame drawn by a fullscreen pass, as the original multiplies its art by its vertices' colour; lit by night by the light statics and the hero's own light (docs/re/daynight.md, docs/decisions/2026-10-18-night.md).
* **Sound:** the hero's footsteps at the moments of the walk and run clips (docs/02 §4.1), the group of the footstep profile by the nature of the cell underfoot, by the client's own table (docs/re/sound.md); the music of the hero's sector, switching as the hero enters a sector of another music, and the wood tracks of the `music` profile one after another in a sector without (docs/re/sound.md, "The region's music"); beside it, the ambient loop that the nature underfoot calls for, by the client's own switch (docs/re/sound.md), the day's.
  `bevy_kira_audio` decodes a whole track into memory, so only the one playing is loaded.
* **Measured:** at 3840 × 2160 while the hero runs across sectors (`--benchmark`, music on, on a development machine with 32 logical cores), about 690 frames a second on 2026-10-18 with four compute threads (#40), and 750 at 1920 × 1080 with the hero standing; the slowest frames, 30 ms, came when sectors spawned.
  Later the same day, with the ground filtered across the cells' edges, about 570 at 3840 × 2160 (#38); on 2026-10-19, running across sectors, 580 without the overlay and 566 with it (docs/10 §5, #170).
  It has not been measured since the creatures, their life and the fight came (2026-10-20).
  Before the four compute threads, a few frames a run took 30–150 ms, with vsync too and with the hero standing: Bevy's compute pool had 24 threads, and with the main and render threads, the audio's and the rest of the system's there were more threads ready to run than cores, so a thread that lost its core waited for the scheduler's next tick.
  The stalls came in multiples of 15.6 ms, Windows' tick; ordinary systems (`propagate_parent_transforms`, `prepare_lights`) took 20–30 ms in them, a thread of the process that only slept a millisecond at a time woke up to 78 ms late, and another process's such thread up to 31 ms late; the sound crackled with them.
  The game gives the compute pool four threads (`COMPUTE_THREADS`, #2): no stall in the benchmark's runs since, and more frames.
  `--benchmark` counts the process's stalls (a thread of its own woken more than 20 ms late).
* **Looking at frames:** `--screenshot FILE` saves one frame once the scene is ready, `--record DIR` the next `--record-frames` (90) as `DIR/frame-NNNN.png`, with `--target` sending the hero off meanwhile: flicker and stutter show frame by frame.
  A capture of the desktop does not see the window (FFmpeg's `gdigrab` gets the wallpaper behind it).

## 4. Performance budget

| Metric | Target |
|---|---|
| Frames per second at 3840×2160, at the widest view (zoom 2, §3.6) | ≥ 60 on a mid-range graphics card |
| CPU time to prepare a frame | ≤ 4 ms |
| Draw calls per frame | hundreds, not thousands (batching) |
| VRAM for the base assets | ≤ 2 GB (only the hero's surroundings are loaded) |

The game holds its textures as uncompressed RGBA8 without mipmaps, composed per sector: the ground's atlas (13 MB for the median sector) and the sprites' pages of 2,048 texels across (up to 16 MB each); only the textures of the loaded sectors and of the creatures in them are in memory at any one time.
In BC7, which comes with KTX2 in M7, all 25,535 base textures at 256² would take ≈1.6 GB (estimate).

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
| Windows 11 x64 | Vulkan; DX12 as fallback | the main target; backend choice in the settings (planned: the settings hold the display and the debug panels so far, docs/10 §3) |
| Linux | Vulkan | the same functionality |
| Android | Vulkan; GLES as fallback | a smaller streaming radius; ASTC; touch controls |
| Chrome (WebAssembly) | WebGPU; WebGL2 as fallback | no GPU particles on WebGL2; sectors streamed over HTTP from a local server |

## 7. Testing

* **Screenshot tests:** a headless render of test scenes (a sector, a character, an effect) to PNG and comparison with a reference within a tolerance.
  **Implemented 2026-10-17** for a sector: `opensacred-game`'s `scene_tests` writes a synthetic world in the converter's formats (a patterned tile sheet, a frame of water, a sector with a hill, a pond and light, trees and an underlay slab), renders it through the game's terrain, liquid, sprite and camera code into an image with no window, waits until every texture is loaded and no pipeline compiles, and compares it with `crates/opensacred-game/tests/screenshots/scene.png` in blocks of 4 × 4 pixels (at most 1 % of the blocks may differ by more than 12 levels; a 2 % zoom fails).
  It needs no game files, so CI runs it, on Mesa's software Vulkan; the reference was rendered on a development machine's GPU, and Windows' software adapter (WARP, `WGPU_BACKEND=dx12 WGPU_FORCE_FALLBACK_ADAPTER=1`) matches it too, in 70 s instead of 1: neither differs in any block, while a 2 % zoom differs in 6.5 % of them.
* **Cross-check with the original:** reference frames of the original, which runs on Windows 11 without a wrapper in a 1024×768 window (docs/09 §2.2), in 32-bit color mode (`GFX32 : 1`, the GOG default), for visual cross-checking, with no requirement for pixel matching.
  A frame whose draw calls have to be inspected is captured in RenderDoc through dgVoodoo2, since RenderDoc does not capture D3D7 (docs/03 §1).
* **Debugging:** RenderDoc, the Vulkan SDK validation layers, Bevy's built-in diagnostics (frame time, number of draw calls).
  In the game, F3 shows the frame rate and times and F4 the log (docs/10 §5, #170, #169).

## 8. Rendering milestones

| Step | Result | Agent-hours (estimate) | Plan milestone |
|---|---|---|---|
| R1 First sector | terrain mesh, tile atlas, orthographic camera, 4K | 8–15 | M2 |
| R2 Slice | sector streaming, sprite objects, hero with animations, water | 15–30 | M2 |
| R3 Full world | all object types, shadows, day and night lighting, weather, effects (the shadows and day and night came with M2's slice, §3.6) | 25–50 | M5 |
| R4 4K polish | UI scale, graphics settings | 8–15 | M6 |
| R5 HD pipeline | upscaling, KTX2, path-based substitution | 15–30 | M7 |

## Open questions

1. Readiness of `bevy_hanabi` for Bevy 0.20.
2. How well upscaling copes with 4-bit alpha: check on a sample in M7.

## Sources

* Heights, the floor cell and the UV table: docs/02, §5.2–5.4; `tools/analysis/world/uv_table.py`.
  Render states: `tools/analysis/render/d3d7scan.py`.
* Community measurements and workarounds (sacred-bild), behavior at high resolutions: docs/09, §2.
* Bevy 0.20: bevy.org/news/bevy-0-20; wgpu 30.0.1: crates.io.
