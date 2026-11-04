//! The hidden stashes (#1518; docs/re/containers.md, "The hidden stashes"): the places that the
//! scripts' `MachVersteck`s lay (`scripts::Live::stashes`) stand in the world as stashes, at their
//! cells' centres; the cursor shows the hand and the stash's text over one (`cursors`), a left
//! click sends the hero there, and as it stands near (`0x53BA29`) the stash's function runs
//! (`VERSTECK`) and the stash is gone, searched for good. The function's `CreateObj` of `0x54`
//! and no type makes a random item for the hero (`0x4863D4`, `opensacred_rules::drops::
//! script_item`), which this module makes and throws to the hero's feet. Each stash blinks
//! (#1519, `0x40A230`): for the first 1,024 milliseconds of each 8,192 of the game's clock, all
//! together, a square of `PARTICLE_SPARK07` grows from its cell's centre at the height 0 and
//! shrinks again as it turns, tinted `0xBB9360`, and a grey one of half its side over it, both
//! added to the frame.
//!
//! The scripts' `MouseEvent` records (#1584, `0x4A6950`) lay events of the same kind, which stand
//! and are used the same way: the well, the gravestones, the stones and the like. An event loses
//! a use as its function runs, and leaves the world with none left: a stash and an event of
//! `0x66` (the well) have one, the others never run out; those of the flags 1, every `MouseEvent`
//! of `0x10`, do not blink.
//!
//! An event of a box (`0x0C`, `0x0D`; #1586) is reached by it alone: the cursor in the box about
//! its cell's point on the screen, in the original's pixels at the zoom 1 and smaller as the view
//! shows more (`0x62CF20`, [`BoxedUnder`]); one without is picked as a creature is.
//!
//! Not followed: the static object's mark (the stash stands at its cell's centre on the ground,
//! whatever its floor), the sparks
//! of the events of neither flag (`0x409D00`), the arrival's straight way, the item maker's use of
//! the `CreateObj`'s `0x79`, and the hero's walk of the order 7 (the click's own walk takes it
//! there).

use std::collections::HashSet;

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::drops::ScriptItem;
use opensacred_rules::quests::Look;

use crate::attack::{self, Aim};
use crate::frame::{Control, Playing, Step, Unpaused};
use crate::fx::{Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::hero::{self, Body};
use crate::hud::Pointer;
use crate::iso;
use crate::scripts::Live;
use crate::terrain::Heights;

/// How near a stash the hero searches it, standing, in world units (as a container's, `0x53BE8C`;
/// **hypothesis**: the order 7's reach is not read).
const SEARCHES_WITHIN: f32 = 300.0;
/// How high above its cell's ground a stash is picked, in world units (**estimate**: about the
/// middle of a bush).
const PICKED_ABOVE: f32 = 40.0;
/// How far about the hero a random item lands, along each axis (as a container's, `R(20, 50)`).
const LANDS_FROM: u32 = 20;
const LANDS_TO: u32 = 50;
/// The frames the hero waits for its walk to begin before it searches from where it stands.
const WALK_BEGINS_WITHIN: u8 = 10;
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
/// leaves it where it shows: the original draws it over its 2D ground, which hides none of it,
/// where the game's ground would hide the half below its centre; the farthest corner below it,
/// `10 · 2 · √2` units down the screen, lies `√3` times that in front of the ground.
const BLINK_TOWARD: f32 = 49.0;

/// A hidden stash or a mouse event in the world: its cell, the name of the text the cursor shows
/// over it, and how it shows.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct Stash {
    pub cell: (i32, i32),
    pub text: String,
    pub look: Look,
    /// Its box, by which alone it is reached (`opensacred_rules::quests::Stash::boxed`).
    pub boxed: Option<[i16; 4]>,
}

/// The event of a box under the cursor in this frame, which the cursor and the click take in
/// place of the line of sight's (`0x62CF20`: the world view lays each such event's box at its
/// static's point into the list that the mouse picks from, its id with the bit `0x80000000`).
#[derive(Resource, Default)]
pub(crate) struct BoxedUnder(pub(crate) Option<Entity>);

