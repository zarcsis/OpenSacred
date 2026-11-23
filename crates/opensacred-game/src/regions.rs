//! The regions' kills (#1574; docs/re/maps.md, "The overview map"): the regions of the balance's
//! seventh table, and each creature's death that someone caused counting its region down by one
//! (`opensacred_rules::regions`, `0x424250`; **hypothesis**: every death counts, whoever the
//! killer); the overview writes the hero's region's line (`overview`). As a region's count first
//! falls below 1,000 its warning shows as an info line (`info_lines`), and its last battle is set
//! up: the scripts' function `RegionCounterZero<n>` of its number runs, which sets up its side
//! quest 5080 + `n` (the engine's events `0xB` and `0x1A`, `0x618090`; #3420).

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

/// The warning's text of `warning` (`0x424250`): `UI_REGION_FINALBATTLE` while a region of 1 to 9
/// has not warned, else `UI_REGION_ANCARIA`.
fn warning_key(warning: regions::Warning) -> &'static str {
    match warning {
        regions::Warning::FinalBattle => "UI_REGION_FINALBATTLE",
        regions::Warning::Ancaria => "UI_REGION_ANCARIA",
    }
}

/// The scripts' function that sets up the last battle of the region of the number `number`
/// (`RegionCounterZero1` to `9`, the engine's case `0x1A`).
fn last_battle(number: u8) -> String {
    format!("RegionCounterZero{number}")
}

/// Each death that someone caused counts its region down (`0x424250`); a region's first count
/// below 1,000 shows its warning as an info line and runs the function of its last battle
/// (#3420).
fn count(
    mut commands: Commands,
    mut region_kills: ResMut<RegionKills>,
    (streamer, texts, mut shown): (
        Option<Res<crate::sectors::Streamer>>,
        Option<Res<Texts>>,
        ResMut<crate::info_lines::Shown>,
    ),
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
            let key = warning_key(warning);
            let line = texts
                .as_deref()
                .and_then(|texts| texts.named(key))
                .unwrap_or(key);
            shown.0.push(line.to_owned());
            let Some(region) =
                regions::region_of(regions, area).and_then(|index| regions.get(index))
            else {
                continue;
            };
            let function = last_battle(region.index);
            commands.queue(move |world: &mut World| {
                match crate::scripts::run_function(world, &function) {
                    Ok(lines) => info!("regions: {}", lines.join("; ")),
                    Err(e) => warn!("regions: {e}"),
                }
            });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_regions_warning_tells_its_battle_and_runs_its_function() {
        assert_eq!(
            warning_key(regions::Warning::FinalBattle),
            "UI_REGION_FINALBATTLE"
        );
        assert_eq!(warning_key(regions::Warning::Ancaria), "UI_REGION_ANCARIA");
        assert_eq!(last_battle(1), "RegionCounterZero1");
        assert_eq!(last_battle(9), "RegionCounterZero9");
    }
}
