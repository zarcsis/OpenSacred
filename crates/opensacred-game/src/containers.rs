//! The containers that the hero opens (#1380; docs/re/containers.md): the chests, barrels, vases,
//! coffins, graves, cabinets, book stands and shrines that the scripts' `CreateObj`s place, the
//! `items.pak` category 4, the client's `cChest3D`. A left click on one gives the hero the event 7
//! and the AI's state 1 (`0x53A170`), which walks it there and, standing within 300 of it, sends
//! it the event `0x21`: its own trigger opens with its model's clip of the slot 170
//! (`MOTION_ACTIVATE`) and its type's sound (`0x44BCD0`, `0x695800`), its `Take:` function runs,
//! which fills it (`0x4A71F0`), and it throws what it holds to the hero (`0x5C9BA0`, #1381) and is
//! emptied for good (`+0x14` bit `0x4000`): the cursor shows no hand over it, a click on it does
//! nothing, and its sector places it again emptied. Its function's `FillChest`s (78, `0x4A02A0`;
//! `opensacred_rules::containers`) make what it holds, seeded by the hero and the container, which
//! it throws along arcs to the hero's feet (#1381).
//!
//! As the doors, the game picks the container nearest the cursor's line of sight and the hero
//! walks where the click sends it, beside the container's closed cell (`hero::steer`). Not
//! followed yet: the town stash (5200), whose window opens instead, and the straight line of
//! walkable cells that the opening wants (`0x4FCC80`).

use std::time::Duration;

use bevy::gltf::Gltf;
use bevy::prelude::*;
use opensacred_data::netscript::{Body as RecordBody, Record, Value};
use opensacred_rules::containers::{Content, Fill, Opening};

use crate::attack::{self, Aim};
use crate::frame::{Control, Step, Unpaused};
use crate::hero::{self, Body};
use crate::hud::Pointer;
use crate::npcs::{Npcs, Object};
use crate::sfx;
use crate::terrain::Heights;

/// How near a container the hero opens it, standing, in world units (`0x53BE8C`).
const OPENS_WITHIN: f32 = 300.0;
/// The town stash, which opens its window rather than throwing what it holds (`0x428760`).
const STASH: u32 = 5200;

/// A container: its type, by which it sounds, its model, its clip of opening (`*_ACTIVATE`) and
/// its animation player once it has them, its `Take:` function and its own trigger, its
/// placement, and whether it is emptied.
#[derive(Component)]
pub struct Container {
    kind: u32,
    gltf: Handle<Gltf>,
    clip: Option<AnimationNodeIndex>,
    player: Option<Entity>,
    function: Option<String>,
    trigger: Option<String>,
    index: usize,
    /// Emptied (`+0x14` bit `0x4000`): it opens no more.
    pub emptied: bool,
}

impl Container {
    /// The container of the type `kind` and the model `gltf`, its `Take:` function and its own
    /// trigger, of the placement `index`, emptied where its hero emptied it before.
    pub(crate) fn new(
        kind: u32,
        gltf: Handle<Gltf>,
        (function, trigger): (Option<String>, Option<String>),
        index: usize,
        emptied: bool,
    ) -> Self {
        Self {
            kind,
            gltf,
            clip: None,
            player: None,
            function,
            trigger,
            index,
            emptied,
        }
    }

    /// Whether the hero may open it: not emptied, nor a town stash.
    pub fn opens(&self) -> bool {
        !self.emptied && self.kind != STASH
    }
}

/// How near a container the hero opens it without a walk, in world units (`0x53A856`).
const NO_WALK_WITHIN: f32 = 100.0;
/// The frames that the hero waits for its walk to a container to be ordered.
const WALK_BEGINS_WITHIN: u8 = 10;

/// The container that the hero was sent to by a click, which it opens as it stands near once its
/// walk there ends, and whether the walk began, and the frames it waited for it.
#[derive(Component)]
pub struct Opens(Entity, bool, u8);

