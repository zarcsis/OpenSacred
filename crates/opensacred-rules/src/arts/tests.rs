use opensacred_data::tables::{Spell, WeaponArt};

use super::*;

const SERAPHIM: u32 = 1;
const GLADIATOR: u32 = 2;
const WOOD_ELF: u32 = 5;
const VAMPIRESS_BY_DAY: u32 = 6;
const DAEMON: u32 = 9;

fn spell(id: u16, regeneration: [u16; 2], classes: u32) -> Spell {
    Spell {
        id,
        name: 0,
        description: 0,
        short_description: 0,
        icon: String::new(),
        sprites: [0; 3],
        launch: 1,
        regeneration,
        effects: [[0; 2]; 13],
        values: [[0; 2]; 3],
        caps: [0; 3],
        element: 0,
        classes,
        flags: 0,
    }
}

fn weapon_art(id: u16, regeneration: [f32; 2], classes: u32) -> WeaponArt {
    WeaponArt {
        id,
        name: 0,
        description: 0,
        short_description: 0,
        icon: String::new(),
        sprites: [0; 3],
        value_3c: 0,
        regeneration,
        distance: 130,
        launch: 1,
        factor: [0.75, 0.05],
        value_58: [0.0; 2],
        value_60: [0; 2],
        value_68: [0; 2],
        classes,
        flags: 1,
    }
}

/// Records as GOG 2.28's: the Seraphim's 18 and 1071, the Gladiator's 1057 and 1059, the Wood
/// Elf's 1061, the Vampiress's 1074 and 1039, the Daemon's 1092 and 1097, and the generic 1002.
fn tables() -> Tables {
    Tables {
        spells: vec![spell(18, [15, 9], 1)],
        weapon_arts: vec![
            weapon_art(1002, [10.0, 6.0], 0),
            weapon_art(1057, [5.0, 3.0], 2),
            weapon_art(1059, [10.0, 5.0], 2),
            weapon_art(1061, [5.0, 3.0], 0x10),
            weapon_art(1071, [10.0, 5.0], 1),
            weapon_art(1074, [5.0, 3.0], 0x20),
            weapon_art(1039, [5.0, 3.0], 0x20),
            weapon_art(1092, [8.0, 4.0], 0x0080_0000),
            weapon_art(1097, [20.0, 12.0], 0x0080_0000),
            weapon_art(2000, [20.0, -1.0], 0),
        ],
    }
}

fn entry(table: Table, art: u16, level: u8, bonus: u8) -> Entry {
    Entry {
        table,
        art,
        level,
        bonus,
        flags: 0,
        time: 0.0,
        factor: 0.0,
        remaining: 0.0,
    }
}

fn arts(entries: &[Entry]) -> Arts {
    let mut arts = Arts {
        entries: entries.to_vec(),
    };
    arts.times(&tables());
    arts
}

fn levels(arts: &Arts) -> Vec<(u16, u8)> {
    arts.entries.iter().map(|e| (e.art, e.level)).collect()
}

#[test]
fn a_time_is_the_records_with_half_of_what_the_bonus_adds() {
    let arts = arts(&[
        entry(Table::WeaponArts, 1071, 1, 2),
        entry(Table::Spells, 18, 2, 0),
        // A record whose time falls with the level: the bonus takes nothing off.
        entry(Table::WeaponArts, 2000, 2, 3),
        // An art of neither table keeps its time.
        Entry {
            time: 7.0,
            ..entry(Table::WeaponArts, 3000, 1, 0)
        },
    ]);
    let times: Vec<_> = arts.entries.iter().map(|e| (e.time, e.factor)).collect();
    // 1071: 15 at level 1, 25 at level 3, so 15 + (25 − 15)/2; 18: 15 + 9·2.
    assert_eq!(times, [(20.0, 1.0), (33.0, 1.0), (18.0, 1.0), (7.0, 1.0)]);
}

#[test]
fn a_creature_that_is_no_players_makes_an_art_it_lacks_at_a_fifth_of_its_level() {
    let arts = arts(&[entry(Table::WeaponArts, 1059, 3, 1)]);
    let goblin = Creature {
        kind: 42,
        level: 12,
        player: false,
    };
    let hero = Creature {
        kind: GLADIATOR,
        level: 12,
        player: true,
    };
    assert_eq!(arts.level_in_play(1059, hero), Some(4));
    assert_eq!(arts.level_in_play(1057, goblin), Some(3));
    assert_eq!(arts.level_in_play(1057, hero), None);
}

