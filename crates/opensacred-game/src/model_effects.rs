//! The effects that the original's second model renderer draws on the models of the types whose
//! flags have the bit `0x100000` (`cGranny::renderComplex`, `0x406510`; #1309; docs/re/drawing.md,
//! "The models' effects"): an effect texture of the type's (`+0x66`) moved by a texture matrix on
//! the real-time clock, by the type's kind of effect (`+0x6E`).
//!
//! The game follows the kind 9, the Seraphim's wings 3111 alone: the model's first primitive, the
//! two tubes along the wings, is drawn with the effect texture instead of the type's, unlit, its
//! colour added to the frame without writing the depth, its texture running along the tubes twice
//! in each cycle of 2,048 ms and then standing for 1,024 ms (`0x4068B9`). Every pair of wings
//! glows in step, as the clock is the frame's. `npcs::retexture` gives the primitive its material.

use bevy::asset::AssetId;
use bevy::gltf::GltfAssetLabel;
use bevy::math::Affine2;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::models::{Effect, EffectMaterials};

/// The kind of the effect whose bands run along the model's first primitive (`0x4068B9`): the
/// wings 3111's.
pub const RUNNING_BANDS: u8 = 9;
/// The kind of the effect and the glow under every texel of the model (`0x406510`'s kind 1,
/// 48 types; #1834).
pub const MULTITEX: u8 = 1;
/// The kinds of an effect alone under every texel (#1848): the shield 4017's, running along `v`,
/// the wings 4018's and the bow 1747's along `u`.
pub const ALONG_V: u8 = 2;
pub const ALONG_U: u8 = 3;
pub const ALONG_U_FAST: u8 = 4;
/// The kind of the effect texture added twice over the whole model, once white and once in a
/// grey that pulses (`0x406510`'s kind 5, `0x40D310`): the big demons and Anducar's guard (#1850).
pub const PULSE: u8 = 5;
/// The pulse's half period in milliseconds: its grey is `(sin(t·π/1024) + 1)·127.5`.
const PULSE_HALF: f32 = 1_024.0;

/// The colour of the kind 5's two added passes laid in one at the frame's real time `t` in
/// milliseconds: the white's 1 and the grey's share, 1 to 2.
pub fn pulse(t: u16) -> f32 {
    1.0 + ((f32::from(t) * std::f32::consts::PI / PULSE_HALF).sin() + 1.0) / 2.0
}

/// How many milliseconds a texture's width takes along both axes under the kind 1: the effect's
/// (`0x89071C`, 1/16384) and the glow's (`0x890718`, 1/32767); along one axis under the kinds 2
/// and 3 (`0x890714`, 1/2048) and 4 (`0x890710`, 1/1024).
const EFFECT_DRIFT: f32 = 16_384.0;
const GLOW_DRIFT: f32 = 32_767.0;
const ALONG_DRIFT: f32 = 2_048.0;
const FAST_DRIFT: f32 = 1_024.0;

/// Whether the kind `kind` lays its effect under every texel of the model (the kinds 1 to 4).
pub fn under(kind: u8) -> bool {
    (MULTITEX..=ALONG_U_FAST).contains(&kind)
}

/// The length of the kind 9's cycle in milliseconds, and how long each of its two runs lasts.
const CYCLE: u16 = 2_048;
const RUN: f32 = 512.0;
/// How many milliseconds a texture's width takes along `u` (`0x8906F8`, 1/128), and the power of
/// a run's share along `v` (the double 1.3 at `0x8906E8`).
const ACROSS: f32 = 128.0;
const SPEED_UP: f32 = 1.3;

/// The materials of the effects: each model material's copy with an effect texture, by the
/// material and the texture's file, and the textures loaded.
#[derive(Resource, Default)]
pub struct Materials {
    running_bands: HashMap<(AssetId<StandardMaterial>, String), Handle<StandardMaterial>>,
    textures: HashMap<String, Handle<Image>>,
}

impl Materials {
    /// The effect texture `effect` (a file, loaded repeated once) of the kind 5's pulse
    /// (`models::Pulse`).
    pub fn pulse(&mut self, effect: &str, assets: &AssetServer) -> crate::models::Pulse {
        crate::models::Pulse(
            self.textures
                .entry(effect.to_owned())
                .or_insert_with(|| crate::npcs::load_texture(assets, effect))
                .clone(),
        )
    }

