//! The pool of the ground's tile sheets that the loaded sectors share (#2484): a 2D array texture
//! of 256 × 256 layers, each sheet written into a layer of its own as the first sector that takes
//! it comes in, and its layer free again once the last such sector is gone. A sector's ground
//! draws its cells' stacks of layers from the pool (`terrain`, `ground.wesl`), so that a sheet
//! goes to the graphics card once for every loaded sector that uses it, where each sector's
//! composed atlas sent it again (20 to 30 MB a sector on the overworld, docs/05 §4).
//!
//! A layer is written alone through the render queue: changing the pool's image would send the
//! whole of it again.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, bail};
use bevy::asset::RenderAssetUsages;
use bevy::extract::{ExtractSchedule, MainWorld};
use bevy::prelude::{
    App, AssetId, Assets, Handle, Image, IntoScheduleConfigs, Plugin, Res, ResMut, Resource,
};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{
    Extent3d, Origin3d, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect,
    TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};
use bevy::render::renderer::RenderQueue;
use bevy::render::texture::GpuImage;
use bevy::render::{Render, RenderApp, RenderSystems};
use opensacred_data::texture::Rgba;

/// A tile sheet's side, in texels (docs/02 §5.4).
pub const SHEET: usize = 256;
/// The pool's layers: the most distinct sheets that 4 × 4 sectors take on the world's 6,050 is
/// 576, and those that one takes 248 (a scan of 2026-11-10), so that the 3 × 3 loaded about the
/// hero and the sectors coming in fit with room to spare; 256 MB in all.
pub const LAYERS: u16 = 1024;

/// The pool: its image, the layer and the users of each sheet it holds, its free layers, and the
/// sheets to write into their layers at the next extraction.
#[derive(Resource)]
pub struct SheetPool {
    image: Handle<Image>,
    held: HashMap<u32, Held>,
    free: Vec<u16>,
    pending: Vec<(u16, Arc<Rgba>)>,
}

/// A sheet in the pool: its layer and how many sectors take it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Held {
    layer: u16,
    users: u32,
}

impl SheetPool {
    /// An empty pool, its image added to `images` with no texels: the layers are written as
    /// sheets come.
    pub fn new(images: &mut Assets<Image>) -> Self {
        let side = u32::try_from(SHEET).unwrap_or(u32::MAX);
        let mut image = Image::new_uninit(
            Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: u32::from(LAYERS),
            },
            TextureDimension::D2,
            // The sheets' values as stored, whole: the shader blends them as integers, as the
            // atlas was composed (`terrain::compose_slot`).
            TextureFormat::Rgba8Uint,
            RenderAssetUsages::RENDER_WORLD,
        );
        image.texture_view_descriptor = Some(TextureViewDescriptor {
            dimension: Some(TextureViewDimension::D2Array),
            ..TextureViewDescriptor::default()
        });
        Self {
            image: images.add(image),
            held: HashMap::new(),
            free: (0..LAYERS).rev().collect(),
            pending: Vec::new(),
        }
    }

    /// The pool's image, a 2D array of [`LAYERS`] layers.
    pub fn image(&self) -> Handle<Image> {
        self.image.clone()
    }

    /// The layer of sheet `sheet` if the pool holds it.
    pub fn layer(&self, sheet: u32) -> Option<u16> {
        self.held.get(&sheet).map(|held| held.layer)
    }

    /// Takes sheet `sheet` for one more sector: its layer, written with `texels` at the next
    /// extraction where the pool did not hold it. Answers the layer and the bytes it is to write.
    pub fn take(&mut self, sheet: u32, texels: &Arc<Rgba>) -> Result<(u16, usize)> {
        if let Some(held) = self.held.get_mut(&sheet) {
            held.users += 1;
            return Ok((held.layer, 0));
        }
        let Some(layer) = self.free.pop() else {
            bail!("the pool of tile sheets is full, {LAYERS} layers");
        };
        let texels = padded(texels)?;
        let bytes = texels.pixels.len();
        self.held.insert(sheet, Held { layer, users: 1 });
        self.pending.push((layer, texels));
        Ok((layer, bytes))
    }

    /// Gives sheet `sheet` back for one sector; its layer is free once no sector takes it.
    pub fn release(&mut self, sheet: u32) {
        let Some(held) = self.held.get_mut(&sheet) else {
            return;
        };
        held.users = held.users.saturating_sub(1);
        if held.users == 0 {
            let layer = held.layer;
            self.held.remove(&sheet);
            // A layer freed before its write went out needs it no longer.
            self.pending.retain(|&(pending, _)| pending != layer);
            self.free.push(layer);
        }
    }

    /// The sheets it holds.
    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// The texels to write into layer `layer` at the next extraction, if any.
    #[cfg(test)]
    pub fn pending_of(&self, layer: u16) -> Option<Arc<Rgba>> {
        self.pending
            .iter()
            .find(|&&(pending, _)| pending == layer)
            .map(|(_, texels)| Arc::clone(texels))
    }
}

