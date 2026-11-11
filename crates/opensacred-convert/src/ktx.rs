//! `build`'s textures (#700, docs/02 §12.3): a PNG master of `textures/` or `ui/screens/` → a
//! KTX2 file of [`opensacred_data::texture`]. A texture that the game reads on the CPU
//! ([`Use::Read`]) keeps its texels as they are, one level; one that the GPU samples
//! ([`Use::Sampled`]) gets a full mip chain, in BC7 where its texels are all opaque and its sides
//! whole blocks of 4, else as they are (docs/decisions/drawing.md, "Textures as KTX2 with
//! mipmaps").
//!
//! A level halves the one before by a box of 2 × 2 texels: the colours averaged in linear light,
//! each weighted by its alpha, so that the colour of a texel the alpha hides does not bleed into
//! its neighbours; the alpha averaged, then raised where the share of texels the models' alpha
//! test keeps falls short of the level 0's, as the coverage of foliage would thin out with each
//! level otherwise (Castaño, "Computing Alpha Mipmaps", 2010). It is never lowered: a model's
//! texels of alpha under 255 are blended, so a lowered alpha would make the whole model fainter
//! where only its edge grew (#816).
//!
//! A static object's sprite ([`Use::Sprite`]) gets a few levels, its edge texels repeated to whole
//! multiples first; where its texels are all of 4 bits a channel, as the original's sprites are
//! (ARGB4444), its levels are rounded to 4 bits a channel too: averaged to 8 bits, they took twice
//! the space of the images in `world.zip` (#839).

use std::sync::LazyLock;

use anyhow::{Context, Result, bail};
use intel_tex_2::{RgbaSurface, bc7};
use opensacred_data::texture::{self, Encoding};
use opensacred_data::world::{SPRITE_ALIGN, SPRITE_LEVELS};

/// What the game does with a texture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    /// Composed or read on the CPU: the world's tile sheets and atlases, the cursor.
    Read,
    /// Sampled by the GPU.
    Sampled,
    /// A static object's sprite, which the game packs into its pages on the CPU with its levels
    /// (#839): its edge texels repeated to whole multiples of [`SPRITE_ALIGN`] at its right and
    /// bottom, then [`SPRITE_LEVELS`] levels as they are, where the art reaching the edge to go on
    /// in the next sprite keeps its edge texels opaque at every level, of the art's 4 bits a
    /// channel where it has them ([`four_bits`]).
    Sprite,
}

/// The models' alpha test, the original's, which drops the texels of alpha 64 or less
/// (docs/decisions/drawing.md, "Models' translucent meshes drawn twice"): the alpha whose coverage
/// the levels keep.
const CUTOFF: f32 = 64.5 / 255.0;
/// The widest scale of a level's alpha, enough to raise the least alpha over [`CUTOFF`].
const MOST_SCALE: f32 = 256.0;

/// An image's texels in linear light: red, green and blue, and the alpha, from 0 to 1.
struct Linear {
    width: usize,
    height: usize,
    texels: Vec<[f32; 4]>,
}

/// sRGB's 8-bit values in linear light.
static TO_LINEAR: LazyLock<[f32; 256]> = LazyLock::new(|| {
    std::array::from_fn(|value| {
        let x = f32::from(u8::try_from(value).unwrap_or(u8::MAX)) / 255.0;
        if x <= 0.040_45 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    })
});

/// A value from 0 to 1 in 8 bits, rounded.
fn to_u8(x: f32) -> u8 {
    // The float is clamped to 0..255 first, so that the cast neither wraps nor truncates.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0..=255 and rounded first"
    )]
    let value = (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    value
}

/// A linear-light value in sRGB's 8 bits.
fn to_srgb(x: f32) -> u8 {
    let x = x.clamp(0.0, 1.0);
    to_u8(if x <= 0.003_130_8 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    })
}

