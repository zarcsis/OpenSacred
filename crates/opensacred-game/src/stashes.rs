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
//! Not followed: the static object's mark (the stash stands at its cell's centre, picked as a
//! creature is), the item maker's use of the `CreateObj`'s `0x79`, and the hero's walk of the
//! order 7 (the click's own walk takes it there).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::drops::ScriptItem;

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

/// A hidden stash in the world: its cell, and the name of the text the cursor shows over it.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct Stash {
    pub cell: (i32, i32),
    pub text: String,
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

/// The stashes in the world kept in step with the scripts' (`MachVersteck`, the searches): one
/// made for each new stash at its cell's centre on the ground, one taken away for each gone.
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
    for (entity, stash) in &stashes {
        if !listed.iter().any(|listed| listed.cell == stash.cell) {
            commands.entity(entity).despawn();
        }
    }
    for listed in listed {
        if stashes.iter().any(|(_, stash)| stash.cell == listed.cell) {
            continue;
        }
        let point = crate::paths::centre(listed.cell);
        let Some(ground) = heights.at(point.x, point.y) else {
            continue;
        };
        commands.spawn((
            Stash {
                cell: listed.cell,
                text: listed.text.clone(),
            },
            Transform::from_xyz(point.x, ground + PICKED_ABOVE, point.y),
            Visibility::Hidden,
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// On a press of the left button that went to no attack, the stash nearest the cursor's line of
/// sight, if one is near it, becomes the hero's to search (`0x617A5B`); any press ends the last
/// such order.
fn pick(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    pointer: Res<Pointer>,
    aim: Res<Aim>,
    camera: Single<(&Camera, &GlobalTransform), crate::iso::Eye<With<Camera>>>,
    stashes: Query<(Entity, &GlobalTransform), With<Stash>>,
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
    let Some(stash) = stashes
        .iter()
        .filter_map(|(entity, at)| Some((entity, picked(ray, at.translation())?)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(entity, _)| entity)
    else {
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
        let Some(dropped) = crate::drops::contained(tables, &mut dice, (item, made_at, 1)) else {
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
        "{} hidden stashes within {radius} cells of the hero, {} in all",
        found.len(),
        live.stashes().len()
    )];
    lines.extend(found.into_iter().map(|(_, stash)| {
        format!(
            "cell {},{} runs {} shows {}",
            stash.cell.0, stash.cell.1, stash.function, stash.text
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
    mut materials: ResMut<Assets<StandardMaterial>>,
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

/// Lays every stash's blink out at the frame's time: its square at its cell's centre at the
/// height 0, brought toward the camera, and the grey half square over it, both turned alike.
fn blink(
    time: Res<Time>,
    mesh: Option<Res<BlinkMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    stashes: Query<&Stash>,
) {
    let Some(mesh) = mesh else {
        return;
    };
    let mut quads = Quads::default();
    if let Some((half, angle)) = blink_at(time.elapsed().as_millis()) {
        let whole = (Vec2::ZERO, 1.0);
        for stash in &stashes {
            let point = crate::paths::centre(stash.cell);
            let centre = Vec3::new(point.x, 0.0, point.y) - iso::forward() * BLINK_TOWARD;
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
                    pick.in_set(Control).after(attack::aim).before(hero::steer),
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
