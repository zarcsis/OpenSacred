//! The land explored (#1568; docs/re/maps.md, "The lens and the fog of war"): as the hero steps
//! into a cell, the cells about it explored by the stamp of the time and the place
//! (`opensacred_rules::explored`, `0x5997C0`): a disc 29 cells across by day, 15 at night, a
//! smaller patch in a dungeon; the world map's lens shows the rest under its fog (`minimap`).

use bevy::prelude::*;
use opensacred_rules::explored::{Explored, Sectors, Stamp};

use crate::daynight::{Clock, Dungeons};
use crate::game::AppState;
use crate::hero::Body;
use crate::paths;

/// The land the hero has explored.
#[derive(Resource, Default, Debug, Deref, DerefMut)]
pub struct Land(pub Explored);

/// The sectors the hero has seen and entered, which the world map's names and frontier follow
/// (#1567, `0x6263B0`).
#[derive(Resource, Default, Debug, Deref, DerefMut)]
pub struct MapSectors(pub Sectors);

/// Stamps the cells about the hero as it steps into a cell, the last one kept (`0xAD4404`).
fn stamp(
    heroes: Query<&GlobalTransform, With<Body>>,
    (clock, dungeons): (Option<Res<Clock>>, Option<Res<Dungeons>>),
    (mut land, mut sectors): (ResMut<Land>, ResMut<MapSectors>),
    mut last: Local<Option<paths::Cell>>,
) {
    let Some(hero) = heroes.iter().next() else {
        return;
    };
    let at = hero.translation();
    let cell = paths::cell_of(at.x, at.z);
    if *last == Some(cell) {
        return;
    }
    *last = Some(cell);
    let sector = (
        u32::try_from(cell.0.div_euclid(64)).unwrap_or(0),
        u32::try_from(cell.1.div_euclid(64)).unwrap_or(0),
    );
    let dungeon = dungeons.is_some_and(|dungeons| dungeons.sector(sector));
    // The sector entered, as its cell's first step into it marks it (`0x6263B0`).
    if let (Ok(x), Ok(y)) = (u16::try_from(sector.0), u16::try_from(sector.1))
        && !sectors.entered((x, y))
    {
        sectors.enter((x, y));
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the minutes of a day, below 1,440"
    )]
    let minute = clock.map_or(12 * 60, |clock| clock.minutes() as u32);
    land.stamp(cell, Stamp::at(minute, dungeon));
}

/// A new game's land, unexplored.
fn reset(mut land: ResMut<Land>, mut sectors: ResMut<MapSectors>) {
    *land = Land::default();
    *sectors = MapSectors::default();
}

/// The land explored.
pub struct ExploredPlugin;

impl Plugin for ExploredPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Land>()
            .init_resource::<MapSectors>()
            .add_systems(OnEnter(AppState::InGame), reset)
            .add_systems(
                Update,
                stamp
                    .after(crate::frame::Step::HeroMove)
                    .in_set(crate::frame::Playing),
            );
    }
}
