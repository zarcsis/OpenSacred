//! The fight's formulas and the game's rolls against the vectors that `opensacred-oracle` records
//! from the developer's own `Sacred.exe` (docs/03 §3.4, docs/04 §2.3): the original's functions
//! run in the emulator, the rolls with `rand()` answering a script that the rules' `Script` dice
//! answer alike (#1433),
//! their inputs and outputs in JSON files outside the repository. Like the other tests that need
//! the game, they run locally: first `cargo run --release -p opensacred-oracle --features
//! emulator`, which writes the vectors to `OPENSACRED_VECTORS`, else to
//! `%LOCALAPPDATA%\OpenSacred\vectors` on Windows and `$XDG_DATA_HOME/OpenSacred/vectors`
//! elsewhere. The most health's also reads the balance of `OriginalGame/bin/Balance.bin`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use opensacred_data::tables::{Balance, BalanceValue, BalanceVariable};
use opensacred_formats::bin_tables::{self, BALANCE_KEYS, Kind};
use opensacred_rules::drops::between;
use opensacred_rules::fight::{
    Rules, creature_earns, dealt, exponent, hero_health, hit_chance, level_end, level_of,
    most_health,
};
use opensacred_rules::spawning::Script;
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
/// tables` names them; the tables of its tail, which the fight's formulas do not read, left out.
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
        ..Balance::default()
    }
}

/// `0x5658F0`: the most health of a base at a level, of a hero's type without the level's factor
/// and of another with it and bronze's `ProzHP` (#408).
#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn the_most_health_is_the_originals() {
    let vectors: Vec<MostHealth> = vectors("most_health");
    let rules = Rules::of(Some(&balance()));
    for v in &vectors {
        let most = if v.hero {
            hero_health(v.base, v.level, &[], &rules)
        } else {
            most_health(v.base, v.level, &[], 1.0, &rules)
        };
        assert_eq!(
            most, v.most,
            "{:?} at level {}, a hero's type: {}",
            v.base, v.level, v.hero
        );
    }
    assert!(!vectors.is_empty(), "no vectors of the most health");
}

#[derive(Deserialize)]
struct Between {
    least: i32,
    most: i32,
    rolls: Vec<u32>,
    value: i32,
    taken: usize,
}

/// `0x443B20`: the game's roll between two bounds, `rand()` answering a script, and how many of
/// its values the roll took (#1433).
#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn a_roll_between_two_takes_the_originals_values() {
    let vectors: Vec<Between> = vectors("between");
    for v in vectors {
        let mut script = Script::new(v.rolls.clone());
        let value = between(i64::from(v.least), i64::from(v.most), &mut script);
        assert_eq!(
            (value, script.taken()),
            (i64::from(v.value), v.taken),
            "R({}, {}) of {:?}",
            v.least,
            v.most,
            v.rolls
        );
    }
}

/// Whether a divisor of the weight `0x43EEE0` divides, beyond the double ±0.001.
fn divides(divisor: f32) -> bool {
    f64::from(divisor).abs() > 0.001
}

