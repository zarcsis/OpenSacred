//! The horses (docs/re/horses.md, #1822): the horse data's rows, the riding test, the place by
//! a horse that its rider mounts from and gets off at, and the riders' clips.

use opensacred_data::tables::Balance;

/// The `Creature.pak` class of the horses ridden (`0x4266F0`).
pub const CLASS: u8 = 4;
/// The hero type that never rides, the Dwarf (`0x4267D0`).
pub const DWARF: u32 = 8;
/// The motion slots of riding (the names' table from `0x8E9DF0`): the mount, the ride, the
/// dismount, the death on horseback and the idle on horseback.
pub const MOUNT: u16 = 165;
pub const RIDE: u16 = 166;
pub const DISMOUNT: u16 = 167;
pub const DEATH: u16 = 168;
pub const RIDE_IDLE: u16 = 169;
/// How far a click on a horse reaches it, and how near its point a player mounts (the AI's
/// state 1, `0x53A170`'s sub-states 0 and 4).
pub const MOUNTS_FROM: f32 = 1_879.0;
pub const MOUNTS_WITHIN: f32 = 25.0;
/// How near its horse the horse's button of the portraits' column mounts it rather than calls
/// it (`0x6D6C70`), how near the horse called walks to its owner rather than is put beside
/// him, and how far from his place, along each axis at most, it comes (`0x530D68`).
pub const BUTTON_MOUNTS_WITHIN: f32 = 250.0;
pub const CALLED_WALKS_WITHIN: f32 = 1_200.0;
pub const CALLED_ABOUT: u32 = 50;
/// The bone of a horse that its rider's `Bip01` is attached to (`0x401AE0(18)`), and the bone
/// of its place by its side for the riders of the types of [`by_start_bone`] (`0x59A3E0`).
pub const HORSE_BONE: &str = "horse_bone";
pub const START_BONE: &str = "start_bone";
pub const RIDER_BONE: &str = "Bip01";

/// A class's row of the horse data (`Balance.bin`'s tail table 4, `HorseData:`, the map
/// `0xAD57D4`): the horse's pace (the stats' `+0x40`), its trample's most against one creature
/// and its damage, the two parameters of its health bonus's curve, its trample's regeneration
/// and the riding skill needed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub pace: u32,
    pub most: u32,
    pub damage: u32,
    pub bonus: [u32; 2],
    pub regeneration: u32,
    pub riding: u32,
}

impl Row {
    /// The row of the class `class` among `rows`, each its key and its seven words; a class of
    /// none an empty row, which needs no riding (`0x4267D0`).
    pub fn of(rows: &[(u32, [u32; 7])], class: u8) -> Self {
        rows.iter()
            .find(|&&(key, _)| key == u32::from(class))
            .map_or_else(Self::default, |&(_, w)| Self {
                pace: w[0],
                most: w[1],
                damage: w[2],
                bonus: [w[3], w[4]],
                regeneration: w[5],
                riding: w[6],
            })
    }

    /// The factor of a horse's speeds by its pace (`0x4F75F0`: the pace times 0.01).
    pub fn pace_factor(&self) -> f32 {
        crate::fight::to_f32(self.pace) * 0.01
    }
}

/// The balance image's word of the horses' base prices (`0xAD4CC0`, its `+0x830`): three floats
/// a class from 1 to 9, `a`, `b` and `c₂`, which no name of the file's list keys.
const PRICES_AT: usize = 0x830 / 4;
const PRICED_CLASSES: usize = 9;

/// The horses' base prices' floats by the class (`a`, `b` and `c₂`: 1,000 times the class, 100
/// and 2 in the file).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prices([[f32; 3]; PRICED_CLASSES]);

impl Default for Prices {
    /// The file's.
    fn default() -> Self {
        Self(std::array::from_fn(|i| {
            let class = u16::try_from(i + 1).unwrap_or(u16::MAX);
            [1_000.0 * f32::from(class), 100.0, 2.0]
        }))
    }
}

