//! The weather drawn and heard (#1073; docs/re/weather.md, "What is drawn and heard"): the rain's
//! drops, lines of 5 to 20 of the original's pixels at random anew each frame, slanted by the time
//! of day and fading in from their tops; the fog's two layers of `WEATHER_FOG01` over them,
//! drifting and moved with the camera, at its alpha and tinted by the night; the storm's
//! lightning, a grey added over the screen and a thunder; the snow's flakes of `PARTICLE_SPARK03`,
//! added, falling and swaying as the original's; and the rain's loop, at the volume of its drops
//! over 3,000. None of it in a dungeon. They are drawn by one full-screen material over the world
//! and under the interface (`skies.wesl`), its parameters from the weather's state (`weather`) at
//! each frame, blended as premultiplied colours so that the flash and the flakes add to what lies
//! below. A quake of the screen moves the camera by a random offset within its amplitude at each
//! frame, with the earthquake's sound, in a dungeon too.
//!
//! Not followed yet: the jolts of the big creatures' steps, the butterflies and the birds
//! (#1073); the drops' and the flakes' light is the night's tint over the whole screen, not the
//! light where each falls (`0x41B510`).

use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendState, RenderPipelineDescriptor, ShaderType,
};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial, UiMaterialPlugin};
use bevy::ui_render::ui_material::UiMaterialKey;
use bevy::window::PrimaryWindow;
use bevy_kira_audio::AudioApp;
use bevy_kira_audio::prelude::{AudioChannel, AudioControl};
use opensacred_rules::spawning::Rng;
use opensacred_rules::weather::QUAKE;

use crate::daynight::{self, Clock, Dungeons};
use crate::frame::{Step, Unpaused};
use crate::game::{AppState, Data};
use crate::sfx::{self, Sfx};
use crate::weather::Weather;
use crate::{hero, hud, iso, paths};

/// The shader, embedded in the binary.
const SHADER: &str = "embedded://opensacred_game/skies.wesl";
/// The fog's texture, and the snow's.
const FOG: &str = "weather_fog01";
const SPARK: &str = "particle_spark03";
/// The fog's two layers' drifts, in its texture's widths a second (`+0x180` to `+0x18C`).
const DRIFTS: [Vec2; 2] = [Vec2::new(0.045, 0.028), Vec2::new(0.079, 0.061)];
/// The night's tint of the fog's white, red and green and blue lowered by these shares of the
/// night's level of 255 (`0x890940`).
const NIGHT_TINT: (f32, f32) = (0.508, 0.273);
/// The rain's loop (`SOUND_FX_RAIN01`), at the volume of its drops over [`FULL_RAIN`], and the
/// thunders (`THUNDER01` to `05`).
const RAIN_SOUND: u32 = 205;
const FULL_RAIN: f32 = 3_000.0;
const THUNDERS: [u32; 5] = [200, 201, 202, 203, 204];
/// A lightning's flash: the grey of `0xCC` added over the screen (the quad `+0x3A8`, blended
/// `ONE`, `ONE`), and how long it lasts, 3 to 8 of the original's frames at the 60 a second it
/// draws.
const FLASH: f32 = 0.8;
const FLASH_FRAMES: (f32, f32) = (3.0, 8.0);
const FRAME: f32 = 1.0 / 60.0;
/// The original's frames a second, at which its snow drifts by the slant at each frame.
const FRAMES: f32 = 60.0;
/// The snow's flakes kept (`+0x4B4`), their sizes 2 to 10, how fast they fall and sway by their
/// size, and the height past which a flake's place on the screen wraps (`& 0x3FF`).
const MOST_FLAKES: usize = 3_000;
const FLAKE_SIZES: (f32, f32) = (2.0, 8.0);
const FALL: f32 = 10.0;
const SWAY: f32 = 5.0;
const SWAY_PERIOD: f32 = 0.1;
const WRAP: f32 = 1_024.0;
/// The side of a cell of the snow's grid, in the original's pixels: the most a flake's half side
/// reaches at the closest zoom, so that a flake covers at most two cells along each axis.
const SNOW_CELL: f32 = 32.0;
/// The earthquake's sound (`SOUND_FX_EARTHQUAKE_LOOP`), and how soon after it it plays again, at
/// the soonest.
const QUAKE_SOUND: u32 = 209;
const QUAKE_SOUND_AGAIN: (f32, f32) = (0.1, 0.25);

