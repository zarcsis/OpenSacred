//! The fight's formulas against the vectors that `opensacred-oracle` records from the developer's
//! own `Sacred.exe` (docs/03 §3.4, docs/04 §2.3): the original's functions run in the emulator,
//! their inputs and outputs in JSON files outside the repository. Like the other tests that need
//! the game, they run locally: first `cargo run --release -p opensacred-oracle --features
//! emulator`, which writes the vectors to `OPENSACRED_VECTORS`, else to
//! `%LOCALAPPDATA%\OpenSacred\vectors` on Windows and `$XDG_DATA_HOME/OpenSacred/vectors`
//! elsewhere. The most health's also reads the balance of `OriginalGame/bin/Balance.bin`.

use std::fs;
use std::path::{Path, PathBuf};

use opensacred_data::tables::{Balance, BalanceValue, BalanceVariable};
use opensacred_formats::bin_tables::{self, BALANCE_KEYS, Kind};
use opensacred_rules::fight::{
    Rules, dealt, exponent, hero_health, hit_chance, level_end, level_of, most_health,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;

/// The manifest of the reference installation, whose `Sacred.exe` the vectors must come from.
const MANIFEST: &str = include_str!("../../../docs/manifest-gog-2.28.txt");

/// A set of vectors as the oracle writes it.
#[derive(Deserialize)]
struct Set<T> {
    sacred_exe: String,
    vectors: Vec<T>,
}

/// The vectors' directory, as the oracle chooses it.
fn directory() -> PathBuf {
    let absolute = |name: &str| {
        std::env::var_os(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    if let Some(dir) = absolute("OPENSACRED_VECTORS") {
        return dir;
    }
    let base = if cfg!(windows) {
        absolute("LOCALAPPDATA")
    } else {
        absolute("XDG_DATA_HOME").or_else(|| absolute("HOME").map(|h| h.join(".local/share")))
    };
    base.unwrap_or_else(|| panic!("no directory for the vectors: set OPENSACRED_VECTORS"))
        .join("OpenSacred")
        .join("vectors")
}

/// The vectors of the set `set`, made from the manifest's `Sacred.exe`.
fn vectors<T: DeserializeOwned>(set: &str) -> Vec<T> {
    let file = directory().join(format!("{set}.json"));
    let text = fs::read_to_string(&file).unwrap_or_else(|error| {
        panic!(
            "{}: {error}; record the vectors first: cargo run --release -p opensacred-oracle \
             --features emulator (docs/03 §3.4)",
            file.display()
        )
    });
    let set: Set<T> =
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", file.display()));
    let exe = MANIFEST
        .lines()
        .find_map(|line| line.strip_suffix("  Sacred.exe"))
        .unwrap_or_else(|| panic!("the manifest names no Sacred.exe"));
    assert_eq!(
        set.sacred_exe,
        exe,
        "{}: not made from the GOG 2.28 Sacred.exe",
        file.display()
    );
    assert!(!set.vectors.is_empty(), "{}: no vectors", file.display());
    set.vectors
}

#[derive(Deserialize)]
struct LevelEnd {
    level: u32,
    end: u64,
}

/// `0x57F6E0`: the experience at which each level ends.
#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn the_levels_end_where_the_originals_do() {
    let vectors: Vec<LevelEnd> = vectors("level_end");
    assert_eq!(vectors.len(), 216);
    for v in vectors {
        assert_eq!(level_end(v.level), v.end, "level {}", v.level);
    }
}

#[derive(Deserialize)]
struct LevelOf {
    experience: u64,
    held: u32,
    level: u32,
}

/// `0x57F8D0`: the level of an experience about each level's end, by the level held.
#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn an_experience_gives_the_originals_level() {
    let vectors: Vec<LevelOf> = vectors("level_of");
    for v in vectors {
        assert_eq!(
            level_of(v.experience, v.held),
            v.level,
            "the experience {} at the level {}",
            v.experience,
            v.held
        );
    }
}

#[derive(Deserialize)]
struct Chance {
    attack: f32,
    defence: f32,
    chance: f32,
}

/// `0x43EEE0` as the strike's resolution takes the hit chance: `A/(A + D)`, 0.9999 against a
/// defence within ±0.001 (#406).
#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn a_strike_hits_by_the_originals_chance() {
    let vectors: Vec<Chance> = vectors("hit_chance");
    for v in vectors {
        let chance = hit_chance(v.attack, v.defence);
        assert!(
            (chance - v.chance).abs() <= 1e-6,
            "{} against {}: {chance}, the original's {}",
            v.attack,
            v.defence,
            v.chance
        );
    }
}

