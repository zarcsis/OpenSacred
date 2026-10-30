use std::collections::VecDeque;

use opensacred_data::tables::{Item, Weapon, WeaponModifier};

use super::*;
use crate::spawning::Rng;

/// A script of rolls: each roll takes the next value, which must lie below its bound.
struct Script(VecDeque<u32>);

impl Dice for Script {
    fn below(&mut self, n: u32) -> u32 {
        let value = self.0.pop_front().expect("a roll that the script lacks");
        assert!(value < n, "{value} scripted for a roll below {n}");
        value
    }
}

fn script(rolls: &[u32]) -> Script {
    Script(rolls.iter().copied().collect())
}

fn dead(class: u8, level: u32, rank: Rank) -> Dead {
    Dead {
        id: 0,
        creature: 76,
        class,
        level,
        worth: 0,
        rank,
    }
}

fn hero(level: u32) -> Hero {
    Hero {
        creature: 1,
        level,
        ..Hero::default()
    }
}

fn death(dead: Dead, hero_level: u32) -> Death {
    Death {
        dead,
        hero: hero(hero_level),
        ..Death::default()
    }
}

/// A record of the class bits and the other flags `flags`, of the levels `levels` and the
/// template's words 38 to 40 `limits`.
fn record(flags: u32, levels: [u8; 2], limits: [i64; 3]) -> Record {
    Record {
        flags,
        levels,
        skill: [0, 0],
        kind: 0,
        set: false,
        limits: Some(limits),
    }
}

/// A catalogue of types, their categories and records, every type listed.
fn catalogue(types: &[(u32, u8, Record)]) -> Catalogue {
    let mut lists: HashMap<u8, Vec<u32>> = HashMap::new();
    for &(item, category, _) in types {
        lists.entry(category).or_default().push(item);
    }
    Catalogue {
        categories: types.iter().map(|&(item, c, _)| (item, c)).collect(),
        records: types.iter().map(|&(item, _, r)| (item, r)).collect(),
        lists,
        slots: types.iter().map(|&(item, ..)| (item, item)).collect(),
        readable: HashSet::new(),
        runes: Vec::new(),
        models: HashMap::new(),
    }
}

/// A slot of `items.pak` of a category, flags 2 (an item).
fn slot(slot: u32, category: u8) -> Item {
    Item {
        slot,
        kind: 2,
        class: 0,
        subclass: 0,
        name: String::new(),
        texture: 0,
        sprite: 0,
        byte_03: 0,
        words_08: [0; 2],
        words_14: [0, 0, 0, 0, 0, 0, u32::from(category) << 16, 0],
        bytes_34: [0; 3],
        byte_57: 0,
        words_58: [0; 10],
    }
}

/// A record of `Weapon.pak` of a parent, of no flags.
fn weapon(id: u32, parent: u32) -> Weapon {
    Weapon {
        id,
        parent,
        name: String::new(),
        floats_00: [0.0; 5],
        bytes_14: [0; 4],
        float_18: 0.0,
        bytes_1c: [0; 8],
        words_84: vec![0; 63],
    }
}

#[test]
fn a_derived_type_is_listed_with_its_roots_entry_and_shows_its_model() {
    // The knife 1727 and the hand axe 1741 of the weapons, the gladiator's armour 4053 of the
    // category 21 on file, and the derived types 2029 of 1727, 2114 of 1741, 2100 of 2029, 6600
    // of 4053 and 2500 of a parent without a slot (#596).
    let items = [slot(1727, 5), slot(1741, 5), slot(4053, 21)];
    let weapons = [
        weapon(1727, 0),
        weapon(1741, 0),
        weapon(2029, 1727),
        weapon(2100, 2029),
        weapon(2114, 1741),
        weapon(2500, 9999),
        weapon(4053, 0),
        weapon(6600, 4053),
    ];
    let mut words = vec![1727, 1741, 4053, 0, 0, 3];
    words.resize(53, 0);
    let templates = Templates::new(
        &[WeaponModifier {
            words,
            entries: Vec::new(),
        }],
        &weapons,
    );
    let catalogue = Catalogue::new(&items, &weapons, &[], &templates);
    assert_eq!(catalogue.list(5), [1727, 1741, 2029, 2100, 2114]);
    // 4053 takes the category 6 at the loader, and its derived type with it.
    assert_eq!(catalogue.list(6), [4053, 6600]);
    assert!(catalogue.list(21).is_empty());
    assert_eq!(catalogue.kind(2029), Some(Kind::Weapon));
    assert_eq!(catalogue.kind(6600), Some(Kind::Armour));
    // A derived type shows its root's model, a derived type's derived one too.
    assert_eq!(
        [1727, 2029, 2100, 2114, 6600, 2500].map(|item| catalogue.slot(item)),
        [
            Some(1727),
            Some(1727),
            Some(1727),
            Some(1741),
            Some(4053),
            None
        ]
    );
    // The drops' lookup takes the parent once: 2029 finds 1727's template, 2100 none, so the
    // pass never takes it.
    assert!(catalogue.records[&2029].limits.is_some());
    assert!(catalogue.records[&2100].limits.is_none());
}

const ANY: u32 = CLASS_BITS;

/// The records of the potions, the gold piles and the dagger, as the game's, and no type listed:
/// every weapon, armour and jewel the dagger.
fn unlisted() -> Catalogue {
    let bare = Record {
        limits: None,
        ..record(0, [1, 1], [0, 0, 0])
    };
    let mut types: Vec<(u32, u8, Record)> = POTIONS
        .iter()
        .flatten()
        .map(|&potion| (potion, 9, bare))
        .chain(GOLD.map(|pile| (pile, 15, bare)))
        .collect();
    types.push((DAGGER, 5, record(ANY, [1, 1], [1, 0, 0])));
    Catalogue {
        lists: HashMap::new(),
        ..catalogue(&types)
    }
}

#[test]
fn a_calls_chance_is_350_and_700_for_a_boss_or_a_champion() {
    let rules = Rules::default();
    assert_eq!(rules.chance(Rank::Common, 0), 350);
    assert_eq!(rules.chance(Rank::Boss, 0), 700);
    assert_eq!(rules.chance(Rank::Champion, 0), 700);
    // A champion's levelling alone has no mark, which the factor takes (#621).
    assert_eq!(rules.chance(Rank::Levelled, 0), 350);
    // A treasure of 101 adds v = 10 + 180·(1 − 1/2) = 100 per cent.
    assert_eq!(rules.chance(Rank::Common, 101), 700);
}

