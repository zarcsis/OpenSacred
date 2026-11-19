//! What lies under the cursor (#2873; docs/re/cursor.md, "The pick"): the rectangle that each
//! object drawn leaves on the screen, a living creature's of its type's size about its feet
//! (`0x44CF20`, `0x428850`), the classes that the rectangles under the cursor fall into, and the
//! one that the pick takes (`0x626C50`): of the first class that holds any, in the order of the
//! hostile creatures, the items and the rest, or the items first while Alt is held, the one whose
//! middle lies nearest the cursor, and none where that one's middle lies 100 pixels or more away.

/// A rectangle on the original's screen of 1,024 × 768 pixels, its least corner and its size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub at: [f32; 2],
    pub size: [f32; 2],
}

impl Rect {
    /// The rectangle from the corner `low` to the corner `high`.
    pub fn between(low: [f32; 2], high: [f32; 2]) -> Self {
        Self {
            at: low,
            size: [high[0] - low[0], high[1] - low[1]],
        }
    }

    /// Whether `point` lies in it, its edges in (`0x626CDB`).
    pub fn contains(&self, point: [f32; 2]) -> bool {
        (0..2).all(|axis| {
            self.at[axis] <= point[axis] && point[axis] <= self.at[axis] + self.size[axis]
        })
    }

    /// Its middle, by which the pick weighs it (`0x626D3B`).
    pub fn middle(&self) -> [f32; 2] {
        [
            self.at[0] + self.size[0] / 2.0,
            self.at[1] + self.size[1] / 2.0,
        ]
    }

    /// Each side held at least at [`LEAST_SIDE`] about its middle (`0x44D3BB`).
    pub fn held(self) -> Self {
        let mut held = self;
        for axis in 0..2 {
            if held.size[axis] < LEAST_SIDE {
                held.at[axis] += held.size[axis] / 2.0 - (LEAST_SIDE / 2.0).floor();
                held.size[axis] = LEAST_SIDE;
            }
        }
        held
    }

    /// Halved about its middle, as a ridden horse's is (`0x44D3FF`).
    pub fn halved(self) -> Self {
        let size = [self.size[0] / 2.0, self.size[1] / 2.0];
        Self {
            at: [self.at[0] + size[0] / 2.0, self.at[1] + size[1] / 2.0],
            size,
        }
    }
}

/// The least side of an object's rectangle on the screen, in pixels, at any zoom (`0x44D3BB`).
pub const LEAST_SIDE: f32 = 35.0;

/// The screen's pixels at the zoom 1 that a unit of the original's frame takes across and upright
/// along the ground, as its device's matrices draw the models (`0x623B60`, read in the running
/// original on 2026-11-19): the projection's 534 units across the 1,024 pixels, and its 400 units
/// up the 768 times the view's 0.447 of the ground's depth upright, the camera's tilt.
pub const ACROSS: f32 = 1024.0 / 534.0;
pub const UPRIGHT: f32 = 768.0 / 400.0 * 0.447_213_6;

/// The half width, the height and the lift of the box of a living creature of the type `kind`
/// (`0x428850`): the sizes that the code gives some types, else 20 and 170 and no lift.
pub fn creature_size(kind: u32) -> (u16, u16, u16) {
    match kind {
        35 | 56..=59 => (60, 110, 0),
        37 | 41..=43 | 67 => (25, 180, 0),
        39 | 40 | 182 | 185 | 187 | 189 | 250..=252 => (30, 195, 0),
        45 | 60..=62 | 68 => (25, 190, 0),
        49 | 51 | 137 => (30, 230, 0),
        52 | 74 | 88 | 100..=103 | 247 | 354 => (100, 280, 0),
        336 => (100, 280, 60),
        53 | 94 | 183 => (30, 430, 0),
        54 | 167 => (40, 230, 80),
        55 => (75, 280, 0),
        69 | 191 | 195 => (30, 70, 0),
        71 | 513..=516 | 560..=563 | 565 | 566 | 589 | 590 | 594..=596 => (20, 20, 0),
        72 | 261 | 523..=528 | 530..=533 | 536..=538 | 540 | 541 | 550..=558 => (60, 160, 0),
        75 => (40, 40, 0),
        190 => (60, 70, 40),
        335 => (50, 80, 60),
        193 | 512 | 520..=522 | 543..=545 | 567..=570 => (50, 80, 0),
        1249 => (15, 50, 0),
        _ => (20, 170, 0),
    }
}

