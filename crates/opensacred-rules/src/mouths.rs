//! The ways across a dungeon's mouth that the compass's markers point at (#3419; docs/re/
//! interface.md, "The markers (`sdk_MARKER_RESOLVE`, `0x4A5980`)"): where a marker's target lies
//! in a hidden sector and the hero does not, the way in, where the teleport of a base trigger
//! near the target sets one down by the dungeon's mouth outside (`0x4A5420`), quest 61's a fixed
//! cell while the hero holds two of its elements; where the hero stands in a hidden sector and
//! the target does not, the way out, the base trigger nearest the hero whose teleport leads out
//! of the dungeons; each kept by its sector for the game. The original's way out (`0x4A4E20`)
//! takes the first trigger of the sectors' numbering whose function begins with a teleport,
//! wherever the hero stands and wherever it leads, and returns it only at its first call in each
//! sector, the target's own cell after it: defects repaired here, the way out sought as the way in
//! is and returned at every call (docs/decisions/interface.md, "Compass points at the way out of
//! a dungeon").

use std::collections::HashMap;

use crate::quests::Triggers;

/// A global cell, or a sector of 64 × 64 of them.
pub type Cell = (i32, i32);

/// The cells of a sector's side.
const SIDE: i32 = 64;

/// How far from the cell it starts from a way seeks its sectors, in cells, by the distance of
/// their first cells, and the distance it starts from (`0x4A5420`, `0x4A4E20`).
pub const WITHIN: i64 = 25_000;

/// The quest whose way into a hidden sector is a fixed cell while the hero's main quest's items
/// hold the air's and the fire's elements and not the earth's (`0x5D8490`), the cell
/// (**hypothesis**: the earth element's place), and the three elements.
pub const FIXED_QUEST: u32 = 61;
pub const FIXED_WAY: Cell = (3174, 1145);
const AIR: u32 = 6105;
const EARTH: u32 = 6106;
const FIRE: u32 = 6108;

/// The sector of the global cell `cell`.
pub fn sector_of((x, y): Cell) -> Cell {
    (x.div_euclid(SIDE), y.div_euclid(SIDE))
}

/// The distance of two cells, the root of the squares rounded to the nearest whole (`0x4B2710`).
fn distance(a: Cell, b: Cell) -> i64 {
    let (dx, dy) = (
        i64::from(a.0) - i64::from(b.0),
        i64::from(a.1) - i64::from(b.1),
    );
    let squares = dx * dx + dy * dy;
    let root = squares.isqrt();
    if squares - root * root > root {
        root + 1
    } else {
        root
    }
}

/// The sectors that hold a base trigger within [`WITHIN`] of the cell `cell`, nearest it first by
/// the distance of their first cells, two alike in the sectors' numbering, row by row (the keyx
/// ids, which a way walks with a strict test against the nearest so far).
fn nearest(triggers: &Triggers, cell: Cell) -> Vec<(Cell, &[crate::quests::TriggerBox])> {
    let mut sectors: Vec<(i64, Cell, &[crate::quests::TriggerBox])> = triggers
        .sectors()
        .map(|(sector, boxes)| {
            let first = (sector.0 * SIDE, sector.1 * SIDE);
            (distance(first, cell), sector, boxes)
        })
        .filter(|&(d, _, _)| d < WITHIN)
        .collect();
    sectors.sort_by_key(|&(d, (x, y), _)| (d, y, x));
    sectors
        .into_iter()
        .map(|(_, sector, boxes)| (sector, boxes))
        .collect()
}

/// The way into the hidden sector of the target `target` (`0x4A5420`): of the sectors within
/// [`WITHIN`] of it, the nearest that holds a base trigger whose function begins with a
/// `Teleport`, where the last such of its triggers leads; `leads` answers where a trigger's
/// function's `Teleport` leads.
pub fn way_in(
    triggers: &Triggers,
    target: Cell,
    leads: impl Fn(&str) -> Option<Cell>,
) -> Option<Cell> {
    nearest(triggers, target)
        .into_iter()
        .find_map(|(_, boxes)| boxes.iter().rev().find_map(|b| leads(&b.name)))
}

/// The way out of the dungeons from the hero's cell `hero`, sought as the way in is: of the
/// sectors within [`WITHIN`] of it, the nearest that holds a base trigger whose function begins
/// with a `Teleport` to a place in no dungeon's sector, the last such trigger's own cell, the
/// lower corner of its box's part in that sector (`0x4A4E20`'s answer, the walk it was evidently
/// meant to make); `leads` answers where a trigger's function's `Teleport` leads, `dungeon`
/// whether a sector is a dungeon's.
pub fn way_out(
    triggers: &Triggers,
    hero: Cell,
    leads: impl Fn(&str) -> Option<Cell>,
    dungeon: impl Fn(Cell) -> bool,
) -> Option<Cell> {
    nearest(triggers, hero).into_iter().find_map(|(_, boxes)| {
        boxes
            .iter()
            .rev()
            .find(|b| leads(&b.name).is_some_and(|to| !dungeon(sector_of(to))))
            .map(|b| b.from)
    })
}

