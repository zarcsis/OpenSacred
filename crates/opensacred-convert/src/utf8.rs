//! The converter's text files are UTF-8 without a BOM with LF line ends, on every platform
//! (CLAUDE.md, "Code conventions"; #835): [`pretty`] lays RON out with LF, as `ron`'s default
//! writes `\r\n` on Windows, and [`normalized`] makes a text of the original's or of a master so.

use anyhow::{Context, Result};
use ron::ser::PrettyConfig;

/// The extensions of the masters that `build` writes as text, through [`normalized`].
const TEXT: [&str; 6] = ["ftl", "json", "luau", "ron", "toml", "txt"];

/// `ron`'s pretty layout, with LF line ends.
pub fn pretty() -> PrettyConfig {
    PrettyConfig::default().new_line("\n")
}

/// Whether the file at `path` is a text file, by its extension.
pub fn is_text(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, extension)| TEXT.contains(&extension))
}

/// `data` as UTF-8 without a BOM with LF line ends: the BOM dropped, CRLF and a lone CR made LF;
/// an error where it is not UTF-8.
pub fn normalized(data: &[u8]) -> Result<Vec<u8>> {
    let text = std::str::from_utf8(data).context("not UTF-8")?;
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    Ok(text.replace("\r\n", "\n").replace('\r', "\n").into_bytes())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde::Serialize;

    use super::*;
    use crate::testing::repo_root;

    #[derive(Serialize)]
    struct Nested {
        name: String,
        values: Vec<u32>,
        inner: Option<Box<Nested>>,
    }

    #[test]
    fn ron_is_laid_out_with_lf() {
        let value = Nested {
            name: "a".to_owned(),
            values: vec![1, 2],
            inner: Some(Box::new(Nested {
                name: "b".to_owned(),
                values: Vec::new(),
                inner: None,
            })),
        };
        let text = ron::ser::to_string_pretty(&value, pretty()).unwrap();
        assert!(text.lines().count() > 5, "{text}");
        assert!(!text.contains('\r'), "{text:?}");
    }

    #[test]
    fn a_text_loses_its_bom_and_cr_and_must_be_utf8() {
        assert_eq!(
            normalized(b"\xEF\xBB\xBFone\r\ntwo\rthree\n").unwrap(),
            b"one\ntwo\nthree\n"
        );
        assert_eq!(normalized("é\n".as_bytes()).unwrap(), "é\n".as_bytes());
        assert!(normalized(b"\xE9\r\n").is_err());
        assert!(is_text("ui/texts/credits.txt") && is_text("sound/sound.ron"));
        assert!(!is_text("world/sectors/0000-0000.sector") && !is_text("ui/fonts/antqs__.ttf"));
    }

    /// Every file that Git tracks as text, all but those it takes for binary, is UTF-8 without a
    /// BOM and without CR (#834).
    #[test]
    fn the_repositorys_text_files_are_utf8_with_lf() {
        let root = repo_root();
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["ls-files", "--eol", "-z"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let listing = String::from_utf8(output.stdout).unwrap();
        let mut checked = 0;
        let mut wrong = Vec::new();
        for record in listing.split('\0').filter(|r| !r.is_empty()) {
            let (info, path) = record.split_once('\t').unwrap();
            // `i/-text`: what Git stores as binary.
            if info.starts_with("i/-text") {
                continue;
            }
            // A file deleted from the working tree but not yet from the index.
            let Ok(data) = fs::read(root.join(path)) else {
                continue;
            };
            checked += 1;
            if data.starts_with(b"\xEF\xBB\xBF")
                || data.contains(&b'\r')
                || std::str::from_utf8(&data).is_err()
            {
                wrong.push(path.to_owned());
            }
        }
        assert!(checked > 300, "{checked} text files");
        assert!(wrong.is_empty(), "{wrong:?}");
    }
}