/// Whether the cursor at `cursor` lies in the box `boxed` about the point `point`, both in the
/// original's pixels on the screen, at the view's zoom `zoom` (2 shows twice the world): the box's
/// corners in pixels at the zoom 1, edges in (**seen**: the well at the Seraphim's start, its box
/// (−53, −47) to (52, 50), the hand from 53 left to 52 right of its cell's point and from 47 above
/// to 50 below, not a pixel beyond).
pub(crate) fn inside(cursor: Vec2, point: Vec2, boxed: [i16; 4], zoom: f32) -> bool {
    let at = (cursor - point) * zoom;
    let [x0, y0, x1, y1] = boxed.map(f32::from);
    (x0..=x1).contains(&at.x) && (y0..=y1).contains(&at.y)
}

/// Finds the event of a box under the cursor, the first of them (`0x62CF20`).
pub(crate) fn boxed_under(
    mut under: ResMut<BoxedUnder>,
    (pointer, ui_scale): (Res<Pointer>, Res<UiScale>),
    camera: Single<(&Camera, &GlobalTransform, &Projection), crate::iso::Eye<With<Camera>>>,
    stashes: Query<(Entity, &GlobalTransform, &Stash)>,
) {
    under.0 = None;
    let Some(cursor) = pointer.0 else {
        return;
    };
    let (camera, view, projection) = *camera;
    let zoom = match projection {
        Projection::Orthographic(ortho) => ortho.scale.max(0.01),
        _ => 1.0,
    };
    let scale = ui_scale.0.max(0.01);
    under.0 = stashes.iter().find_map(|(entity, at, stash)| {
        let boxed = stash.boxed?;
        // Its cell's point on the ground, below where it is placed.
        let ground = at.translation() - Vec3::Y * PICKED_ABOVE;
        let point = camera.world_to_viewport(view, ground).ok()?;
        inside(cursor / scale, point / scale, boxed, zoom).then_some(entity)
    });
}

/// One random item that a `CreateObj` of `0x54` and no type makes for the hero (`scripts`).
#[derive(Message, Clone, Copy, Debug)]
pub struct RandomItem;

/// The stash that the hero was sent to by a click, which it searches as it stands near once its
/// walk there ends, whether the walk began, and the frames it waited for it.
#[derive(Component)]
struct Searches(Entity, bool, u8);

