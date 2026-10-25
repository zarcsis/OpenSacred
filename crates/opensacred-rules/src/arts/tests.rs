use std::sync::LazyLock;

use opensacred_data::tables::{Spell, WeaponArt};

use super::*;

/// The original's curves of the skills that shorten the times.
static ORIGINAL: LazyLock<Shortening> = LazyLock::new(Shortening::default);

/// A creature of type `kind` without a skill that shortens a time.
fn context(kind: u32, tables: &Tables) -> Context<'_> {
    Context {
        tables,
        kind,
        skills: &[],
        shortening: &ORIGINAL,
    }
}

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
    assert_eq!(arts.read_rune(18, &context(SERAPHIM, &tables)), Ok(()));
    // Added at level 0 with the time 15, it rises to level 1 and 24: the 9 s it grew are still
    // to run.
    let taught = *arts.get(18).unwrap();
    assert_eq!(
        (taught.table, taught.level, taught.flags),
        (Table::Spells, 1, TAUGHT)
    );
    assert_eq!((taught.time, taught.remaining), (24.0, 9.0));
    assert_eq!(arts.read_rune(18, &context(SERAPHIM, &tables)), Ok(()));
    let raised = arts.get(18).unwrap();
    assert_eq!(
        (raised.level, raised.time, raised.remaining),
        (2, 33.0, 18.0)
    );
    assert_eq!(arts.read_rune(1071, &context(SERAPHIM, &tables)), Ok(()));
    assert_eq!(levels(&arts), [(1071, 2), (18, 2)]);
    assert_eq!(arts.levels(), 4);
}

#[test]
fn a_rune_of_another_class_or_of_no_art_is_refused() {
    let tables = tables();
    let mut arts = arts(&[entry(Table::WeaponArts, 1071, 1, 0)]);
    let before = arts.clone();
    assert_eq!(
        arts.read_rune(1059, &context(SERAPHIM, &tables)),
        Err(Refusal::NotForClass)
    );
    assert_eq!(
        arts.read_rune(0, &context(SERAPHIM, &tables)),
        Err(Refusal::NoArt)
    );
    // The generic `Attacke` is of no class.
    assert_eq!(
        arts.read_rune(1002, &context(SERAPHIM, &tables)),
        Err(Refusal::NotForClass)
    );
    assert_eq!(arts, before);
}

#[test]
fn a_level_stops_at_255() {
    let tables = tables();
    let mut arts = arts(&[entry(Table::WeaponArts, 1071, 255, 0)]);
    assert_eq!(
        arts.read_rune(1071, &context(SERAPHIM, &tables)),
        Err(Refusal::Highest)
    );
    assert_eq!(arts.entries[0].level, 255);
    // A bonus rises apart from the level, unmarked and with the time as it was.
    assert!(arts.raise(0, 4, false, true, &context(SERAPHIM, &tables)));
    let entry = arts.entries[0];
    assert_eq!((entry.level, entry.bonus, entry.flags), (255, 4, 0));
}

#[test]
fn the_vampiresss_runes_teach_both_her_forms() {
    let tables = tables();
    let mut arts = Arts::default();
    assert_eq!(
        arts.read_rune(1074, &context(VAMPIRESS_BY_DAY, &tables)),
        Ok(())
    );
    assert_eq!(levels(&arts), [(1074, 1), (1039, 1)]);
    assert!(arts.entries.iter().all(Entry::taught));
    assert_eq!(
        arts.read_rune(1074, &context(VAMPIRESS_BY_DAY, &tables)),
        Ok(())
    );
    assert_eq!(levels(&arts), [(1074, 2), (1039, 2)]);
    assert_eq!(counterpart(1039, VAMPIRESS_BY_DAY), 1074);
    assert_eq!(counterpart(1074, 7), 1039);
}

#[test]
fn the_daemons_runes_teach_her_pairs() {
    let tables = tables();
    let mut arts = Arts::default();
    assert_eq!(arts.read_rune(1092, &context(DAEMON, &tables)), Ok(()));
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
    let arts = Arts::new_game(&template, &context(GLADIATOR, &tables()));
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

/// The skills of the heroes' types of `OriginalGame/PAK/Creature.pak`, at level 1, by type.
fn game_skills() -> Vec<(u32, Vec<(u8, u32)>)> {
    let creatures = opensacred_formats::tables::creatures(&game_file("PAK/Creature.pak")).unwrap();
    creatures
        .iter()
        .filter(|c| (1..=9).contains(&c.id))
        .map(|c| {
            let record = opensacred_data::tables::Creature {
                id: c.id,
                class: c.class,
                flags: c.flags,
                experience: c.experience,
                base: c.base,
                skills: c.skills,
                skills_extra: c.skills_extra,
                speed: c.speed,
                bonus: c.bonus,
                bonus_value: c.bonus_value,
                bonus_class: c.bonus_class,
                damping: c.damping,
            };
            let base = crate::fight::Base::of(&record);
            (c.id, crate::fight::skills(&base, 1, true))
        })
        .collect()
}

/// Each class's hero at level 1 holds its template's arts that no rune taught (#506), with the
/// times its template holds: the records' times shortened by its skills, as the original's
/// recomputation left them (the Seraphim's 1071 15.0 s; the Gladiator's 1059 15/1.12 by his skill
/// 25, #507); every entry of the templates, taught or not, recomputes to its time.
#[test]
#[ignore = "needs the game files in OriginalGame/"]
fn each_heros_new_game_holds_its_templates_arts() {
    let tables = game_tables();
    let skills = game_skills();
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
        let context = Context {
            skills: &skills.iter().find(|(id, _)| *id == class).unwrap().1,
            ..context(class, &tables)
        };
        let mut all = Arts {
            entries: template.iter().filter_map(Entry::of_template).collect(),
        };
        all.recompute(&context);
        for (entry, held) in all.entries.iter().zip(&template) {
            assert_eq!(
                entry.time.to_bits(),
                held.time.to_bits(),
                "{path} {}",
                held.id
            );
        }
        let arts = Arts::new_game(&template, &context);
        for entry in &arts.entries {
            assert_eq!((entry.level, entry.factor, entry.remaining), (1, 1.0, 0.0));
        }
        let mut arts: Vec<(u16, f32)> = arts.entries.iter().map(|e| (e.art, e.time)).collect();
        arts.sort_by_key(|&(art, _)| art);
        heroes.push((class, arts));
    }
    heroes.sort_by_key(|&(class, _)| class);
    assert_eq!(
        heroes,
        [
            (1, vec![(1071, 15.0)]),
            (2, vec![(1059, 13.392_858)]),
            (3, vec![(5, 5.0)]),
            (4, vec![(1066, 7.142_857)]),
            (5, vec![(1062, 8.0)]),
            (6, vec![(1022, 30.0), (1023, 2.678_571_5)]),
            (8, vec![(1080, 15.0)]),
            (9, vec![(1092, 12.0), (1097, 32.0)]),
        ]
    );
}

/// The Gladiator's two skills at level 1 (2 and 25, `Creature.pak`), the second shortening his
/// weapon arts by its curve's 12 per cent.
const GLADIATORS_SKILLS: [(u8, u32); 2] = [(2, 1), (25, 1)];

