//! The game's own texts (#1986): the words that the game shows of its own, not the original's
//! (`lines::Texts`, the converted `global.ftl`), in the Fluent files of the crate's
//! `text/<language>/`, embedded, as its shaders are, so that they show before any assets are
//! made. A message's text is its pattern's text and string literals; a name that no file has
//! shows as itself.

use std::collections::HashMap;
use std::sync::LazyLock;

use fluent_syntax::ast::{Entry, Expression, InlineExpression, PatternElement};

/// The files, in English: the keys' help's (`help`).
const FILES: [&str; 1] = [include_str!("../text/en-US/help.ftl")];

/// The texts of every file, by their messages' names.
static TEXTS: LazyLock<HashMap<&'static str, String>> =
    LazyLock::new(|| FILES.into_iter().flat_map(messages).collect());

/// The text of the message `name`, the name itself where no file has it.
pub fn text(name: &str) -> &str {
    TEXTS.get(name).map_or(name, String::as_str)
}

/// The messages of the Fluent file `ftl` and their texts. What it cannot parse and a message's
/// other placeables are left out; a test holds every file whole.
fn messages(ftl: &str) -> Vec<(&str, String)> {
    let resource = fluent_syntax::parser::parse(ftl).unwrap_or_else(|(resource, _)| resource);
    resource
        .body
        .into_iter()
        .filter_map(|entry| {
            let Entry::Message(message) = entry else {
                return None;
            };
            let mut text = String::new();
            for element in message.value?.elements {
                match element {
                    PatternElement::TextElement { value }
                    | PatternElement::Placeable {
                        expression: Expression::Inline(InlineExpression::StringLiteral { value }),
                    } => text.push_str(value),
                    PatternElement::Placeable { .. } => {}
                }
            }
            Some((message.id.name, text))
        })
        .collect()
}

/// Whether a file has the message `name`.
#[cfg(test)]
pub fn has(name: &str) -> bool {
    TEXTS.contains_key(name)
}

/// The messages' names of every file.
#[cfg(test)]
pub fn names() -> impl Iterator<Item = &'static str> {
    TEXTS.keys().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_file_parses_whole_into_plain_messages() {
        for file in FILES {
            let resource =
                fluent_syntax::parser::parse(file).unwrap_or_else(|(_, e)| panic!("{e:?}"));
            for entry in &resource.body {
                match entry {
                    Entry::Message(message) => {
                        let value = message.value.as_ref().unwrap();
                        assert!(
                            value.elements.iter().all(|element| matches!(
                                element,
                                PatternElement::TextElement { .. }
                                    | PatternElement::Placeable {
                                        expression: Expression::Inline(
                                            InlineExpression::StringLiteral { .. }
                                        ),
                                    }
                            )),
                            "{}",
                            message.id.name
                        );
                        assert!(message.attributes.is_empty(), "{}", message.id.name);
                    }
                    Entry::Comment(_) | Entry::GroupComment(_) | Entry::ResourceComment(_) => {}
                    other => panic!("{other:?}"),
                }
            }
        }
        // No name in two places.
        let count: usize = FILES.into_iter().map(|file| messages(file).len()).sum();
        assert_eq!(TEXTS.len(), count);
    }

    #[test]
    fn a_message_is_its_text_and_a_name_unknown_itself() {
        let messages = messages("a = one {\" \"}two\nb = {$x}three\n# c\nd =\n");
        assert_eq!(
            messages,
            [("a", "one  two".to_owned()), ("b", "three".to_owned())]
        );
        assert_eq!(text("help-title"), "Keys");
        assert_eq!(text("help-no-such-message"), "help-no-such-message");
    }
}
