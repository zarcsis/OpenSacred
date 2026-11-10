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
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on};
use opensacred_data::world::{SIDE, Sector, Sprites, World};

use crate::frame::Step;
use crate::game::AppState;
use crate::hero::{Body, Hero};
use crate::liquids::Liquids;
use crate::parallel::{self, Spread};
use crate::sheet_pool::SheetPool;
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
/// How far what a sector draws reaches past its cells' diamonds on the original's screen at zoom
/// 1, up, down, left and right, where the world's index does not tell (`SectorEntry::reach`,
/// written before #1656): the most of the 6,050 sectors.
const MOST_REACH: [u16; 4] = [2_034, 841, 1_139, 1_080];
/// The bytes of a sector's textures that go ahead of it in a frame, and one texture more, while
/// it comes in out of view ([`Built::stage`]): an overworld sector brings 30 to 110 MB of its
/// ground's atlas and its sprites' pages, which made the frame it came in some 40 to 70 ms when
/// they all went at once (#1651).
const UPLOAD_PER_FRAME: usize = 8 << 20;

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

impl Built {
    /// Sends the sector's next tile sheets to the pool and adds its next textures to `images`,
    /// ahead of its entities ([`stage`]); answers whether any was left, false once they are all in.
    fn stage(&mut self, images: &mut Assets<Image>, pool: &mut SheetPool) -> bool {
        let (bytes, taken) = self.ground.stage(pool, UPLOAD_PER_FRAME);
        if bytes >= UPLOAD_PER_FRAME {
            return true;
        }
        stage(self.statics.staged(), images, taken.then_some(bytes))
    }
}

/// Adds the next of `textures` not added yet to `images`, [`UPLOAD_PER_FRAME`] bytes of them and
/// one more, counting the bytes `added` this frame already, if any; answers whether any was left
/// to add.
fn stage<'a>(
    textures: impl Iterator<Item = &'a mut Staged>,
    images: &mut Assets<Image>,
    mut added: Option<usize>,
) -> bool {
    for texture in textures {
        if added.is_some_and(|added| added >= UPLOAD_PER_FRAME) {
            break;
        }
        if let Staged::Waiting(_) = texture {
            *added.get_or_insert(0) += texture.add(images);
        }
    }
    added.is_some()
}

/// A sector's texture, added to the assets ahead of its sector's entities ([`Built::stage`]), or
/// not yet: the render world uploads what is added in the frame it is added.
pub enum Staged {
    Waiting(Image),
    Added(Handle<Image>),
}

impl Staged {
    /// Adds it to `images` unless it is already; answers the bytes it added.
    fn add(&mut self, images: &mut Assets<Image>) -> usize {
        let Self::Waiting(image) = self else {
            return 0;
        };
        let bytes = image.data.as_ref().map_or(0, Vec::len);
        *self = Self::Added(images.add(std::mem::take(image)));
        bytes
    }

    /// Its handle, added to `images` now if it was not before.
    pub fn handle(self, images: &mut Assets<Image>) -> Handle<Image> {
        match self {
            Self::Waiting(image) => images.add(image),
            Self::Added(handle) => handle,
        }
    }
}