/// The parameters of [`Skies`], as `skies.wesl` reads them.
#[derive(ShaderType, Clone, Copy, Debug, Default)]
struct Params {
    screen: Vec4,
    rain: Vec4,
    fog: Vec4,
    fog2: Vec4,
    tint: Vec4,
    snow: Vec4,
}

/// The material of the weather's layer.
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
struct Skies {
    #[uniform(0)]
    params: Params,
    #[texture(1)]
    #[sampler(2)]
    fog: Option<Handle<Image>>,
    #[texture(3)]
    #[sampler(4)]
    spark: Option<Handle<Image>>,
    #[storage(5, read_only)]
    flakes: Handle<ShaderBuffer>,
    #[storage(6, read_only)]
    cells: Handle<ShaderBuffer>,
}

impl UiMaterial for Skies {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    /// Premultiplied colours, so that the flash and the flakes add to the frame as the fog and
    /// the drops lie over it.
    fn specialize(descriptor: &mut RenderPipelineDescriptor, _key: UiMaterialKey<Self>) {
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING);
            }
        }
    }
}

/// The weather's layer of a game, its material, the fog's drift, the flash's time left, the
/// frames drawn, whose count seeds the rain, and the time before the earthquake's sound may play
/// again.
#[derive(Component)]
struct Layer {
    material: Handle<Skies>,
    drift: [Vec2; 2],
    flash: f32,
    frames: u32,
    rumble: f32,
}

/// The snow's flakes, as the original keeps them (`+0x4B4`): a place on the screen in its pixels
/// and a size, of which the weather's count falls.
#[derive(Resource)]
struct Flakes {
    flakes: Vec<Vec3>,
    dice: Rng,
}

impl Flakes {
    /// The flakes at random on a screen of `screen` pixels.
    fn new(screen: Vec2, seed: usize) -> Self {
        let mut dice = Rng::of(0, 0, seed);
        let flakes = (0..MOST_FLAKES).map(|_| flake(&mut dice, screen)).collect();
        Self { flakes, dice }
    }

    /// The first `count` flakes fall for `dt` seconds (`0x41A4F0`): down by 10 times their size
    /// a second, across by the slant `drift` and by their sway, `cos(0.1·y)·5·size` a second,
    /// round the screen's sides; one past twice the screen's height is made anew at random on it.
    fn fall(&mut self, count: usize, dt: f32, drift: f32, screen: Vec2) {
        let Self { flakes, dice } = self;
        for f in flakes.iter_mut().take(count) {
            f.y += dt * f.z * FALL;
            f.x += drift + (SWAY_PERIOD * f.y).cos() * dt * f.z * SWAY;
            if f.x > screen.x {
                f.x -= screen.x;
            }
            if f.x < 0.0 {
                f.x += screen.x;
            }
            if f.y > 2.0 * screen.y {
                *f = flake(dice, screen);
            }
        }
    }

    /// The snow's grid over a screen of `screen` pixels at the zoom `zoom`: each cell's flakes in
    /// turn, a centre where it is drawn (its height wrapped at 1,024, as the original's) and a
    /// half side of its size over the zoom; each cell's first entry and count; and the cells
    /// across and down.
    fn grid(&self, count: usize, zoom: f32, screen: Vec2) -> (Vec<Vec4>, Vec<UVec2>, u32, u32) {
        let cols = whole(screen.x / SNOW_CELL).max(1);
        let rows = whole(screen.y / SNOW_CELL).max(1);
        let cell_of = |at: f32, most: u32| whole((at / SNOW_CELL).max(0.0).floor()).min(most - 1);
        let mut lists: Vec<Vec<Vec4>> = vec![Vec::new(); usize::try_from(cols * rows).unwrap_or(0)];
        for f in self.flakes.iter().take(count) {
            let half = f.z / zoom.max(0.1);
            let y = f.y.rem_euclid(WRAP);
            if y - half >= screen.y || f.x - half >= screen.x || f.x + half < 0.0 {
                continue;
            }
            let entry = Vec4::new(f.x, y, half, 0.0);
            for row in cell_of(y - half, rows)..=cell_of(y + half, rows) {
                for col in cell_of(f.x - half, cols)..=cell_of(f.x + half, cols) {
                    if let Some(list) = usize::try_from(row * cols + col)
                        .ok()
                        .and_then(|i| lists.get_mut(i))
                    {
                        list.push(entry);
                    }
                }
            }
        }
        let mut entries = Vec::new();
        let mut cells = Vec::with_capacity(lists.len());
        for list in lists {
            let first = u32::try_from(entries.len()).unwrap_or(u32::MAX);
            cells.push(UVec2::new(first, u32::try_from(list.len()).unwrap_or(0)));
            entries.extend(list);
        }
        if entries.is_empty() {
            entries.push(Vec4::ZERO);
        }
        (entries, cells, cols, rows)
    }
}

