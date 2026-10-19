//! Screenshot tests of the scene (M2's exit criterion, docs/05 §7). A synthetic world is written
//! to a scratch directory in the converter's formats: a tile sheet of patterned colours, a frame
//! of water, a sector with a hill, a pond, lighter and darker corners and a few sprite objects,
//! one of them an underlay. It is rendered without a window into an image, through the game's
//! own terrain, liquid, sprite and camera code, and compared with the reference
//! `tests/screenshots/scene.png` within a tolerance that absorbs the differences between GPUs. No
//! game files are needed, but a GPU adapter is (CI uses Mesa's software Vulkan, lavapipe), so the
//! test is ignored by default. `UPDATE_SCREENSHOTS=1` rewrites the reference; a failing run leaves
//! its image next to it as `scene.actual.png`.

#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "the synthetic pixels, heights and positions are small values that fit"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::render::render_resource::{PipelineCache, TextureFormat};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;
use opensacred_data::world::{
    Cell, DIAMONDS, Piece, SIDE, Sector, SectorEntry, Sprite, Sprites, StaticObject, World,
};

use crate::{blended, iso, liquids, shaders, srgb, statics, terrain};

const WIDTH: u32 = 640;
const HEIGHT: u32 = 480;
/// The sector's place on the grid.
const AT: (u32, u32) = (10, 12);
/// Frames in a row with every texture loaded and no pipeline compiling before the capture.
const SETTLED_FRAMES: u32 = 20;
const TIMEOUT: Duration = Duration::from_secs(600);
/// The comparison: blocks of 4 × 4 pixels, averaged; a block differs when a channel differs by
/// more than `BLOCK_TOLERANCE`; at most `DIFFERENT_BLOCKS` of them may.
const BLOCK: usize = 4;
const BLOCK_TOLERANCE: f32 = 12.0;
const DIFFERENT_BLOCKS: f32 = 0.01;

fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) {
    let file = fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(rgba)
        .unwrap();
}

fn read_png(path: &Path) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::BufReader::new(fs::File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba, "{}", path.display());
    pixels.truncate(info.buffer_size());
    (info.width, info.height, pixels)
}

/// A tile sheet: diamond `k` a two-tone checker of its own hue, so that a wrong diamond, a
/// mirrored one or blurred filtering shows (docs/02 §5.6 for the layout).
fn sheet() -> Vec<u8> {
    let mut rgba = vec![0u8; 256 * 256 * 4];
    for k in 0..DIAMONDS {
        let (row, column) = (k / 2, k % 2);
        let shift = if row % 2 == 1 { 52 } else { 0 };
        let (ox, oy) = (104 * column + shift + 2, 25 * row);
        let hue = k as f32 / DIAMONDS as f32;
        let base = Color::hsl(hue * 360.0, 0.55, 0.45).to_srgba();
        for y in 0..48 {
            for x in 0..96 {
                let dx = (x as f32 + 0.5 - 48.0).abs() / 48.0;
                let dy = (y as f32 + 0.5 - 24.0).abs() / 24.0;
                if dx + dy > 1.0 {
                    continue;
                }
                let light = if (x / 8 + y / 4) % 2 == 0 { 1.0 } else { 0.75 };
                let (px, py) = (ox + x, oy + y);
                let i = ((py * 256 + px) * 4) as usize;
                if px < 256 && py < 256 {
                    rgba[i..i + 4].copy_from_slice(&[
                        (base.red * light * 255.0) as u8,
                        (base.green * light * 255.0) as u8,
                        (base.blue * light * 255.0) as u8,
                        255,
                    ]);
                }
            }
        }
    }
    rgba
}

/// A sprite atlas of 128 × 128: a tree (a trunk and a round crown with soft edges) on the left, a
/// flat stone slab on the right.
fn atlas() -> Vec<u8> {
    let mut rgba = vec![0u8; 128 * 128 * 4];
    let mut put = |x: usize, y: usize, c: [u8; 4]| {
        let i = (y * 128 + x) * 4;
        rgba[i..i + 4].copy_from_slice(&c);
    };
    for y in 0..128 {
        for x in 0..64 {
            let (dx, dy) = (x as f32 - 32.0, y as f32 - 40.0);
            let r = (dx * dx + dy * dy).sqrt();
            if r < 30.0 {
                let shade = (200.0 - r * 3.0 + dx) as u8;
                put(x, y, [30, shade, 40, 255]);
            } else if (28..36).contains(&x) && y >= 60 {
                put(x, y, [110, 70, 30, 255]);
            }
        }
        for x in 64..128 {
            let (dx, dy) = ((x as f32 - 96.0) / 30.0, (y as f32 - 96.0) / 15.0);
            if dx.abs() + dy.abs() <= 1.0 {
                let grain = if (x / 6 + y / 3) % 2 == 0 { 150 } else { 130 };
                put(x, y, [grain, grain, grain + 10, 255]);
            }
        }
    }
    rgba
}

