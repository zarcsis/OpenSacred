//! A save's picture (#1991; docs/re/saves.md, "The picture"): the frame as the game is saved,
//! 128 × 96 at four bits a channel, as the original's `cEngine_captureInternal` (`0x613670`)
//! makes it of its 1024 × 768 back buffer, which the saves' window shows in its rows.
//!
//! The original takes it as the menu over a game opens (its `(0x13, 5, 1)`, `0x6BB6C0`), right
//! before a quick save and before an autosave, and a save keeps the last taken. The game takes
//! the window's frame ([`take`]), its middle of the original's screen's shape where the window is
//! wider or taller (docs/decisions/interface.md, "Save's picture of the screen's middle"), and
//! samples it as the original does ([`Picture::sample`]); the frame arrives a frame or two later,
//! counted in [`Taken`], which a save that waits for its picture asks.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use serde::{Deserialize, Serialize};

/// The picture's size.
pub const WIDTH: u32 = 128;
pub const HEIGHT: u32 = 96;
/// The original's screen, whose shape the picture takes of a window's frame.
const SCREEN: (u32, u32) = (1024, 768);
/// The step of the samples on the original's screen (`0x660AD0`).
const STEP: i64 = 8;

/// A save's picture: [`WIDTH`] × [`HEIGHT`] pixels from the top, each a little-endian ARGB4444
/// word of alpha 15, as the original's save keeps them (its part `0x93`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Picture(pub Vec<u8>);

impl Picture {
    /// The picture of a frame `width` × `height` of RGBA8 `rgba`, from the top: of its middle of
    /// the original's screen's shape, each pixel (x, y) the mean of each channel's top four bits
    /// at the screen's (8x − 8, 8y − 8), (8x, 8y − 8), (8x − 8, 8y) and (8x, 8y), those left or
    /// above the screen left out (`0x660AD0` with its filter, a sparse sample rather than a box).
    pub fn sample(rgba: &[u8], width: u32, height: u32) -> Option<Self> {
        let (w, h) = (u64::from(width), u64::from(height));
        if w == 0 || h == 0 || rgba.len() < usize::try_from(w * h * 4).ok()? {
            return None;
        }
        // The screen's shape in the frame, centred.
        let (sw, sh) = (u64::from(SCREEN.0), u64::from(SCREEN.1));
        let (cw, ch) = if w * sh > h * sw {
            (h * sw / sh, h)
        } else {
            (w, w * sh / sw)
        };
        let (x0, y0) = ((w - cw) / 2, (h - ch) / 2);
        let texel = |sx: i64, sy: i64| -> Option<[u8; 3]> {
            let sx = u64::try_from(sx).ok()?;
            let sy = u64::try_from(sy).ok()?;
            let px = (x0 + sx * cw / sw).min(w - 1);
            let py = (y0 + sy * ch / sh).min(h - 1);
            let at = usize::try_from((py * w + px) * 4).ok()?;
            let pixel = rgba.get(at..at + 3)?;
            Some([pixel[0] >> 4, pixel[1] >> 4, pixel[2] >> 4])
        };
        let mut bytes = Vec::with_capacity(usize::try_from(WIDTH * HEIGHT * 2).ok()?);
        for y in 0..i64::from(HEIGHT) {
            for x in 0..i64::from(WIDTH) {
                let samples: Vec<[u8; 3]> = [
                    (STEP * x - STEP, STEP * y - STEP),
                    (STEP * x, STEP * y - STEP),
                    (STEP * x - STEP, STEP * y),
                    (STEP * x, STEP * y),
                ]
                .into_iter()
                .filter_map(|(sx, sy)| texel(sx, sy))
                .collect();
                let count = u16::try_from(samples.len()).unwrap_or(1).max(1);
                let mean = |channel: usize| {
                    samples.iter().map(|s| u16::from(s[channel])).sum::<u16>() / count
                };
                let word = 0xF000 | mean(0) << 8 | mean(1) << 4 | mean(2);
                bytes.extend(word.to_le_bytes());
            }
        }
        Some(Self(bytes))
    }