impl Linear {
    fn of(width: usize, height: usize, rgba: &[u8]) -> Self {
        let texels = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&[r, g, b, a]| {
                [
                    TO_LINEAR[usize::from(r)],
                    TO_LINEAR[usize::from(g)],
                    TO_LINEAR[usize::from(b)],
                    f32::from(a) / 255.0,
                ]
            })
            .collect();
        Self {
            width,
            height,
            texels,
        }
    }

    /// The next level, half as wide and as tall, a side of 1 staying.
    fn half(&self) -> Self {
        let (width, height) = ((self.width / 2).max(1), (self.height / 2).max(1));
        let mut texels = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let box_ = [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dx, dy)| {
                    let sx = (2 * x + dx).min(self.width - 1);
                    let sy = (2 * y + dy).min(self.height - 1);
                    self.texels[sy * self.width + sx]
                });
                let weight: f32 = box_.iter().map(|t| t[3]).sum();
                let mut out = [0.0; 4];
                for k in 0..3 {
                    out[k] = if weight > 0.0 {
                        box_.iter().map(|t| t[k] * t[3]).sum::<f32>() / weight
                    } else {
                        box_.iter().map(|t| t[k]).sum::<f32>() / 4.0
                    };
                }
                out[3] = weight / 4.0;
                texels.push(out);
            }
        }
        Self {
            width,
            height,
            texels,
        }
    }

    /// The share of texels whose alpha, scaled by `scale`, passes the alpha test.
    fn coverage(&self, scale: f32) -> f32 {
        let kept = self.texels.iter().filter(|t| t[3] * scale > CUTOFF).count();
        kept as f32 / self.texels.len() as f32
    }

    /// The scale of the alpha, 1 or more, that brings the coverage nearest `target`: 1 where the
    /// coverage reaches it already, as a scale under 1 would take every opaque texel's alpha
    /// under 255 with it.
    fn alpha_scale(&self, target: f32) -> f32 {
        if self.coverage(1.0) >= target {
            return 1.0;
        }
        // The coverage grows with the scale: bisect between 1, under, and the widest, over.
        let (mut low, mut high) = (1.0, MOST_SCALE);
        for _ in 0..24 {
            let middle = 0.5 * (low + high);
            if self.coverage(middle) < target {
                low = middle;
            } else {
                high = middle;
            }
        }
        if target - self.coverage(low) <= self.coverage(high) - target {
            low
        } else {
            high
        }
    }

    /// The texels in 8 bits, the alpha scaled by `alpha_scale`.
    fn rgba(&self, alpha_scale: f32) -> Vec<u8> {
        self.texels
            .iter()
            .flat_map(|&[r, g, b, a]| [to_srgb(r), to_srgb(g), to_srgb(b), to_u8(a * alpha_scale)])
            .collect()
    }
}

/// The levels of an image of `width` × `height` texels, the image first, down to a side of 1 or
/// `most` of them.
fn levels(width: usize, height: usize, rgba: &[u8], most: usize) -> Vec<Vec<u8>> {
    let first = Linear::of(width, height, rgba);
    let translucent = rgba.as_chunks::<4>().0.iter().any(|t| t[3] < u8::MAX);
    let target = first.coverage(1.0);
    let mut out = vec![rgba.to_vec()];
    let mut level = first;
    while (level.width > 1 || level.height > 1) && out.len() < most {
        level = level.half();
        let scale = if translucent {
            level.alpha_scale(target)
        } else {
            1.0
        };
        out.push(level.rgba(scale));
    }
    out
}

/// A level in BC7, its texels repeated at the right and the bottom to whole blocks.
fn bc7_level(width: usize, height: usize, rgba: &[u8]) -> Result<Vec<u8>> {
    let (padded_width, padded_height) = (width.next_multiple_of(4), height.next_multiple_of(4));
    let mut padded = Vec::with_capacity(4 * padded_width * padded_height);
    for y in 0..padded_height {
        let row = &rgba[4 * width * y.min(height - 1)..][..4 * width];
        padded.extend_from_slice(row);
        let last = &row[4 * (width - 1)..];
        for _ in width..padded_width {
            padded.extend_from_slice(last);
        }
    }
    let surface = RgbaSurface {
        data: &padded,
        width: u32::try_from(padded_width)?,
        height: u32::try_from(padded_height)?,
        stride: u32::try_from(4 * padded_width)?,
    };
    Ok(bc7::compress_blocks(
        &bc7::opaque_basic_settings(),
        &surface,
    ))
}