/// Quest 61's fixed way into a hidden sector, for the marker of the quest `quest` while the
/// hero's main quest's items `items` hold the air's and the fire's elements and not the earth's
/// (`0x5D8490`): [`FIXED_WAY`], tested at every call after the way kept.
pub fn fixed_way(quest: Option<u32>, items: &[u32]) -> Option<Cell> {
    let holds = |item| items.contains(&item);
    (quest == Some(FIXED_QUEST) && holds(AIR) && holds(FIRE) && !holds(EARTH)).then_some(FIXED_WAY)
}

/// The ways that the markers keep, each by its sector, for the game: the way in by the target's
/// sector (`0x4A5420`'s map `0xAB7848`) and the way out by the hero's (`0x4A4E20`'s
/// `0xAACF68`), a sector of none kept as such. The original's are kept for the run, across new
/// games and loads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ways {
    into: HashMap<Cell, Option<Cell>>,
    out: HashMap<Cell, Option<Cell>>,
}

impl Ways {
    /// Where a marker points from the hero's cell `hero` toward its target's cell `target`
    /// (`0x4A5980`): the way in where the target's sector is hidden and the hero's is not, or
    /// `fixed` in its place where given ([`fixed_way`]), the way out where the hero's is hidden
    /// and the target's is not, the target itself otherwise or where no way is found; `hidden`
    /// answers whether a sector is hidden (`crate::explored::hidden`), `dungeon` whether it is a
    /// dungeon's.
    pub fn toward(
        &mut self,
        triggers: &Triggers,
        (hero, target): (Cell, Cell),
        (hidden, dungeon): (impl Fn(Cell) -> bool, impl Fn(Cell) -> bool),
        leads: impl Fn(&str) -> Option<Cell>,
        fixed: Option<Cell>,
    ) -> Cell {
        let way = match (hidden(sector_of(hero)), hidden(sector_of(target))) {
            (false, true) => self.inward(triggers, target, leads, fixed),
            (true, false) => *self
                .out
                .entry(sector_of(hero))
                .or_insert_with(|| way_out(triggers, hero, leads, dungeon)),
            _ => None,
        };
        way.unwrap_or(target)
    }

