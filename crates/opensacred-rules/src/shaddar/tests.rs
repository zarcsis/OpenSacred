use super::*;
use crate::spawning::{Rng, Script};

#[test]
fn shaddar_takes_harm_only_from_a_marked_attacker() {
    assert!(wards(SHADDAR, false));
    assert!(!wards(SHADDAR, true));
    assert!(!wards(697, false));
    assert_eq!(mark_turn(0), 0.0);
    assert_eq!(mark_turn(512), 1.0);
    assert_eq!(mark_turn(0x1_0000 + 1024), 2.0);
}

#[test]
fn a_slave_starts_on_a_rolled_heading_and_turn() {
    // r₁ 0, r₂ the highest, r₃ a half: (1, −1) normalised, a turn of 0.3.
    let slave = Slave::new(&mut Script::new(vec![0, 32_767, 16_384]));
    let half = std::f32::consts::FRAC_1_SQRT_2;
    assert!((slave.heading[0] - half).abs() < 1e-5 && (slave.heading[1] + half).abs() < 1e-5);
    assert!((slave.turn - (0.1 + 0.4 * 16_384.0 / 32_767.0)).abs() < 1e-6);
    // Rolls next to a half still give a whole heading, the x of r₂ and the y of r₁.
    let near = Slave::new(&mut Script::new(vec![16_383, 16_384, 0]));
    let length = near.heading[0].hypot(near.heading[1]);
    assert!((length - 1.0).abs() < 1e-3 && near.heading[0] > 0.0 && near.heading[1] < 0.0);
    assert!(!slave.red && !slave.hunting && !slave.fading && slave.cooldown == 0.0);
}

#[test]
fn a_slave_wanders_turning_and_bounces_off_what_it_may_not_cross() {
    let mut slave = Slave::new(&mut Script::new(vec![16_384, 32_767, 0]));
    slave.heading = [1.0, 0.0];
    slave.turn = 0.5;
    slave.wander(1.0);
    assert!((slave.heading[0] - 0.5_f32.cos()).abs() < 1e-6);
    assert!((slave.heading[1] - 0.5_f32.sin()).abs() < 1e-6);
    // Its y turns as well, by the turn times the time.
    slave.heading = [0.0, 1.0];
    slave.wander(0.5);
    assert!((slave.heading[0] + 0.25_f32.sin()).abs() < 1e-6);
    assert!((slave.heading[1] - 0.25_f32.cos()).abs() < 1e-6);
    slave.heading = [0.6, 0.8];
    slave.wander(0.0);
    assert_eq!(slave.heading, [0.6, 0.8]);
    slave.bounce(&mut Script::new(vec![32_767]));
    assert_eq!(slave.heading, [-0.6, -0.8]);
    assert!((slave.turn - 0.2).abs() < 1e-6);
    slave.bounce(&mut Script::new(vec![16_384]));
    assert!((slave.turn - 0.2 * 16_384.0 / 32_767.0).abs() < 1e-6);
    // Its speed: 85 wandering, 255 hunting.
    assert_eq!(slave.speed(85.0), 85.0);
    slave.hunting = true;
    assert_eq!(slave.speed(85.0), 255.0);
    assert_eq!(slave.speed(85.5), 256.0);
}

#[test]
fn a_hunter_turns_toward_its_target_three_radians_a_second_at_most() {
    let mut slave = Slave::new(&mut Script::default());
    slave.heading = [1.0, 0.0];
    slave.steer([0.0, 5.0], 0.1);
    assert!((slave.heading[1].atan2(slave.heading[0]) - 0.3).abs() < 1e-5);
    // Within reach: straight at it; the shorter way round.
    slave.heading = [1.0, 0.0];
    slave.steer([1.0, 0.1], 1.0);
    assert!((slave.heading[1].atan2(slave.heading[0]) - 0.1_f32.atan2(1.0)).abs() < 1e-5);
    slave.heading = [1.0, 0.0];
    slave.steer([0.0, -2.0], 0.1);
    assert!((slave.heading[1].atan2(slave.heading[0]) + 0.3).abs() < 1e-5);
    slave.heading = [-1.0, 0.01];
    slave.steer([-1.0, -0.01], 1.0);
    assert!(slave.heading[0] < -0.99 && slave.heading[1] < 0.0);
    // A target straight behind: it turns the positive way.
    slave.heading = [1.0, 0.0];
    slave.steer([-1.0, 0.0], 0.1);
    assert!((slave.heading[1].atan2(slave.heading[0]) - 0.3).abs() < 1e-5);
}

#[test]
fn a_wanderer_marks_heroes_and_hunts_at_shaddars_touch() {
    let mut slave = Slave::new(&mut Script::default());
    let deeds = slave.touch(&[Touched::Other, Touched::Hero], false);
    assert_eq!(deeds.marks, vec![1]);
    assert!(deeds.strikes.is_empty() && !deeds.hunts_hero);
    assert_eq!(slave.flash, 0.5);
    // At Shaddar, red and hunting the hero; a frame's last signal stands.
    let deeds = slave.touch(&[Touched::Hero, Touched::Shaddar], false);
    assert_eq!(deeds.marks, vec![0]);
    assert!(deeds.hunts_hero && slave.red && slave.hunting);
    let mut flashed = Slave::new(&mut Script::default());
    let deeds = flashed.touch(&[Touched::Shaddar, Touched::Hero], false);
    assert!(!deeds.hunts_hero && !flashed.hunting && flashed.flash == 0.5);
    // Its cooldown keeps it from marking.
    let mut cooled = Slave::new(&mut Script::default());
    cooled.cooldown = 0.5;
    assert_eq!(cooled.touch(&[Touched::Hero], false), Deeds::default());
}

