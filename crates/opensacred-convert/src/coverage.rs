//! `coverage` subcommand: what becomes of every file of the reference installation (G1 in
//! PLAN.md §1.2: the converter processes 100% of the files of the manifest). Each file is
//! converted by a subcommand, converted in part, skipped for a stated reason, or not converted
//! yet.

use anyhow::Result;

use crate::verify::{GOG_MANIFEST, parse_manifest};

/// What becomes of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The subcommand that converts it.
    Converted(&'static str),
    /// The subcommand that uses part of it, and what is not converted yet.
    #[expect(
        dead_code,
        reason = "no file is converted in part at the moment; the status stays for the next one"
    )]
    Partial(&'static str, &'static str),
    /// Why it needs no conversion.
    Skipped(&'static str),
    /// Not converted yet: what it holds.
    Pending(&'static str),
}

/// The status of each path, by the first rule whose pattern matches: a pattern ending in `/` is
/// a directory prefix, one starting with `*` a suffix, any other an exact path; all without
/// regard to case.
const RULES: &[(&str, Status)] = &[
    ("PAK/texture.pak", Status::Converted("textures")),
    ("PAK/texture03.pak", Status::Converted("textures")),
    ("PAK/models.pak", Status::Converted("models")),
    ("PAK/models03.pak", Status::Converted("models")),
    ("PAK/tiles.pak", Status::Converted("world")),
    ("PAK/mixed.pak", Status::Converted("world")),
    ("World/sectors.keyx", Status::Converted("world")),
    ("World/sectors.wldx", Status::Converted("world")),
    ("World/Static.PAK", Status::Converted("world")),
    ("World/Floor.PAK", Status::Converted("world")),
    ("PAK/Items.pak", Status::Converted("tables")),
    ("PAK/Items03.pak", Status::Converted("tables")),
    ("PAK/Creature.pak", Status::Converted("tables")),
    ("PAK/Weapon.pak", Status::Converted("tables")),
    ("PAK/MOTIONS.PAK", Status::Converted("tables")),
    ("World/Triggers.PAK", Status::Converted("tables")),
    ("PAK/sound.pak", Status::Converted("sound")),
    ("PAK/sndProfiles.pak", Status::Converted("sound")),
    ("mp3/", Status::Converted("sound")),
    ("movie/", Status::Converted("video")),
    ("scripts/us/global.res", Status::Converted("text")),
    (
        "PAK/Models.tmp",
        Status::Skipped("a cache of the model descriptors, rebuilt from the paks"),
    ),
    (
        "PAK/Texture.TMP",
        Status::Skipped("a cache of the texture directory, rebuilt from the paks"),
    ),
    (
        "Sacred.exe",
        Status::Skipped(
            "the original program: rules recovered in M3; `sound` reads its sound names, `tables` its combat arts",
        ),
    ),
    (
        "GameServer.exe",
        Status::Skipped("the network server: out of scope"),
    ),
    ("Config.exe", Status::Skipped("the original settings tool")),
    (
        "*.dll",
        Status::Skipped("a library of the original program"),
    ),
    (
        "mssds3d.m3d",
        Status::Skipped("a Miles sound provider of the original program"),
    ),
    (
        "mssmp3.asi",
        Status::Skipped("a Miles MP3 decoder of the original program"),
    ),
    ("*.bmp", Status::Converted("ui")),
    ("font/", Status::Converted("ui")),
    ("Templates/", Status::Converted("heroes")),
    ("credits.txt", Status::Converted("ui")),
    ("credits2.txt", Status::Converted("ui")),
    ("*/FunkCode.bin", Status::Converted("netscript")),
    ("*/StartCode.bin", Status::Converted("netscript")),
    ("*/QuestCode.bin", Status::Converted("netscript")),
    ("*/Vectoren.bin", Status::Converted("netscript")),
    ("bin/Balance.bin", Status::Converted("tables")),
    ("bin/World.bin", Status::Converted("tables")),
    ("bin/World2.bin", Status::Converted("tables")),
    ("bin/sets.bin", Status::Converted("tables")),
    ("bin/wpmod.bin", Status::Converted("tables")),
    ("bin/wea.bin", Status::Converted("tables")),
    ("bin/Rust.bin", Status::Converted("tables")),
    ("bin/treppe.bin", Status::Converted("tables")),
    ("bin/static10_18.bin", Status::Converted("tables")),
    ("bin/merc.bin", Status::Converted("tables")),
    ("bin/Addon/merc.bin", Status::Converted("tables")),
    ("bin/MultiStart.bin", Status::Converted("tables")),
];

/// The status of `path`; files that no rule names are pending.
pub fn status(path: &str) -> Status {
    let path = path.to_ascii_lowercase();
    RULES
        .iter()
        .find(|(pattern, _)| {
            let pattern = pattern.to_ascii_lowercase();
            if let Some(suffix) = pattern.strip_prefix('*') {
                path.ends_with(suffix)
            } else if pattern.ends_with('/') {
                path.starts_with(&pattern)
            } else {
                path == pattern
            }
        })
        .map_or(Status::Pending("not described"), |(_, status)| *status)
}

/// Counts of the manifest's files by status.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Coverage {
    pub converted: usize,
    pub partial: Vec<(String, &'static str)>,
    pub skipped: usize,
    pub pending: Vec<(String, &'static str)>,
}

pub fn coverage() -> Result<Coverage> {
    let mut out = Coverage::default();
    for entry in parse_manifest(GOG_MANIFEST)? {
        match status(entry.path) {
            Status::Converted(_) => out.converted += 1,
            Status::Partial(_, missing) => out.partial.push((entry.path.to_owned(), missing)),
            Status::Skipped(_) => out.skipped += 1,
            Status::Pending(what) => out.pending.push((entry.path.to_owned(), what)),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_match_paths_without_regard_to_case() {
        assert_eq!(status("PAK/TEXTURE.PAK"), Status::Converted("textures"));
        assert_eq!(status("mp3/MUSIC_CREDITS.mp3"), Status::Converted("sound"));
        assert_eq!(
            status("MSVCRT.DLL"),
            Status::Skipped("a library of the original program")
        );
        assert_eq!(
            status("bin/Addon/NetScript/FunkCode.bin"),
            Status::Converted("netscript")
        );
        assert_eq!(status("bin/wea.bin"), Status::Converted("tables"));
        assert_eq!(status("unknown.dat"), Status::Pending("not described"));
    }

    /// Every file of the manifest has a rule; the counts track M1.
    #[test]
    fn every_file_of_the_manifest_has_a_rule() {
        let coverage = coverage().unwrap();
        assert!(
            coverage
                .pending
                .iter()
                .all(|(_, what)| *what != "not described"),
            "{:?}",
            coverage.pending
        );
        let total =
            coverage.converted + coverage.partial.len() + coverage.skipped + coverage.pending.len();
        assert_eq!(total, 319);
    }
}
