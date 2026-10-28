//! The interface over the world: the original's cursor, the picture that `cursors` chooses
//! (`MOUSE_MAIN.TGA` and the others), scaled in whole steps with the system's display scale, and
//! the interface laid out on the original's 768-pixel-high screen scaled to the window's height,
//! as the world is. The original lays its interface out in code from the `gui_main_*` atlases
//! (docs/02 §11); the bar and the hero's portrait are `bar`'s (#1087), the windows
//! `inventory`'s and the others' (docs/decisions/2026-10-17-hud.md).
//!
//! Bevy's interface writes its colours and texels into the frame as they are, and the frame holds
//! sRGB values (`srgb`): an interface gives them in those values, its colours as linear ones and
//! its images read as stored, so that they show as given and blend as the original's interface
//! does.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageLoaderSettings;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::{CursorIcon, CustomCursor, CustomCursorImage};
use opensacred_data::texture;

use crate::cursors::{Look, Shown};
use crate::files;

/// The height of the original's screen, which the interface is laid out on.
const SCREEN_HEIGHT: f32 = 768.0;

/// The original's cursors (`cursors::Look`): each picture in the top left 32 × 32 of its 64 × 64
/// image, its point at the corner.
const CURSOR_SIDE: usize = 32;

/// The interface's images.
#[derive(Resource)]
pub struct Hud {
    /// The cursors' image files that the converted textures have.
    pub cursors: HashMap<Look, PathBuf>,
}

/// The converted texture named `name` (`textures/texture/<record>-<name>.ktx2`).
pub fn find_texture(assets: &Path, name: &str) -> Option<PathBuf> {
    let dir = assets.join("textures").join("texture");
    let suffix = format!("-{name}.{}", texture::EXTENSION);
    files::read_dir(&dir).ok()?.into_iter().find(|path| {
        path.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(&suffix))
    })
}

/// The converted textures named `names`, relative to the asset directory with `/` between the
/// parts, as the asset server takes them; one look through the directory for all.
pub fn find_textures(assets: &Path, names: &[&str]) -> Vec<Option<String>> {
    let suffixes: Vec<String> = names
        .iter()
        .map(|name| format!("-{name}.{}", texture::EXTENSION))
        .collect();
    let mut found = vec![None; names.len()];
    let dir = assets.join("textures").join("texture");
    for path in files::read_dir(&dir).into_iter().flatten() {
        let Some(file) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        if let Some(i) = suffixes.iter().position(|suffix| file.ends_with(suffix)) {
            found[i] = Some(format!("textures/texture/{file}"));
        }
    }
    found
}

/// The cursors' image files (`cursors::Look::texture`), one look through the directory for all.
pub fn cursor_files(assets: &Path) -> HashMap<Look, PathBuf> {
    let dir = assets.join("textures").join("texture");
    let mut found = HashMap::new();
    for path in files::read_dir(&dir).into_iter().flatten() {
        let Some(file) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        if let Some(look) = Look::ALL
            .into_iter()
            .find(|look| file.ends_with(&format!("-{}.{}", look.texture(), texture::EXTENSION)))
        {
            found.insert(look, path.clone());
        }
    }
    found
}

/// An interface image, read as stored: in the frame's sRGB values.
pub fn image(assets: &AssetServer, path: impl Into<String>) -> Handle<Image> {
    assets
        .load_builder()
        .with_settings(|settings: &mut ImageLoaderSettings| settings.is_srgb = false)
        .load(path.into())
}

/// The top left `CURSOR_SIDE` square of an 8-bit RGBA texture, each pixel `scale` times wider
/// and taller.
fn cursor_image(path: &Path, scale: usize) -> Result<Image> {
    let data = files::read(path).map_err(|e| anyhow!("reading {}: {e}", path.display()))?;
    let texture::Rgba {
        width,
        height,
        pixels,
    } = texture::read(&data).map_err(|e| anyhow!("decoding {}: {e}", path.display()))?;
    if width < CURSOR_SIDE || height < CURSOR_SIDE {
        bail!("{}: not an image of at least 32 x 32", path.display());
    }
    let side = CURSOR_SIDE * scale;
    let mut out = Vec::with_capacity(side * side * 4);
    for y in 0..side {
        for x in 0..side {
            let i = ((y / scale) * width + x / scale) * 4;
            out.extend_from_slice(&pixels[i..i + 4]);
        }
    }
    let side = u32::try_from(side)?;
    Ok(Image::new(
        Extent3d {
            width: side,
            height: side,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        out,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    ))
}

/// The cursor's scale: the system's display scale in whole steps, rounded down and at least 1, so
/// that the cursor grows as the system's own does and keeps its texels square, whatever the
/// window's size and mode (#442). The window's scale factor is held at 1 (`main`), so the system's
/// is the backend's.
fn cursor_scale(display_scale: f32) -> usize {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a display's scale is a small positive number"
    )]
    let scale = display_scale.floor().max(1.0) as usize;
    scale
}

