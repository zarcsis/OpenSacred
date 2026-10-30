//! Images embedded in GRN files (docs/02 §4.2): raw RGBX and RGBA, and VTex, a one-frame Bink 1
//! video (`BIKi`, a complete `.bik` file). The game looks every texture up by name in
//! `texture.pak` first (`cGranny::bindTextures`, community-reported), and 244 of the 270 embedded
//! textures have an image of the same name there; the converter decodes only the other 26.
//!
//! FFmpeg decodes VTex: its Bink decoder runs as an external program ([`crate::ffmpeg`]), as for
//! the cutscenes (docs/03), so no Bink code enters the converter. FFmpeg
//! turns the YUV of the stream into RGB by BT.601 with the limited range, as grn2glb does; the
//! images match their `texture.pak` twins in layout and orientation (docs/02 §4.2).

use anyhow::{Context, Result, ensure};
use opensacred_formats::grn::{Texture, TextureFormat};

use crate::ffmpeg;
use crate::textures::encode_png;

/// PNG of an embedded texture; `None` for an external one, which has no pixels in the file.
pub fn to_png(texture: &Texture) -> Result<Option<Vec<u8>>> {
    let (width, height) = (texture.width, texture.height);
    let rgba = match texture.format {
        TextureFormat::External => return Ok(None),
        // Rows from the top, bytes R, G, B and A or padding, as the image twins in texture.pak
        // show.
        TextureFormat::Rgba => texture.payload.to_vec(),
        TextureFormat::Rgbx => {
            let mut rgba = texture.payload.to_vec();
            for pixel in rgba.as_chunks_mut::<4>().0 {
                pixel[3] = 255;
            }
            rgba
        }
        TextureFormat::VTex | TextureFormat::VTexAlpha => bink_frame(texture.payload)?,
    };
    let len = rgba_len(width, height)?;
    ensure!(
        rgba.len() == len,
        "{width}x{height} texture: {} bytes of pixels, expected {len}",
        rgba.len()
    );
    encode_png(width, height, &rgba).map(Some)
}

fn rgba_len(width: u32, height: u32) -> Result<usize> {
    usize::try_from(u64::from(width) * u64::from(height) * 4).context("texture size")
}

/// The first frame of the Bink file `data` as RGBA, decoded by FFmpeg.
fn bink_frame(data: &[u8]) -> Result<Vec<u8>> {
    let args = [
        "-f",
        "bink",
        "-i",
        "pipe:0",
        "-frames:v",
        "1",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "pipe:1",
    ];
    ffmpeg::pipe(&args, data, "a VTex texture")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::read_png_bytes;

    fn texture(format: TextureFormat, payload: &[u8]) -> Texture<'_> {
        Texture {
            name: b"t",
            width: 2,
            height: 1,
            format,
            payload,
        }
    }

    #[test]
    fn raw_textures_are_rgba_from_the_top() {
        let pixels = [1, 2, 3, 4, 5, 6, 7, 8];
        let png = to_png(&texture(TextureFormat::Rgba, &pixels))
            .unwrap()
            .unwrap();
        assert_eq!(read_png_bytes(&png), (2, 1, pixels.to_vec()));
        let png = to_png(&texture(TextureFormat::Rgbx, &pixels))
            .unwrap()
            .unwrap();
        assert_eq!(
            read_png_bytes(&png),
            (2, 1, vec![1, 2, 3, 255, 5, 6, 7, 255])
        );
        assert!(
            to_png(&texture(TextureFormat::External, b""))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn a_short_payload_is_an_error() {
        let err = to_png(&texture(TextureFormat::Rgba, &[0; 7])).unwrap_err();
        assert_eq!(
            err.to_string(),
            "2x1 texture: 7 bytes of pixels, expected 8"
        );
    }
}

/// Tests on the GOG installation files and FFmpeg, which CI does not have.
#[cfg(test)]
mod game_tests {
    use opensacred_formats::mdl::MdlPak;

    use super::*;
    use crate::testing::read_png_bytes;
    use crate::verify::read_game_file;

    #[test]
    #[ignore = "needs the game files in OriginalGame/ and FFmpeg"]
    fn every_embedded_texture_decodes() {
        let game = opensacred_locate::original();
        let mut decoded = std::collections::HashMap::new();
        let mut uniform = Vec::new();
        for path in ["PAK/models.pak", "PAK/models03.pak"] {
            let data = read_game_file(&game.join(path)).unwrap_or_else(|e| panic!("{e:#}"));
            let pak = MdlPak::parse(&data).unwrap_or_else(|e| panic!("{path}: {e}"));
            for index in 0..pak.len() {
                let record = pak
                    .record(index)
                    .unwrap_or_else(|e| panic!("{path} record {index}: {e}"));
                let what = format!("{path} record {index} `{}`", record.name().escape_ascii());
                let Some(grn) = record.parse_grn().unwrap_or_else(|e| panic!("{what}: {e}")) else {
                    continue;
                };
                let model = grn.model().unwrap_or_else(|e| panic!("{what}: {e}"));
                for texture in model.textures {
                    // The record, the texture and the error's whole chain, with FFmpeg's exit
                    // status and message (#447).
                    let png = to_png(&texture).unwrap_or_else(|e| {
                        panic!("{what}, texture `{}`: {e:#}", texture.name.escape_ascii())
                    });
                    let Some(png) = png else {
                        continue;
                    };
                    let (width, height, pixels) = read_png_bytes(&png);
                    assert_eq!((width, height), (texture.width, texture.height));
                    let first = &pixels[..4];
                    if pixels.chunks(4).all(|p| p == first) {
                        uniform.push((texture.format, texture.payload.len()));
                    }
                    *decoded.entry(format!("{:?}", texture.format)).or_insert(0) += 1;
                }
            }
        }
        let count = |format: &str| decoded.get(format).copied().unwrap_or(0);
        assert_eq!(
            [
                count("Rgbx"),
                count("Rgba"),
                count("VTex"),
                count("VTexAlpha")
            ],
            [6, 9, 126, 129]
        );
        // One-colour placeholders: every raw RGBA texture (weapons, which the game textures by
        // the item) and 97 VTex streams of at most 256 bytes, such as the eight of each special
        // attack motion, whose real images texture.pak holds by name.
        let raw = uniform
            .iter()
            .filter(|(f, _)| *f == TextureFormat::Rgba)
            .count();
        let small = uniform
            .iter()
            .filter(|&&(f, len)| f != TextureFormat::Rgba && len <= 256)
            .count();
        assert_eq!((uniform.len(), raw, small), (106, 9, 97), "{uniform:?}");
    }
}
