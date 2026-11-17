//! `opensacred-docs`: development only, what keeps the documents and the comments in line with
//! the tree (#2793), with no game files and no dependency. The map of the code that
//! `docs/11-code-map.md` holds is made from each source file's own header ([`code_map`]), and
//! what the documents and the comments cite is looked up in the tree ([`broken_citations`]): the
//! files, the numbered sections, the sections and paragraphs cited by their titles, the
//! decisions' rows, and the modules that a note of `docs/re/` says implement it. The crate's
//! tests fail when the map is stale or a citation leads nowhere, so that CI refuses a change that
//! leaves either behind.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The map's path from the repository's root.
pub const MAP: &str = "docs/11-code-map.md";

/// The map's lines before its first crate.
const MAP_HEAD: &str = "\
# The map of the code

A line per source file of `crates/`: the first words of the file's own header, its `//!` doc comment or a shader's first comment.
The lines are generated, never written by hand: `UPDATE_CODE_MAP=1 cargo nextest run -p opensacred-docs` writes the map again, and the crate's test fails while the map and the headers disagree.
To find where a thing lives, search this file for its word, then read the header of the file it names: the header tells what the file holds and cites the notes of `docs/re/` it follows.
A file's line is changed by changing its header.
";

/// The directories that are never walked: what Git ignores, and the other checkouts.
const SKIPPED: [&str; 10] = [
    ".git",
    ".venv",
    "OriginalGame",
    "__pycache__",
    "assets",
    "assets-hd",
    "assets-src",
    "node_modules",
    "target",
    "worktrees",
];

/// The files whose text cites the tree, by their extension.
const TEXTS: [&str; 9] = ["java", "js", "md", "py", "rs", "toml", "txt", "wesl", "yml"];

/// The directories whose files are cited by their path from the root.
const ROOTS: [&str; 6] = [
    "crates/", "docs/", "tools/", ".claude/", ".github/", "release/",
];

/// The paths that the documents name and the tree lacks on purpose, each with its reason.
const ELSEWHERE: [(&str, &str); 11] = [
    (
        ".claude/worktrees",
        "the sessions' worktrees, which Git ignores",
    ),
    (
        ".claude/settings.local.json",
        "the developer's own settings, never committed",
    ),
    (
        "crates/opensacred-rules/tests/fight_properties.proptest-regressions",
        "written when a property's case fails, and there is none",
    ),
    (
        "tools/usage/session_cost.py",
        "the usage script's predecessor, in the Git history",
    ),
    ("tools/data/functions_eng.tsv", "a file of sacred-bild"),
    ("docs/RE_NOTES.md", "a file of sacred-bild"),
    ("docs/Display.md", "a file of another project (docs/09)"),
    ("docs/CHANGELOG.md", "a file of another project (docs/09)"),
    (
        "docs/tr1/CHANGELOG.md",
        "a file of another project (docs/09)",
    ),
    (
        "docs/tr2/CHANGELOG.md",
        "a file of another project (docs/09)",
    ),
    (
        ".claude/knowledge",
        "a directory of another project (docs/09)",
    ),
];

/// The repository's root: two directories above this crate.
#[must_use]
pub fn root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .nth(2)
        .unwrap_or(manifest)
        .to_path_buf()
}

/// Every file of the checkout at `root` but those of the [`SKIPPED`] directories, as its path
/// from the root with `/` between its parts, in the paths' order.
///
/// # Errors
///
/// A directory or an entry that cannot be read.
pub fn files(root: &Path) -> io::Result<Vec<String>> {
    let mut found = Vec::new();
    let mut pending = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, prefix)) = pending.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = format!("{prefix}{name}");
            if entry.file_type()?.is_dir() {
                if !SKIPPED.contains(&name.as_str()) {
                    pending.push((entry.path(), format!("{path}/")));
                }
            } else {
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
}

/// The first paragraph of a source file's header, on one line: its `//!` lines, or a shader's
/// `//` lines, up to the first that is empty or is not a header's.
#[must_use]
pub fn header(text: &str, marker: &str) -> String {
    let mut words = Vec::new();
    for line in text.lines() {
        match line.strip_prefix(marker).map(str::trim) {
            Some(body) if !body.is_empty() => words.push(body),
            _ => break,
        }
    }
    words.join(" ")
}

