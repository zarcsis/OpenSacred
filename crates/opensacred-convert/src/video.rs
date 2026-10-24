//! `video` subcommand: the WMV cutscenes of `movie/` → lossless masters under `<out>/video/`
//! (docs/02 §10.3; the format is [`opensacred_data::video`]), FFV1 version 3 video in the decoded
//! `yuv420p` and FLAC audio in 16 bits in Matroska, by FFmpeg ([`crate::ffmpeg`]) without random ids
//! (`+bitexact`) (#698); and stage B's encoding of a master into AV1 and Opus in WebM ([`encode`]),
//! with SVT-AV1 on all threads, whose output may vary from run to run (#694). No index lists them:
//! a cutscene's file is named by the cutscene (#828).

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use opensacred_data::video::{MASTER_EXTENSION, file};
use rayon::prelude::*;

use crate::ffmpeg;

/// The cutscenes' directory under the output.
pub const DIR: &str = "video";
/// The index of the cutscenes that the runs before #828 wrote, removed by a full run.
const OLD_INDEX: &str = "video.ron";

pub fn output_dir(out: &Path) -> PathBuf {
    out.join(DIR)
}

/// Encodes the master `mkv` into the game's `webm`: AV1 by SVT-AV1 at CRF 30, preset 6, and Opus
/// at 128 kbit/s.
pub fn encode(mkv: &Path, webm: &Path) -> Result<()> {
    let args: Vec<&OsStr> = [
        OsStr::new("-y"),
        OsStr::new("-i"),
        mkv.as_os_str(),
        OsStr::new("-map_metadata"),
        OsStr::new("-1"),
        OsStr::new("-c:v"),
        OsStr::new("libsvtav1"),
        OsStr::new("-crf"),
        OsStr::new("30"),
        OsStr::new("-preset"),
        OsStr::new("6"),
        OsStr::new("-pix_fmt"),
        OsStr::new("yuv420p"),
        OsStr::new("-c:a"),
        OsStr::new("libopus"),
        OsStr::new("-b:a"),
        OsStr::new("128k"),
        OsStr::new("-f"),
        OsStr::new("webm"),
        webm.as_os_str(),
    ]
    .to_vec();
    ffmpeg::run(&args, &mkv.display().to_string())
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub movies: usize,
    pub bytes: u64,
}

/// Converts every cutscene, or for a non-empty `only` those with these names (without regard to
/// case or the extension).
pub fn convert(game_dir: &Path, out: &Path, only: &[String]) -> Result<Summary> {
    let source = game_dir.join("movie");
    let mut names: Vec<String> = fs::read_dir(&source)
        .with_context(|| format!("reading {}", source.display()))?
        .map(|entry| entry.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<Result<_, _>>()
        .with_context(|| format!("reading {}", source.display()))?;
    names.retain(|n| n.to_ascii_lowercase().ends_with(".wmv"));
    names.sort();
    let dir = output_dir(out);
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    // A full run removes the WebM that the converter wrote here before the masters (#698), and
    // the index it wrote before #828.
    if only.is_empty() {
        for entry in fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
            let path = entry
                .with_context(|| format!("reading {}", dir.display()))?
                .path();
            if path.extension().is_some_and(|e| e == "webm")
                || path.file_name().is_some_and(|n| n == OLD_INDEX)
            {
                fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
            }
        }
    }

    let wanted = |name: &str| {
        let stem = name[..name.len() - 4].to_ascii_lowercase();
        only.is_empty()
            || only
                .iter()
                .any(|o| o.to_ascii_lowercase().trim_end_matches(".wmv") == stem)
    };
    let sizes = names
        .par_iter()
        .filter(|name| wanted(name))
        .map(|name| {
            let stem = name[..name.len() - 4].to_ascii_lowercase();
            let input = source.join(name);
            let output = dir.join(file(&stem, MASTER_EXTENSION));
            let args: Vec<&OsStr> = [
                OsStr::new("-y"),
                OsStr::new("-i"),
                input.as_os_str(),
                OsStr::new("-map_metadata"),
                OsStr::new("-1"),
                OsStr::new("-fflags"),
                OsStr::new("+bitexact"),
                OsStr::new("-flags:v"),
                OsStr::new("+bitexact"),
                OsStr::new("-flags:a"),
                OsStr::new("+bitexact"),
                OsStr::new("-c:v"),
                OsStr::new("ffv1"),
                OsStr::new("-level"),
                OsStr::new("3"),
                OsStr::new("-pix_fmt"),
                OsStr::new("yuv420p"),
                OsStr::new("-c:a"),
                OsStr::new("flac"),
                OsStr::new("-sample_fmt"),
                OsStr::new("s16"),
                OsStr::new("-f"),
                OsStr::new("matroska"),
                output.as_os_str(),
            ]
            .to_vec();
            ffmpeg::run(&args, &format!("movie/{name}"))?;
            fs::metadata(&output)
                .with_context(|| format!("reading {}", output.display()))
                .map(|m| m.len())
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Summary {
        movies: sizes.len(),
        bytes: sizes.iter().sum(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{repo_root, scratch_dir};

    /// The MD5 of a cutscene's stream as FFmpeg decodes it, the frames in `yuv420p` and the
    /// samples in 16 bits: whole, as the WMV's and the FLAC's packets differ in length.
    fn decoded(file: &Path, stream: &str) -> String {
        let output = std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-i"])
            .arg(file)
            .args(["-map", stream, "-sample_fmt", "s16", "-pix_fmt", "yuv420p"])
            .args(["-f", "md5", "-"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    /// The shortest cutscene's master holds the WMV's frames and samples, and the game's WebM
    /// that stage B encodes from it is AV1 video and Opus audio; no index lists it.
    #[test]
    #[ignore = "needs the game files in OriginalGame/ and FFmpeg"]
    fn a_cutscene_becomes_a_lossless_master_and_av1_and_opus() {
        let out = scratch_dir("video");
        let game = repo_root().join("OriginalGame");
        let summary = convert(&game, &out, &["ascaron".to_owned()]).unwrap();
        assert_eq!(summary.movies, 1);
        let wmv = game.join("movie/ascaron.wmv");
        let mkv = output_dir(&out).join("ascaron.mkv");
        for stream in ["0:v", "0:a"] {
            let theirs = decoded(&wmv, stream);
            assert!(theirs.starts_with("MD5="), "{stream}: {theirs}");
            assert_eq!(decoded(&mkv, stream), theirs, "{stream}");
        }
        let webm = output_dir(&out).join("ascaron.webm");
        encode(&mkv, &webm).unwrap();
        let probe = std::process::Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "stream=codec_name",
                "-of",
                "csv=p=0",
            ])
            .arg(&webm)
            .output()
            .unwrap();
        let codecs = String::from_utf8_lossy(&probe.stdout);
        assert_eq!(
            codecs.split_whitespace().collect::<Vec<_>>(),
            ["av1", "opus"]
        );
        let mut written: Vec<_> = fs::read_dir(output_dir(&out))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        written.sort();
        assert_eq!(written, ["ascaron.mkv", "ascaron.webm"]);
        fs::remove_dir_all(out).unwrap();
    }
}
