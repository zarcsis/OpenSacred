//! The creatures' spit (880, #2099; docs/re/effects.md, "The creatures' spit"): the giant spider
//! (55) and the killer crab queen (348), coming on at their target from 371 away, may take the
//! melee's state 9 (`creature_casts`, `opensacred_rules::casts::SPIT`), play their clip and, at
//! its share, spit a gib part of the giant spider's set ahead of them along their facing, from the
//! spider's head (`Head_02`) or the queen's feet (`opensacred_rules::particles::Spit`): it flies
//! its arc tumbling, trailing green blobs (the queen's red) laid over by their alpha and tinted by
//! the night, strikes once a second every creature hostile to its spitter it comes within its size
//! and theirs of, the hero among them, with the spit's fixed damages and no hit chance
//! (`opensacred_rules::fight::Rules::spit_damages`), and lands, laying a mark of blood and
//! sounding; over a cell that may not be walked on it ends at once. In one case of three the giant
//! spider bears two to four young of half its level on its spot instead.

use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;
use opensacred_rules::casts::{
    CRAB_CAGES, CRAB_CAGES_HOLD, CRAB_QUEEN, SPIT_LANDS, SPIT_MARK_HALF, SPIT_MARK_HOLDS,
    SPIT_MARKS, SPIT_SOUNDS, YOUNG_SPIDER, bears_young, crab_cages, spit_reach, spit_sound, young,
    young_level,
};
use opensacred_rules::particles::{
    SPIT_GRID, SPIT_SCALE, SPIT_SIZE, SPIT_TEXTURES, Spit, spit_part,
};

use crate::bolt_looks::{models, world};
use crate::crowd::InTheWay;
use crate::fight::{self, Health};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::hero::Body;
use crate::npcs::{Dies, Retexture};
use crate::spawning::Rng;
use crate::terrain::{Heights, Level};
use crate::{ai, iso, models as model_looks, paths};

/// The bone the spit leaves from (the FX event `0x66`'s bone 41 of the table at `0x8E7200`): the
/// giant spider's head; a spitter without it spits from its feet.
const HEAD: &str = "Head_02";

/// A creature that spits at the clip's share of its cast (`creature_casts`).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Spat {
    pub spitter: Entity,
    pub target: Entity,
}

/// The spits' dice (`rand()`; **estimate**: any seed).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x880))
    }
}

/// The blobs' materials, the spiders' and the crab queen's, laid over by their alpha; made at the
/// first spit, none where a texture is missing.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    blobs: [Option<Handle<Effect>>; 2],
}

/// A spit in flight: its system in the frame of its start, its spitter, its damages, the level it
/// flies on, its model and the mesh of its blobs.
#[derive(Component)]
struct Flight {
    spit: Spit,
    spitter: Entity,
    damages: [f32; 4],
    level: Level,
    model: Option<Entity>,
    mesh: Handle<Mesh>,
}

/// A unit roll, `rand() / 32767`.
fn unit(dice: &mut Rng) -> f32 {
    f32::from(u16::try_from(dice.below(0x8000)).unwrap_or(0)) / 32_767.0
}