/// Where the first sentence of `paragraph` ends: at a full stop before a space and a capital or a
/// backtick, else at the paragraph's end.
fn sentence(paragraph: &str) -> &str {
    let mut rest = paragraph.char_indices().peekable();
    while let Some((at, c)) = rest.next() {
        if c == '.' && rest.peek().is_some_and(|&(_, next)| next == ' ') {
            let after = paragraph[at + 1..].trim_start();
            if after.starts_with(|next: char| next.is_ascii_uppercase() || next == '`') {
                return &paragraph[..at];
            }
        }
    }
    paragraph.strip_suffix('.').unwrap_or(paragraph)
}

/// `text` up to the first of `marks` that stands outside parentheses and before a space or the
/// end.
fn clause<'a>(text: &'a str, marks: &[char]) -> &'a str {
    let mut depth = 0_u32;
    let mut rest = text.char_indices().peekable();
    while let Some((at, c)) = rest.next() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0
                && marks.contains(&c)
                && rest.peek().is_none_or(|&(_, next)| next == ' ') =>
            {
                return &text[..at];
            }
            _ => {}
        }
    }
    text
}

/// `words` without a parenthesis at their end, where a header names its provenance.
fn bare(words: &str) -> &str {
    let words = words.trim();
    match words.rfind(" (") {
        Some(at) if words.ends_with(')') && !words[at + 2..].contains('(') => words[..at].trim(),
        _ => words,
    }
}

/// The words by which the map names a file: its header's first sentence up to its first colon or
/// semicolon, up to its first comma when that is longer than 80 characters, without the
/// parenthesis of its provenance at the end. A header that begins with a name alone
/// (`` `opensacred-rules`: the game's rules… ``) gives the name and the clause after it.
#[must_use]
pub fn title(paragraph: &str) -> String {
    let sentence = sentence(paragraph);
    let mut words = clause(sentence, &[':', ';']);
    if words.chars().count() > 80 {
        words = clause(words, &[',']);
    }
    let name = bare(words);
    let named = name.starts_with('`') && name.ends_with('`') && name.matches('`').count() == 2;
    match sentence[words.len()..].strip_prefix(": ") {
        Some(rest) if named => format!("{name}: {}", bare(clause(rest, &[':', ';', ',']))),
        _ => name.to_owned(),
    }
}

/// Whether the map has a line for the file at `path`: the Rust sources and the shaders of
/// `crates/`, but for the files of a module's tests alone, which carry no header.
fn mapped(path: &str) -> bool {
    path.starts_with("crates/")
        && (path.ends_with(".rs") || path.ends_with(".wesl"))
        && !path.ends_with("/tests.rs")
}

/// The map of the code: a section per crate, a line per source file with its header's title.
///
/// # Errors
///
/// A file that cannot be read, or a source file without a header: every file tells what it holds.
pub fn code_map(root: &Path) -> io::Result<String> {
    let mut map = String::from(MAP_HEAD);
    let mut current = "";
    let all = files(root)?;
    for path in all.iter().filter(|path| mapped(path)) {
        let name = path.split('/').nth(1).unwrap_or_default();
        if name != current {
            map.push_str(&format!("\n## `crates/{name}`\n\n"));
            current = name;
        }
        let text = fs::read_to_string(root.join(path))?;
        let marker = if path.ends_with(".rs") { "//!" } else { "//" };
        let title = title(&header(&text, marker));
        if title.is_empty() {
            return Err(io::Error::other(format!(
                "{path}: no header, a `{marker}` comment at its first line that tells what it holds"
            )));
        }
        map.push_str(&format!("- `{path}`: {title}\n"));
    }
    Ok(map)
}

/// A file's text as the citations are searched in it: of a Rust source or a shader the comment
/// lines alone, of the others every line; each without its comment's marker, on one line, so that
/// a citation wrapped over two lines reads whole.
fn prose(path: &str, text: &str) -> String {
    let code = path.ends_with(".rs") || path.ends_with(".wesl");
    let mut flat = String::with_capacity(text.len());
    for line in text.lines() {
        let line = line.trim_start();
        let body = ["//!", "///", "//", "#", "*"]
            .iter()
            .find_map(|marker| line.strip_prefix(marker));
        match body {
            Some(body) if code || !path.ends_with(".md") => flat.push_str(body.trim_start()),
            _ if code => continue,
            _ => flat.push_str(line),
        }
        flat.push(' ');
    }
    flat
}

