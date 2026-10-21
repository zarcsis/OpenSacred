//! The archers' arrows (docs/re/fight.md, "The archers"): a shot's arrow (`cArrow3D`, type 3070)
//! flies straight from its shooter's left hand toward its target's point at 600 a second for
//! 1.5 s (`0x5BB010`), ends on a cell that may not be walked on, but the ground's nibble 2, or at
//! the ground (`0x5BAA60`), and in each frame hits its target, or the first creature, nearer the
//! part of its way it flew than the sum of the two types' sizes (`0x5FFC70`, `0x6226B0`), when
//! that creature may be hit (`0x548F60`); after a hit it sticks in a creature of a size above 20
//! for 5 s, but not in the local player's hero (`0x5BAB40`).
//! `opensacred-game` (`arrows`) makes the arrows, draws them, finds the creatures near them and
//! sends the hits; this module is the rules alone, over points of the ground ([`Spot`]) and
//! heights.

use crate::minds::Spot;

/// How fast an arrow flies, in world units a second (`+0x200`), and how long, in seconds
/// (`+0x234`): at most 900 from the hand.
pub const SPEED: f32 = 600.0;
pub const LIFE: f32 = 1.5;
/// The size of a type whose `items.pak +0x14` is 0, as the arrows' and the heroes' are
/// (`0x428CE0`).
const SIZE: f32 = 50.0;
/// How long an arrow stays stuck in a creature after its hit, in seconds, and the size above
/// which a creature takes it (`0x5BAB40`).
pub const STICKS_FOR: f32 = 5.0;
const STICKS_ABOVE: f32 = 20.0;

/// A type's size for the arrows' hits: its `items.pak +0x14`, 50 for 0 (`0x428CE0`).
pub fn size(word: u32) -> f32 {
    match u16::try_from(word) {
        Ok(0) | Err(_) => SIZE,
        Ok(size) => f32::from(size),
    }
}

/// A point of the world: its point of the ground and its height, in world units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub ground: Spot,
    pub height: f32,
}

impl Point {
    pub const fn new(ground: Spot, height: f32) -> Self {
        Self { ground, height }
    }

    /// The point `seconds` on along `velocity`, a velocity a second.
    fn on(self, velocity: Self, seconds: f32) -> Self {
        Self::new(
            self.ground + velocity.ground * seconds,
            self.height + velocity.height * seconds,
        )
    }
}

/// An arrow in flight (`cArrow3D`, `0x5BB010`): where it is, its velocity a second, fixed at its
/// start in three dimensions without gravity, and how long it still flies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arrow {
    at: Point,
    velocity: Point,
    left: f32,
}

impl Arrow {
    /// An arrow from `from` toward `toward`, straight at 600 a second for 1.5 s (`0x4EB700`); it
    /// leads no moving target. One aimed at its own point stands where it is.
    pub fn new(from: Point, toward: Point) -> Self {
        let (along, up) = (toward.ground - from.ground, toward.height - from.height);
        let length = (along.length().powi(2) + up * up).sqrt();
        let velocity = if length >= 1e-7 {
            Point::new(along * (SPEED / length), up * (SPEED / length))
        } else {
            Point::default()
        };
        Self {
            at: from,
            velocity,
            left: LIFE,
        }
    }

    /// Where it is.
    pub fn at(&self) -> Point {
        self.at
    }

    /// Its velocity a second, the way it points.
    pub fn velocity(&self) -> Point {
        self.velocity
    }

    /// Its flight in a frame of `dt` seconds (`0x5BB010`): none once its time is out, when it is
    /// destroyed; else its time runs down and its next point lies `dt` seconds on, where it goes
    /// when `open` lets it fly there ([`open`], `0x5BAA60`), and else its time ends where it is.
    /// The part of its way it flew, from its last point to its next, on which its hits are
    /// tested ([`picks`]) whether or not it got there.
    pub fn fly(&mut self, dt: f32, open: impl FnOnce(Point) -> bool) -> Option<(Spot, Spot)> {
        if self.left <= 0.0 {
            return None;
        }
        self.left -= dt;
        let next = self.at.on(self.velocity, dt);
        let from = self.at.ground;
        if open(next) {
            self.at = next;
        } else {
            self.left = 0.0;
        }
        Some((from, next.ground))
    }
}

/// Whether an arrow may fly to a point (`0x5BAA60`): its cell may be walked on (`walkable`,
/// `0x636C10`), or is of the ground's nibble 2 (`flown_over`, the ponds, the fences and the
/// walls), which it flies over; and it lies above the ground's height there (`ground`,
/// `0x636520`), which is none outside the world. The statics do not stop it.
pub fn open(walkable: bool, flown_over: bool, height: f32, ground: Option<f32>) -> bool {
    (walkable || flown_over) && ground.is_some_and(|ground| height > ground)
}