/// The rectangle of a living creature of the type `kind` whose feet stand at `feet` on the
/// screen, at the view's zoom `zoom` (2 shows twice the world), a ridden horse's where `ridden`
/// (`0x44CF20`): its box `x ± w` across and from `y + h/6 − c` to `y − h − c` along the ground of
/// its type's size ([`creature_size`]), its two corners on the screen, each side held at least at
/// 35 about its middle; a ridden horse's then halved about its middle.
pub fn creature_rect(feet: [f32; 2], kind: u32, zoom: f32, ridden: bool) -> Rect {
    let (half, height, lift) = creature_size(kind);
    let (half, height, lift) = (f32::from(half), f32::from(height), f32::from(lift));
    let across = half * ACROSS / zoom;
    let up = (height + lift) * UPRIGHT / zoom;
    let down = (height / 6.0 - lift) * UPRIGHT / zoom;
    let low = [feet[0] - across, feet[1] - up];
    let high = [feet[0] + across, (feet[1] + down).max(low[1])];
    let rect = Rect::between(low, high).held();
    if ridden { rect.halved() } else { rect }
}

/// The classes of the pick (the object's query 1, `0x626D6E`): a living creature hostile to the
/// local player (`0x55D3F2`), an item, or a scripts' event of a box (`0x5C70E0`, the ids of the
/// bit 31), and the rest: the peaceful creatures, the dead, the containers, the doors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Foe,
    Item,
    Other,
}

/// The classes in the order the pick tries them (`0x95F934`), and while Alt is held
/// (`0x95F944`).
const ORDER: [Class; 3] = [Class::Foe, Class::Item, Class::Other];
const ALT_ORDER: [Class; 3] = [Class::Item, Class::Foe, Class::Other];
/// The most rectangles that the pick weighs; more pick nothing (`0x626C7E`).
pub const MOST: usize = 1000;
/// The squared distance of a middle from the cursor below which alone the pick takes its object
/// (`0x626F80`: 9,999).
const NEAR: f32 = 9999.0;

