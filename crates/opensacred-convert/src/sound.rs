//! `sound` subcommand: `PAK/sound.pak`, `PAK/sndProfiles.pak`, `mp3/` and the sound names of
//! `Sacred.exe` → `<out>/sound/` (docs/02 §10; the format is [`opensacred_data::sound`]): FLAC
//! masters, encoded by FFmpeg ([`crate::ffmpeg`]) bit-exactly, so that a rerun gives the same
//! files. A WAV sample becomes lossless FLAC; an MP3 sample and the music are decoded from their
//! first frame into 16-bit FLAC, which keeps what the MP3 decoder gives (#697).
//!
//! `build` encodes each master into Ogg Opus for the game ([`encode_opus`], #701) and copies
//! `sound.ron`, which names the files by the samples' ids and the music's names alone (#827).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use opensacred_data::sound::{
    INDEX, MASTER_EXTENSION, Music, Profile, Sound, SoundName, sample_file,
};
use opensacred_formats::sound::{Profiles, Sample, SoundPak, mp3_first_frame};
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

/// FFmpeg's arguments for an MP3 on stdin, from its first frame, to 16-bit FLAC on stdout.
const MP3_FLAC: [&str; 18] = [
    "-f",
    "mp3",
    "-i",
    "pipe:0",
    "-map_metadata",
    "-1",
    "-fflags",
    "+bitexact",
    "-flags:a",
    "+bitexact",
    "-sample_fmt",
    "s16",
    "-compression_level",
    "8",
    "-f",
    "flac",
    "-y",
    "pipe:1",
];

/// The bitrates of the game's Opus (#701, docs/decisions/2026-10-23-opus.md): a sample's mono and
/// stereo, at least the 48 to 96 kbit/s of the MP3 samples but for 72 of 128; the music's, at the
/// MP3s' 256 kbit/s.
const SAMPLE_BITRATES: [&str; 2] = ["96k", "128k"];
const MUSIC_BITRATES: [&str; 2] = ["128k", "256k"];

pub fn output_dir(out: &Path) -> PathBuf {
    out.join("sound")
}

/// The channels of the FLAC stream `flac`, from its first metadata block, the stream's info.
fn flac_channels(flac: &[u8]) -> Result<u8> {
    if flac.get(..4) != Some(b"fLaC") || flac.get(4).map(|b| b & 0x7F) != Some(0) {
        bail!("not a FLAC stream that begins with its STREAMINFO");
    }
    // After the sample rate's 20 bits, at the byte 18 of the stream, 3 bits of channels less one.
    let byte = *flac.get(20).context("a FLAC stream cut short")?;
    Ok(((byte >> 1) & 0x07) + 1)
}

