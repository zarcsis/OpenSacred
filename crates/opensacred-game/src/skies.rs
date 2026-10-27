//! The weather drawn and heard (#1073; docs/re/weather.md, "What is drawn and heard"): the fog's
//! two layers of `WEATHER_FOG01`, drifting and moved with the camera, at its alpha and tinted by
//! the night; the rain's drops, lines of 5 to 20 of the original's pixels at random anew each
//! frame, slanted by the time of day and fading in from their tops; the storm's lightning, a grey
//! added over the screen and a thunder; and the rain's loop, at the volume of its drops over
//! 3,000. None of it in a dungeon. They are drawn by one full-screen material over the world and
//! under the interface (`skies.wesl`), its parameters from the weather's state (`weather`) at
//! each frame, blended as premultiplied colours so that the flash adds to what lies below.
//!
//! Not followed yet: the snow, the quakes and jolts, the butterflies and the birds (#1073); the
//! drops' light is the night's tint over the whole screen, not the light where each falls
//! (`0x41B510`).

use bevy::asset::embedded_asset;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendState, RenderPipelineDescriptor, ShaderType,
};
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial, UiMaterialPlugin};
use bevy::ui_render::ui_material::UiMaterialKey;
use bevy::window::PrimaryWindow;
use bevy_kira_audio::AudioApp;
use bevy_kira_audio::prelude::{AudioChannel, AudioControl};

use crate::daynight::{self, Clock, Dungeons};
use crate::game::{AppState, Data};
use crate::sfx::{self, Sfx};
use crate::weather::Weather;
use crate::{hero, hud, iso, paths};

/// The shader, embedded in the binary.
const SHADER: &str = "embedded://opensacred_game/skies.wesl";
/// The fog's texture.
const FOG: &str = "weather_fog01";
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

/// The parameters of [`Skies`], as `skies.wesl` reads them.
#[derive(ShaderType, Clone, Copy, Debug, Default)]
struct Params {
    screen: Vec4,
    rain: Vec4,
    fog: Vec4,
    fog2: Vec4,
    tint: Vec4,
}

/// The material of the weather's layer.
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
struct Skies {
    #[uniform(0)]
    params: Params,
    #[texture(1)]
    #[sampler(2)]
    fog: Option<Handle<Image>>,
}

impl UiMaterial for Skies {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    /// Premultiplied colours, so that the flash adds to the frame as the fog and the drops lie
    /// over it.
    fn specialize(descriptor: &mut RenderPipelineDescriptor, _key: UiMaterialKey<Self>) {
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING);
            }
        }
    }
}

/// The weather's layer of a game, its material, the fog's drift, the flash's time left and the
/// frames drawn, whose count seeds the rain.
#[derive(Component)]
struct Layer {
    material: Handle<Skies>,
    drift: [Vec2; 2],
    flash: f32,
    frames: u32,
}

/// The channel of the rain's loop.
#[derive(Resource)]
struct RainLoop;

/// Whether the rain's loop plays.
#[derive(Resource, Default)]
struct Raining(bool);

/// The weather's layer of a game played, over the world and under the interface.
fn spawn(
    mut commands: Commands,
    data: Option<Res<Data>>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<Skies>>,
) {
    let fog = data.and_then(|data| {
        hud::find_textures(&data.0.assets, &[FOG])
            .pop()
            .flatten()
            .map(|path| {
                assets
                    .load_builder()
                    .with_settings(|settings: &mut ImageLoaderSettings| {
                        settings.is_srgb = false;
                        settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                            address_mode_u: ImageAddressMode::Repeat,
                            address_mode_v: ImageAddressMode::Repeat,
                            ..ImageSamplerDescriptor::linear()
                        });
                    })
                    .load(path)
            })
    });
    let material = materials.add(Skies {
        params: Params::default(),
        fog,
    });
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        Layer {
            material: material.clone(),
            drift: [Vec2::ZERO; 2],
            flash: 0.0,
            frames: 0,
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
/// `(2·|t/1439 − 0.5| + 0.2) · cos(2π·t/1440)` (`[+0x74] · [+0x68]`).
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
/// offsets by the camera and its drift, the light, and a lightning's flash with its thunder; none
/// in a dungeon.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources one by one"
)]
fn draw(
    time: Res<Time<Real>>,
    weather: Option<ResMut<Weather>>,
    (clock, dungeons): (Option<Res<Clock>>, Option<Res<Dungeons>>),
    window: Option<Single<&Window, With<PrimaryWindow>>>,
    cameras: Query<(&GlobalTransform, &Projection), With<crate::srgb::Frame>>,
    heroes: Query<&Transform, With<hero::Body>>,
    mut layers: Query<&mut Layer>,
    mut materials: ResMut<Assets<Skies>>,
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
    // A lightning: a flash and one of the thunders, but in a dungeon.
    let lightnings = std::mem::take(&mut weather.lightnings);
    if lightnings > 0 && !underground {
        let frames =
            FLASH_FRAMES.0 + (FLASH_FRAMES.1 - FLASH_FRAMES.0 + 1.0) * sfx.roll(1000) / 1000.0;
        layer.flash = frames.floor() * FRAME;
        let thunder = THUNDERS[usize::try_from(sfx.dice.below(5)).unwrap_or(0)];
        if let Some((id, file)) = sfx.with_id(thunder) {
            let sample = sfx.sample(id, file, &assets);
            audio.play(sample).with_volume(sfx::decibels(1.0));
        }
    }
    let flash = if layer.flash > 0.0 { FLASH } else { 0.0 };
    layer.flash = (layer.flash - dt).max(0.0);
    let state = &weather.state;
    let (drops, fog) = if underground {
        (0.0, 0.0)
    } else {
        (
            to_f32(state.rain.now),
            to_f32(state.fog.now.min(255)) / 255.0,
        )
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
    if let Some(mut material) = materials.get_mut(&layer.material) {
        material.params = Params {
            screen: Vec4::new(screen.x, screen.y, zoom, to_f32(layer.frames % 65_536)),
            rain: Vec4::new(drops, slant(minutes), 0.0, 0.0),
            fog: Vec4::new(
                fog,
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
        };
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

/// Stops the rain's loop as the game ends.
fn hush(channel: Res<AudioChannel<RainLoop>>, mut raining: ResMut<Raining>) {
    channel.stop();
    raining.0 = false;
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
}