#[test]
fn a_skill_shortens_the_times_of_its_arts() {
    let tables = tables();
    let gladiator = Context {
        skills: &GLADIATORS_SKILLS,
        ..context(GLADIATOR, &tables)
    };
    let mut arts = Arts {
        entries: vec![
            Entry {
                remaining: 14.0,
                ..entry(Table::WeaponArts, 1059, 1, 0)
            },
            entry(Table::Spells, 18, 1, 0),
        ],
    };
    arts.recompute(&gladiator);
    // 15/1.12, what is still to run held to it; the spell 18 is no art of the skill 25's.
    let shortened = arts.entries[0];
    assert_eq!(
        (shortened.time, shortened.remaining),
        (13.392_858, 13.392_858)
    );
    assert_eq!(arts.entries[1].time, 24.0);
    // A time is held at 1 s at least.
    let mut short = Arts {
        entries: vec![Entry {
            time: 1.05,
            ..entry(Table::WeaponArts, 3000, 1, 0)
        }],
    };
    short.shorten(&[(25, 1)], &ORIGINAL);
    assert_eq!(short.entries[0].time, 1.05);
    short.entries[0].art = 1059;
    short.shorten(&[(25, 1)], &ORIGINAL);
    assert_eq!(short.entries[0].time, 1.0);
}

/// The Gladiator learns 1057 as runes teach it, as the original showed it with its driver (#507):
/// 7.1 s at level 1 and 9.8 s at level 2, the growth 7.143 → 9.821 still to run.
#[test]
fn the_gladiators_rune_grows_a_shortened_time() {
    let tables = tables();
    let gladiator = Context {
        skills: &GLADIATORS_SKILLS,
        ..context(GLADIATOR, &tables)
    };
    let mut arts = Arts::default();
    assert!(arts.teach(Table::WeaponArts, 1057, 1, &gladiator));
    let learnt = *arts.get(1057).unwrap();
    assert_eq!((learnt.time, learnt.remaining), (7.142_857, 2.678_571_2));
    arts.entries[0].remaining = 0.0;
    assert!(arts.teach(Table::WeaponArts, 1057, 1, &gladiator));
    let raised = arts.get(1057).unwrap();
    assert_eq!((raised.level, raised.time), (2, 9.821_428));
    assert!(
        (raised.remaining - 2.678_571).abs() < 1e-5,
        "{}",
        raised.remaining
    );
}

#[test]
fn a_use_starts_every_art_of_its_kind_each_held_to_its_time() {
    let mut arts = arts(&[
        entry(Table::WeaponArts, 1071, 1, 0),
        entry(Table::WeaponArts, 1057, 1, 0),
        entry(Table::Spells, 18, 1, 0),
    ]);
    assert!(arts.ready(1071));
    let time = arts.time_of(1071, &tables());
    arts.start(time, Table::WeaponArts, None, 0);
    let remaining: Vec<f32> = arts.entries.iter().map(|e| e.remaining).collect();
    // 1071's 15 s, 1057's own 8 s; the spell of the other kind stays ready.
    assert_eq!(remaining, [15.0, 8.0, 0.0]);
    assert!(!arts.ready(1071) && arts.ready(18) && arts.ready(1002));
    // The items' and sets' flag of the weapon arts takes the time to 0.8 of it.
    let mut quicker = arts.clone();
    quicker.entries.iter_mut().for_each(|e| e.remaining = 0.0);
    quicker.start(15.0, Table::WeaponArts, None, QUICKER_WEAPON_ARTS);
    assert_eq!(quicker.entries[0].remaining, 12.0);
    quicker.start(15.0, Table::Spells, None, QUICKER_WEAPON_ARTS);
    assert_eq!(quicker.entries[2].remaining, 15.0);
    // An art the creature lacks adds its record's time at level 0.
    assert_eq!(arts.time_of(1059, &tables()), 10.0);
}

#[test]
fn the_traps_and_the_bites_start_their_own() {
    let mut arts = Arts {
        entries: [1047, 1067, 1030]
            .map(|art| Entry {
                time: 20.0,
                ..entry(Table::WeaponArts, art, 1, 0)
            })
            .to_vec(),
    };
    // A trap's use starts the traps, and an art of the other group, not the Attacke.
    arts.start(10.0, Table::WeaponArts, Some(1047), 0);
    let remaining = |arts: &Arts| arts.entries.iter().map(|e| e.remaining).collect::<Vec<_>>();
    assert_eq!(remaining(&arts), [10.0, 0.0, 10.0]);
    // The Attacke's use starts the arts of no group alone.
    arts.start(5.0, Table::WeaponArts, Some(1067), 0);
    assert_eq!(remaining(&arts), [10.0, 5.0, 10.0]);
    // Without a group, every art of the kind.
    arts.start(5.0, Table::WeaponArts, None, 0);
    assert_eq!(remaining(&arts), [15.0, 10.0, 15.0]);
}

#[test]
fn the_regeneration_runs_each_kind_at_its_speed() {
    let mut arts = arts(&[
        entry(Table::WeaponArts, 1071, 1, 0),
        entry(Table::Spells, 18, 1, 0),
        entry(Table::WeaponArts, 1085, 1, 0),
    ]);
    for entry in &mut arts.entries {
        entry.time = 30.0;
        entry.remaining = 10.0;
    }
    // 1085 starts as a spell but runs at the weapon arts' speed.
    assert_eq!(arts.entries[2].kind(), Table::Spells);
    let speeds = Speeds {
        spells: 0.5,
        weapon_arts: 1.25,
    };
    arts.regenerate(4.0, speeds);
    let remaining: Vec<f32> = arts.entries.iter().map(|e| e.remaining).collect();
    assert_eq!(remaining, [5.0, 8.0, 5.0]);
    arts.regenerate(10.0, speeds);
    assert_eq!(arts.entries[0].remaining, 0.0);
    assert!(arts.ready(1071));
    assert!(arts.ready(1085));
    assert!(!arts.ready(18));
}

#[test]
fn the_speeds_follow_the_items_and_the_regeneration_attributes() {
    let speeds = Speeds::of([0.1, 0.0], 0.0, 20.0, 50.0, SERAPHIM);
    assert_eq!(
        speeds,
        Speeds {
            spells: 1.32,
            weapon_arts: 1.5
        }
    );
    // The Dwarf's spells at the weapon arts' speed.
    assert_eq!(Speeds::of([0.1, 0.0], 0.0, 20.0, 50.0, 8).spells, 1.5);
    // None below 0.
    assert_eq!(Speeds::of([-3.0, 0.0], 0.0, 0.0, 0.0, 1).spells, 0.0);
    assert_eq!(Speeds::default().spells, 1.0);
}

#[test]
fn the_classes_own_arts_strike_by_their_style() {
    assert_eq!(style(1071), Some(Style::Attacke));
    assert_eq!(style(1059), Some(Style::Attacke));
    assert_eq!(style(1057), Some(Style::Rundum));
    assert_eq!(style(1069), Some(Style::Rundum));
    assert_eq!(style(1058), Some(Style::Hardhit));
    assert_eq!(style(1070), Some(Style::Hardhit));
    assert_eq!((style(1013), style(18)), (None, None));
}

