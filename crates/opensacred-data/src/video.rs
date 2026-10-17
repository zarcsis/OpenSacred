//! The converted cutscenes (docs/02 §10.3): `video.ron` holds the [`Video`] index of the AV1 +
//! Opus WebM files under the video directory.

use serde::{Deserialize, Serialize};

/// The index file under the video directory.
pub const INDEX: &str = "video.ron";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Video {
    pub movies: Vec<Movie>,
}

/// A cutscene. The game names 16, of which the release ships 10; a missing one is skipped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Movie {
    /// The name the game uses, lower case: `intro`, `act1`, ….
    pub name: String,
    /// The file, relative to `video.ron`.
    pub file: String,
}
