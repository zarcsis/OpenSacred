//! The library of the game's shaders: what the materials' shaders import (`post.wesl`, the
//! last steps of Bevy's own materials), loaded once so that the imports find it.

use bevy::prelude::{App, Plugin};

pub struct ShaderLibrary;

impl Plugin for ShaderLibrary {
    fn build(&self, app: &mut App) {
        bevy::shader::load_shader_library!(app, "post.wesl");
    }
}
