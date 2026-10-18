//! Sector streaming: the sectors around the hero's are built in the background (ground image,
//! mesh, liquid surfaces and sprite quads, `terrain`, `liquids` and `statics`) and spawned when
//! ready; those far away go.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use bevy::prelude::*;
use bevy::tasks::futures::check_ready;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use opensacred_data::world::{SIDE, Sector, Sprites, World};

use crate::frame::Step;
use crate::hero::Body;
use crate::liquids::Liquids;
use crate::terrain::Heights;
use crate::{daynight, iso, liquids, statics, terrain};

/// Sectors around the hero's that load: a radius of 1 is 3 × 3.
const LOAD_RADIUS: i64 = 1;
/// Loaded sectors farther than this go.
const KEEP_RADIUS: i64 = 2;
/// The grid of sectors.
const GRID: i64 = 100;
/// Sectors built at once: Bevy compiles its pipelines on the same pool of threads.
const BUILDING_AT_ONCE: usize = 2;

/// What every build reads: the asset directory, the world index and the sprite table.
pub struct Shared {
    pub assets: PathBuf,
    pub world: World,
    pub sprites: Sprites,
}

/// A sector built off the main thread.
pub struct Built {
    pub sector: Sector,
    ground: terrain::Ground,
    liquids: liquids::Surfaces,
    statics: statics::Statics,
}

/// The outcome of building a sector: `None` where the grid has no sector.
type Building = Result<Option<Built>>;

/// Builds sector `at`.
pub fn build(shared: &Shared, at: (u32, u32)) -> Building {
    let Some(sector) = terrain::read_sector(&shared.assets, &shared.world, at)? else {
        return Ok(None);
    };
    let ground = terrain::build(&shared.assets, &shared.world, &sector)?;
    let liquids = liquids::build(&sector);
    let statics = statics::build(&shared.assets, &shared.world, &shared.sprites, &sector)?;
    Ok(Some(Built {
        sector,
        ground,
        liquids,
        statics,
    }))
}

#[derive(Resource)]
pub struct Streamer {
    shared: Arc<Shared>,
    /// The entities of each spawned sector.
    loaded: HashMap<(u32, u32), Vec<Entity>>,
    building: HashMap<(u32, u32), Task<Building>>,
    /// Built and waiting to spawn, one a frame.
    ready: Vec<((u32, u32), Built)>,
    /// Positions without a sector, or whose build failed: not tried again.
    missing: HashSet<(u32, u32)>,
}

impl Streamer {
    /// The sectors spawned.
    pub fn sectors(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.loaded.keys().copied()
    }

    /// Makes `entity` part of sector `at`: it goes when the sector goes.
    pub fn adopt(&mut self, at: (u32, u32), entity: Entity) {
        self.loaded.entry(at).or_default().push(entity);
    }

    /// Whether every sector near the hero is in place.
    pub fn idle(&self) -> bool {
        self.building.is_empty() && self.ready.is_empty()
    }

    /// A streamer that starts with sector `at`, built already.
    pub fn new(shared: Shared, at: (u32, u32), first: Built) -> Self {
        Self {
            shared: Arc::new(shared),
            loaded: HashMap::new(),
            building: HashMap::new(),
            ready: vec![(at, first)],
            missing: HashSet::new(),
        }
    }
}

/// The sector under world point `p`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a sector index of the world is a small whole number"
)]
fn sector_of(p: Vec3) -> (i64, i64) {
    let size = iso::CELL * SIDE as f32;
    ((p.x / size).floor() as i64, (p.z / size).floor() as i64)
}

/// The sectors within `radius` of `(x, y)` on the grid.
fn around((x, y): (i64, i64), radius: i64) -> impl Iterator<Item = (u32, u32)> {
    (-radius..=radius)
        .flat_map(move |dy| (-radius..=radius).map(move |dx| (x + dx, y + dy)))
        .filter(|&(x, y)| (0..GRID).contains(&x) && (0..GRID).contains(&y))
        .filter_map(|(x, y)| Some((u32::try_from(x).ok()?, u32::try_from(y).ok()?)))
}