#[test]
fn a_champion_and_a_boss_make_more_calls_with_a_bonus() {
    let rules = Rules::default();
    let of = |rank, level, hero_level| Death {
        kill: Kill {
            by_player: false,
            ..Kill::default()
        },
        ..death(dead(3, level, rank), hero_level)
    };
    let mut dice = script(&[2, 0]);
    let calls = |death: &Death, dice: &mut Script| rules.calls(death, dice);
    assert_eq!(
        calls(&of(Rank::Common, 5, 5), &mut dice),
        Calls { count: 1, bonus: 0 }
    );
    // R(1, 3) of 3 calls more, and R(1, 4) of 1.
    assert_eq!(
        calls(&of(Rank::Champion, 5, 5), &mut dice),
        Calls {
            count: 4,
            bonus: 35
        }
    );
    assert_eq!(
        calls(&of(Rank::Boss, 5, 5), &mut dice),
        Calls {
            count: 2,
            bonus: 50
        }
    );
    // A champion's levelling alone takes the champions' bonus and calls (#621): R(1, 3) of 2.
    let mut levelled_dice = script(&[1]);
    assert_eq!(
        calls(&of(Rank::Levelled, 5, 5), &mut levelled_dice),
        Calls {
            count: 3,
            bonus: 35
        }
    );
    // Too weak, of level 1 to a hero of 30: no more calls, the bonus kept.
    assert_eq!(
        calls(&of(Rank::Champion, 1, 30), &mut dice),
        Calls {
            count: 1,
            bonus: 35
        }
    );
    // A player's kill: 121 minutes add 0.5·50, a luck of 125 adds √2,500.
    let played = Death {
        kill: Kill {
            by_player: true,
            minutes: 121.0,
            luck: 125.0,
            treasure: 0,
        },
        ..of(Rank::Common, 5, 5)
    };
    assert_eq!(
        calls(&played, &mut dice),
        Calls {
            count: 1,
            bonus: 75
        }
    );
    assert!(dice.0.is_empty());
}

#[test]
fn the_bonus_compounds_by_the_creatures_worth_at_each_pass() {
    let rules = Rules::default();
    // √(40·10)/40 = 0.5 a pass.
    assert_eq!(rules.compound(35, 76, 10), 17);
    assert_eq!(rules.compound(17, 76, 10), 8);
    // A worth of 40 keeps it; a player's or a worthless creature's stays.
    assert_eq!(rules.compound(35, 76, 40), 35);
    assert_eq!(rules.compound(35, 5, 10), 35);
    assert_eq!(rules.compound(35, 76, 0), 35);
}

#[test]
fn the_keys_share_out_by_their_weights_at_g_10() {
    let rules = Rules::default();
    // A creature whose (E + 9)/(H + 9) is at least 1.1 weighs 1: g = 10.
    let g = rules.throw_factor(10, 1);
    assert!((g - 10.0).abs() < f32::EPSILON);
    let sums = rules.sums(0, g);
    let ends: Vec<u32> = sums.iter().map(|&(_, sum)| sum).collect();
    assert_eq!(
        ends,
        [
            70, 160, 160, 190, 240, 300, 380, 480, 600, 730, 870, 1020, 1170, 1320, 1470, 1620,
            4620, 10060
        ]
    );
    // In 10,000: the key 0 in 0.7 %, 1 in 0.9 %, 2 never, 16 in 30 % and 17 in 53.8 %.
    let share = |key: usize| sums[key].1.min(10_000) - key.checked_sub(1).map_or(0, |k| sums[k].1);
    assert_eq!([0, 1, 2, 16, 17].map(share), [70, 90, 0, 3000, 5380]);
    // u = R(1, 10000): 1 and 69 the key 0, 70 the key 1, 4,619 the key 16, 4,620 and 10,000
    // the key 17.
    let keys = [0, 68, 69, 4618, 4619, 9999].map(|roll| rules.key(0, g, &mut script(&[roll])));
    assert_eq!(keys, [0, 0, 1, 16, 17, 17]);
    // A bonus of 100 doubles each weight but the key 16's.
    let doubled = rules.sums(100, g);
    assert_eq!((doubled[0], doubled[16]), ((0, 140), (16, 2 * 1620 + 3000)));
    // A weak creature weighs 0.04: g = 10·(0.85·0.04 + 0.15) = 1.84, the key 17 in 91.5 %.
    let weak = rules.throw_factor(1, 30);
    assert!((weak - 1.84).abs() < 1e-6, "{weak}");
    assert_eq!(rules.sums(0, weak)[16], (16, 853));
}

#[test]
fn the_potions_size_follows_the_difficulty() {
    let take = |difficulty, rolls: &[u32]| potion(difficulty, &mut script(rolls));
    // The healing potions in 601 cases of 1,000: small on bronze, small or medium on silver,
    // medium or large on gold, large above.
    assert_eq!(take(Difficulty::Bronze, &[600]), 5171);
    assert_eq!(
        [0, 1].map(|c| take(Difficulty::Silver, &[0, c])),
        [5171, 5145]
    );
    assert_eq!(
        [0, 1].map(|c| take(Difficulty::Gold, &[0, c])),
        [5145, 5140]
    );
    assert_eq!(take(Difficulty::Platinum, &[0]), 5140);
    assert_eq!(take(Difficulty::Niobium, &[0]), 5140);
    // Else a row of 0 to 4.
    assert_eq!(take(Difficulty::Bronze, &[601, 2]), 5168);
    assert_eq!(take(Difficulty::Niobium, &[999, 4]), 5141);
}

#[test]
fn gold_is_17_to_23_at_level_1_and_165_to_231_at_level_10() {
    let rules = Rules::default();
    let amount = |level, roll| rules.gold(level, &mut script(&[roll]));
    assert_eq!((amount(1, 0), amount(1, 6)), (17, 23));
    assert_eq!((amount(10, 0), amount(10, 66)), (165, 231));
    // The piles: 5132, but in a case of 13 above level 4 5133, of those in a case of 7 above
    // level 9 5134, and of those in a case of 4 above level 19 5135.
    let of = |level, rolls: &[u32]| pile(level, &mut script(rolls));
    assert_eq!(of(4, &[0]), 5132);
    assert_eq!(of(5, &[0, 0]), 5133);
    assert_eq!(of(10, &[0, 0, 0]), 5134);
    assert_eq!(of(20, &[0, 0, 0]), 5135);
    assert_eq!((of(20, &[1]), of(20, &[0, 1])), (5132, 5133));
}

#[test]
fn an_items_levels_lo_and_hi_on_bronze_and_silver() {
    let rules = Rules::default();
    let levels = |level, difficulty| rules.item_levels(level, difficulty);
    // On bronze the creature's own level, hi a twentieth more, rounded; 1.05 is a single
    // float a little below it, so that lo 10 keeps hi 10.
    assert_eq!(levels(1, Difficulty::Bronze), (1, 1));
    assert_eq!(levels(10, Difficulty::Bronze), (10, 10));
    assert_eq!(levels(11, Difficulty::Bronze), (11, 12));
    assert_eq!(levels(20, Difficulty::Bronze), (20, 21));
    // On silver (L − 1)/1.09.
    assert_eq!(levels(10, Difficulty::Silver), (8, 8));
    assert_eq!(levels(50, Difficulty::Silver), (44, 46));
    // The weighing's level on bronze's scale, rounded.
    assert_eq!(rules.bronze(10, Difficulty::Bronze), 10);
    assert_eq!(rules.bronze(50, Difficulty::Silver), 45);
    assert_eq!(rules.bronze(1, Difficulty::Silver), 1);
}