/// What a container of the type `kind` sounds as it opens, by `0x695800`'s choice: the chests'
/// and the barrels' profile 605 (5200 to 5213 and 5220), the shrines' 609 (5214, 5215), the
/// vase's 610 (5218), the book stand's 611 (5222), the graves' `GRAVE_OPEN01` or `02` (324, 325)
/// at even odds by `heads`; none for another type.
fn opening_sound(kind: u32, heads: bool) -> Option<sfx::Of> {
    match kind {
        5200..=5213 | 5220 => Some(sfx::Of::Profile(605, sfx::OPEN)),
        5214 | 5215 => Some(sfx::Of::Profile(609, sfx::OPEN)),
        5218 => Some(sfx::Of::Profile(610, sfx::OPEN)),
        5222 => Some(sfx::Of::Profile(611, sfx::OPEN)),
        5216 | 5217 | 5219 | 5223..=5226 => Some(sfx::Of::Id(if heads { 324 } else { 325 })),
        _ => None,
    }
}

/// On a press of the left button that went to no attack, the container that the hero may open
/// whose model is nearest the cursor's line of sight, if one is near it, becomes the hero's to
/// open; any press ends the last such order.
pub(crate) fn pick(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    pointer: Res<Pointer>,
    aim: Res<Aim>,
    camera: Single<(&Camera, &GlobalTransform), crate::iso::Eye<With<Camera>>>,
    containers: Query<(Entity, &GlobalTransform, &Container)>,
    heroes: Query<Entity, With<Body>>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    for hero in &heroes {
        commands.entity(hero).remove::<Opens>();
    }
    let Some(cursor) = pointer.0 else {
        return;
    };
    let (camera, view) = *camera;
    let (false, Ok(ray)) = (aim.0, camera.viewport_to_world(view, cursor)) else {
        return;
    };
    let Some(container) = containers
        .iter()
        .filter(|(_, _, container)| container.opens())
        .map(|(entity, at, _)| (entity, attack::from_line(ray, at.translation())))
        .filter(|&(_, distance)| distance <= attack::PICKED_WITHIN)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(entity, _)| entity)
    else {
        return;
    };
    for hero in &heroes {
        debug!("containers: the hero goes to open {container}");
        commands.entity(hero).insert(Opens(container, false, 0));
    }
}

/// The hero that stands, once its way to a container it was sent to ends: within 300 of the
/// container's logical position (`npcs::Object`) it opens it, else it gives up. The opening
/// plays the container's clip and its sound, opens its own trigger, runs its `Take:` function
/// (`scripts::run_function`; its `FillChest`s, which fill the container, are #1381's), and
/// empties it.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn open(
    mut commands: Commands,
    mut containers: Query<(&Object, &mut Container)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    mut heroes: Query<(Entity, &Transform, &Body, &mut Opens)>,
    (mut npcs, mut heights): (ResMut<Npcs>, ResMut<Heights>),
    speed: Res<hero::GameSpeed>,
    mut dice: ResMut<crate::drops::Dice>,
    mut sounds: MessageWriter<sfx::Play>,
) {
    for (hero, transform, body, mut opens) in &mut heroes {
        // Its walk there, ordered for the world's next tick or begun (`hero::Body`).
        if body.target().is_some() || body.ordered() {
            opens.1 = true;
            continue;
        }
        let Ok((at, mut container)) = containers.get_mut(opens.0) else {
            commands.entity(hero).remove::<Opens>();
            continue;
        };
        // Farther than 100, it walks there first (`0x53A856`).
        let near = (at.at - transform.translation).xz().length() <= NO_WALK_WITHIN;
        if !opens.1 && !near && opens.2 < WALK_BEGINS_WITHIN {
            opens.2 += 1;
            continue;
        }
        commands.entity(hero).remove::<Opens>();
        if !container.opens() || !opens_from(transform.translation, at.at) {
            continue;
        }
        debug!(
            "containers: the hero opens {} ({})",
            opens.0, container.kind
        );
        if let (Some(clip), Some(player)) = (container.clip, container.player)
            && let Ok((mut player, mut transitions)) = players.get_mut(player)
        {
            transitions
                .play(&mut player, clip, Duration::ZERO)
                .set_speed(speed.0);
        }
        if let Some(sound) = opening_sound(container.kind, dice.0.below(2) == 0) {
            sounds.write(sfx::Play {
                sound,
                at: Some(opens.0),
                rate: (0.45, sfx::Roll::Tenths),
            });
        }
        if let Some(trigger) = &container.trigger {
            crate::npcs::open_container_trigger(&mut npcs, &mut heights, trigger);
        }
        if let Some(function) = container.function.clone() {
            let opened = opens.0;
            commands.queue(move |world: &mut World| {
                match crate::scripts::run_function(world, &function) {
                    Ok(lines) => {
                        for line in lines {
                            debug!("containers: {line}");
                        }
                    }
                    Err(e) => warn!("containers: {e}"),
                }
                let fills = take_fills(world);
                if let Err(e) = world.run_system_cached_with(fill, (opened, fills)) {
                    warn!("containers: {e}");
                }
            });
        }
        container.emptied = true;
        npcs.emptied.insert(container.index);
    }
}

