//! The hidden stashes (#1518; docs/re/containers.md, "The hidden stashes"): the places that the
//! scripts' `MachVersteck`s lay (`scripts::Live::stashes`) stand in the world as stashes, at their
//! cells' centres; the cursor shows the hand and the stash's text over one (`cursors`), a left
//! click sends the hero there, and as it stands near (`0x53BA29`) the stash's function runs
//! (`VERSTECK`) and the stash is gone, searched for good. The function's `CreateObj` of `0x54`
//! and no type makes a random item for the hero (`0x4863D4`, `opensacred_rules::drops::
//! script_item`), which this module makes and throws to the hero's feet. Each stash blinks
//! (#1519, `0x40A230`): for the first 1,024 milliseconds of each 8,192 of the game's clock, all
//! together, a square of `PARTICLE_SPARK07` grows from its cell's centre on its ground (#1788) and
//! shrinks again as it turns, tinted `0xBB9360`, and a grey one of half its side over it, both
//! added to the frame, at its cell's last place in the original's order of drawing, after the
//! cell's statics and objects (`0x62CF12`, `fx::Orders::after_statics`, #2220).
//!
//! The scripts' `MouseEvent` records (#1584, `0x4A6950`) lay events of the same kind, which stand
//! and are used the same way: the well, the gravestones, the stones and the like. An event loses
//! a use as its function runs, and leaves the world with none left: a stash and an event of
//! `0x66` (the well) have one, the others never run out; those of the flags 1, every `MouseEvent`
//! of `0x10`, do not blink, and those of neither flag show sparks in the blink's place
//! (`0x409D00`, `glitter`).
//!
//! An event of a box (`0x0C`, `0x0D`; #1586) is reached by it alone: its box about its cell's
//! point on the screen, in the original's pixels at the zoom 1 and smaller as the view shows more,
//! goes into the list that the cursor picks from among the items (`0x62CF20`, [`box_rect`],
//! `under_cursor`); one without is found where the cursor picks nothing else by its cell, the cell
//! under the cursor's point on the plane of the height 0 whatever the ground there, on the hero's
//! own floor alone (`0x612909`, `0x617A5B`, `0x634C50`; docs/re/containers.md, "The mark of an
//! event's cell", #3089), so that one on a building's floor is found far below its drawn floor,
//! from that floor; an event on a floor neither blinks, nor sparks, nor lays its box.
//!
//! A click sends the hero by a way to the event's point, on the event's floor, unless its cell
//! lies less than 2 from the event's (`0x53B7A6`–`0x53B945`, [`event_point`], [`walks_to`], #3839;
//! docs/re/containers.md, "The walk"), in place of the cell that the click picks; as it arrives,
//! wherever its walk ended, it searches the event only where it reaches the event's point by a
//! straight way, or cramped from near it (`0x53BB60`, [`reaches`], #3086).

use std::collections::HashSet;

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::drops::ScriptItem;
use opensacred_rules::quests::Look;

