//! `ui` subcommand: the loading and save screens of `PAK/*.bmp` → PNG, the fonts of `font/` and
//! the credits → `<out>/ui/` (docs/02 §11), the credits as UTF-8 with LF line ends (#835).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use opensacred_formats::bmp;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffmpeg;
    use crate::testing::{read_png_bytes, scratch_dir};

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
        assert_eq!((summary.screens, summary.fonts, summary.texts), (6, 2, 2));
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
