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
    // Their strikes' whole damage times D: nothing on bronze, 2.34 on niobium (#1727).
    assert_eq!(
        stats(&wearing(39, 6), Difficulty::Niobium).whole_damage,
        Some(2.34)
    );
    assert_eq!(
        stats(&wearing(39, 5), Difficulty::Niobium).whole_damage,
        None
    );
    // The whole set 28 adds 10·D to the survival bonus's share (#1761).
    assert!(close(
        stats(&wearing(28, 6), Difficulty::Gold).survival,
        16.7
    ));
    assert_eq!(stats(&wearing(28, 5), Difficulty::Gold).survival, 0.0);
    assert!(
        (crate::fight::survival_raise(121, 0.0) - 1.25).abs() < 1e-9,
        "half of the curve's 50 after 121 minutes"
    );
    // Set 60's flag only while one of its pieces holds others in its three first sockets.
    assert_eq!(stats(&wearing(60, 4), Difficulty::Bronze).flags, 0);
    let filled = wearing(60, 4).filled(60);
    assert_eq!(stats(&filled, Difficulty::Bronze).flags, 0x100_0000);
    // Set 23 has no case; the specials wait for the wearer's values.
    assert_eq!(
        stats(&wearing(23, 6), Difficulty::Bronze),
        Stats {
            worn: wearing(23, 6),
            factor: 1.0,
            ..Stats::default()
        }
    );
    assert_eq!(
        stats(&wearing(24, 5), Difficulty::Silver).specials,
        [Special::StrengthPart(53.0)]
    );
}

#[test]
fn the_list_tells_the_bonuses_reached_as_they_are_applied() {
    // Set 2 at 5 on gold, in its bonuses' order: the skill 10 that its text calls a life leech.
    let worn = wearing(2, 5);
    assert_eq!(
        told(2, &worn, Difficulty::Gold),
        [
            Told::Bonus(Bonus::Attribute(1, 16.0)),
            Told::Bonus(Bonus::Attribute(3, 16.0)),
            Told::Bonus(Bonus::Skill(9, 5)),
            Told::Bonus(Bonus::Skill(11, 5)),
            Told::Bonus(Bonus::Skill(10, 3)),
        ]
    );
    // A set's own lines alone.
    let both = Worn::of((0..5).map(|i| (7000 + i, 2)).chain([(7100, 3), (7101, 3)]));
    assert_eq!(
        told(2, &both, Difficulty::Gold),
        told(2, &worn, Difficulty::Gold)
    );
    assert!(told(3, &both, Difficulty::Gold).is_empty());
    assert!(both.wears(7100) && !both.wears(7102));
    // Set 3 at 4: its arts, and no line of the flag that nothing reads yet.
    assert_eq!(
        told(3, &wearing(3, 4), Difficulty::Bronze),
        [Told::Bonus(Bonus::Arts(&[1014, 1015, 1016], 2))]
    );
}

#[test]
fn the_whole_damage_and_the_survival_tell_their_shares() {
    // Set 39 at 6: the whole damage's line none on bronze, +134 % on niobium.
    let worn = wearing(39, 6);
    assert!(
        !told(39, &worn, Difficulty::Bronze)
            .iter()
            .any(|line| matches!(line, Told::WholeDamage(_)))
    );
    assert!(matches!(
        told(39, &worn, Difficulty::Niobium).last(),
        Some(&Told::WholeDamage(share)) if close(share, 1.34)
    ));
    // Set 28's survival bonus last, from the six pieces the stats add it at.
    assert!(matches!(
        told(28, &wearing(28, 6), Difficulty::Gold).last(),
        Some(&Told::Survival(share)) if close(share, 16.7)
    ));
    assert!(
        !told(28, &wearing(28, 5), Difficulty::Gold)
            .iter()
            .any(|line| matches!(line, Told::Survival(_)))
    );
    // Set 53's seven pieces turn the items into gold, its line last.
    assert!(into_gold(&wearing(53, 7)) && !into_gold(&wearing(53, 6)));
    assert_eq!(
        told(53, &wearing(53, 7), Difficulty::Bronze).last(),
        Some(&Told::IntoGold)
    );
    assert!(!told(53, &wearing(53, 6), Difficulty::Bronze).contains(&Told::IntoGold));
}

