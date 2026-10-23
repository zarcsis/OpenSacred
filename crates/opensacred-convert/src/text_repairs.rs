//! Repairs of the texts (docs/02 §8) that `text` makes as it writes `global.ftl`, with words of
//! the project's own (#403, docs/decisions/2026-10-23-text-repairs.md). A repair names its strings
//! by id and the original's word by its length and SHA-256, never by its wording (docs/08 §3), so
//! it replaces only the word of 2.28 that it was written for; a string it names that holds no such
//! word is left as it is, with a warning.

use sha2::{Digest, Sha256};

/// A word of the original replaced wherever it stands in the strings that the repair names, a
/// compound's part too.
#[derive(Debug, Clone, Copy)]
pub struct Word {
    /// What the word is, for the summary and the warnings.
    pub what: &'static str,
    /// The strings that hold it.
    pub ids: &'static [u32],
    /// The original word's length in UTF-16 units.
    pub len: usize,
    /// The SHA-256 of the original word's UTF-16LE bytes, in hex.
    pub sha256: &'static str,
    /// The project's own word in its place.
    pub text: &'static str,
}

/// The repairs that `text` makes.
pub const WORDS: [Word; 1] = [GOBLIN_GOD];

/// The goblins' god, whom the goblins' and the orcs' lines invoke by a German name, a goblin's
/// form of the verb for stomping, which the English text leaves untranslated and no string of it
/// renders (docs/02 §8): 14 times in ten strings, twice in four of them, once as the first part of
/// a compound. Its English name takes the same form from the English verb.
const GOBLIN_GOD: Word = Word {
    what: "the goblins' god's name",
    ids: &[
        63_580_866,
        63_580_868,
        567_832_966,
        588_918_616,
        640_731_051,
        724_553_692,
        724_553_694,
        990_141_306,
        1_478_592_224,
        1_478_592_228,
    ],
    len: 7,
    sha256: "793472b294b97d9d774f37f9742f97d5c24e54841eda5759ea3aab8396ca6a81",
    text: "Stompa",
};

/// What [`apply`] did to a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// No repair names the string.
    None,
    /// Repaired: the string's new text and the number of words replaced.
    Repaired(String, usize),
    /// A string that a repair names whose text is not what the repair expects: left as it is.
    Mismatch(String),
}

/// Applies the repairs of `words` that name the string `id` to its `text`.
pub fn apply(words: &[Word], id: u32, text: &str) -> Outcome {
    let mut units: Vec<u16> = text.encode_utf16().collect();
    let mut replaced = 0;
    let mut named = false;
    for word in words.iter().filter(|w| w.ids.contains(&id)) {
        named = true;
        let found = replace(&mut units, word);
        if found == 0 {
            return Outcome::Mismatch(format!(
                "string {id} lacks {}, which its repair replaces; left as it is",
                word.what
            ));
        }
        replaced += found;
    }
    if !named {
        return Outcome::None;
    }
    match String::from_utf16(&units) {
        Ok(text) => Outcome::Repaired(text, replaced),
        Err(_) => Outcome::Mismatch(format!(
            "string {id} is not UTF-16 once repaired; left as it is"
        )),
    }
}

/// Whether `text` holds `word`'s original.
#[cfg(test)]
pub fn holds(word: &Word, text: &str) -> bool {
    let digest = digest(word);
    let units: Vec<u16> = text.encode_utf16().collect();
    units.windows(word.len).any(|span| is(span, &digest))
}

/// Replaces every span of `units` that is `word`'s original, from the start on; the number of
/// spans replaced.
fn replace(units: &mut Vec<u16>, word: &Word) -> usize {
    let digest = digest(word);
    let new: Vec<u16> = word.text.encode_utf16().collect();
    let mut out = Vec::with_capacity(units.len());
    let mut count = 0;
    let mut rest = units.as_slice();
    while let Some((&first, tail)) = rest.split_first() {
        match rest.get(..word.len) {
            Some(span) if is(span, &digest) => {
                out.extend_from_slice(&new);
                rest = rest.get(word.len..).unwrap_or_default();
                count += 1;
            }
            _ => {
                out.push(first);
                rest = tail;
            }
        }
    }
    *units = out;
    count
}

/// The bytes of `word`'s SHA-256.
fn digest(word: &Word) -> Vec<u8> {
    word.sha256
        .as_bytes()
        .chunks(2)
        .filter_map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
        .collect()
}

/// Whether the SHA-256 of `span`'s UTF-16LE bytes is `digest`.
fn is(span: &[u16], digest: &[u8]) -> bool {
    let bytes: Vec<u8> = span.iter().flat_map(|u| u.to_le_bytes()).collect();
    Sha256::digest(&bytes).as_slice() == digest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify::hex;

    /// A repair of the word "Grunt", written for these tests.
    fn grunt(ids: &'static [u32]) -> Word {
        let bytes: Vec<u8> = "Grunt".encode_utf16().flat_map(u16::to_le_bytes).collect();
        let sha256: &'static str = hex(&Sha256::digest(&bytes)).leak();
        Word {
            what: "the test's word",
            ids,
            len: 5,
            sha256,
            text: "Oink",
        }
    }

    #[test]
    fn a_word_is_replaced_wherever_it_stands() {
        let words = [grunt(&[7])];
        assert_eq!(
            apply(&words, 7, "Grunt said Grunt, Gruntfood!"),
            Outcome::Repaired("Oink said Oink, Oinkfood!".into(), 3)
        );
    }

    #[test]
    fn a_string_that_no_repair_names_is_left() {
        let words = [grunt(&[7])];
        assert_eq!(apply(&words, 8, "Grunt"), Outcome::None);
    }

    #[test]
    fn a_named_string_without_the_word_is_left_with_a_warning() {
        let words = [grunt(&[7])];
        let Outcome::Mismatch(warning) = apply(&words, 7, "grunt, Grun t") else {
            panic!("expected a mismatch");
        };
        assert!(
            warning.starts_with("string 7 lacks the test's word"),
            "{warning}"
        );
    }

    #[test]
    fn holds_finds_the_word_anywhere() {
        let word = grunt(&[]);
        assert!(holds(&word, "a Gruntfood"));
        assert!(!holds(&word, "grunt"));
    }

    #[test]
    fn the_table_names_each_string_once_and_hashes_whole() {
        for word in WORDS {
            let mut ids = word.ids.to_vec();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), word.ids.len(), "{}", word.what);
            assert_eq!(digest(&word).len(), 32, "{}", word.what);
        }
    }
}