    /// Its pixels as RGBA8, each channel's four bits made eight, from the top.
    pub fn rgba(&self) -> Vec<u8> {
        self.0
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&pair| {
                let word = u16::from_le_bytes(pair);
                let four = |shift: u16| u8::try_from((word >> shift) & 0xF).unwrap_or(0) * 17;
                [four(8), four(4), four(0), four(12)]
            })
            .collect()
    }
}

/// The last picture taken, and how many have been taken, which a save waiting for its picture
/// counts on.
#[derive(Resource, Default)]
pub struct Taken {
    pub picture: Option<Picture>,
    pub count: u32,
}

/// Takes the window's next frame as the save's picture ([`Taken`]).
pub fn take(commands: &mut Commands) {
    commands.spawn(Screenshot::primary_window()).observe(taken);
}

/// Takes the window's next frame as the save's picture, from a system of the whole world.
pub fn take_in(world: &mut World) {
    world.spawn(Screenshot::primary_window()).observe(taken);
}

/// The frame captured, sampled into [`Taken`].
fn taken(captured: On<ScreenshotCaptured>, mut taken: ResMut<Taken>) {
    let picture = captured
        .image
        .clone()
        .try_into_dynamic()
        .ok()
        .and_then(|frame| {
            let frame = frame.to_rgba8();
            Picture::sample(frame.as_raw(), frame.width(), frame.height())
        });
    if picture.is_none() {
        warn!("save picture: the frame could not be read");
    }
    taken.picture = picture;
    taken.count += 1;
}

/// The saves' pictures.
pub struct SavePicturePlugin;

impl Plugin for SavePicturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Taken>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame of `width` × `height` whose pixel (x, y) is `paint(x, y)`.
    fn frame(width: u32, height: u32, paint: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
        let mut rgba = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let [r, g, b] = paint(x, y);
                rgba.extend([r, g, b, 255]);
            }
        }
        rgba
    }

    fn word(picture: &Picture, x: usize, y: usize) -> u16 {
        let at = (y * 128 + x) * 2;
        u16::from_le_bytes([picture.0[at], picture.0[at + 1]])
    }

    #[test]
    fn a_pixel_is_the_mean_of_four_sparse_samples_of_four_bits() {
        // The red channel the screen's x, the green its y, the blue 255: the corner keeps its one
        // sample, a middle pixel the mean of its four.
        let rgba = frame(1024, 768, |x, y| {
            [
                u8::try_from(x % 256).unwrap(),
                u8::try_from(y % 256).unwrap(),
                255,
            ]
        });
        let picture = Picture::sample(&rgba, 1024, 768).unwrap();
        assert_eq!(picture.0.len(), 128 * 96 * 2);
        assert_eq!(word(&picture, 0, 0), 0xF00F);
        // (8, 8): samples at x 0 and 8, y 0 and 8, every top four bits 0.
        assert_eq!(word(&picture, 1, 1), 0xF00F);
        // (20, 10): x 152 and 160 (9 and 10), y 72 and 80 (4 and 5): means 9 and 4.
        assert_eq!(word(&picture, 20, 10), 0xF94F);
    }

    #[test]
    fn a_wide_frame_gives_its_middle_of_the_screens_shape() {
        // A 1920 × 1080 frame: its middle 1440 wide is the screen, the sides apart.
        let rgba = frame(1920, 1080, |x, _| {
            if (240..1680).contains(&x) {
                [255, 255, 255]
            } else {
                [0, 0, 0]
            }
        });
        let picture = Picture::sample(&rgba, 1920, 1080).unwrap();
        for x in [0, 64, 127] {
            assert_eq!(word(&picture, x, 48), 0xFFFF, "{x}");
        }
        assert!(Picture::sample(&rgba, 1920, 1081).is_none());
    }
}
