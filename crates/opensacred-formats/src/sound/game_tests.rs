//! Tests on the GOG installation files: every sample of `sound.pak` and every profile of
//! `sndProfiles.pak` against the tables of the Python prototype
//! `tools/analysis/formats/sound_dump.py`. CI does not have the game files, so the test is ignored
//! by default.

use std::ffi::OsStr;
use std::fs;

use super::*;
use crate::testing::{game_file, run_python, scratch_dir};

#[test]
#[ignore = "needs the game files in OriginalGame/ and the Python of pyproject.toml through uv"]
fn samples_and_profiles_match_sound_dump_py() {
    let out = scratch_dir("sound");
    let (samples_table, profiles_table) = (out.join("samples.tsv"), out.join("profiles.tsv"));
    let game_dir = opensacred_locate::original();
    run_python(
        "tools/analysis/formats/sound_dump.py",
        &[
            OsStr::new("--game-dir"),
            game_dir.as_os_str(),
            OsStr::new("--samples"),
            samples_table.as_os_str(),
            OsStr::new("--profiles"),
            profiles_table.as_os_str(),
        ],
    );

    let data = game_file("PAK/sound.pak");
    let pak = SoundPak::parse(&data).unwrap();
    let text = fs::read_to_string(&samples_table).unwrap();
    let mut lines = text.lines().skip(1);
    let mut count = 0;
    let mut leads = Vec::new();
    for id in pak.ids() {
        let bytes = pak.bytes(id).unwrap();
        let (flags, kind) = match pak.get(id).unwrap().unwrap() {
            Sample::Wav { format, .. } => (32, if format.tag == 1 { "pcm" } else { "adpcm" }),
            Sample::Mp3 { data, lead } => {
                assert_eq!(data, bytes, "sample {id}");
                if lead > 0 {
                    leads.push(lead);
                }
                (33, "mp3")
            }
        };
        let ours = format!(
            "{id}\t{flags}\t{}\t{kind}\t{}",
            bytes.len(),
            crc32fast::hash(bytes)
        );
        assert_eq!(lines.next().unwrap(), ours);
        count += 1;
    }
    assert!(lines.next().is_none());
    assert_eq!(count, 6598);
    // docs/02 §10.2: 73 MP3 records start with one frame's worth of zeros.
    assert_eq!(leads.len(), 73);
    assert!(leads.iter().all(|&l| l == 208 || l == 417), "{leads:?}");

    let data = game_file("PAK/sndProfiles.pak");
    let profiles = Profiles::parse(&data).unwrap();
    let text = fs::read_to_string(&profiles_table).unwrap();
    let mut lines = text.lines().skip(1);
    for slot in 0..profiles.len() {
        let Some(profile) = profiles.get(slot).unwrap() else {
            continue;
        };
        let name: String = profile.name.iter().map(|&b| char::from(b)).collect();
        let groups: Vec<String> = profile
            .groups
            .iter()
            .map(|g| g.iter().map(u16::to_string).collect::<Vec<_>>().join(" "))
            .collect();
        assert_eq!(
            lines.next().unwrap(),
            format!("{slot}\t{name}\t{}", groups.join(";"))
        );
    }
    assert!(lines.next().is_none());
    fs::remove_dir_all(out).unwrap();
}

#[test]
#[ignore = "needs the game files in OriginalGame/ and the Python of pyproject.toml through uv"]
fn names_match_sound_dump_py() {
    let out = scratch_dir("sound-names");
    let table = out.join("names.tsv");
    let game_dir = opensacred_locate::original();
    run_python(
        "tools/analysis/formats/sound_dump.py",
        &[
            OsStr::new("--game-dir"),
            game_dir.as_os_str(),
            OsStr::new("--names"),
            table.as_os_str(),
        ],
    );
    let exe = game_file("Sacred.exe");
    let names = names(&exe).unwrap();
    assert_eq!(names.len(), 6869);
    assert_eq!(
        names[0],
        Name {
            id: 0,
            name: "SOUND_FX_INVALID"
        }
    );
    let text = fs::read_to_string(&table).unwrap();
    let theirs: Vec<&str> = text.lines().skip(1).collect();
    let ours: Vec<String> = names
        .iter()
        .map(|n| format!("{}\t{}", n.id, n.name))
        .collect();
    assert_eq!(theirs, ours);
    fs::remove_dir_all(out).unwrap();
}