#[test]
fn the_generic_rundum_and_hardhit_are_the_classs_own() {
    let gladiator = Creature {
        kind: GLADIATOR,
        level: 1,
        player: true,
    };
    let wood_elf = Creature {
        kind: WOOD_ELF,
        ..gladiator
    };
    let held = arts(&[
        entry(Table::WeaponArts, 1057, 3, 0),
        entry(Table::WeaponArts, 1061, 2, 0),
    ]);
    assert_eq!(held.level_in_play(1061, gladiator), Some(3));
    assert_eq!(held.level_in_play(1061, wood_elf), Some(2));
    assert_eq!(own(1062, 9), 1091);
    assert_eq!(own(1061, 3), 1061);
    // `0x581D40` finds the class's own for the generic `Attacke`.
    let seraphim = arts(&[
        entry(Table::Spells, 18, 1, 0),
        entry(Table::WeaponArts, 1071, 1, 0),
    ]);
    assert_eq!(seraphim.index(1002), Some(1));
    assert_eq!(seraphim.index(1059), None);
}

#[test]
fn a_rune_teaches_its_art_at_level_1_and_raises_it_after() {
    let tables = tables();
    let mut arts = arts(&[entry(Table::WeaponArts, 1071, 1, 0)]);
    assert_eq!(arts.read_rune(18, SERAPHIM, &tables), Ok(()));
    // Added at level 0 with the time 15, it rises to level 1 and 24: the 9 s it grew are still
    // to run.
    let taught = *arts.get(18).unwrap();
    assert_eq!(
        (taught.table, taught.level, taught.flags),
        (Table::Spells, 1, TAUGHT)
    );
    assert_eq!((taught.time, taught.remaining), (24.0, 9.0));
    assert_eq!(arts.read_rune(18, SERAPHIM, &tables), Ok(()));
    let raised = arts.get(18).unwrap();
    assert_eq!(
        (raised.level, raised.time, raised.remaining),
        (2, 33.0, 18.0)
    );
    assert_eq!(arts.read_rune(1071, SERAPHIM, &tables), Ok(()));
    assert_eq!(levels(&arts), [(1071, 2), (18, 2)]);
    assert_eq!(arts.levels(), 4);
}

#[test]
fn a_rune_of_another_class_or_of_no_art_is_refused() {
    let tables = tables();
    let mut arts = arts(&[entry(Table::WeaponArts, 1071, 1, 0)]);
    let before = arts.clone();
    assert_eq!(
        arts.read_rune(1059, SERAPHIM, &tables),
        Err(Refusal::NotForClass)
    );
    assert_eq!(arts.read_rune(0, SERAPHIM, &tables), Err(Refusal::NoArt));
    // The generic `Attacke` is of no class.
    assert_eq!(
        arts.read_rune(1002, SERAPHIM, &tables),
        Err(Refusal::NotForClass)
    );
    assert_eq!(arts, before);
}

#[test]
fn a_level_stops_at_255() {
    let tables = tables();
    let mut arts = arts(&[entry(Table::WeaponArts, 1071, 255, 0)]);
    assert_eq!(
        arts.read_rune(1071, SERAPHIM, &tables),
        Err(Refusal::Highest)
    );
    assert_eq!(arts.entries[0].level, 255);
    // A bonus rises apart from the level, unmarked and with the time as it was.
    assert!(arts.raise(0, 4, false, true, SERAPHIM, &tables));
    let entry = arts.entries[0];
    assert_eq!((entry.level, entry.bonus, entry.flags), (255, 4, 0));
}

#[test]
fn the_vampiresss_runes_teach_both_her_forms() {
    let tables = tables();
    let mut arts = Arts::default();
    assert_eq!(arts.read_rune(1074, VAMPIRESS_BY_DAY, &tables), Ok(()));
    assert_eq!(levels(&arts), [(1074, 1), (1039, 1)]);
    assert!(arts.entries.iter().all(Entry::taught));
    assert_eq!(arts.read_rune(1074, VAMPIRESS_BY_DAY, &tables), Ok(()));
    assert_eq!(levels(&arts), [(1074, 2), (1039, 2)]);
    assert_eq!(counterpart(1039, VAMPIRESS_BY_DAY), 1074);
    assert_eq!(counterpart(1074, 7), 1039);
}

