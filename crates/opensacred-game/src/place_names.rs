//! The world map's places' names (#1577; docs/re/maps.md, "The places, the people, the portals,
//! the frontier and the pacified areas"): each name that the start scripts' `SetMapIcon`s keep
//! (`scripts::Live::map_names`) written on an arc about its place, as the original's font 3 writes
//! it with the flags `0x820` (`0x6C8A40`, `cFontTTF2`'s draw `0x64E9C0`).
//!
//! The arc: the curvature R gives the inner radius 5120 / |R| (the text's rectangle's width 256
//! times 20), the glyphs from it out to it and their cell's height, 0.01 rad of the arc a pixel
//! of the text's width, the text's middle at the angle; R > 0 the arc's centre above the place,
//! the text under it, the angle counter-clockwise; R < 0 the centre below, the text over it, the
//! angle clockwise. The original lays each glyph's quad from its pen's angle to its width's
//! 0.01 rad further; here the line is written upright once and every pixel of the arc's box
//! takes it through the arc's inverse, which is the same map.
//!
//! The ink (the flag `0x800`): black whose opacity is `ui_worldMapFontBg`'s alpha at the map's
//! pixel, wrapped every 512 pixels, times the glyph's coverage; nothing behind the letters.
//!
//! The glyphs are hinted and their advances whole at the font's size, as GDI's cells are; the
//! arc's angle takes no kerning in the original either. Not followed: the font 3's glyph cell's
//! width and height, taken as the advances and the font's ascent and descent (**hypothesis**).

use std::f32::consts::{PI, TAU};

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use swash::FontRef;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::Format;

/// The font 3, AntiquaSSK 16, at the size the overview writes it (`overview`).
pub const SIZE: f32 = 24.0;
/// The arc's angle a pixel of the text's width (`0x64E9C0`).
const STEP: f32 = 0.01;
/// The arc's inner radius times the curvature (`0x64E9C0`: the rectangle's width 256 times 20).
const RADIUS: f32 = 5120.0;
/// The map's pixels that the ink's texture spans, whatever its size: its coordinates are the
/// map's pixel over 512 (`0x6C8A40`, the scroll over 512 added to the screen's).
const INK_SPAN: f32 = 512.0;

/// A line written upright: each pixel's coverage, row by row.
struct Strip {
    width: usize,
    height: usize,
    coverage: Vec<f32>,
}

impl Strip {
    /// The coverage at `(u, v)`, between the pixels' middles, none outside.
    fn at(&self, u: f32, v: f32) -> f32 {
        let (x, y) = (u - 0.5, v - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let texel = |x: f32, y: f32| -> f32 {
            let (Some(x), Some(y)) = (index(x), index(y)) else {
                return 0.0;
            };
            if x >= self.width || y >= self.height {
                return 0.0;
            }
            self.coverage[y * self.width + x]
        };
        let top = texel(x0, y0) * (1.0 - fx) + texel(x0 + 1.0, y0) * fx;
        let bottom = texel(x0, y0 + 1.0) * (1.0 - fx) + texel(x0 + 1.0, y0 + 1.0) * fx;
        top * (1.0 - fy) + bottom * fy
    }
}

/// A count or a place of an image's pixels as a float, the images far below 65,536 pixels.
fn float(pixels: usize) -> f32 {
    f32::from(u16::try_from(pixels).unwrap_or(u16::MAX))
}

/// `value`, a place of an image's pixels, as an index: none below 0 or not finite.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a place in an image, small and checked not negative"
)]
fn index(value: f32) -> Option<usize> {
    (value.is_finite() && value >= 0.0).then_some(value as usize)
}

