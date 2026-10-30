//! `textures` subcommand: TEX records of `PAK/texture*.pak` → RGBA8 PNG masters in
//! `<out>/textures/<pak>/<index>-<name>.png` (docs/02 §3.1).
//!
//! A file name starts with the record index: other containers (`tiles.pak`, `mixed.pak`) refer
//! to textures by index, and names are not unique — 25 names occur twice in `texture.pak`,
//! 6 of them with different pixels.
//!
//! The frames of three lavas, a tile of a menu and the blacksmith's apron are repaired as they
//! are written ([`STREAKED`], [`FAINT`], [`HOLED`]).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use opensacred_formats::tex::{Format, TexPak, Texture};
use rayon::prelude::*;

use crate::verify::read_game_file;

/// Texture containers and their output directories under `<out>/textures/`.
pub const PAKS: [(&str, &str); 2] = [
    ("PAK/texture.pak", "texture"),
    ("PAK/texture03.pak", "texture03"),
];

/// The liquids' series whose frames hold texels that are not opaque at the end of row 0 (and,
/// in `E_LAVA`, of row 2), the junk of the tool that made them, which the surfaces' blending
/// shows as grey and purple streaks (#347, docs/02 §5.2): `A_LAVA` (all 50 frames), `D_LAVA` (26)
/// and `E_LAVA` (50). Every other texel of the liquids' frames is opaque.
const STREAKED: [&str; 3] = ["a_lava", "d_lava", "e_lava"];

/// `GUI_UW_OPTION12.TGA`, a tile of the Underworld main menu, whose alpha over its body runs from
/// 195 to 255 where the other tiles' is 255, so that the menu's water shows through its rocks
/// (#352, docs/02 §3.1), and that lowest alpha of its body: its alpha is scaled to make it 255.
const FAINT: (&str, u8) = ("gui_uw_option12", 195);

/// `BLACKSMITH_BELT.TGA` (record 8202), the blacksmith's apron, whose alpha is 0 over two strokes
/// across it, 1,176 texels, 406 of them under 16 triangles of the flaps below the belt, while its
/// colour runs on unbroken beneath them: the hole in the blacksmith's model that the community
/// reports (#401, docs/02 §4.6). Every other texel is opaque; it is made opaque whole.
const HOLED: &str = "blacksmith_belt";

/// Records of one container by format.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub argb4444: usize,
    pub bgra8888: usize,
    pub empty: usize,
    /// Textures repaired ([`STREAKED`], [`FAINT`], [`HOLED`]), and their texels.
    pub repaired_textures: usize,
    pub repaired_texels: usize,
}

/// Directory of the PNGs converted from container `dir_name` (see [`PAKS`]).
pub fn output_dir(out: &Path, dir_name: &str) -> PathBuf {
    out.join("textures").join(dir_name)
}

/// Converts every record of the container `src` into a PNG in `dst_dir`, in parallel.
pub fn convert_pak(src: &Path, dst_dir: &Path) -> Result<Summary> {
    let data = read_game_file(src)?;
    let pak = TexPak::parse(&data).with_context(|| src.display().to_string())?;
    fs::create_dir_all(dst_dir).with_context(|| format!("creating {}", dst_dir.display()))?;

    let formats = (0..pak.len())
        .into_par_iter()
        .map(|index| {
            convert_record(&pak, index, dst_dir).with_context(|| src.display().to_string())
        })
        .collect::<Result<Vec<_>>>()?;

    let mut summary = Summary::default();
    for converted in formats {
        let Some((format, repaired)) = converted else {
            summary.empty += 1;
            continue;
        };
        match format {
            Format::Argb4444Zlib => summary.argb4444 += 1,
            Format::Bgra8888 => summary.bgra8888 += 1,
        }
        if repaired > 0 {
            summary.repaired_textures += 1;
            summary.repaired_texels += repaired;
        }
    }
    Ok(summary)
}

