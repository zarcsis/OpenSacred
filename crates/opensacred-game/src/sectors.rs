//! Sector streaming: the sectors around the hero's are built in the background (ground image,
//! mesh, liquid surfaces and sprite quads, `terrain`, `liquids` and `statics`) and spawned when
//! ready; those far away go. A hero that arrives somewhere (the game's start, a teleport, a
//! command, its rise) finds the sectors in view about it whole: its frame waits for them (#1650).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use bevy::prelude::*;
use bevy::tasks::futures::check_ready;
use bevy::tasks::{AsyncComputeTaskPool, ComputeTaskPool, Task, block_on};
use opensacred_data::world::{SIDE, Sector, Sprites, World};

use crate::frame::Step;
use crate::game::AppState;
use crate::hero::{Body, Hero};
use crate::liquids::Liquids;
use crate::tables::Tables;
use crate::terrain::Heights;
use crate::{blended, daynight, iso, liquids, statics, terrain};

/// Sectors around the hero's that load: a radius of 1 is 3 × 3.
const LOAD_RADIUS: i64 = 1;
/// Loaded sectors farther than this go.
const KEEP_RADIUS: i64 = 2;
/// The grid of sectors.
const GRID: i64 = 100;
/// Sectors built at once: Bevy compiles its pipelines on the same pool of threads.
const BUILDING_AT_ONCE: usize = 2;
/// How near a hero that arrives somewhere a sector comes in with it, along either axis of the
/// ground: half a sector, 1,717 units, while the view at its widest zoom on a 16:9 screen reaches
/// 1,621 from the point it looks at (`iso`: 1,079 to the right, 1,214 up the ground). The rest of
/// the 3 × 3 follows in the background, out of view.
const VIEW: f32 = (SIDE / 2) as f32 * iso::CELL;

/// What every build reads: the asset directory, the world index, the sprite table and the
/// shadow atlas, relative to the asset directory, if the converted textures have it; and the
/// tables that the start and every game's load read, each parsed once (#830).
pub struct Shared {
    pub assets: PathBuf,
    pub world: World,
    pub sprites: Sprites,
    pub tables: Tables,
    pub shadows: Option<String>,
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
    let statics = statics::build(
        &shared.assets,
        &shared.world,
        &shared.sprites,
        shared.shadows.as_deref(),
        &sector,
    )?;
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

    /// The area of the sector at `at` in the world's index, which the client turns into a region
    /// (docs/re/sound.md); `None` where the world has no sector.
    pub fn area(&self, at: (u32, u32)) -> Option<u8> {
        self.shared
            .world
            .sectors
            .iter()
            .find(|entry| (entry.x, entry.y) == at)
            .map(|entry| entry.area)
    }

    /// Makes `entity` part of sector `at`: it goes when the sector goes, and leaves the sector's
    /// list when it goes before ([`forget`]).
    pub fn adopt(&mut self, commands: &mut Commands, at: (u32, u32), entity: Entity) {
        self.loaded.entry(at).or_default().push(entity);
        commands.entity(entity).try_insert(InSector(at));
    }

    /// Takes `entity` from its sector, which no longer despawns it: a creature that the scripts
    /// hire goes with the hero wherever it goes (#657).
    pub fn disown(&mut self, entity: Entity) {
        for entities in self.loaded.values_mut() {
            entities.retain(|&kept| kept != entity);
        }
    }

    /// Whether every sector near world point `hero` is in place, spawned or known to be missing,
    /// and nothing more is building. Right after a teleport the sectors of the new place are not
    /// asked for yet, so empty queues alone do not tell (#202).
    pub fn settled(&self, hero: Vec3) -> bool {
        self.building.is_empty()
            && self.ready.is_empty()
            && around(sector_of(hero), LOAD_RADIUS)
                .all(|at| self.loaded.contains_key(&at) || self.missing.contains(&at))
    }

    /// The sectors being built or waiting to spawn.
    pub fn building(&self) -> usize {
        self.building.len() + self.ready.len()
    }

    /// A streamer that starts with sector `at`, built already.
    pub fn new(shared: Arc<Shared>, at: (u32, u32), first: Built) -> Self {
        Self {
            ready: vec![(at, first)],
            ..Self::empty(shared)
        }
    }

    /// A streamer with no sector yet.
    fn empty(shared: Arc<Shared>) -> Self {
        Self {
            shared,
            loaded: HashMap::new(),
            building: HashMap::new(),
            ready: Vec::new(),
            missing: HashSet::new(),
        }
    }