#[test]
fn the_readers_of_the_counts_take_their_shares() {
    // Set 19 at 5 on gold: a kill's experience 16.7 % more, its line after its bonuses.
    let nineteen = stats(&wearing(19, 5), Difficulty::Gold);
    assert!(close(nineteen.experience(), 0.167));
    assert!(matches!(
        told(19, &wearing(19, 5), Difficulty::Gold).last(),
        Some(&Told::Experience(share)) if close(share, 0.167)
    ));
    // Set 1 at 6 on niobium: the rising chance 90 cut to ⌊90·(100 − 70.2)·0.01⌋ = 26.
    let feac = stats(&wearing(1, 6), Difficulty::Niobium);
    assert_eq!(feac.rising(90), 26);
    assert_eq!(stats(&wearing(1, 5), Difficulty::Niobium).rising(90), 90);
    // Set 3 at 8 on silver: the luck 100 to ⌊100·1.268 + 0.5⌋ = 127; set 21 never reaches its
    // reader, asking six of its five pieces.
    assert!(close(
        stats(&wearing(3, 8), Difficulty::Silver).luck(100.0),
        127.0
    ));
    assert!(close(
        stats(&wearing(21, 5), Difficulty::Silver).luck(100.0),
        100.0
    ));
    // Set 1 at 5 on gold against the undead: fire times 2.002 and 1.501, the others 1.501; none
    // against a dragon. Its fire's line at 2, its four kinds' at 5.
    let feac = stats(&wearing(1, 5), Difficulty::Gold);
    let undead = feac.against(UNDEAD);
    assert!(close(undead[FIRE], 2.002 * 1.501) && close(undead[PHYSICAL], 1.501));
    assert_eq!(feac.against(DRAGONS), [1.0; KINDS]);
    assert!(matches!(
        told(1, &wearing(1, 5), Difficulty::Gold)[..],
        [Told::Against(UNDEAD, &[FIRE], fire), Told::Against(UNDEAD, ALL_KINDS, all)]
            if close(fire, 1.002) && close(all, 0.501)
    ));
    // Set 28 at 4 on niobium: the defence against a shot times 2.17.
    assert!(close(
        stats(&wearing(28, 4), Difficulty::Niobium).ranged_defence(),
        2.17
    ));
    assert!(close(
        stats(&wearing(28, 3), Difficulty::Niobium).ranged_defence(),
        1.0
    ));
    // The sight: set 29 at 3 and one of sets 52 and 57, once though both hold.
    let sight = Worn::of(
        (0..3)
            .map(|i| (7200 + i, 29))
            .chain((0..4).map(|i| (7300 + i, 52)))
            .chain((0..6).map(|i| (7400 + i, 57))),
    );
    assert!(close(stats(&sight, Difficulty::Bronze).sight(), 150.0));
    assert_eq!(
        told(29, &sight, Difficulty::Gold).last(),
        Some(&Told::Sight(50.0 * 1.67))
    );
    // Two sets of luck in turn, each rounded: 127, then ⌊127·1.268 + 0.5⌋ = 161.
    let both = Worn::of(
        (0..8)
            .map(|i| (7000 + i, 3))
            .chain((0..7).map(|i| (7100 + i, 9))),
    );
    assert!(close(stats(&both, Difficulty::Silver).luck(100.0), 161.0));
    // The stun's flag: set 64 at 3 adds 0.9·D, set 42 at 5 0.05·D; one term alone.
    assert!(close(
        stats(&wearing(64, 3), Difficulty::Gold).stun(10.0),
        10.0 + 0.9 * 1.67
    ));
    assert!(close(
        stats(&wearing(42, 5), Difficulty::Bronze).stun(10.0),
        10.05
    ));
    assert!(close(
        stats(&wearing(42, 4), Difficulty::Bronze).stun(10.0),
        10.0
    ));
    assert!(matches!(
        told(64, &wearing(64, 3), Difficulty::Bronze)[..],
        [.., Told::Stun(term)] if close(term, 0.9)
    ));
    // Six pieces of set 28 raise the creatures' levels by the difficulty's factor truncated.
    assert_eq!(stats(&wearing(28, 6), Difficulty::Gold).played_part(), 1);
    assert_eq!(stats(&wearing(28, 6), Difficulty::Niobium).played_part(), 2);
    assert_eq!(stats(&wearing(28, 5), Difficulty::Niobium).played_part(), 0);
    // Seven pieces of set 26 let the Dark Elf go unseen, six do not.
    assert!(stats(&wearing(26, 7), Difficulty::Bronze).unseen());
    assert!(!stats(&wearing(26, 6), Difficulty::Bronze).unseen());
    assert_eq!(
        told(26, &wearing(26, 7), Difficulty::Bronze).last(),
        Some(&Told::Unseen)
    );
    // Set 61 at 4 adds 0.15·D to the killer's exorcism, set 54 at 7 0.3·D after it.
    let both = Worn::of(
        (0..4)
            .map(|i| (7500 + i, 61))
            .chain((0..7).map(|i| (7600 + i, 54))),
    );
    assert!(close(
        stats(&both, Difficulty::Platinum).exorcist(10.0),
        10.9
    ));
    assert!(close(
        stats(&wearing(61, 3), Difficulty::Platinum).exorcist(10.0),
        10.0
    ));
    // Set 16 at 6 adds 30·D to the struck hero's 866, its line held at the difficulty's most;
    // set 30 at 5 adds 10·D to the 856 of a hit.
    let gold = wearing(16, 6);
    assert!(close(
        stats(&gold, Difficulty::Gold).gold_for_health(5.0),
        55.1
    ));
    assert!(close(
        stats(&wearing(16, 5), Difficulty::Gold).gold_for_health(5.0),
        5.0
    ));
    assert!(matches!(
        told(16, &gold, Difficulty::Gold).last(),
        Some(&Told::GoldForHealth(chance)) if close(chance, 50.1)
    ));
    assert!(close(
        stats(&wearing(30, 5), Difficulty::Silver).gold_pile(2.0),
        15.4
    ));
    assert!(matches!(
        told(30, &wearing(30, 5), Difficulty::Silver).last(),
        Some(&Told::GoldPile(chance)) if close(chance, 13.4)
    ));
}