#[derive(Deserialize)]
struct Dealt {
    damage: f32,
    resistance: f32,
    above: u32,
    dealt: f64,
}

/// `0x43EEE0` as the strike's resolution takes each kind's damage against its resistance:
/// `d·(1 − 1/(1 + d/r)^k)`, `k` by the attacker's levels above the defender's (#406, #407).
#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn a_resistance_lets_the_originals_damage_through() {
    let vectors: Vec<Dealt> = vectors("dealt");
    for v in vectors {
        let through = f64::from(dealt(v.damage, v.resistance, exponent(v.above)));
        assert!(
            (through - v.dealt).abs() <= 1e-5 * v.dealt.abs().max(1.0),
            "{} against {}, {} levels above: {through}, the original's {}",
            v.damage,
            v.resistance,
            v.above,
            v.dealt
        );
    }
}

#[derive(Deserialize)]
struct MostHealth {
    base: [u8; 6],
    level: u32,
    hero: bool,
    most: u32,
}

/// The balance of `OriginalGame/bin/Balance.bin`, its variables named as `opensacred-convert
/// tables` names them.
fn balance() -> Balance {
    let file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../OriginalGame/bin/Balance.bin")
        .canonicalize()
        .unwrap_or_else(|error| panic!("OriginalGame/bin/Balance.bin: {error}"));
    let data = fs::read(&file).unwrap_or_else(|error| panic!("{}: {error}", file.display()));
    let read = bin_tables::balance(&data).unwrap_or_else(|error| panic!("{error}"));
    let variables = BALANCE_KEYS
        .iter()
        .map(|key| {
            let word = read
                .word(key)
                .unwrap_or_else(|| panic!("no word for {}", key.name));
            BalanceVariable {
                name: key.name.to_owned(),
                offset: key.offset,
                value: match key.kind {
                    Kind::F32 => BalanceValue::F32(f32::from_bits(word)),
                    Kind::I32 => BalanceValue::I32(i32::from_le_bytes(word.to_le_bytes())),
                },
            }
        })
        .collect();
    Balance {
        variables,
        image: read.image,
        rest: read.rest,
    }
}

/// `0x5658F0`: the most health of a base at a level, of a hero's type without the level's factor
/// and of another with it and bronze's `ProzHP` (#408).
#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn the_most_health_is_the_originals() {
    let vectors: Vec<MostHealth> = vectors("most_health");
    let balance = balance();
    let rules = Rules::of(Some(&balance));
    let last = balance
        .f32("HP_s5")
        .unwrap_or_else(|| panic!("the balance has no HP_s5"));
    let mut compared = 0;
    for v in &vectors {
        // Above the level factor's last point the original goes on along its last segment
        // (#418), and a most that the factor takes below 1 it keeps at 0 (#417).
        let level = f32::from(u16::try_from(v.level).unwrap_or(u16::MAX));
        if !v.hero && (level > last || v.most == 0) {
            continue;
        }
        compared += 1;
        let most = if v.hero {
            hero_health(v.base, v.level)
        } else {
            most_health(v.base, v.level, &rules)
        };
        assert_eq!(
            most, v.most,
            "{:?} at level {}, a hero's type: {}",
            v.base, v.level, v.hero
        );
    }
    assert!(compared >= vectors.len() / 2, "only {compared} compared");
}
