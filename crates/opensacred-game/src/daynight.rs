//! Day and night (docs/re/daynight.md): the game's clock, its phases and the night's darkness.
//!
//! The clock starts at 08:00 and goes 24 seconds of the day a second, as the original's does
//! (**hypothesis** for the unit of the frame's time: a day lasts an hour). By the minutes of the
//! day, the dawn comes at 03:20, the day at 05:20, the dusk at 20:20 and the night at 22:20; the
//! night's level is 0 by day, 255 by night, and goes between them over the two hours of the dusk
//! and of the dawn.
//!
//! The original colours every point of the world by the light around it and the night's level:
//! by night a ramp of the light's level, built from the setting `NIGHT_DARKNESS` (50), and
//! `255 − level` added to each channel, so that the day is white. The game draws the frame first
//! and then multiplies it by that colour (`night.wesl`, a fullscreen pass): the same for the art,
//! which the original multiplies by its vertices' colour, if not for its blending.
//!
//! The light's level at a point is 32 and, for every light source whose radius reaches it, the
//! radius less the distance, at most 223 each and 255 in all, in the pixels of the original's
//! screen at zoom 1 (`0x41B510`). The sources are the static objects of the types flagged
//! `0x20000` (the invisible `SimpleLightTiny`, `Small`, `Medium` and `Big`, of radii 220, 300, 450
//! and 600, some miniatures, the fires), each of its type's radius (`items.pak +0x32`), and the
//! hero, of radius 1000 − level · 700/255 between the dusk and the dawn, twice that for the Wood
//! Elf (`0x54A7B0`). In a dungeon (`world.ron`'s `dungeon`, keyx's entries of code 4) the lights
//! shine at any hour, the hero's of radius 200, and the colour is a grey ramp,
//! `(blue ramp) · 0.75 + 64`, whatever the night's level. The lamps' glows (`statics`) show from
//! the dusk to the dawn, (255, 236, 149) at an opacity of half the night's level, and in a
//! dungeon (255, 218, 43) at 64 out of 255 (`0x62FF60`). **Not yet** (#61): the creatures' and
//! the spells' lights, and the torches.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use bevy::asset::embedded_asset;
use bevy::core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin};
use bevy::math::{Vec2, Vec3, Vec4};
use bevy::prelude::{
    AlphaMode, App, AssetServer, Assets, Camera, Color, Commands, Component, GlobalTransform,
    IntoScheduleConfigs, Plugin, Projection, Query, Res, ResMut, Resource, StandardMaterial,
    Startup, Time, Transform, Update, With, Without, default,
};
use bevy::render::RenderApp;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_resource::ShaderType;
use bevy::shader::ShaderRef;
use opensacred_data::tables::Item;
use opensacred_data::world::{Sector, StaticObject, World};

use crate::blended::{self, Layer};

/// The shader of [`Night`], embedded in the binary.
pub const SHADER: &str = "embedded://opensacred_game/night.wesl";

/// Seconds of the day that a second of the frames' time adds (the clock's `+0x0C`).
const SPEED: f64 = 24.0;
const DAY_SECONDS: f64 = 86_400.0;
/// The renderer's times, in minutes of the day: the dawn's start, the day's, the dusk's, the
/// night's.
const DAWN: f64 = 200.0;
const DAY: f64 = 320.0;
const DUSK: f64 = 1220.0;
const NIGHT: f64 = 1340.0;
/// The night's level per minute of the dusk and of the dawn (255 over 120 minutes).
const PER_MINUTE: f64 = 2.125;
/// `NIGHT_DARKNESS` of the original's settings (`Settings.cfg`, 50), as a fraction.
const DARKNESS: f32 = 0.5;
/// The light's level at a point that no light source reaches.
const DARK: f32 = 32.0;
/// The type flag of the light sources (`items.pak +0x00`).
const LIGHT: u32 = 0x20000;
/// The light sources the pass takes, the nearest the screen's centre first.
pub const LIGHTS: usize = 32;
/// The original's screen height, in whose pixels the radii are (`iso::SCREEN_HEIGHT`).
const SCREEN: f32 = 768.0;

/// The phase of the day, as the renderer tells the sound manager (`+0xA34`: 0 to 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Day,
    Dusk,
    Night,
    Dawn,
}

/// The game's clock: the seconds of the day.
#[derive(Resource, Clone)]
pub struct Clock {
    seconds: f64,
}