/// The PNG `png` decoded into 8-bit RGBA: its width, its height and its texels.
fn decode_png(png: &[u8]) -> Result<(usize, usize, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().context("reading the PNG")?;
    let size = reader
        .output_buffer_size()
        .context("the PNG is too large")?;
    let mut texels = vec![0; size];
    let info = reader.next_frame(&mut texels).context("decoding the PNG")?;
    texels.truncate(info.buffer_size());
    let (width, height) = (usize::try_from(info.width)?, usize::try_from(info.height)?);
    let texels = match (info.color_type, info.bit_depth) {
        (png::ColorType::Rgba, png::BitDepth::Eight) => texels,
        (png::ColorType::GrayscaleAlpha, png::BitDepth::Eight) => texels
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[v, a]| [v, v, v, a])
            .collect(),
        (color, depth) => bail!("a PNG of {color:?} in {depth:?}, not 8-bit RGBA"),
    };
    Ok((width, height, texels))
}

/// A texture of one row or one column made two texels thick, each texel twice across it: a
/// KTX2 file of a side of 1 is read as a one-dimensional texture, which can hold no smaller levels
/// and is no two-dimensional one to sample (`GRN_SHADETEXTURE02`, 64 × 1; #2180). Sampled at its
/// row's or its column's middle, it gives the texels it held.
fn thickened(width: usize, height: usize, rgba: Vec<u8>) -> (usize, usize, Vec<u8>) {
    if height == 1 && width > 1 {
        let mut twice = rgba.clone();
        twice.extend_from_slice(&rgba);
        (width, 2, twice)
    } else if width == 1 && height > 1 {
        let twice = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|texel| [*texel, *texel])
            .flatten()
            .collect();
        (2, height, twice)
    } else {
        (width, height, rgba)
    }
}

/// The image `rgba` of `width` × `height` texels, its last column and row repeated to whole
/// multiples of `align` across and down.
fn repeated(width: usize, height: usize, rgba: &[u8], align: usize) -> (usize, usize, Vec<u8>) {
    let (wide, tall) = (
        width.next_multiple_of(align),
        height.next_multiple_of(align),
    );
    let mut out = Vec::with_capacity(4 * wide * tall);
    for y in 0..tall {
        let row = &rgba[4 * width * y.min(height - 1)..][..4 * width];
        out.extend_from_slice(row);
        let last = &row[4 * (width - 1)..];
        for _ in width..wide {
            out.extend_from_slice(last);
        }
    }
    (wide, tall, out)
}

/// The step between two values of 4 bits widened to 8 (`0xF` → `0xFF`).
const FOUR_BITS: u8 = 17;

/// Whether every value of `rgba` is one of 4 bits widened to 8, as the original's ARGB4444
/// textures' are.
fn of_four_bits(rgba: &[u8]) -> bool {
    rgba.iter().all(|v| v % FOUR_BITS == 0)
}

/// `level` rounded to 4 bits a channel, widened to 8: an alpha that passes the alpha test
/// (over 64) still does, as 65 rounds to 68.
fn four_bits(level: &mut [u8]) {
    for value in level {
        let rounded = (u16::from(*value) + u16::from(FOUR_BITS / 2)) / u16::from(FOUR_BITS);
        *value = u8::try_from(rounded).unwrap_or(15) * FOUR_BITS;
    }
}