impl Prices {
    /// The floats of the balance `balance`'s image, the file's where it lacks them.
    pub fn of(balance: Option<&Balance>) -> Self {
        let mut prices = Self::default();
        if let Some(words) =
            balance.and_then(|b| b.image.get(PRICES_AT..PRICES_AT + 3 * PRICED_CLASSES))
        {
            for (row, floats) in prices.0.iter_mut().zip(words.as_chunks::<3>().0) {
                for (value, &word) in row.iter_mut().zip(floats) {
                    *value = f32::from_bits(word);
                }
            }
        }
        prices
    }

    /// The base price of a horse of the class `class` and the level `level` (`0x426920`): for a
    /// class 1 to 9 `⌊(c₂·x + b)·x + a⌋` of its floats, `x = max(L − 1, 1)`; for another
    /// `(5·L + 50)·20`.
    pub fn price(&self, class: u8, level: u32) -> u32 {
        let Some(&[a, b, c2]) = usize::from(class)
            .checked_sub(1)
            .and_then(|i| self.0.get(i))
        else {
            return (5 * level + 50) * 20;
        };
        let x = f64::from(level.saturating_sub(1).max(1));
        let price = (f64::from(c2) * x + f64::from(b)) * x + f64::from(a);
        whole(price)
    }
}

/// A rider's trample on its horse (the override, docs/re/horses.md, "Fighting from horseback"):
/// the damage it deals in all (the effect 24's value, `0x552B90`), the most that one creature
/// takes (its factor, `0x552D70`), its seconds (`0x552EF0`) and its regeneration's seconds
/// (`0x552F60`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trample {
    pub damage: f32,
    pub most: f32,
    pub seconds: f32,
    pub regeneration: f32,
}

/// The rider's term of its riding skill `skill` (`0x552790`): 0 below 1, else the curve
/// `1 + 98·(1 − 1/((s − 1)/100 + 1))` (`0x565D70(1, s, 100, 50)`).
fn riding_term(skill: u32) -> f32 {
    if skill == 0 {
        0.0
    } else {
        crate::fight::curve([1.0, 100.0, 50.0], skill)
    }
}

/// The trample of a rider of the level `rider` and the riding skill `skill` on a horse of the row
/// `row` and the level `horse`, of the balance's `HorseOverrideTime` `time` (10): with `c` the
/// rider's term ([`riding_term`]) and `m = 1 + 0.01·c`, the damage
/// `⌊w2·(min(⌊1.25·L_r + 0.5⌋, L_h + s) + 9)·0.1 + 0.5⌋·m^1.5`, the most
/// `⌊w1·(L_h + 9)·0.1 + 0.5⌋·m`, the seconds `m·time` and the regeneration `w5/(1 + 0.015·s)`.
pub fn trample(row: &Row, (horse, rider, skill): (u32, u32, u32), time: f32) -> Trample {
    let m = f64::from(riding_term(skill)) * f64::from(0.01_f32) + 1.0;
    let reach = whole(f64::from(rider) * 1.25 + 0.5).min(horse + skill);
    let tenth = f64::from(0.1_f32);
    let damage = whole(f64::from(row.damage * (reach + 9)) * tenth + 0.5);
    let most = whole(f64::from(row.most * (horse + 9)) * tenth + 0.5);
    let skill = crate::fight::to_f32(skill);
    Trample {
        damage: single(f64::from(damage) * m.powf(1.5)),
        most: single(f64::from(most) * m),
        seconds: single(m * f64::from(time)),
        regeneration: crate::fight::to_f32(row.regeneration) / (skill * 0.015 + 1.0),
    }
}

/// The share in per cent that a horse of the row `row` and the level `horse` raises its rider's
/// most health by, the rider of the level `rider` (`0x5527F0`, `0x5526C0`; the description's
/// `0x55C9F0`): its curve `0x565D70(w3, L_h, 100, w4)` times `min(1, (L_h + 9)/(L_r + 9))`, a half
/// up.
pub fn health_bonus(row: &Row, horse: u32, rider: u32) -> u32 {
    let [w3, w4] = row.bonus.map(crate::fight::to_f32);
    let curve = crate::fight::curve([w3, 100.0, w4], horse);
    let share = (f64::from(horse + 9) / f64::from(rider + 9)).min(1.0);
    whole(f64::from(curve) * share + 0.5)
}