#[test]
fn an_arts_factor_rises_by_its_level_and_halves_with_two_weapons() {
    let tables = tables();
    let attacke = tables.weapon_arts.iter().find(|a| a.id == 1071).unwrap();
    // The Seraphim's `Attacke`: 0.75 + 0.05·L, 0.8 at level 1 and 0.9 at level 3.
    assert_eq!(factor(attacke, 1), 0.8);
    assert_eq!(factor(attacke, 3), 0.9);
    assert_eq!(held_factor(0.8, false), 0.8);
    assert_eq!(held_factor(0.8, true), 0.4);
    assert_eq!(held_factor(0.0, true), 0.5);
}

fn candidate(id: u32, distance: u32, hostile: bool) -> Candidate {
    Candidate {
        id,
        distance,
        reach: 130,
        hostile,
    }
}

#[test]
fn an_attacke_strikes_its_target_or_the_next_within_its_reach() {
    let others = [
        candidate(7, 100, false),
        candidate(8, 180, true),
        candidate(9, 120, true),
    ];
    // Its target within the reach and 45.
    assert_eq!(
        attacke_target(Some(candidate(5, 175, true)), &others),
        Some(5)
    );
    // Gone beyond them, or dead: the first hostile creature within them, in the list's order.
    assert_eq!(
        attacke_target(Some(candidate(5, 176, true)), &others),
        Some(9)
    );
    assert_eq!(
        attacke_target(Some(candidate(5, 10, false)), &others),
        Some(9)
    );
    assert_eq!(attacke_target(None, &others[..2]), None);
}

#[test]
fn a_rundum_strikes_the_nearest_within_its_radius_but_its_last() {
    let tables = tables();
    let mut rundum = weapon_art(1057, [5.0, 3.0], 2);
    rundum.value_60 = [160, 20];
    // 160 + 20·L per cent of the reach: 180 % of 130 at level 1.
    let radius = rundum_radius(&rundum, 1, 130);
    assert!((radius - 234.0).abs() < 1e-4, "{radius}");
    let candidates = [
        candidate(4, 240, true),
        candidate(5, 200, true),
        candidate(6, 90, false),
    ];
    let radius = |reach| rundum_radius(&rundum, 1, reach);
    // The nearest is 6, which may not be struck: none.
    assert_eq!(rundum_target(&candidates, radius, None), None);
    assert_eq!(rundum_target(&candidates[..2], radius, None), Some(5));
    // Not the one it struck last.
    assert_eq!(rundum_target(&candidates[..2], radius, Some(5)), None);
    // The radius by each one's reach with the striker: 4 at 240 within 180 % of 135.
    let far = Candidate {
        reach: 135,
        ..candidate(4, 240, true)
    };
    assert_eq!(rundum_target(&[far], radius, None), Some(4));
    assert!(tables.weapon_arts.iter().any(|a| a.id == 1057));
}

#[test]
fn a_kick_strikes_at_its_classs_point_and_may_knock_its_target_back() {
    assert_eq!(style(1060), Some(Style::Kick));
    assert_eq!(style(1072), Some(Style::Kick));
    assert_eq!(style(1068), Some(Style::Kick));
    // The Gladiator's 148 and the Seraphim's 81, at the rate 1.0 whatever the attack speed.
    let fixed = |kind| match action(kind, Style::Kick, 1) {
        Some(ArtAction {
            clip: ArtClip::Slot(slot),
            rate,
            fixed: true,
            ..
        }) => Some((slot, rate)),
        _ => None,
    };
    assert_eq!(fixed(2), Some((148, 1.0)));
    assert_eq!(fixed(1), Some((81, 1.0)));
    assert_eq!(kick_point(2, 1), 0.6);
    assert_eq!(kick_point(2, 3), 0.55);
    assert_eq!(kick_point(1, 2), 0.4);
    // The Gladiator's 1060 at level 1 on a goblin warrior of level 1: 500 %, always; on one of
    // level 10, 50 %; a boss and a big one less; a dragon (52) never.
    assert_eq!(knockback_chance(1, (76, 1), false, false), 500.0);
    assert_eq!(knockback_chance(1, (76, 10), false, false), 50.0);
    assert!((knockback_chance(1, (76, 10), true, false) - 38.461_54).abs() < 1e-3);
    assert!((knockback_chance(1, (76, 10), false, true) - 41.666_67).abs() < 1e-3);
    assert_eq!(knockback_chance(1, (52, 1), false, false), 0.0);
    // 1,000 back, as the original's goblin took (#650), or less against a stronger target.
    let mut kick = weapon_art(1060, [5.0, 3.0], 2);
    kick.value_60 = [1000, 0];
    assert_eq!(knockback(&kick, 1, 1), Some(1000));
    assert_eq!(knockback(&kick, 1, 10), Some(500));
    assert_eq!(knockback(&kick, 1, 600), None);
}

#[test]
fn the_other_classes_arts_play_their_executors_clips() {
    let slot = |kind, style, class| match action(kind, style, class) {
        Some(ArtAction {
            clip: ArtClip::Slot(slot),
            fixed,
            fallback,
            ..
        }) => Some((slot, fixed, fallback)),
        Some(_) => Some((0, false, None)),
        None => None,
    };
    // The Dark Elf's: his starting 1066 with his blades (class 8), bare-handed and with a sword.
    assert_eq!(slot(4, Style::Hardhit, 8), Some((207, false, None)));
    assert_eq!(slot(4, Style::Hardhit, 0), Some((145, false, None)));
    assert_eq!(slot(4, Style::Hardhit, 3), Some((0, false, None)));
    assert_eq!(slot(4, Style::Rundum, 4), Some((223, false, None)));
    // The Wood Elf's, with a dagger (5) and a staff (4).
    assert_eq!(slot(5, Style::Attacke, 5), Some((215, false, None)));
    assert_eq!(slot(5, Style::Hardhit, 4), Some((205, false, None)));
    // The Vampiress's claws by night, a two-handed weapon by day.
    assert_eq!(slot(7, Style::Rundum, 2), Some((219, false, None)));
    assert_eq!(slot(6, Style::Rundum, 2), Some((221, false, None)));
    assert_eq!(slot(6, Style::Hardhit, 2), Some((0, false, None)));
    // The Dwarf's at 1.0, falling back to 211, 202 and 220; with a gun 248 and 247.
    assert_eq!(slot(8, Style::Attacke, 14), Some((248, true, Some(211))));
    assert_eq!(slot(8, Style::Hardhit, 13), Some((247, true, Some(202))));
    assert_eq!(slot(8, Style::Rundum, 3), Some((222, true, Some(220))));
    assert_eq!(slot(8, Style::Rundum, 14), None);
    // The Daemon's `Attacke` at 1.0, and neither a `Rundum` nor his charge.
    assert_eq!(slot(9, Style::Attacke, 1), Some((211, true, None)));
    assert_eq!(slot(9, Style::Rundum, 1), None);
    assert_eq!(slot(9, Style::Hardhit, 1), None);
    // The Mage has none, and the bows shoot.
    assert_eq!(slot(3, Style::Attacke, 1), None);
    assert_eq!(slot(5, Style::Attacke, 7), None);
}