#[test]
fn the_kinds_come_in_their_order() {
    let rules = Rules::default();
    let templates = Templates::default();
    let kinds = |class, rolls: &[u32]| {
        let death = death(dead(class, 5, Rank::Common), 5);
        let catalogue = Catalogue::default();
        let mut dropping = Dropping {
            death: &death,
            rules: &rules,
            catalogue: &catalogue,
            templates: &templates,
            last: &mut LastJewel::default(),
        };
        let mut dice = script(rolls);
        let first = dropping.kind(0, &mut dice);
        assert!(dice.0.is_empty(), "{:?} left", dice.0);
        (first, dropping.last.jewel.is_some())
    };
    // A potion in 180 ‰.
    assert_eq!(kinds(3, &[179, 0]).0, (Kind::Potion, Some(5171)));
    // Past the potion a rune's roll, which a hero of level 5 with no arts and no runes holds to
    // 100 ‰: 999 makes none; then gold in 300 ‰ of the rest, a pile by the level.
    assert_eq!(kinds(3, &[180, 999, 299, 1]).0, (Kind::Gold, Some(5132)));
    // A weapon in 330 ‰ of the rest: of the levels tested in one case of three, for the hero's
    // class in one of two; none of an empty catalogue.
    assert_eq!(
        kinds(3, &[180, 999, 300, 600, 329, 1, 1]).0,
        (Kind::Weapon, None)
    );
    // Armour in 880 ‰ of the rest, of eleven categories, the bridles drawn once again.
    assert_eq!(
        kinds(3, &[180, 999, 300, 600, 330, 879, 0, 10, 0, 0]).0,
        (Kind::Armour, None)
    );
    // Else a jewel, which the creature then has made.
    assert_eq!(
        kinds(3, &[180, 999, 300, 600, 330, 880, 1, 0]),
        ((Kind::Jewel, None), true)
    );
    // The animals roll no rune and always drop gold past a potion, the ghosts in 600 ‰ more and
    // no weapon or armour.
    assert_eq!(kinds(ANIMALS, &[180, 999, 1]).0, (Kind::Gold, Some(5132)));
    assert_eq!(
        kinds(GHOSTS, &[180, 999, 300, 599, 1]).0,
        (Kind::Gold, Some(5132))
    );
    assert_eq!(
        kinds(GHOSTS, &[180, 999, 300, 600, 0, 0, 2, 1]),
        ((Kind::Jewel, None), true)
    );
}

#[test]
fn a_creature_makes_one_jewel_and_then_the_dagger() {
    let rules = Rules::default();
    let templates = Templates::default();
    let death = death(dead(3, 5, Rank::Common), 5);
    let catalogue = Catalogue::default();
    let mut dropping = Dropping {
        death: &death,
        rules: &rules,
        catalogue: &catalogue,
        templates: &templates,
        // This creature made the last jewel and the last rune.
        last: &mut LastJewel {
            jewel: Some(0),
            rune: Some(0),
        },
    };
    let mut dice = script(&[180, 300, 600, 330, 880]);
    assert_eq!(dropping.kind(0, &mut dice), (Kind::Jewel, None));
    assert!(dice.0.is_empty());
}

#[test]
fn the_type_of_a_drop_is_of_its_level_class_and_quality() {
    let types = [
        (1000, 5, record(ANY, [1, 1], [1, 0, 0])),
        // The Seraphim's, the Gladiator's.
        (1001, 5, record(0x1, [1, 1], [1, 0, 0])),
        (1002, 5, record(0x2, [1, 1], [1, 0, 0])),
        // Of a least level 5, of a least quality 3.
        (1003, 5, record(ANY, [1, 1], [1, 5, 0])),
        (1004, 5, record(ANY, [1, 1], [1, 0, 3])),
        // A set piece, a unique of the word 38 29, a record of the level 6, a record of the
        // flag 0x100000.
        (
            1005,
            5,
            Record {
                set: true,
                ..record(ANY, [1, 1], [1, 0, 0])
            },
        ),
        (1006, 5, record(ANY | UNIQUE, [1, 1], [29, 0, 0])),
        (1007, 5, record(ANY, [6, 6], [1, 0, 0])),
        (1008, 5, record(ANY | EXCLUDED_TOO, [1, 1], [1, 0, 0])),
    ];
    let catalogue = catalogue(&types);
    let seraphim = hero(1);
    let wanted = |level, class, loose, quality| Wanted {
        category: 5,
        level,
        class,
        loose,
        quality,
        kind: None,
    };
    let choose = |wanted, rolls: &[u32]| {
        let mut dice = script(rolls);
        let item = catalogue.choose(wanted, &seraphim, &mut dice);
        assert!(dice.0.is_empty(), "{:?} left", dice.0);
        item
    };
    // For the Seraphim at level 1 (no jitter), in one case of two among those of its bit: 1000
    // and 1001.
    assert_eq!(choose(wanted(1, 1, false, 0), &[1, 0]), Some(1000));
    assert_eq!(choose(wanted(1, 1, false, 0), &[1, 1]), Some(1001));
    // For any class, any of 1000, 1001 and 1002, or the first of three draws the hero wears.
    assert_eq!(choose(wanted(1, 0, false, 0), &[1, 0, 2]), Some(1002));
    assert_eq!(choose(wanted(1, 0, false, 0), &[0, 1, 2, 2, 0]), Some(1000));
    assert_eq!(choose(wanted(1, 0, false, 0), &[0, 1, 2, 2, 2]), Some(1002));
    // Untested levels take 1007; a quality of 3 takes 1004; level 5 takes 1003.
    assert_eq!(choose(wanted(1, 0, true, 0), &[1, 0, 3]), Some(1007));
    assert_eq!(choose(wanted(1, 0, false, 3), &[1, 0, 3]), Some(1004));
    assert_eq!(choose(wanted(5, 0, false, 0), &[1, 0, 3]), Some(1003));
    // No type of the category: none.
    assert_eq!(
        choose(
            Wanted {
                category: 8,
                ..wanted(1, 0, false, 0)
            },
            &[]
        ),
        None
    );
}

#[test]
fn above_level_5_the_jitter_lets_a_type_of_higher_levels_drop() {
    let catalogue = catalogue(&[(2000, 6, record(ANY, [7, 7], [1, 0, 0]))]);
    let wanted = Wanted {
        category: 6,
        level: 6,
        class: 0,
        loose: false,
        quality: 0,
        kind: None,
    };
    let choose = |rolls: &[u32]| catalogue.choose(wanted, &hero(6), &mut script(rolls));
    // A jitter of 1·1: the record's level 7 is not below 7.
    assert_eq!(choose(&[0, 0]), None);
    // Of 2·2: below 10.
    assert_eq!(choose(&[1, 1, 1, 0]), Some(2000));
}

