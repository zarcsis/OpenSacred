//! The interface over the world: the original's cursor, the picture that `cursors` chooses
//! (`MOUSE_MAIN.TGA` and the others), scaled with the system's display scale (#2436), and
//! the interface laid out on the original's 768-pixel-high screen scaled to the window's height,
//! as the world is. The original lays its interface out in code from the `gui_main_*` atlases
//! (docs/02 §11); the bar and the hero's portrait are `bar`'s (#1087), the windows
//! `inventory`'s and the others' (docs/decisions/interface.md, "Bar, portrait and cursor scaled").
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

use crate::cursors::{Glow, Look, Shown};
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

/// The glow's reach and step about the cursor's picture in the original's pixels, and the alpha
/// of each of its copies (`0x6548A0`, docs/re/cursor.md, "The point, the size, the tint and the
/// buttons").
const GLOW_REACH: usize = 6;
const GLOW_STEP: usize = 2;
const GLOW_ALPHA: f32 = 8.0 / 255.0;

/// A byte of a share of 1.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the share is held to 0..=1 first"
)]
pub(crate) fn byte(share: f32) -> u8 {
    (share.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// `square`, `side` × `side` RGBA, with the original's glow about it, `GLOW_REACH` wider on each
/// side: the picture drawn again at the offsets −6 to +6 by 2 on both axes, 49 times, its colours
/// added at the alpha 8/255, then the picture over them (#1461). A system's cursor blends rather
/// than adds, so the light added is given as a colour of its own alpha, its brightest channel.
fn glowing(square: &[u8], side: usize) -> (Vec<u8>, usize) {
    let wide = side + 2 * GLOW_REACH;
    let mut light = vec![[0.0_f32; 3]; wide * wide];
    for dy in (0..=2 * GLOW_REACH).step_by(GLOW_STEP) {
        for dx in (0..=2 * GLOW_REACH).step_by(GLOW_STEP) {
            for (i, texel) in square.as_chunks::<4>().0.iter().enumerate() {
                let (x, y) = (i % side, i / side);
                let alpha = f32::from(texel[3]) / 255.0 * GLOW_ALPHA;
                let at = &mut light[(y + dy) * wide + x + dx];
                for c in 0..3 {
                    at[c] += f32::from(texel[c]) / 255.0 * alpha;
                }
            }
        }
    }
    let mut out = Vec::with_capacity(wide * wide * 4);
    for (i, added) in light.iter().enumerate() {
        let (x, y) = (i % wide, i / wide);
        let glow = added.iter().copied().fold(0.0, f32::max).min(1.0);
        let base = if glow > 0.0 {
            added.map(|c| (c / glow).min(1.0))
        } else {
            [0.0; 3]
        };
        let inside = (GLOW_REACH..GLOW_REACH + side).contains(&x)
            && (GLOW_REACH..GLOW_REACH + side).contains(&y);
        let texel = inside.then(|| {
            let j = ((y - GLOW_REACH) * side + x - GLOW_REACH) * 4;
            &square[j..j + 4]
        });
        let over = texel.map_or(0.0, |texel| f32::from(texel[3]) / 255.0);
        let alpha = over + glow * (1.0 - over);
        for c in 0..3 {
            let top = texel.map_or(0.0, |texel| f32::from(texel[c]) / 255.0);
            let colour = if alpha > 0.0 {
                (top * over + base[c] * glow * (1.0 - over)) / alpha
            } else {
                0.0
            };
            out.push(byte(colour));
        }
        out.push(byte(alpha));
    }
    (out, wide)
}

/// The side of a picture of `unscaled` pixels at the scale of `percent` hundredths, rounded.
fn scaled_side(unscaled: usize, percent: u32) -> usize {
    let percent = usize::try_from(percent).unwrap_or(100);
    (unscaled * percent + 50) / 100
}

/// `square`, `unscaled` × `unscaled` RGBA, at the scale of `percent` hundredths ([`scaled_side`]):
/// at a whole scale each pixel repeated, so that its texels stay square; between whole scales
/// resampled bilinearly over its colours weighed by their alpha, as the system scales its own
/// cursors (#2436).
fn scaled_picture(square: &[u8], unscaled: usize, percent: u32) -> (Vec<u8>, usize) {
    let side = scaled_side(unscaled, percent);
    let mut out = Vec::with_capacity(side * side * 4);
    if percent.is_multiple_of(100) {
        let whole = side / unscaled.max(1);
        for y in 0..side {
            for x in 0..side {
                let i = ((y / whole) * unscaled + x / whole) * 4;
                out.extend_from_slice(&square[i..i + 4]);
            }
        }
        return (out, side);
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a cursor's sides are a few dozen pixels"
    )]
    let ratio = unscaled as f32 / side as f32;
    let last = unscaled.saturating_sub(1);
    // The source's coordinate under an output's pixel's centre, its two neighbours and the
    // second's weight.
    let source = |at: usize| {
        #[expect(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a cursor's coordinates are a few dozen pixels"
        )]
        let (low, weight) = {
            let s = ((at as f32 + 0.5) * ratio - 0.5).clamp(0.0, last as f32);
            (s.floor() as usize, s.fract())
        };
        (low, (low + 1).min(last), weight)
    };
    let texel = |x: usize, y: usize| {
        let i = (y * unscaled + x) * 4;
        let alpha = f32::from(square[i + 3]) / 255.0;
        [
            f32::from(square[i]) * alpha,
            f32::from(square[i + 1]) * alpha,
            f32::from(square[i + 2]) * alpha,
            alpha,
        ]
    };
    for y in 0..side {
        let (y0, y1, wy) = source(y);
        for x in 0..side {
            let (x0, x1, wx) = source(x);
            let mut mixed = [0.0f32; 4];
            for (tx, ty, w) in [
                (x0, y0, (1.0 - wx) * (1.0 - wy)),
                (x1, y0, wx * (1.0 - wy)),
                (x0, y1, (1.0 - wx) * wy),
                (x1, y1, wx * wy),
            ] {
                for (sum, value) in mixed.iter_mut().zip(texel(tx, ty)) {
                    *sum += value * w;
                }
            }
            let alpha = mixed[3];
            let colour = |value: f32| {
                let value = if alpha > 0.0 { value / alpha } else { 0.0 };
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "held to a byte's range"
                )]
                let byte = value.round().clamp(0.0, 255.0) as u8;
                byte
            };
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "held to a byte's range"
            )]
            let opacity = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
            out.extend_from_slice(&[
                colour(mixed[0]),
                colour(mixed[1]),
                colour(mixed[2]),
                opacity,
            ]);
        }
    }
    (out, side)
}