/// A frame of the first liquid material (`b_water`): diagonal ripples of two blues.
fn water() -> Vec<u8> {
    let mut rgba = Vec::with_capacity(128 * 128 * 4);
    for y in 0..128usize {
        for x in 0..128usize {
            let ripple = ((x + 2 * y) / 8) % 2 == 0;
            rgba.extend(if ripple {
                [40, 110, 170, 255]
            } else {
                [20, 70, 130, 255]
            });
        }
    }
    rgba
}

/// The height in pixels of grid point `(x, y)` of the sector: a hill in the north, a hollow
/// for the pond in the south.
fn height(x: f32, y: f32) -> i8 {
    let hill = 60.0 * (-((x - 24.0).powi(2) + (y - 22.0).powi(2)) / 40.0).exp();
    let hollow = -20.0 * (-((x - 38.0).powi(2) + (y - 34.0).powi(2)) / 12.0).exp();
    (hill + hollow).round() as i8
}

fn sector() -> Sector {
    let mut cells = Vec::with_capacity(SIDE * SIDE);
    for row in 0..SIDE {
        for column in 0..SIDE {
            let (c, r) = (column as f32, row as f32);
            let corners = [(c, r + 1.0), (c, r), (c + 1.0, r), (c + 1.0, r + 1.0)];
            let heights = corners.map(|(x, y)| height(x, y));
            // Light falls off toward the east.
            let light = corners.map(|(x, _)| (255.0 - x * 1.5) as u8);
            let pond = (column as i32 - 38).pow(2) + (row as i32 - 34).pow(2) < 10;
            cells.push(Cell {
                tile: ((row / 6 + column / 6) % DIAMONDS as usize) as u32,
                overlays: Vec::new(),
                heights,
                light,
                heights2: [0; 4],
                offsets: [0; 2],
                flags: 0,
                ground: if pond { 0x90 } else { 0x10 },
            });
        }
    }
    let object = |row: usize, column: usize, sprite: u32, type_flags: u32, size: (i32, i32)| {
        let cx = (AT.0 as usize * SIDE + column) as i32;
        let cy = (AT.1 as usize * SIDE + row) as i32;
        // The sprite's bottom centre at the cell's centre on screen.
        let (sx, sy) = (48 * (cx - cy), 24 * (cx + cy) + 24);
        StaticObject {
            cell: u16::try_from(row * SIDE + column).unwrap(),
            kind: 1,
            flags: 0,
            x: sx - size.0 / 2,
            y: sy - size.1,
            record: 0,
            type_flags,
            mask: 0,
            sprite,
            miniature: None,
            shadow: None,
            children: Vec::new(),
        }
    };
    let statics = vec![
        // A slab under the tree beside it: an underlay draws behind it, though in front on the
        // diagonal.
        object(31, 31, 2, 0x04, (64, 32)),
        object(30, 30, 1, 25, (64, 128)),
        object(34, 30, 1, 25, (64, 128)),
        object(28, 36, 1, 25, (64, 128)),
    ];
    Sector {
        x: AT.0,
        y: AT.1,
        id: 1,
        liquids: [0, 0],
        cells,
        statics,
        layers: Vec::new(),
    }
}

fn sprites() -> Sprites {
    Sprites {
        sprites: vec![
            None,
            Some(Sprite {
                width: 64,
                height: 128,
                pieces: vec![Piece {
                    atlas: 0,
                    source: [0, 0, 64, 128],
                    rect: [0, 0, 64, 128],
                }],
            }),
            Some(Sprite {
                width: 64,
                height: 32,
                pieces: vec![Piece {
                    atlas: 0,
                    source: [64, 80, 128, 112],
                    rect: [0, 0, 64, 32],
                }],
            }),
        ],
    }
}