/// What the pick takes of the objects `drawn`, each its class and its rectangle, for the cursor at
/// `cursor` with Alt held where `alt` (`0x626C50`): nothing for more than [`MOST`]; of the
/// rectangles that hold the cursor, the first class in the order that holds any, and in it the
/// one whose middle lies nearest the cursor, the first of two as near; none where that one's
/// middle lies 100 pixels or more from the cursor, the next class not tried.
pub fn choose<T: Copy>(drawn: &[(T, Class, Rect)], cursor: [f32; 2], alt: bool) -> Option<T> {
    if drawn.len() > MOST {
        return None;
    }
    let order = if alt { ALT_ORDER } else { ORDER };
    let class = order.into_iter().find(|&class| {
        drawn
            .iter()
            .any(|&(_, of, rect)| of == class && rect.contains(cursor))
    })?;
    let squared = |rect: &Rect| {
        let [x, y] = rect.middle();
        (cursor[0] - x).powi(2) + (cursor[1] - y).powi(2)
    };
    drawn
        .iter()
        .filter(|&&(_, of, rect)| of == class && rect.contains(cursor))
        .fold(None, |nearest: Option<(T, f32)>, &(object, _, rect)| {
            let distance = squared(&rect);
            match nearest {
                Some((_, least)) if least <= distance => nearest,
                _ => Some((object, distance)),
            }
        })
        .filter(|&(_, distance)| distance < NEAR)
        .map(|(object, _)| object)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_creatures_box_takes_its_types_size() {
        // The smith and the heroes the default; the dragon, the wolf and the chickens their own.
        assert_eq!(creature_size(688), (20, 170, 0));
        assert_eq!(creature_size(1), (20, 170, 0));
        assert_eq!(creature_size(52), (100, 280, 0));
        assert_eq!(creature_size(512), (50, 80, 0));
        assert_eq!(
            [560, 565, 566, 564].map(creature_size)[..3],
            [(20, 20, 0); 3]
        );
        assert_eq!(creature_size(564), (20, 170, 0));
        // The lifted: 54 and 167 by 80, 190 by 40, 335 and 336 by 60.
        assert_eq!(creature_size(167), (40, 230, 80));
        assert_eq!(creature_size(190), (60, 70, 40));
        assert_eq!(creature_size(335), (50, 80, 60));
        assert_eq!(creature_size(336), (100, 280, 60));
    }

    #[test]
    fn a_creatures_rectangle_stands_on_its_feet_by_its_types_size() {
        // The default type, the Seraphim's at her start: 77 across and 170 high, from 146 above
        // her feet to 24 below (seen: 474, 238 for her feet at 512.5, 384).
        let hero = creature_rect([512.5, 384.0], 1, 1.0, false);
        assert!((hero.size[0] - 76.7).abs() < 0.1, "{hero:?}");
        assert!((hero.size[1] - 170.3).abs() < 0.2, "{hero:?}");
        assert!((hero.at[1] - (384.0 - 146.0)).abs() < 0.2, "{hero:?}");
        // The big demon, 30 and 430: 115 across, 430 high (seen: 115 × 430).
        let demon = creature_rect([130.0, 387.0], 94, 1.0, false);
        assert!((demon.size[0] - 115.1).abs() < 0.1 && (demon.size[1] - 430.8).abs() < 0.2);
        // The lift of 54 raises its box by 80: from 266 above its feet to 36 above them.
        let lifted = creature_rect([657.0, 224.0], 54, 1.0, false);
        assert!((lifted.at[1] - (224.0 - 266.2)).abs() < 0.2, "{lifted:?}");
        assert!((lifted.at[1] + lifted.size[1] - (224.0 - 35.8)).abs() < 0.2);
        // The small spider held at 35 high about its middle; at the zoom 2 all halves but that.
        let spider = creature_rect([800.0, 386.0], 71, 1.0, false);
        assert_eq!(spider.size[1], LEAST_SIDE);
        let far = creature_rect([512.0, 384.0], 1, 2.0, false);
        assert!((far.size[0] - 38.4).abs() < 0.1 && (far.size[1] - 85.2).abs() < 0.1);
        // A ridden horse's halved about its middle.
        let horse = creature_rect([512.0, 384.0], 1, 1.0, true);
        assert_eq!(horse.middle(), hero_at([512.0, 384.0]).middle());
        assert!((horse.size[0] - hero.size[0] / 2.0).abs() < 1e-3);
    }

    fn hero_at(feet: [f32; 2]) -> Rect {
        creature_rect(feet, 1, 1.0, false)
    }

    #[test]
    fn the_pick_takes_the_first_class_and_in_it_the_nearest_middle() {
        let rect = |x: f32, y: f32| Rect::between([x - 30.0, y - 30.0], [x + 30.0, y + 30.0]);
        let drawn = [
            (1, Class::Other, rect(100.0, 140.0)),
            (2, Class::Item, rect(110.0, 100.0)),
            (3, Class::Foe, rect(120.0, 100.0)),
            (4, Class::Foe, rect(95.0, 100.0)),
        ];
        // A foe before an item before the rest, the nearer of two foes.
        assert_eq!(choose(&drawn, [100.0, 100.0], false), Some(4));
        assert_eq!(choose(&drawn, [125.0, 100.0], false), Some(3));
        // Alt held: the item first.
        assert_eq!(choose(&drawn, [100.0, 100.0], true), Some(2));
        // Of the rest alone, outside every other; nothing outside all.
        assert_eq!(choose(&drawn, [100.0, 160.0], false), Some(1));
        assert_eq!(choose(&drawn, [0.0, 0.0], false), None);
        // Two as near: the first.
        let twins = [
            (5, Class::Other, rect(0.0, 0.0)),
            (6, Class::Other, rect(0.0, 0.0)),
        ];
        assert_eq!(choose(&twins, [1.0, 1.0], false), Some(5));
    }

    #[test]
    fn a_middle_100_pixels_away_picks_nothing_and_no_other_class() {
        // A big demon's rectangle under the cursor at its head, 200 above its middle, and a
        // peaceful creature's beside: the foe's class is tried alone, and its middle is too far.
        let demon = creature_rect([500.0, 600.0], 94, 1.0, false);
        let head = [500.0, demon.at[1] + 5.0];
        let other = Rect::between([480.0, head[1] - 10.0], [520.0, head[1] + 10.0]);
        let drawn = [(1, Class::Foe, demon), (2, Class::Other, other)];
        assert_eq!(choose(&drawn, head, false), None);
        // At its middle, it is picked.
        assert_eq!(choose(&drawn, demon.middle(), false), Some(1));
        // 99 pixels from the middle still picks, 100 does not.
        let tall = Rect::between([0.0, 0.0], [10.0, 400.0]);
        let one = [(7, Class::Other, tall)];
        assert_eq!(choose(&one, [5.0, 200.0 - 99.0], false), Some(7));
        assert_eq!(choose(&one, [5.0, 100.0], false), None);
    }

    #[test]
    fn more_than_a_thousand_rectangles_pick_nothing() {
        let rect = Rect::between([0.0, 0.0], [10.0, 10.0]);
        let many: Vec<_> = (0..=MOST).map(|k| (k, Class::Other, rect)).collect();
        assert_eq!(choose(&many, [5.0, 5.0], false), None);
        assert_eq!(choose(&many[..MOST], [5.0, 5.0], false), Some(0));
    }
}