/// The top left `CURSOR_SIDE` square of an 8-bit RGBA texture, with its glow where `glows`
/// ([`glowing`]), at the scale of `percent` hundredths ([`scaled_picture`]).
fn cursor_image(path: &Path, percent: u32, glows: bool) -> Result<Image> {
    let data = files::read(path).map_err(|e| anyhow!("reading {}: {e}", path.display()))?;
    let texture::Rgba {
        width,
        height,
        pixels,
    } = texture::read(&data).map_err(|e| anyhow!("decoding {}: {e}", path.display()))?;
    if width < CURSOR_SIDE || height < CURSOR_SIDE {
        bail!("{}: not an image of at least 32 x 32", path.display());
    }
    let mut square = Vec::with_capacity(CURSOR_SIDE * CURSOR_SIDE * 4);
    for y in 0..CURSOR_SIDE {
        let row = y * width * 4;
        square.extend_from_slice(&pixels[row..row + CURSOR_SIDE * 4]);
    }
    let (square, unscaled) = if glows {
        glowing(&square, CURSOR_SIDE)
    } else {
        (square, CURSOR_SIDE)
    };
    let (out, side) = scaled_picture(&square, unscaled, percent);
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

/// The cursor's scale in hundredths: the system's display scale, at least 1, so that the cursor
/// grows as the system's own do, whatever the window's size and mode (#442), 48 × 48 at 150 % by
/// the maintainer's decision of 2026-11-09 (#2436). The window's scale factor is held at 1
/// (`main`), so the system's is the backend's.
fn cursor_scale(display_scale: f32) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a display's scale is a small positive number"
    )]
    let percent = (display_scale.max(1.0) * 100.0).round() as u32;
    percent
}

/// `value` pixels of the unscaled picture at the scale of `percent` hundredths, rounded.
fn scaled_point(value: u16, percent: u32) -> u16 {
    let scaled = (u32::from(value) * percent + 50) / 100;
    u16::try_from(scaled).unwrap_or(u16::MAX)
}

