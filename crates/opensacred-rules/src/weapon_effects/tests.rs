use super::*;
use crate::spawning::Rng;

#[test]
fn a_weapons_first_styled_bonus_gives_its_glows_colour() {
    // An empty slot, a slot of no style, then the effect 4 (blue) and the effect 5 (red).
    let slots = [(0, 0), (0, 6), (0x0102, 4 | 0x0003_0000), (0, 5)];
    assert_eq!(bonus_style(slots), Some(4));
    // A slot of a condition alone, of the effect 0, gives none.
    assert_eq!(bonus_style([(7, 0), (0, 25)]), Some(2));
    assert_eq!(bonus_style([(0, 0), (0, 2)]), Some(2));
    assert_eq!(bonus_style([(0, 29)]), Some(3));
    assert_eq!(bonus_style([(0, 42)]), Some(5));
    assert_eq!(bonus_style([(0, 44), (0, 52), (0, 200)]), None);
    assert_eq!(bonus_style(std::iter::empty()), None);
}

#[test]
fn every_effect_takes_its_style_from_the_table_at_the_effect_less_one() {
    // `0x5CEBA8`'s bytes for the effects 1 to 51 (#3888), 0 for none.
    const TABLE: [u8; 51] = [
        3, 2, 5, 4, 1, 0, 5, 5, 5, 5, // 1 to 10
        1, 0, 5, 0, 5, 2, 5, 5, 5, 5, // 11 to 20
        5, 5, 1, 1, 2, 2, 4, 3, 3, 5, // 21 to 30
        5, 5, 3, 4, 4, 5, 5, 5, 5, 5, // 31 to 40
        0, 5, 5, 0, 0, 0, 0, 0, 0, 0, // 41 to 50
        5, // 51
    ];
    assert_eq!(style_of(0), None);
    for (effect, &style) in (1..).zip(&TABLE) {
        assert_eq!(style_of(effect), (style != 0).then_some(style), "{effect}");
    }
    for effect in [52, 53, 0xFFFF] {
        assert_eq!(style_of(effect), None, "{effect}");
    }
}

#[test]
fn a_weapons_glow_sheds_sparks_of_its_style_and_pulses() {
    let mut dice = Rng::of(3, 4, 776);
    let mut glow = WeaponGlow::new(1);
    for _ in 0..10 {
        glow.update(0.01, &mut dice);
    }
    // Some 20 born at its point at the detail 2 (one each 0.005 s), a quarter second's life.
    let count = glow.system.particles.len();
    assert!((15..=25).contains(&count), "{count}");
    assert!(glow.system.particles.iter().all(|p| p.place == [0.0; 3]));
    assert_eq!(WeaponGlow::layers()[0].life, -1000.0);
    assert_eq!(WeaponGlow::layers()[0].size, -2.7);
    assert_eq!(WeaponGlow::block().size.value, 3.0);
    assert_eq!(WeaponGlow::block().interval, 0.005);
    assert!((glow.flare() - (17.0 + 1.5 * (0.2_f32.sin() + 1.0))).abs() < 1e-4);
    // White at its birth, then its style's colour, darker, then nothing.
    let red = WeaponGlow::ramp(1);
    assert_eq!(red.at(255.0), [1.0; 4]);
    assert_eq!(red.at(247.0), [0.5, 0.2, 0.2, 1.0]);
    assert_eq!(red.at(55.0), [0.3, 0.1, 0.1, 0.8]);
    assert!(red.at(0.0)[3] < 0.02);
    for (style, bright) in [
        (2, [0.2, 0.5, 0.2]),
        (3, [0.3, 0.3, 0.3]),
        (4, [0.2, 0.2, 0.5]),
        (5, [0.4, 0.2, 0.5]),
    ] {
        assert_eq!(WeaponGlow::ramp(style).at(247.0)[..3], bright);
    }
    assert_eq!(WeaponGlow::ramp(4).at(55.0), [0.1, 0.1, 0.3, 0.8]);
    assert_eq!(WeaponGlow::ramp(5).at(55.0), [0.3, 0.2, 0.3, 0.8]);
}