use crate::attack::{self, Aim};
use crate::frame::{Control, Playing, Step, Unpaused};
use crate::fx::{Orders, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::hero::{self, Body};
use crate::hud::Pointer;
use crate::iso;
use crate::scripts::Live;
use crate::terrain::Heights;

/// How near the event's cell, in cells, the hero's stands for it to search the event without a
/// walk (`0x53B7A6`: the distance between the cells, its whole square root rounded, below 2).
const NO_WALK_WITHIN: i32 = 2;
/// The function that runs as the hero arrives wherever it stands, its name compared case aside
/// (`0x53BBC5`; the air's element of the ancestors' forge).
const ANYWHERE: &str = "NimmtElementLuft";
/// How near the event's point the hero reaches it without a straight way, in world units
/// (`0x52E460`: 129, the distance's whole square root rounded).
const CRAMPED_WITHIN: f32 = 129.5;
/// The cells about the hero's that `0x52E460` tests whether they may be walked on, as its scan
/// adds each offset of the 3 × 3 to the one before rather than to the hero's cell (**confirmed** by
/// code, a quirk): four cells up and left of the hero's, each twice, and its own last.
const CRAMPED_CELLS: [(i32, i32); 9] = [
    (-1, -1),
    (-1, -2),
    (0, -3),
    (-1, -3),
    (-1, -3),
    (0, -3),
    (-1, -2),
    (-1, -1),
    (0, 0),
];

/// The point of an event at `cell` of the box `boxed`, in world units on the ground
/// (`0x53BA9E`–`0x53BB15`): its cell's centre, whole, moved along both axes by 3/2 of the box's
/// fourth number, toward 0, where the box's second or fourth number is not 0.
fn event_point(cell: (i32, i32), boxed: Option<[i16; 4]>) -> Vec2 {
    let shift = boxed
        .filter(|boxed| boxed[1] != 0 || boxed[3] != 0)
        .map_or(0.0, |boxed| (f32::from(boxed[3]) * 1.5).trunc());
    crate::paths::centre(cell).map(f32::trunc) + Vec2::splat(shift)
}

/// Whether the hero standing at `hero` reaches the event at `point` as it arrives (`0x53BB60`):
/// by a straight way of walkable cells to it (`0x4FCC80`, `paths::straight`), else from within
/// [`CRAMPED_WITHIN`] where at most one of the cells of [`CRAMPED_CELLS`] about the hero's may be
/// walked on (`0x52E460`; #3086).
fn reaches(hero: Vec2, point: Vec2, walkable: impl Fn(crate::paths::Cell) -> bool) -> bool {
    if crate::paths::straight(hero, point, &walkable) {
        return true;
    }
    let (x, y) = crate::paths::cell_of(hero.x, hero.y);
    (point - hero).length() < CRAMPED_WITHIN
        && CRAMPED_CELLS
            .iter()
            .filter(|&&(dx, dy)| walkable((x + dx, y + dy)))
            .count()
            <= 1
}

/// How high above its cell's ground a stash is picked, in world units (**estimate**: about the
/// middle of a bush).
pub(crate) const PICKED_ABOVE: f32 = 40.0;
/// How far about the hero a random item lands, along each axis (as a container's, `R(20, 50)`).
const LANDS_FROM: u32 = 20;
const LANDS_TO: u32 = 50;
/// The blink (`0x40A230`): its texture (the glitter's grey one, `0x401030`'s `+0xC0`), its period
/// and its length in milliseconds of the game's clock, its square's largest half side in the
/// glitter's units of two world units (`glitter`), the square's tint and the grey of the half
/// square over it.
const BLINK_TEXTURE: &str = "particle_spark07";
const BLINK_PERIOD: u128 = 8192;
const BLINK_LENGTH: u128 = 1024;
const BLINK_HALF: f32 = 10.0;
const BLINK_SPACE: f32 = 2.0;
const BLINK_TINT: [f32; 4] = [187.0 / 255.0, 147.0 / 255.0, 96.0 / 255.0, 1.0];
const BLINK_GREY: [f32; 4] = [187.0 / 255.0, 187.0 / 255.0, 187.0 / 255.0, 1.0];
/// How far toward the camera the blink is drawn, along its line of sight, in world units, which
/// leaves it where it shows: in front of its cell's place in the order, whatever its cell's
/// ground, so that every corner is laid back at that place ([`Quads::in_order`]); the farthest
/// corner below it, `10 · 2 · √2` units down the screen, lies `√3` times that in front of the
/// ground.
const BLINK_TOWARD: f32 = 49.0;

/// A hidden stash or a mouse event in the world: its cell, the name of the text the cursor shows
/// over it, and how it shows.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct Stash {
    pub cell: (i32, i32),
    /// Its floor, 0 the ground's, from which alone it is found (`opensacred_rules::quests::Stash`).
    pub floor: i32,
    pub text: String,
    pub look: Look,
    /// Its box, by which alone it is reached (`opensacred_rules::quests::Stash::boxed`).
    pub boxed: Option<[i16; 4]>,
}