/// Writes the synthetic world under `dir/world`.
fn write_world(dir: &Path) -> (World, Sector, Sprites) {
    let world_dir = dir.join("world");
    fs::create_dir_all(world_dir.join("sectors")).unwrap();
    write_png(&world_dir.join("sheet.png"), 256, 256, &sheet());
    write_png(&world_dir.join("atlas.png"), 128, 128, &atlas());
    let textures = dir.join("textures").join("texture");
    fs::create_dir_all(&textures).unwrap();
    write_png(&textures.join("00001-b_water00.png"), 128, 128, &water());
    let sector = sector();
    let file = format!("sectors/{}-{}.sector", AT.0, AT.1);
    fs::write(world_dir.join(&file), sector.encode().unwrap()).unwrap();
    let sprites = sprites();
    fs::write(
        world_dir.join(opensacred_data::world::SPRITES_FILE),
        sprites.encode().unwrap(),
    )
    .unwrap();
    let world = World {
        sectors: vec![SectorEntry {
            x: AT.0,
            y: AT.1,
            id: 1,
            file,
            music: 0,
            area: 0,
            dungeon: false,
        }],
        sheets: vec!["sheet.png".into()],
        atlases: vec!["atlas.png".into()],
    };
    let text = ron::ser::to_string_pretty(&world, ron::ser::PrettyConfig::default()).unwrap();
    fs::write(world_dir.join(opensacred_data::world::INDEX), text).unwrap();
    (world, sector, sprites)
}

/// What the startup system spawns.
#[derive(Resource)]
struct Scene {
    ground: Option<terrain::Ground>,
    liquids: Option<liquids::Surfaces>,
    statics: Option<statics::Statics>,
    target: Handle<Image>,
    look_at: Vec3,
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources one by one"
)]
fn spawn_scene(
    mut commands: Commands,
    mut scene: ResMut<Scene>,
    mut liquids: ResMut<liquids::Liquids>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<blended::Sorted>>,
    mut sprite_materials: ResMut<Assets<statics::SpriteMaterial>>,
    fade: Res<statics::Fade>,
    mut grounds: ResMut<Assets<terrain::GroundMaterial>>,
) {
    if let Some(ground) = scene.ground.take() {
        terrain::spawn(
            &mut commands,
            ground,
            &mut images,
            &mut meshes,
            &mut grounds,
        );
    }
    if let Some(surfaces) = scene.liquids.take() {
        liquids::spawn(
            &mut commands,
            surfaces,
            &mut liquids,
            &assets,
            &mut meshes,
            &mut materials,
        );
    }
    if let Some(statics) = scene.statics.take() {
        statics::spawn(
            &mut commands,
            statics,
            &assets,
            &mut images,
            &mut meshes,
            &mut sprite_materials,
            statics::Common {
                fade: &fade,
                shadows: None,
            },
        );
    }
    commands.spawn((
        iso::camera(scene.look_at, 1.0),
        RenderTarget::Image(scene.target.clone().into()),
    ));
}

/// Whether every material's texture is loaded and the render world compiles no pipeline.
fn settled(app: &App) -> bool {
    let world = app.world();
    let images = world.resource::<Assets<Image>>();
    let loaded = world
        .resource::<Assets<blended::Sorted>>()
        .iter()
        .all(|(_, m)| {
            m.base
                .base_color_texture
                .as_ref()
                .is_none_or(|t| images.contains(t))
        })
        && world
            .resource::<Assets<statics::SpriteMaterial>>()
            .iter()
            .all(|(_, m)| images.contains(&m.atlas));
    let compiling = app
        .get_sub_app(RenderApp)
        .and_then(|render| render.world().get_resource::<PipelineCache>())
        .is_none_or(|cache| cache.waiting_pipelines().next().is_some());
    loaded && !compiling
}

