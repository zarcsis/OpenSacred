use super::*;

/// `n` distinct pieces of the set `set`.
fn wearing(set: u8, n: u32) -> Worn {
    Worn::of((0..n).map(|i| (7000 + i, set)))
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn the_difficulty_scales_by_its_factor() {
    assert_eq!(Difficulty::ALL.map(factor), [1.0, 1.34, 1.67, 2.0, 2.34]);
    // `⌊c·D⌋` as the emulation gave it: 10 → 10, 13, 16, 20, 23; 3 → 3, 4, 5, 6, 7.
    assert_eq!(
        Difficulty::ALL.map(|d| whole(10.0, factor(d))),
        [10.0, 13.0, 16.0, 20.0, 23.0]
    );
    assert_eq!(
        Difficulty::ALL.map(|d| whole(3.0, factor(d))),
        [3.0, 4.0, 5.0, 6.0, 7.0]
    );
}

#[test]
fn the_pieces_count_by_their_distinct_types() {
    // Two of one type count once, an item of no set none.
    let worn = Worn::of([(7001, 2), (7001, 2), (7002, 2), (5000, 0), (7100, 9)]);
    assert_eq!((worn.count(2), worn.count(9), worn.count(0)), (2, 1, 0));
    assert_eq!(worn.sets().collect::<Vec<_>>(), [(2, 2), (9, 1)]);
}

#[test]
fn a_set_gives_its_rows_from_their_counts_on() {
    // Set 2 at 3 pieces on gold: a1 and a3 +16; its skills wait for 4.
    let three = stats(&wearing(2, 3), Difficulty::Gold);
    assert_eq!(three.attributes, [0.0, 16.0, 0.0, 16.0, 0.0, 0.0]);
    assert!(three.skills.is_empty());
    // At 5 the skills 9 and 11 of ⌊3·D⌋ and the skill 10 of ⌊2·D⌋.
    let five = stats(&wearing(2, 5), Difficulty::Gold);
    assert_eq!(five.skills, [(9, 5), (11, 5), (10, 3)]);
}

#[test]
fn the_values_take_the_difficulty_as_the_switch_does() {
    // Set 7 at 5 on niobium: the attack speed +46, the skill 25 +7, A's multiplier +0.351.
    let seven = stats(&wearing(7, 5), Difficulty::Niobium);
    assert_eq!(
        (seven.attack_speed, seven.skills.as_slice()),
        (46.0, &[(25, 7)][..])
    );
    assert!(close(seven.attack_times, 0.351));
    // Set 22 at 5 on silver: the fire resistance +40, its weapons' multiplier and its factor
    // +1.34.
    let starion = stats(&wearing(22, 5), Difficulty::Silver);
    assert_eq!(starion.resistances, [0.0, 40.0, 0.0, 0.0]);
    assert!(close(starion.weapon[1], 1.34) && close(starion.resistance_factors[1], 1.34));
    // Set 12 at 4 on bronze: the skill 19's ⌊2·D⌋ that its text calls ⌊3·D⌋.
    let octanion = stats(&wearing(12, 4), Difficulty::Bronze);
    assert_eq!(octanion.skills, [(14, 3), (19, 2)]);
    assert_eq!(octanion.attributes[4], 10.0);
    // Set 6 at 5 on bronze: a3 of ⌊5·D⌋ at 4 and ⌊10·D⌋ more at 5.
    assert_eq!(
        stats(&wearing(6, 5), Difficulty::Bronze).attributes[3],
        15.0
    );
    // Set 51 at 4 on niobium: the pace times 1.15·D.
    let haste = stats(&wearing(51, 4), Difficulty::Niobium);
    assert!(haste.pace_times.is_some_and(|times| close(times, 2.691)));
}

#[test]
fn the_flags_and_the_filled_sets_bonuses() {
    // The six sets' last flags, of the group 0x7C2 that multiplies the whole damage by D.
    for (set, pieces, bit) in [(39, 6, 0x2), (41, 6, 0x400), (44, 5, 0x40), (45, 5, 0x200)] {
        assert_eq!(
            stats(&wearing(set, pieces), Difficulty::Bronze).flags & 0x7C2,
            bit
        );
    }
    // Set 60's flag only while one of its pieces holds others in its three first sockets.
    assert_eq!(stats(&wearing(60, 4), Difficulty::Bronze).flags, 0);
    let filled = wearing(60, 4).filled(60);
    assert_eq!(stats(&filled, Difficulty::Bronze).flags, 0x100_0000);
    // Set 23 has no case; the specials wait for the wearer's values.
    assert_eq!(stats(&wearing(23, 6), Difficulty::Bronze), Stats::default());
    assert_eq!(
        stats(&wearing(24, 5), Difficulty::Silver).specials,
        [Special::StrengthPart(53.0)]
    );
}