/// Writes `text` upright in the font `font` at `size` pixels an em: each glyph's outline at its
/// pen, the pens by the glyphs' advances, each whole at [`SIZE`] as GDI's are (`scale` of the
/// strip's pixels a text's pixel), the baseline the font's ascent down, the strip as high as the
/// ascent and the descent.
fn strip(font: &[u8], text: &str, size: f32, scale: f32) -> Option<Strip> {
    let font = FontRef::from_index(font, 0)?;
    let metrics = font.metrics(&[]).scale(size);
    let glyph_metrics = font.glyph_metrics(&[]).scale(size);
    let advance = |glyph| (glyph_metrics.advance_width(glyph) / scale).round() * scale;
    let charmap = font.charmap();
    let glyphs: Vec<_> = text.chars().map(|c| charmap.map(c)).collect();
    let width: f32 = glyphs.iter().map(|&glyph| advance(glyph)).sum();
    let height = metrics.ascent + metrics.descent;
    let (Some(columns), Some(rows)) = (index(width.ceil()), index(height.ceil())) else {
        return None;
    };
    if columns == 0 || rows == 0 {
        return None;
    }
    let mut coverage = vec![0.0; columns * rows];
    let mut context = ScaleContext::new();
    let mut scaler = context.builder(font).size(size).hint(true).build();
    let mut pen = 0.0;
    for glyph in glyphs {
        if let Some(image) = Render::new(&[Source::Outline])
            .format(Format::Alpha)
            .render(&mut scaler, glyph)
        {
            let placement = image.placement;
            let offset = |value: i32| f32::from(i16::try_from(value).unwrap_or(0));
            let left = offset(placement.left) + pen;
            let top = metrics.ascent - offset(placement.top);
            for (row, line) in image
                .data
                .chunks(usize::try_from(placement.width).unwrap_or(1).max(1))
                .enumerate()
            {
                for (column, &alpha) in line.iter().enumerate() {
                    let x = (left + float(column)).round();
                    let y = (top + float(row)).round();
                    let (Some(x), Some(y)) = (index(x), index(y)) else {
                        continue;
                    };
                    if x < columns && y < rows {
                        let cell = &mut coverage[y * columns + x];
                        *cell = (*cell + f32::from(alpha) / 255.0).min(1.0);
                    }
                }
            }
        }
        pen += advance(glyph);
    }
    Some(Strip {
        width: columns,
        height: rows,
        coverage,
    })
}

/// A name's arc on the map: its centre, its inner radius, the glyphs' height, the angle of the
/// text's middle, its width, and whether it runs under the centre (R > 0).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Arc {
    centre: Vec2,
    inner: f32,
    height: f32,
    middle: f32,
    width: f32,
    under: bool,
}

impl Arc {
    /// The arc of a text `width` pixels wide and `height` high at the map's pixel `place`, of the
    /// curvature `curvature` and the angle `angle` in degrees; none for the curvature 0.
    fn new(place: Vec2, (curvature, angle): (i8, i8), (width, height): (f32, f32)) -> Option<Self> {
        if curvature == 0 {
            return None;
        }
        let inner = RADIUS / f32::from(curvature).abs();
        let under = curvature > 0;
        Some(Self {
            centre: place + Vec2::new(0.0, if under { -inner } else { inner }),
            inner,
            height,
            middle: f32::from(angle).to_radians(),
            width,
            under,
        })
    }

    /// The map's point of the strip's point `(u, v)`, in the text's pixels from its top left.
    fn point(&self, u: f32, v: f32) -> Vec2 {
        let angle = self.middle + (u - self.width / 2.0) * STEP;
        let (sin, cos) = angle.sin_cos();
        if self.under {
            self.centre + (self.inner + v) * Vec2::new(sin, cos)
        } else {
            self.centre + (self.inner + self.height - v) * Vec2::new(sin, -cos)
        }
    }

    /// The strip's point of the map's point `at`.
    fn strip_point(&self, at: Vec2) -> (f32, f32) {
        let d = at - self.centre;
        let (angle, radius) = if self.under {
            (d.x.atan2(d.y), d.length())
        } else {
            (d.x.atan2(-d.y), d.length())
        };
        let turned = (angle - self.middle + PI).rem_euclid(TAU) - PI;
        let u = turned / STEP + self.width / 2.0;
        let v = if self.under {
            radius - self.inner
        } else {
            self.inner + self.height - radius
        };
        (u, v)
    }