#[test]
fn the_daemons_runes_teach_her_pairs() {
    let tables = tables();
    let mut arts = Arts::default();
    assert_eq!(arts.read_rune(1092, DAEMON, &tables), Ok(()));
    assert_eq!(levels(&arts), [(1092, 1), (1097, 1)]);
    let times: Vec<_> = arts.entries.iter().map(|e| e.time).collect();
    assert_eq!(times, [12.0, 32.0]);
    assert_eq!(counterpart(1101, DAEMON), 1099);
}

#[test]
fn a_new_game_keeps_the_arts_that_no_rune_taught() {
    let template = [
        HeroArt {
            table: 2,
            id: 1059,
            level: 1,
            bonus: 0,
            flags: 0,
            time: 13.392_858,
            factor: 1.0,
            remaining: 0.0,
        },
        HeroArt {
            table: 2,
            id: 1057,
            level: 1,
            bonus: 0,
            flags: 1,
            time: 7.142_857,
            factor: 1.0,
            remaining: 0.0,
        },
    ];
    let arts = Arts::new_game(&template, &tables());
    assert_eq!(levels(&arts), [(1059, 1)]);
    // The time of the record, before any skill shortens it (#507).
    assert_eq!(arts.entries[0].time, 15.0);
    assert_eq!(arts.entries[0].slot(), 0);
}

/// The two tables of `OriginalGame/Sacred.exe`, as `opensacred-convert tables` writes them but
/// for the texts.
fn game_tables() -> Tables {
    let exe = game_file("Sacred.exe");
    let spells = opensacred_formats::arts::spells(&exe).unwrap();
    let weapon_arts = opensacred_formats::arts::weapon_arts(&exe).unwrap();
    Tables {
        spells: spells
            .iter()
            .map(|s| Spell {
                regeneration: s.regeneration,
                values: s.values,
                caps: s.caps,
                ..spell(s.head.id, s.regeneration, s.classes)
            })
            .collect(),
        weapon_arts: weapon_arts
            .iter()
            .map(|a| WeaponArt {
                factor: a.factor,
                ..weapon_art(a.head.id, a.regeneration, a.classes)
            })
            .collect(),
    }
}

fn game_file(path: &str) -> Vec<u8> {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../OriginalGame")
        .join(path);
    std::fs::read(&file).unwrap_or_else(|error| panic!("{}: {error}", file.display()))
}

/// Each class's hero at level 1 holds its template's arts that no rune taught, with the times of
/// their records: the Seraphim's 1071 15.0 s and the Gladiator's 1059 15.0 s, as the original's
/// new games give them before the skills shorten them (#506, #507).
#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn each_heros_new_game_holds_its_templates_arts() {
    let tables = game_tables();
    let mut heroes = Vec::new();
    for path in (0..8).map(|i| format!("Templates/Hero{i:02}.ptx")) {
        let amh = opensacred_formats::amh::Amh::parse(&game_file(&path)).unwrap();
        let class = amh.summary().unwrap().class;
        let template: Vec<HeroArt> = amh
            .arts()
            .unwrap()
            .iter()
            .map(|a| HeroArt {
                table: a.table,
                id: a.id,
                level: a.level,
                bonus: a.bonus,
                flags: a.flags,
                time: a.time,
                factor: a.factor,
                remaining: a.remaining,
            })
            .collect();
        let arts = Arts::new_game(&template, &tables);
        for entry in &arts.entries {
            let level = u32::from(entry.level);
            assert_eq!(
                f64::from(entry.time),
                time(&tables, entry.art, level).unwrap(),
                "{path}"
            );
            assert_eq!((entry.level, entry.factor, entry.remaining), (1, 1.0, 0.0));
        }
        let mut arts: Vec<u16> = arts.entries.iter().map(|e| e.art).collect();
        arts.sort_unstable();
        heroes.push((class, arts));
    }
    heroes.sort_unstable();
    assert_eq!(
        heroes,
        [
            (1, vec![1071]),
            (2, vec![1059]),
            (3, vec![5]),
            (4, vec![1066]),
            (5, vec![1062]),
            (6, vec![1022, 1023]),
            (8, vec![1080]),
            (9, vec![1092, 1097]),
        ]
    );
    let seraphim = Arts::new_game(
        &[HeroArt {
            table: 2,
            id: 1071,
            level: 1,
            bonus: 0,
            flags: 0,
            time: 0.0,
            factor: 0.0,
            remaining: 0.0,
        }],
        &tables,
    );
    assert_eq!(seraphim.entries[0].time, 15.0);
}
