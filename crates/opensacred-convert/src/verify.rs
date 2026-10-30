//! Checking an installation against the SHA-256 manifest of the GOG release
//! (`docs/manifest-gog-2.28.txt`, docs/02 §13), and of the four files in which the Steam release
//! differs from it (`docs/manifest-steam-2.28.txt`, docs/01 §1, #1490).
//!
//! The manifests are embedded in the binary so that the converter always knows the reference it
//! works with. The GOG one lists only the files that the project uses and the game never writes:
//! `Settings.cfg`, `Gameserver.cfg`, the saves of `save/` and the `DefPos.bin` caches change when
//! the game runs and are not in it, nor are the extra installation files (`unins*`, `goggame-*`,
//! PDFs, empty `Quest*Code.bin`), nor the files that nothing of the project reads (#1482):
//! `Config.exe`; `MSVCRT.DLL`, `MSVCP60.DLL` and `unicows.dll`, which the original never loads;
//! and the caches `PAK/Models.tmp` and `PAK/Texture.TMP`, which it writes again when they are
//! lost.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

/// Manifest of the reference GOG Sacred Gold 2.28 ENG installation in `sha256sum` format.
pub const GOG_MANIFEST: &str = include_str!("../../../docs/manifest-gog-2.28.txt");

/// The files of the Steam release of Sacred Gold 2.28 that differ from the GOG release's, in
/// `sha256sum` format: its `Sacred.exe`, the same build wrapped by Steam's DRM, whose data
/// sections are the GOG file's at the same addresses, two loading screens and an intro's logo.
/// Its other files are the GOG release's.
pub const STEAM_MANIFEST: &str = include_str!("../../../docs/manifest-steam-2.28.txt");

/// Contents of a file of the installation.
pub fn read_game_file(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).with_context(|| format!("reading {}", path.display()))
}

/// Contents of a file of the installation that must be the reference's own, its SHA-256 the GOG
/// manifest's or the Steam one's: for a reader that takes its data from fixed places of the
/// 2.28 build's data sections (the combat arts' tables of `Sacred.exe`, docs/02 §6.5), which the
/// two releases hold alike.
pub fn read_reference_file(game_dir: &Path, path: &str) -> Result<Vec<u8>> {
    read_checked(
        game_dir,
        path,
        &parse_manifest(GOG_MANIFEST)?,
        &parse_manifest(STEAM_MANIFEST)?,
    )
}

fn read_checked(
    game_dir: &Path,
    path: &str,
    manifest: &[Entry<'_>],
    variants: &[Entry<'_>],
) -> Result<Vec<u8>> {
    let expected = manifest
        .iter()
        .find(|e| e.path == path)
        .with_context(|| format!("{path} is not in the manifest"))?
        .sha256;
    let file = game_dir.join(path);
    let bytes = read_game_file(&file)?;
    let found = hex(&Sha256::digest(&bytes));
    let variant = variants.iter().any(|e| e.path == path && e.sha256 == found);
    if found != expected && !variant {
        bail!(
            "{} is not the GOG or Steam Sacred Gold 2.28 file: its SHA-256 is {found}, the GOG \
             manifest's {expected}; its data is read from that build's addresses",
            file.display()
        );
    }
    Ok(bytes)
}

/// Manifest entry: lower-case SHA-256 and a path relative to the installation root.
#[derive(Debug, PartialEq, Eq)]
pub struct Entry<'a> {
    pub sha256: &'a str,
    pub path: &'a str,
}

/// Parses a manifest in `sha256sum` format: lines `<64 hex digits>  <path>`.
pub fn parse_manifest(text: &str) -> Result<Vec<Entry<'_>>> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.is_empty())
        .map(|(i, line)| {
            let (sha256, path) = line
                .split_once("  ")
                .filter(|(_, path)| !path.is_empty())
                .with_context(|| format!("manifest line {}: expected `<sha256>  <path>`", i + 1))?;
            if sha256.len() != 64
                || !sha256
                    .bytes()
                    .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
            {
                bail!("manifest line {}: bad SHA-256 `{sha256}`", i + 1);
            }
            Ok(Entry { sha256, path })
        })
        .collect()
}

/// Mismatch between an installation file and the manifest.
#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    Missing,
    Mismatch,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Problem::Missing => "missing",
            Problem::Mismatch => "SHA-256 mismatch",
        })
    }
}

#[derive(Debug, Default)]
pub struct Report<'a> {
    /// Bytes read and hashed.
    pub bytes: u64,
    /// Mismatching files in manifest order.
    pub problems: Vec<(&'a str, Problem)>,
    /// The files that matched a variant of `variants` and not the manifest, in manifest order.
    pub variants: Vec<&'a str>,
}

/// Hashes every manifest file in `game_dir` and collects the mismatches; a file that is one of
/// `variants` instead, a release's own (the Steam release's four, [`STEAM_MANIFEST`]), matches.
pub fn verify<'a>(
    game_dir: &Path,
    manifest: &[Entry<'a>],
    variants: &[Entry<'_>],
) -> Result<Report<'a>> {
    let mut report = Report::default();
    for entry in manifest {
        let path = game_dir.join(entry.path);
        let (sha256, len) = match sha256_file(&path) {
            Ok(digest) => digest,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                report.problems.push((entry.path, Problem::Missing));
                continue;
            }
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        report.bytes += len;
        if sha256 == entry.sha256 {
            continue;
        }
        if variants
            .iter()
            .any(|v| v.path == entry.path && v.sha256 == sha256)
        {
            report.variants.push(entry.path);
        } else {
            report.problems.push((entry.path, Problem::Mismatch));
        }
    }
    Ok(report)
}