/// `x` truncated, held within a `u32`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a price, a damage or a share, whole and within a u32"
)]
fn whole(x: f64) -> u32 {
    x.clamp(0.0, f64::from(u32::MAX)) as u32
}

/// `x` as the float that the original stores it in.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a value that the original keeps as a float"
)]
fn single(x: f64) -> f32 {
    x as f32
}

/// The class of a horse by its byte `+0x400`: 1 where it is 0 (the creature's query `0xD`,
/// `0x55D260`).
pub fn class(byte: u8) -> u8 {
    byte.max(1)
}

/// Whether a hero of the type `hero` and of the riding skill (22) `riding` may ride a horse of
/// the row `row` (`0x4267D0`): never the Dwarf, else where its skill reaches the row's need.
pub fn rides(hero: u32, riding: u32, row: &Row) -> bool {
    hero != DWARF && row.riding <= riding
}

/// Whether a rider of the type `rider` mounts from, and gets off at, the place of the horse's
/// [`START_BONE`] (`0x59A3E0`): the Seraphim, the Wood Elf, the Vampiress in both her forms, and
/// the types 309, 691, 693 and 697.
pub fn by_start_bone(rider: u32) -> bool {
    matches!(rider, 1 | 5 | 6 | 7 | 0x135 | 0x2B3 | 0x2B5 | 0x2B9)
}

/// The place by a horse of a rider of the type `rider` and the model `model` that does not take
/// the horse's start bone ([`by_start_bone`]), its mount's or, getting `off`, its dismount's
/// (`0x59A3E0`): along the horse's facing and across it, in world units, by the model
/// (its file name, case aside) and the Mage's type 3.
pub fn place_by(rider: u32, model: &str, off: bool) -> [f32; 2] {
    let named = |name: &str| model.eq_ignore_ascii_case(name);
    let (mount, dismount) = if rider == 3 {
        ([5.0, -30.0], [-5.0, 0.0])
    } else if named("gladiator.grn") {
        ([4.0, -30.0], [-8.0, -1.0])
    } else if named("valor.grn") || named("warrior.grn") || named("warrior_brown.grn") {
        ([5.0, -37.0], [-8.0, -5.0])
    } else if named("darkelve.grn") {
        ([5.0, -39.0], [-4.0, 0.0])
    } else if named("elve_sorceress.grn") {
        ([42.0, -34.0], [27.0, 0.0])
    } else {
        ([5.0, -36.0], [-5.0, 0.0])
    };
    if off { dismount } else { mount }
}

/// The point by a horse at `horse` facing `facing` (a unit vector on the ground) that a rider
/// mounts from or gets off at ([`place_by`]), and the heading the rider turns to there, the
/// horse's facing turned by 90° (`0x59A3E0`).
pub fn point_by(
    horse: [f32; 2],
    facing: [f32; 2],
    [along, across]: [f32; 2],
) -> ([f32; 2], [f32; 2]) {
    let [fx, fy] = facing;
    let side = [-fy, fx];
    let at = [
        horse[0] + fx * along + side[0] * across,
        horse[1] + fy * along + side[1] * across,
    ];
    (at, side)
}

