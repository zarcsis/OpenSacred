//! Repairs of the texts (docs/02 §8) that `text` makes as it writes `global.ftl`, with words of
//! the project's own (#403, docs/decisions/2026-10-23-text-repairs.md). A repair names its strings
//! by id and the original's word by its length and SHA-256, never by its wording (docs/08 §3), so
//! it replaces only the word of 2.28 that it was written for; a string it names that holds no such
//! word is left as it is, with a warning. A string the file lacks is added under the hash of its
//! name, unless the file holds it.

use opensacred_formats::text::sacred_hash;
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

/// The repairs that `text` makes: the goblins' god's name, and each gamebox's name.
pub const WORDS: [Word; 12] = [
    GOBLIN_GOD,
    // 5150, `953`: a starburst on black.
    gamebox(&[77_105_259], "Starburst Game Box"),
    // 5151, `954`: cartoon card players on red.
    gamebox(&[77_105_260], "Card Players Game Box"),
    // 5152, `955`: a cartoon clown amid a jumble of colours.
    gamebox(&[77_105_261], "Clown Game Box"),
    // 5153, `956`: a golden sword over a battle.
    gamebox(&[77_105_262], "Golden Sword Game Box"),
    // 5154, `957`: a dark armoured demon on red.
    gamebox(&[77_105_263], "Red Demon Game Box"),
    // 5155, `958`: a football on green.
    gamebox(&[77_105_264], "Green Football Game Box"),
    // 5156, `959`: a footballer's legs and ball on white.
    gamebox(&[77_105_265], "White Football Game Box"),
    // 5157, `960`: cartoon flying machines in a blue sky.
    gamebox(&[77_105_266], "Flying Machines Game Box"),
    // 5158, `961`: a red splash on black.
    gamebox(&[77_105_267], "Red Splash Game Box"),
    // 5159, `962`: a glowing maze on violet.
    gamebox(&[77_105_268], "Neon Maze Game Box"),
    // 5160, `963`: an airship sketched on copper.
    gamebox(&[77_105_372], "Airship Game Box"),
];

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

/// The name of a gamebox, the items 5150 to 5160, in place of the one generic word of 7 units
/// that their eleven strings hold whole, the record's internal name as well (#686,
/// docs/decisions/2026-10-25-gamebox-names.md): a name that tells the box by its art, the box of
/// another game, without naming that game. The string's id is the hash of the item's number
/// (docs/02 §6.2), and its model is the item's `+0x70`, 953 to 963 in order.
const fn gamebox(ids: &'static [u32], text: &'static str) -> Word {
    Word {
        what: "a gamebox's generic name",
        ids,
        len: 7,
        sha256: "9aea6dd9c07271992acc7b49b6ad2b2b5d140c0612c6c67a5172b8df37806739",
        text,
    }
}

/// A string that the file lacks, added under the hash of its name.
#[derive(Debug, Clone, Copy)]
pub struct Added {
    /// The string's name, whose hash is its id: a creature type's number in decimal (docs/02 §6.2),
    /// or a key that the scripts name whole.
    pub name: &'static str,
    /// The project's own text.
    pub text: &'static str,
}

/// The strings that `text` adds: the names of the waters' shoals of fish, the only creatures of a
/// type without a name that the scripts or the sectors' runner make (#684,
/// docs/decisions/2026-10-23-fish-names.md), and the lines of the four quest-log keys that the
/// scripts' `QuestBook` records name and the file lacks (#687,
/// docs/decisions/2026-10-25-quest-log-lines.md). The runner makes the fish without a script
/// (docs/re/spawning.md, "The waters' crabs and fish"), where the scripts give every creature of
/// the other such types that they make a name of its own.
pub const ADDED: [Added; 6] = [
    // `fisc_01.grn`: three small fish and a middle-sized one.
    Added {
        name: "164",
        text: "Shoal of Fish",
    },
    // `fisc_02.grn`: three small fish.
    Added {
        name: "165",
        text: "Shoal of Small Fish",
    },
    // The main quest 61's element of the air, taken before the quest asks for it, as its siblings
    // of fire, earth and water note theirs.
    Added {
        name: "HQ_6_2_1_Log_preQuest",
        text: "An odd element, light and restless as a gust of wind!",
    },
    // The heading of the quest 7607, the last of three insects to kill, as its siblings 7605 and
    // 7606 have theirs.
    Added {
        name: "NQ_7607_LOG_HEADER",
        text: "The Last of the Brood",
    },
    // The quests 9009 and 9010 lost, beside the generic lines of a quest completed and failed.
    Added {
        name: "NQ_LOG_QLOSE",
        text: "Quest lost.",
    },
    // The quest 901 won: the abducted women brought home.
    Added {
        name: "R09NQ01_Log_Qsieg",
        text: "The women are home again, safe in their village.",
    },
];

/// Every string that `text` adds, its name and its text: [`ADDED`], then the names of the items
/// without one (`item_names`, #685).
pub fn added() -> Vec<(String, &'static str)> {
    ADDED
        .iter()
        .map(|string| (string.name.to_owned(), string.text))
        .chain(
            crate::item_names::ITEM_NAMES
                .iter()
                .map(|&(item, text)| (item.to_string(), text)),
        )
        .collect()
}

/// A string's id by its name: the hash of its name (docs/02 §8).
pub fn id_of(name: &str) -> u32 {
    sacred_hash(name.as_bytes())
}

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
    fn the_added_strings_are_named_once() {
        let added = added();
        let mut ids: Vec<u32> = added.iter().map(|(name, _)| id_of(name)).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), ADDED.len() + 300);
        assert_eq!(ids.len(), added.len());
        // A creature type's name is the hash of its number (docs/02 §6.2): type 37 is id 5818.
        assert_eq!(id_of("37"), 5818);
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