    /// The box on the map that holds the arc's text.
    fn bounds(&self) -> Rect {
        let mut bounds = Rect::from_center_size(self.point(0.0, 0.0), Vec2::ZERO);
        let steps = 64_u8;
        for i in 0..=steps {
            let u = self.width * f32::from(i) / f32::from(steps);
            for v in [0.0, self.height] {
                bounds = bounds.union_point(self.point(u, v));
            }
        }
        for i in 0..=steps {
            let v = self.height * f32::from(i) / f32::from(steps);
            for u in [0.0, self.width] {
                bounds = bounds.union_point(self.point(u, v));
            }
        }
        Rect {
            min: bounds.min.floor() - Vec2::ONE,
            max: bounds.max.ceil() + Vec2::ONE,
        }
    }
}

/// The ink's backdrop: the alpha of `ui_worldMapFontBg`, wrapped over the map.
pub struct Ink {
    width: usize,
    height: usize,
    alpha: Vec<f32>,
}

impl Ink {
    /// The backdrop of the texture file `file` of the assets, an 8-bit RGBA one.
    pub fn read(file: &str) -> anyhow::Result<Self> {
        let data = crate::files::read(file).map_err(|e| anyhow::anyhow!("reading {file}: {e}"))?;
        let texture = opensacred_data::texture::read(&data)
            .map_err(|e| anyhow::anyhow!("decoding {file}: {e}"))?;
        Self::new(texture.width, texture.height, &texture.pixels)
            .ok_or_else(|| anyhow::anyhow!("{file}: an empty texture"))
    }

    /// The backdrop of an 8-bit RGBA texture `width` by `height`.
    pub fn new(width: usize, height: usize, rgba: &[u8]) -> Option<Self> {
        if width == 0 || height == 0 || rgba.len() < width * height * 4 {
            return None;
        }
        let alpha = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&[_, _, _, alpha]| f32::from(alpha) / 255.0)
            .collect();
        Some(Self {
            width,
            height,
            alpha,
        })
    }

    /// The opacity at the map's point `at`, the texture spanning [`INK_SPAN`] pixels, between
    /// the texels' middles, wrapped.
    fn at(&self, at: Vec2) -> f32 {
        let texels = Vec2::new(float(self.width), float(self.height)) / INK_SPAN;
        let (x, y) = (at.x * texels.x - 0.5, at.y * texels.y - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let texel = |x: f32, y: f32| -> f32 {
            let wrap = |value: f32, side: usize| -> usize {
                index(value.rem_euclid(float(side))).map_or(0, |i| i % side)
            };
            self.alpha[wrap(y, self.height) * self.width + wrap(x, self.width)]
        };
        let top = texel(x0, y0) * (1.0 - fx) + texel(x0 + 1.0, y0) * fx;
        let bottom = texel(x0, y0 + 1.0) * (1.0 - fx) + texel(x0 + 1.0, y0 + 1.0) * fx;
        top * (1.0 - fy) + bottom * fy
    }
}

/// What a name's image is made of: its text, its cell, its arc and its image's scale.
#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    text: String,
    cell: (i32, i32),
    arc: (i8, i8),
    scale: f32,
}

impl Key {
    /// The key of the name `place` written at `scale`.
    pub fn of(place: &opensacred_rules::quests::MapName, scale: f32) -> Self {
        Self {
            text: place.text.clone(),
            cell: place.cell,
            arc: (place.curvature, place.angle),
            scale,
        }
    }
}