/// The rider's clip on horseback (`0x5467A0`): its model's slot 169 where it has it, else 166.
pub fn ride_slot(has: impl Fn(u16) -> bool) -> Option<u16> {
    [RIDE_IDLE, RIDE].into_iter().find(|&slot| has(slot))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_horses_price_grows_with_its_level_by_its_class() {
        let prices = Prices::default();
        // The class 1 at the levels 1 and 2 (x = 1) and 10 (x = 9), as docs/re/horses.md reads.
        assert_eq!(prices.price(1, 1), 1_102);
        assert_eq!(prices.price(1, 2), 1_102);
        assert_eq!(prices.price(1, 10), 2_062);
        assert_eq!(prices.price(9, 10), 9_000 + 9 * (100 + 2 * 9));
        // A class of no row: (5·L + 50)·20.
        assert_eq!(prices.price(12, 10), 2_000);
    }

    #[test]
    fn the_trample_grows_with_the_riders_skill_and_the_horses_level() {
        let row = Row {
            pace: 140,
            most: 5,
            damage: 25,
            bonus: [5, 15],
            regeneration: 55,
            riding: 0,
        };
        // No riding skill: m = 1; the reach min(⌊1.25 + 0.5⌋, 1) = 1.
        let plain = trample(&row, (1, 1, 0), 10.0);
        assert_eq!(plain.damage, 25.0);
        assert_eq!(plain.most, 5.0);
        assert_eq!(plain.seconds, 10.0);
        assert_eq!(plain.regeneration, 55.0);
        // The skill 1: c = 1, m = 1.01.
        let skilled = trample(&row, (1, 1, 1), 10.0);
        assert!((skilled.seconds - 10.1).abs() < 1e-4, "{skilled:?}");
        // A rider of the level 20 on a horse of the level 10 with the skill 5: the reach
        // min(25, 15) = 15, the damage ⌊25·24·0.1 + 0.5⌋ = 60 before the term.
        let far = trample(&row, (10, 20, 5), 10.0);
        let m = f64::from(riding_term(5)) * f64::from(0.01_f32) + 1.0;
        assert!(
            (f64::from(far.damage) - 60.0 * m.powf(1.5)).abs() < 1e-3,
            "{far:?}"
        );
        assert!((far.regeneration - 55.0 / 1.075).abs() < 1e-4);
    }

    #[test]
    fn the_health_bonus_is_its_curve_held_by_the_levels() {
        let row = Row {
            bonus: [5, 15],
            ..Row::default()
        };
        // At the level 1 the curve is its offset, 5; a rider below the horse takes it whole.
        assert_eq!(health_bonus(&row, 1, 1), 5);
        // A rider far above: (1 + 9)/(41 + 9) = 0.2 of it.
        assert_eq!(health_bonus(&row, 1, 41), 1);
        // The curve rises toward its double less the offset: at the level 101, 5 + 20·0.5 = 15.
        assert_eq!(health_bonus(&row, 101, 101), 15);
    }

    #[test]
    fn the_prices_are_read_from_the_balances_image() {
        let mut image = vec![0_u32; PRICES_AT + 27];
        image[PRICES_AT] = 500.0_f32.to_bits();
        image[PRICES_AT + 1] = 10.0_f32.to_bits();
        image[PRICES_AT + 2] = 1.0_f32.to_bits();
        let balance = Balance {
            image,
            ..Balance::default()
        };
        assert_eq!(Prices::of(Some(&balance)).price(1, 10), 500 + 9 * (10 + 9));
        assert_eq!(Prices::of(None), Prices::default());
    }

    #[test]
    fn a_horses_row_tells_its_pace_and_the_riding_it_needs() {
        let rows = [
            (1, [140, 5, 25, 5, 15, 55, 0]),
            (9, [175, 30, 180, 25, 62, 80, 25]),
        ];
        let light = Row::of(&rows, class(0));
        assert_eq!(light.pace, 140);
        assert!((light.pace_factor() - 1.4).abs() < 1e-6);
        let war = Row::of(&rows, 9);
        assert!(rides(1, 25, &war) && !rides(1, 24, &war) && !rides(DWARF, 255, &light));
        assert_eq!(Row::of(&rows, 5), Row::default());
    }

    #[test]
    fn a_rider_mounts_beside_its_horse_by_its_model() {
        assert!(by_start_bone(1) && by_start_bone(6) && !by_start_bone(2) && !by_start_bone(3));
        assert_eq!(place_by(3, "mage.grn", false), [5.0, -30.0]);
        assert_eq!(place_by(2, "GLADIATOR.GRN", true), [-8.0, -1.0]);
        assert_eq!(place_by(9, "daemon.grn", false), [5.0, -36.0]);
        // A horse facing +x: the mount's point 5 on and 36 to its right, the rider facing +y.
        let (at, heading) = point_by([100.0, 100.0], [1.0, 0.0], [5.0, -36.0]);
        assert_eq!(at, [105.0, 64.0]);
        assert_eq!(heading, [0.0, 1.0]);
        assert_eq!(ride_slot(|slot| slot == RIDE), Some(RIDE));
        assert_eq!(ride_slot(|_| true), Some(RIDE_IDLE));
    }
}
