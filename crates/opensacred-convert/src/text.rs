//! `text` subcommand: `scripts/us/global.res` → `<out>/text/en-US/global.ftl` (docs/02 §8). The
//! message `id-N` is the string whose id is `N`, the [`sacred_hash`] of its name: the game, its
//! tables and its scripts name strings by that hash, and the names of 10,158 of the 23,123 are not
//! shipped. The strings are written as they are but for the repairs of [`text_repairs`].
//!
//! [`sacred_hash`]: opensacred_formats::text::sacred_hash

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use opensacred_formats::text::GlobalRes;

use crate::text_repairs::{self, Outcome};
use crate::verify::read_game_file;

/// The language directory of the GOG installation and its Fluent locale.
pub const LANGUAGES: [(&str, &str); 1] = [("scripts/us/global.res", "en-US")];

pub fn output_dir(out: &Path) -> PathBuf {
    out.join("text")
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Summary {
    pub strings: usize,
    pub bytes: usize,
    /// The strings that [`text_repairs`] repaired, and the words it replaced in them.
    pub repaired_strings: usize,
    pub repaired_words: usize,
    /// The strings that [`text_repairs`] added.
    pub added_strings: usize,
    /// The repairs left undone, each with its string.
    pub warnings: Vec<String>,
}

pub fn convert(game_dir: &Path, out: &Path) -> Result<Summary> {
    let mut summary = Summary::default();
    for (source, locale) in LANGUAGES {
        let data = read_game_file(&game_dir.join(source))?;
        let res = GlobalRes::parse(&data).context(source)?;
        let mut ftl = format!(
            "# The strings of {source}, converted by opensacred-convert: message id-N is the string\n\
             # whose id is N, the hash of its name (docs/02 §8).\n\n"
        );
        for entry in res.entries() {
            let text = entry
                .text()
                .with_context(|| format!("{source}: string {} is not UTF-16", entry.id))?;
            let text = match text_repairs::apply(&text_repairs::WORDS, entry.id, &text) {
                Outcome::None => text,
                Outcome::Repaired(repaired, words) => {
                    summary.repaired_strings += 1;
                    summary.repaired_words += words;
                    repaired
                }
                Outcome::Mismatch(warning) => {
                    summary.warnings.push(format!("{source}: {warning}"));
                    text
                }
            };
            let _ = writeln!(ftl, "id-{} = {}", entry.id, pattern(&text));
            summary.strings += 1;
        }
        for word in &text_repairs::WORDS {
            for &id in word.ids.iter().filter(|&&id| res.get(id).is_none()) {
                summary.warnings.push(format!(
                    "{source}: string {id}, whose {} a repair replaces, is missing",
                    word.what
                ));
            }
        }
        let mut added = String::new();
        for string in &text_repairs::ADDED {
            let id = string.id();
            if res.get(id).is_some() {
                summary.warnings.push(format!(
                    "{source}: string {} ({id}), which a repair adds, is in the file; left as it is",
                    string.name
                ));
                continue;
            }
            let _ = writeln!(added, "id-{id} = {}", pattern(string.text));
            summary.added_strings += 1;
        }
        if !added.is_empty() {
            let _ = write!(
                ftl,
                "\n# Strings that {source} lacks, added by opensacred-convert's repairs (docs/02 §8).\n\n{added}"
            );
        }
        let dir = output_dir(out).join(locale);
        fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        let path = dir.join("global.ftl");
        fs::write(&path, &ftl).with_context(|| format!("writing {}", path.display()))?;
        summary.bytes += ftl.len();
    }
    Ok(summary)
}

/// A Fluent pattern that reads back as `text`: braces become string literals, and so do the
/// spaces at the ends of a line, which Fluent would trim, and the characters that start a variant
/// or an attribute at the start of a continued line; an empty line becomes an empty literal.
pub fn pattern(text: &str) -> String {
    let mut out = String::new();
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            out.push_str("\n    ");
        }
        if line.is_empty() {
            out.push_str("{\"\"}");
            continue;
        }
        let body = line.trim_start_matches(' ');
        let lead = &line[..line.len() - body.len()];
        let middle = body.trim_end_matches(' ');
        let trail = &body[middle.len()..];
        if !lead.is_empty() {
            let _ = write!(out, "{{\"{lead}\"}}");
        }
        for (k, c) in middle.chars().enumerate() {
            match c {
                '{' | '}' => {
                    let _ = write!(out, "{{\"{c}\"}}");
                }
                '[' | '*' | '.' if k == 0 && lead.is_empty() && index > 0 => {
                    let _ = write!(out, "{{\"{c}\"}}");
                }
                _ => out.push(c),
            }
        }
        if !trail.is_empty() {
            let _ = write!(out, "{{\"{trail}\"}}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use fluent_syntax::ast::{Entry, Expression, InlineExpression, PatternElement};

    use super::*;
    use crate::testing::{repo_root, scratch_dir};

    /// The messages of a Fluent resource, each value read back as plain text.
    fn read_back(ftl: &str) -> Vec<(String, String)> {
        let resource = fluent_syntax::parser::parse(ftl).unwrap_or_else(|(_, e)| panic!("{e:?}"));
        resource
            .body
            .iter()
            .filter_map(|entry| match entry {
                Entry::Message(m) => {
                    let mut text = String::new();
                    for element in &m.value.as_ref().unwrap().elements {
                        match element {
                            PatternElement::TextElement { value } => text.push_str(value),
                            PatternElement::Placeable {
                                expression:
                                    Expression::Inline(InlineExpression::StringLiteral { value }),
                            } => text.push_str(value),
                            other => panic!("{other:?}"),
                        }
                    }
                    Some((m.id.name.to_owned(), text))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn patterns_read_back_as_their_text() {
        let texts = [
            "Seraphim",
            "  two leading, three trailing   ",
            "braces {like} these",
            "a\nsecond line\n\n[not a variant]\n  indented",
            "tab\tand quote \" and backslash \\",
            "",
        ];
        let mut ftl = String::new();
        for (k, text) in texts.iter().enumerate() {
            let _ = writeln!(ftl, "id-{k} = {}", pattern(text));
        }
        let back = read_back(&ftl);
        assert_eq!(back.len(), texts.len());
        for ((id, text), expected) in back.iter().zip(texts) {
            assert_eq!(text, expected, "{id}");
        }
    }

    /// Every string of the game reads back from the Fluent file as it is in `global.res`, or as
    /// its repair makes it, and the added strings follow them: every repair holds, and no string
    /// keeps a word that one replaces.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn every_string_reads_back() {
        let game = repo_root().join("OriginalGame");
        let out = scratch_dir("text");
        let summary = convert(&game, &out).unwrap();
        assert_eq!(summary.strings, 23_123);
        assert_eq!(summary.warnings, Vec::<String>::new());
        // docs/02 §8: the goblins' god's name 14 times in ten strings, and the gameboxes' word
        // once in each of their eleven.
        assert_eq!((summary.repaired_strings, summary.repaired_words), (21, 25));
        assert_eq!(summary.added_strings, text_repairs::ADDED.len());
        let ftl = fs::read_to_string(output_dir(&out).join("en-US/global.ftl")).unwrap();
        let back = read_back(&ftl);
        let data = read_game_file(&game.join(LANGUAGES[0].0)).unwrap();
        let res = GlobalRes::parse(&data).unwrap();
        let (strings, added) = back.split_at(res.entries().len());
        let expected: Vec<(String, String)> = text_repairs::ADDED
            .iter()
            .map(|string| (format!("id-{}", string.id()), string.text.to_owned()))
            .collect();
        assert_eq!(added, expected.as_slice());
        for ((id, text), entry) in strings.iter().zip(res.entries()) {
            assert_eq!(id, &format!("id-{}", entry.id));
            let original = entry.text().unwrap();
            let expected = match text_repairs::apply(&text_repairs::WORDS, entry.id, &original) {
                Outcome::Repaired(repaired, _) => repaired,
                _ => original,
            };
            assert_eq!(text, &expected, "{id}");
            for word in &text_repairs::WORDS {
                assert!(!text_repairs::holds(word, text), "{id} keeps {}", word.what);
            }
        }
        fs::remove_dir_all(out).unwrap();
    }
}
