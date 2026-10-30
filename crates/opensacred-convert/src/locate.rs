//! Finding the player's installation of Sacred Gold 2.28 (#1496, #1494): the GOG release by the
//! key that its installer writes into the registry, the Steam release by Steam's libraries, each
//! library's manifest of the game naming its directory under `steamapps\common`. `locate` checks
//! each place found against the manifests (`verify`) and takes the first that passes, GOG's
//! before Steam's; the other commands take the first place that holds a `Sacred.exe` when no
//! `--game-dir` is given and `OriginalGame/` is not there.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// The default `--game-dir`, the developer's installation in the repository.
pub const DEFAULT_GAME_DIR: &str = "OriginalGame";

/// GOG's id of Sacred Gold, the name of its registry key.
#[cfg(windows)]
const GOG_ID: &str = "1207658688";
/// Steam's id of Sacred Gold, in the name of its manifest in a library.
const STEAM_ID: &str = "12320";

/// The release of an installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Release {
    Gog,
    Steam,
}

impl Release {
    pub fn name(self) -> &'static str {
        match self {
            Release::Gog => "GOG",
            Release::Steam => "Steam",
        }
    }
}

/// A place where an installation may lie, and the release that put it there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Place {
    pub release: Release,
    pub dir: PathBuf,
}

/// The places that the registry and Steam's libraries name, GOG's first, each once.
pub fn places() -> Vec<Place> {
    let gog = gog_dirs().into_iter().map(|dir| Place {
        release: Release::Gog,
        dir,
    });
    let steam = steam_roots()
        .into_iter()
        .flat_map(|root| steam_dirs(&root))
        .map(|dir| Place {
            release: Release::Steam,
            dir,
        });
    let mut found: Vec<Place> = Vec::new();
    for place in gog.chain(steam) {
        if !found.iter().any(|seen| same_dir(&seen.dir, &place.dir)) {
            found.push(place);
        }
    }
    found
}

/// The installation directory to take for `given`: itself where it is there or was named on the
/// command line, else, for the default that is not there, the first place found that holds a
/// `Sacred.exe`, with what it found for the log.
pub fn resolve(given: PathBuf) -> (PathBuf, Option<Place>) {
    if given.exists() || given != Path::new(DEFAULT_GAME_DIR) {
        return (given, None);
    }
    match places()
        .into_iter()
        .find(|place| place.dir.join("Sacred.exe").is_file())
    {
        Some(place) => (place.dir.clone(), Some(place)),
        None => (given, None),
    }
}

/// Whether two directories are the same, by their names, case aside on Windows (the registry
/// writes GOG's in lower case) and their separators aside.
fn same_dir(a: &Path, b: &Path) -> bool {
    let key = |path: &Path| {
        let text = path.to_string_lossy().replace('\\', "/");
        let text = text.trim_end_matches('/').to_owned();
        if cfg!(windows) {
            text.to_lowercase()
        } else {
            text
        }
    };
    key(a) == key(b)
}

/// The directories that GOG's installer wrote into the registry, its 32-bit view's first.
#[cfg(windows)]
fn gog_dirs() -> Vec<PathBuf> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    [
        format!(r"SOFTWARE\WOW6432Node\GOG.com\Games\{GOG_ID}"),
        format!(r"SOFTWARE\GOG.com\Games\{GOG_ID}"),
    ]
    .iter()
    .filter_map(|key| registry_string(HKEY_LOCAL_MACHINE, key, "path"))
    .map(PathBuf::from)
    .collect()
}

#[cfg(not(windows))]
fn gog_dirs() -> Vec<PathBuf> {
    Vec::new()
}

/// Steam's own directories: the user's `SteamPath` and the machine's `InstallPath` on Windows,
/// `~/.local/share/Steam` and `~/.steam/steam` elsewhere.
#[cfg(windows)]
fn steam_roots() -> Vec<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    [
        (HKEY_CURRENT_USER, r"Software\Valve\Steam", "SteamPath"),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Valve\Steam",
            "InstallPath",
        ),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Valve\Steam", "InstallPath"),
    ]
    .iter()
    .filter_map(|&(root, key, value)| registry_string(root, key, value))
    // The user's `SteamPath` is written with forward slashes.
    .map(|text| PathBuf::from(text.replace('/', "\\")))
    .collect()
}

#[cfg(not(windows))]
fn steam_roots() -> Vec<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| vec![home.join(".local/share/Steam"), home.join(".steam/steam")])
        .unwrap_or_default()
}

