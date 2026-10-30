//! The assets converted for a player (#1494): where they lie, and the mark that the converter
//! writes into them last, by which the game knows them whole and of its own version.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The mark's file in the assets' directory, RON of [`Converted`].
pub const MARK: &str = "converted.ron";

/// The game's directory under the user's data directory.
const DIRECTORY: &str = "OpenSacred";

/// What the converter writes into the assets as its last step (`opensacred-convert install`,
/// #1497): the version of the converter that made them, which is the game's of the same release,
/// and the release of the installation they were made from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Converted {
    /// `opensacred-convert --version`'s, from `git describe`.
    pub version: String,
    /// `gog` or `steam`.
    pub release: String,
}

/// The game's directory in the user's data directory: `%LOCALAPPDATA%\OpenSacred` on Windows,
/// `$XDG_DATA_HOME/OpenSacred` (`~/.local/share/OpenSacred`) elsewhere.
pub fn data_dir() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        env_dir("LOCALAPPDATA")
    } else {
        env_dir("XDG_DATA_HOME")
            .or_else(|| env_dir("HOME").map(|home| home.join(".local").join("share")))
    };
    base.map(|base| base.join(DIRECTORY))
}

/// The player's assets: `assets` in [`data_dir`].
pub fn assets_dir() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("assets"))
}

/// The absolute directory in the environment variable `name`.
fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}