/// Spawns the sectors that are ready, starts building those that come near the hero, and
/// despawns those far from it.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources one by one"
)]
pub fn stream(
    mut commands: Commands,
    mut streamer: ResMut<Streamer>,
    bodies: Query<&Transform, With<Body>>,
    mut heights: ResMut<Heights>,
    mut liquids: ResMut<Liquids>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut sprite_materials: ResMut<Assets<statics::SpriteMaterial>>,
    mut grounds: ResMut<Assets<terrain::GroundMaterial>>,
    light_types: Res<daynight::LightTypes>,
    glow: Option<Res<statics::Glow>>,
) {
    let streamer = &mut *streamer;
    let finished: Vec<_> = streamer
        .building
        .iter_mut()
        .filter_map(|(&at, task)| check_ready(task).map(|result| (at, result)))
        .collect();
    for (at, result) in finished {
        streamer.building.remove(&at);
        match result {
            Ok(Some(built)) => streamer.ready.push((at, built)),
            Ok(None) => {
                streamer.missing.insert(at);
            }
            Err(e) => {
                error!("sector {},{}: {e:#}", at.0, at.1);
                streamer.missing.insert(at);
            }
        }
    }
    // One sector a frame: uploading a ground image takes a few milliseconds.
    if let Some((at, mut built)) = streamer.ready.pop() {
        info!(
            "sector {},{}: {} static objects, {} with a sprite",
            at.0, at.1, built.statics.objects, built.statics.drawn
        );
        heights.add(&built.sector);
        let lights = daynight::sources(&built.sector, &light_types);
        let mut entities = terrain::spawn(
            &mut commands,
            built.ground,
            &mut images,
            &mut meshes,
            &mut grounds,
        );
        entities.extend(liquids::spawn(
            &mut commands,
            built.liquids,
            &mut liquids,
            &assets,
            &mut meshes,
            &mut materials,
        ));
        let glows = std::mem::take(&mut built.statics.glows);
        if let Some(glow) = &glow {
            entities.extend(statics::spawn_glows(
                &mut commands,
                glows,
                &mut meshes,
                &glow.0,
            ));
        }
        entities.extend(statics::spawn(
            &mut commands,
            built.statics,
            &assets,
            &mut images,
            &mut meshes,
            &mut sprite_materials,
        ));
        entities.extend(lights.into_iter().map(|(position, radius)| {
            commands
                .spawn((
                    Transform::from_translation(position),
                    daynight::Light(radius),
                ))
                .id()
        }));
        streamer.loaded.insert(at, entities);
    }

    let Some(hero) = bodies.iter().next() else {
        return;
    };
    let here = sector_of(hero.translation);
    for at in around(here, LOAD_RADIUS) {
        if streamer.building.len() >= BUILDING_AT_ONCE {
            break;
        }
        if streamer.loaded.contains_key(&at)
            || streamer.building.contains_key(&at)
            || streamer.ready.iter().any(|(ready, _)| *ready == at)
            || streamer.missing.contains(&at)
        {
            continue;
        }
        let shared = Arc::clone(&streamer.shared);
        let task = AsyncComputeTaskPool::get().spawn(async move { build(&shared, at) });
        streamer.building.insert(at, task);
    }
    let near: HashSet<(u32, u32)> = around(here, KEEP_RADIUS).collect();
    let far: Vec<(u32, u32)> = streamer
        .loaded
        .keys()
        .filter(|at| !near.contains(at))
        .copied()
        .collect();
    for at in far {
        for entity in streamer.loaded.remove(&at).unwrap_or_default() {
            commands.entity(entity).despawn();
        }
    }
}

/// The sectors streamed in and out around the hero.
pub struct SectorsPlugin;

impl Plugin for SectorsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, stream.in_set(Step::Sectors));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hero_loads_the_sectors_around_it_on_the_grid() {
        let point = iso::ground(50.5 * SIDE as f32, 39.2 * SIDE as f32);
        assert_eq!(sector_of(point), (50, 39));
        assert_eq!(around((50, 39), 1).count(), 9);
        assert_eq!(
            around((0, 0), 1).collect::<Vec<_>>(),
            [(0, 0), (1, 0), (0, 1), (1, 1)]
        );
    }
}
