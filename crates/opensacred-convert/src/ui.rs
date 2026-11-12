//! `ui` subcommand: the loading and save screens of `PAK/*.bmp` → PNG, the main menu's picture
//! assembled from its twelve tiles of `texture.pak` (#1824), the fonts of `font/` and the
//! credits → `<out>/ui/` (docs/02 §11), the credits as UTF-8 with LF line ends (#835).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use opensacred_formats::bmp;
use opensacred_formats::tex::{TexPak, Texture};

use crate::textures::encode_png;
use crate::verify::read_game_file;

/// The screens, fonts and texts, as the manifest names them.
pub const SCREENS: [&str; 6] = [
    "PAK/loading0.bmp",
    "PAK/loading1.bmp",
    "PAK/LoadingUW00.bmp",
    "PAK/LoadingUW01.bmp",
    "PAK/loadgame.bmp",
    "PAK/savegame.bmp",
];
pub const FONTS: [&str; 2] = ["font/ANTQS__.TTF", "font/CAROLING.TTF"];

/// The menus' backgrounds, each twelve tiles of `texture.pak`, 256 texels a side, four to a row
/// from the top left (docs/02 §3.1), and the one picture of 1024 × 768 that `ui` assembles from
/// each, the faint twelfth of the main menu's repaired first (`textures::repair`, #352), so that
/// the game draws one image and a larger screen samples it across the tiles' joins: the main
/// menu's (#1824), the hero's choice's, the options' window's and the saves' window's (#2634).
/// Each is its tiles' names but for their numbers, 1 to 12 after them, and its picture's file.
pub const MENU_TILES: &str = "PAK/texture.pak";
pub const MENU_PICTURES: [(&str, &str); 4] = [
    ("gui_uw_option", MENU_PICTURE),
    ("gui_uw_char_", "choice.png"),
    ("uw_option_1_", "options.png"),
    ("uw_option_", "saves.png"),
];
const MENU_SIDE: u32 = 256;
const MENU_COLUMNS: u32 = 4;
const MENU_COUNT: usize = 12;
pub const MENU_PICTURE: &str = "mainmenu.png";
pub const CREDITS: [&str; 2] = ["credits.txt", "credits2.txt"];

pub fn output_dir(out: &Path) -> PathBuf {
    out.join("ui")
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub screens: usize,
    pub fonts: usize,
    pub texts: usize,
}

/// The file name of `path` in lower case, its extension replaced by `extension` if given.
fn target(path: &str, extension: Option<&str>) -> String {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    match extension {
        Some(extension) => {
            let stem = name.rsplit_once('.').map_or(name.as_str(), |(s, _)| s);
            format!("{stem}.{extension}")
        }
        None => name,
    }
}

pub fn convert(game_dir: &Path, out: &Path) -> Result<Summary> {
    let dir = output_dir(out);
    for sub in ["screens", "fonts", "texts"] {
        fs::create_dir_all(dir.join(sub)).with_context(|| format!("creating {sub}"))?;
    }
    let mut summary = Summary::default();
    for path in SCREENS {
        let data = read_game_file(&game_dir.join(path))?;
        let bitmap = bmp::decode(&data).context(path)?;
        let png = encode_png(bitmap.width, bitmap.height, &bitmap.rgba)?;
        let file = dir.join("screens").join(target(path, Some("png")));
        fs::write(&file, png).with_context(|| format!("writing {}", file.display()))?;
        summary.screens += 1;
    }
    let names = MENU_PICTURES.iter().map(|&(_, name)| name);
    for (name, (width, height, rgba)) in names.zip(menu_pictures(game_dir)?) {
        let file = dir.join("screens").join(name);
        fs::write(&file, encode_png(width, height, &rgba)?)
            .with_context(|| format!("writing {}", file.display()))?;
        summary.screens += 1;
    }
    for (paths, sub, count, text) in [
        (&FONTS[..], "fonts", &mut summary.fonts, false),
        (&CREDITS[..], "texts", &mut summary.texts, true),
    ] {
        for path in paths {
            let mut data = read_game_file(&game_dir.join(path))?;
            if text {
                data = crate::utf8::normalized(&data).context(path)?;
            }
            let file = dir.join(sub).join(target(path, None));
            fs::write(&file, data).with_context(|| format!("writing {}", file.display()))?;
            *count += 1;
        }
    }
    Ok(summary)
}

/// The number of a tile of the picture whose tiles' names are `prefix` and a number, 1 to 12;
/// none for another name (`uw_option_` takes `uw_option_07`, not `uw_option_1_07`).
fn tile_number(stem: &str, prefix: &str) -> Option<usize> {
    stem.strip_prefix(prefix)
        .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|n| n.parse::<usize>().ok())
        .filter(|n| (1..=MENU_COUNT).contains(n))
}