/// `sheet` as a pool's layer: [`SHEET`] texels square, those beyond a smaller sheet clear, as
/// the atlas read nothing beyond a sheet.
fn padded(sheet: &Arc<Rgba>) -> Result<Arc<Rgba>> {
    if sheet.width == SHEET && sheet.height == SHEET {
        return Ok(Arc::clone(sheet));
    }
    if sheet.width > SHEET || sheet.height > SHEET {
        bail!(
            "a tile sheet of {} × {} texels, more than {SHEET} square",
            sheet.width,
            sheet.height
        );
    }
    let mut pixels = vec![0; 4 * SHEET * SHEET];
    for (y, row) in sheet.pixels.chunks(4 * sheet.width).enumerate() {
        pixels[4 * SHEET * y..4 * SHEET * y + row.len()].copy_from_slice(row);
    }
    Ok(Arc::new(Rgba {
        width: SHEET,
        height: SHEET,
        pixels,
    }))
}

/// The pool's sheets to write, in the render world, and its image's id; a sheet waits there until
/// the image is on the graphics card.
#[derive(Resource, Default)]
struct Unwritten {
    image: Option<AssetId<Image>>,
    sheets: Vec<(u16, Arc<Rgba>)>,
}

/// Takes the pool's sheets to write into the render world.
fn extract(mut main: ResMut<MainWorld>, mut unwritten: ResMut<Unwritten>) {
    let Some(mut pool) = main.get_resource_mut::<SheetPool>() else {
        return;
    };
    unwritten.image = Some(pool.image.id());
    let pending = std::mem::take(&mut pool.pending);
    unwritten.sheets.extend(pending);
}

/// Writes each sheet waiting into its layer of the pool's texture.
fn write(
    mut unwritten: ResMut<Unwritten>,
    images: Res<RenderAssets<GpuImage>>,
    queue: Res<RenderQueue>,
) {
    let Some(gpu) = unwritten.image.and_then(|image| images.get(image)) else {
        return;
    };
    let side = u32::try_from(SHEET).unwrap_or(u32::MAX);
    for (layer, sheet) in std::mem::take(&mut unwritten.sheets) {
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &gpu.texture,
                mip_level: 0,
                origin: Origin3d {
                    x: 0,
                    y: 0,
                    z: u32::from(layer),
                },
                aspect: TextureAspect::All,
            },
            &sheet.pixels,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * side),
                rows_per_image: Some(side),
            },
            Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: 1,
            },
        );
    }
}

/// The pool and its writes.
pub struct SheetPoolPlugin;

impl Plugin for SheetPoolPlugin {
    fn build(&self, app: &mut App) {
        let pool = SheetPool::new(&mut app.world_mut().resource_mut::<Assets<Image>>());
        app.insert_resource(pool);
        if let Some(render) = app.get_sub_app_mut(RenderApp) {
            render
                .init_resource::<Unwritten>()
                .add_systems(ExtractSchedule, extract)
                .add_systems(Render, write.in_set(RenderSystems::PrepareResources));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet(width: usize, height: usize, value: u8) -> Arc<Rgba> {
        Arc::new(Rgba {
            width,
            height,
            pixels: vec![value; 4 * width * height],
        })
    }

    #[test]
    fn a_sheet_takes_one_layer_for_all_its_sectors_and_frees_it_with_the_last() {
        let mut images = Assets::<Image>::default();
        let mut pool = SheetPool::new(&mut images);
        let one = sheet(SHEET, SHEET, 1);
        // The first sector writes it, the second finds it.
        let (layer, bytes) = pool.take(7, &one).unwrap();
        assert_eq!(bytes, 4 * SHEET * SHEET);
        assert_eq!(pool.take(7, &one).unwrap(), (layer, 0));
        assert_eq!(pool.pending.len(), 1);
        // Released by one, it stays; by both, its layer is free and taken by the next sheet.
        pool.release(7);
        assert_eq!(pool.layer(7), Some(layer));
        pool.release(7);
        assert_eq!(pool.layer(7), None);
        assert!(pool.pending.is_empty(), "a freed layer's write is dropped");
        let (next, _) = pool.take(8, &one).unwrap();
        assert_eq!(next, layer);
        assert_eq!(pool.held(), 1);
    }

    #[test]
    fn a_full_pool_refuses_a_sheet() {
        let mut images = Assets::<Image>::default();
        let mut pool = SheetPool::new(&mut images);
        let one = sheet(SHEET, SHEET, 1);
        for k in 0..u32::from(LAYERS) {
            pool.take(k, &one).unwrap();
        }
        assert!(pool.take(u32::from(LAYERS), &one).is_err());
        pool.release(3);
        assert!(pool.take(u32::from(LAYERS), &one).is_ok());
    }

    #[test]
    fn a_smaller_sheet_is_padded_clear() {
        let small = sheet(150, 100, 9);
        let out = padded(&small).unwrap();
        assert_eq!((out.width, out.height), (SHEET, SHEET));
        let at = |x: usize, y: usize| out.pixels[4 * (SHEET * y + x)];
        assert_eq!(at(149, 99), 9);
        assert_eq!(at(150, 99), 0);
        assert_eq!(at(149, 100), 0);
        assert!(padded(&sheet(300, 10, 1)).is_err());
    }
}