#[test]
fn the_whip_hangs_and_trails_after_its_place() {
    let mut whip = MagicWhip::new();
    assert_eq!(whip.points.len(), 50);
    assert_eq!(whip.points[3].0, [0.0, -4.5, 0.0]);
    // Moved 10 along x: its first point follows at once, the others stay 1.5 apart.
    whip.update([10.0, 0.0, 0.0]);
    assert_eq!(whip.points[0].0, [10.0, 0.0, 0.0]);
    for k in 1..50 {
        let [a, b] = [whip.points[k - 1].0, whip.points[k].0];
        let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        assert!((d - 1.5).abs() < 1e-4, "{k}: {d}");
    }
    // The second point leant toward the move and keeps 0.675 of its move as its velocity.
    let (place, velocity) = whip.points[1];
    assert!(place[0] > 0.0 && place[1] < 0.0);
    assert!((velocity[0] - place[0] * 0.675).abs() < 1e-5);
    assert!((velocity[1] - (place[1] + 1.5) * 0.675).abs() < 1e-5);
    // Its velocity carries on: the next link leans by its offset plus its velocity.
    let (place, velocity) = whip.points[1];
    whip.update([10.0, 0.0, 0.0]);
    let way: [f32; 3] = std::array::from_fn(|i| place[i] - [10.0, 0.0, 0.0][i] + velocity[i]);
    let length = way.iter().map(|v| v * v).sum::<f32>().sqrt();
    let want: [f32; 3] = std::array::from_fn(|i| [10.0, 0.0, 0.0][i] + 1.5 * way[i] / length);
    assert!((0..3).all(|i| (whip.points[1].0[i] - want[i]).abs() < 1e-5));
    // Points on one another stay so, with no velocity.
    let mut heap = MagicWhip::new();
    heap.points
        .iter_mut()
        .for_each(|point| *point = ([1.0; 3], [0.0; 3]));
    heap.update([1.0; 3]);
    assert!(
        heap.points
            .iter()
            .all(|&point| point == ([1.0; 3], [0.0; 3]))
    );
    assert_eq!(MagicWhip::HALF, 4.0);
    assert_eq!(
        MagicWhip::CORNERS,
        [0xFF00_0080, 0xFF40_4080, 0xFF00_4080, 0xFF40_0080]
    );
    // Hung from a place, as from 0.
    let mut hung = MagicWhip::new();
    hung.hang_from([3.0, 4.0, 5.0]);
    assert_eq!(hung.points[2].0, [3.0, 1.0, 5.0]);
    assert_eq!(hung.points[2].1, [0.0; 3]);
}