/// How far the cursor's line of sight passes a stash's point, if it picks it: none farther than a
/// creature is picked within (`attack::PICKED_WITHIN`).
pub(crate) fn picked(ray: bevy::shape::Ray3d, at: Vec3) -> Option<f32> {
    let distance = (at - ray.origin).cross(ray.direction.as_vec3()).length();
    (distance <= attack::PICKED_WITHIN).then_some(distance)
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
    let cells: HashSet<(i32, i32)> = listed.iter().map(|listed| listed.cell).collect();
    let mut standing = HashSet::new();
    for (entity, stash) in &stashes {
        if cells.contains(&stash.cell) {
            standing.insert(stash.cell);
        } else {
            commands.entity(entity).despawn();
        }
    }
    for listed in listed {
        if standing.contains(&listed.cell) {
            continue;
        }
        let point = crate::paths::centre(listed.cell);
        let Some(ground) = heights.at(point.x, point.y) else {
            continue;
        };
        standing.insert(listed.cell);
        commands.spawn((
            Stash {
                cell: listed.cell,
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

/// On a press of the left button that went to no attack, the event of a box under the cursor, else
/// the stash without one nearest the cursor's line of sight, if one is near it, becomes the hero's
/// to search (`0x617A5B`); any press ends the last such order.
fn pick(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    pointer: Res<Pointer>,
    aim: Res<Aim>,
    camera: Single<(&Camera, &GlobalTransform), crate::iso::Eye<With<Camera>>>,
    (stashes, boxed): (Query<(Entity, &GlobalTransform, &Stash)>, Res<BoxedUnder>),
    heroes: Query<Entity, With<Body>>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    for hero in &heroes {
        commands.entity(hero).remove::<Searches>();
    }
    let Some(cursor) = pointer.0 else {
        return;
    };
    let (camera, view) = *camera;
    let (false, Ok(ray)) = (aim.0, camera.viewport_to_world(view, cursor)) else {
        return;
    };
    let Some(stash) = boxed.0.or_else(|| {
        stashes
            .iter()
            .filter(|(_, _, stash)| stash.boxed.is_none())
            .filter_map(|(entity, at, _)| Some((entity, picked(ray, at.translation())?)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(entity, _)| entity)
    }) else {
        return;
    };
    for hero in &heroes {
        debug!("stashes: the hero goes to search {stash}");
        commands.entity(hero).insert(Searches(stash, false, 0));
    }
}

/// The hero that stands, once its way to a stash it was sent to ends: within 300 of it it searches
/// it (`0x53BA29`): the stash leaves the scripts' list and its function runs
/// (`scripts::run_function`); farther, it gives up.
fn search(
    mut commands: Commands,
    stashes: Query<(&Transform, &Stash)>,
    mut heroes: Query<(Entity, &Transform, &Body, &mut Searches)>,
) {
    for (hero, transform, body, mut searches) in &mut heroes {
        // Its walk there, ordered for the world's next tick or begun (`hero::Body`).
        if body.target().is_some() || body.ordered() {
            searches.1 = true;
            continue;
        }
        let Ok((at, stash)) = stashes.get(searches.0) else {
            commands.entity(hero).remove::<Searches>();
            continue;
        };
        let near = (at.translation - transform.translation).xz().length() <= SEARCHES_WITHIN;
        if !searches.1 && !near && searches.2 < WALK_BEGINS_WITHIN {
            searches.2 += 1;
            continue;
        }
        commands.entity(hero).remove::<Searches>();
        if !near {
            continue;
        }
        let cell = stash.cell;
        commands.queue(move |world: &mut World| {
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
    experience: Res<crate::fight::Experience>,
    difficulty: Option<Res<crate::game::Difficulty>>,
    (pack, arts): (Res<crate::pickup::Pack>, Option<Res<crate::arts::HeroArts>>),
    heroes: Query<(&Transform, &crate::fight::Base, &Body)>,
) {
    let count = made.read().count();
    let (Some(tables), Some((at, base, body))) = (tables.as_deref(), heroes.iter().next()) else {
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
        let Some(dropped) =
            crate::drops::contained(tables, (&mut dice, difficulty), (item, made_at, 1))
        else {
            continue;
        };
        let mut aside = || {
            let along = dice.0.below(LANDS_TO - LANDS_FROM + 1) + LANDS_FROM;
            let along = f32::from(u16::try_from(along).unwrap_or(0));
            if dice.0.below(2) == 0 { along } else { -along }
        };
        let to = at.translation + Vec3::new(aside(), 0.0, aside());
        info!("stashes: a random item {item} of level {made_at} for the hero");
        crate::drops::throw_to(
            &mut commands,
            (&assets, tables),
            (at.translation, to, body.level()),
            dropped,
        );
    }
}

/// The hidden stashes within `radius` cells of the hero along each axis, the nearest first: each
/// its cell, its function and its text's name (the command `stashes`).
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
/// turned alike.
fn blink(
    time: Res<Time>,
    mesh: Option<Res<BlinkMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    stashes: Query<(&Stash, &Transform)>,
) {
    let Some(mesh) = mesh else {
        return;
    };
    let mut quads = Quads::default();
    if let Some((half, angle)) = blink_at(time.elapsed().as_millis()) {
        let whole = (Vec2::ZERO, 1.0);
        for (_, at) in stashes
            .iter()
            .filter(|(stash, _)| stash.look == Look::Blink)
        {
            let centre = drawn_at(at);
            quads.push(centre, half * BLINK_SPACE, angle, whole, BLINK_TINT);
            quads.push(centre, half * 0.5 * BLINK_SPACE, angle, whole, BLINK_GREY);
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
            .init_resource::<BoxedUnder>()
            .add_systems(OnEnter(AppState::InGame), begin_blink)
            .add_systems(Update, blink.in_set(Playing))
            .add_systems(
                Update,
                (
                    place.run_if(in_state(AppState::InGame)),
                    give.run_if(in_state(AppState::InGame)),
                    boxed_under.run_if(in_state(AppState::InGame)),
                ),
            )
            .add_systems(
                Update,
                (
                    pick.in_set(Control)
                        .after(attack::aim)
                        .after(boxed_under)
                        .before(hero::steer),
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
    fn an_event_of_a_box_is_under_the_cursor_within_it() {
        // The well at the Seraphim's start, its cell's point at (560, 505): the hand from 507 to
        // 612 across and from 458 to 555 down, as seen in the original.
        let (well, point) = ([-53, -47, 52, 50], Vec2::new(560.0, 505.0));
        let at = |x: f32, y: f32, zoom: f32| inside(Vec2::new(x, y), point, well, zoom);
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
}