#[test]
fn a_kind_asked_takes_the_weapons_of_that_kind_alone() {
    let weapon = |kind| Record {
        kind,
        ..record(ANY, [1, 1], [1, 0, 0])
    };
    // A sword (1), a bow (7) and a crossbow (12); a creature's crossbow becomes a bow (#1217).
    let catalogue = catalogue(&[
        (1000, 5, weapon(1)),
        (1001, 5, weapon(7)),
        (1002, 5, weapon(12)),
    ]);
    let wanted = |kind| Wanted {
        category: 5,
        level: 1,
        class: 0,
        loose: false,
        quality: 14,
        kind,
    };
    let choose = |kind, rolls: &[u32]| {
        let mut dice = script(rolls);
        let item = catalogue.choose(wanted(kind), &hero(1), &mut dice);
        assert!(dice.0.is_empty(), "{:?} left", dice.0);
        item
    };
    assert_eq!(choose(Some(7), &[1, 0]), Some(1001));
    assert_eq!(choose(Some(1), &[1, 0]), Some(1000));
    assert_eq!(choose(Some(4), &[]), None);
    // None asks for any kind, as a drop does.
    assert_eq!(choose(None, &[1, 0, 2]), Some(1002));
}

#[test]
fn a_type_that_drops_nothing_is_drawn_again_and_a_missing_record_five_times() {
    let rules = Rules::default();
    let templates = Templates::default();
    let death = death(dead(3, 1, Rank::Common), 1);
    let pass = |catalogue: &Catalogue, rolls: Vec<u32>| {
        let mut dropping = Dropping {
            death: &death,
            rules: &rules,
            catalogue,
            templates: &templates,
            last: &mut LastJewel::default(),
        };
        let (mut bonus, mut misses) = (0, 0);
        let mut dice = Script(rolls.into());
        let drop = dropping.pass(350, &mut bonus, &mut misses, &mut dice);
        assert!(dice.0.is_empty(), "{:?} left", dice.0);
        (drop, misses)
    };
    // A try: the chance, the key 17, a healing potion.
    let potion_try = [100, 9999, 0, 0];
    // A healing potion of the word 38 −1 is drawn again, and the next try fails its chance.
    let never = catalogue(&[(5171, 9, record(0, [1, 1], [-1, 0, 0]))]);
    let mut rolls = potion_try.to_vec();
    rolls.push(350);
    assert_eq!(pass(&never, rolls), (None, 0));
    // Without its record, five times, and then made as it is at the sixth try.
    let rolls = potion_try.repeat(6);
    let (drop, misses) = pass(&Catalogue::default(), rolls);
    assert_eq!(misses, 5);
    assert_eq!(
        drop,
        Some(Dropped {
            kind: Kind::Potion,
            item: 5171,
            level: 0,
            key: 17,
            quality: 0,
            amount: 1,
            values: None,
            named: None,
        })
    );
}

/// The template 29 of `wpmod.bin` for the dagger 1712 alone, of physical damage always and of no
/// element, `A` or `D`.
fn dagger_template() -> WeaponModifier {
    let mut words = vec![1712, 0, 0, 0, 0, 1, 50, 10, 100, 64, 77];
    words.resize(53, 0);
    WeaponModifier {
        words,
        entries: Vec::new(),
    }
}

#[test]
fn a_death_drops_the_dagger_where_no_weapon_is_found_and_gold_by_its_level() {
    let rules = Rules::default();
    let templates = Templates::new(&[dagger_template()], &[]);
    // The dagger's record and the first pile's, which has no template, and no weapon listed.
    let pile = Record {
        limits: None,
        ..record(0, [1, 1], [0, 0, 0])
    };
    let catalogue = Catalogue {
        lists: HashMap::new(),
        ..catalogue(&[
            (DAGGER, 5, record(ANY, [1, 1], [1, 0, 0])),
            (GOLD[0], 15, pile),
        ])
    };
    let death = death(dead(3, 1, Rank::Common), 1);
    let take = |rolls: &[u32]| {
        let mut dice = script(rolls);
        let made = drops(
            &death,
            (&rules, &catalogue, &templates),
            &[],
            &mut LastJewel::default(),
            &mut dice,
        );
        assert!(dice.0.is_empty(), "{:?} left", dice.0);
        made
    };
    // The chance, the key 17, no potion, no rune, no gold, the ghosts' roll, a weapon of the
    // levels not tested and of any class, none found; the dagger at R(0, 1) twice, its damage's
    // six rolls and its name's roll.
    let dagger = take(&[
        100, 9999, 180, 999, 300, 600, 0, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 1,
    ]);
    assert_eq!(dagger.len(), 1);
    let drop = dagger[0];
    assert_eq!(
        (drop.kind, drop.item, drop.level, drop.key, drop.quality),
        (Kind::Weapon, DAGGER, 1, 17, 0)
    );
    assert_eq!(drop.values.map(|v| v.damage[0]), Some([12.0, 15.0]));
    // Gold: the key 0 taken to 16, a pile of level 1 and its amount.
    let gold = take(&[100, 0, 180, 999, 299, 5, 3]);
    assert_eq!(
        gold,
        [Dropped {
            kind: Kind::Gold,
            item: 5132,
            level: 0,
            key: 16,
            quality: 0,
            amount: 20,
            values: None,
            named: None,
        }]
    );
    // A failed chance drops nothing.
    assert!(take(&[350]).is_empty());
    // A creature of the heroes' class drops nothing.
    let a_hero = Death {
        dead: dead(HEROES, 1, Rank::Common),
        ..death.clone()
    };
    assert!(
        drops(
            &a_hero,
            (&rules, &catalogue, &templates),
            &[],
            &mut LastJewel::default(),
            &mut script(&[])
        )
        .is_empty()
    );
}

#[test]
fn an_item_is_made_by_its_key_between_lo_and_hi() {
    let rules = Rules::default();
    let templates = Templates::default();
    let catalogue = catalogue(&[(DAGGER, 5, record(ANY, [1, 1], [1, 0, 0]))]);
    let death = death(dead(3, 20, Rank::Common), 20);
    let dropping = Dropping {
        death: &death,
        rules: &rules,
        catalogue: &catalogue,
        templates: &templates,
        last: &mut LastJewel::default(),
    };
    let made = |key, rolls: &[u32]| {
        let mut dice = script(rolls);
        let drop = dropping.make(Kind::Weapon, DAGGER, key, &mut dice);
        assert!(dice.0.is_empty(), "{:?} left", dice.0);
        (drop.key, drop.quality, drop.level)
    };
    // lo 20, hi 21. The keys 2 to 15 with 16 − key at R(lo, hi), rolled twice.
    assert_eq!(made(2, &[0, 1]), (2, 14, 21));
    assert_eq!(made(15, &[1, 0]), (15, 1, 20));
    // 0, a unique left out, with the key 14; 1, a set piece left out, as 2.
    assert_eq!(made(0, &[0, 0]), (0, 14, 20));
    assert_eq!(made(1, &[0, 1]), (2, 14, 21));
    // 16 at lo, 17 at R(10, 20).
    assert_eq!(made(16, &[]), (16, 0, 20));
    assert_eq!(made(17, &[0, 3]), (17, 0, 13));
}

