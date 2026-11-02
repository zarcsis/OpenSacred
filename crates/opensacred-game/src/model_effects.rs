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

/// The kind of the effect whose bands run along the model's first primitive (`0x4068B9`): the
/// wings 3111's.
pub const RUNNING_BANDS: u8 = 9;

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

/// Moves the kind 9's textures by the frame's real time, the original's performance counter
/// (`0x406592`), which runs on in the pause and at any speed of the game: in the model materials
/// made of them (`models::Converted`), which draw the meshes, and in them, for those still to be
/// made.
fn run(
    time: Res<Time<bevy::time::Real>>,
    (effects, converted): (Res<Materials>, Res<crate::models::Converted>),
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut models: ResMut<Assets<crate::models::ModelMaterial>>,
) {
    if effects.running_bands.is_empty() {
        return;
    }
    let t = u16::try_from(time.elapsed().as_millis() & 0xFFFF).unwrap_or(0);
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
}