    /// The way into the hidden sector of the target `target`, kept by its sector ([`way_in`]),
    /// or `fixed` in its place where given.
    pub fn inward(
        &mut self,
        triggers: &Triggers,
        target: Cell,
        leads: impl Fn(&str) -> Option<Cell>,
        fixed: Option<Cell>,
    ) -> Option<Cell> {
        let kept = *self
            .into
            .entry(sector_of(target))
            .or_insert_with(|| way_in(triggers, target, leads));
        fixed.or(kept)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dungeons' sectors of the tests: the column 50 from the row 0 to 9.
    fn dungeon((x, y): Cell) -> bool {
        x == 50 && (0..10).contains(&y)
    }

    /// Where the tests' triggers lead.
    fn leads(name: &str) -> Option<Cell> {
        match name {
            // Out of the dungeon, by its mouth in the sector 52,12.
            "out" => Some((3340, 800)),
            // From one of the dungeon's sectors into another.
            "deeper" => Some((3210, 300)),
            // From outside into the dungeon.
            "in" => Some((3215, 90)),
            _ => None,
        }
    }

    fn triggers() -> Triggers {
        let mut triggers = Triggers::default();
        // The dungeon's exit in its sector 50,1, then a teleport deeper set after it there.
        triggers.set("out", (3205, 70, 0), Some((3206, 71)));
        triggers.set("deeper", (3220, 80, 0), None);
        // The dungeon's mouth outside, in the sector 52,12, and a trigger of no teleport.
        triggers.set("in", (3340, 801, 0), None);
        triggers.set("talk", (3341, 802, 0), None);
        triggers
    }

    #[test]
    fn the_way_in_is_where_the_nearest_sectors_last_teleport_leads() {
        let triggers = triggers();
        // A target in the dungeon's sector 50,1: its own sector's last teleport, the deeper one.
        assert_eq!(way_in(&triggers, (3230, 100), leads), Some((3210, 300)));
        // A target nearer the mouth's sector: the mouth's teleport into the dungeon.
        assert_eq!(way_in(&triggers, (3330, 790), leads), Some((3215, 90)));
        // No sector within 25,000 cells.
        let mut far = Triggers::default();
        far.set("out", (30_000, 30_000, 0), None);
        assert_eq!(way_in(&far, (0, 0), leads), None);
    }

    #[test]
    fn the_way_out_is_the_nearest_trigger_that_leads_out_of_the_dungeons() {
        let triggers = triggers();
        // The deeper teleport stands last in the hero's sector and leads into a dungeon: the
        // exit's own cell instead, the lower corner of its box.
        assert_eq!(
            way_out(&triggers, (3230, 100), leads, dungeon),
            Some((3205, 70))
        );
        let mut none = Triggers::default();
        none.set("deeper", (3220, 80, 0), None);
        assert_eq!(way_out(&none, (3230, 100), leads, dungeon), None);
    }

    #[test]
    fn the_distance_rounds_to_the_nearest_whole_and_ties_go_by_the_sectors_numbering() {
        assert_eq!(distance((0, 0), (1, 1)), 1);
        assert_eq!(distance((0, 0), (1, 2)), 2);
        assert_eq!(distance((0, 0), (2, 2)), 3);
        assert_eq!(distance((0, 0), (3, 4)), 5);
        // Two sectors as far from the target, 10,1 and 9,2: the one of the earlier row, its
        // number the lower, though its column is the later.
        let mut triggers = Triggers::default();
        triggers.set("in", (9 * 64, 2 * 64, 0), None);
        triggers.set("out", (10 * 64, 64, 0), None);
        let target = (9 * 64 + 32, 64 + 32);
        let first = |sector: Cell| (sector.0 * SIDE, sector.1 * SIDE);
        assert_eq!(
            distance(first((10, 1)), target),
            distance(first((9, 2)), target)
        );
        assert_eq!(way_in(&triggers, target, leads), leads("out"));
    }

    #[test]
    fn the_sectors_are_measured_from_their_first_cells_and_below_25000_alone() {
        let way = |sectors: &[Cell], target| {
            let mut triggers = Triggers::default();
            for (n, &(x, y)) in sectors.iter().enumerate() {
                let name = ["out", "deeper"][n];
                triggers.set(name, (x * SIDE, y * SIDE, 0), None);
            }
            way_in(&triggers, target, leads)
        };
        // The first cells (64, 0) and (192, 0), (128, 0) and (640, 0), (0, 64) and (0, 192).
        assert_eq!(way(&[(1, 0), (3, 0)], (180, 0)), leads("deeper"));
        assert_eq!(way(&[(2, 0), (10, 0)], (300, 0)), leads("out"));
        assert_eq!(way(&[(0, 1), (0, 3)], (0, 180)), leads("deeper"));
        // The sector 390,0's first cell lies 25,000 cells from (−40, 0), 24,999 from (−39, 0).
        assert_eq!(way(&[(390, 0)], (-40, 0)), None);
        assert_eq!(way(&[(390, 0)], (-39, 0)), leads("out"));
    }

    #[test]
    fn quest_61_takes_its_fixed_way_while_the_hero_holds_the_air_and_the_fire() {
        assert_eq!(fixed_way(Some(61), &[AIR, FIRE]), Some(FIXED_WAY));
        assert_eq!(fixed_way(Some(61), &[FIRE, 1, AIR]), Some(FIXED_WAY));
        assert_eq!(fixed_way(Some(61), &[AIR, FIRE, EARTH]), None);
        assert_eq!(fixed_way(Some(61), &[AIR]), None);
        assert_eq!(fixed_way(Some(60), &[AIR, FIRE]), None);
        assert_eq!(fixed_way(None, &[AIR, FIRE]), None);
    }

    #[test]
    fn a_marker_points_across_the_mouth_at_the_way_kept_for_each_sector() {
        let triggers = triggers();
        let hidden = |(x, y): Cell| {
            u32::try_from(x)
                .ok()
                .zip(u32::try_from(y).ok())
                .is_some_and(|sector| crate::explored::hidden(sector, dungeon((x, y))))
        };
        let mut ways = Ways::default();
        let mut toward = |triggers: &Triggers, cells, fixed| {
            ways.toward(triggers, cells, (hidden, dungeon), leads, fixed)
        };
        // The hero inside, the target outside: the way out, at every call (the repair of the
        // original's first call alone).
        for _ in 0..3 {
            assert_eq!(
                toward(&triggers, ((3230, 100), (3000, 2000)), None),
                (3205, 70)
            );
        }
        // The hero outside, the target inside: the way in.
        assert_eq!(
            toward(&triggers, ((3000, 2000), (3230, 100)), None),
            (3210, 300)
        );
        // Kept by the target's sector: another target in it takes the same way, whatever the
        // triggers have become.
        assert_eq!(
            toward(&Triggers::default(), ((3000, 2000), (3250, 120)), None),
            (3210, 300)
        );
        // Quest 61's fixed way at its call, the kept way at the next.
        assert_eq!(
            toward(&triggers, ((3000, 2000), (3230, 100)), Some(FIXED_WAY)),
            FIXED_WAY
        );
        assert_eq!(
            toward(&triggers, ((3000, 2000), (3230, 100)), None),
            (3210, 300)
        );
        // Both inside, or both outside: the target itself.
        assert_eq!(
            toward(&triggers, ((3230, 100), (3240, 400)), None),
            (3240, 400)
        );
        assert_eq!(
            toward(&triggers, ((3000, 2000), (3100, 2100)), Some(FIXED_WAY)),
            (3100, 2100)
        );
    }
}