#[test]
fn a_hero_rising_pays_a_share_of_its_gold_by_the_difficulty() {
    // Seen on bronze (#976): 18 kept 18, 500 became 475 and then 452.
    assert_eq!(Difficulty::Bronze.death_toll(18), 0);
    assert_eq!(Difficulty::Bronze.death_toll(500), 25);
    assert_eq!(Difficulty::Bronze.death_toll(475), 23);
    assert_eq!(Difficulty::Silver.death_toll(475), 47);
    assert_eq!(Difficulty::Gold.death_toll(475), 67);
    assert_eq!(Difficulty::Platinum.death_toll(475), 95);
    assert_eq!(Difficulty::Niobium.death_toll(475), 237);
}

#[test]
fn the_scripts_drops_of_a_type_each_make_their_item_in_their_share_of_the_deaths() {
    let rules = Rules::default();
    let (catalogue, templates) = (unlisted(), Templates::default());
    let death = death(dead(3, 5, Rank::Common), 5);
    let mut rng = Rng::of(51, 39, 0);
    // Quests 15015 and 10259 on one type (#930): each item at its own chance, both in some
    // deaths, the random drops only where neither comes.
    let scripted = [(6005, 75), (4931, 65)];
    let deaths = 20_000_u32;
    let (mut first, mut second, mut both, mut random) = (0_u32, 0_u32, 0_u32, 0_u32);
    for _ in 0..deaths {
        let made = drops(
            &death,
            (&rules, &catalogue, &templates),
            &scripted,
            &mut LastJewel::default(),
            &mut rng,
        );
        let has = |item| made.iter().any(|d| d.item == item && d.kind == Kind::Other);
        first += u32::from(has(6005));
        second += u32::from(has(4931));
        both += u32::from(has(6005) && has(4931));
        random += u32::from(!has(6005) && !has(4931) && !made.is_empty());
        assert!(
            made.iter()
                .filter(|d| d.kind == Kind::Other)
                .all(|d| d.amount == 1)
        );
    }
    let per_cent = |n: u32| f64::from(n) * 100.0 / f64::from(deaths);
    assert!((per_cent(first) - 75.0).abs() < 1.0, "{}", per_cent(first));
    assert!(
        (per_cent(second) - 65.0).abs() < 1.0,
        "{}",
        per_cent(second)
    );
    assert!((per_cent(both) - 48.75).abs() < 1.0, "{}", per_cent(both));
    // Neither in 8.75 % of the deaths, which drop at random in 35 % of theirs.
    assert!(
        (per_cent(random) - 3.06).abs() < 0.5,
        "{}",
        per_cent(random)
    );
}

#[test]
fn an_ordinary_creature_drops_in_35_per_cent_of_its_deaths_by_the_kinds_shares() {
    let rules = Rules::default();
    let (catalogue, templates) = (unlisted(), Templates::default());
    // A hero of many arts' levels, whose runes' chance is the least, 0.5 ‰: the kinds as they
    // share out without the runes.
    let death = Death {
        hero: Hero {
            arts: 1000,
            ..hero(5)
        },
        ..death(dead(3, 5, Rank::Common), 5)
    };
    let mut rng = Rng::of(51, 39, 0);
    let deaths = 100_000_u32;
    let mut with_item = 0_u32;
    let mut kinds: HashMap<Kind, u32> = HashMap::new();
    for _ in 0..deaths {
        let made = drops(
            &death,
            (&rules, &catalogue, &templates),
            &[],
            &mut LastJewel::default(),
            &mut rng,
        );
        assert!(made.len() <= 1);
        if let Some(drop) = made.first() {
            with_item += 1;
            *kinds.entry(drop.kind).or_default() += 1;
        }
    }
    let per_cent = |n: u32, of: u32| f64::from(n) * 100.0 / f64::from(of);
    let items = per_cent(with_item, deaths);
    assert!((items - 35.0).abs() < 0.5, "{items}");
    // 18 % potions, 82 % · 30 % gold, of the rest 33 % weapons, of theirs 88 % armour, and jewels.
    for (kind, share) in [
        (Kind::Potion, 18.0),
        (Kind::Gold, 24.6),
        (Kind::Weapon, 18.94),
        (Kind::Armour, 33.84),
        (Kind::Jewel, 4.62),
    ] {
        let got = per_cent(kinds.get(&kind).copied().unwrap_or(0), with_item);
        assert!((got - share).abs() < 0.8, "{kind:?} {got}");
    }
}

#[test]
fn a_creature_makes_one_jewel_over_its_deaths_until_another_makes_one() {
    // A ring and an amulet that any hero wears, beside the potions, the gold and the dagger.
    const RING: u32 = 3001;
    const AMULET: u32 = 3101;
    let rules = Rules::default();
    let templates = Templates::default();
    let mut catalogue = unlisted();
    for (item, category) in [(RING, 8), (AMULET, 20)] {
        catalogue.categories.insert(item, category);
        catalogue
            .records
            .insert(item, record(ANY, [1, 1], [0, 0, 0]));
        catalogue.lists.insert(category, vec![item]);
    }
    let of = |id| {
        death(
            Dead {
                id,
                ..dead(3, 5, Rank::Common)
            },
            5,
        )
    };
    let (goblin, other) = (of(7), of(8));
    let jewels = |made: &[Dropped]| {
        made.iter()
            .filter(|d| matches!(d.item, RING | AMULET))
            .count()
    };
    let mut rng = Rng::of(51, 39, 2);
    // The same creature dying 5,000 times, as the original's driver's `drop --id`: one jewel,
    // the dagger wherever a later pass comes to the jewel (#610).
    let mut last = LastJewel::default();
    let made: Vec<Dropped> = (0..5000)
        .flat_map(|_| {
            drops(
                &goblin,
                (&rules, &catalogue, &templates),
                &[],
                &mut last,
                &mut rng,
            )
        })
        .collect();
    assert_eq!(jewels(&made), 1);
    assert!(
        made.iter()
            .any(|d| d.kind == Kind::Jewel && d.item == DAGGER)
    );
    // Another creature's jewel lets the first make one again.
    while jewels(&drops(
        &other,
        (&rules, &catalogue, &templates),
        &[],
        &mut last,
        &mut rng,
    )) == 0
    {}
    let again = (0..5000)
        .map(|_| {
            jewels(&drops(
                &goblin,
                (&rules, &catalogue, &templates),
                &[],
                &mut last,
                &mut rng,
            ))
        })
        .sum::<usize>();
    assert_eq!(again, 1);
    // Creatures dying once each make theirs: about 1.6 in 100 deaths.
    let many: usize = (0..5000)
        .map(|_| {
            let made = drops(
                &goblin,
                (&rules, &catalogue, &templates),
                &[],
                &mut LastJewel::default(),
                &mut rng,
            );
            jewels(&made)
        })
        .sum();
    assert!((50..=110).contains(&many), "{many}");
}