/// A flake made at random on a screen of `screen` pixels, of a size of 2 to 10 (`0x41A4F0`).
fn flake(dice: &mut Rng, screen: Vec2) -> Vec3 {
    let mut unit = || to_f32(dice.below(32_768)) * 3.051_851e-5;
    Vec3::new(
        unit() * screen.x,
        unit() * screen.y,
        FLAKE_SIZES.0 + FLAKE_SIZES.1 * unit(),
    )
}

/// The channel of the rain's loop.
#[derive(Resource)]
struct RainLoop;

/// Whether the rain's loop plays.
#[derive(Resource, Default)]
struct Raining(bool);

/// The weather's layer of a game played, over the world and under the interface, and the snow's
/// flakes.
fn spawn(
    mut commands: Commands,
    data: Option<Res<Data>>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<Skies>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
) {
    let found = data.map_or_else(
        || vec![None, None],
        |data| hud::find_textures(&data.0.assets, &[FOG, SPARK]),
    );
    let [fog, spark] = [
        (0, ImageAddressMode::Repeat),
        (1, ImageAddressMode::ClampToEdge),
    ]
    .map(|(i, address)| {
        found.get(i).cloned().flatten().map(|path| {
            assets
                .load_builder()
                .with_settings(move |settings: &mut ImageLoaderSettings| {
                    settings.is_srgb = false;
                    settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                        address_mode_u: address,
                        address_mode_v: address,
                        ..ImageSamplerDescriptor::linear()
                    });
                })
                .load(path)
        })
    });
    let material = materials.add(Skies {
        params: Params::default(),
        fog,
        spark,
        flakes: buffers.add(ShaderBuffer::new(
            vec![Vec4::ZERO],
            RenderAssetUsages::default(),
        )),
        cells: buffers.add(ShaderBuffer::new(
            vec![UVec2::ZERO],
            RenderAssetUsages::default(),
        )),
    });
    commands.insert_resource(Flakes::new(Vec2::new(1_024.0, iso::SCREEN_HEIGHT), 1073));
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        Layer {
            material: material.clone(),
            drift: [Vec2::ZERO; 2],
            flash: 0.0,
            frames: 0,
            rumble: 0.0,
        },
        MaterialNode(material),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        GlobalZIndex(-1),
        Pickable::IGNORE,
    ));
}

/// The slant of the rain's drops across per pixel down at the minute `minutes` of the day:
/// `(2·|t/1439 − 0.5| + 0.2) · cos(2π·t/1440)` (`[+0x74] · [+0x68]`), by which the snow drifts at
/// each of the original's frames too.
fn slant(minutes: f32) -> f32 {
    let lean = 2.0 * (minutes / 1439.0 - 0.5).abs() + 0.2;
    lean * (minutes / 1440.0 * std::f32::consts::TAU).cos()
}

/// The light's colour at the night's level `night` of 255: white by day; red and green lowered by
/// 0.508 of it and blue by 0.273 by night.
fn tint(night: f32) -> Vec3 {
    let (red_green, blue) = NIGHT_TINT;
    Vec3::new(
        1.0 - red_green * night / 255.0,
        1.0 - red_green * night / 255.0,
        1.0 - blue * night / 255.0,
    )
}