/// Lower-case SHA-256 of a file and its length.
pub(crate) fn sha256_file(path: &Path) -> io::Result<(String, u64)> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0; 1 << 20];
    let mut len = 0;
    loop {
        let n = match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        hasher.update(&buf[..n]);
        len += n as u64;
    }
    Ok((hex(&hasher.finalize()), len))
}

/// Lower-case hexadecimal of a digest.
pub(crate) fn hex(digest: &[u8]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::scratch_dir;

    /// SHA-256 of the string `hello`.
    const HELLO: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    #[test]
    fn embedded_manifest_is_the_gog_reference() {
        let manifest = parse_manifest(GOG_MANIFEST).unwrap();
        assert_eq!(manifest.len(), 313);
        // PLAN.md §2: SHA-256 `db9b86a4…71e74`
        let exe = manifest.iter().find(|e| e.path == "Sacred.exe").unwrap();
        assert!(exe.sha256.starts_with("db9b86a4") && exe.sha256.ends_with("71e74"));
        assert!(manifest.iter().all(|e| e.path != "Settings.cfg"
            && e.path != "Gameserver.cfg"
            && !e.path.starts_with("save/")
            && !e.path.ends_with("/DefPos.bin")));
        // Nothing of the project reads them: the client loads none of the libraries and writes
        // the caches again when they are lost (#1482).
        for unused in [
            "Config.exe",
            "MSVCP60.DLL",
            "MSVCRT.DLL",
            "PAK/Models.tmp",
            "PAK/Texture.TMP",
            "unicows.dll",
        ] {
            assert!(manifest.iter().all(|e| e.path != unused), "{unused}");
        }
        assert!(
            manifest.windows(2).all(|w| w[0].path < w[1].path),
            "paths must be sorted and unique"
        );
    }

    #[test]
    fn rejects_malformed_lines() {
        assert!(parse_manifest("abc  Sacred.exe\n").is_err());
        assert!(parse_manifest(&format!("{}  \n", "0".repeat(64))).is_err());
        assert!(parse_manifest(&format!("{} Sacred.exe\n", "0".repeat(64))).is_err());
    }

    /// The Steam release's four files are of the GOG manifest's paths, in its order, and differ
    /// from them (docs/01 §1, #1490).
    #[test]
    fn the_steam_manifest_names_four_gog_files_anew() {
        let gog = parse_manifest(GOG_MANIFEST).unwrap();
        let steam = parse_manifest(STEAM_MANIFEST).unwrap();
        let paths: Vec<&str> = steam.iter().map(|e| e.path).collect();
        assert_eq!(
            paths,
            [
                "PAK/loading0.bmp",
                "PAK/loading1.bmp",
                "Sacred.exe",
                "movie/oem.wmv"
            ]
        );
        for entry in &steam {
            let own = gog.iter().find(|e| e.path == entry.path).unwrap();
            assert_ne!(own.sha256, entry.sha256, "{}", entry.path);
        }
    }

    #[test]
    fn reports_missing_and_mismatch() {
        let dir = scratch_dir("verify");
        fs::write(dir.join("ok.bin"), "hello").unwrap();
        fs::write(dir.join("changed.bin"), "hello!").unwrap();
        fs::write(dir.join("variant.bin"), "hello!").unwrap();
        let text = format!(
            "{HELLO}  ok.bin\n{HELLO}  changed.bin\n{HELLO}  variant.bin\n{HELLO}  absent.bin\n"
        );
        let changed = hex(&Sha256::digest(b"hello!"));
        let variants = format!("{changed}  variant.bin\n");

        let report = verify(
            &dir,
            &parse_manifest(&text).unwrap(),
            &parse_manifest(&variants).unwrap(),
        )
        .unwrap();

        assert_eq!(
            report.problems,
            [
                ("changed.bin", Problem::Mismatch),
                ("absent.bin", Problem::Missing),
            ]
        );
        assert_eq!(report.variants, ["variant.bin"]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_reference_file_reads_only_as_a_manifest_has_it() {
        let dir = scratch_dir("verify-reference");
        fs::write(dir.join("ok.bin"), "hello").unwrap();
        fs::write(dir.join("changed.bin"), "hello!").unwrap();
        fs::write(dir.join("variant.bin"), "hello!").unwrap();
        let text = format!("{HELLO}  ok.bin\n{HELLO}  changed.bin\n{HELLO}  variant.bin\n");
        let manifest = parse_manifest(&text).unwrap();
        let variants = format!("{}  variant.bin\n", hex(&Sha256::digest(b"hello!")));
        let variants = parse_manifest(&variants).unwrap();
        assert_eq!(
            read_checked(&dir, "ok.bin", &manifest, &variants).unwrap(),
            b"hello"
        );
        assert_eq!(
            read_checked(&dir, "variant.bin", &manifest, &variants).unwrap(),
            b"hello!"
        );
        let error = read_checked(&dir, "changed.bin", &manifest, &variants)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("not the GOG or Steam Sacred Gold 2.28 file"),
            "{error}"
        );
        let error = read_checked(&dir, "other.bin", &manifest, &variants)
            .unwrap_err()
            .to_string();
        assert!(error.contains("not in the manifest"), "{error}");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn installation_matches_manifest() {
        let game_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../OriginalGame");
        let report = verify(&game_dir, &parse_manifest(GOG_MANIFEST).unwrap(), &[]).unwrap();
        assert!(report.problems.is_empty(), "{:?}", report.problems);
    }
}