/// Converts record `index` of `pak` into a PNG in `dst_dir`; its format and the number of texels
/// repaired, or `None` for an empty directory entry.
pub fn convert_record(
    pak: &TexPak,
    index: usize,
    dst_dir: &Path,
) -> Result<Option<(Format, usize)>> {
    let Some(record) = pak.record(index)? else {
        return Ok(None);
    };
    let name = record.header.name();
    let mut texture = record
        .decode()
        .with_context(|| format!("`{}`", name.escape_ascii()))?;
    let repaired = if streaked(name) {
        fill_from_columns(&mut texture)
            .with_context(|| format!("repairing `{}`", name.escape_ascii()))?
    } else if stem(name, b".tga") == FAINT.0 {
        strengthen(&mut texture, FAINT.1)
    } else if stem(name, b".tga") == HOLED {
        make_opaque(&mut texture)
    } else {
        0
    };
    let path = dst_dir.join(file_name(index, name));
    write_png(&path, &texture).with_context(|| format!("writing {}", path.display()))?;
    Ok(Some((record.header.format, repaired)))
}

/// Whether `name` is a frame of a [`STREAKED`] series: the series, two digits and `.tga`.
fn streaked(name: &[u8]) -> bool {
    let stem = stem(name, b".tga");
    STREAKED.iter().any(|series| {
        stem.strip_prefix(series)
            .is_some_and(|n| n.len() == 2 && n.bytes().all(|b| b.is_ascii_digit()))
    })
}

/// Scales the alpha of every texel of `texture` by 255 / `lowest`, held at 255, so that alphas
/// from `lowest` up become opaque and the ones below keep their ramp; the number changed.
fn strengthen(texture: &mut Texture, lowest: u8) -> usize {
    let mut changed = 0;
    for alpha in texture.rgba.iter_mut().skip(3).step_by(4) {
        let scaled = (u32::from(*alpha) * 255 + u32::from(lowest) / 2) / u32::from(lowest);
        let scaled = u8::try_from(scaled.min(255)).unwrap_or(u8::MAX);
        if scaled != *alpha {
            *alpha = scaled;
            changed += 1;
        }
    }
    changed
}

/// Makes every texel of `texture` opaque, its colour kept; the number changed.
fn make_opaque(texture: &mut Texture) -> usize {
    let mut changed = 0;
    for alpha in texture.rgba.iter_mut().skip(3).step_by(4) {
        if *alpha != 255 {
            *alpha = 255;
            changed += 1;
        }
    }
    changed
}

/// Gives every texel of `texture` that is not opaque the colour of the nearest opaque texel of
/// its column, the one above on a tie, so that the frame is opaque; the number of texels
/// replaced. A column with no opaque texel is an error.
fn fill_from_columns(texture: &mut Texture) -> Result<usize> {
    let width = usize::try_from(texture.width)?;
    let height = usize::try_from(texture.height)?;
    let rgba = &mut texture.rgba;
    let opaque = |rgba: &[u8], x: usize, y: usize| rgba[(y * width + x) * 4 + 3] == 255;
    let mut replaced = 0;
    for x in 0..width {
        let rows: Vec<usize> = (0..height).filter(|&y| opaque(rgba, x, y)).collect();
        for y in 0..height {
            if opaque(rgba, x, y) {
                continue;
            }
            let source = rows
                .iter()
                .copied()
                .min_by_key(|&r| (r.abs_diff(y), r))
                .with_context(|| format!("column {x} has no opaque texel"))?;
            let (to, from) = ((y * width + x) * 4, (source * width + x) * 4);
            rgba.copy_within(from..from + 4, to);
            replaced += 1;
        }
    }
    Ok(replaced)
}

/// `06868-dryade_scout_arms.png`: the five-digit record index, then the [`stem`] of the name
/// without `.tga`.
pub fn file_name(index: usize, name: &[u8]) -> String {
    format!("{index:05}-{}.png", stem(name, b".tga"))
}

/// `name` in lower case without `suffix`; bytes outside `[a-z0-9._-]` (cp1252 letters such as
/// `ß`) become `_`.
pub fn stem(name: &[u8], suffix: &[u8]) -> String {
    let name = name.to_ascii_lowercase();
    let stem = name.strip_suffix(suffix).unwrap_or(&name);
    stem.iter()
        .map(|&b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-') {
                char::from(b)
            } else {
                '_'
            }
        })
        .collect()
}

/// Encodes in memory and writes the file in one call, so that a write error is not lost in a
/// buffered writer's `drop`.
fn write_png(path: &Path, texture: &Texture) -> Result<()> {
    fs::write(
        path,
        encode_png(texture.width, texture.height, &texture.rgba)?,
    )?;
    Ok(())
}