#[test]
fn a_hunter_strikes_all_but_shaddar_and_turns_back() {
    let mut slave = Slave::new(&mut Script::default());
    slave.red = true;
    slave.hunting = true;
    let deeds = slave.touch(&[Touched::Shaddar, Touched::Hero, Touched::Other], true);
    assert_eq!(deeds.strikes, vec![1, 2]);
    assert!(deeds.marks.is_empty() && !deeds.hunts_hero);
    assert!(!slave.red && !slave.hunting);
    assert_eq!(slave.cooldown, 1.0);
    // Shaddar alone: nothing.
    slave.red = true;
    slave.hunting = true;
    assert_eq!(slave.touch(&[Touched::Shaddar], true), Deeds::default());
    assert!(slave.hunting);
    // A hunter whose flash set its rays orange turns red and hunts again at its strike.
    slave.red = false;
    let deeds = slave.touch(&[Touched::Hero], true);
    assert_eq!(deeds.strikes, vec![0]);
    assert!(slave.red && slave.hunting && !deeds.hunts_hero);
}

#[test]
fn a_slaves_rays_flash_redden_and_fade() {
    let mut dice = Rng::of(1, 2, 921);
    let mut slave = Slave::new(&mut dice);
    for _ in 0..40 {
        assert!(slave.update(0.05, &mut dice));
    }
    // Some 190 rays at the detail 2, 40 over its place.
    let count = slave.system.particles.len();
    assert!((170..=200).contains(&count), "{count}");
    assert!(
        slave
            .system
            .particles
            .iter()
            .all(|p| (p.place[2] - 40.0).abs() <= 1.0)
    );
    assert_eq!(Slave::block(0.0).size.value, 30.0);
    assert_eq!(Slave::block(0.5).size.value, 60.0);
    assert_eq!(Slave::block(0.0).size.spread, 7.0);
    assert_eq!(Slave::block(0.0).angle.value, std::f32::consts::PI);
    assert_eq!(Slave::block(0.0).angle.spread, std::f32::consts::PI);
    assert_eq!(Slave::block(0.0).offset.value, [0.0, 0.0, 40.0]);
    assert_eq!(Slave::block(0.0).interval, 0.0066);
    assert_eq!(Slave::layers()[0].life, -200.0);
    assert_eq!(Slave::layers()[0].size, -25.0);
    // A red slave with no flash stays red.
    slave.red = true;
    slave.update(0.05, &mut dice);
    assert!(slave.red && slave.flash == 0.0);
    // Its flash ends its red, whatever it does.
    slave.flash = 0.5;
    slave.red = true;
    slave.hunting = true;
    slave.cooldown = 0.3;
    slave.update(0.3, &mut dice);
    assert!(slave.red && (slave.flash - 0.2).abs() < 1e-6 && slave.cooldown == 0.0);
    slave.update(0.3, &mut dice);
    assert!(!slave.red && slave.hunting && slave.flash == 0.0);
    // Faded, it emits no more and ends as its rays die.
    slave.fade();
    assert!(!slave.hunting);
    let mut alive = true;
    for _ in 0..40 {
        alive = slave.update(0.05, &mut dice);
    }
    assert!(!alive && slave.system.particles.is_empty());
    let [orange, red] = [Slave::ramp(false), Slave::ramp(true)];
    assert_eq!(
        orange.at(255.0)[0..3],
        [254.0 / 255.0, 96.0 / 255.0, 1.0 / 255.0]
    );
    assert_eq!(
        red.at(255.0)[0..3],
        [254.0 / 255.0, 1.0 / 255.0, 1.0 / 255.0]
    );
    assert_eq!(
        orange.at(179.0),
        [253.0 / 255.0, 131.0 / 255.0, 76.0 / 255.0, 254.0 / 255.0]
    );
    assert_eq!(
        red.at(113.0),
        [167.0 / 255.0, 67.0 / 255.0, 67.0 / 255.0, 254.0 / 255.0]
    );
}

#[test]
fn the_balance_gives_the_slaves_their_strike_and_speed() {
    let rules = crate::fight::Rules::default();
    assert_eq!(rules.slave_damages(), [100.0; 4]);
    assert_eq!(rules.slave_speed(), 85.0);
}

#[test]
fn shaddars_held_clip_leaps_back_to_its_half_outside_its_stretch() {
    // At its first tick near its start, and past 0.65 of its span.
    assert!(held_clip_leaps(0.99) && held_clip_leaps(0.851) && held_clip_leaps(0.34));
    assert!(held_clip_leaps(0.0));
    // Within its stretch it plays on.
    assert!(!held_clip_leaps(0.85) && !held_clip_leaps(0.5) && !held_clip_leaps(0.35));
}
