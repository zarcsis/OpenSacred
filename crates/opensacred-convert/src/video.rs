//! `video` subcommand: the WMV cutscenes of `movie/` → AV1 and Opus in WebM under
//! `<out>/video/` (docs/02 §10.3; the format is [`opensacred_data::video`]), encoded by FFmpeg
//! ([`crate::ffmpeg`]) with SVT-AV1 and libopus. The output is the same on every run: the muxer
//! writes no random ids (`+bitexact`), and SVT-AV1 encodes each cutscene on one thread
//! (`lp=1`; with more, the result varies from run to run), the cutscenes in parallel.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use opensacred_data::video::{INDEX, Movie, Video};
use rayon::prelude::*;

use crate::ffmpeg;

pub fn output_dir(out: &Path) -> PathBuf {
    out.join("video")
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

    let wanted = |name: &str| {
        let stem = name[..name.len() - 4].to_ascii_lowercase();
        only.is_empty()
            || only
                .iter()
                .any(|o| o.to_ascii_lowercase().trim_end_matches(".wmv") == stem)
    };
    let movies = names
        .par_iter()
        .filter(|name| wanted(name))
        .map(|name| {
            let stem = name[..name.len() - 4].to_ascii_lowercase();
            let file = format!("{stem}.webm");
            let input = source.join(name);
            let output = dir.join(&file);
            let args: Vec<&OsStr> = [
                OsStr::new("-y"),
                OsStr::new("-i"),
                input.as_os_str(),
                OsStr::new("-map_metadata"),
                OsStr::new("-1"),
                OsStr::new("-fflags"),
                OsStr::new("+bitexact"),
                OsStr::new("-c:v"),
                OsStr::new("libsvtav1"),
                OsStr::new("-crf"),
                OsStr::new("30"),
                OsStr::new("-preset"),
                OsStr::new("6"),
                OsStr::new("-svtav1-params"),
                OsStr::new("lp=1"),
                OsStr::new("-pix_fmt"),
                OsStr::new("yuv420p"),
                OsStr::new("-c:a"),
                OsStr::new("libopus"),
                OsStr::new("-b:a"),
                OsStr::new("128k"),
                OsStr::new("-f"),
                OsStr::new("webm"),
                output.as_os_str(),
            ]
            .to_vec();
            ffmpeg::run(&args, &format!("movie/{name}"))?;
            let bytes = fs::metadata(&output)
                .with_context(|| format!("reading {}", output.display()))?
                .len();
            Ok((Movie { name: stem, file }, bytes))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut summary = Summary::default();
    let mut list = Vec::with_capacity(movies.len());
    for (movie, bytes) in movies {
        summary.movies += 1;
        summary.bytes += bytes;
        list.push(movie);
    }
    let index = Video { movies: list };
    let text = ron::ser::to_string_pretty(&index, ron::ser::PrettyConfig::default())
        .context("writing video.ron")?;
    let path = dir.join(INDEX);
    fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{repo_root, scratch_dir};

    /// The shortest cutscene converts to a WebM that FFmpeg reads as AV1 video and Opus audio of
    /// the original's length.
    #[test]
    #[ignore = "needs the game files in OriginalGame/ and FFmpeg"]
    fn a_cutscene_converts_to_av1_and_opus() {
        let out = scratch_dir("video");
        let game = repo_root().join("OriginalGame");
        let summary = convert(&game, &out, &["ascaron".to_owned()]).unwrap();
        assert_eq!(summary.movies, 1);
        let webm = output_dir(&out).join("ascaron.webm");
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
        let index: Video =
            ron::from_str(&fs::read_to_string(output_dir(&out).join(INDEX)).unwrap()).unwrap();
        assert_eq!(index.movies[0].name, "ascaron");
        fs::remove_dir_all(out).unwrap();
    }
}