    /// The effect of the kind `kind` (1 to 4) of the texture `effect`, and the glow `glow` of the
    /// kind 1 (files, loaded repeated once), which a model's meshes lie over (`models::Effect`).
    pub fn under(
        &mut self,
        kind: u8,
        (effect, glow): (&str, Option<&str>),
        assets: &AssetServer,
    ) -> Effect {
        let mut load = |file: &str| {
            self.textures
                .entry(file.to_owned())
                .or_insert_with(|| crate::npcs::load_texture(assets, file))
                .clone()
        };
        Effect {
            kind,
            effect: load(effect),
            glow: glow.filter(|_| kind == MULTITEX).map(&mut load),
        }
    }

    /// The material that the kind 9's pass draws a primitive of the material `original` with: its
    /// copy with the effect texture `texture` (a file, loaded repeated), white, unlit and added to
    /// the frame (the blend `ONE`, `ONE`, the depth write off), made once and shared; none where
    /// `original` is not loaded.
    pub fn running_bands(
        &mut self,
        original: &Handle<StandardMaterial>,
        texture: &str,
        (assets, materials): (&AssetServer, &mut Assets<StandardMaterial>),
    ) -> Option<Handle<StandardMaterial>> {
        let key = (original.id(), texture.to_owned());
        if let Some(handle) = self.running_bands.get(&key) {
            return Some(handle.clone());
        }
        let mut copy = materials.get(original)?.clone();
        let image = self
            .textures
            .entry(texture.to_owned())
            .or_insert_with(|| crate::npcs::load_texture(assets, texture))
            .clone();
        copy.base_color = Color::WHITE;
        copy.base_color_texture = Some(image);
        copy.unlit = true;
        copy.alpha_mode = AlphaMode::Add;
        let handle = materials.add(copy);
        self.running_bands.insert(key, handle.clone());
        Some(handle)
    }
}

/// Whether `mesh` is its model's first primitive, the first rendering state that the kind 9
/// singles out (`0x406B13`, `0x406D80`): the first pass of the model's file, which the converter
/// writes as its first glTF primitive.
pub fn first_primitive(assets: &AssetServer, mesh: &Mesh3d) -> bool {
    let first = GltfAssetLabel::Primitive {
        mesh: 0,
        primitive: 0,
    }
    .to_string();
    assets
        .get_path(mesh.0.id())
        .is_some_and(|path| path.label() == Some(first.as_str()))
}

/// The offset of the kind 9's texture matrix at the frame's real time `t` in milliseconds, held
/// to 16 bits (`0x4068B9`): in each cycle of 2,048 ms, two runs of 512 ms along `v` from 0 to 1,
/// speeding up by the power 1.3, while `u` runs a width each 128 ms; then 1,024 ms at rest.
pub fn running_bands(t: u16) -> Vec2 {
    let r = f32::from(t % CYCLE);
    if r < RUN {
        Vec2::new(r / ACROSS, (r / RUN).powf(SPEED_UP))
    } else if r < 2.0 * RUN {
        Vec2::new(r / ACROSS, ((r - RUN) / RUN).powf(SPEED_UP))
    } else {
        Vec2::ZERO
    }
}

/// The offsets of the kind `kind`'s matrices at the frame's real time `t` in milliseconds, held
/// to 16 bits (`0x406510`), the effect's in `x` and `y`, the glow's in `z` and `w`: the kind 1's
/// effect `_31 = _32 = t / 16384` and glow `t / 32767`; the kind 2's `_32 = t / 2048`, the kind
/// 3's `_31 = t / 2048`, the kind 4's `_31 = t / 1024`.
pub fn offsets(kind: u8, t: u16) -> Vec4 {
    let t = f32::from(t);
    match kind {
        MULTITEX => Vec4::new(
            t / EFFECT_DRIFT,
            t / EFFECT_DRIFT,
            t / GLOW_DRIFT,
            t / GLOW_DRIFT,
        ),
        ALONG_V => Vec4::new(0.0, t / ALONG_DRIFT, 0.0, 0.0),
        ALONG_U => Vec4::new(t / ALONG_DRIFT, 0.0, 0.0, 0.0),
        ALONG_U_FAST => Vec4::new(t / FAST_DRIFT, 0.0, 0.0, 0.0),
        _ => Vec4::ZERO,
    }
}