/// The name `text` at the map's pixel `place` on its arc, written into an image of `scale` of its
/// pixels a map's pixel, black of the ink's opacity: the map's box it covers and the image; none
/// for an empty text or the curvature 0.
pub fn render(
    font: &[u8],
    text: &str,
    (place, curvature, angle): (Vec2, i8, i8),
    ink: Option<&Ink>,
    scale: f32,
) -> Option<(Rect, Image)> {
    let scale = scale.max(1.0);
    let strip = strip(font, text, SIZE * scale, scale)?;
    let size = |pixels: usize| float(pixels) / scale;
    let arc = Arc::new(
        place,
        (curvature, angle),
        (size(strip.width), size(strip.height)),
    )?;
    let bounds = arc.bounds();
    let (Some(width), Some(height)) = (
        index((bounds.width() * scale).ceil()),
        index((bounds.height() * scale).ceil()),
    ) else {
        return None;
    };
    let mut data = vec![0; width * height * 4];
    for row in 0..height {
        for column in 0..width {
            let pixel = Vec2::new(float(column) + 0.5, float(row) + 0.5);
            let at = bounds.min + pixel / scale;
            let (u, v) = arc.strip_point(at);
            let coverage = strip.at(u * scale, v * scale);
            if coverage <= 0.0 {
                continue;
            }
            let opacity = coverage * ink.map_or(1.0, |ink| ink.at(at));
            data[(row * width + column) * 4 + 3] = byte(opacity);
        }
    }
    let image = Image::new(
        Extent3d {
            width: u32::try_from(width).ok()?,
            height: u32::try_from(height).ok()?,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    Some((
        Rect::from_corners(
            bounds.min,
            bounds.min + Vec2::new(size(width), size(height)),
        ),
        image,
    ))
}

/// An opacity from 0 to 1 as a byte.
fn byte(value: f32) -> u8 {
    index((value.clamp(0.0, 1.0) * 255.0).round())
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_arc_maps_the_strip_and_back() {
        for (curvature, angle) in [(73, 43), (-71, 8), (107, -31), (-30, 25)] {
            let arc = Arc::new(Vec2::new(500.0, 300.0), (curvature, angle), (90.0, 22.0))
                .expect("a curvature");
            for (u, v) in [(0.0, 0.0), (45.0, 11.0), (89.0, 21.0)] {
                let (back_u, back_v) = arc.strip_point(arc.point(u, v));
                assert!((back_u - u).abs() < 1e-2, "{curvature}: {u} {back_u}");
                assert!((back_v - v).abs() < 1e-2, "{curvature}: {v} {back_v}");
            }
        }
    }

    #[test]
    fn the_arc_runs_through_its_place_at_the_angle_0() {
        let place = Vec2::new(500.0, 300.0);
        // Under its centre the text's top runs through the place, the text below it, and it
        // turns counter-clockwise as the angle grows: its middle moves right and up.
        let under = Arc::new(place, (64, 0), (100.0, 20.0)).expect("a curvature");
        assert!((under.point(50.0, 0.0) - place).length() < 1e-3);
        assert!(under.point(50.0, 20.0).y > place.y);
        let turned = Arc::new(place, (64, 10), (100.0, 20.0)).expect("a curvature");
        let middle = turned.point(50.0, 0.0);
        assert!(middle.x > place.x && middle.y < place.y);
        // Over its centre the text's bottom runs through it, and it turns clockwise: right and
        // down.
        let over = Arc::new(place, (-64, 0), (100.0, 20.0)).expect("a curvature");
        assert!((over.point(50.0, 20.0) - place).length() < 1e-3);
        assert!(over.point(50.0, 0.0).y < place.y);
        let turned = Arc::new(place, (-64, 10), (100.0, 20.0)).expect("a curvature");
        let middle = turned.point(50.0, 20.0);
        assert!(middle.x > place.x && middle.y > place.y);
        // 0.01 rad a pixel: the text's ends 0.5 rad either side of its middle.
        let start = under.point(0.0, 0.0) - under.centre;
        assert!((start.x.atan2(start.y) + 0.5).abs() < 1e-4);
        assert!(Arc::new(place, (0, 0), (100.0, 20.0)).is_none());
    }

    #[test]
    fn the_ink_spans_512_pixels_of_the_map_and_wraps() {
        // A texture of 2 x 2, its top left texel opaque: each texel 256 pixels of the map.
        let mut rgba = vec![0; 2 * 2 * 4];
        rgba[3] = 255;
        let ink = Ink::new(2, 2, &rgba).expect("a texture");
        assert!((ink.at(Vec2::new(128.0, 128.0)) - 1.0).abs() < 1e-6);
        assert!((ink.at(Vec2::new(128.0 + 1024.0, 128.0 + 512.0)) - 1.0).abs() < 1e-6);
        assert!(ink.at(Vec2::new(384.0, 128.0)).abs() < 1e-6);
        // Half way between the texels' middles.
        assert!((ink.at(Vec2::new(256.0, 128.0)) - 0.5).abs() < 1e-6);
    }
}
