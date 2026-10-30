//! The regions' kills (#1574; docs/re/maps.md, "The overview map"): the regions of the balance's
//! seventh table, and each creature's death that someone caused counting its region down by one
//! (`opensacred_rules::regions`, `0x424250`; **hypothesis**: every death counts, whoever the
//! killer); the overview writes the hero's region's line (`overview`). A region's warning as its
//! count first falls below 1,000 is logged, what it shows not read.

use bevy::prelude::*;
use opensacred_data::tables::Region;
use opensacred_rules::regions::{self, Kills};

use crate::game::{AppState, Data};
use crate::lines::Texts;
use crate::npcs::Dies;

/// The regions and the kills that remain in each.
#[derive(Resource, Debug, Default)]
pub struct RegionKills {
    regions: Vec<Region>,
    kills: Kills,
}

/// The line's format's label (`0x9E0FBC`: "label: name (remaining/total)").
const LABEL: &str = "UI_REGION_REGION";

impl RegionKills {
    /// The line of the region of the area `area` (`0x9E0FBC`, `0x9E0FDC` where both counts are
    /// 0), none for an area of no region.
    pub fn line(&self, area: u8, texts: Option<&Texts>) -> Option<String> {
        let index = regions::region_of(&self.regions, area)?;
        let region = self.regions.get(index)?;
        let said = |key: &str| {
            texts
                .and_then(|texts| texts.named(key))
                .unwrap_or(key)
                .to_owned()
        };
        let remaining = self.kills.remaining(index);
        Some(if remaining == 0 && region.total == 0 {
            format!("{}: {}", said(LABEL), said(&region.name))
        } else {
            format!(
                "{}: {} ({remaining}/{})",
                said(LABEL),
                said(&region.name),
                region.total
            )
        })
    }
}

/// A new game's counts, each region's total.
fn begin(mut commands: Commands, data: Option<Res<Data>>) {
    let regions = data
        .as_ref()
        .and_then(|data| data.0.tables.balance().ok())
        .map(|balance| balance.regions.clone())
        .unwrap_or_default();
    let kills = Kills::new(&regions);
    commands.insert_resource(RegionKills { regions, kills });
}

/// Each death that someone caused counts its region down (`0x424250`).
fn count(
    mut region_kills: ResMut<RegionKills>,
    streamer: Option<Res<crate::sectors::Streamer>>,
    deaths: Query<(&Dies, &crate::ai::Mind, &GlobalTransform), Added<Dies>>,
) {
    let Some(streamer) = streamer else {
        return;
    };
    for (dies, mind, at) in &deaths {
        if !dies.killed {
            continue;
        }
        let at = at.translation();
        let sector = crate::music::sector_of(at.x, at.z);
        let Some(area) = streamer.area(sector) else {
            continue;
        };
        let RegionKills { regions, kills } = &mut *region_kills;
        if let Some(warning) = kills.kill(regions, area, mind.creature) {
            info!("regions: the area {area}'s region warns: {warning:?}");
        }
    }
}

/// The regions' kills.
pub struct RegionsPlugin;

impl Plugin for RegionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RegionKills>()
            .add_systems(OnEnter(AppState::InGame), begin)
            .add_systems(Update, count.in_set(crate::frame::Playing));
    }
}