#[test]
fn a_rundum_looks_from_its_classs_part_of_its_clip() {
    // The Gladiator's from 0.3 of its clip to its end: the original's 1057 struck first at
    // +766 ms of about 1.85 s (#644).
    assert!(!rundum_looks(2, 0.0));
    assert!(!rundum_looks(2, 0.29));
    assert!(rundum_looks(2, 0.31));
    assert!(rundum_looks(2, 0.99));
    // The Seraphim's throughout: her 1069 struck at +215 and +1249 ms of about 1.45 s.
    assert!(rundum_looks(SERAPHIM, 0.0));
    assert!(rundum_looks(SERAPHIM, 0.86));
    // The Dark Elf's and the Wood Elf's alike.
    assert!(rundum_looks(4, 0.1));
    assert!(rundum_looks(5, 0.95));
}

#[test]
fn an_arts_action_plays_its_classs_clip_for_what_it_holds() {
    let slot = |kind, style, class| match action(kind, style, class) {
        Some(ArtAction {
            clip: ArtClip::Slot(slot),
            rate,
            fixed: false,
            ..
        }) => Some((slot, rate)),
        _ => None,
    };
    // The Seraphim's `Attacke` with her blade (class 1): the slot 211 at 0.73 of her speed.
    assert_eq!(slot(1, Style::Attacke, 1), Some((211, 0.73)));
    assert_eq!(slot(1, Style::Attacke, 2), Some((212, 1.0)));
    assert_eq!(slot(1, Style::Rundum, 1), Some((153, 1.0)));
    assert_eq!(
        action(1, Style::Hardhit, 1).map(|a| a.clip),
        Some(ArtClip::Attack)
    );
    // The Gladiator's by the class of what he holds.
    assert_eq!(slot(2, Style::Attacke, 1), Some((211, 1.0)));
    assert_eq!(slot(2, Style::Attacke, 0), Some((210, 1.0)));
    assert_eq!(slot(2, Style::Rundum, 1), Some((220, 1.0)));
    assert_eq!(slot(2, Style::Rundum, 3), Some((222, 1.0)));
    assert_eq!(slot(2, Style::Hardhit, 1), Some((202, 1.0)));
    assert_eq!(slot(2, Style::Hardhit, 8), Some((207, 1.0)));
    // With a bow the arts are shots; the Mage's executor is not read.
    assert_eq!(action(1, Style::Attacke, 7), None);
    assert_eq!(action(3, Style::Attacke, 1), None);
}

#[test]
fn an_arts_clip_strikes_at_its_hit_points() {
    // `SERA_SPECIAL_MULTI_1H`'s seven, and `GLAD_SPECIAL_MULTI_1H`'s four.
    let seraphim = [
        0.136_963, 0.228_119, 0.315_969, 0.453_803, 0.577_059, 0.725_921, 0.746_75, 0.0,
    ];
    assert_eq!(hit_points(&seraphim).len(), 7);
    let gladiator = [
        0.151_934, 0.363_931, 0.550_761, 0.702_088, 0.0, 0.0, 0.0, 0.0,
    ];
    assert_eq!(
        hit_points(&gladiator),
        [0.151_934, 0.363_931, 0.550_761, 0.702_088]
    );
    assert_eq!(hardhit_point(&gladiator), 0.151_934);
    assert_eq!(hardhit_point(&[]), 0.5);
    assert_eq!(hardhit_point(&[1.5]), 0.5);
    assert!(!due(0.5, 0.5) && due(0.5, 0.51));
    // 101 to 250 the reach, within 10; 0 where it stands; the jumps not here.
    assert_eq!(made_from(130, 130.0), Some((130.0, 10.0)));
    assert_eq!(made_from(0, 130.0).map(|(at, _)| at), Some(0.0));
    assert_eq!(made_from(-290, 130.0), None);
}

#[test]
fn the_dark_elfs_boons_give_him_their_effects() {
    // GOG 2.28's 1047 and 1048: `[+0x50]` 0.3 + 0.2·L, `[+0x58]` 0.2 + 0.05·L, `[+0x60]` 29 + L.
    let record = |id| WeaponArt {
        factor: [0.3, 0.2],
        value_58: [0.2, 0.05],
        value_60: [29, 1],
        launch: 5,
        distance: 0,
        ..weapon_art(id, [0.0; 2], 8)
    };
    assert_eq!(style(1047), Some(Style::Boon));
    assert_eq!(style(1048), Some(Style::Boon));
    assert_eq!(
        action(4, Style::Boon, 1).map(|a| (a.clip, a.fixed)),
        Some((ArtClip::Slot(180), true))
    );
    let testosterone = boon(&record(1047), 3).unwrap();
    assert_eq!(testosterone.point, 0.7);
    let [(attack, seconds), (poison, _)] = testosterone.effects.as_slice() else {
        panic!("two effects");
    };
    assert_eq!((attack.id, attack.art, *seconds), (1, 1047, 32.0));
    assert!((attack.value - 0.9).abs() < 1e-6 && (poison.value - 0.35).abs() < 1e-6);
    assert_eq!(poison.id, 6);
    let adrenaline = boon(&record(1048), 1).unwrap();
    assert_eq!(adrenaline.point, 0.8);
    assert_eq!(
        adrenaline
            .effects
            .iter()
            .map(|(e, s)| (e.id, *s))
            .collect::<Vec<_>>(),
        [(2, 30.0), (12, 30.0)]
    );
    assert!(boon(&record(1071), 1).is_none());
}

#[test]
fn the_changeling_changes_the_vampiress_for_the_night_forms_time() {
    assert_eq!(style(1022), Some(Style::Changeling));
    assert_eq!(style(1023), Some(Style::Changeling));
    for kind in [VAMPIRESS_BY_DAY, VAMPIRESS_BY_NIGHT] {
        assert_eq!(
            action(kind, Style::Changeling, 1).map(|a| (a.clip, a.rate, a.fixed)),
            Some((ArtClip::Slot(144), CHANGELING_RATE, true))
        );
    }
    // 1022's record: `[+0x50]` 24 + 6·L seconds, doubled but by day (seen: 30 s at 08:04).
    let changeling = WeaponArt {
        factor: [24.0, 6.0],
        launch: 0,
        ..weapon_art(1022, [30.0, 0.0], 32)
    };
    assert_eq!(changeling_seconds(&changeling, 1, true), 30.0);
    assert_eq!(changeling_seconds(&changeling, 1, false), 60.0);
    assert_eq!(changeling_seconds(&changeling, 10, true), 84.0);
    // Her day arts by day, her night arts by night, the others in either.
    assert!(in_form(1022, VAMPIRESS_BY_DAY));
    assert!(!in_form(1022, VAMPIRESS_BY_NIGHT));
    assert!(in_form(1023, VAMPIRESS_BY_NIGHT));
    assert!(!in_form(1023, VAMPIRESS_BY_DAY));
    assert!(in_form(1038, VAMPIRESS_BY_NIGHT));
    assert!(!in_form(1073, VAMPIRESS_BY_NIGHT));
    assert!(in_form(1030, VAMPIRESS_BY_DAY));
    assert!(in_form(1043, 4));
}