/// Spits for each creature whose clip reaches its share (`0x4E4CF0`'s sub-state 5, `0x4E5A2A`):
/// the giant spider bears its young where the first roll asks it, else the spit flies from the
/// spitter's head toward a point ahead of it along its facing, 300 to 400 away, level with it.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn spit(
    mut commands: Commands,
    mut spat: MessageReader<Spat>,
    spitters: Query<
        (
            &Transform,
            &ai::Mind,
            &InTheWay,
            Option<&crate::npcs::Arms>,
            Option<&fight::Combat>,
        ),
        Without<Dies>,
    >,
    (children, bones, worn, targets): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
        Query<(), (With<fight::Health>, Without<Dies>)>,
    ),
    (data, assets, tables, rules): (
        Option<Res<Data>>,
        Res<AssetServer>,
        Option<Res<crate::drops::Tables>>,
        Res<fight::Rules>,
    ),
    (mut dice, mut looks, mut materials, mut meshes, mut sounds): (
        ResMut<Dice>,
        ResMut<Looks>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
        MessageWriter<crate::sfx::Play>,
    ),
) {
    for &Spat { spitter, target } in spat.read() {
        let Ok((at, mind, way, arms, combat)) = spitters.get(spitter) else {
            continue;
        };
        let dice = &mut dice.0;
        // The first roll, for every spitter (`0x4E5A2A`).
        if bears_young(mind.creature, dice.below(3)) {
            let count = young(|| dice.below(3));
            let level = young_level(mind.level);
            info!("spits: {spitter} bears {count} young of level {level}");
            let (place, layer) = (at.translation, way.level);
            commands.queue(move |world: &mut World| bear(world, (place, layer), count, level));
            continue;
        }
        let crab = mind.creature == CRAB_QUEEN;
        // The crab queen's cages on its target, by an odd roll, before its spit (`0x4E58EF`).
        if crab && crab_cages(mind.creature, dice.below(0x8000)) && targets.contains(target) {
            info!("spits: {spitter} drops its cages on {target}");
            hold(&mut commands, target);
        }
        if !looks.made {
            looks.made = true;
            if let Some(data) = data.as_deref() {
                looks.blobs = SPIT_TEXTURES.map(|name| {
                    let image = particle_texture(&data.0.assets, &assets, name);
                    if image.is_none() {
                        warn!("spits: no texture {name}; its blobs are not drawn");
                    }
                    image.map(|image| materials.add(particle_material(image, AlphaMode::Blend)))
                });
            }
        }
        // Its part, rolled as it is made (`0x77B040`), then its reach (`0x4E5B3B`).
        let part = spit_part(dice);
        let reach = spit_reach(unit(dice));
        let place = at.translation;
        let facing = (at.rotation * Vec3::X).with_y(0.0).normalize_or(Vec3::X);
        let end = place + facing * reach * iso::MODEL_SCALE;
        let start =
            crate::making::bone(spitter, &[HEAD], &children, (&bones, &worn)).unwrap_or(place);
        let flight = Spit::new(crab, ([0.0; 3], models(end - start)), dice);
        let skill_2 = arms.map_or(0, |arms| arms.skill_level(mind.level, 2));
        let dexterity = combat.map_or(0.0, |combat| combat.attributes[2]);
        let damages = rules
            .0
            .spit_damages(mind.creature, mind.level, (dexterity, skill_2));
        let model =
            tables
                .as_deref()
                .and_then(|tables| tables.look(part))
                .map(|(model, texture, _)| {
                    let mut model = commands.spawn((
                        model_looks::scene(&assets, model),
                        Transform::from_translation(start).with_scale(Vec3::ZERO),
                        model_looks::Ordered::default(),
                        DespawnOnExit(AppState::InGame),
                    ));
                    if let Some(texture) = texture {
                        model.insert(Retexture::load(&assets, texture));
                    }
                    model.id()
                });
        let mesh = meshes.add(empty());
        let mut flying = commands.spawn((
            Transform::from_translation(start),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(material) = looks.blobs[usize::from(crab)].clone() {
            flying.with_child((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
            ));
        }
        flying.insert(Flight {
            spit: flight,
            spitter,
            damages,
            level: way.level,
            model,
            mesh,
        });
        let flying = flying.id();
        info!("spits: {spitter} spits {part} toward {end} with {damages:?}");
        // Its sound as it starts (`0x77B700`), in four cases of five.
        if let Some(sound) = spit_sound(SPIT_SOUNDS, dice.below(5)) {
            sounds.write(crate::sfx::Play {
                sound: crate::sfx::Of::Id(sound),
                at: Some(flying),
                rate: (0.4, crate::sfx::Roll::Share(0.2)),
            });
        }
    }
}

/// Holds `target` by the crab queen's cages: the plant cage's effect 15 for 3 s (`0x5B5CC8`)
/// and its ten cages about it, the first and the sixth sounding (`plant_cages`).
fn hold(commands: &mut Commands, target: Entity) {
    let effect = opensacred_rules::fight::Effect {
        id: opensacred_rules::spells::HELD,
        whole: 0,
        value: 0.0,
        art: u32::from(opensacred_rules::casts::CAGE),
        level: 0,
    };
    commands
        .entity(target)
        .entry::<crate::effects::Effects>()
        .or_default()
        .and_modify(move |mut effects| effects.add(effect, CRAB_CAGES_HOLD));
    crate::plant_cages::cage(commands, target, CRAB_CAGES, (false, CRAB_CAGES_HOLD));
}

/// Makes `count` young spiders of `level` on the spot `place` of the layer `layer`, hostile
/// (`sdk_CREATE(75, …)`, `+0x1F4 |= 1`, `sdk_WAKE`): the original's offset of 0 to 2 world units
/// puts them all on the spider's own spot.
fn bear(world: &mut World, (place, layer): (Vec3, Level), count: u32, level: u32) {
    let floor = world
        .get_resource::<Heights>()
        .and_then(|heights| heights.floor(layer))
        .map_or(0, |(_, floor)| i32::from(floor));
    let (x, y) = paths::cell_of(place.x, place.z);
    let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
        return;
    };
    let request = crate::npcs::Spawn {
        creature: YOUNG_SPIDER,
        cells: vec![(x, y); usize::try_from(count).unwrap_or(0)],
        floor,
        level: Some(level),
        rank: fight::Rank::Common,
        hostile: Some(true),
        immortal: false,
        roams: false,
    };
    match world.run_system_cached_with(crate::npcs::spawn_creatures, request) {
        Ok(Ok(made)) => debug!("spits: young spiders {made:?}"),
        Ok(Err(e)) => warn!("spits: no young spider is made: {e}"),
        Err(e) => warn!("spits: no young spider is made: {e}"),
    }
}

/// The square of a blob's frame of the 2 × 2 grid: its corner and its side.
fn frame(frame: u8) -> (Vec2, f32) {
    let side = f32::from(SPIT_GRID);
    let corner = Vec2::new(f32::from(frame % SPIT_GRID), f32::from(frame / SPIT_GRID)) / side;
    (corner, 1.0 / side)
}

/// Flies each spit a frame on (`0x77B180`, `0x5A6680`'s flags 8 and `0x40000`): over a cell that
/// may not be walked on it ends at once; it strikes the creatures hostile to its spitter that it
/// comes within its size and theirs of on its level, the hero among them, once a second; landed,
/// it lays its mark and sounds; its model tumbles at twice its size and its blobs fade; it goes
/// with its last blob.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    mut flights: Query<(Entity, &mut Flight, &Transform)>,
    mut models: Query<&mut Transform, Without<Flight>>,
    minds: Query<&ai::Mind, Without<Dies>>,
    hero: Query<(Entity, &GlobalTransform, &Body, &Health), (Without<Flight>, Without<Dies>)>,
    creatures: Query<
        (
            Entity,
            &GlobalTransform,
            &InTheWay,
            &Health,
            Option<&crate::arrows::Size>,
        ),
        (Without<Body>, Without<Flight>, Without<Dies>),
    >,
    (heights, orders): (Res<Heights>, Orders),
    (mut meshes, mut dice, mut hits, mut sounds): (
        ResMut<Assets<Mesh>>,
        ResMut<Dice>,
        MessageWriter<fight::Hit>,
        MessageWriter<crate::sfx::PlayAt>,
    ),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let hero = hero.iter().next();
    for (entity, mut flight, place) in &mut flights {
        let origin = place.translation;
        let Flight {
            spit,
            spitter,
            damages,
            level,
            model,
            mesh,
        } = &mut *flight;
        let lands = spit.update(dt, &mut dice.0);
        let head = origin + world(spit.place());
        let cell = paths::cell_of(head.x, head.z);
        let open = heights.walkable_on(cell, *level) || heights.flown_over(cell, *level);
        if spit.flies() && !open {
            debug!("spits: {entity} ends over {cell:?}");
            if let Some(model) = model.take() {
                commands.entity(model).try_despawn();
            }
            commands.entity(entity).despawn();
            continue;
        }
        if lands {
            // Its mark of blood on the ground (`0x5B33AB`) and its sound (`0x77B180`).
            let ground = heights.standing(head.x, head.z, *level).unwrap_or(head.y);
            let at = head.with_y(ground);
            let marks = &SPIT_MARKS[usize::from(spit.crab)];
            let texture = marks[usize::try_from(dice.0.below(6)).unwrap_or(0) % marks.len()];
            crate::marks::lay(
                &mut commands,
                at,
                (texture, SPIT_MARK_HOLDS, SPIT_MARK_HALF),
            );
            if let Some(sound) = spit_sound(SPIT_LANDS, dice.0.below(5)) {
                sounds.write(crate::sfx::PlayAt {
                    sound: crate::sfx::Of::Id(sound),
                    point: at,
                    rate: (0.4, crate::sfx::Roll::Share(0.2)),
                });
            }
            if let Some(model) = model.take() {
                commands.entity(model).try_despawn();
            }
        }
        if spit.done() {
            commands.entity(entity).despawn();
            continue;
        }
        // The creatures it touches, hostile to its spitter and alive (`0x5AB490`, `0x548F30`).
        if spit.flies()
            && let Ok(spitter_mind) = minds.get(*spitter)
        {
            let within = |at: Vec3, size: f32| (at - head).xz().length() < SPIT_SIZE + size;
            let mut touched: Vec<Entity> = Vec::new();
            if let Some((hero, at, body, health)) = hero
                && body.level() == *level
                && health.current > 0
                && spitter_mind.hostile()
                && within(at.translation(), crate::arrows::size(0))
            {
                touched.push(hero);
            }
            touched.extend(
                creatures
                    .iter()
                    .filter(|&(other, at, way, health, size)| {
                        other != *spitter
                            && way.creature
                            && way.level == *level
                            && health.current > 0
                            && minds
                                .get(other)
                                .is_ok_and(|other| spitter_mind.hostile_to(other))
                            && within(
                                at.translation(),
                                size.map_or(crate::arrows::size(0), |size| size.0),
                            )
                    })
                    .map(|(other, ..)| other),
            );
            if !touched.is_empty() && spit.strikes() {
                for target in touched {
                    debug!("spits: {entity} of {spitter} strikes {target}");
                    hits.write(fight::Hit {
                        attacker: *spitter,
                        target,
                        share: 1.0,
                        ranged: true,
                        art: None,
                        fixed: Some(*damages),
                        sure: true,
                        poison: false,
                    });
                }
            }
        }
        if let Some(model) = *model
            && let Ok(mut transform) = models.get_mut(model)
        {
            let [a, b] = spit.turns;
            *transform = Transform::from_translation(head)
                .with_rotation(Quat::from_rotation_z(b) * Quat::from_rotation_x(a))
                .with_scale(Vec3::splat(SPIT_SCALE * s));
        }
        let mut quads = Quads::default();
        for blob in &spit.system.particles {
            let [r, g, b] = blob.colour;
            quads.push(
                world(blob.place),
                blob.size * s,
                blob.angle,
                frame(blob.frame),
                [r, g, b, blob.life / 255.0],
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&*mesh) {
            quads.in_order(orders.at(head), origin);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The creatures' spit and the giant spider's young.
pub struct SpitsPlugin;

impl Plugin for SpitsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Spat>()
            .init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (spit, fly)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
