//! The converted sound (docs/02 §10): `sound.ron` holds the [`Sound`] index of the samples, the
//! profiles that name them, the music and the original program's names of the sound ids. The
//! masters are FLAC files under `samples/` and `music/`, the game's Ogg Opus files (#701). The
//! index names no file: a sample's is named by its id ([`sample_file`]) and a music file by its
//! original name ([`Music::file`]), so the masters' index is the game's (#827).

use serde::{Deserialize, Serialize};

/// The index file under the sound directory.
pub const INDEX: &str = "sound.ron";
/// The extension of the game's sound files, Ogg Opus.
pub const EXTENSION: &str = "opus";
/// The extension of the masters' sound files, FLAC.
pub const MASTER_EXTENSION: &str = "flac";

/// The sound index, `sound.ron`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sound {
    /// The original ids of the samples, which profiles and scripts name.
    pub samples: Vec<u32>,
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
    /// The original file's name without its extension, `MUSIC_WOOD01`.
    pub name: String,
}

impl Music {
    /// `music/MUSIC_WOOD01.opus`, the file of the `extension`, relative to `sound.ron`.
    pub fn file(&self, extension: &str) -> String {
        format!("music/{}.{extension}", self.name)
    }
}

/// `samples/01020.opus`, the file of the sample `id` of the `extension`, relative to `sound.ron`.
pub fn sample_file(id: u32, extension: &str) -> String {
    format!("samples/{id:05}.{extension}")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoundName {
    pub id: u32,
    pub name: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_samples_file_is_named_by_its_id_and_a_musics_by_its_name() {
        assert_eq!(sample_file(1020, MASTER_EXTENSION), "samples/01020.flac");
        assert_eq!(sample_file(2, EXTENSION), "samples/00002.opus");
        let wood = Music {
            id: Some(6560),
            name: "MUSIC_WOOD01".to_owned(),
        };
        assert_eq!(wood.file(MASTER_EXTENSION), "music/MUSIC_WOOD01.flac");
        assert_eq!(wood.file(EXTENSION), "music/MUSIC_WOOD01.opus");
    }
}