/// 8-bit RGBA PNG of `rgba`, rows from the top.
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgba)?;
    writer.finish()?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use miniz_oxide::deflate::compress_to_vec_zlib;

    use super::*;
    use crate::testing::{Tex, scratch_dir, tex_pak};

    fn read_png(path: &Path) -> (u32, u32, Vec<u8>) {
        let file = fs::File::open(path).unwrap();
        let mut reader = png::Decoder::new(std::io::BufReader::new(file))
            .read_info()
            .unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!(
            (info.color_type, info.bit_depth),
            (png::ColorType::Rgba, png::BitDepth::Eight)
        );
        pixels.truncate(info.buffer_size());
        (info.width, info.height, pixels)
    }

    #[test]
    fn file_name_is_index_and_sanitized_lower_case_name() {
        assert_eq!(
            file_name(6868, b"DRYADE_SCOUT_ARMS.TGA"),
            "06868-dryade_scout_arms.png"
        );
        assert_eq!(file_name(25_534, b"MIX9999.444"), "25534-mix9999.444.png");
        // Truncated 32-byte name and cp1252 letters.
        assert_eq!(
            file_name(6561, b"AMAZONE_ARMOUR_KURZKHEMD_CELAL.T"),
            "06561-amazone_armour_kurzkhemd_celal.t.png"
        );
        assert_eq!(
            file_name(1, b"GR\xdcN SCH\xdfN.tga"),
            "00001-gr_n_sch_n.png"
        );
        assert_eq!(file_name(2, b"..\\a/b:c"), "00002-.._a_b_c.png");
    }

    #[test]
    fn converts_both_formats_to_rgba_png() {
        let dir = scratch_dir("textures");
        let argb = compress_to_vec_zlib(&[0x4C, 0xF8, 0x34, 0x12], 6);
        let src = dir.join("texture.pak");
        fs::write(
            &src,
            tex_pak(&[
                None,
                Some(Tex {
                    name: b"A.TGA",
                    width: 2,
                    height: 1,
                    format: 4,
                    data: argb,
                }),
                Some(Tex {
                    name: b"Ui.tga",
                    width: 1,
                    height: 1,
                    format: 6,
                    data: vec![1, 2, 3, 4],
                }),
            ]),
        )
        .unwrap();
        let dst = output_dir(&dir.join("assets"), "texture");

        let summary = convert_pak(&src, &dst).unwrap();

        assert_eq!(
            summary,
            Summary {
                argb4444: 1,
                bgra8888: 1,
                empty: 1,
                ..Summary::default()
            }
        );
        let mut files: Vec<_> = fs::read_dir(&dst)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        files.sort();
        assert_eq!(files, ["00001-a.png", "00002-ui.png"]);
        assert_eq!(
            read_png(&dst.join("00001-a.png")),
            (2, 1, vec![136, 68, 204, 255, 34, 51, 68, 17])
        );
        assert_eq!(
            read_png(&dst.join("00002-ui.png")),
            (1, 1, vec![3, 2, 1, 4])
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_streaked_lavas_take_their_columns_opaque_colours() {
        let dir = scratch_dir("textures-lava");
        // One column of four BGRA texels, top down: junk, red, junk, blue; the second junk texel
        // is as near the red as the blue and takes the red, above it.
        let column = vec![9, 9, 9, 0, 0, 0, 255, 255, 1, 1, 1, 17, 255, 0, 0, 255];
        let tex = |name| {
            Some(Tex {
                name,
                width: 1,
                height: 4,
                format: 6,
                data: column.clone(),
            })
        };
        let src = dir.join("texture.pak");
        fs::write(&src, tex_pak(&[tex(b"E_LAVA07.TGA"), tex(b"B_LAVA07.TGA")])).unwrap();
        let dst = output_dir(&dir.join("assets"), "texture");

        let summary = convert_pak(&src, &dst).unwrap();

        assert_eq!((summary.repaired_textures, summary.repaired_texels), (1, 2));
        let (red, blue) = ([255, 0, 0, 255], [0, 0, 255, 255]);
        assert_eq!(
            read_png(&dst.join("00000-e_lava07.png")).2,
            [red, red, red, blue].concat()
        );
        // Another series is written as it is.
        assert_eq!(
            read_png(&dst.join("00001-b_lava07.png")).2,
            [[9, 9, 9, 0], red, [1, 1, 1, 17], blue].concat()
        );
        assert!(streaked(b"a_lava49.tga") && streaked(b"D_LAVA00.TGA"));
        assert!(
            !streaked(b"e_lava.tga") && !streaked(b"e_lava123.tga") && !streaked(b"c_lava00.tga")
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_faint_menu_tile_is_made_opaque() {
        let mut texture = Texture {
            width: 4,
            height: 1,
            rgba: [[9, 9, 9, 0], [9, 9, 9, 100], [9, 9, 9, 195], [9, 9, 9, 230]].concat(),
        };
        assert_eq!(strengthen(&mut texture, 195), 3);
        let alphas: Vec<u8> = texture.rgba.chunks(4).map(|t| t[3]).collect();
        assert_eq!(alphas, [0, 131, 255, 255]);
        assert!(texture.rgba.chunks(4).all(|t| t[..3] == [9, 9, 9]));
    }

    #[test]
    fn the_blacksmiths_apron_is_made_opaque() {
        let dir = scratch_dir("textures-apron");
        // Two BGRA texels, a transparent one over a colour and an opaque one.
        let tex = |name| {
            Some(Tex {
                name,
                width: 2,
                height: 1,
                format: 6,
                data: vec![10, 20, 30, 0, 40, 50, 60, 255],
            })
        };
        let src = dir.join("texture.pak");
        fs::write(
            &src,
            tex_pak(&[tex(b"BLACKSMITH_BELT.TGA"), tex(b"WARRIOR_BELT.TGA")]),
        )
        .unwrap();
        let dst = output_dir(&dir.join("assets"), "texture");

        let summary = convert_pak(&src, &dst).unwrap();

        assert_eq!((summary.repaired_textures, summary.repaired_texels), (1, 1));
        assert_eq!(
            read_png(&dst.join("00000-blacksmith_belt.png")).2,
            [30, 20, 10, 255, 60, 50, 40, 255]
        );
        // Another texture keeps its alpha.
        assert_eq!(
            read_png(&dst.join("00001-warrior_belt.png")).2,
            [30, 20, 10, 0, 60, 50, 40, 255]
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn the_lavas_frames_come_out_opaque() {
        let game = opensacred_locate::original();
        let dir = scratch_dir("textures-lavas");
        let dst = output_dir(&dir, "texture");
        let summary = convert_pak(&game.join(PAKS[0].0), &dst).unwrap();
        // docs/02 §5.2: all 50 frames of A_LAVA, 26 of D_LAVA and all 50 of E_LAVA; §3.1, the
        // menu's tile, its body and its edge; and §4.6, the blacksmith's apron.
        assert_eq!(summary.repaired_textures, 128);
        let (_, _, tile) = read_png(&dst.join("00347-gui_uw_option12.png"));
        let alphas: Vec<u8> = tile.chunks(4).map(|t| t[3]).collect();
        // 39,076 texels of alphas from 195 opaque, 14 of them opaque already; 47,822 of alphas
        // from 2 to 254 changed.
        assert_eq!(alphas.iter().filter(|&&a| a == 255).count(), 39_076);
        assert_eq!(summary.repaired_texels, 600 + 141 + 1394 + 47_822 + 1176);
        let (_, _, apron) = read_png(&dst.join("08202-blacksmith_belt.png"));
        assert!(apron.chunks(4).all(|t| t[3] == 255));
        let mut frames = 0;
        for entry in fs::read_dir(&dst).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            let stem = name.split_once('-').unwrap().1;
            if STREAKED.iter().any(|s| stem.starts_with(s)) {
                let (_, _, rgba) = read_png(&dst.join(&name));
                assert!(rgba.chunks(4).all(|t| t[3] == 255), "{name}");
                frames += 1;
            }
        }
        assert_eq!(frames, 150);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn error_names_file_and_record() {
        let dir = scratch_dir("textures-error");
        let src = dir.join("texture.pak");
        fs::write(
            &src,
            tex_pak(&[
                None,
                Some(Tex {
                    name: b"BAD.TGA",
                    width: 1,
                    height: 1,
                    format: 4,
                    data: b"junk".to_vec(),
                }),
            ]),
        )
        .unwrap();

        let err = convert_pak(&src, &dir.join("out")).unwrap_err();

        let message = format!("{err:#}");
        assert!(message.starts_with(&src.display().to_string()), "{message}");
        assert!(
            message.contains("`BAD.TGA`: record 1, offset 0x"),
            "{message}"
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