/// The rectangle of an event of the box `boxed` about the point `point` of its cell, in the
/// original's pixels on the screen, at the view's zoom `zoom` (2 shows twice the world), which the
/// world view lays into the list that the mouse picks from, its id with the bit `0x80000000`
/// (`0x62CF20`): the box's corners in pixels at the zoom 1, edges in (**seen**: the well at the
/// Seraphim's start, its box (−53, −47) to (52, 50), the hand from 53 left to 52 right of its
/// cell's point and from 47 above to 50 below, not a pixel beyond).
pub(crate) fn box_rect(point: Vec2, boxed: [i16; 4], zoom: f32) -> opensacred_rules::pick::Rect {
    let [x0, y0, x1, y1] = boxed.map(f32::from);
    let low = point + Vec2::new(x0, y0) / zoom;
    let high = point + Vec2::new(x1, y1) / zoom;
    opensacred_rules::pick::Rect::between(low.to_array(), high.to_array())
}

/// One random item that a `CreateObj` of `0x54` and no type makes for the hero (`scripts`).
#[derive(Message, Clone, Copy, Debug)]
pub struct RandomItem;

/// The stash that the hero was sent to by a click, which it searches once its walk there ends.
#[derive(Component)]
pub(crate) struct Searches(Entity);

/// The hero's search of a hidden stash as a save keeps it (#3763): the stash by its name.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedSearches {
    #[serde(with = "crate::names::required")]
    target: Entity,
}

impl bevy::ecs::entity::MapEntities for SavedSearches {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.target = mapper.get_mapped(self.target);
    }
}

impl crate::doings::Saves for Searches {
    type Saved = SavedSearches;

    fn saved(&self, _: &crate::doings::Moment) -> Option<SavedSearches> {
        Some(SavedSearches { target: self.0 })
    }

    fn again(saved: SavedSearches, _: &crate::doings::Moment) -> Option<Self> {
        Some(Self(crate::names::found(Some(saved.target))?))
    }
}

/// The cell under the cursor's point on the plane of the height 0, whatever the ground's height
/// there (the view's `0x629F30`, made a cell by `0x6224E0`; docs/re/containers.md, "The mark of an
/// event's cell"): none for a line of sight that never meets it.
pub(crate) fn cell_at_zero(ray: bevy::shape::Ray3d) -> Option<(i32, i32)> {
    let down = ray.direction.y;
    if down.abs() < f32::EPSILON {
        return None;
    }
    let point = ray.origin - ray.direction.as_vec3() * (ray.origin.y / down);
    Some(crate::paths::cell_of(point.x, point.z))
}

impl Stash {
    /// Whether the cursor and the click find it without a box on the cell `cell` from the floor
    /// `floor`, the hero's (`0x612909`, `0x617A5B`): its own cell, its own floor.
    pub(crate) fn found(&self, cell: (i32, i32), floor: i32) -> bool {
        self.boxed.is_none() && self.cell == cell && self.floor == floor
    }
}

/// The floor that the hero at `level` stands on, 0 the ground's, as the events count it (its byte
/// `+0x24`).
pub(crate) fn floor_of(heights: &Heights, level: crate::terrain::Level) -> i32 {
    i32::try_from(heights.floor_number(level)).unwrap_or(0)
}

