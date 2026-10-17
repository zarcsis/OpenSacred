//! `textures` subcommand: TEX records of `PAK/texture*.pak` → RGBA8 PNG masters in
//! `<out>/textures/<pak>/<index>-<name>.png` (docs/02 §3.1).
//!
//! A file name starts with the record index: other containers (`tiles.pak`, `mixed.pak`) refer
//! to textures by index, and names are not unique — 25 names occur twice in `texture.pak`,
//! 6 of them with different pixels.

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

/// Records of one container by format.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub argb4444: usize,
    pub bgra8888: usize,
    pub empty: usize,
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
    for format in formats {
        match format {
            None => summary.empty += 1,
            Some(Format::Argb4444Zlib) => summary.argb4444 += 1,
            Some(Format::Bgra8888) => summary.bgra8888 += 1,
        }
    }
    Ok(summary)
}

/// Converts record `index` of `pak` into a PNG in `dst_dir`; its format, or `None` for an empty
/// directory entry.
pub fn convert_record(pak: &TexPak, index: usize, dst_dir: &Path) -> Result<Option<Format>> {
    let Some(record) = pak.record(index)? else {
        return Ok(None);
    };
    let name = record.header.name();
    let texture = record
        .decode()
        .with_context(|| format!("`{}`", name.escape_ascii()))?;
    let path = dst_dir.join(file_name(index, name));
    write_png(&path, &texture).with_context(|| format!("writing {}", path.display()))?;
    Ok(Some(record.header.format))
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
                empty: 1
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
