//! The statics' sprites with a depth per texel, to compare (#1437, research; docs/decisions/
//! drawing.md, "Sprites' depth per texel, to compare"). `tools/analysis/world/sprite_depth.py`
//! makes a map beside a sprite's image in the loose files of the assets, a texel's depth in front
//! of its sprite's quad, from a monocular depth model; with `[graphics] sprites = "depth"` (the
//! console's `sprites`) the sprites' shader writes each opaque texel's depth from it
//! (`sprites.wesl`), and the models take their own depth whole rather than squeezed about their
//! cell's place (`models.wesl`), so that a model passes behind a column or between a tree's trunk
//! and its crown by the sprite's shape rather than by its cell. A sprite without a map, an
//! underlay and an upper storey's floor stay flat; with `flat`, the default, everything is drawn
//! as before.

use std::io::Cursor;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use bevy::asset::RenderAssetUsages;
use bevy::ecs::change_detection::DetectChanges;
use bevy::prelude::{
    App, Asset, AssetEvent, AssetId, Assets, Handle, Image, IntoScheduleConfigs, MessageReader,
    Plugin, Res, ResMut, Resource, Update, resource_exists,
};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use opensacred_data::world::sprite_image;

use crate::files;
use crate::models::ModelMaterial;
use crate::settings::Settings;
use crate::statics::SpriteMaterial;

/// A map's extension after the sprite's record (`world/sprites/<record>.depth.png`).
const EXTENSION: &str = "depth.png";
/// The extension of a map of the texels' heights (`world/sprites/<record>.height.png`).
const HEIGHTS: &str = "height.png";
/// A relief's value of a texel on its sprite's quad: the transparent texels' and every texel of a
/// sprite without a map. The others lie eight steps a world unit in front of it or behind
/// (`sprites.wesl`'s `RELIEF_STEPS`); the heights are eight a unit over 0.
pub const ZERO: u16 = 32_768;

/// The maps of sprite `id` of the world directory `dir`, `width` × `height` texels over its
/// rectangle as its image's first level, if they were made: each texel's relief and height,
/// interleaved.
pub fn read(dir: &Path, id: u32, width: usize, height: usize) -> Result<Option<Vec<u16>>> {
    let relief = read_map(dir, id, EXTENSION, width, height)?;
    let heights = read_map(dir, id, HEIGHTS, width, height)?;
    if relief.is_none() && heights.is_none() {
        return Ok(None);
    }
    let relief = relief.unwrap_or_else(|| vec![ZERO; width * height]);
    let heights = heights.unwrap_or_else(|| vec![0; width * height]);
    Ok(Some(
        relief
            .into_iter()
            .zip(heights)
            .flat_map(|(relief, height)| [relief, height])
            .collect(),
    ))
}

/// The map `extension` of sprite `id`, if it was made.
fn read_map(
    dir: &Path,
    id: u32,
    extension: &str,
    width: usize,
    height: usize,
) -> Result<Option<Vec<u16>>> {
    let path = dir.join(sprite_image(id, extension));
    if !files::exists(&path) {
        return Ok(None);
    }
    let what = || format!("decoding {}", path.display());
    let data = files::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut reader = png::Decoder::new(Cursor::new(data))
        .read_info()
        .with_context(what)?;
    let size = reader.output_buffer_size().with_context(what)?;
    let mut buffer = vec![0; size];
    let info = reader.next_frame(&mut buffer).with_context(what)?;
    let shape = (usize::try_from(info.width)?, usize::try_from(info.height)?);
    ensure!(
        info.color_type == png::ColorType::Grayscale
            && info.bit_depth == png::BitDepth::Sixteen
            && shape == (width, height),
        "{}: {:?} of {:?} bits, {} × {}, for a sprite of {width} × {height}",
        path.display(),
        info.color_type,
        info.bit_depth,
        shape.0,
        shape.1
    );
    buffer.truncate(info.buffer_size());
    Ok(Some(
        buffer
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| u16::from_be_bytes(pair))
            .collect(),
    ))
}

/// The texture of a page's maps, `width` × `height` texels of `texels`, a relief and a height
/// each, which the shader loads whole (`sprites.wesl`).
pub fn image(width: usize, height: usize, texels: &[u16]) -> Result<Image> {
    let size = Extent3d {
        width: u32::try_from(width)?,
        height: u32::try_from(height)?,
        depth_or_array_layers: 1,
    };
    Ok(texture(size, texels))
}

