//! The converted sound (docs/02 §10): `sound.ron` holds the [`Sound`] index of the samples, the
//! profiles that name them, the music and the original program's names of the sound ids. Samples
//! are FLAC (from the WAVs) or MP3 files under `samples/`, the music MP3 files under `music/`.

use serde::{Deserialize, Serialize};

/// The index file under the sound directory.
pub const INDEX: &str = "sound.ron";

/// The sound index, `sound.ron`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sound {
    /// Every sample by its original id, which profiles and scripts name.
    pub samples: Vec<SampleEntry>,
    pub profiles: Vec<Profile>,
    /// The music, jingles and ambient loops.
    pub music: Vec<Music>,
    /// The names of the sound ids in the original program, in the order of its table: every
    /// sample's and the music's, `SOUND_FX_ANIMAL_BAT01`, `SOUND_FX_MUSIC_WOOD01`, ….
    pub names: Vec<SoundName>,
}

/// A music file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Music {
    /// The sound id that the `music` profile names it by, matched by the file's name
    /// (`MUSIC_WOOD01.mp3`, `SOUND_FX_MUSIC_WOOD01`); none for the two files the program does not
    /// name.
    pub id: Option<u32>,
    /// The file, relative to `sound.ron`.
    pub file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoundName {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleEntry {
    pub id: u32,
    /// The file, relative to `sound.ron`.
    pub file: String,
}

/// A sound profile: the sample variants of each of its 8 event groups, of which the game picks
/// one at random.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    /// The original slot, which the game's tables name.
    pub slot: u32,
    /// `music`, `footsteps_hero_male`, `weapon_bow`, ….
    pub name: String,
    /// Sample ids per group; 102 ids of the game's profiles have no sample (docs/02 §10.2).
    pub groups: Vec<Vec<u32>>,
}