/// The distance in the plane of `point` from the part of a way from `from` to `to` (`0x6226B0`).
fn from_way(from: Spot, to: Spot, point: Spot) -> f32 {
    let way = to - from;
    let length = way.x * way.x + way.y * way.y;
    let along = if length > 0.0 {
        let to_point = point - from;
        ((to_point.x * way.x + to_point.y * way.y) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (point - (from + way * along)).length()
}

/// The creature that an arrow of size `size`, flying from `from` to `to` in a frame, comes on
/// (`0x5FFC70`, `0x6226B0`): of the creatures `near`, each with its point of the ground and its
/// size, those whose point lies nearer that part of its way, in the plane, than the sum of the two
/// sizes; of them its `target` when it is one, else the first; the height is not tested. The
/// arrow hits it only when it may be hit, alive and hostile to the shooter (`0x548F60`); any
/// other lets it fly on.
pub fn picks<T: Copy + PartialEq>(
    (from, to): (Spot, Spot),
    size: f32,
    near: impl IntoIterator<Item = (T, Spot, f32)>,
    target: T,
) -> Option<T> {
    let mut first = None;
    for (creature, at, its) in near {
        if from_way(from, to, at) >= size + its {
            continue;
        }
        if creature == target {
            return Some(creature);
        }
        first = first.or(Some(creature));
    }
    first
}

/// Whether a stopped arrow sticks into the creature it hit (`0x5BAB40`): one of a size above 20,
/// but not the local player's hero (`hero`).
pub fn sticks(size: f32, hero: bool) -> bool {
    !hero && size > STICKS_ABOVE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, y: f32) -> Spot {
        Spot::new(x, y)
    }

    #[test]
    fn a_size_of_0_is_50() {
        assert_eq!(size(0), 50.0);
        assert_eq!(size(45), 45.0);
        assert_eq!(size(25), 25.0);
    }

    #[test]
    fn an_arrow_flies_straight_at_600_a_second_for_1_5_s() {
        // From a hand 90 over the ground toward a neck 130 over it, 600 away along x.
        let from = Point::new(at(0.0, 0.0), 90.0);
        let toward = Point::new(at(600.0, 0.0), 130.0);
        let mut arrow = Arrow::new(from, toward);
        let speed = arrow.velocity();
        let length = (speed.ground.length().powi(2) + speed.height.powi(2)).sqrt();
        assert!((length - SPEED).abs() < 1e-3, "{length}");
        // A second takes it on by 600 along its way, rising as it goes.
        let mut flown = 0.0;
        let mut frames = 0;
        while let Some((last, next)) = arrow.fly(0.125, |_| true) {
            flown += (next - last).length();
            frames += 1;
            if frames == 8 {
                let p = arrow.at();
                assert!(
                    (p.ground.x - 598.7).abs() < 0.5 && p.height > 125.0,
                    "{p:?}"
                );
            }
        }
        // It flies 1.5 s, 12 frames of 0.125 s, about 900 along the ground, and then is gone.
        assert_eq!(frames, 12);
        assert!((flown - 898.0).abs() < 2.0, "{flown}");
        assert_eq!(arrow.fly(0.1, |_| true), None);
    }

    #[test]
    fn an_arrow_ends_where_it_may_not_fly_on() {
        let mut arrow = Arrow::new(
            Point::new(at(0.0, 0.0), 90.0),
            Point::new(at(600.0, 0.0), 0.0),
        );
        // The frame it meets a wall it stays where it was, its way tested for hits still, and
        // it is gone the next.
        let way = arrow.fly(0.1, |_| false);
        assert_eq!(arrow.at().ground, at(0.0, 0.0));
        assert!(way.is_some_and(|(last, next)| last == at(0.0, 0.0) && next.x > 50.0));
        assert_eq!(arrow.fly(0.1, |_| true), None);
        // A wall's cell, a pond's, a fence's of nibble 2 it flies over; under the ground or off
        // the world it ends.
        assert!(open(true, false, 50.0, Some(10.0)));
        assert!(open(false, true, 50.0, Some(10.0)));
        assert!(!open(false, false, 50.0, Some(10.0)));
        assert!(!open(true, false, 10.0, Some(10.0)));
        assert!(!open(true, false, 50.0, None));
    }

    #[test]
    fn an_arrow_hits_its_target_or_the_first_creature_near_its_way() {
        let way = (at(0.0, 0.0), at(100.0, 0.0));
        // The hero 90 off the way: within the arrow's 50 and its own 50.
        let hero = ("hero", at(50.0, 90.0), 50.0);
        // A wolf of 45 that 96 off misses; a goblin of 25, 41 from the way's end, is near.
        let wolf = ("wolf", at(50.0, -96.0), 45.0);
        let goblin = ("goblin", at(140.0, 10.0), 25.0);
        assert_eq!(picks(way, 50.0, [wolf, hero, goblin], "hero"), Some("hero"));
        // Without its target near, the first that is, the farther ones not.
        assert_eq!(picks(way, 50.0, [wolf, goblin], "hero"), Some("goblin"));
        assert_eq!(picks(way, 50.0, [wolf], "hero"), None);
        let far = ("hero", at(50.0, 101.0), 50.0);
        assert_eq!(picks(way, 50.0, [far], "hero"), None);
        // A way of no length measures from its point.
        let still = (at(0.0, 0.0), at(0.0, 0.0));
        assert_eq!(
            picks(still, 50.0, [("hero", at(0.0, 99.0), 50.0)], "hero"),
            Some("hero")
        );
    }

    #[test]
    fn an_arrow_sticks_in_a_creature_but_not_in_the_hero() {
        assert!(sticks(45.0, false));
        assert!(!sticks(20.0, false));
        assert!(!sticks(50.0, true));
    }
}