fn texture(size: Extent3d, texels: &[u16]) -> Image {
    Image::new(
        size,
        TextureDimension::D2,
        texels
            .iter()
            .flat_map(|texel| texel.to_le_bytes())
            .collect(),
        TextureFormat::Rg16Uint,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// The map of the atlases and the pages whose sprites have none: one texel, which the shader
/// takes for a flat sprite.
#[derive(Resource)]
pub struct NoRelief(pub Handle<Image>);

/// The map of the sprites without one, and the setting followed by the materials.
pub struct ReliefPlugin;

impl Plugin for ReliefPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                follow_sprites,
                follow_models.run_if(resource_exists::<Assets<ModelMaterial>>),
            ),
        );
    }

    fn finish(&self, app: &mut App) {
        let one = Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };
        let image = texture(one, &[ZERO, 0]);
        let handle = app.world_mut().resource_mut::<Assets<Image>>().add(image);
        app.insert_resource(NoRelief(handle));
    }
}

/// Keeps the sprites' materials in the setting's mode: every one as the setting changes, a new one
/// as it is added; the statics' shadows keep their places.
fn follow_sprites(
    settings: Option<Res<Settings>>,
    mut materials: ResMut<Assets<SpriteMaterial>>,
    mut events: MessageReader<AssetEvent<SpriteMaterial>>,
) {
    let Some(settings) = settings else {
        return;
    };
    let depth = settings.graphics.sprites;
    for id in changed(&settings, &materials, &mut events) {
        if materials
            .get(id)
            .is_some_and(|material| !material.keeps && material.depth != depth)
            && let Some(mut material) = materials.get_mut(id)
        {
            material.depth = depth;
        }
    }
}

/// Keeps the models' materials in the setting's mode, as [`follow_sprites`] the sprites'.
fn follow_models(
    settings: Option<Res<Settings>>,
    mut materials: ResMut<Assets<ModelMaterial>>,
    mut events: MessageReader<AssetEvent<ModelMaterial>>,
) {
    let Some(settings) = settings else {
        return;
    };
    let depth = settings.graphics.sprites;
    for id in changed(&settings, &materials, &mut events) {
        if materials
            .get(id)
            .is_some_and(|material| material.extension.depth != depth)
            && let Some(mut material) = materials.get_mut(id)
        {
            material.extension.depth = depth;
        }
    }
}

/// The materials to check: all of them as the setting changes, else those just added.
fn changed<A: Asset>(
    settings: &Res<Settings>,
    materials: &Assets<A>,
    events: &mut MessageReader<AssetEvent<A>>,
) -> Vec<AssetId<A>> {
    let ids = if settings.is_changed() {
        materials.ids().collect()
    } else {
        events.read().filter_map(added).collect()
    };
    events.clear();
    ids
}

/// The asset an event tells of, if it was added.
fn added<A: Asset>(event: &AssetEvent<A>) -> Option<AssetId<A>> {
    match *event {
        AssetEvent::Added { id } => Some(id),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;

    /// Writes a 16-bit grey PNG of `width` × `height` texels `values`, as `sprite_depth.py` does.
    fn write_map(path: &Path, width: u32, height: u32, values: &[u16]) {
        let file = fs::File::create(path).unwrap();
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Sixteen);
        let bytes: Vec<u8> = values.iter().flat_map(|v| v.to_be_bytes()).collect();
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&bytes)
            .unwrap();
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("opensacred-relief-{name}-{}", std::process::id()));
        fs::create_dir_all(dir.join(opensacred_data::world::SPRITES_DIR)).unwrap();
        dir
    }

    #[test]
    fn a_sprites_maps_are_read_interleaved_and_a_missing_one_is_flat() {
        let dir = scratch("read");
        let sprites = dir.join(opensacred_data::world::SPRITES_DIR);
        // Sprite 1 with both maps, 2 with its relief alone, 3 with none.
        write_map(
            &sprites.join("00001.depth.png"),
            2,
            1,
            &[ZERO + 8, ZERO - 16],
        );
        write_map(&sprites.join("00001.height.png"), 2, 1, &[40, 0]);
        write_map(&sprites.join("00002.depth.png"), 2, 1, &[ZERO, ZERO + 1]);
        assert_eq!(
            read(&dir, 1, 2, 1).unwrap(),
            Some(vec![ZERO + 8, 40, ZERO - 16, 0])
        );
        assert_eq!(
            read(&dir, 2, 2, 1).unwrap(),
            Some(vec![ZERO, 0, ZERO + 1, 0])
        );
        assert_eq!(read(&dir, 3, 2, 1).unwrap(), None);
        // A map of another size than its sprite's rectangle does not draw.
        let error = read(&dir, 1, 3, 1).err().unwrap().to_string();
        assert!(error.contains("for a sprite of 3 × 1"), "{error}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_shader_reads_the_maps_as_they_are_written() {
        let source = include_str!("sprites.wesl");
        let zero = crate::statics::tests::shader_constant(source, "RELIEF_ZERO");
        assert_eq!(zero, f32::from(ZERO));
        // Eight steps a unit, as `sprite_depth.py` writes them.
        let steps = crate::statics::tests::shader_constant(source, "RELIEF_STEPS");
        assert_eq!(steps, 8.0);
    }
}
