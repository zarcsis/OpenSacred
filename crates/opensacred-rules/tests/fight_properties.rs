//! Properties of the fight's formulas on random inputs (#1428), which run in CI where the
//! oracle's vectors do not (`tests/oracle.rs`): each holds on every vector of the oracle's sets
//! as well (`the_fights_properties_hold_on_the_originals_vectors` there). The inputs keep to the
//! game's ranges, the levels 1 to 216 and non-negative attacks, defences, damages and resistances
//! up to twice the largest of the vectors, with a bias toward their bounds and toward the band of
//! ±0.001 where a defence or a resistance divides nothing.

use opensacred_rules::fight::{
    creature_earns, dealt, exponent, hit_chance, level_end, level_of, level_start,
};
use proptest::prelude::*;

/// The top level (`0x57F6E0`'s table).
const TOP: u32 = 216;
/// The largest attack, defence, damage or resistance taken: twice the vectors' largest, 5,000.
const MOST: f32 = 10_000.0;
/// The hit chance against a defence within ±0.001, whatever the attack: `1 − 1/(1 + 10,000)`.
const NO_DEFENCE: f32 = 1.0 - 1.0 / 10_001.0;
/// The share of a value that the floats' rounding may take off it.
const ROUNDING: f64 = 1e-6;

/// A level, its bounds the more often.
fn level() -> impl Strategy<Value = u32> {
    prop_oneof![1 => Just(1), 1 => Just(TOP), 8 => 1..=TOP]
}

/// A level below the top, its bounds the more often.
fn below_top() -> impl Strategy<Value = u32> {
    prop_oneof![1 => Just(1), 1 => Just(TOP - 1), 8 => 1..TOP]
}

/// An experience, at and about the levels' ends the more often, up past the top level's end.
fn points() -> impl Strategy<Value = u64> {
    prop_oneof![
        1 => Just(0),
        2 => (1..=TOP).prop_map(level_end),
        2 => (1..=TOP).prop_map(|level| level_end(level) - 1),
        5 => 0..=level_end(TOP) + 1_000,
    ]
}

/// A divisor of the weight `0x43EEE0` within ±0.001, which it takes as dividing nothing: the
/// floats below the float 0.001, which lies a little above the double 0.001.
fn band() -> impl Strategy<Value = f32> {
    prop_oneof![1 => Just(0.0), 3 => 0.0..0.001_f32]
}

/// A divisor that divides: from the float 0.001 up.
fn dividing() -> BoxedStrategy<f32> {
    prop_oneof![1 => Just(0.001), 1 => Just(MOST), 8 => 0.001..=MOST].boxed()
}

/// An attack, a defence, a damage or a resistance, its bounds and the band the more often.
fn amount() -> BoxedStrategy<f32> {
    prop_oneof![1 => Just(0.0), 1 => Just(MOST), 2 => band(), 6 => 0.0..=MOST].boxed()
}

/// An exponent of the damage: of the attacker's levels above the defender's (`0x56B130`), whose
/// exponent is not monotone in them, or any between its least and its most.
fn exponents() -> BoxedStrategy<f32> {
    prop_oneof![(0..=255_u32).prop_map(exponent), 1.0..=7.65_f32].boxed()
}

/// Two values of `values`, the lesser first.
fn ordered(values: BoxedStrategy<f32>) -> impl Strategy<Value = (f32, f32)> {
    (values.clone(), values).prop_map(|(a, b)| if a <= b { (a, b) } else { (b, a) })
}