    /// Whether sector `at` is in place: spawned, or known to be missing.
    fn holds(&self, at: (u32, u32)) -> bool {
        self.loaded.contains_key(&at) || self.missing.contains(&at)
    }

    /// Starts building sector `at` in the background.
    fn start(&mut self, at: (u32, u32)) {
        let shared = Arc::clone(&self.shared);
        let task = AsyncComputeTaskPool::get().spawn(async move { build(&shared, at) });
        self.building.insert(at, task);
    }

    /// Takes sector `at` built: ready to spawn, or missing where the grid has none or its build
    /// failed.
    fn finish(&mut self, at: (u32, u32), result: Building) {
        match result {
            Ok(Some(built)) => self.ready.push((at, built)),
            Ok(None) => {
                self.missing.insert(at);
            }
            Err(e) => {
                error!("sector {},{}: {e:#}", at.0, at.1);
                self.missing.insert(at);
            }
        }
    }

    /// Builds every sector within [`VIEW`] of world point `hero` that is neither in place nor
    /// ready, all at once, and waits for them, those building already among them; answers how
    /// many it waited for. The original's view takes the 3 × 3 about its centre whenever the
    /// centre changes (`0x62A5C0`) through its sector cache (`0x634340`), which reads a sector not
    /// in memory on the spot (`0x637790`), so that its frame waits for them; here a sector's
    /// ground is a picture to upload, and those out of view follow in the background.
    fn complete(&mut self, hero: Vec3) -> usize {
        let wanted: Vec<_> = in_view(hero)
            .filter(|&at| !self.holds(at) && !self.ready.iter().any(|(ready, _)| *ready == at))
            .collect();
        // Half of those not building yet go to the background's pool, the other half to the
        // systems' pool, idle while the frame waits: twice the threads, half the wait.
        let new: Vec<_> = wanted
            .iter()
            .filter(|at| !self.building.contains_key(at))
            .copied()
            .collect();
        let (background, now) = new.split_at(new.len() / 2);
        for &at in background {
            self.start(at);
        }
        let shared = &*self.shared;
        let built = ComputeTaskPool::get().scope(|scope| {
            for &at in now {
                scope.spawn(async move { (at, build(shared, at)) });
            }
        });
        for (at, result) in built {
            self.finish(at, result);
        }
        for &at in &wanted {
            if let Some(task) = self.building.remove(&at) {
                let result = block_on(task);
                self.finish(at, result);
            }
        }
        wanted.len()
    }
}

/// The sector that adopted an entity ([`Streamer::adopt`]).
#[derive(Component)]
pub struct InSector((u32, u32));

/// Takes an adopted entity that goes before its sector off the sector's list, whatever despawns
/// it: an item the hero picks up, a creature the scripts remove, an undead's corpse made anew; its
/// sector, once gone, would despawn it again (#1588).
fn forget(
    gone: On<Despawn<InSector>>,
    sectors: Query<&InSector>,
    streamer: Option<ResMut<Streamer>>,
) {
    let (Ok(InSector(at)), Some(mut streamer)) = (sectors.get(gone.entity), streamer) else {
        return;
    };
    if let Some(entities) = streamer.loaded.get_mut(at) {
        entities.retain(|&kept| kept != gone.entity);
    }
}