impl Clock {
    /// A clock at `hours:minutes`.
    pub fn at(hours: u32, minutes: u32) -> Self {
        Self {
            seconds: f64::from(hours * 3600 + minutes * 60) % DAY_SECONDS,
        }
    }

    /// The minutes of the day, whole, as the original counts them (hours · 60 + minutes).
    pub fn minutes(&self) -> f64 {
        (self.seconds / 60.0).floor()
    }

    pub fn phase(&self) -> Phase {
        phase(self.minutes())
    }

    /// The clock after a frame's time of `seconds`.
    fn advance(&mut self, seconds: f64) {
        self.seconds = (self.seconds + seconds * SPEED).rem_euclid(DAY_SECONDS);
    }
}

impl Default for Clock {
    /// 08:00, the original's start.
    fn default() -> Self {
        Self::at(8, 0)
    }
}

/// The phase at a minute of the day (`0x41A4F0`).
pub fn phase(minutes: f64) -> Phase {
    if DAY < minutes && minutes < DUSK {
        Phase::Day
    } else if (DUSK..=NIGHT).contains(&minutes) {
        Phase::Dusk
    } else if !(DAWN..=NIGHT).contains(&minutes) {
        Phase::Night
    } else {
        Phase::Dawn
    }
}

/// The night's level at a minute of the day, 0 to 255 (`0x41C2D0`).
pub fn level(minutes: f64) -> f64 {
    match phase(minutes) {
        Phase::Day => 0.0,
        Phase::Dusk => (minutes - DUSK) * PER_MINUTE,
        Phase::Night => 255.0,
        Phase::Dawn => (DAY - minutes) * PER_MINUTE,
    }
}

/// The night ramp's red and green, and its blue, at a light's level, out of 255 (the renderer's
/// tables, `0x417F00`; `night.wesl` computes them).
#[cfg(test)]
fn ramp(light: f32) -> (f32, f32) {
    let base = 64.0 + 64.0 * DARKNESS;
    (base + light / 4.0, base + light / 3.0)
}

/// The clock, the night's pass and its shader, the lights, the glows and the shadows' fading.
pub struct DayNightPlugin;

impl Plugin for DayNightPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "night.wesl");
        app.add_plugins(FullscreenMaterialPlugin::<Night>::default())
            .add_systems(Startup, make_glow)
            .add_systems(Update, (tick, light, glow, shadows).chain());
    }
}

/// `HH:MM` of a time of day.
pub fn parse_time(text: &str) -> Result<Clock, String> {
    let (h, m) = text.split_once(':').ok_or("expected HH:MM")?;
    let number = |s: &str| s.trim().parse::<u32>().map_err(|e| format!("{s}: {e}"));
    let (hours, minutes) = (number(h)?, number(m)?);
    if hours > 23 || minutes > 59 {
        return Err(format!("{text}: not a time of day"));
    }
    Ok(Clock::at(hours, minutes))
}

/// Advances the clock by the frame's time.
pub fn tick(time: Res<Time>, mut clock: ResMut<Clock>) {
    clock.advance(f64::from(time.delta_secs()));
}

/// The frame's colour by the night: the ramp of the light's level, and the part of the day's
/// white that the night's level leaves, added to it.
#[derive(Component, ExtractComponent, Clone, Copy, ShaderType, Default)]
#[extract_app(RenderApp)]
pub struct Night {
    /// The darkness (`NIGHT_DARKNESS` as a fraction), the dark's level, the day's part out of 1,
    /// and the light sources' count.
    params: Vec4,
    /// The original's screen pixels per unit of the texture coordinates across and down, and 1
    /// in a dungeon.
    scale: Vec4,
    /// The light sources: their texture coordinates and radius in the original's pixels.
    lights: [Vec4; LIGHTS],
}

/// The radius of each light source's type, by the type (`items.pak +0x32` of the types flagged
/// [`LIGHT`]).
#[derive(Resource, Default)]
pub struct LightTypes(HashMap<u32, f32>);

impl LightTypes {
    /// Reads the light sources' types of `items.ron`.
    pub fn read(assets: &Path) -> Result<Self> {
        let path = assets.join("tables").join(opensacred_data::tables::ITEMS);
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let items: Vec<Item> =
            ron::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        Ok(Self(
            items
                .iter()
                .filter(|item| flags(item) & LIGHT != 0)
                .map(|item| (item.slot, f32::from(radius(item))))
                .collect(),
        ))
    }
}

/// A type's flags, `items.pak +0x00` as a u32.
fn flags(item: &Item) -> u32 {
    u32::from_le_bytes([item.kind, item.class, item.subclass, item.byte_03])
}