/// Puts the original's cursor that `cursors` chooses on the window, scaled with the system's
/// display scale, its point at the top left (`0x6548A0`), and makes it again when either changes:
/// the scale is not known yet at the start, and a window moved to another monitor takes that
/// monitor's. A picture made is kept for its scale; one whose texture is missing shows the arrow.
fn cursor(
    mut commands: Commands,
    hud: Res<Hud>,
    look: Res<Shown>,
    window: Single<(Entity, &Window)>,
    mut images: ResMut<Assets<Image>>,
    mut made: Local<HashMap<(Look, usize), Handle<Image>>>,
    mut shown: Local<Option<(Look, usize)>>,
) {
    let (entity, window) = *window;
    let display_scale = window.resolution.base_scale_factor();
    let scale = cursor_scale(display_scale);
    let look = if hud.cursors.contains_key(&look.0) {
        look.0
    } else {
        Look::Arrow
    };
    if *shown == Some((look, scale)) {
        return;
    }
    let Some(path) = hud.cursors.get(&look) else {
        return;
    };
    if shown.is_none_or(|(_, before)| before != scale) {
        info!(
            "cursor: {0} x {0} pixels at the display scale {display_scale}",
            CURSOR_SIDE * scale
        );
    }
    *shown = Some((look, scale));
    let handle = match made.get(&(look, scale)) {
        Some(handle) => handle.clone(),
        None => match cursor_image(path, scale) {
            Ok(image) => {
                let handle = images.add(image);
                made.insert((look, scale), handle.clone());
                handle
            }
            Err(e) => {
                bevy::log::warn!("cursor: {e:#}");
                return;
            }
        },
    };
    commands
        .entity(entity)
        .insert(CursorIcon::Custom(CustomCursor::Image(CustomCursorImage {
            handle,
            hotspot: (0, 0),
            ..default()
        })));
}

/// Scales the interface with the window's height.
pub fn scale(window: Single<&Window>, mut ui_scale: ResMut<UiScale>) {
    let scale = window.height() / SCREEN_HEIGHT;
    if ui_scale.0 != scale {
        ui_scale.0 = scale;
    }
}

/// Where the cursor is over the window, as the game's input saw it last: the moves of the
/// system's cursor, and those the remote protocol sends (`remote`), which leave the system's
/// cursor where it is.
#[derive(Resource, Default)]
pub struct Pointer(pub Option<Vec2>);

fn point(
    mut pointer: ResMut<Pointer>,
    mut moved: MessageReader<CursorMoved>,
    mut left: MessageReader<CursorLeft>,
) {
    if left.read().count() > 0 {
        pointer.0 = None;
    }
    if let Some(moved) = moved.read().last() {
        pointer.0 = Some(moved.position);
    }
}

/// The cursor, where it points, and the interface's scale.
pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pointer>()
            .add_systems(PreUpdate, point.after(bevy::input::InputSystems))
            .add_systems(Update, (cursor, scale));
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn the_cursor_grows_in_whole_steps_of_the_display_scale() {
        assert_eq!(cursor_scale(1.0), 1);
        assert_eq!(cursor_scale(1.5), 1);
        assert_eq!(cursor_scale(1.75), 1);
        assert_eq!(cursor_scale(2.0), 2);
        assert_eq!(cursor_scale(3.5), 3);
        assert_eq!(cursor_scale(0.5), 1);
    }

    #[test]
    fn the_cursor_is_the_top_left_square_scaled_in_whole_steps() {
        let path = std::env::temp_dir().join(format!(
            "opensacred-game-cursor-{}.{}",
            std::process::id(),
            texture::EXTENSION
        ));
        let mut rgba = vec![0u8; 64 * 64 * 4];
        // An opaque red pixel at (1, 0) and one outside the square at (40, 40).
        rgba[4..8].copy_from_slice(&[255, 0, 0, 255]);
        let far = (40 * 64 + 40) * 4;
        rgba[far..far + 4].copy_from_slice(&[0, 255, 0, 255]);
        let file = texture::write(texture::Encoding::Rgba8, 64, 64, &[rgba]).unwrap();
        fs::write(&path, file).unwrap();
        let image = cursor_image(&path, 2).unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!((image.width(), image.height()), (64, 64));
        let data = image.data.unwrap();
        let at = |x: usize, y: usize| &data[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
        assert_eq!(at(2, 0), [255, 0, 0, 255]);
        assert_eq!(at(3, 1), [255, 0, 0, 255]);
        assert_eq!(at(0, 0), [0, 0, 0, 0]);
        assert!(data.chunks(4).all(|p| p[1] == 0));
    }
}