/// Despawns every sector of the game left, with what they adopted; the sectors being built go
/// with the streamer (`game`).
pub fn leave(mut commands: Commands, streamer: Option<Res<Streamer>>) {
    for entity in streamer.iter().flat_map(|s| s.loaded.values().flatten()) {
        commands.entity(*entity).try_despawn();
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

/// The sectors within [`VIEW`] of world point `p` along both axes of the ground: its own, and
/// those across the edges and the corner of the quarter of it that `p` lies in, at most four.
fn in_view(p: Vec3) -> impl Iterator<Item = (u32, u32)> {
    let size = iso::CELL * SIDE as f32;
    let beyond = move |low: f32, at: f32| (low - at).max(at - (low + size));
    around(sector_of(p), LOAD_RADIUS).filter(move |&(x, y)| {
        beyond(x as f32 * size, p.x) < VIEW && beyond(y as f32 * size, p.z) < VIEW
    })
}

/// Spawns the sectors that are ready, starts building those that come near the hero, and
/// despawns those far from it; the sectors in view of a hero that arrives somewhere, or of one
/// near a sector not in yet, come in within the frame.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources one by one"
)]
pub fn stream(
    mut commands: Commands,
    mut streamer: ResMut<Streamer>,
    (bodies, hero): (Query<(&Transform, &Body)>, Option<Res<Hero>>),
    roaming: Query<&Transform, With<crate::npcs::Placed>>,
    mut heights: ResMut<Heights>,
    mut liquids: ResMut<Liquids>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<blended::Sorted>>,
    mut sprite_materials: ResMut<Assets<statics::SpriteMaterial>>,
    mut grounds: ResMut<Assets<terrain::GroundMaterial>>,
    light_types: Res<daynight::LightTypes>,
    glow: Option<Res<statics::Glow>>,
    shadows: Option<Res<statics::StaticShadows>>,
    fade: Res<statics::Fade>,
) {
    let streamer = &mut *streamer;
    let finished: Vec<_> = streamer
        .building
        .iter_mut()
        .filter_map(|(&at, task)| check_ready(task).map(|result| (at, result)))
        .collect();
    for (at, result) in finished {
        streamer.building.remove(&at);
        streamer.finish(at, result);
    }
    // Where the hero stands, or starts while its model loads, and whether it arrives there.
    let (point, arriving) = match (bodies.iter().next(), hero) {
        (Some((transform, body)), _) => (transform.translation, body.arriving()),
        (None, Some(hero)) => (hero.position, true),
        (None, None) => return,
    };
    let here = sector_of(point);
    let near: HashSet<(u32, u32)> = around(here, KEEP_RADIUS).collect();
    // Built for a place the hero has left.
    streamer.ready.retain(|(at, _)| near.contains(at));
    // A hero that arrives somewhere sees its place whole in its frame, as the original's (#1650),
    // and a walk never shows a sector missing.
    let whole = arriving || in_view(point).any(|at| !streamer.holds(at));
    if whole {
        let started = Instant::now();
        let waited = streamer.complete(point);
        if waited > 0 {
            info!(
                "sectors: {waited} in view about {},{} built at once in {} ms",
                here.0,
                here.1,
                started.elapsed().as_millis()
            );
        }
    }
    // Those in view at once; the rest one a frame, out of view: uploading a ground image takes a
    // few milliseconds.
    let spawning: Vec<_> = if whole {
        let view: HashSet<(u32, u32)> = in_view(point).collect();
        let (mut now, mut later): (Vec<_>, Vec<_>) = std::mem::take(&mut streamer.ready)
            .into_iter()
            .partition(|(at, _)| view.contains(at));
        now.extend(later.pop());
        streamer.ready = later;
        now
    } else {
        streamer.ready.pop().into_iter().collect()
    };
    for (at, mut built) in spawning {
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
            statics::Common {
                fade: &fade,
                shadows: shadows.as_deref(),
            },
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

    for at in around(here, LOAD_RADIUS) {
        if streamer.building.len() >= BUILDING_AT_ONCE {
            break;
        }
        if streamer.holds(at)
            || streamer.building.contains_key(&at)
            || streamer.ready.iter().any(|(ready, _)| *ready == at)
        {
            continue;
        }
        streamer.start(at);
    }
    let far: Vec<(u32, u32)> = streamer
        .loaded
        .keys()
        .filter(|at| !near.contains(at))
        .copied()
        .collect();
    for at in far {
        for entity in streamer.loaded.remove(&at).unwrap_or_default() {
            // A creature that walked into a sector that stays goes with that one (#329).
            if let Ok(transform) = roaming.get(entity)
                && let Some(to) = around(sector_of(transform.translation), 0).next()
                && to != at
                && near.contains(&to)
                && streamer.loaded.contains_key(&to)
            {
                streamer.adopt(&mut commands, to, entity);
                continue;
            }
            commands.entity(entity).despawn();
        }
    }
}

/// The sectors streamed in and out around the hero.
pub struct SectorsPlugin;

impl Plugin for SectorsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, stream.in_set(Step::Sectors))
            .add_systems(OnExit(AppState::InGame), leave)
            .add_observer(forget);
    }
}

#[cfg(test)]
mod tests {
    use opensacred_data::world::World;

    use super::*;

    fn shared() -> Arc<Shared> {
        Arc::new(Shared {
            assets: PathBuf::new(),
            world: World {
                sectors: Vec::new(),
                sheets: Vec::new(),
                atlases: Vec::new(),
                assembled_from: Vec::new(),
            },
            sprites: Sprites {
                sprites: Vec::new(),
            },
            tables: Tables::new(&PathBuf::new()),
            shadows: None,
        })
    }