/// The properties that `tests/fight_properties.rs` asserts on random inputs, asserted on the
/// original's own outputs (#1428): a property that a vector breaks is none of the original's.
#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn the_fights_properties_hold_on_the_originals_vectors() {
    // The levels' ends rise, each a whole hundred, and the levels' spans grow.
    let mut ends: Vec<LevelEnd> = vectors("level_end");
    ends.sort_by_key(|v| v.level);
    for pair in ends.windows(2) {
        assert!(
            pair[0].end < pair[1].end,
            "the end of level {}",
            pair[0].level
        );
    }
    for triple in ends.windows(3) {
        assert_eq!(
            triple[0].end % 100,
            0,
            "the end of level {}",
            triple[0].level
        );
        assert!(
            triple[1].end - triple[0].end < triple[2].end - triple[1].end,
            "the span of level {}",
            triple[1].level
        );
    }
    let level_at_end: HashMap<u64, u32> = ends.iter().map(|v| (v.end, v.level)).collect();
    let first = ends[0].end;
    let top = &ends[ends.len() - 1];
    // An experience at or above the first level's end gives its level whatever the level held,
    // below it the level held; its end gives the next level, a point below it the level; and a
    // creature's experience answers a level only above its own, of its points.
    let mut by_experience: HashMap<u64, u32> = HashMap::new();
    for v in vectors::<LevelOf>("level_of") {
        if v.experience < first {
            assert_eq!(v.level, v.held, "the experience {}", v.experience);
        } else {
            let level = *by_experience.entry(v.experience).or_insert(v.level);
            assert_eq!(
                v.level, level,
                "the experience {} at {}",
                v.experience, v.held
            );
        }
        if let Some(&level) = level_at_end.get(&v.experience)
            && level < top.level
        {
            assert_eq!(v.level, level + 1, "the end of level {level}");
        }
        if let Some(&level) = level_at_end.get(&(v.experience + 1))
            && level >= 2
        {
            assert_eq!(v.level, level, "below the end of level {level}");
        }
        if v.experience < top.end {
            let mut points = 0;
            assert_eq!(
                creature_earns(&mut points, v.held, v.experience),
                (v.level > v.held).then_some(v.level),
                "the experience {} earned at {}",
                v.experience,
                v.held
            );
        }
    }
    // A hit chance lies below 1, rises with the attack and falls with a defence that divides; it
    // is 0.9999 against one that does not.
    let chances: Vec<Chance> = vectors("hit_chance");
    for v in &chances {
        assert!(
            (0.0..1.0).contains(&v.chance),
            "{} against {}",
            v.attack,
            v.defence
        );
        if !divides(v.defence) {
            assert!(
                (v.chance - 0.9999).abs() <= 1e-6,
                "{} against nothing",
                v.attack
            );
        }
    }
    for a in &chances {
        for b in &chances {
            if a.defence == b.defence && divides(a.defence) && a.attack < b.attack {
                assert!(
                    a.chance <= b.chance,
                    "{} and {} against {}",
                    a.attack,
                    b.attack,
                    a.defence
                );
            }
            if a.attack == b.attack && divides(a.defence) && a.defence < b.defence {
                assert!(
                    a.chance >= b.chance,
                    "{} against {} and {}",
                    a.attack,
                    a.defence,
                    b.defence
                );
            }
        }
    }
    // The damage let through lies within the damage, none of none; against no resistance all but a
    // ten thousandth of it, and at no level above its share `d²/(d + r)`; it falls as a resistance
    // that divides rises, rises with the exponent, and with the levels above up to 98.
    let dealts: Vec<Dealt> = vectors("dealt");
    for v in &dealts {
        let (damage, resistance) = (f64::from(v.damage), f64::from(v.resistance));
        assert!(
            (0.0..=damage).contains(&v.dealt),
            "{} against {}",
            v.damage,
            v.resistance
        );
        if !divides(v.resistance) {
            let least = damage * (1.0 - 1.0 / 10_001.0) * (1.0 - 1e-6);
            assert!(v.dealt >= least, "{} against nothing", v.damage);
        } else if v.above == 0 {
            let share = damage * damage / (damage + resistance);
            assert!(
                (v.dealt - share).abs() <= 1e-5 * share.max(1.0),
                "{} against {} at no level above",
                v.damage,
                v.resistance
            );
        }
    }
    for a in &dealts {
        for b in &dealts {
            if a.damage != b.damage {
                continue;
            }
            if a.above == b.above && divides(a.resistance) && a.resistance < b.resistance {
                assert!(
                    a.dealt >= b.dealt,
                    "{} against {} and {}",
                    a.damage,
                    a.resistance,
                    b.resistance
                );
            }
            if a.resistance == b.resistance && a.above < b.above && b.above <= 98 {
                assert!(
                    a.dealt <= b.dealt,
                    "{} against {} at {} and {} above",
                    a.damage,
                    a.resistance,
                    a.above,
                    b.above
                );
            }
            if a.resistance == b.resistance && exponent(a.above) < exponent(b.above) {
                assert!(
                    a.dealt <= b.dealt,
                    "{} at {} and {} above",
                    a.damage,
                    a.above,
                    b.above
                );
            }
        }
    }
}