/// The Ogg Opus file of the sound whose master is the FLAC `flac` (`build`, #701): FFmpeg's
/// libopus at 48 kHz, at the bitrate of a music file or of a sample and of its channels.
pub fn encode_opus(flac: &[u8], music: bool, what: &str) -> Result<Vec<u8>> {
    let [mono, stereo] = if music {
        MUSIC_BITRATES
    } else {
        SAMPLE_BITRATES
    };
    let bitrate = if flac_channels(flac).with_context(|| what.to_owned())? == 1 {
        mono
    } else {
        stereo
    };
    let args = [
        "-f",
        "flac",
        "-i",
        "pipe:0",
        "-map_metadata",
        "-1",
        "-fflags",
        "+bitexact",
        "-flags:a",
        "+bitexact",
        "-ar",
        "48000",
        "-c:a",
        "libopus",
        "-b:a",
        bitrate,
        "-f",
        "ogg",
        "-y",
        "pipe:1",
    ];
    ffmpeg::pipe(&args, flac, what)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    /// The samples from a WAV and from an MP3, all FLAC now.
    pub flac: usize,
    pub mp3: usize,
    pub profiles: usize,
    pub music: usize,
    /// Music files written from their first frame, the bytes before it dropped.
    pub music_trimmed: usize,
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
        .map(|&slot| {
            let what = format!("PAK/sound.pak sample {slot}");
            let bytes = match pak.get(slot).context(what.clone())? {
                Some(Sample::Wav { data, .. }) => ffmpeg::pipe(&FLAC, data, &what)?,
                // Decoded from the first frame: the zeros some records carry before it are left
                // out (#354, docs/decisions/2026-10-20-mp3-leading-zeros.md).
                Some(Sample::Mp3 { data, lead }) => ffmpeg::pipe(&MP3_FLAC, &data[lead..], &what)?,
                None => unreachable!("ids() lists used slots"),
            };
            let id = u32::try_from(slot).context("sample id")?;
            let path = dir.join(sample_file(id, MASTER_EXTENSION));
            fs::write(&path, &bytes).with_context(|| format!("writing {}", path.display()))?;
            Ok((slot, id, bytes.len()))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut summary = Summary::default();
    let mut samples = Vec::with_capacity(converted.len());
    for (slot, id, len) in converted {
        let mp3 = pak
            .get(slot)
            .ok()
            .flatten()
            .is_some_and(|sample| matches!(sample, Sample::Mp3 { .. }));
        if mp3 {
            summary.mp3 += 1;
        } else {
            summary.flac += 1;
        }
        summary.bytes += u64::try_from(len).unwrap_or(0);
        samples.push(id);
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
        // The zeros or junk before the first frame of 154 files are dropped (#367,
        // docs/decisions/2026-10-20-music-junk.md).
        let start = mp3_first_frame(&data)
            .with_context(|| format!("mp3/{name}: no MPEG-1 Layer III frame"))?;
        if start > 0 {
            summary.music_trimmed += 1;
        }
        let stem = stem.to_owned();
        music.push((id, stem, name, start, data));
    }
    // Decoded from the first frame into FLAC (#697).
    let decoded = music
        .par_iter()
        .map(|(id, stem, name, start, data)| {
            let music = Music {
                id: *id,
                name: stem.clone(),
            };
            let flac = ffmpeg::pipe(&MP3_FLAC, &data[*start..], &format!("mp3/{name}"))?;
            let path = dir.join(music.file(MASTER_EXTENSION));
            fs::write(&path, &flac).with_context(|| format!("writing {}", path.display()))?;
            Ok((music, flac.len()))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut music = Vec::with_capacity(decoded.len());
    for (entry, len) in decoded {
        summary.bytes += u64::try_from(len).unwrap_or(0);
        music.push(entry);
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

    /// A FLAC of a second of a tone, mono and stereo, becomes Ogg Opus at its bitrate that FFmpeg
    /// decodes back to the same length at 48 kHz.
    #[test]
    #[ignore = "needs FFmpeg"]
    fn a_master_becomes_ogg_opus_of_its_length() {
        let tone = |channels: &str| {
            let args = [
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=44100:duration=1",
                "-ac",
                channels,
                "-f",
                "flac",
                "pipe:1",
            ];
            ffmpeg::pipe(&args, &[], "a tone").unwrap()
        };
        for (channels, count) in [("1", 1), ("2", 2)] {
            let flac = tone(channels);
            assert_eq!(flac_channels(&flac).unwrap(), count);
            let opus = encode_opus(&flac, false, "a tone").unwrap();
            assert_eq!(&opus[..4], b"OggS");
            let pcm = ffmpeg::pipe(
                &["-i", "pipe:0", "-f", "s16le", "-ac", "1", "pipe:1"],
                &opus,
                "the tone's Opus",
            )
            .unwrap();
            assert_eq!(pcm.len(), 2 * 48_000, "{channels} channels");
        }
        assert!(flac_channels(b"RIFF").is_err());
    }

    /// A PCM, an IMA ADPCM and an MP3 sample become FLAC that FFmpeg decodes to the PCM it
    /// decodes the original to, the MP3 from its first frame as 16 bits; so does the music; a
    /// second run writes the same bytes.
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
                    Sample::Mp3 { lead, .. } => want == "mp3" && lead > 0,
                })
                .unwrap();
            picked.push(id);
        }
        let out = scratch_dir("sound");
        let summary = convert(&game, &out, &picked).unwrap();
        assert_eq!((summary.flac, summary.mp3, summary.profiles), (2, 1, 175));
        let dir = output_dir(&out);
        let pcm = |data: &[u8], format: &str| {
            let args = [
                "-f",
                format,
                "-i",
                "pipe:0",
                "-sample_fmt",
                "s16",
                "-f",
                "s16le",
                "pipe:1",
            ];
            ffmpeg::pipe(&args, data, "a test sample").unwrap()
        };
        let master = |slot: usize| {
            fs::read(dir.join(sample_file(u32::try_from(slot).unwrap(), MASTER_EXTENSION))).unwrap()
        };
        for &id in &picked[..2] {
            let Some(Sample::Wav { data, .. }) = pak.get(id).unwrap() else {
                panic!("{id}");
            };
            assert_eq!(pcm(&master(id), "flac"), pcm(data, "wav"), "sample {id}");
        }
        let flac = master(picked[2]);
        let Some(Sample::Mp3 { data, lead }) = pak.get(picked[2]).unwrap() else {
            panic!("{}", picked[2]);
        };
        assert_eq!(pcm(&flac, "flac"), pcm(&data[lead..], "mp3"));
        let first = master(picked[0]);
        convert(&game, &out, &picked[..1]).unwrap();
        assert_eq!(master(picked[0]), first);
        let index: Sound = ron::from_str(&fs::read_to_string(dir.join(INDEX)).unwrap()).unwrap();
        assert_eq!(index.samples, [u32::try_from(picked[0]).unwrap()]);
        assert_eq!(index.music.len(), 173);
        // docs/02 §10.3: 154 music files hold bytes before their first frame.
        assert_eq!(summary.music_trimmed, 154);
        // The first music file: the PCM of its FLAC, that of the MP3 from its first frame.
        let first = &index.music[0];
        let ours = fs::read(dir.join(first.file(MASTER_EXTENSION))).unwrap();
        let theirs = fs::read(game.join("mp3").join(format!("{}.mp3", first.name))).unwrap();
        let start = mp3_first_frame(&theirs).unwrap();
        assert_eq!(
            pcm(&ours, "flac"),
            pcm(&theirs[start..], "mp3"),
            "{}",
            first.name
        );
        assert!(
            index
                .music
                .iter()
                .all(|m| dir.join(m.file(MASTER_EXTENSION)).is_file())
        );
        assert_eq!(index.music.iter().filter(|m| m.id.is_some()).count(), 171);
        assert_eq!(index.names.len(), 6869);
        let wood = index.music.iter().find(|m| m.id == Some(6560)).unwrap();
        assert_eq!(wood.name, "MUSIC_WOOD01");
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