/// Moves the kind 9's textures by the frame's real time, the original's performance counter
/// (`0x406592`), which runs on in the pause and at any speed of the game: in the model materials
/// made of them (`models::Converted`), which draw the meshes, and in them, for those still to be
/// made.
fn run(
    time: Res<Time<bevy::time::Real>>,
    (effects, converted, under): (
        Res<Materials>,
        Res<crate::models::Converted>,
        Res<EffectMaterials>,
    ),
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut models: ResMut<Assets<crate::models::ModelMaterial>>,
) {
    let t = u16::try_from(time.elapsed().as_millis() & 0xFFFF).unwrap_or(0);
    let pulsed = pulse(t);
    for (handle, kind) in &under.0 {
        if let Some(mut material) = models.get_mut(handle) {
            if *kind == PULSE {
                material.base.base_color = Color::linear_rgb(pulsed, pulsed, pulsed);
            } else {
                material.extension.offsets = offsets(*kind, t);
            }
        }
    }
    if effects.running_bands.is_empty() {
        return;
    }
    let offset = Affine2::from_translation(running_bands(t));
    for handle in effects.running_bands.values() {
        if let Some(mut material) = materials.get_mut(handle) {
            material.uv_transform = offset;
        }
        if let Some(model) = converted.mesh_of(handle.id())
            && let Some(mut material) = models.get_mut(model)
        {
            material.base.uv_transform = offset;
        }
    }
}

/// The models' effects.
pub struct ModelEffectsPlugin;

impl Plugin for ModelEffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Materials>().add_systems(Update, run);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pulse_runs_from_white_to_twice_white_in_2048_ms() {
        assert!((pulse(0) - 1.5).abs() < 1e-6);
        assert!((pulse(512) - 2.0).abs() < 1e-6);
        assert!((pulse(1_536) - 1.0).abs() < 1e-6);
        assert!((pulse(2_048) - 1.5).abs() < 1e-4);
    }

    #[test]
    fn the_wings_bands_run_twice_in_a_cycle_and_then_stand() {
        // The first run: from rest, past halfway at 300 ms, and almost done at 511 ms.
        assert_eq!(running_bands(0), Vec2::ZERO);
        let middle = running_bands(256);
        assert_eq!(middle.x, 2.0);
        assert!((middle.y - 0.5_f32.powf(1.3)).abs() < 1e-6);
        assert!(running_bands(511).y > 0.99);
        // The second run begins again from 0.
        assert_eq!(running_bands(512), Vec2::new(4.0, 0.0));
        assert!((running_bands(768).y - middle.y).abs() < 1e-6);
        // At rest for the cycle's second half, and again in the next cycle.
        assert_eq!(running_bands(1_024), Vec2::ZERO);
        assert_eq!(running_bands(2_047), Vec2::ZERO);
        assert_eq!(running_bands(2_048 + 256), middle);
        // The clock's 16 bits are 32 cycles, so its wrap makes no jump.
        assert_eq!(running_bands(u16::MAX), Vec2::ZERO);
    }

    #[test]
    fn the_effects_drift_by_their_kinds() {
        // The kind 1's effect and glow a width in 16 and 33 seconds.
        assert_eq!(offsets(MULTITEX, 0), Vec4::ZERO);
        assert_eq!(
            offsets(MULTITEX, 16_384),
            Vec4::new(1.0, 1.0, 16_384.0 / 32_767.0, 16_384.0 / 32_767.0)
        );
        assert!((offsets(MULTITEX, u16::MAX).z - 2.0).abs() < 1e-3);
        // The shield's along `v`, the wings' and the bow's along `u`, the bow's twice as fast.
        assert_eq!(offsets(ALONG_V, 1_024), Vec4::new(0.0, 0.5, 0.0, 0.0));
        assert_eq!(offsets(ALONG_U, 1_024), Vec4::new(0.5, 0.0, 0.0, 0.0));
        assert_eq!(offsets(ALONG_U_FAST, 1_024), Vec4::new(1.0, 0.0, 0.0, 0.0));
        assert!(under(1) && under(4) && !under(5) && !under(RUNNING_BANDS));
    }
}