#[test]
fn the_backbreaker_holds_its_target_and_lets_it_go_at_its_point() {
    assert_eq!(style(1010), Some(Style::Backbreaker));
    assert_eq!(
        action(2, Style::Backbreaker, 1).map(|a| (a.clip, a.rate, a.fixed)),
        Some((ArtClip::Slot(202), 1.0, true))
    );
    assert_eq!(backbreaker_lets_go(1), 0.5);
    assert_eq!(backbreaker_lets_go(3), 0.7);
    // `2·L' + 3 − M` from 5 to 10.
    assert_eq!(backbreaker_hold(1, 1), 5.0);
    assert_eq!(backbreaker_hold(5, 4), 9.0);
    assert_eq!(backbreaker_hold(20, 1), 10.0);
    assert!((backbreaker_factor(3) - 1.3).abs() < 1e-6);
    // `100·L/M`: always on one of its level or below, a boss's times 0.769.
    assert!(backbreaker_holds(1, 1, false, 99));
    assert!(!backbreaker_holds(1, 4, false, 25));
    assert!(backbreaker_holds(1, 4, false, 24));
    assert!(!backbreaker_holds(1, 1, true, 77));
    assert_eq!(made_from(80, 70.0), Some((80.0, 10.0)));
}

#[test]
fn the_right_button_makes_the_art_of_the_form() {
    // The Vampiress's of her form.
    assert_eq!(right_button(1023, VAMPIRESS_BY_DAY, |_| false), 1022);
    assert_eq!(right_button(1022, VAMPIRESS_BY_NIGHT, |_| false), 1023);
    assert_eq!(right_button(1030, VAMPIRESS_BY_DAY, |_| false), 1030);
    // The Daemon's second while his first's effect holds, else the first.
    assert_eq!(right_button(1091, 9, |_| false), 1094);
    assert_eq!(right_button(1094, 9, |_| false), 1094);
    assert_eq!(right_button(1094, 9, |key| key == 1094), 1091);
    assert_eq!(right_button(1091, 9, |key| key == 1094), 1091);
    assert_eq!(right_button(1092, 9, |key| key == 1097), 1092);
    assert_eq!(right_button(1092, 9, |_| false), 1097);
    assert_eq!(right_button(1012, 2, |_| true), 1012);
}

#[test]
fn the_vampiresss_bloodlust_raises_her_strikes_by_night() {
    // From none her first strike takes it to 11, which adds 11/65 of a strike's damage.
    assert!((bloodlust_rises(0.0) - 11.0).abs() < 1e-4);
    assert_eq!(bloodlust_damage(30, 11.0), 35);
    // It grows to its most: from 90, 110; above 90 it stays.
    assert!((bloodlust_rises(90.0) - 110.0).abs() < 1e-3);
    assert_eq!(bloodlust_rises(100.0), 100.0);
    assert_eq!(bloodlust_damage(30, 110.0), 80);
    assert_eq!(bloodlust_damage(30, 0.0), 30);
    assert!((frenzy_falls(11.0, BLOODLUST_FALLS, 1.0, false) - 6.0).abs() < 1e-5);
}

#[test]
fn the_dwarfs_frenzy_quickens_his_attack() {
    assert_eq!(style(1082), Some(Style::Frenzy));
    assert_eq!(
        action(8, Style::Frenzy, 0).map(|a| (a.clip, a.rate, a.fixed)),
        Some((ArtClip::Slot(157), 1.0, true))
    );
    let record = WeaponArt {
        factor: [39.0, 1.0],
        value_58: [28.0, 2.0],
        value_60: [5, 27],
        ..weapon_art(1082, [20.0, 12.0], 0)
    };
    // Seen at level 4: the effect 47 of the whole value 36 and the value 5 for 43 s.
    let boon = boon(&record, 4).expect("1082 is a boon");
    assert_eq!(boon.point, 0.1);
    let (effect, seconds) = boon.effects[0];
    assert_eq!(
        (effect.id, effect.whole, effect.value, effect.art),
        (FRENZY, 36, 5.0, 1082)
    );
    assert_eq!(seconds, 43.0);
    // Seen: 12 after his first strike; 10.18 rose to about 18.05 at the next.
    assert!((frenzy_rises(0.0, 36) - 12.0).abs() < 1e-5);
    assert!((frenzy_rises(10.18, 36) - 18.79).abs() < 0.01);
    assert_eq!(frenzy_rises(40.0, 36), 36.0);
    // 1 a second while he fights, 5 once he does not, and 0 at the least.
    assert!((frenzy_falls(20.0, 5.0, 0.5, true) - 19.5).abs() < 1e-5);
    assert!((frenzy_falls(20.0, 5.0, 0.5, false) - 17.5).abs() < 1e-5);
    assert_eq!(frenzy_falls(1.0, 5.0, 0.5, false), 0.0);
    assert_eq!(quickened(140.0, 25.7), 165.0);
    assert_eq!(quickened(210.0, 25.7), 220.0);
}

#[test]
fn the_special_arrows_are_shots() {
    assert_eq!(style(1004), Some(Style::Arrow));
    assert_eq!(style(1007), Some(Style::Arrow));
    let dumdum = WeaponArt {
        factor: [2.35, 0.35],
        ..weapon_art(1004, [7.0, 5.5], 0)
    };
    assert!((factor(&dumdum, 1) - 2.7).abs() < 1e-5);
    let club = WeaponArt {
        value_60: [15, 0],
        ..weapon_art(1007, [7.0, 4.2], 0)
    };
    assert_eq!(club_knockback(&club, 1), 15);
    assert_eq!(club_knockback(&club, 9), 15);
    assert_eq!(style(1008), Some(Style::Arrow));
    let blade = WeaponArt {
        factor: [5.66, 0.35],
        value_58: [0.7, 0.05],
        ..weapon_art(1008, [10.0, 6.0], 0)
    };
    let (seconds, factor) = blades(&blade, 2);
    assert!((seconds - 6.36).abs() < 1e-5 && (factor - 0.8).abs() < 1e-6);
}

#[test]
fn the_deaths_kiss_curses_attack_and_defence() {
    assert_eq!(style(1033), Some(Style::Kiss));
    assert_eq!(
        action(VAMPIRESS_BY_NIGHT, Style::Kiss, 0).map(|a| (a.clip, a.rate)),
        Some((ArtClip::Slot(148), 1.0))
    );
    let kiss = WeaponArt {
        factor: [1.0, 0.1],
        ..weapon_art(1033, [12.0, 8.0], 32)
    };
    assert!((curse(&kiss, 1) + 0.909_09).abs() < 1e-4);
    assert!((curse(&kiss, 9) + 0.526_31).abs() < 1e-4);
}