/// The record of `FillChest` (`0x4A02A0`) and its operands' tags: a container's name, a type, the
/// value and the count of books.
const FILL_CHEST: u16 = 78;
const NAME: u8 = 0x01;
const ITEM: u8 = 0x02;
const VALUE: u8 = 0x53;
const BOOKS: u8 = 0x86;
/// How far about the hero a container's items land, along each axis (`0x5C9BA0`: `R(20, 50)`).
const LANDS_FROM: u32 = 20;
const LANDS_TO: u32 = 50;
/// The id that a container's seed takes for the local hero's (**hypothesis**: the player's).
const HERO_ID: u32 = 1;
/// The record of a container's rolls, by which its seed's generator is made.
const FILL_ROLL: usize = 0x4A02;

/// Takes the `FillChest`s that a container's function ran out of the world's records, which a
/// container's fill carries out (`fill`).
fn take_fills(world: &mut World) -> Vec<Record> {
    let Some(mut records) = world.get_resource_mut::<crate::scripts::WorldRecords>() else {
        return Vec::new();
    };
    let (fills, others) = std::mem::take(&mut records.0)
        .into_iter()
        .partition(|record| record.kind == FILL_CHEST);
    records.0 = others;
    fills
}

/// A `FillChest`'s operands ([`Fill`]).
fn fill_of(record: &Record) -> Fill {
    let RecordBody::Operands(ops) = &record.body else {
        return Fill::default();
    };
    let number = |tag: u8| {
        ops.iter().find_map(|op| match (op.tag, &op.value) {
            (t, Value::I32(n)) if t == tag => Some(*n),
            (t, Value::U16(n)) if t == tag => Some(i32::from(*n)),
            (t, Value::U8(n)) if t == tag => Some(i32::from(*n)),
            _ => None,
        })
    };
    Fill {
        value: number(VALUE),
        item: number(ITEM).and_then(|n| u32::try_from(n).ok()),
        books: number(BOOKS)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0),
        named: ops.iter().any(|op| op.tag == NAME),
    }
}