    #[test]
    fn leaving_the_game_despawns_its_sectors_and_what_they_adopted() {
        let shared = shared();
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .insert_state(AppState::InGame)
            .add_systems(OnExit(AppState::InGame), leave);
        let ground = app.world_mut().spawn(Transform::default()).id();
        let npc = app.world_mut().spawn(Transform::default()).id();
        let weapon = app
            .world_mut()
            .spawn((Transform::default(), ChildOf(npc)))
            .id();
        let camera = app.world_mut().spawn(Transform::default()).id();
        let mut streamer = Streamer::empty(shared);
        let mut commands = app.world_mut().commands();
        streamer.adopt(&mut commands, (50, 39), ground);
        streamer.adopt(&mut commands, (50, 39), npc);
        app.insert_resource(streamer);
        app.update();
        app.world_mut()
            .resource_mut::<NextState<AppState>>()
            .set(AppState::Menu);
        app.update();
        for gone in [ground, npc, weapon] {
            assert!(app.world().get_entity(gone).is_err());
        }
        assert!(app.world().get_entity(camera).is_ok());
    }

    #[test]
    fn an_adopted_entity_despawned_before_its_sector_leaves_its_list() {
        let mut app = App::new();
        app.add_observer(forget);
        let item = app.world_mut().spawn(Transform::default()).id();
        let npc = app.world_mut().spawn(Transform::default()).id();
        let mut streamer = Streamer::empty(shared());
        let mut commands = app.world_mut().commands();
        streamer.adopt(&mut commands, (50, 39), item);
        streamer.adopt(&mut commands, (50, 39), npc);
        app.insert_resource(streamer);
        app.world_mut().flush();
        // Picked up: the sector keeps only the creature.
        app.world_mut().despawn(item);
        assert_eq!(app.world().resource::<Streamer>().loaded[&(50, 39)], [npc]);
        // The sector goes, and the creature that walked out of it goes with the next one (#329).
        let mut streamer = app.world_mut().remove_resource::<Streamer>().unwrap();
        streamer.loaded.remove(&(50, 39));
        let mut commands = app.world_mut().commands();
        streamer.adopt(&mut commands, (51, 39), npc);
        app.insert_resource(streamer);
        app.world_mut().flush();
        app.world_mut().despawn(npc);
        assert!(app.world().resource::<Streamer>().loaded[&(51, 39)].is_empty());
    }

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

    #[test]
    fn the_sectors_are_settled_only_around_where_the_hero_is_now() {
        let mut streamer = Streamer::empty(shared());
        for at in around((50, 39), LOAD_RADIUS) {
            streamer.loaded.insert(at, Vec::new());
        }
        let start = iso::ground(50.5 * SIDE as f32, 39.5 * SIDE as f32);
        assert!(streamer.settled(start));
        // Teleported: nothing builds yet, but the sectors of the new place are not in.
        let away = iso::ground(45.5 * SIDE as f32, 52.5 * SIDE as f32);
        assert!(!streamer.settled(away));
        for at in around((45, 52), LOAD_RADIUS) {
            streamer.missing.insert(at);
        }
        assert!(streamer.settled(away));
    }

    #[test]
    fn a_hero_that_arrives_waits_for_the_sectors_in_view() {
        AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::new);
        ComputeTaskPool::get_or_init(bevy::tasks::TaskPool::new);
        let at = |x: f32, y: f32| iso::ground(x * SIDE as f32, y * SIDE as f32);
        // The view takes in the neighbours across the edges of the quarter the hero stands in.
        let corner = at(45.9, 52.2);
        assert_eq!(
            in_view(corner).collect::<HashSet<_>>(),
            HashSet::from([(45, 51), (46, 51), (45, 52), (46, 52)])
        );
        let mut streamer = Streamer::empty(shared());
        // One building in the background already, from an earlier frame.
        streamer.start((46, 51));
        assert_eq!(streamer.complete(corner), 4);
        // The grid of the test has no sectors: each is known to be missing as the frame goes on,
        // the rest of the 3 × 3 not yet asked for.
        assert!(streamer.building.is_empty());
        assert_eq!(streamer.missing.len(), 4);
        assert_eq!(streamer.complete(corner), 0);
        // At the grid's corner, the one that the grid has.
        assert_eq!(streamer.complete(at(0.1, 0.1)), 1);
    }
}