#[test]
fn a_bosss_calls_each_drop_in_70_per_cent() {
    let rules = Rules::default();
    let (catalogue, templates) = (unlisted(), Templates::default());
    let death = death(dead(3, 5, Rank::Boss), 5);
    let mut rng = Rng::of(51, 39, 1);
    let deaths = 20_000_u32;
    let made: usize = (0..deaths)
        .map(|_| {
            drops(
                &death,
                (&rules, &catalogue, &templates),
                &[],
                &mut LastJewel::default(),
                &mut rng,
            )
            .len()
        })
        .sum();
    // 1 + R(1, 4) calls, 3.5 on average, each with an item in 70 %.
    let mean = f64::from(u32::try_from(made).unwrap()) / f64::from(deaths);
    assert!((mean - 2.45).abs() < 0.05, "{mean}");
}

/// A drop thrown from the origin at 750 along x, 0 along y and no spin, flown at 60 frames a
/// second until it rests or 5 s pass: where it rests, after how long.
fn flown(ground: impl Fn(f32, f32) -> f32) -> (Flight, f32) {
    let mut flight = Flight::thrown(0.0, 0.0, &mut script(&[150, 75, 6, 6, 6]));
    assert_eq!(flight.velocity, [750.0, 0.0, 190.0]);
    let dt = 1.0 / 60.0;
    let mut t = 0.0;
    while !flight.resting && t < 5.0 {
        flight.step(dt, &ground);
        t += dt;
    }
    (flight, t)
}

#[test]
fn a_drop_rises_and_falls_in_place_after_about_1_6_s() {
    let (flight, t) = flown(|_, _| 0.0);
    assert!(flight.resting);
    assert!((1.5..1.65).contains(&t), "{t}");
    // Its velocity along the ground turning back at every frame (#478), it lands within a unit
    // of where it was thrown, as the original's do.
    let [x, y, z] = flight.at;
    assert!((0.0..1.3).contains(&x), "{x}");
    assert_eq!((y, z), (0.0, 0.0));
    assert_eq!(flight.angles, [0.0; 3]);
    // A frame takes it a tenth of the velocity on, and the next back.
    let mut first = Flight::thrown(0.0, 0.0, &mut script(&[150, 75, 6, 6, 6]));
    first.step(0.1, |_, _| 0.0);
    assert!((first.at[0] - 7.5).abs() < 1e-4 && first.velocity[0] < 0.0);
    // The spin turns it as it flies.
    let mut spun = Flight::thrown(0.0, 0.0, &mut script(&[75, 75, 12, 0, 6]));
    spun.step(0.5, |_, _| 0.0);
    assert_eq!(spun.angles, [3.0, -3.0, 0.0]);
}

#[test]
fn a_drop_lands_on_a_raised_cell_at_its_height() {
    let (flight, t) = flown(|_, _| 40.0);
    assert!(flight.resting);
    assert!((1.35..1.45).contains(&t), "{t}");
    assert!((flight.at[2] - 40.0).abs() < f32::EPSILON);
    // Above the throw's height, it lies on the ground at once.
    let (flight, t) = flown(|_, _| 80.0);
    assert!(flight.resting && t < 0.02, "{t}");
    assert!((flight.at[2] - 80.0).abs() < f32::EPSILON);
}

#[test]
fn the_gold_and_the_runes_stay_upright_and_the_small_spiders_drop_nothing() {
    assert!(upright(5132) && upright(5135) && upright(5375) && upright(5631));
    assert!(!upright(5136) && !upright(DAGGER) && !upright(5171));
    assert!(never_drops(71) && never_drops(75) && !never_drops(76));
}

#[test]
fn a_test_throws_an_item_of_its_kind_made_at_its_level() {
    let templates = Templates::new(&[dagger_template()], &[]);
    let bare = Record {
        limits: None,
        ..record(0, [1, 1], [0, 0, 0])
    };
    let catalogue = catalogue(&[
        (DAGGER, 5, record(ANY, [1, 1], [1, 0, 0])),
        (GOLD[2], 15, bare),
        (5171, 9, bare),
        (5400, 28, bare),
        (4097, 16, bare),
    ]);
    let throw = |item, key, rolls: &[u32]| {
        let mut dice = script(rolls);
        let made = thrown(&catalogue, &templates, (item, 7, key, 250), &mut dice);
        assert!(dice.0.is_empty(), "{:?} left", dice.0);
        made
    };
    // The dagger at level 7, of the maker's key 6 for the pass's key 10, its damage's rolls and
    // its name's roll.
    let dagger = throw(DAGGER, 10, &[0, 0, 0, 0, 0, 0, 1]).unwrap();
    assert_eq!(
        (dagger.kind, dagger.level, dagger.key, dagger.quality),
        (Kind::Weapon, 7, 10, 6)
    );
    assert!(dagger.values.is_some());
    assert_eq!(dagger.named, Some(true));
    // Plain at the key 16; the key 1 taken as 2.
    assert_eq!(throw(DAGGER, 16, &[0; 7]).unwrap().quality, 0);
    assert_eq!(throw(DAGGER, 1, &[0; 7]).unwrap().quality, 14);
    // Gold of the amount asked, a potion as it is, a rune as it is (#1183).
    let gold = throw(GOLD[2], 16, &[]).unwrap();
    assert_eq!((gold.kind, gold.amount, gold.level), (Kind::Gold, 250, 0));
    let potion = throw(5171, 16, &[]).unwrap();
    assert_eq!((potion.kind, potion.values), (Kind::Potion, None));
    let rune = throw(5400, 16, &[]).unwrap();
    assert_eq!((rune.kind, rune.values), (Kind::Rune, None));
    // A book of no drop's kind as it is (#1358); a type the table does not hold, none.
    let book = throw(4097, 16, &[]).unwrap();
    assert_eq!(
        (book.kind, book.item, book.values),
        (Kind::Other, 4097, None)
    );
    assert_eq!(throw(9999, 16, &[]), None);
}

#[test]
fn a_rune_comes_by_the_heros_level_its_runes_and_its_arts() {
    let rules = Rules::default();
    let seraphim = |level, runes, arts| Hero {
        creature: 1,
        level,
        runes,
        arts,
        ..Hero::default()
    };
    // x = 1.85·10/(3/3 + 10) above 1: x⁵, held at 2, times 50.
    assert!((rules.rune_chance(10, false, &seraphim(10, 3, 10)) - 100.0).abs() < 1e-4);
    // x = 1.85/3 below 1: x⁴ times the weighing of the levels, times 50.
    let x = 1.85_f64 / 3.0;
    let weighing = f64::from(crate::fight::weighing(4, 1, rules.levels));
    let expected = (weighing * x.powi(4)).max(0.01) * 50.0;
    assert!((f64::from(rules.rune_chance(4, false, &seraphim(1, 0, 3))) - expected).abs() < 1e-3);
    // A boss's or a champion's: x⁵, at least 0.01, times 1.5.
    let ranked = x.powi(5).max(0.01) * 1.5 * 50.0;
    assert!((f64::from(rules.rune_chance(4, true, &seraphim(1, 0, 3))) - ranked).abs() < 1e-3);
    // The Battle Mage's 2.2 a level.
    let mage = Hero {
        creature: 3,
        ..seraphim(1, 0, 4)
    };
    let x = 2.2_f64 / 4.0;
    let expected =
        (f64::from(crate::fight::weighing(1, 1, rules.levels)) * x.powi(4)).max(0.01) * 50.0;
    assert!((f64::from(rules.rune_chance(1, false, &mage)) - expected).abs() < 1e-3);
}

