//! The oracle's inputs and outputs: the files of the installation it runs on, checked against the
//! manifest, and the directory of the vectors it writes, outside the repository.
//!
//! A set of vectors is a JSON file `<set>.json`: the set's name, the original's function, the
//! SHA-256 of the `Sacred.exe` it ran, the FPU's control word, and the vectors, each the inputs
//! and the outputs by name (docs/03 §3.4). `opensacred-rules`' tests read them.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
#[cfg(feature = "emulator")]
use serde::Serialize;
use sha2::{Digest, Sha256};

/// The manifest of the reference GOG Sacred Gold 2.28 installation (docs/02 §13).
const MANIFEST: &str = include_str!("../../../docs/manifest-gog-2.28.txt");

/// The environment variable that names the vectors' directory, for the oracle and the tests.
pub const DIRECTORY_VARIABLE: &str = "OPENSACRED_VECTORS";

/// The SHA-256 that the manifest gives the file `path` of the installation, cased as there.
pub fn expected_sha256(path: &str) -> Option<&'static str> {
    MANIFEST.lines().find_map(|line| {
        let (sha256, name) = line.split_once("  ")?;
        (name == path).then_some(sha256)
    })
}

/// Lower-case hexadecimal SHA-256 of `bytes`.
pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The file `path` of the installation in `game_dir`, refused unless it is the manifest's: the
/// oracle runs on the GOG 2.28 build alone, whose addresses docs/re names.
pub fn checked(game_dir: &Path, path: &str) -> Result<Vec<u8>> {
    let Some(expected) = expected_sha256(path) else {
        bail!("{path} is not in the manifest");
    };
    let file = game_dir.join(path);
    let bytes = fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
    let found = sha256(&bytes);
    if found != expected {
        bail!(
            "{} is not the GOG Sacred Gold 2.28 file: its SHA-256 is {found}, the manifest's \
             {expected}; the oracle runs on that build alone",
            file.display()
        );
    }
    Ok(bytes)
}

/// The vectors' directory: `out` when given, else the variable `OPENSACRED_VECTORS`, else
/// `%LOCALAPPDATA%\OpenSacred\vectors` on Windows and `$XDG_DATA_HOME/OpenSacred/vectors`
/// (`~/.local/share/OpenSacred/vectors`) elsewhere.
pub fn directory(out: Option<&Path>) -> Result<PathBuf> {
    if let Some(out) = out {
        return Ok(out.to_owned());
    }
    if let Some(dir) = env_dir(DIRECTORY_VARIABLE) {
        return Ok(dir);
    }
    let base = if cfg!(windows) {
        env_dir("LOCALAPPDATA")
    } else {
        env_dir("XDG_DATA_HOME")
            .or_else(|| env_dir("HOME").map(|home| home.join(".local").join("share")))
    };
    match base {
        Some(base) => Ok(base.join("OpenSacred").join("vectors")),
        None => bail!("no directory for the vectors: give --out or {DIRECTORY_VARIABLE}"),
    }
}

/// The absolute directory in the environment variable `name`.
fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

/// The repository's root, which the vectors never go into: they are made of the original's code
/// (docs/08 §3).
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Makes the directory `dir` for the vectors, refusing one inside the repository.
pub fn prepare(dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let resolved = fs::canonicalize(dir).with_context(|| format!("resolving {}", dir.display()))?;
    if let Ok(repository) = fs::canonicalize(repository())
        && resolved.starts_with(&repository)
    {
        bail!(
            "{} is inside the repository: the vectors come from the original's code and are \
             never committed (docs/08 §3); give a directory outside it",
            dir.display()
        );
    }
    Ok(dir.to_owned())
}

/// What a set of vectors is.
#[cfg(feature = "emulator")]
#[derive(Serialize)]
pub struct About {
    /// The set's name, its file's.
    pub set: &'static str,
    /// The original's function, its address.
    pub function: String,
    /// What the vectors are, in a sentence.
    pub description: &'static str,
    /// The SHA-256 of the `Sacred.exe` that ran.
    pub sacred_exe: String,
    /// The FPU's control word of the calls.
    pub fpu_control_word: u16,
}

/// A set of vectors as written.
#[cfg(feature = "emulator")]
#[derive(Serialize)]
struct Set<'v, T> {
    #[serde(flatten)]
    about: About,
    vectors: &'v [T],
}

/// Writes the set `about` of `vectors` to `<dir>/<set>.json`, returning the file.
#[cfg(feature = "emulator")]
pub fn write<T: Serialize>(dir: &Path, about: About, vectors: &[T]) -> Result<PathBuf> {
    let file = dir.join(format!("{}.json", about.set));
    let mut text = serde_json::to_string_pretty(&Set { about, vectors })
        .context("writing the vectors as JSON")?;
    text.push('\n');
    fs::write(&file, text).with_context(|| format!("writing {}", file.display()))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manifest_names_the_exe_and_the_balance() {
        assert_eq!(
            expected_sha256("Sacred.exe"),
            Some("db9b86a44c8c1cc8c5ed9e3f427121ab8b4ed62760861e58995f10af9d971e74")
        );
        assert!(expected_sha256("bin/Balance.bin").is_some());
        assert_eq!(expected_sha256("sacred.exe"), None);
    }

    #[test]
    fn a_file_that_is_not_the_manifests_is_refused() {
        let dir = std::env::temp_dir().join(format!("opensacred-oracle-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("Sacred.exe"), b"not the game").unwrap();
        let error = checked(&dir, "Sacred.exe").unwrap_err().to_string();
        fs::remove_dir_all(&dir).unwrap();
        assert!(
            error.contains("is not the GOG Sacred Gold 2.28 file"),
            "{error}"
        );
    }

    #[test]
    fn the_vectors_never_go_into_the_repository() {
        let inside = repository().join("target").join("oracle-test-vectors");
        let error = prepare(&inside).unwrap_err().to_string();
        let _ = fs::remove_dir(&inside);
        assert!(error.contains("is inside the repository"), "{error}");
    }

    #[test]
    fn the_hash_is_lower_case_hexadecimal() {
        assert_eq!(
            sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