/// The stashes and the events in the world kept in step with the scripts' (`MachVersteck`,
/// `MouseEvent`, their uses): one made for each new event at its cell's centre on the ground,
/// one taken away for each gone.
fn place(
    mut commands: Commands,
    live: Option<NonSend<Live>>,
    heights: Res<Heights>,
    stashes: Query<(Entity, &Stash)>,
) {
    let Some(live) = live else {
        return;
    };
    let listed = live.stashes();
    let places: HashSet<((i32, i32), i32)> = listed
        .iter()
        .map(|listed| (listed.cell, listed.floor))
        .collect();
    let mut standing = HashSet::new();
    for (entity, stash) in &stashes {
        if places.contains(&(stash.cell, stash.floor)) {
            standing.insert((stash.cell, stash.floor));
        } else {
            commands.entity(entity).despawn();
        }
    }
    for listed in listed {
        if standing.contains(&(listed.cell, listed.floor)) {
            continue;
        }
        let point = crate::paths::centre(listed.cell);
        let Some(ground) = heights.at(point.x, point.y) else {
            continue;
        };
        standing.insert((listed.cell, listed.floor));
        commands.spawn((
            Stash {
                cell: listed.cell,
                floor: listed.floor,
                text: listed.text.clone(),
                look: listed.look,
                boxed: listed.boxed,
            },
            Transform::from_xyz(point.x, ground + PICKED_ABOVE, point.y),
            Visibility::Hidden,
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// On a press of the left button that went to no attack, the event of a box that the cursor
/// picks (`under_cursor`), else, where it picks nothing, the event without one of the cell under
/// the cursor at the height 0 on the hero's floor ([`cell_at_zero`], [`Stash::found`]) becomes the
/// hero's to search (`0x617A5B`), and the hero goes by a way to the event's point on the event's
/// floor ([`event_point`]), or stands where it is with its cell less than 2 from the event's
/// ([`walks_to`]), in place of the click's cell (`hero::steer`; `0x53B7A6`–`0x53B945`); any press
/// ends the last such order.
fn pick(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, under): (Res<Pointer>, Res<crate::under_cursor::UnderCursor>),
    aim: Res<Aim>,
    camera: Single<(&Camera, &GlobalTransform), crate::iso::Eye<With<Camera>>>,
    stashes: Query<(Entity, &Stash)>,
    (mut heroes, heights): (Query<(Entity, &Transform, &mut Body)>, Res<Heights>),
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    for (hero, ..) in &heroes {
        commands.entity(hero).remove::<Searches>();
    }
    let Some(cursor) = pointer.0 else {
        return;
    };
    let (camera, view) = *camera;
    let (false, Ok(ray)) = (aim.0, camera.viewport_to_world(view, cursor)) else {
        return;
    };
    let boxed = under.0.filter(|&entity| {
        stashes
            .get(entity)
            .is_ok_and(|(_, stash)| stash.boxed.is_some())
    });
    let unboxed = || {
        let cell = cell_at_zero(ray)?;
        let (_, _, body) = heroes.iter().next()?;
        let floor = floor_of(&heights, body.level());
        stashes
            .iter()
            .find(|(_, stash)| stash.found(cell, floor))
            .map(|(entity, _)| entity)
    };
    let Some((stash, event)) = boxed
        .or_else(|| under.0.is_none().then(unboxed).flatten())
        .and_then(|entity| stashes.get(entity).ok())
    else {
        return;
    };
    let point = event_point(event.cell, event.boxed);
    for (hero, transform, mut body) in &mut heroes {
        let here = transform.translation;
        debug!("stashes: the hero goes to search {stash} from {point}");
        if walks_to(crate::paths::cell_of(here.x, here.z), event.cell) {
            // On the event's floor: the ground's, or the hero's own, from which alone one on a
            // building's floor is found.
            let level = if event.floor == 0 { None } else { body.level() };
            body.send_onto(Vec3::new(point.x, here.y, point.y), level);
        } else {
            body.stop();
        }
        commands.entity(hero).insert(Searches(stash));
    }
}

/// Whether the hero, its cell at `hero`, walks to an event at `event` to search it: its cell
/// [`NO_WALK_WITHIN`] cells or more from the event's, the distance's whole square root rounded
/// (`0x53B7A6`).
fn walks_to(hero: (i32, i32), event: (i32, i32)) -> bool {
    let (dx, dy) = (f64::from(hero.0 - event.0), f64::from(hero.1 - event.1));
    (dx * dx + dy * dy).sqrt().round() >= f64::from(NO_WALK_WITHIN)
}

/// The hero that stands, once its way to a stash it was sent to ends, wherever it ends (the
/// original's arrival tests no distance, `0x53BA29`): it searches the stash where it reaches the
/// event's point ([`reaches`]) or the event's function runs anywhere ([`ANYWHERE`]): the stash
/// leaves the scripts' list and its function runs (`scripts::run_function`); out of its reach it
/// gives up, the stash kept (#3086).
fn search(
    mut commands: Commands,
    heights: Res<Heights>,
    stashes: Query<&Stash>,
    heroes: Query<(Entity, &Transform, &Body, &Searches)>,
) {
    for (hero, transform, body, searches) in &heroes {
        // Its walk there, ordered for the world's next tick or begun (`hero::Body`).
        if body.target().is_some() || body.ordered() {
            continue;
        }
        commands.entity(hero).remove::<Searches>();
        let Ok(stash) = stashes.get(searches.0) else {
            continue;
        };
        let cell = stash.cell;
        let level = body.level();
        let reached = reaches(
            transform.translation.xz(),
            event_point(cell, stash.boxed),
            |cell| heights.walkable_on(cell, level),
        );
        commands.queue(move |world: &mut World| {
            let anywhere = world.get_non_send::<Live>().is_some_and(|live| {
                live.stashes()
                    .iter()
                    .find(|stash| stash.cell == cell)
                    .is_some_and(|stash| stash.function.eq_ignore_ascii_case(ANYWHERE))
            });
            if !reached && !anywhere {
                info!(
                    "stashes: the hero does not reach the stash at {cell:?} from where it stands"
                );
                return;
            }
            let function = world
                .get_non_send_mut::<Live>()
                .and_then(|mut live| live.search_stash(cell))
                .map(|stash| stash.function);
            let Some(function) = function else {
                return;
            };
            info!("stashes: the hero searches the stash at {cell:?}, which runs {function}");
            match crate::scripts::run_function(world, &function) {
                Ok(lines) => {
                    for line in lines {
                        debug!("stashes: {line}");
                    }
                }
                Err(e) => warn!("stashes: {e}"),
            }
        });
    }
}

/// Each random item of a `CreateObj` (`0x4863D4`): a potion, or a type chosen as the rule asks
/// (a weapon's once more where none fits), made and thrown to the hero's feet.
fn give(
    mut commands: Commands,
    mut made: MessageReader<RandomItem>,
    (tables, mut dice, assets): (
        Option<Res<crate::drops::Tables>>,
        ResMut<crate::drops::Dice>,
        Res<AssetServer>,
    ),
    (experience, npcs): (
        Res<crate::fight::Experience>,
        Option<Res<crate::npcs::Npcs>>,
    ),
    difficulty: Option<Res<crate::game::Difficulty>>,
    (pack, arts): (Res<crate::pickup::Pack>, Option<Res<crate::arts::HeroArts>>),
    heroes: Query<(&Transform, &crate::fight::Base, &Body)>,
) {
    let count = made.read().count();
    let (Some(tables), Some(npcs), Some((at, base, body))) =
        (tables.as_deref(), npcs.as_deref(), heroes.iter().next())
    else {
        return;
    };
    let level = experience.0.level;
    let difficulty = difficulty.map(|d| d.0).unwrap_or_default();
    let mut hero = crate::drops::hero(base, level);
    crate::drops::carrying(&mut hero, &pack, arts.as_deref(), tables);
    for _ in 0..count {
        let (item, made_at) =
            match opensacred_rules::drops::script_item(level, base.id, difficulty, &mut dice.0) {
                ScriptItem::Potion(potion) => (Some(potion), level),
                ScriptItem::Wanted(wanted) => {
                    let chosen = tables.catalogue().choose(wanted, &hero, &mut dice.0);
                    match chosen {
                        Some(item) => (Some(item), wanted.level),
                        None => {
                            let again = opensacred_rules::drops::script_weapon(
                                level,
                                base.id,
                                difficulty,
                                &mut dice.0,
                            );
                            (
                                tables.catalogue().choose(again, &hero, &mut dice.0),
                                again.level,
                            )
                        }
                    }
                }
            };
        let Some(item) = item else {
            debug!("stashes: no item fits a random item's roll");
            continue;
        };
        // The type's roll's level only chose it: the item is made as any that `CreateObj` lays,
        // at the hero's level within its sector's levels (`0x487AB0`, #1125).
        let level = npcs.script_level(level, at.translation);
        let Some(dropped) =
            crate::drops::scripted(tables, difficulty, (item, level, false), &mut dice.0).or_else(
                || crate::drops::contained(tables, (&mut dice, difficulty), (item, level, 0)),
            )
        else {
            continue;
        };
        let mut aside = || {
            let along = dice.0.below(LANDS_TO - LANDS_FROM + 1) + LANDS_FROM;
            let along = f32::from(u16::try_from(along).unwrap_or(0));
            if dice.0.below(2) == 0 { along } else { -along }
        };
        let to = at.translation + Vec3::new(aside(), 0.0, aside());
        info!("stashes: a random item {item} chosen at {made_at}, made at {level}");
        crate::drops::throw_to(
            &mut commands,
            (&assets, tables),
            (at.translation, to, body.level()),
            dropped,
        );
    }
}

/// The hidden stashes and the mouse events within `radius` cells of the hero along each axis, the
/// nearest first: each its cell, its function, its text's name, its uses left and how it shows
/// (the command `stashes`).
pub(crate) fn near(world: &mut World, radius: u32) -> Result<Vec<String>, String> {
    let mut heroes = world.query_filtered::<&Transform, With<Body>>();
    let hero = heroes
        .iter(world)
        .next()
        .map(|at| crate::paths::cell_of(at.translation.x, at.translation.z))
        .ok_or("no hero")?;
    let live = world
        .get_non_send::<Live>()
        .ok_or("the scripts do not run")?;
    let radius = i32::try_from(radius).unwrap_or(i32::MAX);
    let mut found: Vec<_> = live
        .stashes()
        .iter()
        .filter(|stash| {
            (stash.cell.0 - hero.0).abs() <= radius && (stash.cell.1 - hero.1).abs() <= radius
        })
        .map(|stash| {
            let (dx, dy) = (stash.cell.0 - hero.0, stash.cell.1 - hero.1);
            (dx * dx + dy * dy, stash)
        })
        .collect();
    found.sort_by_key(|&(distance, _)| distance);
    let mut lines = vec![format!(
        "{} hidden stashes and mouse events within {radius} cells of the hero, {} in all",
        found.len(),
        live.stashes().len()
    )];
    lines.extend(found.into_iter().map(|(_, stash)| {
        let uses = if stash.uses >= opensacred_rules::quests::ENDLESS {
            "endless".to_owned()
        } else {
            format!("{} uses", stash.uses)
        };
        format!(
            "cell {},{} runs {} shows {}, {uses}, {:?}",
            stash.cell.0, stash.cell.1, stash.function, stash.text, stash.look
        )
    }));
    Ok(lines)
}

/// The blink's mesh, all the stashes' squares.
#[derive(Resource)]
struct BlinkMesh(Handle<Mesh>);

/// Makes the blink's mesh and its material as a game starts; none where the texture is missing.
fn begin_blink(
    mut commands: Commands,
    data: Option<Res<Data>>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<crate::fx::Effect>>,
) {
    let Some(data) = data else {
        return;
    };
    let Some(image) = particle_texture(&data.0.assets, &assets, BLINK_TEXTURE) else {
        warn!("stashes: no texture {BLINK_TEXTURE}; the stashes do not blink");
        return;
    };
    let mesh = meshes.add(empty());
    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(materials.add(particle_material(image, AlphaMode::Add))),
        Transform::default(),
        NoFrustumCulling,
        RenderLayers::layer(iso::UNTINTED),
        DespawnOnExit(AppState::InGame),
    ));
    commands.insert_resource(BlinkMesh(mesh));
}

/// The blink's half side and its turn at `ms` milliseconds of the game's clock (`0x40A230`): none
/// past the first 1,024 of each 8,192; else `10 sin(πs)` and `2s` radians, `s` the share of the
/// 1,024 passed.
fn blink_at(ms: u128) -> Option<(f32, f32)> {
    let into = ms % BLINK_PERIOD;
    if into >= BLINK_LENGTH {
        return None;
    }
    let s = f32::from(u16::try_from(into).unwrap_or(0)) / 1024.0;
    Some((BLINK_HALF * (std::f32::consts::PI * s).sin(), 2.0 * s))
}

/// Where a stash or an event of the place `at` (`place`) draws its blink or its sparks: its
/// cell's centre on the cell's ground, which the original's flat ground stands at the height 0
/// (`0x62FF60`; #1788), brought toward the camera.
pub(crate) fn drawn_at(at: &Transform) -> Vec3 {
    at.translation - Vec3::Y * PICKED_ABOVE - iso::forward() * BLINK_TOWARD
}

/// Lays every stash's blink out at the frame's time: its square at its cell's centre on its
/// ground, brought toward the camera ([`drawn_at`]), and the grey half square over it, both
/// turned alike, at the depth of its cell's last place in the order: the row walk lays its
/// entry of the kind `0x20` after the cell's statics and objects (`0x62CF12`, #2220), so that it
/// lies over a creature standing in its cell and under what a later cell draws.
fn blink(
    time: Res<Time>,
    mesh: Option<Res<BlinkMesh>>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, Orders),
    stashes: Query<(&Stash, &Transform)>,
) {
    let Some(mesh) = mesh else {
        return;
    };
    let mut quads = Quads::default();
    if let Some((half, angle)) = blink_at(time.elapsed().as_millis()) {
        let whole = (Vec2::ZERO, 1.0);
        // An event on a floor blinks not (`0x62CF4C`, the floor 0's lookup).
        for (_, at) in stashes
            .iter()
            .filter(|(stash, _)| stash.look == Look::Blink && stash.floor == 0)
        {
            let centre = drawn_at(at);
            let mut own = Quads::default();
            own.push(centre, half * BLINK_SPACE, angle, whole, BLINK_TINT);
            own.push(centre, half * 0.5 * BLINK_SPACE, angle, whole, BLINK_GREY);
            own.in_order(orders.after_statics(at.translation), Vec3::ZERO);
            quads.append(own);
        }
    }
    if let Some(mut mesh) = meshes.get_mut(&mesh.0) {
        quads.into_mesh(&mut mesh);
    }
}