#[test]
fn a_streak_steps_two_hundred_a_second_drawn_out_along_its_axis() {
    let mut streak = MagicStreak::new();
    // Its first update starts where it is; a held axis draws it out along x.
    for _ in 0..20 {
        streak.update(0.05, [5.0, 5.0, 0.0], [1.0, 0.0, 0.0]);
    }
    let first = streak.points[0].0;
    assert!((first[0] - 5.0).abs() < 0.1 && (first[1] - 5.0).abs() < 0.1);
    let last = streak.points[49].0;
    assert!(last[0] > first[0] + 20.0, "{last:?}");
    for k in 1..50 {
        let [a, b] = [streak.points[k - 1].0, streak.points[k].0];
        let d = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        assert!((d - 0.75).abs() < 1e-4, "{k}: {d}");
    }
    // A step's share of the way: a time of 0.005 s is one step from where it stood.
    let mut one = MagicStreak::new();
    one.points[0].0 = [2.0, 2.0, 0.0];
    one.update(0.005, [4.0, 2.0, 0.0], [0.0; 3]);
    assert_eq!(one.points[0].0, [2.0, 2.0, 0.0]);
    one.update(0.01, [4.0, 2.0, 0.0], [0.0; 3]);
    assert_eq!(one.points[0].0, [3.0, 2.0, 0.0]);
    // Its time held at half a second: a hundred steps.
    let mut long = MagicStreak::new();
    long.points[0].0 = [1.0, 1.0, 0.0];
    long.update(2.0, [101.0, 1.0, 0.0], [0.0; 3]);
    assert!((long.points[0].0[0] - 100.0).abs() < 1e-3);
    // A new streak starts at its place, whatever its points held.
    let mut fresh = MagicStreak::new();
    fresh.update(0.01, [4.0, 2.0, 0.0], [0.0; 3]);
    assert_eq!(fresh.points[0].0, [4.0, 2.0, 0.0]);
    // A first point off the axes' zero moves on from where it stood.
    let mut off = MagicStreak::new();
    off.points[0].0 = [0.0, 2.0, 0.0];
    off.update(0.01, [4.0, 2.0, 0.0], [0.0; 3]);
    assert_eq!(off.points[0].0, [2.0, 2.0, 0.0]);
    // The pull along the axis by the weights 0.7 and 0.686, a link of 0.75 each.
    let mut pulled = MagicStreak::new();
    pulled.points[0].0 = [0.0, 0.5, 0.0];
    pulled.points[1].0 = [0.0, 1.25, 0.0];
    pulled.points[2].0 = [0.0, 2.0, 0.0];
    pulled.update(0.005, [9.0, 9.0, 0.0], [1.0, 0.0, 0.0]);
    let link = |from: [f32; 3], to: [f32; 3], weight: f32| {
        let way = [to[0] - from[0] + weight, to[1] - from[1], to[2] - from[2]];
        let length = (way[0] * way[0] + way[1] * way[1] + way[2] * way[2]).sqrt();
        [0, 1, 2].map(|i| from[i] + 0.75 * way[i] / length)
    };
    let first = link([0.0, 0.5, 0.0], [0.0, 1.25, 0.0], 0.7);
    let second = link(first, [0.0, 2.0, 0.0], 0.686);
    for (got, want) in [(pulled.points[1].0, first), (pulled.points[2].0, second)] {
        assert!(
            (0..3).all(|i| (got[i] - want[i]).abs() < 1e-5),
            "{got:?} {want:?}"
        );
    }
    assert_eq!((MagicStreak::HALF, MagicStreak::HAZE), (1.5, 10.0));
    assert_eq!(
        MagicStreak::CORNERS,
        [0xFF00_00C0, 0xFF80_80C0, 0xFF00_80C0, 0xFF80_00C0]
    );
    assert_eq!(
        MagicStreak::HAZE_CORNERS,
        [0x4000_0040, 0x4020_2040, 0x4000_2040, 0x4020_0040]
    );
    // Nothing moves in no time.
    let mut still = MagicStreak::new();
    still.update(0.004, [3.0, 3.0, 0.0], [1.0, 0.0, 0.0]);
    assert_eq!(still, MagicStreak::new());
}

#[test]
fn the_swords_sparks_live_half_a_second_within_forty() {
    let mut dice = Rng::of(1, 7, 858);
    let mut worms = MagicWorms::default();
    for _ in 0..10 {
        worms.update(0.05, &mut dice);
    }
    let count = worms.system.particles.len();
    assert!((20..=30).contains(&count), "{count}");
    assert!(
        worms
            .system
            .particles
            .iter()
            .all(|p| p.place.iter().all(|v| v.abs() <= 40.0))
    );
    assert_eq!(MagicWorms::layers()[0].life, -500.0);
    assert_eq!(MagicWorms::block().size.value, 1.0);
    assert_eq!(MagicWorms::block().offset.spread, [40.0; 3]);
    assert_eq!(MagicWorms::block().interval, 0.02);
    assert_eq!(WORMS_COLOUR, [1.0, 68.0 / 255.0, 68.0 / 255.0]);
}