/// A string value of the registry, none where the key or the value is not there.
#[cfg(windows)]
fn registry_string(root: winreg::HKEY, key: &str, value: &str) -> Option<String> {
    winreg::RegKey::predef(root)
        .open_subkey(key)
        .ok()?
        .get_value::<String, _>(value)
        .ok()
        .filter(|text| !text.trim().is_empty())
}

/// Sacred Gold's directory in each library of the Steam at `root` that holds its manifest: the
/// root's own library and those of its `steamapps\libraryfolders.vdf`.
fn steam_dirs(root: &Path) -> Vec<PathBuf> {
    let listed = std::fs::read_to_string(root.join("steamapps").join("libraryfolders.vdf"))
        .map(|text| libraries(&text))
        .unwrap_or_default();
    let mut found: Vec<PathBuf> = Vec::new();
    for library in std::iter::once(root.to_path_buf()).chain(listed) {
        let steamapps = library.join("steamapps");
        let manifest = steamapps.join(format!("appmanifest_{STEAM_ID}.acf"));
        let Some(dir) = std::fs::read_to_string(manifest)
            .ok()
            .and_then(|text| install_dir(&text))
        else {
            continue;
        };
        let dir = steamapps.join("common").join(dir);
        if !found.iter().any(|seen| same_dir(seen, &dir)) {
            found.push(dir);
        }
    }
    found
}

/// The libraries that Steam's `libraryfolders.vdf` lists: each entry's `path`.
pub fn libraries(text: &str) -> Vec<PathBuf> {
    let Some(vdf::Value::Map(root)) = vdf::parse(text) else {
        return Vec::new();
    };
    root.iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case("libraryfolders"))
        .flat_map(|(_, folders)| folders.entries())
        .filter_map(|(_, folder)| folder.get("path"))
        .map(PathBuf::from)
        .collect()
}

/// The directory under `steamapps\common` that a Steam app's manifest names, its `installdir`.
pub fn install_dir(text: &str) -> Option<String> {
    let vdf::Value::Map(root) = vdf::parse(text)? else {
        return None;
    };
    root.iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("AppState"))
        .and_then(|(_, state)| state.get("installdir"))
        .filter(|dir| !dir.is_empty())
        .map(str::to_owned)
}

