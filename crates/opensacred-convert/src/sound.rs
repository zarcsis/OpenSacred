//! `sound` subcommand: `PAK/sound.pak`, `PAK/sndProfiles.pak`, `mp3/` and the sound names of
//! `Sacred.exe` → `<out>/sound/` (docs/02 §10; the format is [`opensacred_data::sound`]). A WAV sample becomes lossless FLAC, encoded
//! by FFmpeg ([`crate::ffmpeg`]) bit-exactly, so that a rerun gives the same files; an MP3 sample
//! and the music are copied, an MP3 sample without the zero bytes before its first frame.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use opensacred_data::sound::{INDEX, Music, Profile, SampleEntry, Sound, SoundName};
use opensacred_formats::sound::{Profiles, Sample, SoundPak};
use rayon::prelude::*;

use crate::ffmpeg;
use crate::verify::read_game_file;

/// FFmpeg's arguments for a WAV on stdin to FLAC on stdout, without metadata or encoder tags.
const FLAC: [&str; 16] = [
    "-f",
    "wav",
    "-i",
    "pipe:0",
    "-map_metadata",
    "-1",
    "-fflags",
    "+bitexact",
    "-flags:a",
    "+bitexact",
    "-compression_level",
    "8",
    "-f",
    "flac",
    "-y",
    "pipe:1",
];

pub fn output_dir(out: &Path) -> PathBuf {
    out.join("sound")
}

/// `samples/01020.flac`.
pub fn sample_file(id: usize, extension: &str) -> String {
    format!("samples/{id:05}.{extension}")
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub flac: usize,
    pub mp3: usize,
    pub profiles: usize,
    pub music: usize,
    pub bytes: u64,
}