#[test]
fn the_domination_holds_its_target_by_its_level() {
    assert_eq!(style(1024), Some(Style::Dominate));
    assert_eq!(style(1025), Some(Style::Dominate));
    assert_eq!(
        action(VAMPIRESS_BY_DAY, Style::Dominate, 1).map(|a| (a.clip, a.rate)),
        Some((ArtClip::Slot(149), 1.0))
    );
    let day = WeaponArt {
        factor: [17.0, 3.0],
        ..weapon_art(1024, [20.0, 12.0], 32)
    };
    assert_eq!(domination_seconds(&day, 1, 1), 20.0);
    assert_eq!(domination_seconds(&day, 1, 5), 20.0);
    // Above 5L, times 5L/M.
    assert_eq!(domination_seconds(&day, 1, 10), 10.0);
}

#[test]
fn the_blood_bite_drains_its_factor_from_its_target() {
    assert_eq!(style(1030), Some(Style::Bite));
    assert_eq!(
        action(VAMPIRESS_BY_NIGHT, Style::Bite, 0).map(|a| (a.clip, a.rate)),
        Some((ArtClip::Slot(144), 1.0))
    );
    assert_eq!(action(VAMPIRESS_BY_DAY, Style::Bite, 0), None);
    let bite = WeaponArt {
        factor: [10.0, 20.0],
        ..weapon_art(1030, [9.0, 6.0], 32)
    };
    assert_eq!(bite_drain(&bite, 1), 30);
    assert_eq!(bite_drain(&bite, 5), 110);
}

#[test]
fn the_claw_jump_strikes_and_holds_the_creatures_about_her_landing() {
    assert_eq!(style(1032), Some(Style::Jump));
    assert_eq!(
        action(VAMPIRESS_BY_NIGHT, Style::Jump, 0).map(|a| a.clip),
        Some(ArtClip::Slot(153))
    );
    assert_eq!(jump_points(VAMPIRESS_BY_NIGHT), [0.8, 0.35]);
    let claw = WeaponArt {
        factor: [190.0, 10.0],
        value_58: [5.0, 10.0],
        ..weapon_art(1032, [9.0, 6.0], 32)
    };
    assert_eq!(claw_quake(&claw, 1), (200.0, 15.0));
    assert_eq!(claw_quake(&claw, 3), (220.0, 35.0));
}

#[test]
fn the_dash_puts_the_vampiress_at_its_point_within_its_reach() {
    assert_eq!(style(1028), Some(Style::Dash));
    assert_eq!(style(1029), Some(Style::Dash));
    assert_eq!(
        action(VAMPIRESS_BY_DAY, Style::Dash, 1).map(|a| (a.clip, a.rate, a.fixed)),
        Some((ArtClip::Slot(144), DASH_RATE, true))
    );
    let day = WeaponArt {
        value_60: [400, 25],
        launch: 0,
        distance: 0,
        ..weapon_art(1028, [4.0, 3.0], 32)
    };
    let night = WeaponArt {
        value_60: [500, 50],
        ..day.clone()
    };
    assert_eq!(dash_reach(&day, 1), 425);
    assert_eq!(dash_reach(&night, 2), 600);
    assert!(in_form(1028, VAMPIRESS_BY_DAY) && !in_form(1028, VAMPIRESS_BY_NIGHT));
}

#[test]
fn the_panther_leaps_to_its_point_within_its_reach() {
    assert_eq!(style(1043), Some(Style::Panther));
    assert_eq!(
        action(4, Style::Panther, 1).map(|a| (a.clip, a.fixed)),
        Some((ArtClip::Slot(182), true))
    );
    // 1043's record: `[+0x50]` 400 + 50·L.
    let panther = WeaponArt {
        factor: [400.0, 50.0],
        launch: 3,
        distance: 0,
        ..weapon_art(1043, [4.0, 2.0], 8)
    };
    assert_eq!(panther_reach(&panther, 1), 450);
    assert_eq!(panther_reach(&panther, 3), 550);
    // The rate by the reach: 6 − 4.545 at 450, 1 beyond 500.
    assert!((panther_rate(450) - 1.4545).abs() < 1e-3);
    assert_eq!(panther_rate(550), 1.0);
    assert_eq!(panther_rate(5), 5.0);
    // Within the reach the point itself; beyond it, brought to the reach; far, to the reach.
    assert_eq!(
        panther_point([0.0, 0.0], [300.0, 0.0], 450, false),
        [300.0, 0.0]
    );
    assert_eq!(
        panther_point([0.0, 0.0], [900.0, 0.0], 450, false),
        [450.0, 0.0]
    );
    assert_eq!(
        panther_point([0.0, 0.0], [300.0, 0.0], 450, true),
        [450.0, 0.0]
    );
    assert_eq!(
        panther_point([10.0, 10.0], [10.0, 10.0], 450, false),
        [10.0, 10.0]
    );
}

#[test]
fn the_multi_shot_gives_its_arrows_by_the_level() {
    assert_eq!(style(1005), Some(Style::Volley));
    // 1005's record: `[+0x50]` 50 + 144·L, `[+0x58]` 77 + 20·L, `[+0x60]` 20.
    let multi = WeaponArt {
        factor: [50.0, 144.0],
        value_58: [77.0, 20.0],
        value_60: [20, 0],
        launch: 0,
        distance: 0,
        ..weapon_art(1005, [20.0, 12.0], 0x10)
    };
    let arrows = |level| volley(&multi, level).0.whole;
    assert_eq!((arrows(1), arrows(10), arrows(18)), (2, 5, 6));
    let (effect, seconds) = volley(&multi, 1);
    assert_eq!((effect.id, effect.art, seconds), (0x15, 1005, 20.0));
}

#[test]
fn the_daemons_forms_replace_each_other() {
    assert_eq!(style(1097), Some(Style::Form));
    assert_eq!(style(1100), Some(Style::Form));
    assert_eq!(
        action(9, Style::Form, 0).map(|a| (a.clip, a.fixed)),
        Some((ArtClip::Slot(160), true))
    );
    let record = |id, value_60, value_68| WeaponArt {
        factor: [44.0, 1.0],
        value_58: [10.0, 20.0],
        value_60,
        value_68,
        launch: 2,
        distance: 0,
        ..weapon_art(id, [30.0, 12.0], 0x80_0000)
    };
    let fierce = form(&record(1097, [15, 5], [15, 5]), 1).unwrap();
    let ids: Vec<(u32, u32, f32)> = fierce
        .effects
        .iter()
        .map(|(e, s)| (e.id, e.art, *s))
        .collect();
    assert_eq!(
        ids,
        [
            (3, 1097, 45.0),
            (1, 1097, 45.0),
            (7, 1097, 45.0),
            (60, DAEMON_FORM, 45.0)
        ]
    );
    assert!((fierce.effects[0].0.value - 0.2).abs() < 1e-6);
    assert_eq!(fierce.replaces, [1094, 1097, 1098, 1099, 1100, DAEMON_FORM]);
    let fire = form(&record(1098, [0, 0], [0, 871]), 1).unwrap();
    let (aura, resistance) = (fire.effects[0].0, fire.effects[1].0);
    assert_eq!(
        (aura.id, aura.whole, aura.level, resistance.id),
        (53, 1, 0x100, 8)
    );
    assert!((aura.value - 0.871).abs() < 1e-6 && (resistance.value - 30.0).abs() < 1e-6);
    assert_eq!(
        form(&record(1099, [0, 0], [0, 871]), 1).unwrap().effects[1]
            .0
            .id,
        10
    );
    assert_eq!(
        form(&record(1100, [0, 0], [0, 871]), 1).unwrap().effects[1]
            .0
            .id,
        9
    );
    assert_eq!(
        form(&record(1097, [15, 5], [15, 5]), 200).unwrap().effects[0].1,
        180.0
    );
}