proptest! {
    #[test]
    fn a_levels_end_lies_below_the_next_and_is_where_the_next_starts(level in below_top()) {
        prop_assert!(level_end(level) < level_end(level + 1));
        prop_assert_eq!(level_start(level + 1), level_end(level));
    }

    #[test]
    fn a_levels_end_is_a_whole_hundred_and_the_levels_spans_grow(level in below_top()) {
        prop_assert_eq!(level_end(level) % 100, 0);
        if level + 1 < TOP {
            let span = |level| level_end(level + 1) - level_end(level);
            prop_assert!(span(level) < span(level + 1));
        }
    }

    #[test]
    fn an_experience_gives_its_level_whatever_the_level_held(
        points in points(),
        held in level(),
        other in level(),
    ) {
        if points >= level_end(1) {
            prop_assert_eq!(level_of(points, held), level_of(points, other));
        } else {
            prop_assert_eq!(level_of(points, held), held);
        }
    }

    #[test]
    fn a_levels_end_is_the_next_levels_first_experience(level in below_top(), held in level()) {
        prop_assert_eq!(level_of(level_end(level), held), level + 1);
        if level >= 2 {
            prop_assert_eq!(level_of(level_end(level) - 1, held), level);
        }
    }

    #[test]
    fn a_hit_chance_lies_below_one(attack in amount(), defence in amount()) {
        let chance = hit_chance(attack, defence);
        prop_assert!((0.0..1.0).contains(&chance), "{attack} against {defence}: {chance}");
    }

    #[test]
    fn a_hit_chance_rises_with_the_attack_and_falls_with_the_defence(
        (less, more) in ordered(amount()),
        (low, high) in ordered(dividing()),
    ) {
        prop_assert!(hit_chance(less, low) <= hit_chance(more, low));
        prop_assert!(hit_chance(less, low) >= hit_chance(less, high));
    }

    #[test]
    fn against_no_defence_a_hit_chance_is_the_same_whatever_the_attack(
        attack in amount(),
        defence in band(),
    ) {
        prop_assert_eq!(hit_chance(attack, defence), NO_DEFENCE);
    }

    #[test]
    fn the_damage_let_through_lies_within_the_damage(
        damage in amount(),
        resistance in amount(),
        k in exponents(),
    ) {
        let through = dealt(damage, resistance, k);
        prop_assert!((0.0..=damage).contains(&through), "{damage} against {resistance}: {through}");
        prop_assert_eq!(dealt(0.0, resistance, k), 0.0);
    }

    #[test]
    fn the_damage_let_through_falls_with_the_resistance_and_rises_with_the_exponent(
        damage in amount(),
        (low, high) in ordered(dividing()),
        (lesser, greater) in ordered(exponents()),
    ) {
        prop_assert!(dealt(damage, low, lesser) >= dealt(damage, high, lesser));
        prop_assert!(dealt(damage, low, lesser) <= dealt(damage, low, greater));
    }

    #[test]
    fn against_no_resistance_a_damage_lets_all_but_a_ten_thousandth_through(
        damage in amount(),
        resistance in band(),
        k in exponents(),
    ) {
        let through = f64::from(dealt(damage, resistance, k));
        let least = f64::from(damage) * (1.0 - 1.0 / 10_001.0) * (1.0 - ROUNDING);
        prop_assert!(through >= least, "{damage} against {resistance}: {through}");
    }

    #[test]
    fn at_no_level_above_a_damage_lets_its_share_through(
        damage in amount(),
        resistance in dividing(),
    ) {
        let (d, r) = (f64::from(damage), f64::from(resistance));
        let share = d * d / (d + r);
        let through = f64::from(dealt(damage, resistance, exponent(0)));
        prop_assert!(
            (through - share).abs() <= 1e-5 * share.max(1.0),
            "{damage} against {resistance}: {through}, d²/(d + r) {share}"
        );
    }

    #[test]
    fn the_exponent_rises_with_the_levels_above_up_to_98(
        (fewer, more) in (0..98_u32).prop_flat_map(|fewer| (Just(fewer), fewer + 1..=98)),
    ) {
        prop_assert!(exponent(fewer) < exponent(more));
    }

    #[test]
    fn a_creature_keeps_its_points_below_the_top_and_rises_only_to_their_level(
        points in points(),
        level in level(),
        earned in points(),
    ) {
        let mut kept = points;
        let reached = creature_earns(&mut kept, level, earned);
        prop_assert!(kept < level_end(TOP));
        prop_assert_eq!(kept, (points + earned).min(level_end(TOP) - 1));
        let of_points = level_of(kept, level);
        prop_assert_eq!(reached, (of_points > level).then_some(of_points));
    }
}