#[test]
fn a_side_quests_rune_comes_by_the_heros_arts_and_runes_over_its_level() {
    let rules = Rules::default();
    let hero = |creature, level, runes, arts| Hero {
        creature,
        level,
        runes,
        arts,
        ..Hero::default()
    };
    // The Seraphim of level 1 with one art at level 1: x = 1/1.85, x⁵·1000 = 46, the count set
    // to 1; a roll of 0 to 46 always below 50.
    let mut given = 0;
    assert_eq!(rules.quest_rune_range(&hero(1, 1, 0, 1), &mut given), 46);
    assert_eq!(given, 1);
    assert!(rules.rune_given(46, &mut Rng::of(50, 39, 0)));
    // Her arts and runes behind the count given: they count as its two thirds and its third,
    // `(3/2 + 6)/4 = 1.75`, x = 1.75/1.85, x⁵·1000 = 757.
    let mut given = 9;
    assert_eq!(rules.quest_rune_range(&hero(1, 4, 0, 1), &mut given), 757);
    assert_eq!(given, 9);
    // The Dwarf and the Daemon take 1 for their runes per level; the range is held at 32,767.
    let mut given = 0;
    assert_eq!(
        rules.quest_rune_range(&hero(8, 1, 0, 30), &mut given),
        32_767
    );
    // A range of 0 rolls 0, below 50.
    assert!(rules.rune_given(0, &mut Rng::of(50, 39, 0)));
    let mut rng = Rng::of(50, 39, 0);
    let given = (0..1000)
        .filter(|_| rules.rune_given(32_767, &mut rng))
        .count();
    assert!(given < 10, "{given}");
}

#[test]
fn a_rune_is_one_of_the_runes_and_a_creature_makes_one_until_another_does() {
    let rules = Rules::default();
    let templates = Templates::default();
    let catalogue = Catalogue {
        readable: [5375, 5376].into_iter().collect(),
        runes: vec![5375, 5376, 5377],
        ..Catalogue::default()
    };
    let rune = |creature: u32, rolls: &[u32], last: &mut LastJewel| {
        let death = Death {
            dead: Dead {
                creature,
                ..dead(3, 5, Rank::Common)
            },
            hero: hero(5),
            ..Death::default()
        };
        let mut dropping = Dropping {
            death: &death,
            rules: &rules,
            catalogue: &catalogue,
            templates: &templates,
            last,
        };
        let mut dice = script(rolls);
        let made = dropping.kind(0, &mut dice);
        assert!(dice.0.is_empty(), "{:?} left", dice.0);
        made
    };
    // Below the 100 ‰ the second of the three runes.
    let mut last = LastJewel::default();
    assert_eq!(rune(76, &[180, 99, 1], &mut last), (Kind::Rune, Some(5376)));
    assert_eq!(last.rune, Some(0));
    // The same creature makes no rune again: gold.
    assert_eq!(
        rune(76, &[180, 299, 1], &mut last),
        (Kind::Gold, Some(5132))
    );
    // A rune whose entry has none, or the types 516, 592 and 593: a potion, the creature still
    // the last to make a rune.
    let mut last = LastJewel::default();
    assert_eq!(
        rune(76, &[180, 0, 2, 0], &mut last),
        (Kind::Potion, Some(5171))
    );
    assert_eq!(last.rune, Some(0));
    let mut last = LastJewel::default();
    assert_eq!(
        rune(592, &[180, 0, 0, 0], &mut last),
        (Kind::Potion, Some(5171))
    );
}

/// A template of the word 38 `word_38` for the type `item` alone.
fn named_template(item: u32, word_38: i32) -> WeaponModifier {
    let mut words = vec![item, 0, 0, 0, 0, 1];
    words.resize(53, 0);
    words[38] = word_38.cast_unsigned();
    WeaponModifier {
        words,
        entries: Vec::new(),
    }
}

/// The name that the maker gives a made item of type `item`, of the values `values` and the
/// roll `coin`, of the names that `texts` holds.
fn made_name(
    tables: (&Rules, &Catalogue, &Templates),
    (item, values, coin): (u32, ItemValues, bool),
    texts: &[&str],
) -> Option<String> {
    let dropped = Dropped {
        kind: Kind::Armour,
        item,
        level: 1,
        key: 15,
        quality: 1,
        amount: 1,
        values: Some(values),
        named: Some(coin),
    };
    name(&dropped, tables, |key| texts.contains(&key))
}

#[test]
fn a_made_item_goes_by_the_words_of_its_element_and_its_class() {
    // The Daemonia's gloves 1221 (#1424): of the class 27, armour, their resistance poison's
    // alone. The original named three of them `Gen_st_gi_Handschuhe`, `…1` and again `…`.
    let rules = Rules::default();
    let templates = Templates::new(&[named_template(1221, 27)], &[]);
    let catalogue = catalogue(&[(1221, 6, record(ANY, [1, 1], [27, 0, 0]))]);
    let tables = (&rules, &catalogue, &templates);
    let poison = ItemValues {
        resistance: [0.0, 0.0, 0.0, 4.0],
        ..ItemValues::default()
    };
    let texts = [
        "EWT_Handschuhe",
        "Gen_st_gi_Handschuhe",
        "Gen_st_gi_Handschuhe1",
    ];
    let named = |coin, texts: &[&str]| made_name(tables, (1221, poison, coin), texts);
    assert_eq!(
        named(false, &texts).as_deref(),
        Some("Gen_st_gi_Handschuhe")
    );
    assert_eq!(
        named(true, &texts).as_deref(),
        Some("Gen_st_gi_Handschuhe1")
    );
    // Without a text for the words, the family's generic name; without that, the type's.
    assert_eq!(named(true, &texts[..1]).as_deref(), Some("EWT_Handschuhe"));
    assert_eq!(named(true, &[]), None);
    // An item that the maker did not make goes by its type's name.
    let fixed = Dropped {
        kind: Kind::Armour,
        item: 1221,
        level: 1,
        key: 15,
        quality: 1,
        amount: 1,
        values: Some(poison),
        named: None,
    };
    assert_eq!(name(&fixed, tables, |_| true), None);
    // The elements by their shares: fire above 50 per cent of the most damage, magic above 40,
    // poison of a bonus to its multiplier twice its value; else physical.
    let elements = [
        "Gen_st_ph_Handschuhe",
        "Gen_st_fe_Handschuhe",
        "Gen_st_ma_Handschuhe",
        "Gen_st_gi_Handschuhe",
    ];
    let element = |values| made_name(tables, (1221, values, false), &elements);
    let damage = |most: [f32; 4]| ItemValues {
        damage: most.map(|most| [1.0, most]),
        ..ItemValues::default()
    };
    assert_eq!(
        element(damage([4.0, 6.0, 0.0, 0.0])).as_deref(),
        Some(elements[1])
    );
    assert_eq!(
        element(damage([5.0, 5.0, 0.0, 0.0])).as_deref(),
        Some(elements[0])
    );
    assert_eq!(
        element(damage([8.0, 2.0, 10.0, 0.0])).as_deref(),
        Some(elements[2])
    );
    let mut bonus = ItemValues::default();
    bonus.bonuses[2] = fight::Bonus {
        condition: 0,
        effect: 0x328,
        value: 20,
    };
    assert_eq!(element(bonus).as_deref(), Some(elements[3]));
}