/// Fills the container `opened` by its `FillChest`s `fills` (`0x4A02A0`) and throws what it holds
/// to the hero (`0x5C9BA0`): its items and gold made, each flying from the container to a point
/// `R(20, 50)` from the hero along each axis, and a creature raised beside the hero.
fn fill(
    In((opened, fills)): In<(Entity, Vec<Record>)>,
    mut commands: Commands,
    (tables, mut dice, assets): (
        Option<Res<crate::drops::Tables>>,
        ResMut<crate::drops::Dice>,
        Res<AssetServer>,
    ),
    (experience, pack, arts, mut given): (
        Res<crate::fight::Experience>,
        Res<crate::pickup::Pack>,
        Option<Res<crate::arts::HeroArts>>,
        ResMut<crate::rewards::RunesGiven>,
    ),
    containers: Query<(&Object, &Container)>,
    heroes: Query<(&Transform, &crate::fight::Base, &Body)>,
    lying: Query<&crate::drops::Lying>,
) {
    let (Some(tables), Ok((at, container)), Some((hero_at, base, body))) = (
        tables.as_deref(),
        containers.get(opened),
        heroes.iter().next(),
    ) else {
        return;
    };
    if fills.is_empty() {
        return;
    }
    let level = experience.0.level;
    let mut hero = crate::drops::hero(base, level);
    crate::drops::carrying(&mut hero, &pack, arts.as_deref(), tables);
    let icon_in_world = lying
        .iter()
        .any(|lying| (5101..=5106).contains(&lying.dropped.item))
        || pack
            .items
            .iter()
            .any(|(_, dropped)| (5101..=5106).contains(&dropped.item));
    // The seed of the hero and the container (`0x4A0705`).
    let seed = opensacred_rules::containers::seed(
        (base.id, HERO_ID, experience.0.points),
        (
            whole(at.at.x),
            whole(at.at.z),
            u32::try_from(container.index).unwrap_or(u32::MAX),
        ),
    );
    let mut rolls = opensacred_rules::spawning::Rng::of(
        u32::try_from(seed & 0xFFFF_FFFF).unwrap_or(0),
        u32::try_from(seed >> 32).unwrap_or(0),
        FILL_ROLL,
    );
    let mut opening = Opening {
        hero: &hero,
        gold_level: level,
        rules: tables.rules(),
        raises: Opening::raising(container.kind),
        icon_in_world,
        given: &mut given.0,
    };
    let mut made = Vec::new();
    for record in &fills {
        made.extend(opensacred_rules::containers::fill(
            &fill_of(record),
            &mut opening,
            tables.catalogue(),
            |item| tables.price(item),
            &mut rolls,
        ));
    }
    let hero_point = hero_at.translation;
    for content in made {
        let (item, level, amount) = match content {
            Content::Item { item, level } => (item, level, 1),
            Content::Gold { pile, amount } => (pile, level, amount),
            Content::Creature(creature) => {
                raise(&mut commands, creature, hero_point, level);
                continue;
            }
        };
        let Some(dropped) = crate::drops::contained(tables, &mut dice, (item, level, amount))
        else {
            continue;
        };
        let mut aside = || {
            let along = rolls.below(LANDS_TO - LANDS_FROM + 1) + LANDS_FROM;
            let along = f32::from(u16::try_from(along).unwrap_or(0));
            if rolls.below(2) == 0 { along } else { -along }
        };
        let to = hero_point + Vec3::new(aside(), 0.0, aside());
        debug!("containers: {opened} throws {item} ({amount})");
        crate::drops::throw_to(
            &mut commands,
            (&assets, tables),
            (at.at, to, body.level()),
            dropped,
        );
    }
}

/// Raises the creature `creature` at the level `level` beside the hero at `hero`, as a grave or
/// a shrine does where its value gives nothing (`0x4A0786`; **hypothesis** for its place,
/// `0x45FF80`, which is not read: the cell beside the hero's).
fn raise(commands: &mut Commands, creature: u32, hero: Vec3, level: u32) {
    let (x, y) = crate::paths::cell_of(hero.x, hero.z);
    let (Ok(x), Ok(y)) = (u32::try_from(x + 1), u32::try_from(y + 1)) else {
        return;
    };
    let spawn = crate::npcs::Spawn {
        creature,
        cells: vec![(x, y)],
        floor: 0,
        level: Some(level),
        rank: crate::fight::Rank::Common,
        hostile: None,
        immortal: false,
        roams: false,
    };
    commands.queue(move |world: &mut World| {
        match world.run_system_cached_with(crate::npcs::spawn_creatures, spawn) {
            Ok(Ok(made)) => debug!("containers: raised {creature}: {made:?}"),
            Ok(Err(e)) => warn!("containers: {e}"),
            Err(e) => warn!("containers: {e}"),
        }
    });
}