/// Puts the original's cursor that `cursors` chooses on the window, scaled with the system's
/// display scale, its point at the top left (`0x6548A0`), and makes it again when either changes:
/// the scale is not known yet at the start, and a window moved to another monitor takes that
/// monitor's. A picture made is kept for its scale; one whose texture is missing shows the arrow.
fn cursor(
    mut commands: Commands,
    hud: Res<Hud>,
    (look, glow): (Res<Shown>, Res<Glow>),
    window: Single<(Entity, &Window)>,
    mut images: ResMut<Assets<Image>>,
    mut made: Local<HashMap<(Look, u32, bool), Handle<Image>>>,
    mut shown: Local<Option<(Look, u32, bool)>>,
) {
    let glows = glow.0;
    let (entity, window) = *window;
    let display_scale = window.resolution.base_scale_factor();
    let scale = cursor_scale(display_scale);
    let look = if hud.cursors.contains_key(&look.0) {
        look.0
    } else {
        Look::Arrow
    };
    if *shown == Some((look, scale, glows)) {
        return;
    }
    let Some(path) = hud.cursors.get(&look) else {
        return;
    };
    if shown.is_none_or(|(_, before, _)| before != scale) {
        info!(
            "cursor: {0} x {0} pixels at the display scale {display_scale}",
            scaled_side(CURSOR_SIDE, scale)
        );
    }
    *shown = Some((look, scale, glows));
    let handle = match made.get(&(look, scale, glows)) {
        Some(handle) => handle.clone(),
        None => match cursor_image(path, scale, glows) {
            Ok(image) => {
                let handle = images.add(image);
                made.insert((look, scale, glows), handle.clone());
                handle
            }
            Err(e) => {
                bevy::log::warn!("cursor: {e:#}");
                return;
            }
        },
    };
    // The glow widens the picture by its reach on each side, its point as far in; a picture's
    // own point by the scale.
    let spot = scaled_point(look.spot(), scale);
    let point = if glows {
        scaled_point(u16::try_from(GLOW_REACH).unwrap_or(0), scale)
    } else {
        0
    };
    commands
        .entity(entity)
        .insert(CursorIcon::Custom(CustomCursor::Image(CustomCursorImage {
            handle,
            hotspot: (point + spot, point + spot),
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

pub(crate) fn point(
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
    fn the_cursor_grows_with_the_display_scale() {
        // The system's scale exactly (#2436), at least 1.
        assert_eq!(cursor_scale(1.0), 100);
        assert_eq!(cursor_scale(1.25), 125);
        assert_eq!(cursor_scale(1.5), 150);
        assert_eq!(cursor_scale(2.0), 200);
        assert_eq!(cursor_scale(0.5), 100);
        // The picture's side and its point: 48 × 48 at 150 %, 40 at 125 %, 64 at 200 %.
        assert_eq!(scaled_side(CURSOR_SIDE, 150), 48);
        assert_eq!(scaled_side(CURSOR_SIDE, 125), 40);
        assert_eq!(scaled_side(CURSOR_SIDE, 200), 64);
        assert_eq!(scaled_point(16, 150), 24);
        assert_eq!(scaled_point(6, 125), 8);
    }

    #[test]
    fn a_cursors_picture_repeats_at_a_whole_scale_and_blends_between() {
        // A 2 × 2 picture: an opaque red pixel and three clear ones.
        let mut square = vec![0u8; 2 * 2 * 4];
        square[..4].copy_from_slice(&[255, 0, 0, 255]);
        let (whole, side) = scaled_picture(&square, 2, 200);
        assert_eq!(side, 4);
        for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            assert_eq!(
                &whole[(y * 4 + x) * 4..(y * 4 + x) * 4 + 4],
                &[255, 0, 0, 255]
            );
        }
        assert_eq!(whole[(2 * 4 + 2) * 4 + 3], 0);
        // At 150 % the corner stays red and opaque, the clear ones stay clear, and the colour of a
        // blended pixel is the red's, its alpha between, no darker fringe.
        let (between, side) = scaled_picture(&square, 2, 150);
        assert_eq!(side, 3);
        let at = |x: usize, y: usize| &between[(y * 3 + x) * 4..(y * 3 + x) * 4 + 4];
        assert_eq!(at(0, 0), &[255, 0, 0, 255]);
        assert_eq!(at(2, 2)[3], 0);
        let middle = at(1, 1);
        assert_eq!(&middle[..3], &[255, 0, 0]);
        assert!(middle[3] > 0 && middle[3] < 255, "{middle:?}");
    }

    #[test]
    fn the_glow_spreads_a_pictures_colours_six_pixels_about_it() {
        // One opaque white pixel: 49 copies, each a 6-pixel step's reach, add 8/255 apiece.
        let mut square = vec![0u8; 4 * 4 * 4];
        square[..4].copy_from_slice(&[255, 255, 255, 255]);
        let (out, wide) = glowing(&square, 4);
        assert_eq!(wide, 16);
        let at = |x: usize, y: usize| &out[(y * wide + x) * 4..(y * wide + x) * 4 + 4];
        // The pixel itself, at the reach in from the corner, over its own copy.
        assert_eq!(at(6, 6), &[255, 255, 255, 255]);
        // A copy 6 to the right and 6 down, faint white; none between the steps.
        assert_eq!(at(12, 12), &[255, 255, 255, 8]);
        assert_eq!(at(7, 6)[3], 0);
        assert_eq!(at(0, 0), &[255, 255, 255, 8]);
    }

    #[test]
    fn the_cursor_is_the_top_left_square_scaled() {
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
        let image = cursor_image(&path, 200, false).unwrap();
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