/// A light source's radius, `items.pak +0x32`.
fn radius(item: &Item) -> u16 {
    let [_, _, low, high] = item.words_14[7].to_le_bytes();
    u16::from_le_bytes([low, high])
}

/// The dungeons' sectors (`world.ron`).
#[derive(Resource, Default)]
pub struct Dungeons(HashSet<(u32, u32)>);

impl Dungeons {
    pub fn of(world: &World) -> Self {
        Self(
            world
                .sectors
                .iter()
                .filter(|s| s.dungeon)
                .map(|s| (s.x, s.y))
                .collect(),
        )
    }

    /// Whether world `(x, z)` lies in a dungeon's sector.
    fn holds(&self, x: f32, z: f32) -> bool {
        self.0.contains(&crate::music::sector_of(x, z))
    }
}

/// The glows' texture, relative to the asset directory (`PARTICLE_GLOW01`).
#[derive(Resource)]
pub struct GlowTexture(pub String);

/// Makes the glows' material (`statics::Glow`), hidden until the dusk: the texture times the
/// colour, unlit, blended over what is drawn by its opacity, as the original blends it, over the
/// other blended things.
pub fn make_glow(
    mut commands: Commands,
    texture: Option<Res<GlowTexture>>,
    assets: Res<AssetServer>,
    mut materials: ResMut<Assets<blended::Sorted>>,
) {
    let Some(texture) = texture.filter(|t| !t.0.is_empty()) else {
        return;
    };
    let material = StandardMaterial {
        base_color: Color::NONE,
        base_color_texture: Some(assets.load(texture.0.clone())),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    };
    commands.insert_resource(crate::statics::Glow(
        materials.add(blended::sorted(material, Layer::Glows)),
    ));
}

/// The glows' colour by the night (`0x62FF60`): from the dusk to the dawn (255, 236, 149) at an
/// opacity of half the night's level, in a dungeon (255, 218, 43) at 64, at any hour; none by day.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "half the night's level, from 0 to 127"
)]
fn glow_colour(minutes: f64, dungeon: bool) -> Color {
    if dungeon {
        Color::srgba_u8(255, 218, 43, 64)
    } else if phase(minutes) == Phase::Day {
        Color::NONE
    } else {
        Color::srgba_u8(255, 236, 149, (level(minutes) / 2.0) as u8)
    }
}

/// Keeps the glows' colour by the clock and the hero's sector.
pub fn glow(
    clock: Res<Clock>,
    dungeons: Res<Dungeons>,
    hero: Query<&Transform, With<crate::hero::Body>>,
    glow: Option<Res<crate::statics::Glow>>,
    mut materials: ResMut<Assets<blended::Sorted>>,
) {
    let Some(glow) = glow else {
        return;
    };
    let dungeon = hero
        .iter()
        .next()
        .is_some_and(|t| dungeons.holds(t.translation.x, t.translation.z));
    let colour = glow_colour(clock.minutes(), dungeon);
    if materials
        .get(&glow.0)
        .is_some_and(|m| m.base.base_color != colour)
        && let Some(mut material) = materials.get_mut(&glow.0)
    {
        material.base.base_color = colour;
    }
}

/// The statics' shadows' alpha at a minute of the day, out of 1: their vertices' `255 − N`
/// (`0x41B9E0`, `0x631EB0`), so they fade out over the dusk and come back over the dawn, in a
/// dungeon too.
fn shadow_alpha(minutes: f64) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a level of 0 to 255, out of 255"
    )]
    let alpha = ((255.0 - level(minutes)) / 255.0) as f32;
    alpha
}

/// Keeps the statics' shadows' alpha by the clock.
pub fn shadows(
    clock: Res<Clock>,
    shadows: Option<Res<crate::statics::StaticShadows>>,
    mut materials: ResMut<Assets<crate::statics::SpriteMaterial>>,
) {
    let Some(shadows) = shadows else {
        return;
    };
    let alpha = shadow_alpha(clock.minutes());
    for handle in [&shadows.list1, &shadows.with_sprites, &shadows.on_floors] {
        if materials.get(handle).is_some_and(|m| m.tint.w != alpha)
            && let Some(mut material) = materials.get_mut(handle)
        {
            material.tint.w = alpha;
        }
    }
}

/// A light source in the world, of its radius in the pixels of the original's screen.
#[derive(Component)]
pub struct Light(pub f32);