/// Valve's KeyValues text (VDF) as Steam's libraries and manifests write it: quoted keys, each
/// with a quoted string or a block of further keys in braces, `\\` and `\"` escaped, `//`
/// comments.
mod vdf {
    /// A value: a string, or the keys of a block in their order.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Value {
        Text(String),
        Map(Vec<(String, Value)>),
    }

    impl Value {
        /// A block's keys and values, none of a string.
        pub fn entries(&self) -> &[(String, Value)] {
            match self {
                Value::Map(entries) => entries,
                Value::Text(_) => &[],
            }
        }

        /// The string of a block's key `name`, case aside.
        pub fn get(&self, name: &str) -> Option<&str> {
            self.entries()
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .and_then(|(_, value)| match value {
                    Value::Text(text) => Some(text.as_str()),
                    Value::Map(_) => None,
                })
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    enum Token {
        Text(String),
        Open,
        Close,
    }

    fn tokens(text: &str) -> Option<Vec<Token>> {
        let mut tokens = Vec::new();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '{' => tokens.push(Token::Open),
                '}' => tokens.push(Token::Close),
                '"' => {
                    let mut word = String::new();
                    loop {
                        match chars.next()? {
                            '"' => break,
                            '\\' => match chars.next()? {
                                'n' => word.push('\n'),
                                't' => word.push('\t'),
                                other => word.push(other),
                            },
                            other => word.push(other),
                        }
                    }
                    tokens.push(Token::Text(word));
                }
                '/' if chars.peek() == Some(&'/') => {
                    for c in chars.by_ref() {
                        if c == '\n' {
                            break;
                        }
                    }
                }
                c if c.is_whitespace() => {}
                // An unquoted word.
                c => {
                    let mut word = c.to_string();
                    while let Some(&next) = chars.peek() {
                        if next.is_whitespace() || matches!(next, '{' | '}' | '"') {
                            break;
                        }
                        word.push(next);
                        chars.next();
                    }
                    tokens.push(Token::Text(word));
                }
            }
        }
        Some(tokens)
    }

    /// The file's top keys as a block, none where it does not parse.
    pub fn parse(text: &str) -> Option<Value> {
        let tokens = tokens(text)?;
        let mut at = 0;
        let entries = block(&tokens, &mut at)?;
        (at == tokens.len()).then_some(Value::Map(entries))
    }

    /// The keys from `at` to the end of their block (a closing brace, taken) or of the file.
    fn block(tokens: &[Token], at: &mut usize) -> Option<Vec<(String, Value)>> {
        let mut entries = Vec::new();
        while let Some(token) = tokens.get(*at) {
            *at += 1;
            let key = match token {
                Token::Close => return Some(entries),
                Token::Text(key) => key.clone(),
                Token::Open => return None,
            };
            let value = match tokens.get(*at)? {
                Token::Text(text) => {
                    *at += 1;
                    Value::Text(text.clone())
                }
                Token::Open => {
                    *at += 1;
                    Value::Map(block(tokens, at)?)
                }
                Token::Close => return None,
            };
            entries.push((key, value));
        }
        Some(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Steam's `libraryfolders.vdf` as Steam writes it, two libraries, the second on another
    /// drive.
    const LIBRARIES: &str = r#""libraryfolders"
{
	"0"
	{
		"path"		"C:\\Games\\Steam"
		"label"		""
		"apps"
		{
			"12320"		"2844757257"
		}
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
		"apps"
		{
		}
	}
}
"#;

    /// An app's manifest, cut short.
    const MANIFEST: &str = r#""AppState"
{
	"appid"		"12320"
	"name"		"Sacred Gold"
	"installdir"		"Sacred Gold"
	"UserConfig"
	{
		"language"		"english"
	}
}
"#;

    #[test]
    fn steams_libraries_and_the_games_directory_read() {
        assert_eq!(
            libraries(LIBRARIES),
            [
                PathBuf::from(r"C:\Games\Steam"),
                PathBuf::from(r"D:\SteamLibrary")
            ]
        );
        assert_eq!(install_dir(MANIFEST).as_deref(), Some("Sacred Gold"));
        assert_eq!(install_dir(r#""AppState" { "installdir" "" }"#), None);
        // A file cut short or of unbalanced braces reads as nothing.
        assert!(libraries(r#""libraryfolders" { "0" { "path" "C:\\"#).is_empty());
        assert!(libraries("\"libraryfolders\" { } }").is_empty());
    }

    #[test]
    fn comments_escapes_and_unquoted_words() {
        let text = "// a comment\n\"a\" { b \"x \\\"y\\\"\" \"c\" { } }";
        let Some(vdf::Value::Map(root)) = vdf::parse(text) else {
            panic!("parsed");
        };
        assert_eq!(root.len(), 1);
        assert_eq!(root[0].1.get("b"), Some("x \"y\""));
        assert_eq!(root[0].1.get("B"), Some("x \"y\""));
        assert_eq!(root[0].1.get("c"), None);
    }

    /// A library that holds Sacred Gold's manifest gives its directory; the root's own library
    /// and a listed one that is the same count once.
    #[test]
    fn a_steam_library_with_the_games_manifest_gives_its_directory() {
        let root = crate::testing::scratch_dir("locate-steam");
        let steamapps = root.join("steamapps");
        std::fs::create_dir_all(&steamapps).unwrap();
        let listed = format!(
            "\"libraryfolders\" {{ \"0\" {{ \"path\" \"{}\" }} }}",
            root.display().to_string().replace('\\', "\\\\")
        );
        std::fs::write(steamapps.join("libraryfolders.vdf"), listed).unwrap();
        assert!(steam_dirs(&root).is_empty());
        std::fs::write(steamapps.join("appmanifest_12320.acf"), MANIFEST).unwrap();
        assert_eq!(
            steam_dirs(&root),
            [steamapps.join("common").join("Sacred Gold")]
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_same_directory_by_its_name() {
        assert!(same_dir(
            Path::new("C:/Games/Sacred Gold/"),
            Path::new(r"C:\Games\Sacred Gold")
        ));
        assert_eq!(
            same_dir(
                Path::new(r"c:\games\sacred gold"),
                Path::new(r"C:\Games\Sacred Gold")
            ),
            cfg!(windows)
        );
    }

    /// A `--game-dir` named on the command line is taken as it is, there or not.
    #[test]
    fn a_named_directory_is_taken_as_it_is() {
        let named = PathBuf::from("no-such-installation");
        assert_eq!(resolve(named.clone()), (named, None));
    }
}