/// The KTX2 file of the texture whose master is `png`, for `usage`.
pub fn encode(png: &[u8], usage: Use) -> Result<Vec<u8>> {
    let (width, height, rgba) = decode_png(png)?;
    if usage == Use::Sprite {
        let (width, height, rgba) = repeated(width, height, &rgba, SPRITE_ALIGN);
        let (w, h) = (u32::try_from(width)?, u32::try_from(height)?);
        let mut levels = levels(width, height, &rgba, SPRITE_LEVELS);
        if of_four_bits(&rgba) {
            levels.iter_mut().skip(1).for_each(|level| four_bits(level));
        }
        return Ok(texture::write(Encoding::Rgba8, w, h, &levels)?);
    }
    let (width, height, rgba) = thickened(width, height, rgba);
    let (w, h) = (u32::try_from(width)?, u32::try_from(height)?);
    if usage == Use::Read {
        return Ok(texture::write(Encoding::Rgba8, w, h, &[rgba])?);
    }
    let opaque = rgba.as_chunks::<4>().0.iter().all(|t| t[3] == u8::MAX);
    let levels = levels(width, height, &rgba, usize::MAX);
    if !opaque || width % 4 != 0 || height % 4 != 0 {
        return Ok(texture::write(Encoding::Rgba8, w, h, &levels)?);
    }
    let blocks = levels
        .iter()
        .enumerate()
        .map(|(index, level)| bc7_level((width >> index).max(1), (height >> index).max(1), level))
        .collect::<Result<Vec<_>>>()?;
    Ok(texture::write(Encoding::Bc7, w, h, &blocks)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
        crate::textures::encode_png(width, height, rgba).unwrap()
    }

    /// A pattern of smooth colours and edges, opaque.
    fn pattern(width: usize, height: usize) -> Vec<u8> {
        (0..width * height)
            .flat_map(|i| {
                let (x, y) = (i % width, i / width);
                let edge = if (x / 8 + y / 8) % 2 == 0 { 40 } else { 0 };
                [
                    u8::try_from(4 * x % 256).unwrap(),
                    u8::try_from(4 * y % 256).unwrap(),
                    u8::try_from(128 + edge).unwrap(),
                    255,
                ]
            })
            .collect()
    }

    #[test]
    fn a_sprite_keeps_its_edges_opaque_and_its_coverage_at_every_level() {
        // A wall's piece of 21 x 13 texels whose art reaches its right and bottom edges, opaque
        // but for a hole of 6 x 4 at its top left, of 4 bits a channel as the original's (#839).
        let (width, height) = (21, 13);
        let art = |colour: [u8; 3]| -> Vec<u8> {
            (0..width * height)
                .flat_map(|i| {
                    let (x, y) = (i % width, i / width);
                    let alpha = if x < 6 && y < 4 { 0 } else { 255 };
                    [colour[0], colour[1], colour[2], alpha]
                })
                .collect()
        };
        let rgba = art([204, 102, 51]);
        let file = encode(&png(21, 13, &rgba), Use::Sprite).unwrap();
        let (header, levels) = levels_of(&file);
        // Repeated to 24 x 16, four levels.
        assert_eq!((header.pixel_width, header.pixel_height), (24, 16));
        assert_eq!(levels.len(), SPRITE_LEVELS);
        let mut coverage = None;
        for (index, level) in levels.iter().enumerate() {
            let (w, h) = (24 >> index, 16 >> index);
            assert_eq!(level.len(), 4 * w * h, "level {index}");
            // The right column and the bottom row opaque, as the edge's art goes on.
            for y in 0..h {
                assert_eq!(level[4 * (y * w + w - 1) + 3], 255, "level {index} row {y}");
            }
            for x in 0..w {
                assert_eq!(
                    level[4 * ((h - 1) * w + x) + 3],
                    255,
                    "level {index} column {x}"
                );
            }
            let kept = level
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|t| t[3] > 64)
                .count();
            let share = kept as f32 / (w * h) as f32;
            let first = *coverage.get_or_insert(share);
            assert!(
                share >= first - 1.0 / (w * h) as f32,
                "level {index}: {share} < {first}"
            );
            // Of the art's 4 bits a channel.
            assert!(of_four_bits(level), "level {index}");
        }
        // Art of 8 bits a channel keeps them at its levels.
        let (_, levels) =
            levels_of(&encode(&png(21, 13, &art([200, 100, 50])), Use::Sprite).unwrap());
        assert_eq!(levels[1][4 * (12 * 3 + 7)..][..4], [200, 100, 50, 255]);
    }

    #[test]
    fn four_bits_round_to_the_nearest_and_keep_what_the_alpha_test_keeps() {
        let mut values = [0, 8, 9, 59, 60, 64, 65, 246, 247, 255];
        four_bits(&mut values);
        assert_eq!(values, [0, 0, 17, 51, 68, 68, 68, 238, 255, 255]);
    }

    fn levels_of(file: &[u8]) -> (ktx2::Header, Vec<Vec<u8>>) {
        let reader = ktx2::Reader::new(file).unwrap();
        let levels = reader
            .levels()
            .map(|level| zstd::decode_all(level.data).unwrap())
            .collect();
        (reader.header(), levels)
    }

    #[test]
    fn an_opaque_texture_becomes_bc7_close_to_its_master_with_every_level() {
        let rgba = pattern(64, 32);
        let file = encode(&png(64, 32, &rgba), Use::Sampled).unwrap();
        let (header, levels) = levels_of(&file);
        assert_eq!(header.format, Some(ktx2::Format::BC7_SRGB_BLOCK));
        // 64 × 32, 32 × 16, … 2 × 1, 1 × 1.
        assert_eq!(header.level_count, 7);
        assert_eq!(levels[0].len(), 16 * 16 * 8);
        assert_eq!(levels[6].len(), 16);
        // Level 0 decoded, block by block, within a few steps of the master.
        let mut decoded = vec![0u8; 64 * 32 * 4];
        for (index, block) in levels[0].chunks(16).enumerate() {
            let (bx, by) = (index % 16, index / 16);
            let at = 4 * (4 * by * 64 + 4 * bx);
            bcdec_rs::bc7(block, &mut decoded[at..], 4 * 64);
        }
        let worst = decoded
            .iter()
            .zip(&rgba)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        assert!(worst <= 8, "BC7 off by {worst}");
    }

    #[test]
    fn a_texture_with_alpha_or_odd_sides_keeps_its_texels_in_every_level() {
        let mut rgba = pattern(8, 8);
        rgba[3] = 0;
        let file = encode(&png(8, 8, &rgba), Use::Sampled).unwrap();
        let (header, levels) = levels_of(&file);
        assert_eq!(header.format, Some(ktx2::Format::R8G8B8A8_SRGB));
        assert_eq!(header.level_count, 4);
        assert_eq!(levels[0], rgba);
        let odd = pattern(6, 5);
        let (header, levels) = levels_of(&encode(&png(6, 5, &odd), Use::Sampled).unwrap());
        assert_eq!(header.format, Some(ktx2::Format::R8G8B8A8_SRGB));
        assert_eq!(levels.len(), 3);
        assert_eq!(levels[2].len(), 4);
        // The CPU's textures: as they are, one level.
        let (header, levels) = levels_of(&encode(&png(8, 8, &rgba), Use::Read).unwrap());
        assert_eq!(header.level_count, 1);
        assert_eq!(levels, vec![rgba]);
    }

    #[test]
    fn a_texture_of_one_row_or_column_is_written_two_texels_thick() {
        // `GRN_SHADETEXTURE02`'s 64 × 1, translucent: two rows alike, its levels down to 1 × 1.
        let mut row = pattern(64, 1);
        row[3] = 0;
        let (header, levels) = levels_of(&encode(&png(64, 1, &row), Use::Sampled).unwrap());
        assert_eq!((header.pixel_width, header.pixel_height), (64, 2));
        assert_eq!(header.level_count, 7);
        assert_eq!(levels[0][..row.len()], row[..]);
        assert_eq!(levels[0][row.len()..], row[..]);
        // A column likewise, each texel twice across.
        let column = pattern(1, 4);
        let (header, levels) = levels_of(&encode(&png(1, 4, &column), Use::Read).unwrap());
        assert_eq!((header.pixel_width, header.pixel_height), (2, 4));
        assert_eq!(
            levels[0][..8],
            [column[..4].to_vec(), column[..4].to_vec()].concat()[..]
        );
    }

    #[test]
    fn a_level_averages_in_linear_light_by_the_alpha() {
        // A white and a black texel average to sRGB 188, not 128; a red texel of alpha 0 lends
        // its neighbour none of its colour.
        let image = Linear::of(2, 1, &[255, 255, 255, 255, 0, 0, 0, 255]);
        assert_eq!(image.half().rgba(1.0), [188, 188, 188, 255]);
        let image = Linear::of(2, 1, &[0, 0, 255, 255, 255, 0, 0, 0]);
        assert_eq!(image.half().rgba(1.0), [0, 0, 255, 128]);
    }

    #[test]
    fn a_levels_alpha_is_raised_to_keep_the_share_that_the_alpha_test_keeps() {
        let level = Linear {
            width: 8,
            height: 1,
            texels: (1..=8).map(|a| [0.0, 0.0, 0.0, a as f32 / 10.0]).collect(),
        };
        // 0.3 to 0.8 pass the test of 64.5 / 255 as they are.
        assert_eq!(level.alpha_scale(0.75), 1.0);
        // 0.2 passes too once raised by a quarter, 0.1 stays out.
        let more = level.alpha_scale(0.875);
        assert_eq!(level.coverage(more), 0.875);
        assert!((1.26..1.28).contains(&more), "{more}");
        // A share over the target stays: a scale under 1 would take every alpha down with it.
        assert_eq!(level.alpha_scale(0.5), 1.0);
        // A blob of soft alpha: each level keeps about level 0's share, where a plain average
        // would let its fringe pass the test.
        let rgba: Vec<u8> = (0..32 * 32)
            .flat_map(|i| {
                let (x, y) = ((i % 32) as f32 - 15.5, (i / 32) as f32 - 15.5);
                let r = (x * x + y * y).sqrt();
                [90, 140, 60, to_u8(1.0 - r / 12.0)]
            })
            .collect();
        let target = Linear::of(32, 32, &rgba).coverage(1.0);
        let levels = levels(32, 32, &rgba, usize::MAX);
        assert_eq!(levels.len(), 6);
        for (index, level) in levels.iter().enumerate().take(4) {
            let alphas: Vec<u8> = level.as_chunks::<4>().0.iter().map(|t| t[3]).collect();
            let kept = alphas.iter().filter(|&&a| a > 64).count() as f32 / alphas.len() as f32;
            assert!(
                (kept - target).abs() <= 0.06,
                "level {index}: {kept} for {target}"
            );
        }
    }

    #[test]
    fn the_opaque_texels_stay_opaque_at_every_level() {
        // An opaque skin with a stroke of alpha 0 down its column 15: at level 1 the boxes over
        // the stroke average to 0.5 and pass the alpha test, more than level 0's share; the
        // texels away from the stroke keep their alpha of 255 all the same (#816).
        let rgba: Vec<u8> = (0..32 * 32)
            .flat_map(|i| [120, 80, 60, if i % 32 == 15 { 0 } else { 255 }])
            .collect();
        let levels = levels(32, 32, &rgba, usize::MAX);
        for (index, level) in levels.iter().enumerate() {
            let width = 32 >> index;
            let stroke = 15 >> index;
            for (texel, &[.., alpha]) in level.as_chunks::<4>().0.iter().enumerate() {
                if texel % width == stroke {
                    assert!(alpha < u8::MAX, "level {index}, texel {texel}");
                } else {
                    assert_eq!(alpha, u8::MAX, "level {index}, texel {texel}");
                }
            }
        }
    }
}