#[test]
fn the_daemons_boon_takes_his_forms_off_and_quickens_him() {
    assert_eq!(style(1094), Some(Style::Boon));
    assert_eq!(
        action(9, Style::Boon, 0).map(|a| (a.clip, a.rate, a.fixed)),
        Some((ArtClip::Slot(150), 1.0, true))
    );
    // 1094's record: `[+0x50]` 29 + L, `[+0x58]` 0.1 + 0.05·L, `[+0x60]` 108 + 2·L.
    let fury = WeaponArt {
        factor: [29.0, 1.0],
        value_58: [0.1, 0.05],
        value_60: [108, 2],
        launch: 5,
        distance: 0,
        ..weapon_art(1094, [20.0, 12.0], 0x80_0000)
    };
    let given = boon(&fury, 1).unwrap();
    assert_eq!(given.point, 0.7);
    let effects: Vec<(u32, u32, f32)> = given
        .effects
        .iter()
        .map(|(e, s)| (e.id, e.art, *s))
        .collect();
    assert_eq!(
        effects,
        [
            (50, 1094, 30.0),
            (2, DAEMON_FORM, 30.0),
            (52, DAEMON_FORM, 30.0)
        ]
    );
    assert!((given.effects[1].0.value - 0.15).abs() < 1e-6);
    assert!((given.effects[2].0.value - 1.1).abs() < 1e-6);
    assert_eq!(given.clears, [1094, 1097, 1098, 1099, 1100, DAEMON_FORM]);
    assert_eq!(given.starts.len(), 1);
    // At most 180 s.
    assert_eq!(boon(&fury, 200).unwrap().effects[0].1, 180.0);
}

#[test]
fn the_arrowproof_gives_its_maker_the_deflection() {
    assert_eq!(style(1045), Some(Style::Ward));
    assert_eq!(
        action(4, Style::Ward, 1).map(|a| (a.clip, a.rate, a.fixed)),
        Some((ArtClip::Slot(146), 1.0, true))
    );
    // 1045's record: `[+0x50]` 44 + L, `[+0x58]` 1 + 0.05·L.
    let proof = WeaponArt {
        factor: [44.0, 1.0],
        value_58: [1.0, 0.05],
        launch: 2,
        distance: 0,
        ..weapon_art(1045, [30.0, 12.0], 8)
    };
    let given = ward(&proof, 3).unwrap();
    assert_eq!(given.point, 0.8);
    let (effect, seconds) = given.effects[0];
    assert_eq!(
        (effect.id, effect.art, effect.level, seconds),
        (0x19, 1045, 3, 47.0)
    );
    assert!((effect.value - 1.15).abs() < 1e-6);
    assert!(ward(&weapon_art(1047, [0.0; 2], 8), 1).is_none());
}

#[test]
fn the_viper_stuns_its_target_and_the_hostiles_about_it() {
    assert_eq!(style(1042), Some(Style::Viper));
    assert_eq!(
        action(4, Style::Viper, 1).map(|a| (a.clip, a.rate, a.fixed)),
        Some((ArtClip::Slot(148), 1.0, true))
    );
    // 1042's record: `[+0x50]` 148 + 2·L, `[+0x58]` 19.5 + 0.5·L.
    let viper_art = WeaponArt {
        factor: [148.0, 2.0],
        value_58: [19.5, 0.5],
        ..weapon_art(1042, [20.0, 12.0], 8)
    };
    assert_eq!(viper(&viper_art, 1), (150.0, 20.0));
    assert_eq!(viper(&viper_art, 10), (168.0, 24.5));
    assert_eq!(VIPER_LEFT, 0.2);
}

#[test]
fn the_roar_knocks_back_and_holds_the_hostiles_about_its_maker() {
    assert_eq!(style(1014), Some(Style::Roar));
    assert_eq!(
        action(2, Style::Roar, 1).map(|a| (a.clip, a.rate, a.fixed)),
        Some((ArtClip::Slot(149), 1.0, true))
    );
    // 1014's record: `[+0x50]` 150 + 10·L, `[+0x58]` 3 + 0.35·L, `[+0x60]` 50 + 3·L,
    // `[+0x68]` 20 and 18.
    let roar = WeaponArt {
        factor: [150.0, 10.0],
        value_58: [3.0, 0.35],
        value_60: [50, 3],
        value_68: [20, 18],
        launch: 2,
        distance: 0,
        ..weapon_art(1014, [10.0, 6.0], 2)
    };
    assert_eq!(roar_radius(&roar, 1), 160);
    assert_eq!(roar_radius(&roar, 10), 250);
    assert_eq!(roar_knockback(&roar, 1), 56);
    assert_eq!(roar_knockback(&roar, 10), 92);
    assert!((roar_hold(&roar, 1) - 3.35).abs() < 1e-6);
    assert!((roar_hold(&roar, 10) - 6.5).abs() < 1e-6);
    // At least 10.
    let weak = WeaponArt {
        value_60: [0, 0],
        value_68: [0, 20],
        ..roar
    };
    assert_eq!(roar_knockback(&weak, 1), 10);
}

#[test]
fn the_jump_flies_to_its_target_and_strikes_as_it_lands() {
    assert_eq!(style(1017), Some(Style::Jump));
    assert_eq!(
        action(1, Style::Jump, 1).map(|a| (a.clip, a.fixed)),
        Some((ArtClip::Slot(147), true))
    );
    // Made where she stands, whatever the record's distance of −330.
    assert_eq!(order_distance(1017, -330), 0);
    assert_eq!(order_distance(1003, -290), 0);
    assert_eq!(order_distance(1071, 130), 130);
    // The rate by the distance.
    assert_eq!(jump_rate(5), 5.0);
    assert_eq!(jump_rate(98), 5.0);
    assert_eq!(jump_rate(99), 4.0);
    assert_eq!(jump_rate(250), 3.0);
    assert_eq!(jump_rate(494), 1.0);
    assert_eq!(jump_rate(497), 1.0);
    assert_eq!(jump_rate(800), 1.0);
    // Short of the target by half the reach of 75, 37.
    let [x, y] = jump_landing([0.0, 0.0], [300.0, 400.0], Some(75));
    assert!((x - 277.8).abs() < 1e-3 && (y - 370.4).abs() < 1e-3);
    assert_eq!(
        jump_landing([0.0, 0.0], [300.0, 400.0], None),
        [300.0, 400.0]
    );
    // The arc: on the ground at both ends, 1/12 of the length at its middle.
    assert_eq!(jump_height(240.0, 0.0), 0.0);
    assert_eq!(jump_height(240.0, 0.5), 20.0);
    assert_eq!(jump_height(240.0, 1.0), 0.0);
    // The Seraphim's points and the Gladiator's.
    assert_eq!(jump_points(1), [0.75, 0.4]);
    assert_eq!(jump_points(2), [0.8, 0.6]);
}