/// The light sources among `sector`'s static objects: where they stand on the ground (at height
/// 0) and their radii.
pub fn sources(sector: &Sector, types: &LightTypes) -> Vec<(Vec3, f32)> {
    fn walk(objects: &[StaticObject], types: &LightTypes, out: &mut Vec<(Vec3, f32)>) {
        for object in objects {
            if object.type_flags & LIGHT != 0
                && let Some(&radius) = types.0.get(&object.kind)
            {
                // `48 (cx − cy)` and `24 (cx + cy)` back to the cell.
                let (across, down) = (object.x as f32 / 48.0, object.y as f32 / 24.0);
                let (cx, cy) = ((across + down) / 2.0, (down - across) / 2.0);
                out.push((crate::iso::ground(cx, cy), radius));
            }
            walk(&object.children, types, out);
        }
    }
    let mut out = Vec::new();
    walk(&sector.statics, types, &mut out);
    out
}

impl FullscreenMaterial for Night {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

/// The hero's light.
#[derive(Resource, Default)]
pub struct HeroLight {
    /// Whether the hero's light is doubled by night: the Wood Elf's, class 5 (and the
    /// Vampiress's in her night's form, class 7, who is not a class of the game yet).
    pub doubled: bool,
}

/// The hero's light's radius at the night's level (`0x54A7B0`): 200 in a dungeon at any hour;
/// elsewhere 700, and from the dusk to the dawn 1000 − level · 700/255, `doubled` or not. Not
/// yet: a torch in hand (`TORCH_SMALL_ON`, `TORCH_LARGE_ON`), which makes it 300 in a dungeon and
/// half as much again elsewhere, and the attribute `0x361`, which replaces it when it is set.
fn hero_radius(phase: Phase, level: f32, dungeon: bool, doubled: bool) -> f32 {
    if dungeon {
        200.0
    } else if phase == Phase::Day {
        700.0
    } else {
        let radius = 1000.0 - level * 700.0 / 255.0;
        if doubled { 2.0 * radius } else { radius }
    }
}

/// Keeps the camera's night by the clock and the light sources on screen.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "a level from 0 to 255 and a count of 32 in floats of the shader"
)]
pub fn light(
    clock: Res<Clock>,
    dungeons: Res<Dungeons>,
    hero_light: Res<HeroLight>,
    mut cameras: Query<(&Camera, &GlobalTransform, &Projection, &mut Night)>,
    hero: Query<&Transform, With<crate::hero::Body>>,
    lights: Query<(&Transform, &Light), Without<crate::hero::Body>>,
) {
    let minutes = clock.minutes();
    let level = level(minutes) as f32;
    let dungeon = hero
        .iter()
        .next()
        .is_some_and(|t| dungeons.holds(t.translation.x, t.translation.z));
    for (camera, view, projection, mut night) in &mut cameras {
        let Some(size) = camera.logical_viewport_size() else {
            continue;
        };
        // The view spans the original's 768 pixels down at zoom 1, and its share of them across.
        let scale = match projection {
            Projection::Orthographic(ortho) => ortho.scale,
            _ => 1.0,
        };
        let down = SCREEN * scale;
        let across = down * size.x / size.y;
        let uv = |world: Vec3| {
            camera
                .world_to_viewport(view, world)
                .ok()
                .map(|p| Vec2::new(p.x / size.x, p.y / size.y))
        };
        let mut found: Vec<(Vec2, f32)> = hero
            .iter()
            .filter_map(|t| {
                uv(t.translation).map(|p| {
                    (
                        p,
                        hero_radius(phase(minutes), level, dungeon, hero_light.doubled),
                    )
                })
            })
            .chain(
                lights
                    .iter()
                    .filter_map(|(t, light)| uv(t.translation).map(|p| (p, light.0))),
            )
            // The sources that can reach the view.
            .filter(|(p, r)| {
                let reach = Vec2::new(r / across, r / down);
                p.x > -reach.x && p.x < 1.0 + reach.x && p.y > -reach.y && p.y < 1.0 + reach.y
            })
            .collect();
        found.sort_by(|a, b| {
            let d = |p: Vec2| (p - Vec2::splat(0.5)).length_squared();
            d(a.0).total_cmp(&d(b.0))
        });
        found.truncate(LIGHTS);
        let mut lights = [Vec4::ZERO; LIGHTS];
        for (slot, (p, r)) in lights.iter_mut().zip(&found) {
            *slot = Vec4::new(p.x, p.y, *r, 0.0);
        }
        let next = Night {
            params: Vec4::new(DARKNESS, DARK, (255.0 - level) / 255.0, found.len() as f32),
            scale: Vec4::new(across, down, f32::from(u8::from(dungeon)), 0.0),
            lights,
        };
        if night.params != next.params || night.scale != next.scale || night.lights != next.lights {
            *night = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_phases_and_the_nights_level_follow_the_renderers_times() {
        let at = |h: f64, m: f64| h * 60.0 + m;
        assert_eq!(phase(at(8.0, 0.0)), Phase::Day);
        assert_eq!(phase(at(20.0, 20.0)), Phase::Dusk);
        assert_eq!(phase(at(23.0, 0.0)), Phase::Night);
        assert_eq!(phase(at(1.0, 0.0)), Phase::Night);
        assert_eq!(phase(at(4.0, 0.0)), Phase::Dawn);
        assert_eq!(level(at(12.0, 0.0)), 0.0);
        assert_eq!(level(at(21.0, 20.0)), 127.5);
        assert_eq!(level(at(22.0, 20.0)), 255.0);
        assert_eq!(level(at(0.0, 0.0)), 255.0);
        assert_eq!(level(at(4.0, 20.0)), 127.5);
        assert_eq!(level(at(5.0, 20.0)), 0.0);
    }

    #[test]
    fn the_clock_starts_at_eight_and_a_day_lasts_an_hour() {
        let mut clock = Clock::default();
        assert_eq!(clock.minutes(), 480.0);
        clock.advance(1800.0);
        assert_eq!(clock.minutes(), 480.0 + 720.0);
        clock.advance(1800.0);
        assert_eq!(clock.minutes(), 480.0);
        assert_eq!(Clock::at(22, 30).phase(), Phase::Night);
    }

    #[test]
    fn a_static_light_stands_at_its_cell() {
        // `SimpleLightSmall` of radius 300 at the iso position of cell (10, 4).
        let mut types = LightTypes::default();
        types.0.insert(1000, 300.0);
        let object = |kind, type_flags| StaticObject {
            cell: 0,
            kind,
            flags: 0,
            x: 48 * (10 - 4),
            y: 24 * (10 + 4),
            record: 0,
            type_flags,
            mask: 0,
            sprite: 0,
            miniature: None,
            shadow: None,
            floor: None,
            children: Vec::new(),
        };
        let sector = Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells: Vec::new(),
            statics: vec![object(1000, 0x20009), object(1001, 0x9)],
            layers: Vec::new(),
            buildings: Vec::new(),
        };
        let found = sources(&sector, &types);
        assert_eq!(found, [(crate::iso::ground(10.0, 4.0), 300.0)]);
        assert_eq!(hero_radius(Phase::Day, 0.0, false, true), 700.0);
        assert_eq!(hero_radius(Phase::Night, 255.0, false, false), 300.0);
        assert_eq!(hero_radius(Phase::Night, 255.0, false, true), 600.0);
        assert_eq!(hero_radius(Phase::Dusk, 0.0, false, false), 1000.0);
        assert_eq!(hero_radius(Phase::Night, 255.0, true, true), 200.0);
    }

    #[test]
    fn the_glows_show_by_night_and_in_the_dungeons() {
        let minutes = |h: f64| h * 60.0;
        assert_eq!(glow_colour(minutes(12.0), false), Color::NONE);
        assert_eq!(
            glow_colour(minutes(23.5), false),
            Color::srgba_u8(255, 236, 149, 127)
        );
        // Half way through the dusk: the night's level 127, the glow's opacity 63.
        assert_eq!(
            glow_colour(minutes(21.0) + 20.0, false),
            Color::srgba_u8(255, 236, 149, 63)
        );
        assert_eq!(
            glow_colour(minutes(12.0), true),
            Color::srgba_u8(255, 218, 43, 64)
        );
    }

    #[test]
    fn the_shadows_fade_out_by_night() {
        let minutes = |h: f64| h * 60.0;
        assert_eq!(shadow_alpha(minutes(12.0)), 1.0);
        assert_eq!(shadow_alpha(minutes(23.5)), 0.0);
        // Half way through the dusk, the night's level 127.5.
        assert!((shadow_alpha(minutes(21.0) + 20.0) - 0.5).abs() < 0.01);
    }

    #[test]
    fn the_night_ramp_is_a_bluish_grey() {
        let (rg, b) = ramp(DARK);
        assert_eq!((rg, b.round()), (104.0, 107.0));
        let (rg, b) = ramp(255.0);
        assert_eq!((rg.round(), b), (160.0, 181.0));
    }
}