/// The hidden stashes.
pub struct StashesPlugin;

impl Plugin for StashesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RandomItem>()
            .add_systems(OnEnter(AppState::InGame), begin_blink)
            .add_systems(Update, blink.in_set(Playing))
            .add_systems(
                Update,
                (
                    place.run_if(in_state(AppState::InGame)),
                    give.run_if(in_state(AppState::InGame)),
                ),
            )
            .add_systems(
                Update,
                (
                    pick.in_set(Control)
                        .in_set(crate::frame::OnWorld)
                        .after(attack::aim)
                        .after(hero::steer),
                    search.after(hero::run),
                )
                    .in_set(Unpaused)
                    .in_set(Step::HeroMove),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hero_walks_to_an_event_from_2_cells_or_more() {
        let event = (3236, 2529);
        // The same cell and a neighbour's, its diagonal's root 1.41 rounded to 1: no walk.
        assert!(!walks_to(event, event));
        assert!(!walks_to((3237, 2530), event));
        // Two cells along an axis, and the knight's move's root 2.24: a walk.
        assert!(walks_to((3238, 2529), event));
        assert!(walks_to((3237, 2531), event));
        // The statue's, from where the maintainer clicked it.
        assert!(walks_to((3240, 2534), event));
    }

    #[test]
    fn an_event_without_a_box_is_found_by_its_cell_at_the_height_0_on_its_floor() {
        // Straight down onto (100, 200): that point's cell, whatever the height it comes from.
        let down = Ray3d::new(Vec3::new(100.0, 500.0, 200.0), Dir3::NEG_Y);
        let cell = crate::paths::cell_of(100.0, 200.0);
        assert_eq!(cell_at_zero(down), Some(cell));
        // Slanting down from 100 up: where it meets the plane, 100 on.
        let slant = Ray3d::new(
            Vec3::new(0.0, 100.0, 50.0),
            Dir3::new(Vec3::new(1.0, -1.0, 0.0)).expect("a direction"),
        );
        assert_eq!(
            cell_at_zero(slant),
            Some(crate::paths::cell_of(100.0, 50.0))
        );
        // Along the plane it never meets it.
        assert_eq!(
            cell_at_zero(Ray3d::new(Vec3::new(0.0, 10.0, 0.0), Dir3::X)),
            None
        );
        // Its own cell from its own floor, without a box.
        let stash = Stash {
            cell,
            floor: 2,
            text: String::new(),
            look: Look::Blink,
            boxed: None,
        };
        assert!(stash.found(cell, 2));
        assert!(!stash.found(cell, 0));
        assert!(!stash.found((cell.0 + 1, cell.1), 2));
        assert!(!stash.found((cell.0, cell.1 + 1), 2));
        let boxed = Stash {
            boxed: Some([-10, -10, 10, 10]),
            ..stash
        };
        assert!(!boxed.found(cell, 2));
    }

    #[test]
    fn an_event_of_a_box_is_under_the_cursor_within_it() {
        // The well at the Seraphim's start, its cell's point at (560, 505): the hand from 507 to
        // 612 across and from 458 to 555 down, as seen in the original.
        let (well, point) = ([-53, -47, 52, 50], Vec2::new(560.0, 505.0));
        let at = |x: f32, y: f32, zoom: f32| box_rect(point, well, zoom).contains([x, y]);
        assert!(at(507.0, 500.0, 1.0) && at(612.0, 500.0, 1.0));
        assert!(!at(505.0, 500.0, 1.0) && !at(614.0, 500.0, 1.0));
        assert!(at(560.0, 458.0, 1.0) && at(560.0, 555.0, 1.0));
        assert!(!at(560.0, 456.0, 1.0) && !at(560.0, 557.0, 1.0));
        // Showing twice the world, the box is half as large on the screen.
        assert!(at(534.0, 500.0, 2.0) && !at(532.0, 500.0, 2.0));
    }

    #[test]
    fn a_blink_lies_on_its_cells_ground() {
        // A stash placed over the ground at 300 draws on it, toward the camera, not at 0.
        let at = Transform::from_xyz(100.0, 300.0 + PICKED_ABOVE, 200.0);
        let drawn = drawn_at(&at);
        let toward = -iso::forward() * BLINK_TOWARD;
        assert!((drawn - Vec3::new(100.0, 300.0, 200.0) - toward).length() < 1e-3);
    }

    #[test]
    fn a_stash_blinks_a_second_in_each_eight() {
        assert_eq!(blink_at(0), Some((0.0, 0.0)));
        let (half, angle) = blink_at(512).unwrap_or_default();
        assert!((half - 10.0).abs() < 1e-5 && (angle - 1.0).abs() < 1e-6);
        assert!(blink_at(1024).is_none());
        assert!(blink_at(8191).is_none());
        let (half, _) = blink_at(8192 + 256).unwrap_or_default();
        assert!((half - 10.0 * std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-4);
    }

    #[test]
    fn a_stash_is_searched_where_the_hero_reaches_its_point() {
        // The well at the Seraphim's start, of the box (−53, −47) to (52, 50): its cell's centre,
        // whole, moved by 75 along both axes; an event of no box its cell's centre (#3086).
        let well = event_point((3252, 2524), Some([-53, -47, 52, 50]));
        let centre = crate::paths::centre((3252, 2524)).map(f32::trunc);
        assert_eq!(well, centre + Vec2::splat(75.0));
        assert_eq!(event_point((3252, 2524), None), centre);
        assert_eq!(event_point((3252, 2524), Some([5, 0, 6, 0])), centre);
        assert_eq!(
            event_point((0, 0), Some([0, 0, 0, -3])),
            crate::paths::centre((0, 0)).map(f32::trunc) - Vec2::splat(4.0)
        );
        // A wall of closed cells between: no straight way; on open ground, reached.
        let point = crate::paths::centre((10, 0));
        let hero = crate::paths::centre((4, 0));
        let open = |_: crate::paths::Cell| true;
        assert!(reaches(hero, point, open));
        let walled = |(x, _): crate::paths::Cell| x != 7;
        assert!(!reaches(hero, point, walled));
        // Within 129 but standing in the open: not reached; cramped, its own cell alone open of
        // the four that the quirk tests, reached.
        let near = crate::paths::centre((8, 0));
        assert!(!reaches(near, point, |(x, _): crate::paths::Cell| x != 9));
        let hero_cell = crate::paths::cell_of(near.x, near.y);
        let cramped = |cell: crate::paths::Cell| cell == hero_cell;
        assert!(reaches(near, point, cramped));
        // Farther than 129, cramped as it may be: not reached.
        assert!(!reaches(hero, point, |cell| cell
            == crate::paths::cell_of(hero.x, hero.y)));
    }
}
