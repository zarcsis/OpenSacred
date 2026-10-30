//! The regions' kills (docs/re/maps.md, "The overview map"; #1574): the regions of
//! `Balance.bin`'s seventh table (`0xAAAD28`), each its areas and its total, and the kills that
//! remain in each, which every creature's death in its region counts down by one (`0x424250`),
//! never below 0, but the type 71's; the first time a region's count falls below 1,000 it warns,
//! of the final battle while another region has not warned yet, else of Ancaria.

use opensacred_data::tables::Region;
use serde::{Deserialize, Serialize};

/// The creature type whose deaths count in no region (`0x424250`).
const UNCOUNTED: u32 = 71;
/// The count below which a region warns, once (`0x424250`).
const WARNING: u32 = 1000;

/// The index among `regions` of the region of the area `area`: the first whose areas hold it
/// (`0x4254A0`), none for the area 0.
pub fn region_of(regions: &[Region], area: u8) -> Option<usize> {
    if area == 0 {
        return None;
    }
    regions
        .iter()
        .position(|region| region.areas.contains(&area))
}

/// A region's warning as its count first falls below 1,000 (`UI_REGION_FINALBATTLE`,
/// `UI_REGION_ANCARIA`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Warning {
    FinalBattle,
    Ancaria,
}

/// The kills that remain in each region, by its index among the regions, and whether it has
/// warned (the record's `+0x38` and `+0x34`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kills {
    remaining: Vec<u32>,
    warned: Vec<bool>,
}

impl Kills {
    /// The counts of a new game, each region's total (`0x425290`; no save keeps them).
    pub fn new(regions: &[Region]) -> Self {
        Self {
            remaining: regions.iter().map(|region| region.total).collect(),
            warned: vec![false; regions.len()],
        }
    }

    /// The kills that remain in the region `region`.
    pub fn remaining(&self, region: usize) -> u32 {
        self.remaining.get(region).copied().unwrap_or(0)
    }

    /// A creature of the type `creature` dead in the area `area`: its region's count less one, and
    /// the warning where it first falls below 1,000.
    pub fn kill(&mut self, regions: &[Region], area: u8, creature: u32) -> Option<Warning> {
        if creature == UNCOUNTED {
            return None;
        }
        let region = region_of(regions, area)?;
        let count = self.remaining.get_mut(region)?;
        *count = count.saturating_sub(1);
        let count = *count;
        let warned = self.warned.get_mut(region)?;
        if count >= WARNING || count == 0 || *warned {
            return None;
        }
        *warned = true;
        Some(if self.warned.iter().all(|&warned| warned) {
            Warning::Ancaria
        } else {
            Warning::FinalBattle
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regions() -> Vec<Region> {
        let region = |index: u8, areas: &[u8], total: u32| Region {
            index,
            areas: areas.to_vec(),
            name: format!("UI_REGION_{index}"),
            total,
        };
        vec![
            region(1, &[1, 2, 13], 1002),
            region(4, &[12, 13, 16], 8500),
            region(5, &[16, 25], 1000),
        ]
    }

    #[test]
    fn an_area_takes_the_first_region_that_holds_it() {
        let regions = regions();
        assert_eq!(region_of(&regions, 13), Some(0));
        assert_eq!(region_of(&regions, 16), Some(1));
        assert_eq!(region_of(&regions, 26), None);
        assert_eq!(region_of(&regions, 0), None);
    }

    #[test]
    fn a_death_counts_its_region_down_and_warns_once_below_a_thousand() {
        let regions = regions();
        let mut kills = Kills::new(&regions);
        assert_eq!(kills.remaining(0), 1002);
        // The type 71 and an area of no region count nowhere.
        assert_eq!(kills.kill(&regions, 1, 71), None);
        assert_eq!(kills.kill(&regions, 26, 13), None);
        assert_eq!(kills.remaining(0), 1002);
        assert_eq!(kills.kill(&regions, 2, 13), None);
        assert_eq!(kills.kill(&regions, 2, 13), None);
        // 999: the final battle's warning, another region not warned yet; then no more.
        assert_eq!(kills.kill(&regions, 1, 13), Some(Warning::FinalBattle));
        assert_eq!(kills.kill(&regions, 1, 13), None);
        assert_eq!(kills.remaining(0), 998);
        // The last regions to warn: Ancaria's once every one has.
        assert_eq!(kills.kill(&regions, 25, 13), Some(Warning::FinalBattle));
        for _ in 0..7501 {
            kills.kill(&regions, 12, 13);
        }
        assert_eq!(kills.remaining(1), 999);
        let mut last = Kills::new(&regions);
        last.warned = vec![true, false, true];
        last.remaining[1] = 1000;
        assert_eq!(last.kill(&regions, 12, 13), Some(Warning::Ancaria));
        // Never below 0.
        last.remaining[2] = 0;
        assert_eq!(last.kill(&regions, 25, 13), None);
        assert_eq!(last.remaining(2), 0);
    }
}
