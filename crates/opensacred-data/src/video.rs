//! The converted cutscenes (docs/02 §10.3): a file of each under the video directory, named by the
//! name the game uses, lower case (`intro`, `act1`, …): the masters FFV1 and FLAC in Matroska, the
//! game's AV1 and Opus in WebM. The game names 16, of which the release ships 10; a missing one is
//! skipped. No index lists them (#828): a cutscene's file follows from its name ([`file`]).

/// The extension of the game's cutscenes, WebM.
pub const EXTENSION: &str = "webm";
/// The extension of the masters' cutscenes, Matroska.
pub const MASTER_EXTENSION: &str = "mkv";

/// `act1.webm`, the file of the cutscene `name` of the `extension`, relative to the video
/// directory.
pub fn file(name: &str, extension: &str) -> String {
    format!("{name}.{extension}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cutscenes_file_is_named_by_the_cutscene() {
        assert_eq!(file("act1", EXTENSION), "act1.webm");
        assert_eq!(file("intro", MASTER_EXTENSION), "intro.mkv");
    }
}