/// The weather's layer's parameters at each frame: the fog's and the rain's levels, the fog's
/// offsets by the camera and its drift, the light, a lightning's flash with its thunder, the
/// snow's flakes fallen and gridded, and the earthquake's sound as a quake comes; none but the
/// sound in a dungeon.
#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "a Bevy system takes its resources one by one; the layer's parameters in one place"
)]
fn draw(
    time: Res<Time<Real>>,
    weather: Option<ResMut<Weather>>,
    (clock, dungeons): (Option<Res<Clock>>, Option<Res<Dungeons>>),
    window: Option<Single<&Window, With<PrimaryWindow>>>,
    cameras: Query<(&GlobalTransform, &Projection), crate::iso::Eye<With<crate::srgb::Frame>>>,
    heroes: Query<&Transform, With<hero::Body>>,
    (mut layers, mut flakes): (Query<&mut Layer>, Option<ResMut<Flakes>>),
    (mut materials, mut buffers): (ResMut<Assets<Skies>>, ResMut<Assets<ShaderBuffer>>),
    (mut sfx, audio, assets): (ResMut<Sfx>, Res<bevy_kira_audio::Audio>, Res<AssetServer>),
) {
    let (Some(mut weather), Ok(mut layer)) = (weather, layers.single_mut()) else {
        return;
    };
    let dt = time.delta_secs();
    layer.frames = layer.frames.wrapping_add(1);
    let underground = heroes.iter().next().is_some_and(|at| {
        let (x, y) = paths::cell_of(at.translation.x, at.translation.z);
        let side = i32::try_from(opensacred_data::world::SIDE).unwrap_or(64);
        let sector = (
            u32::try_from(x.div_euclid(side)).unwrap_or(0),
            u32::try_from(y.div_euclid(side)).unwrap_or(0),
        );
        dungeons.as_ref().is_some_and(|d| d.sector(sector))
    });
    let play = |id: u32, sfx: &mut Sfx| {
        if let Some((id, file)) = sfx.with_id(id) {
            let sample = sfx.sample(id, file, &assets);
            audio.play(sample).with_volume(sfx::decibels(1.0));
        }
    };
    // A lightning: a flash and one of the thunders, but in a dungeon.
    let lightnings = std::mem::take(&mut weather.lightnings);
    if lightnings > 0 && !underground {
        let frames =
            FLASH_FRAMES.0 + (FLASH_FRAMES.1 - FLASH_FRAMES.0 + 1.0) * sfx.roll(1000) / 1000.0;
        layer.flash = frames.floor() * FRAME;
        let thunder = THUNDERS[usize::try_from(sfx.dice.below(5)).unwrap_or(0)];
        play(thunder, &mut sfx);
    }
    // A quake's sound, at most once in its time (`0x697220`).
    layer.rumble = (layer.rumble - dt).max(0.0);
    if std::mem::take(&mut weather.quakes) > 0 && layer.rumble <= 0.0 {
        let (soonest, more) = QUAKE_SOUND_AGAIN;
        layer.rumble = soonest + more * sfx.roll(1000) / 1000.0;
        play(QUAKE_SOUND, &mut sfx);
    }
    let flash = if layer.flash > 0.0 { FLASH } else { 0.0 };
    layer.flash = (layer.flash - dt).max(0.0);
    let state = &weather.state;
    let (drops, fog, snow) = if underground {
        (0, 0, 0)
    } else {
        (state.rain.now, state.fog.now.min(255), state.snow.now)
    };
    for (drift, speed) in layer.drift.iter_mut().zip(DRIFTS) {
        *drift = (*drift + speed * dt).fract_gl();
    }
    let height = window.as_ref().map_or(iso::SCREEN_HEIGHT, |w| w.height());
    let width = window.as_ref().map_or(1024.0, |w| w.width());
    let screen = Vec2::new(
        width * iso::SCREEN_HEIGHT / height.max(1.0),
        iso::SCREEN_HEIGHT,
    );
    let (camera, zoom) =
        cameras
            .iter()
            .next()
            .map_or((Vec2::ZERO, 1.0), |(transform, projection)| {
                let position = transform.translation();
                let zoom = match projection {
                    Projection::Orthographic(ortho) => ortho.scale,
                    _ => 1.0,
                };
                (
                    Vec2::new(
                        position.dot(*transform.right()),
                        -position.dot(*transform.up()),
                    ) * iso::PIXELS_PER_UNIT,
                    zoom,
                )
            });
    // The fog's offset by the camera's place in screens, and by the span's centring.
    let base =
        ((camera - screen / 2.0).rem_euclid(screen)) / screen + Vec2::splat((1.0 - zoom) / 2.0);
    let minutes = clock.as_ref().map_or(480.0, |clock| clock.minutes());
    let night = narrow(daynight::level(minutes));
    let minutes = narrow(minutes);
    let light = tint(night);
    // The snow's flakes fall, those of the weather's count, and lie in the grid the shader reads.
    let count = usize::try_from(snow).unwrap_or(0).min(MOST_FLAKES);
    let mut grid = Vec4::new(1.0, 1.0, SNOW_CELL, 0.0);
    if let Some(flakes) = flakes.as_deref_mut()
        && count > 0
    {
        flakes.fall(count, dt, slant(minutes) * FRAMES * dt, screen);
        let (entries, cells, cols, rows) = flakes.grid(count, zoom, screen);
        grid = Vec4::new(to_f32(cols), to_f32(rows), SNOW_CELL, 1.0);
        if let Some(material) = materials.get(&layer.material) {
            for (handle, data) in [
                (
                    &material.flakes,
                    ShaderBuffer::new(entries, RenderAssetUsages::default()),
                ),
                (
                    &material.cells,
                    ShaderBuffer::new(cells, RenderAssetUsages::default()),
                ),
            ] {
                if let Some(mut buffer) = buffers.get_mut(handle) {
                    *buffer = data;
                }
            }
        }
    }
    if let Some(mut material) = materials.get_mut(&layer.material) {
        material.params = Params {
            screen: Vec4::new(screen.x, screen.y, zoom, to_f32(layer.frames % 65_536)),
            rain: Vec4::new(to_f32(drops), slant(minutes), 0.0, 0.0),
            fog: Vec4::new(
                to_f32(fog) / 255.0,
                base.x + layer.drift[0].x,
                base.y + layer.drift[0].y,
                0.0,
            ),
            fog2: Vec4::new(
                base.x + layer.drift[1].x,
                base.y + layer.drift[1].y,
                0.0,
                0.0,
            ),
            tint: light.extend(flash),
            snow: grid,
        };
    }
}