/// Renders the scene of `dir` and returns its pixels, once it has settled. One capture: on
/// Windows' software adapter (WARP) a second one never arrives.
fn render(dir: &Path) -> Vec<u8> {
    let (world, sector, sprites) = write_world(dir);
    let ground = terrain::build(dir, &world, &sector).unwrap();
    let surfaces = liquids::build(&sector);
    let statics = statics::build(dir, &world, &sprites, None, &sector).unwrap();
    assert_eq!(statics.drawn, 4);
    let look_at = terrain::center(&sector);

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                close_when_requested: false,
                ..default()
            })
            .set(AssetPlugin {
                file_path: dir.to_string_lossy().into_owned(),
                ..default()
            })
            .disable::<WinitPlugin>()
            .disable::<LogPlugin>()
            // The render world stays in the app, so that its pipelines can be checked.
            .disable::<PipelinedRenderingPlugin>(),
    )
    .add_plugins((
        blended::BlendedPlugin,
        shaders::ShaderLibrary,
        srgb::SrgbPlugin,
        statics::StaticsPlugin,
        terrain::GroundPlugin,
    ));
    let mut target = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
    target.asset_usage = RenderAssetUsages::RENDER_WORLD;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(target);
    app.insert_resource(ClearColor(Color::BLACK))
        .insert_resource(liquids::Liquids::read(dir).unwrap())
        .insert_resource(Scene {
            ground: Some(ground),
            liquids: Some(surfaces),
            statics: Some(statics),
            target: target.clone(),
            look_at,
        })
        .add_systems(Startup, spawn_scene);
    app.finish();
    app.cleanup();

    let start = Instant::now();
    let captured: Arc<Mutex<Option<Vec<u8>>>> = Arc::default();
    let mut settled_frames = 0;
    let mut waiting = false;
    // What the loop saw, for the message if it times out.
    let (mut frames, mut most_settled) = (0u32, 0u32);
    loop {
        assert!(
            start.elapsed() < TIMEOUT,
            "no frame in {TIMEOUT:?}: {frames} frames, at most {most_settled} settled in a row, \
             capture requested: {waiting}"
        );
        app.update();
        frames += 1;
        settled_frames = if settled(&app) { settled_frames + 1 } else { 0 };
        most_settled = most_settled.max(settled_frames);
        if settled_frames < SETTLED_FRAMES {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }
        if !waiting {
            let sink = captured.clone();
            app.world_mut()
                .spawn(Screenshot::image(target.clone()))
                .observe(move |event: On<ScreenshotCaptured>| {
                    *sink.lock().unwrap() = event.image.data.clone();
                });
            waiting = true;
            continue;
        }
        if let Some(pixels) = captured.lock().unwrap().take() {
            return pixels;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The share of 4 × 4 blocks whose average differs by more than the tolerance in a channel.
fn different_blocks(a: &[u8], b: &[u8]) -> f32 {
    let (w, h) = (WIDTH as usize / BLOCK, HEIGHT as usize / BLOCK);
    let mut different = 0;
    for by in 0..h {
        for bx in 0..w {
            let mut sums = [0.0f32; 6];
            for y in by * BLOCK..(by + 1) * BLOCK {
                for x in bx * BLOCK..(bx + 1) * BLOCK {
                    let i = (y * WIDTH as usize + x) * 4;
                    for c in 0..3 {
                        sums[c] += f32::from(a[i + c]);
                        sums[3 + c] += f32::from(b[i + c]);
                    }
                }
            }
            let n = (BLOCK * BLOCK) as f32;
            if (0..3).any(|c| (sums[c] - sums[3 + c]).abs() / n > BLOCK_TOLERANCE) {
                different += 1;
            }
        }
    }
    different as f32 / (w * h) as f32
}

fn reference() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/screenshots/scene.png")
}

#[test]
#[ignore = "needs a GPU adapter (CI: Mesa's lavapipe)"]
fn the_scene_matches_its_screenshot() {
    let dir = std::env::temp_dir().join(format!("opensacred-game-scene-{}", std::process::id()));
    let pixels = render(&dir);
    fs::remove_dir_all(&dir).unwrap();
    assert_eq!(pixels.len(), (WIDTH * HEIGHT * 4) as usize);
    let reference = reference();
    if std::env::var_os("UPDATE_SCREENSHOTS").is_some() {
        fs::create_dir_all(reference.parent().unwrap()).unwrap();
        write_png(&reference, WIDTH, HEIGHT, &pixels);
        return;
    }
    let (width, height, expected) = read_png(&reference);
    assert_eq!((width, height), (WIDTH, HEIGHT));
    let share = different_blocks(&pixels, &expected);
    if share > DIFFERENT_BLOCKS {
        let actual = reference.with_file_name("scene.actual.png");
        write_png(&actual, WIDTH, HEIGHT, &pixels);
        panic!(
            "{:.1} % of the blocks differ from {} (at most {:.1} %); this run's image is {}",
            share * 100.0,
            reference.display(),
            DIFFERENT_BLOCKS * 100.0,
            actual.display()
        );
    }
}
