use super::*;
use crate::spawning::Script;

#[test]
fn a_zhadum_kills_one_at_random_a_second_while_any_is_within_reach() {
    let mut zhadum = Zhadum::default();
    let mut dice = Script::new(vec![7, 3]);
    assert_eq!(zhadum.strike(0, &mut dice), None);
    // Of three, the roll 7 % 3.
    assert_eq!(zhadum.strike(3, &mut dice), Some(1));
    assert_eq!((zhadum.cooldown, zhadum.hold), (1.0, 1.0));
    // Not again within its second.
    zhadum.update(0.5);
    assert_eq!(zhadum.strike(3, &mut dice), None);
    zhadum.update(0.5);
    assert_eq!(zhadum.cooldown, 0.0);
    assert_eq!(zhadum.strike(2, &mut dice), Some(1));
    // A cooldown never falls below 0.
    let mut idle = Zhadum::default();
    idle.update(0.25);
    assert_eq!(idle, Zhadum::default());
}

#[test]
fn its_glow_holds_a_second_and_fades_over_two_thirds_of_one() {
    let mut zhadum = Zhadum::default();
    assert_eq!(zhadum.glow(), 0.0);
    zhadum.strike(1, &mut Script::default());
    zhadum.update(0.25);
    assert_eq!(zhadum.glow(), 1.0);
    zhadum.update(0.5);
    assert_eq!((zhadum.hold, zhadum.fade), (0.25, 0.0));
    // The hold ends below 0, the fade starting whole and falling in the same run.
    zhadum.update(0.375);
    assert_eq!(zhadum.hold, 0.0);
    assert!((zhadum.fade - (1.0 - 1.5 * 0.375)).abs() < 1e-6);
    assert_eq!(zhadum.glow(), (255.0_f32 * zhadum.fade).floor() / 255.0);
    // A hold that ends at 0 exactly starts no fade.
    let mut exact = Zhadum {
        hold: 0.5,
        ..Zhadum::default()
    };
    exact.update(0.5);
    assert_eq!((exact.hold, exact.fade), (0.0, 0.0));
    zhadum.update(1.0);
    assert_eq!(zhadum.fade, 0.0);
    assert_eq!(zhadum.glow(), 0.0);
}

#[test]
fn its_beam_goes_two_thousand_toward_its_target() {
    assert_eq!(Zhadum::beam([3.0, -4.0]), [1200.0, -1600.0]);
    assert_eq!(Zhadum::beam([0.0, 0.0]), [0.0, 0.0]);
}