/// A world coordinate as a whole number, as the object's `+0x1C` and `+0x20` hold it.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a world coordinate, far within the integers' range"
)]
fn whole(value: f32) -> i32 {
    value as i32
}

/// Whether a hero standing at `hero` opens the container at `container`: within
/// [`OPENS_WITHIN`] of it on the ground.
fn opens_from(hero: Vec3, container: Vec3) -> bool {
    (container - hero).xz().length() <= OPENS_WITHIN
}

/// Gives each container's animation player its clip of opening, played to its end at once for
/// one emptied before (its sector placed again).
fn clips(
    mut commands: Commands,
    mut containers: Query<&mut Container>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut players: Query<(Entity, &mut AnimationPlayer), Without<AnimationGraphHandle>>,
    parents: Query<&ChildOf>,
) {
    for (entity, mut player) in &mut players {
        // The container this player belongs to: the nearest ancestor with `Container`.
        let mut ancestor = entity;
        let container = loop {
            if containers.contains(ancestor) {
                break Some(ancestor);
            }
            match parents.get(ancestor) {
                Ok(parent) => ancestor = parent.parent(),
                Err(_) => break None,
            }
        };
        let Some(Ok(mut container)) = container.map(|container| containers.get_mut(container))
        else {
            continue;
        };
        let Some(gltf) = gltfs.get(&container.gltf) else {
            continue;
        };
        let mut graph = AnimationGraph::new();
        let root = graph.root;
        let opening = gltf
            .named_animations
            .iter()
            .find(|(name, _)| name.ends_with("_ACTIVATE"))
            .map(|(_, clip)| graph.add_clip(clip.clone(), 1.0, root));
        container.clip = opening;
        container.player = Some(entity);
        let mut transitions = AnimationTransitions::new();
        if let Some(clip) = opening.filter(|_| container.emptied) {
            // Its last pose at once: the clip at a speed that ends it in a frame.
            transitions
                .play(&mut player, clip, Duration::ZERO)
                .set_speed(EMPTIED_AT_ONCE);
        }
        commands
            .entity(entity)
            .insert((AnimationGraphHandle(graphs.add(graph)), transitions));
    }
}

/// The speed of the opening's clip of a container placed again emptied, which ends it in a frame.
const EMPTIED_AT_ONCE: f32 = 1000.0;

/// The containers that the hero opens.
pub struct ContainersPlugin;

impl Plugin for ContainersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                pick.in_set(Control).after(attack::aim).before(hero::steer),
                open.after(hero::run),
            )
                .in_set(Unpaused)
                .in_set(Step::HeroMove),
        )
        .add_systems(Update, clips);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hero_opens_a_container_it_stands_within_300_of() {
        let container = Vec3::new(1000.0, 0.0, 1000.0);
        assert!(opens_from(
            container + Vec3::new(200.0, 50.0, 200.0),
            container
        ));
        assert!(!opens_from(
            container + Vec3::new(250.0, 0.0, 250.0),
            container
        ));
    }

    #[test]
    fn a_container_sounds_by_its_type() {
        let profile = |kind| match opening_sound(kind, true) {
            Some(sfx::Of::Profile(profile, group)) => Some((profile, group)),
            _ => None,
        };
        assert_eq!(profile(5207), Some((605, sfx::OPEN)));
        assert_eq!(profile(5214), Some((609, sfx::OPEN)));
        assert_eq!(profile(5218), Some((610, sfx::OPEN)));
        assert_eq!(profile(5222), Some((611, sfx::OPEN)));
        assert!(matches!(opening_sound(5216, true), Some(sfx::Of::Id(324))));
        assert!(matches!(opening_sound(5216, false), Some(sfx::Of::Id(325))));
        assert!(opening_sound(5195, true).is_none());
    }

    #[test]
    fn an_emptied_container_or_a_stash_opens_no_more() {
        let container =
            |kind, emptied| Container::new(kind, Handle::default(), (None, None), 0, emptied);
        assert!(container(5201, false).opens());
        assert!(!container(5201, true).opens());
        assert!(!container(STASH, false).opens());
    }
}
