//! The library of the game's shaders: what the materials' shaders import (`post.wesl`, the
//! last steps of Bevy's own materials and the frame's encoding, and `order.wesl`, the models'
//! place among the sprites), loaded once so that the imports find it, and the fragment stage of
//! the extensions of Bevy's own material (`standard.wesl`).

use bevy::asset::embedded_asset;
use bevy::prelude::{App, Plugin};

/// The fragment stage of the extensions of Bevy's own material, embedded in the binary.
pub const STANDARD: &str = "embedded://opensacred_game/standard.wesl";

pub struct ShaderLibrary;

impl Plugin for ShaderLibrary {
    fn build(&self, app: &mut App) {
        bevy::shader::load_shader_library!(app, "post.wesl");
        bevy::shader::load_shader_library!(app, "order.wesl");
        embedded_asset!(app, "standard.wesl");
    }
}