#[test]
fn a_made_items_bonuses_count_by_their_weight_into_its_names_digit() {
    // The effect 801's record: its first float 200, its third 0, its fourth 100.
    let mut norms = vec![[1.0_f32; 6]; names::NORMS];
    norms[1] = [200.0, 0.0, 0.0, 100.0, 0.0, 2.0];
    let rules = Rules {
        norms,
        ..Rules::default()
    };
    let templates = Templates::new(&[named_template(1221, 27)], &[]);
    let catalogue = catalogue(&[(1221, 6, record(ANY, [1, 1], [27, 0, 0]))]);
    let tables = (&rules, &catalogue, &templates);
    let with = |bonuses: &[(u16, u32, i16)]| {
        let mut values = ItemValues::default();
        for (slot, &(condition, effect, value)) in values.bonuses.iter_mut().zip(bonuses) {
            *slot = fight::Bonus {
                condition,
                effect,
                value,
            };
        }
        values
    };
    let texts: Vec<String> = (1..=5)
        .map(|digit| format!("Gen_st_ph_Handschuhe{digit}"))
        .chain(["Gen_st_ph_Handschuhe".to_owned()])
        .collect();
    let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
    let named = |values, texts: &[&str]| made_name(tables, (1221, values, false), texts);
    // 50 of 801 weighs ⌊50·100/100 + 0.5⌋ = 50, 2.5 in the count, which keeps 2; the effect 840
    // counts 1 more.
    let two = with(&[(0, 801, 50)]);
    assert_eq!(named(two, &texts).as_deref(), Some(texts[1]));
    let three = with(&[(0, 801, 50), (0, 840, 1)]);
    assert_eq!(named(three, &texts).as_deref(), Some(texts[2]));
    // The highest digit with a text, down to the name without one.
    assert_eq!(
        named(three, &[texts[0], texts[5]]).as_deref(),
        Some(texts[0])
    );
    assert_eq!(named(three, &[texts[5]]).as_deref(), Some(texts[5]));
    // Against an opponent's class an eighth, 6, which weighs the least, 1; scaled by an
    // attribute, by the first float, 25, 1.5.
    let against = with(&[(5, 801, 50)]);
    assert_eq!(named(against, &texts).as_deref(), Some(texts[0]));
    let scaled = with(&[(0, 801 | 0x1_0000, 50)]);
    assert_eq!(named(scaled, &texts).as_deref(), Some(texts[0]));
    // At most 5.
    let many = with(&[(0, 801, 500)]);
    assert_eq!(named(many, &texts).as_deref(), Some(texts[4]));
}

#[test]
fn a_made_item_keeps_its_own_name_or_takes_its_weapon_kinds_word() {
    let rules = Rules::default();
    // Gloves of the weapons' category keep a name of their own that has a text, unless of a set.
    let templates = Templates::new(&[named_template(1221, 27)], &[]);
    let values = ItemValues::default();
    let own = ["1221", "Gen_st_ph_Handschuhe"];
    let weapon = catalogue(&[(1221, 5, record(ANY, [1, 1], [27, 0, 0]))]);
    assert_eq!(
        made_name((&rules, &weapon, &templates), (1221, values, false), &own).as_deref(),
        Some(own[0])
    );
    let piece = catalogue(&[(
        1221,
        5,
        Record {
            set: true,
            ..record(ANY, [1, 1], [27, 0, 0])
        },
    )]);
    assert_eq!(
        made_name((&rules, &piece, &templates), (1221, values, false), &own).as_deref(),
        Some(own[1])
    );
    // A template of the word 38 −1: a sword or an axe by its model, two-handed of the kind 2,
    // and a bow of the kind 7.
    let templates = Templates::new(&[named_template(1800, -1)], &[]);
    let of_kind = |kind, model: &str| {
        let mut catalogue = catalogue(&[(
            1800,
            5,
            Record {
                kind,
                ..record(ANY, [1, 1], [-1, 0, 0])
            },
        )]);
        catalogue.models.insert(1800, model.to_owned());
        catalogue
    };
    let words = [
        "Gen_st_ph_Schwert",
        "Gen_st_ph_2HSchwert",
        "Gen_st_ph_Axt",
        "Gen_st_ph_2HAxt",
        "Gen_st_ph_Bogen",
    ];
    let named = |catalogue: &Catalogue| {
        made_name(
            (&rules, catalogue, &templates),
            (1800, values, false),
            &words,
        )
    };
    assert_eq!(named(&of_kind(1, "SWORD_A.GRN")).as_deref(), Some(words[0]));
    assert_eq!(
        named(&of_kind(2, "SCHWERT_B.GRN")).as_deref(),
        Some(words[1])
    );
    assert_eq!(named(&of_kind(1, "AXE_C.GRN")).as_deref(), Some(words[2]));
    assert_eq!(named(&of_kind(2, "AXT_D.GRN")).as_deref(), Some(words[3]));
    assert_eq!(named(&of_kind(7, "BOW.GRN")).as_deref(), Some(words[4]));
}

#[test]
fn a_scripts_random_item_rolls_its_kind_its_class_and_its_level() {
    /// Dice that answer from a list, then 0.
    struct Answers(Vec<u32>);
    impl Dice for Answers {
        fn below(&mut self, n: u32) -> u32 {
            if self.0.is_empty() {
                0
            } else {
                self.0.remove(0).min(n.saturating_sub(1))
            }
        }
    }
    // Not a potion (0), a weapon (1), of the hero's class (1), 2 levels more, on gold.
    let item = script_item(10, 3, Difficulty::Gold, &mut Answers(vec![0, 1, 1, 2]));
    let ScriptItem::Wanted(wanted) = item else {
        panic!("{item:?}");
    };
    assert_eq!((wanted.category, wanted.class, wanted.level), (5, 3, 15));
    assert!(wanted.loose && wanted.quality == 14);
    // Neither a weapon nor an armour: the class and the level, then the category.
    let item = script_item(
        1,
        3,
        Difficulty::Bronze,
        &mut Answers(vec![0, 0, 0, 0, 0, 0]),
    );
    assert_eq!(
        item,
        ScriptItem::Wanted(Wanted {
            category: 5,
            level: 1,
            class: 0,
            loose: true,
            quality: 14,
            kind: None,
        })
    );
    assert!(matches!(
        script_item(1, 3, Difficulty::Bronze, &mut Answers(vec![1])),
        ScriptItem::Potion(_)
    ));
    // The weights summed: 497 of 995 falls in the helmets' (17).
    assert_eq!(random_category(&mut Answers(vec![16_384])), 17);
    assert_eq!(random_category(&mut Answers(vec![32_767])), 0);
}