/// The screen's quake (`0x4190D0`, `0x6284E0`): while one lasts, the camera moves by a random
/// offset within its amplitude across and down, in the original's pixels, at each frame but in a
/// pause, while the amplitude stands.
fn shake(
    weather: Option<Res<Weather>>,
    mut dice: Local<Option<Rng>>,
    mut cameras: Query<(&mut Transform, &Projection), crate::iso::Eye<With<crate::srgb::Frame>>>,
) {
    let Some(weather) = weather else {
        return;
    };
    let state = &weather.state;
    let quakes = state.flags & QUAKE != 0 && state.quake > 0.0;
    if !quakes && state.jolt <= 0.0 {
        return;
    }
    let dice = dice.get_or_insert_with(|| Rng::of(0, 0, 7));
    let mut offset = || {
        let unit = to_f32(dice.below(32_768)) * 3.051_851e-5;
        (state.quake * (2.0 * unit - 1.0)).trunc()
    };
    let (across, down) = if quakes {
        (offset(), offset())
    } else {
        (0.0, 0.0)
    };
    // A jolt moves the view down by its pixels as it falls (docs/re/weather.md, "The jolt").
    let down = down + state.jolt;
    for (mut transform, projection) in &mut cameras {
        let zoom = match projection {
            Projection::Orthographic(ortho) => ortho.scale,
            _ => 1.0,
        };
        let units = zoom / iso::PIXELS_PER_UNIT;
        let shift = transform.right() * across * units - transform.up() * down * units;
        transform.translation += shift;
    }
}

/// The rain's loop while its drops fall, at their count over 3,000, stopped at none.
fn rain_loop(
    weather: Option<Res<Weather>>,
    channel: Res<AudioChannel<RainLoop>>,
    mut raining: ResMut<Raining>,
    (mut sfx, assets): (ResMut<Sfx>, Res<AssetServer>),
) {
    let drops = weather.map_or(0, |weather| weather.state.rain.now);
    if drops == 0 {
        if raining.0 {
            channel.stop();
            raining.0 = false;
        }
        return;
    }
    channel.set_volume(sfx::decibels((to_f32(drops) / FULL_RAIN).min(1.0)));
    if !raining.0
        && let Some((id, file)) = sfx.with_id(RAIN_SOUND)
    {
        let sample = sfx.sample(id, file, &assets);
        channel.play(sample).looped();
        raining.0 = true;
    }
}