#[test]
fn the_gladiators_jump_strikes_about_him_and_stuns() {
    assert_eq!(style(1003), Some(Style::Jump));
    assert_eq!(
        action(2, Style::Jump, 1).map(|a| (a.clip, a.fixed)),
        Some((ArtClip::Slot(144), true))
    );
    // 1003's record: `[+0x50]` 190 + 10·L, `[+0x58]` 1.8 + 0.2·L.
    let jump = WeaponArt {
        factor: [190.0, 10.0],
        value_58: [1.8, 0.2],
        distance: -290,
        ..weapon_art(1003, [9.0, 6.0], 2)
    };
    let (radius, factor) = jump_quake(&jump, 1);
    assert_eq!(radius, 200.0);
    assert!((factor - 2.0).abs() < 1e-6);
    assert_eq!(jump_stun(1), 4.0);
    assert_eq!(jump_stun(20), 23.0);
}

#[test]
fn the_whirling_blow_strikes_as_her_rundum_and_knocks_back() {
    assert_eq!(style(1020), Some(Style::Whirl));
    assert_eq!(
        action(1, Style::Whirl, 1).map(|a| (a.clip, a.rate, a.fixed)),
        Some((ArtClip::Slot(149), 1.0, true))
    );
    assert_eq!(action(1, Style::Whirl, 7), None);
    // `[+0x58]` 170 + 5·L.
    let whirl = WeaponArt {
        value_58: [170.0, 5.0],
        ..weapon_art(1020, [20.0, 12.0], 1)
    };
    assert_eq!(whirl_knockback(&whirl, 1), 175);
    assert_eq!(whirl_knockback(&whirl, 10), 220);
}

#[test]
fn the_hurled_blades_strike_what_the_blade_touches() {
    assert_eq!(style(1054), Some(Style::Blades));
    assert_eq!(
        action(1, Style::Blades, 1).map(|a| (a.clip, a.rate, a.fixed)),
        Some((ArtClip::Slot(148), 1.6, true))
    );
    // A goblin warrior's 25 and a blade's 50 (`items.pak +0x14` of 0).
    assert!(blade_touches(74.0, 25.0, 50.0));
    assert!(!blade_touches(75.0, 25.0, 50.0));
}

#[test]
fn the_hand_of_god_strikes_its_target_and_the_hostiles_within_the_reach() {
    assert_eq!(style(1012), Some(Style::Smite));
    assert_eq!(
        action(2, Style::Smite, 1).map(|a| (a.clip, a.fixed)),
        Some((ArtClip::Slot(146), true))
    );
    let creature = |id, distance| Candidate {
        id,
        distance,
        reach: 130,
        hostile: true,
    };
    let others = [
        creature(0, 120),
        creature(1, 140),
        creature(2, 129),
        creature(3, 130),
    ];
    // Its target wherever it stands, the others strictly within the reach.
    assert_eq!(smitten(Some(1), &others, 130), [1, 0, 2]);
    assert_eq!(smitten(None, &others, 130), [0, 2]);
}

#[test]
fn the_seraphims_bfg_gives_her_its_effect_halfway_through_its_clip() {
    // GOG 2.28's 1018: `[+0x58]` 29.5 + 0.5·L, of the launch type 5.
    assert_eq!(style(1018), Some(Style::Boon));
    let bfg = WeaponArt {
        value_58: [29.5, 0.5],
        launch: 5,
        distance: 0,
        ..weapon_art(1018, [0.0; 2], 1)
    };
    // Seen at the level 1: the effect 16 of the whole value 1 for 30 s, keyed by 1018.
    let given = boon(&bfg, 1).unwrap();
    assert_eq!(given.point, 0.5);
    assert_eq!(
        given
            .effects
            .iter()
            .map(|(e, s)| (e.id, e.whole, e.art, *s))
            .collect::<Vec<_>>(),
        [(BFG, 1, 1018, 30.0)]
    );
    assert_eq!(boon(&bfg, 7).unwrap().effects[0].0.whole, 7);
    assert_eq!(boon(&bfg, 7).unwrap().effects[0].1, 33.0);
    assert_eq!(
        action(1, Style::Boon, 1).map(|a| (a.clip, a.rate, a.fixed)),
        Some((ArtClip::Slot(149), 1.0, true))
    );
    // Refused while its effect or 1019's holds.
    assert_eq!(refused_while(1018), [1018, 1019]);
}

#[test]
fn the_dwarfs_war_cry_is_a_boon_and_the_boons_wait_for_their_effects() {
    // The Gladiator's 1015, of the launch type 2: `[+0x50]` 0.5 + 0.2·L, `[+0x58]` 39.5 + 0.5·L.
    assert_eq!(style(1015), Some(Style::Boon));
    let heroism = WeaponArt {
        factor: [0.5, 0.2],
        value_58: [39.5, 0.5],
        launch: 2,
        distance: 0,
        ..weapon_art(1015, [0.0; 2], 2)
    };
    let given = boon(&heroism, 1).unwrap();
    assert_eq!(given.point, 0.7);
    assert_eq!(
        given
            .effects
            .iter()
            .map(|(e, s)| (e.id, e.art, *s))
            .collect::<Vec<_>>(),
        [(1, 1015, 40.0), (2, 1015, 40.0)]
    );
    assert!((given.effects[1].0.value - 0.7).abs() < 1e-6);
    assert_eq!(
        action(2, Style::Boon, 1).map(|a| a.clip),
        Some(ArtClip::Slot(151))
    );
    // GOG 2.28's 1089: `[+0x50]` and `[+0x58]` 0.4 + 0.1·L, `[+0x60]` 2950 + 50·L hundredths.
    let war_cry = WeaponArt {
        factor: [0.4, 0.1],
        value_58: [0.4, 0.1],
        value_60: [2950, 50],
        launch: 5,
        distance: 0,
        ..weapon_art(1089, [0.0; 2], 0x40_0000)
    };
    let given = boon(&war_cry, 1).unwrap();
    assert_eq!(given.point, 0.8);
    assert_eq!(
        given
            .effects
            .iter()
            .map(|(e, s)| (e.id, *s))
            .collect::<Vec<_>>(),
        [(1, 30.0), (2, 30.0)]
    );
    assert_eq!(
        action(8, Style::Boon, 1).map(|a| a.clip),
        Some(ArtClip::Slot(154))
    );
    assert_eq!(style(1089), Some(Style::Boon));
    assert_eq!(refused_while(1089), [1089]);
    assert_eq!(refused_while(1019), [1018, 1019]);
    assert!(refused_while(1071).is_empty());
}
