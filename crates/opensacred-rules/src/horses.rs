//! The horses (docs/re/horses.md, #1822): the horse data's rows, the riding test, the place by
//! a horse that its rider mounts from and gets off at, and the riders' clips.

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
