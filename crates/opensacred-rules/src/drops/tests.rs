use std::collections::VecDeque;

use opensacred_data::tables::WeaponModifier;

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
        skills: Vec::new(),
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
    }
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
            jewel: false,
        };
        let mut dice = script(rolls);
        let first = dropping.kind(0, &mut dice);
        assert!(dice.0.is_empty(), "{:?} left", dice.0);
        (first, dropping.jewel)
    };
    // A potion in 180 ‰.
    assert_eq!(kinds(3, &[179, 0]).0, (Kind::Potion, Some(5171)));
    // Gold in 300 ‰ of the rest, a pile by the level.
    assert_eq!(kinds(3, &[180, 299, 1]).0, (Kind::Gold, Some(5132)));
    // A weapon in 330 ‰ of the rest: of the levels tested in one case of three, for the hero's
    // class in one of two; none of an empty catalogue.
    assert_eq!(
        kinds(3, &[180, 300, 600, 329, 1, 1]).0,
        (Kind::Weapon, None)
    );
    // Armour in 880 ‰ of the rest, of eleven categories, the bridles drawn once again.
    assert_eq!(
        kinds(3, &[180, 300, 600, 330, 879, 0, 10, 0, 0]).0,
        (Kind::Armour, None)
    );
    // Else a jewel, which the creature then has made.
    assert_eq!(
        kinds(3, &[180, 300, 600, 330, 880, 1, 0]),
        ((Kind::Jewel, None), true)
    );
    // The animals always drop gold past a potion, the ghosts in 600 ‰ more and no weapon or
    // armour.
    assert_eq!(kinds(ANIMALS, &[180, 999, 1]).0, (Kind::Gold, Some(5132)));
    assert_eq!(
        kinds(GHOSTS, &[180, 300, 599, 1]).0,
        (Kind::Gold, Some(5132))
    );
    assert_eq!(
        kinds(GHOSTS, &[180, 300, 600, 0, 0, 2, 1]),
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
        jewel: true,
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
    };
    let choose = |rolls: &[u32]| catalogue.choose(wanted, &hero(6), &mut script(rolls));
    // A jitter of 1·1: the record's level 7 is not below 7.
    assert_eq!(choose(&[0, 0]), None);
    // Of 2·2: below 10.
    assert_eq!(choose(&[1, 1, 1, 0]), Some(2000));
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
            jewel: false,
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
        let made = drops(&death, &rules, &catalogue, &templates, &mut dice);
        assert!(dice.0.is_empty(), "{:?} left", dice.0);
        made
    };
    // The chance, the key 17, no potion, no gold, the ghosts' roll, a weapon of the levels not
    // tested and of any class, none found; the dagger at R(0, 1) twice, its damage's six rolls.
    let dagger = take(&[100, 9999, 180, 300, 600, 0, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0]);
    assert_eq!(dagger.len(), 1);
    let drop = dagger[0];
    assert_eq!(
        (drop.kind, drop.item, drop.level, drop.key, drop.quality),
        (Kind::Weapon, DAGGER, 1, 17, 0)
    );
    assert_eq!(drop.values.map(|v| v.damage[0]), Some([12.0, 15.0]));
    // Gold: the key 0 taken to 16, a pile of level 1 and its amount.
    let gold = take(&[100, 0, 180, 299, 5, 3]);
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
        }]
    );
    // A failed chance drops nothing.
    assert!(take(&[350]).is_empty());
    // A creature of the heroes' class drops nothing.
    let a_hero = Death {
        dead: dead(HEROES, 1, Rank::Common),
        ..death.clone()
    };
    assert!(drops(&a_hero, &rules, &catalogue, &templates, &mut script(&[])).is_empty());
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
        jewel: false,
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
fn an_ordinary_creature_drops_in_35_per_cent_of_its_deaths_by_the_kinds_shares() {
    let rules = Rules::default();
    let (catalogue, templates) = (unlisted(), Templates::default());
    let death = death(dead(3, 5, Rank::Common), 5);
    let mut rng = Rng::of(51, 39, 0);
    let deaths = 100_000_u32;
    let mut with_item = 0_u32;
    let mut kinds: HashMap<Kind, u32> = HashMap::new();
    for _ in 0..deaths {
        let made = drops(&death, &rules, &catalogue, &templates, &mut rng);
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
fn a_bosss_calls_each_drop_in_70_per_cent() {
    let rules = Rules::default();
    let (catalogue, templates) = (unlisted(), Templates::default());
    let death = death(dead(3, 5, Rank::Boss), 5);
    let mut rng = Rng::of(51, 39, 1);
    let deaths = 20_000_u32;
    let made: usize = (0..deaths)
        .map(|_| drops(&death, &rules, &catalogue, &templates, &mut rng).len())
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
    ]);
    let throw = |item, key, rolls: &[u32]| {
        let mut dice = script(rolls);
        let made = thrown(&catalogue, &templates, (item, 7, key, 250), &mut dice);
        assert!(dice.0.is_empty(), "{:?} left", dice.0);
        made
    };
    // The dagger at level 7, of the maker's key 6 for the pass's key 10, its damage's rolls.
    let dagger = throw(DAGGER, 10, &[0, 0, 0, 0, 0, 0]).unwrap();
    assert_eq!(
        (dagger.kind, dagger.level, dagger.key, dagger.quality),
        (Kind::Weapon, 7, 10, 6)
    );
    assert!(dagger.values.is_some());
    // Plain at the key 16; the key 1 taken as 2.
    assert_eq!(throw(DAGGER, 16, &[0; 6]).unwrap().quality, 0);
    assert_eq!(throw(DAGGER, 1, &[0; 6]).unwrap().quality, 14);
    // Gold of the amount asked, a potion as it is, a rune of no drop's kind.
    let gold = throw(GOLD[2], 16, &[]).unwrap();
    assert_eq!((gold.kind, gold.amount, gold.level), (Kind::Gold, 250, 0));
    let potion = throw(5171, 16, &[]).unwrap();
    assert_eq!((potion.kind, potion.values), (Kind::Potion, None));
    assert_eq!(throw(5400, 16, &[]), None);
    assert_eq!(throw(9999, 16, &[]), None);
}