/// Builds sector `at`, spread as `spread` says: on its thread alone in the background, its
/// ground and its statics at once over the pool of the frame's wait while a frame waits for it
/// ([`parallel`]).
pub fn build(
    shared: &Shared,
    at: (u32, u32),
    spread: Spread,
    decoded: &terrain::Decoded,
) -> Building {
    let Some(sector) = terrain::read_sector(&shared.assets, &shared.world, at)? else {
        return Ok(None);
    };
    let (ground, statics) = parallel::join(
        spread,
        || terrain::build(&shared.assets, &shared.world, &sector, spread, decoded),
        || {
            statics::build(
                &shared.assets,
                &shared.world,
                &shared.sprites,
                shared.shadows.as_deref(),
                &sector,
                spread,
            )
        },
    );
    let liquids = liquids::build(&sector);
    Ok(Some(Built {
        sector,
        ground: ground?,
        liquids,
        statics: statics?,
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

    /// How far what sector `at` draws reaches past its cells, as the world's index tells, else
    /// [`MOST_REACH`].
    fn reach(&self, at: (u32, u32)) -> [u16; 4] {
        self.shared
            .world
            .sectors
            .iter()
            .find(|entry| (entry.x, entry.y) == at)
            .and_then(|entry| entry.reach)
            .unwrap_or(MOST_REACH)
    }

    /// Whether sector `at` is in place: spawned, or known to be missing.
    fn holds(&self, at: (u32, u32)) -> bool {
        self.loaded.contains_key(&at) || self.missing.contains(&at)
    }

    /// Starts building sector `at` in the background.
    fn start(&mut self, at: (u32, u32)) {
        let shared = Arc::clone(&self.shared);
        let task = AsyncComputeTaskPool::get().spawn(async move {
            let decoded = terrain::Decoded::default();
            build(&shared, at, Spread::Alone, &decoded)
        });
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

    /// Builds every sector that `view` about world point `hero` shows ([`in_view`]) that is neither
    /// in place nor ready, all at once, and waits for them, those building already among them; answers how
    /// many it waited for. The original's view takes the 3 × 3 about its centre whenever the
    /// centre changes (`0x62A5C0`) through its sector cache (`0x634340`), which reads a sector not
    /// in memory on the spot (`0x637790`), so that its frame waits for them; here a sector's
    /// ground is a picture to upload, and those out of view follow in the background. Those not
    /// building yet are built at once over the pool of the frame's wait ([`parallel`]).
    fn complete(&mut self, hero: Vec3, view: View) -> usize {
        let wanted: Vec<_> = in_view(hero, view, |at| self.reach(at))
            .filter(|&at| !self.holds(at) && !self.ready.iter().any(|(ready, _)| *ready == at))
            .collect();
        let new: Vec<_> = wanted
            .iter()
            .filter(|at| !self.building.contains_key(at))
            .copied()
            .collect();
        let shared = &*self.shared;
        let decoded = terrain::Decoded::default();
        let built = parallel::map(new, Spread::Wide, |at| {
            (at, build(shared, at, Spread::Wide, &decoded))
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

/// Despawns every sector of the game left, with what they adopted, and gives back the tile sheets
/// that those built and not spawned took of the pool; the sectors being built go with the
/// streamer (`game`).
pub fn leave(
    mut commands: Commands,
    streamer: Option<Res<Streamer>>,
    pool: Option<ResMut<SheetPool>>,
) {
    for entity in streamer.iter().flat_map(|s| s.loaded.values().flatten()) {
        commands.entity(*entity).try_despawn();
    }
    if let (Some(streamer), Some(mut pool)) = (streamer, pool) {
        for (_, built) in &streamer.ready {
            for &sheet in built.ground.taken() {
                pool.release(sheet);
            }
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

/// What the world's camera shows about the point it looks at: half its width and half its
/// height on the original's screen at zoom 1 (`iso`), `384·z` high at the zoom `z` and as wide as
/// the window's proportions make it; at the widest zoom on a 16:9 screen 1,365 by 768.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View(Vec2);

impl View {
    /// The view of the camera's `zoom` (its projection's scale) on a window `aspect` times as
    /// wide as it is high.
    pub fn of(zoom: f32, aspect: f32) -> Self {
        let half = iso::SCREEN_HEIGHT / 2.0 * zoom;
        Self(Vec2::new(half * aspect, half))
    }

    /// The widest view, at the widest zoom on a 16:9 window, while no camera tells.
    fn widest() -> Self {
        Self::of(*iso::ZOOM.end(), 16.0 / 9.0)
    }
}

/// Where world point `p` shows on the original's screen at zoom 1, in pixels right and down,
/// the ground's origin at 0: `48 (cx − cy)`, `24 (cx + cy)` for the ground's point of the global
/// cell `(cx, cy)`, a point above the ground higher by its height.
fn on_screen(p: Vec3) -> Vec2 {
    Vec2::new(p.dot(iso::right()), -p.dot(iso::up())) * iso::PIXELS_PER_UNIT
}

/// Whether sector `at`'s cells' diamonds, widened by `reach` (up, down, left and right), meet the
/// rectangle of `view` about the screen's point `centre`: the two convex shapes apart along none
/// of their edges' normals, the screen's axes and the diamonds' two.
fn meets(at: (u32, u32), reach: [u16; 4], centre: Vec2, view: View) -> bool {
    let side = SIDE as f32;
    let (x, y) = (at.0 as f32 * side, at.1 as f32 * side);
    let diamond = [(x, y), (x + side, y), (x + side, y + side), (x, y + side)]
        .map(|(cx, cy)| on_screen(iso::ground(cx, cy)));
    let [up, down, left, right] = reach.map(f32::from);
    let widened = [(-left, -up), (right, -up), (right, down), (-left, down)].map(Vec2::from);
    let half = view.0;
    let rectangle = [
        centre - half,
        centre + Vec2::new(half.x, -half.y),
        centre + half,
        centre + Vec2::new(-half.x, half.y),
    ];
    let span = |points: &[Vec2], axis: Vec2| {
        points
            .iter()
            .map(|point| point.dot(axis))
            .fold((f32::MAX, f32::MIN), |(low, high), v| {
                (low.min(v), high.max(v))
            })
    };
    [Vec2::X, Vec2::Y, Vec2::new(1.0, 2.0), Vec2::new(1.0, -2.0)]
        .into_iter()
        .all(|axis| {
            let (low, high) = span(&diamond, axis);
            let (wide_low, wide_high) = span(&widened, axis);
            let (seen_low, seen_high) = span(&rectangle, axis);
            low + wide_low <= seen_high && seen_low <= high + wide_high
        })
}

/// The sectors of the 3 × 3 about world point `p` that a view of `view` about it shows, by how
/// far what each draws reaches past its cells (`reach`, #1656): at the original's zoom 1 some
/// 2.4 an arrival on average, against 3.5 within half a sector along both axes of the ground
/// before, and at the widest zoom 3.6 with the tall sprites of a sector below the screen's edge.
/// The rest of the 3 × 3 follows in the background, out of view.
fn in_view(
    p: Vec3,
    view: View,
    reach: impl Fn((u32, u32)) -> [u16; 4],
) -> impl Iterator<Item = (u32, u32)> {
    let centre = on_screen(p);
    around(sector_of(p), LOAD_RADIUS).filter(move |&at| meets(at, reach(at), centre, view))
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
    (mut grounds, mut pool): (ResMut<Assets<terrain::GroundMaterial>>, ResMut<SheetPool>),
    (light_types, fall_types): (
        Res<daynight::LightTypes>,
        Option<Res<crate::waterfalls::Types>>,
    ),
    glow: Option<Res<statics::Glow>>,
    shadows: Option<Res<statics::StaticShadows>>,
    (fade, cameras, windows): (
        Res<statics::Fade>,
        Query<&Projection, iso::Eye>,
        Query<&Window>,
    ),
) {
    let streamer = &mut *streamer;
    // What the world's camera shows, its zoom on the window's proportions.
    let zoom = cameras.iter().find_map(|projection| match projection {
        Projection::Orthographic(ortho) => Some(ortho.scale),
        _ => None,
    });
    let aspect = windows
        .iter()
        .next()
        .map(|window| window.width() / window.height().max(1.0));
    let view = match (zoom, aspect) {
        (Some(zoom), Some(aspect)) => View::of(zoom, aspect),
        _ => View::widest(),
    };
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
    let whole =
        arriving || in_view(point, view, |at| streamer.reach(at)).any(|at| !streamer.holds(at));
    if whole {
        let started = Instant::now();
        let waited = streamer.complete(point, view);
        if waited > 0 {
            info!(
                "sectors: {waited} in view about {},{} built at once in {} ms",
                here.0,
                here.1,
                started.elapsed().as_millis()
            );
        }
    }
    // Those in view at once. The rest come out of view one at a time, in the order they were
    // built: the first sends its textures ahead of it over the frames, and comes in the frame
    // after its last (#1651).
    let mut spawning = Vec::new();
    if whole {
        let view: HashSet<(u32, u32)> = in_view(point, view, |at| streamer.reach(at)).collect();
        let (now, later) = std::mem::take(&mut streamer.ready)
            .into_iter()
            .partition(|(at, _)| view.contains(at));
        spawning = now;
        streamer.ready = later;
    }
    if let Some((_, built)) = streamer.ready.first_mut()
        && !built.stage(&mut images, &mut pool)
    {
        spawning.push(streamer.ready.remove(0));
    }
    for (at, mut built) in spawning {
        info!(
            "sector {},{}: {} static objects, {} with a sprite; the pool holds {} tile sheets",
            at.0,
            at.1,
            built.statics.objects,
            built.statics.drawn,
            pool.held()
        );
        heights.add(&built.sector);
        let lights = daynight::sources(&built.sector, &light_types);
        let falls = fall_types
            .as_deref()
            .map(|types| crate::waterfalls::sources(&built.sector, types))
            .unwrap_or_default();
        let mut entities = terrain::spawn(
            &mut commands,
            built.ground,
            &mut pool,
            &mut images,
            &mut meshes,
            &mut grounds,
        )
        .unwrap_or_else(|e| {
            error!("sector {},{}: its ground: {e:#}", at.0, at.1);
            Vec::new()
        });
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
        entities.extend(falls.into_iter().map(|position| {
            commands
                .spawn((
                    Transform::from_translation(position),
                    crate::waterfalls::Fall,
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
    fn a_hero_that_arrives_waits_for_the_sectors_its_view_shows() {
        AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::new);
        let at = |x: f32, y: f32| iso::ground(x * SIDE as f32, y * SIDE as f32);
        let seen = |p, view, reach: &dyn Fn((u32, u32)) -> [u16; 4]| {
            let mut seen: Vec<_> = in_view(p, view, reach).collect();
            seen.sort_unstable();
            seen
        };
        let none = |_| [0; 4];
        let close = View::of(1.0, 16.0 / 9.0);
        // In the middle of a sector at zoom 1, the view shows it alone.
        let middle = at(45.5, 52.5);
        assert_eq!(seen(middle, close, &none), [(45, 52)]);
        // The sector below it on screen comes in by sprites that reach 1,200 pixels up: its
        // diamond's top lies 1,536 below the middle, the view's edge 384.
        let tall = |at| {
            if at == (46, 53) {
                [1_200, 0, 0, 0]
            } else {
                [0; 4]
            }
        };
        assert_eq!(seen(middle, close, &tall), [(45, 52), (46, 53)]);
        // 1,100 do not reach.
        let short = |at| {
            if at == (46, 53) {
                [1_100, 0, 0, 0]
            } else {
                [0; 4]
            }
        };
        assert_eq!(seen(middle, close, &short), [(45, 52)]);
        // The widest view near a corner takes in the neighbours across it.
        let corner = at(45.9, 52.2);
        let widest = View::widest();
        assert_eq!(
            seen(corner, widest, &none),
            [(45, 51), (45, 52), (46, 51), (46, 52)]
        );
        let mut streamer = Streamer::empty(shared());
        // One building in the background already, from an earlier frame.
        streamer.start((46, 51));
        // The grid of the test has no sectors and its index no reach: each takes the most.
        let wanted = in_view(corner, close, |at| streamer.reach(at)).count();
        assert_eq!(streamer.complete(corner, close), wanted);
        // Each is known to be missing as the frame goes on, the rest of the 3 × 3 not yet asked
        // for.
        assert!(streamer.building.is_empty());
        assert_eq!(streamer.missing.len(), wanted);
        assert_eq!(streamer.complete(corner, close), 0);
        // At the grid's corner, those that the grid has: its own and the two beside it on
        // screen, which the most reach takes in.
        assert_eq!(streamer.complete(at(0.1, 0.1), close), 3);
    }

    #[test]
    fn a_sectors_textures_go_ahead_of_it_a_frames_share_at_a_time() {
        let texture = |bytes: usize| {
            Staged::Waiting(Image {
                data: Some(vec![0; bytes]),
                ..Image::default()
            })
        };
        let mib = 1 << 20;
        let mut textures = vec![
            texture(6 * mib),
            texture(4 * mib),
            texture(4 * mib),
            texture(1),
        ];
        let mut images = Assets::<Image>::default();
        let added = |textures: &[Staged]| {
            textures
                .iter()
                .filter(|texture| matches!(texture, Staged::Added(_)))
                .count()
        };
        // The first frame adds 10 MiB, the one that passes the 8 crossing it.
        assert!(stage(textures.iter_mut(), &mut images, None));
        assert_eq!(added(&textures), 2);
        assert!(stage(textures.iter_mut(), &mut images, None));
        assert_eq!(added(&textures), 4);
        // All in: the sector comes in.
        assert!(!stage(textures.iter_mut(), &mut images, None));
        assert_eq!(images.len(), 4);
        // Its entities take the handles that went ahead.
        let first = textures.swap_remove(0);
        let Staged::Added(handle) = &first else {
            panic!("the first texture went ahead");
        };
        let handle = handle.clone();
        assert_eq!(first.handle(&mut images), handle);
        assert_eq!(images.len(), 4);
    }
}