/// A heading or a row's name as it is compared: without backticks, in lower case, its spaces
/// single.
fn plain(title: &str) -> String {
    title
        .replace('`', "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// What a document offers to a citation.
#[derive(Default)]
struct Document {
    /// The numbers of its numbered headings, `3` and `3.4`.
    numbers: BTreeSet<String>,
    /// Its headings' titles, each also without its number and without a parenthesis at its end,
    /// the names of its tables' rows (a decision is cited by its row's first cell), and the bold
    /// words that begin a paragraph or a list's item and name it.
    titles: BTreeSet<String>,
}

impl Document {
    fn read(text: &str) -> Self {
        let mut document = Self::default();
        for line in text.lines() {
            if let Some(heading) = line.strip_prefix('#') {
                let heading = heading.trim_start_matches('#').trim();
                let number = heading
                    .split(' ')
                    .next()
                    .unwrap_or_default()
                    .trim_end_matches('.');
                let numbered = !number.is_empty()
                    && number.chars().all(|c| c.is_ascii_digit() || c == '.')
                    && number.starts_with(|c: char| c.is_ascii_digit());
                let bare = if numbered {
                    document.numbers.insert(number.to_owned());
                    heading.split_once(' ').map_or("", |(_, rest)| rest)
                } else {
                    heading
                };
                for title in [heading, bare] {
                    document.titles.insert(plain(title));
                    if let Some(at) = title.rfind(" (").filter(|_| title.ends_with(')')) {
                        document.titles.insert(plain(&title[..at]));
                    }
                }
            } else if let Some(row) = line.strip_prefix("| ") {
                if let Some((name, _)) = row.split_once(" |") {
                    document.titles.insert(plain(name));
                }
            } else if let Some(lead) = lead(line) {
                document.titles.insert(plain(lead));
            }
        }
        document
    }

    /// Whether a heading or a row has `title`, whole or as its beginning.
    fn has_title(&self, title: &str) -> bool {
        let title = plain(title);
        self.titles.contains(&title) || self.titles.iter().any(|known| known.starts_with(&title))
    }
}

/// The bold words that begin a paragraph or a list's item, by which the notes name a passage
/// below a heading: `**The price** (…)`, `- **The burst** (…)`, `* **Looking at frames:** …`.
fn lead(line: &str) -> Option<&str> {
    let line = line.trim_start();
    let line = line
        .strip_prefix("- ")
        .or(line.strip_prefix("* "))
        .unwrap_or(line);
    let (lead, _) = line.strip_prefix("**")?.split_once("**")?;
    Some(lead.trim_end_matches(['.', ':']))
}

/// The start of the line by which a note of `docs/re/` names the code that implements it.
const IMPLEMENTED: &str = "Implemented in ";

/// The modules that the line `line` of the document at `path` says implement it and that are not
/// there, pushed onto `broken`: after [`IMPLEMENTED`], each crate in a code span is followed by
/// its modules in parentheses, ``in `opensacred-rules` (`fight`, `drops/quality`)``, and a module
/// is a source file or a directory of the crate's `src/` (a shader by its whole name).
fn implemented(path: &str, line: &str, all: &BTreeSet<&str>, broken: &mut Vec<String>) {
    let Some(mut rest) = line.strip_prefix(IMPLEMENTED) else {
        return;
    };
    while let Some(at) = rest.find("`opensacred-") {
        let named = &rest[at + 1..];
        let Some((name, after)) = named.split_once('`') else {
            return;
        };
        rest = after;
        let Some(listed) = after.strip_prefix(" (") else {
            continue;
        };
        let Some((modules, after)) = listed.split_once(')') else {
            return;
        };
        rest = after;
        for module in modules
            .split(',')
            .map(|module| module.trim().trim_matches('`'))
        {
            let source = format!("crates/{name}/src/{module}");
            let found = if module.contains('.') {
                all.contains(source.as_str())
            } else {
                all.contains(format!("{source}.rs").as_str()) || exists(&format!("{source}/"), all)
            };
            if !found {
                broken.push(format!("{path}: `{name}` has no module `{module}`"));
            }
        }
    }
}

/// Whether `c` may stand in a path that a text cites.
fn of_path(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | '-')
}

/// The documents by the names under which they are cited: each by its path, and those of
/// `docs/` that begin with a number by that number too, as `docs/03`.
fn documents(root: &Path, all: &[String]) -> io::Result<BTreeMap<String, Document>> {
    let mut documents = BTreeMap::new();
    for path in all.iter().filter(|path| path.ends_with(".md")) {
        let text = fs::read_to_string(root.join(path))?;
        if let Some(number) = path.strip_prefix("docs/").and_then(|name| name.get(..2))
            && number.chars().all(|c| c.is_ascii_digit())
            && path.as_bytes().get(7) == Some(&b'-')
        {
            documents.insert(format!("docs/{number}"), Document::read(&text));
        }
        documents.insert(path.clone(), Document::read(&text));
    }
    Ok(documents)
}

/// The citations of `text`, the prose of the file at `path`, that lead nowhere, pushed onto
/// `broken`.
fn check(
    path: &str,
    text: &str,
    all: &BTreeSet<&str>,
    documents: &BTreeMap<String, Document>,
    broken: &mut Vec<String>,
) {
    let mut at = 0;
    while at < text.len() {
        let rest = &text[at..];
        let before = text[..at].chars().next_back();
        let starts = before.is_none_or(|c| !of_path(c));
        let name = ROOTS
            .iter()
            .chain(&["PLAN.md", "CLAUDE.md", "README.md"])
            .find(|name| starts && rest.starts_with(**name));
        let Some(name) = name else {
            at += rest.chars().next().map_or(1, char::len_utf8);
            continue;
        };
        let length = rest.find(|c: char| !of_path(c)).unwrap_or(rest.len());
        let cited = rest[..length].trim_end_matches(['.', '/', '-']);
        at += cited.len().max(name.len());
        let after = &text[at..];
        // A pattern of paths, not a path: `docs/re/<subsystem>.md`, `crates/*`.
        if after.starts_with(['<', '*', '{', '$', '[']) || cited.contains("...") {
            continue;
        }
        if name.ends_with('/') && !exists(cited, all) {
            if !ELSEWHERE.iter().any(|(known, _)| *known == cited) {
                broken.push(format!("{path}: no file `{cited}`"));
            }
            continue;
        }
        let Some(document) = documents.get(cited) else {
            continue;
        };
        sections(path, cited, document, after, broken);
    }
}

/// Whether the tree has the file or the directory `cited`, or files that begin with it, as
/// `docs/03` stands for `docs/03-toolchain.md`.
fn exists(cited: &str, all: &BTreeSet<&str>) -> bool {
    all.range(cited..)
        .next()
        .is_some_and(|next| next.starts_with(cited))
}

/// The sections of `document` that the text `after` its name cites, by number (`§3.4`, `§3.4 and
/// §5`) or by title (`"The price"`, `§9 "The price"`), those that are not there pushed onto
/// `broken`.
fn sections(path: &str, cited: &str, document: &Document, after: &str, broken: &mut Vec<String>) {
    let mut rest = after.strip_prefix(')').unwrap_or(after);
    loop {
        let next = rest.trim_start_matches([',', ' ']);
        let next = next
            .strip_prefix("and ")
            .or(next.strip_prefix("or "))
            .unwrap_or(next);
        if let Some(numbered) = next.strip_prefix('§') {
            let numbered = numbered.trim_start();
            let length = numbered
                .find(|c: char| !c.is_ascii_digit() && c != '.')
                .unwrap_or(numbered.len());
            let number = numbered[..length].trim_end_matches('.');
            if number.is_empty() {
                return;
            }
            if !document.numbers.contains(number) {
                broken.push(format!("{path}: `{cited}` has no §{number}"));
            }
            rest = &numbered[number.len()..];
            // A range, `§9–§10`: its end is a section too.
            rest = rest.strip_prefix('–').unwrap_or(rest);
        } else if let Some(titled) = next.strip_prefix('"') {
            let Some(length) = titled.find('"').filter(|&length| length <= 100) else {
                return;
            };
            if !document.has_title(&titled[..length]) {
                broken.push(format!(
                    "{path}: `{cited}` has no section or row \"{}\"",
                    &titled[..length]
                ));
            }
            rest = &titled[length + 1..];
        } else {
            return;
        }
    }
}

/// What the documents and the comments of the checkout at `root` cite and the tree lacks, a line
/// each: a file of [`ROOTS`] that is not there, a numbered section that its document has not, a
/// titled section, a named paragraph or a decision's row that its document has not, a module that
/// a note of `docs/re/` names as its code and its crate has not.
///
/// # Errors
///
/// A file that cannot be read.
pub fn broken_citations(root: &Path) -> io::Result<Vec<String>> {
    let all = files(root)?;
    let documents = documents(root, &all)?;
    let known: BTreeSet<&str> = all.iter().map(String::as_str).collect();
    let mut broken = Vec::new();
    for path in &all {
        let extension = path.rsplit('.').next().unwrap_or_default();
        let name = path.rsplit('/').next().unwrap_or_default();
        // The tests of the tools name files of their own making.
        if !TEXTS.contains(&extension) || name.starts_with("test_") || path == MAP {
            continue;
        }
        let Ok(text) = fs::read_to_string(root.join(path)) else {
            continue;
        };
        check(path, &prose(path, &text), &known, &documents, &mut broken);
        if path.starts_with("docs/re/") {
            for line in text.lines() {
                implemented(path, line, &known, &mut broken);
            }
        }
    }
    broken.sort();
    broken.dedup();
    Ok(broken)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_header_is_its_first_paragraph_on_one_line() {
        let text = "//! The prices (#1273): what an item\n//! fetches. And more.\n//!\n//! Later.\nfn f() {}\n";
        assert_eq!(
            header(text, "//!"),
            "The prices (#1273): what an item fetches. And more."
        );
        assert_eq!(
            header("// A shader's\n// header.\n\n// More.\n", "//"),
            "A shader's header."
        );
        assert_eq!(header("fn f() {}\n", "//!"), "");
    }

    #[test]
    fn a_title_is_the_headers_first_words() {
        // Up to the colon, without the provenance.
        assert_eq!(
            title("The prices (#1273; docs/re/inventory.md, §9): what an item fetches."),
            "The prices"
        );
        // A full stop inside a name ends no sentence, and a path's colons no clause.
        assert_eq!(
            title("The rules' (`opensacred_rules::ways`) cells of docs/re/movement.md. Then."),
            "The rules' (`opensacred_rules::ways`) cells of docs/re/movement.md"
        );
        // A long first clause is cut at its first comma.
        let long = "The creatures in one another's way, as the client's test of a step has a creature in a cell stop another's step into it: the cells";
        assert_eq!(title(long), "The creatures in one another's way");
        // A short one keeps its comma.
        assert_eq!(
            title("`build`, the converter's stage B (#694): the game's assets."),
            "`build`, the converter's stage B"
        );
        // A name alone takes the clause after it.
        assert_eq!(
            title(
                "`opensacred-rules`: the game's rules with no engine (ADR-04), what the client decides."
            ),
            "`opensacred-rules`: the game's rules with no engine"
        );
        assert_eq!(
            title("`ui` subcommand: the screens → PNG."),
            "`ui` subcommand"
        );
        // A parenthesis in the middle stays.
        assert_eq!(
            title("The look of the armour (58, 1021) and steel: drawn."),
            "The look of the armour (58, 1021) and steel"
        );
    }

    #[test]
    fn a_document_offers_its_headings_and_rows() {
        let document = Document::read(
            "# Note\n## 3. Tools\n### 3.4 The `guard` rails (#12)\n| Decision | Ours |\n|---|---|\n| Bosses levelled | text |\n",
        );
        assert!(document.numbers.contains("3") && document.numbers.contains("3.4"));
        assert!(!document.numbers.contains("4"));
        assert!(document.has_title("The guard rails"));
        assert!(document.has_title("3.4 The `guard` rails (#12)"));
        assert!(document.has_title("Tools"));
        assert!(document.has_title("Bosses levelled"));
        assert!(!document.has_title("The rails"));
        // A paragraph or an item named in bold is cited as a section is.
        let named = Document::read(
            "## Health\n**The hero's death**, seen.\n- **The burst.** Then.\n* **Frames:** saved.\nPlain **words** in a line.\n",
        );
        assert!(named.has_title("The hero's death") && named.has_title("The burst"));
        assert!(named.has_title("Frames"));
        assert!(!named.has_title("words"));
    }

    #[test]
    fn broken_citations_are_found() {
        let mut documents = BTreeMap::new();
        documents.insert(
            "docs/03".to_owned(),
            Document::read("## 3. Tools\n### 3.4 Guardrails\n"),
        );
        documents.insert(
            "docs/re/fight.md".to_owned(),
            Document::read("## The hit\n"),
        );
        let all: BTreeSet<&str> = [
            "docs/03-toolchain.md",
            "docs/re/fight.md",
            "crates/a/src/lib.rs",
        ]
        .into();
        let cases = [
            (
                "see docs/03 §3.4 and §3, then docs/re/fight.md, \"The hit\".",
                0,
            ),
            ("see docs/03, §3.5.", 1),
            (
                "see docs/03 §3.4, §9 and docs/re/fight.md, \"The hit\" and \"The miss\".",
                2,
            ),
            (
                "the file crates/a/src/lib.rs and the directory crates/a/, not crates/b/src/lib.rs.",
                1,
            ),
            (
                "a pattern docs/re/<subsystem>.md, crates/* and tools/data/functions_eng.tsv of another.",
                0,
            ),
        ];
        for (text, count) in cases {
            let mut broken = Vec::new();
            check("x.md", text, &all, &documents, &mut broken);
            assert_eq!(broken.len(), count, "{text}: {broken:?}");
        }
    }

    #[test]
    fn a_notes_modules_are_looked_up() {
        let all: BTreeSet<&str> = [
            "crates/opensacred-rules/src/fight.rs",
            "crates/opensacred-rules/src/drops/quality.rs",
            "crates/opensacred-game/src/night.wesl",
            "crates/opensacred-game/src/tooltip/art.rs",
        ]
        .into();
        let line = "Implemented in `opensacred-rules` (`fight`, `drops/quality`, `minds`) and `opensacred-game` (`tooltip`, `night.wesl`, `skies.wesl`); the driver's side is `tools/original/host.py`.";
        let mut broken = Vec::new();
        implemented("docs/re/x.md", line, &all, &mut broken);
        assert_eq!(
            broken,
            [
                "docs/re/x.md: `opensacred-rules` has no module `minds`",
                "docs/re/x.md: `opensacred-game` has no module `skies.wesl`"
            ]
        );
        // Another line names no modules.
        implemented(
            "docs/re/x.md",
            "The crate `opensacred-rules` (`none`).",
            &all,
            &mut broken,
        );
        assert_eq!(broken.len(), 2);
    }

    #[test]
    fn prose_is_the_comments_of_a_source_and_every_line_of_a_document() {
        let source = "//! See docs/re/fight.md,\n//! \"The hit\".\nlet path = \"docs/none.md\"; // docs/03\n";
        assert_eq!(prose("a.rs", source), "See docs/re/fight.md, \"The hit\". ");
        assert_eq!(
            prose("a.md", "# Title\nSee `docs/03`.\n"),
            "# Title See `docs/03`. "
        );
        assert_eq!(
            prose("a.py", "# See docs/03\nx = 1\n"),
            "See docs/03 x = 1 "
        );
    }

    /// The map of `docs/11-code-map.md` is what the headers give. `UPDATE_CODE_MAP=1` writes it.
    #[test]
    fn the_code_map_is_current() {
        let root = root();
        let map = code_map(&root).unwrap();
        let path = root.join(MAP);
        if std::env::var_os("UPDATE_CODE_MAP").is_some() {
            fs::write(&path, &map).unwrap();
        }
        let kept = fs::read_to_string(&path).unwrap_or_default();
        assert!(
            kept == map,
            "{MAP} is not what the sources' headers give: run `UPDATE_CODE_MAP=1 cargo nextest run -p opensacred-docs` and commit it"
        );
    }

    /// Every file, numbered section and titled section that a document or a comment cites is
    /// there.
    #[test]
    fn the_citations_lead_somewhere() {
        let broken = broken_citations(&root()).unwrap();
        assert!(
            broken.is_empty(),
            "{} citations lead nowhere:\n{}",
            broken.len(),
            broken.join("\n")
        );
    }
}