/// Stops the rain's loop as the game ends, and forgets the snow.
fn hush(
    mut commands: Commands,
    channel: Res<AudioChannel<RainLoop>>,
    mut raining: ResMut<Raining>,
) {
    channel.stop();
    raining.0 = false;
    commands.remove_resource::<Flakes>();
}

/// A double as a float.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a minute of the day or the night's level, far within a float's range"
)]
fn narrow(value: f64) -> f32 {
    value as f32
}

/// A count as a float.
fn to_f32(count: u32) -> f32 {
    f32::from(u16::try_from(count.min(u32::from(u16::MAX))).unwrap_or(u16::MAX))
}

/// The whole cells that cover `span` cells, rounded up.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a screen's cells, a few dozen"
)]
fn whole(span: f32) -> u32 {
    span.ceil().clamp(0.0, 4_096.0) as u32
}

/// The weather drawn and heard.
pub struct SkiesPlugin;

impl Plugin for SkiesPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "skies.wesl");
        app.add_plugins(UiMaterialPlugin::<Skies>::default())
            .add_audio_channel::<RainLoop>()
            .init_resource::<Raining>()
            .add_systems(OnEnter(AppState::InGame), spawn)
            .add_systems(OnExit(AppState::InGame), hush)
            .add_systems(
                Update,
                (draw, rain_loop)
                    .run_if(in_state(AppState::InGame))
                    .run_if(resource_exists::<Sfx>),
            )
            .add_systems(
                Update,
                shake
                    .after(hero::follow)
                    .in_set(Step::Follow)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_drops_lean_by_the_time_of_day() {
        // At midnight the most, 1.2 to the right; at noon 0.2 to the left.
        assert!((slant(0.0) - 1.2).abs() < 1e-3);
        assert!((slant(720.0) + 0.2).abs() < 1e-2);
    }

    #[test]
    fn the_night_tints_the_light_blue() {
        assert_eq!(tint(0.0), Vec3::ONE);
        let night = tint(255.0);
        assert!((night.x - 0.492).abs() < 1e-3 && (night.z - 0.727).abs() < 1e-3);
    }

    #[test]
    fn a_flake_falls_by_its_size_sways_and_wraps_round_the_sides() {
        let screen = Vec2::new(1024.0, 768.0);
        let mut flakes = Flakes::new(screen, 1);
        flakes.flakes[0] = Vec3::new(1023.0, 0.0, 4.0);
        flakes.flakes[1] = Vec3::new(10.0, 1535.0, 10.0);
        flakes.fall(2, 0.5, 3.0, screen);
        // Down by 10 · 4 · 0.5, across by the drift and the sway cos(0.1 · 20) · 0.5 · 4 · 5,
        // past the right side and round to the left.
        let sway = (0.1_f32 * 20.0).cos() * 10.0;
        let x = 1023.0 + 3.0 + sway - 1024.0;
        assert!((flakes.flakes[0].y - 20.0).abs() < 1e-3);
        assert!((flakes.flakes[0].x - x.rem_euclid(1024.0)).abs() < 1e-3);
        // Past twice the screen's height: made anew on the screen, of a size of 2 to 10.
        let anew = flakes.flakes[1];
        assert!(anew.y < 768.0 && (2.0..=10.0).contains(&anew.z));
    }

    #[test]
    fn the_grid_lists_each_cell_with_the_flakes_that_cover_it() {
        let screen = Vec2::new(64.0, 64.0);
        let mut flakes = Flakes::new(screen, 1);
        // A flake of half side 4 at (29, 1053): drawn at the height 29, over the four cells
        // about (32, 32).
        flakes.flakes[0] = Vec3::new(29.0, 1053.0, 4.0);
        let (entries, cells, cols, rows) = flakes.grid(1, 1.0, screen);
        assert_eq!((cols, rows), (2, 2));
        assert_eq!(cells.iter().map(|c| c.y).collect::<Vec<_>>(), [1, 1, 1, 1]);
        assert_eq!(entries[0], Vec4::new(29.0, 29.0, 4.0, 0.0));
        // At the zoom 2 its half side is 2, within the first cell alone.
        let (_, cells, ..) = flakes.grid(1, 2.0, screen);
        assert_eq!(cells.iter().map(|c| c.y).collect::<Vec<_>>(), [1, 0, 0, 0]);
    }
}