/// Converts every sample, or for a non-empty `only` the samples with these ids, the profiles
/// and the music.
pub fn convert(game_dir: &Path, out: &Path, only: &[usize]) -> Result<Summary> {
    let sound = read_game_file(&game_dir.join("PAK/sound.pak"))?;
    let profiles = read_game_file(&game_dir.join("PAK/sndProfiles.pak"))?;
    let pak = SoundPak::parse(&sound).context("PAK/sound.pak")?;
    let profiles = Profiles::parse(&profiles).context("PAK/sndProfiles.pak")?;
    let exe = read_game_file(&game_dir.join("Sacred.exe"))?;
    let names: Vec<SoundName> = opensacred_formats::sound::names(&exe)
        .context("Sacred.exe")?
        .iter()
        .map(|n| SoundName {
            id: n.id,
            name: n.name.to_owned(),
        })
        .collect();
    // The first of an id's names, as the table lists them; two ids have two.
    let mut ids_by_name = HashMap::new();
    for n in &names {
        ids_by_name.entry(n.name.as_str()).or_insert(n.id);
    }
    let dir = output_dir(out);
    fs::create_dir_all(dir.join("samples")).context("creating the samples directory")?;
    fs::create_dir_all(dir.join("music")).context("creating the music directory")?;

    let ids: Vec<usize> = pak
        .ids()
        .filter(|id| only.is_empty() || only.contains(id))
        .collect();
    let converted = ids
        .par_iter()
        .map(|&id| {
            let what = format!("PAK/sound.pak sample {id}");
            let (file, bytes) = match pak.get(id).context(what.clone())? {
                Some(Sample::Wav { data, .. }) => {
                    (sample_file(id, "flac"), ffmpeg::pipe(&FLAC, data, &what)?)
                }
                Some(Sample::Mp3 { data }) => (sample_file(id, "mp3"), data.to_vec()),
                None => unreachable!("ids() lists used slots"),
            };
            let path = dir.join(&file);
            fs::write(&path, &bytes).with_context(|| format!("writing {}", path.display()))?;
            let id = u32::try_from(id).context("sample id")?;
            Ok((SampleEntry { id, file }, bytes.len()))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut summary = Summary::default();
    let mut samples = Vec::with_capacity(converted.len());
    for (entry, len) in converted {
        if entry.file.ends_with(".flac") {
            summary.flac += 1;
        } else {
            summary.mp3 += 1;
        }
        summary.bytes += u64::try_from(len).unwrap_or(0);
        samples.push(entry);
    }

    let mut list = Vec::new();
    for slot in 0..profiles.len() {
        let Some(profile) = profiles
            .get(slot)
            .with_context(|| format!("PAK/sndProfiles.pak slot {slot}"))?
        else {
            continue;
        };
        list.push(Profile {
            slot: u32::try_from(slot).context("profile slot")?,
            name: profile.name.iter().map(|&b| char::from(b)).collect(),
            groups: profile
                .groups
                .iter()
                .map(|g| g.iter().map(|&id| u32::from(id)).collect())
                .collect(),
        });
    }
    summary.profiles = list.len();

    let mut music = Vec::new();
    let source = game_dir.join("mp3");
    let mut files: Vec<_> = fs::read_dir(&source)
        .with_context(|| format!("reading {}", source.display()))?
        .map(|entry| entry.map(|e| e.file_name()))
        .collect::<Result<_, _>>()
        .with_context(|| format!("reading {}", source.display()))?;
    files.sort();
    for name in files {
        let name = name.to_string_lossy().into_owned();
        let Some(stem) = name
            .len()
            .checked_sub(4)
            .filter(|&n| {
                name.get(n..)
                    .is_some_and(|e| e.eq_ignore_ascii_case(".mp3"))
            })
            .and_then(|n| name.get(..n))
        else {
            continue;
        };
        let id = ids_by_name
            .get(format!("SOUND_FX_{}", stem.to_ascii_uppercase()).as_str())
            .copied();
        let data = read_game_file(&source.join(&name))?;
        let file = format!("music/{name}");
        let path = dir.join(&file);
        fs::write(&path, &data).with_context(|| format!("writing {}", path.display()))?;
        summary.bytes += u64::try_from(data.len()).unwrap_or(0);
        music.push(Music { id, file });
    }
    summary.music = music.len();

    let index = Sound {
        samples,
        profiles: list,
        music,
        names,
    };
    let text = ron::ser::to_string_pretty(&index, ron::ser::PrettyConfig::default())
        .context("writing sound.ron")?;
    let path = dir.join(INDEX);
    fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{repo_root, scratch_dir};

    #[test]
    fn sample_files_are_named_by_id() {
        assert_eq!(sample_file(1020, "flac"), "samples/01020.flac");
    }

    /// A PCM and an IMA ADPCM sample become FLAC that FFmpeg decodes back to the PCM it decodes
    /// the original to; an MP3 sample is copied without its leading zeros; a second run writes the
    /// same bytes.
    #[test]
    #[ignore = "needs the game files in OriginalGame/ and FFmpeg"]
    fn samples_convert_losslessly_and_reproducibly() {
        let game = repo_root().join("OriginalGame");
        let sound = read_game_file(&game.join("PAK/sound.pak")).unwrap();
        let pak = SoundPak::parse(&sound).unwrap();
        // The first PCM, the first IMA ADPCM and the first MP3 sample with leading zeros.
        let mut picked = Vec::new();
        for want in ["pcm", "adpcm", "mp3"] {
            let id = pak
                .ids()
                .find(|&id| match pak.get(id).unwrap().unwrap() {
                    Sample::Wav { format, .. } => {
                        (format.tag == 1 && want == "pcm")
                            || (format.tag == 0x11 && want == "adpcm")
                    }
                    Sample::Mp3 { .. } => want == "mp3" && pak.bytes(id).unwrap()[0] == 0,
                })
                .unwrap();
            picked.push(id);
        }
        let out = scratch_dir("sound");
        let summary = convert(&game, &out, &picked).unwrap();
        assert_eq!((summary.flac, summary.mp3, summary.profiles), (2, 1, 175));
        let dir = output_dir(&out);
        let pcm = |data: &[u8], format: &str| {
            let args = ["-f", format, "-i", "pipe:0", "-f", "s16le", "pipe:1"];
            ffmpeg::pipe(&args, data, "a test sample").unwrap()
        };
        for &id in &picked[..2] {
            let Some(Sample::Wav { data, .. }) = pak.get(id).unwrap() else {
                panic!("{id}");
            };
            let flac = fs::read(dir.join(sample_file(id, "flac"))).unwrap();
            assert_eq!(pcm(&flac, "flac"), pcm(data, "wav"), "sample {id}");
        }
        let mp3 = fs::read(dir.join(sample_file(picked[2], "mp3"))).unwrap();
        assert_eq!(&mp3[..2], &[0xFF, 0xFB]);
        let first = fs::read(dir.join(sample_file(picked[0], "flac"))).unwrap();
        convert(&game, &out, &picked[..1]).unwrap();
        assert_eq!(
            fs::read(dir.join(sample_file(picked[0], "flac"))).unwrap(),
            first
        );
        let index: Sound = ron::from_str(&fs::read_to_string(dir.join(INDEX)).unwrap()).unwrap();
        assert_eq!(index.music.len(), 173);
        assert_eq!(index.music.iter().filter(|m| m.id.is_some()).count(), 171);
        assert_eq!(index.names.len(), 6869);
        let wood = index.music.iter().find(|m| m.id == Some(6560)).unwrap();
        assert_eq!(wood.file, "music/MUSIC_WOOD01.mp3");
        let footsteps = index
            .profiles
            .iter()
            .find(|p| p.name == "footsteps_hero_male")
            .unwrap();
        assert_eq!(footsteps.groups[1], [1020, 1021, 1022]);
        assert_eq!(footsteps.groups[2], [1010]);
        fs::remove_dir_all(out).unwrap();
    }
}
