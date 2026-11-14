//! The order of the blended things (docs/decisions/drawing.md, "Sprites' opaque and translucent
//! texels apart"). Bevy draws them after the opaque ones, back to front by the centres of their
//! meshes plus their materials' `depth_bias`. The game's blended meshes span cells to sectors, so
//! their centres say little of what lies over what; they go in layers instead, in the original's
//! order after the ground: the objects of the liquid cells, the liquids' surfaces, then the draw
//! lists (docs/re/drawing.md, "The rows"). Within a layer, the centres order them.
//!
//! Bevy's own material also moves the depth that it tests by its `depth_bias`; [`Sorted`] keeps
//! the bias for the order alone, and encodes its colour for the frame (`srgb`).

use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{
    ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
    MaterialPlugin,
};
use bevy::prelude::{App, Asset, Plugin, StandardMaterial, TypePath};
use bevy::render::render_resource::{
    AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;

/// The `depth_bias` between two layers: beyond the spread of the meshes' depths anywhere in the
/// world, not only in the camera's range (`iso::camera`, 20,000 units). A mesh may sort by a
/// centre far from where it draws: the hero's shadow, after a teleport, by one from before it,
/// some 170,000 units off, which put it before the health rings' layer (#2713). The world's
/// 6,400 cells a side span some 485,000 units across, which a step of a million keeps within;
/// its float keeps half a unit of a centre's distance in the last layer, enough to order meshes
/// a cell apart.
const STEP: f32 = 1_000_000.0;

/// How far apart two meshes of the world can lie along the line of sight at most: the world's
/// diagonal, its sectors' cells a side by a cell's side, times √2.
#[cfg(test)]
const WORLD_ACROSS: f32 = 100.0 * 64.0 * crate::iso::CELL * std::f32::consts::SQRT_2;

/// A layer of the blended things, in the order they are drawn.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Layer {
    /// The miniatures of the liquid cells, which the original draws before the surfaces
    /// (`0x62E410`), and the translucent texels and the shadows of the models it draws there
    /// (`models::Sunk`).
    UnderLiquids,
    /// The clear waters' second layer, which the liquids' pass draws before the surfaces
    /// (`0x62DD00`).
    LiquidBeds,
    /// The liquids' surfaces (`0x62DD00`).
    Liquids,
    /// The sprites of list 0.
    Underlays,
    /// The statics' shadows of list 1 (`0x631EB0`).
    StaticShadows,
    /// The sprites of list 2.
    LateUnderlays,
    /// The health rings on the ground, which the original lays with their creatures in list 3,
    /// before their models and their shadows (`cCreature_render` `0x599910`): under every model's
    /// shadow, which darkens them (`health_rings`, #2701).
    Rings,
    /// The models' shadows, which the original draws with their models, in list 3
    /// (`cGranny::renderShadow`), and the shadows that it draws right before their statics in
    /// their lists (entries of kind `0x11`).
    Shadows,
    /// The sprites of lists 3 and 4, and the models' translucent texels, which the original draws
    /// with their models, in list 3 (`models::Pass::Fringe`).
    Objects,
    /// The lamps' glows, which the original draws over their lamps (`0x62FF60`).
    Glows,
}

impl Layer {
    /// The `depth_bias` of a material of this layer.
    pub fn bias(self) -> f32 {
        STEP * match self {
            Layer::UnderLiquids => -2.0,
            Layer::LiquidBeds => -1.0,
            Layer::Liquids => 0.0,
            Layer::Underlays => 1.0,
            Layer::StaticShadows => 2.0,
            Layer::LateUnderlays => 3.0,
            Layer::Rings => 4.0,
            Layer::Shadows => 5.0,
            Layer::Objects => 6.0,
            Layer::Glows => 7.0,
        }
    }
}

/// A blended [`StandardMaterial`] in its layer.
pub type Sorted = ExtendedMaterial<StandardMaterial, Unbiased>;

/// The extension of [`Sorted`]: the depth as Bevy's own material leaves it, without the bias.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
pub struct Unbiased {}

impl MaterialExtension for Unbiased {
    /// Bevy's own, its colour encoded for the frame (`srgb`).
    fn fragment_shader() -> ShaderRef {
        crate::shaders::STANDARD.into()
    }

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        unbias(descriptor);
        Ok(())
    }
}

/// Takes back the bias of the depth that [`StandardMaterial`] gives a pipeline by its
/// `depth_bias`, for an extension of it whose `depth_bias` is its layer's.
pub fn unbias(descriptor: &mut RenderPipelineDescriptor) {
    if let Some(depth) = descriptor.depth_stencil.as_mut() {
        depth.bias.constant = 0;
    }
}

/// `base` in `layer`.
pub fn sorted(base: StandardMaterial, layer: Layer) -> Sorted {
    Sorted {
        base: StandardMaterial {
            depth_bias: layer.bias(),
            ..base
        },
        extension: Unbiased {},
    }
}

/// The material of the blended things.
pub struct BlendedPlugin;

impl Plugin for BlendedPlugin {
    fn build(&self, app: &mut App) {
        crate::shaders::embed_standard(app);
        app.add_plugins(MaterialPlugin::<Sorted>::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layers_go_in_the_originals_order() {
        let layers = [
            Layer::UnderLiquids,
            Layer::LiquidBeds,
            Layer::Liquids,
            Layer::Underlays,
            Layer::StaticShadows,
            Layer::LateUnderlays,
            Layer::Rings,
            Layer::Shadows,
            Layer::Objects,
            Layer::Glows,
        ];
        for pair in layers.windows(2) {
            // Every mesh of a layer after every mesh of the one before, wherever in the world
            // either sorts (#2713).
            assert!(pair[1].bias() - pair[0].bias() > WORLD_ACROSS, "{pair:?}");
        }
    }
}