/// Whether the master at `path`, relative to the masters' directory, is a tile of one of the
/// menus' pictures, which `ui` assembles from it and `build` therefore leaves out of the game's
/// assets (#2646): a texture of [`MENU_TILES`] (`textures/texture/<number>-<name>.png`) named as
/// one of [`MENU_PICTURES`]' tiles.
pub fn is_menu_tile(path: &str) -> bool {
    let Some(&(_, dir)) = crate::textures::PAKS
        .iter()
        .find(|&&(pak, _)| pak == MENU_TILES)
    else {
        return false;
    };
    path.strip_prefix("textures/")
        .and_then(|rest| rest.strip_prefix(dir))
        .and_then(|rest| rest.strip_prefix('/'))
        .and_then(|name| name.strip_suffix(".png"))
        .and_then(|name| name.split_once('-'))
        .is_some_and(|(_, stem)| {
            MENU_PICTURES
                .iter()
                .any(|&(prefix, _)| tile_number(stem, prefix).is_some())
        })
}

/// The menus' pictures ([`MENU_PICTURES`], [`MENU_TILES`]) in their order: each its width,
/// height and RGBA texels, each tile at its place, its alpha kept, which leaves the menu's water
/// showing below them.
fn menu_pictures(game_dir: &Path) -> Result<Vec<(u32, u32, Vec<u8>)>> {
    let data = read_game_file(&game_dir.join(MENU_TILES))?;
    let pak = TexPak::parse(&data).context(MENU_TILES)?;
    let mut tiles: Vec<Vec<Option<Texture>>> = vec![vec![None; MENU_COUNT]; MENU_PICTURES.len()];
    for index in 0..pak.len() {
        let Some(record) = pak.record(index)? else {
            continue;
        };
        let name = record.header.name();
        let stem = crate::textures::stem(name, b".tga");
        let Some((picture, n)) = MENU_PICTURES
            .iter()
            .enumerate()
            .find_map(|(picture, &(prefix, _))| Some((picture, tile_number(&stem, prefix)?)))
        else {
            continue;
        };
        let mut texture = record
            .decode()
            .with_context(|| format!("`{}`", name.escape_ascii()))?;
        crate::textures::repair(name, &mut texture)?;
        ensure!(
            texture.width == MENU_SIDE && texture.height == MENU_SIDE,
            "`{stem}` is {} × {}, not {MENU_SIDE} a side",
            texture.width,
            texture.height
        );
        if let Some(slot) = tiles[picture].get_mut(n - 1) {
            *slot = Some(texture);
        }
    }
    let rows = u32::try_from(MENU_COUNT).unwrap_or(0) / MENU_COLUMNS;
    let (width, height) = (MENU_SIDE * MENU_COLUMNS, MENU_SIDE * rows);
    let (side, columns) = (usize::try_from(MENU_SIDE)?, usize::try_from(MENU_COLUMNS)?);
    let (row_bytes, tile_bytes) = (side * columns * 4, side * 4);
    MENU_PICTURES
        .iter()
        .zip(tiles)
        .map(|(&(prefix, _), tiles)| {
            let mut rgba = vec![0; row_bytes * usize::try_from(height)?];
            for (n, tile) in tiles.iter().enumerate() {
                let tile = tile
                    .as_ref()
                    .with_context(|| format!("`{prefix}{}` missing from {MENU_TILES}", n + 1))?;
                let (column, row) = (n % columns, n / columns);
                for (y, line) in tile.rgba.chunks_exact(tile_bytes).enumerate() {
                    let at = (row * side + y) * row_bytes + column * tile_bytes;
                    rgba[at..at + tile_bytes].copy_from_slice(line);
                }
            }
            Ok((width, height, rgba))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffmpeg;
    use crate::testing::{read_png_bytes, scratch_dir};

    #[test]
    fn a_tiles_number_is_its_pictures_alone() {
        assert_eq!(tile_number("gui_uw_option12", "gui_uw_option"), Some(12));
        assert_eq!(tile_number("gui_uw_char_07", "gui_uw_char_"), Some(7));
        // The saves' tiles and the options' share a beginning.
        assert_eq!(tile_number("uw_option_07", "uw_option_"), Some(7));
        assert_eq!(tile_number("uw_option_1_07", "uw_option_"), None);
        assert_eq!(tile_number("uw_option_1_07", "uw_option_1_"), Some(7));
        assert_eq!(tile_number("uw_option_13", "uw_option_"), None);
        assert_eq!(tile_number("uw_option_", "uw_option_"), None);
    }

    #[test]
    fn the_menus_tiles_are_textures_of_their_pak_named_as_tiles() {
        for path in [
            "textures/texture/00344-gui_uw_option1.png",
            "textures/texture/00347-gui_uw_option12.png",
            "textures/texture/00317-gui_uw_char_01.png",
            "textures/texture/00398-uw_option_1_01.png",
            "textures/texture/00397-uw_option_12.png",
        ] {
            assert!(is_menu_tile(path), "{path}");
        }
        for path in [
            // Another texture of the menus, a tile's number out of range, another pak, not a
            // texture's master.
            "textures/texture/00001-ui_uw_main_center.png",
            "textures/texture/00001-gui_uw_option13.png",
            "textures/texture03/00001-gui_uw_option1.png",
            "ui/screens/mainmenu.png",
            "textures/texture/gui_uw_option1.png",
        ] {
            assert!(!is_menu_tile(path), "{path}");
        }
    }

    #[test]
    fn targets_are_lower_case_names() {
        assert_eq!(
            target("PAK/LoadingUW00.bmp", Some("png")),
            "loadinguw00.png"
        );
        assert_eq!(target("font/ANTQS__.TTF", None), "antqs__.ttf");
    }

    /// Every screen decodes to the pixels FFmpeg decodes it to, and the credits keep their text
    /// with LF line ends.
    #[test]
    #[ignore = "needs the game files in OriginalGame/ and FFmpeg"]
    fn screens_match_ffmpeg() {
        let game = opensacred_locate::original();
        let out = scratch_dir("ui");
        let summary = convert(&game, &out).unwrap();
        assert_eq!((summary.screens, summary.fonts, summary.texts), (10, 2, 2));
        // The menus' pictures: each tile as `textures` writes it, at its place (#1824, #2634).
        let tiles = scratch_dir("ui-tiles");
        crate::textures::convert_pak(&game.join(MENU_TILES), &tiles).unwrap();
        for (prefix, file) in MENU_PICTURES {
            let png = fs::read(output_dir(&out).join("screens").join(file)).unwrap();
            let (width, height, picture) = read_png_bytes(&png);
            assert_eq!((width, height), (1024, 768), "{file}");
            let mut found = 0;
            for entry in fs::read_dir(&tiles).unwrap() {
                let path = entry.unwrap().path();
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                let Some(n) = name
                    .split_once('-')
                    .and_then(|(_, rest)| rest.strip_suffix(".png"))
                    .and_then(|stem| tile_number(stem, prefix))
                else {
                    continue;
                };
                let (w, h, tile) = read_png_bytes(&fs::read(&path).unwrap());
                assert_eq!((w, h), (256, 256), "{name}");
                let (column, row) = ((n - 1) % 4, (n - 1) / 4);
                for y in 0..256 {
                    let at = ((row * 256 + y) * 1024 + column * 256) * 4;
                    assert!(
                        picture[at..at + 1024] == tile[y * 1024..(y + 1) * 1024],
                        "{file}: {name}, row {y}"
                    );
                }
                found += 1;
            }
            assert_eq!(found, 12, "{file}");
        }
        // `build` leaves out the 48 tiles and no other texture of the pak (#2646).
        let left: Vec<String> = fs::read_dir(&tiles)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| is_menu_tile(&format!("textures/texture/{name}")))
            .collect();
        assert_eq!(left.len(), MENU_PICTURES.len() * MENU_COUNT, "{left:?}");
        for path in SCREENS {
            let data = read_game_file(&game.join(path)).unwrap();
            let args = [
                "-f", "bmp_pipe", "-i", "pipe:0", "-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1",
            ];
            let reference = ffmpeg::pipe(&args, &data, path).unwrap();
            let png = fs::read(
                output_dir(&out)
                    .join("screens")
                    .join(target(path, Some("png"))),
            )
            .unwrap();
            let (width, height, pixels) = read_png_bytes(&png);
            assert_eq!((width, height), (1024, 768));
            assert!(pixels == reference, "{path}");
        }
        // The credits, ASCII with CRLF in the original (docs/02 §11), with LF alone.
        for path in CREDITS {
            let original = read_game_file(&game.join(path)).unwrap();
            let ours = fs::read(output_dir(&out).join("texts").join(target(path, None))).unwrap();
            assert!(
                original.contains(&b'\r') && !ours.contains(&b'\r'),
                "{path}"
            );
            assert_eq!(
                String::from_utf8(ours).unwrap(),
                String::from_utf8(original).unwrap().replace("\r\n", "\n")
            );
        }
        fs::remove_dir_all(out).unwrap();
    }
}
