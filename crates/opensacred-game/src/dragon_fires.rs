//! The dragons' breath of fire (852, `FX_DRAGONFIRE`, #1964; docs/re/effects.md, "The dragons'
//! fire"): a dragon that comes on to its target breathes it now and then (`creature_casts`,
//! `opensacred_rules::casts::DRAGON_FIRE`), a jet of flames from its head that lays fires on the
//! ground under the head (923, `FX_DRAGON_GROUNDFIRE`), both added whole and untinted by the night,
//! and each lighting it (`opensacred_rules::particles::{DragonFire, GroundFire}`). Each fire on
//! the ground strikes the hero and the hero's followers about it once, of the dragon's damages
//! (`opensacred_rules::fight::Rules::ground_fire_damages`, #2084).

use std::collections::HashMap;

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, DragonFire, GroundFire, Ramp};

use crate::frame::{Playing, Unpaused};
use crate::fx::{Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::sfx::{Of, Play, Roll};
use crate::spawning::Rng;
use crate::{bolt_looks, daynight, iso};

/// The bone the breath keeps to (the FX event `0x4C`'s bone 20).
const HEAD: [&str; 1] = ["Bip01 Head"];

/// The fires' dice: the breaths' first fires on the ground and the flames.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x852))
    }
}

/// The flames' materials by their texture, made as they are first asked; none where the texture
/// is missing.
#[derive(Resource, Default)]
struct Materials(HashMap<&'static str, Option<Handle<crate::fx::Effect>>>);

impl Materials {
    fn get(
        &mut self,
        texture: &'static str,
        (data, assets, materials): (Option<&Data>, &AssetServer, &mut Assets<crate::fx::Effect>),
    ) -> Option<Handle<crate::fx::Effect>> {
        self.0
            .entry(texture)
            .or_insert_with(|| {
                let image = data.and_then(|data| particle_texture(&data.0.assets, assets, texture));
                if image.is_none() {
                    warn!("dragon fires: no texture {texture}; its fires are not drawn");
                }
                image.map(|image| materials.add(whole_material(image)))
            })
            .clone()
    }
}

/// A breath to make at its dragon's head.
#[derive(Component)]
struct Unmade {
    dragon: Entity,
    variant: u8,
}

/// Starts a breath of the variant `variant` at the head of `dragon`, which stands at `at`, with
/// its two sounds (`0x4E3DB9`, `0x77AF2D`).
pub fn breathe(
    commands: &mut Commands,
    sounds: &mut MessageWriter<Play>,
    (dragon, at): (Entity, Vec3),
    variant: u8,
) {
    commands.spawn((
        Unmade { dragon, variant },
        Transform::from_translation(at),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
    for _ in 0..2 {
        sounds.write(Play {
            sound: Of::Id(opensacred_rules::casts::FLAME_BREATH),
            at: Some(dragon),
            rate: (0.5, Roll::None),
        });
    }
}

/// A breath that burns: its flames in the frame of its entity's place, the dragon and its head,
/// where the head was a frame ago, and the mesh its flames go into.
#[derive(Component)]
struct Breath {
    fire: DragonFire,
    dragon: Entity,
    head: Entity,
    last: Option<[f32; 3]>,
    mesh: Handle<Mesh>,
    lit: bool,
}

/// A fire on the ground: its flames in the frame of its place, their mesh, the dragon that laid
/// it, its damages, and whether it has struck.
#[derive(Component)]
struct OnTheGround {
    fire: GroundFire,
    ramp: Ramp,
    mesh: Handle<Mesh>,
    caster: Entity,
    damages: [f32; 4],
    struck: bool,
}

/// A fire on the ground's size for its hits (`items.pak +0x14` of 923, `0x428CE0`), read at the
/// start.
#[derive(Resource)]
struct FireSize(f32);

impl Default for FireSize {
    fn default() -> Self {
        Self(opensacred_rules::arrows::size(0))
    }
}

/// Reads the fires' size off the assets' tables at the start.
fn read(mut commands: Commands, data: Option<Res<Data>>) {
    let size = data.and_then(|data| {
        data.0.tables.items().ok().and_then(|items| {
            items
                .iter()
                .find(|item| item.slot == u32::from(opensacred_rules::particles::GROUND_FIRE))
                .map(|item| opensacred_rules::arrows::size(item.words_14[0]))
        })
    });
    if let Some(size) = size {
        commands.insert_resource(FireSize(size));
    }
}

/// A fire's flames' mesh, drawn under its entity of `material` where there is one, untinted by
/// the night.
fn flames(
    commands: &mut Commands,
    entity: Entity,
    meshes: &mut Assets<Mesh>,
    material: Option<Handle<crate::fx::Effect>>,
) -> Handle<Mesh> {
    let mesh = meshes.add(empty());
    if let Some(material) = material {
        commands.entity(entity).with_children(|parts| {
            parts.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::default(),
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        });
    }
    mesh
}

/// Gives each new breath its dragon's head, its flames' mesh and material; a dragon without a
/// head breathes none.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries in groups"
)]
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (data, assets): (Option<Res<Data>>, Res<AssetServer>),
    (mut made, mut materials, mut meshes, mut dice): (
        ResMut<Materials>,
        ResMut<Assets<crate::fx::Effect>>,
        ResMut<Assets<Mesh>>,
        ResMut<Dice>,
    ),
) {
    for (entity, unmade) in &unmade {
        commands.entity(entity).remove::<Unmade>();
        let Some(head) =
            crate::making::bone_entity(unmade.dragon, &HEAD, &children, (&bones, &worn))
        else {
            debug!(
                "dragon fires: {} has no head to breathe from",
                unmade.dragon
            );
            commands.entity(entity).despawn();
            continue;
        };
        let texture = particles::DRAGON_FIRE_TEXTURES
            .get(usize::from(unmade.variant))
            .copied()
            .unwrap_or(particles::DRAGON_FIRE_TEXTURES[0]);
        let material = made.get(texture, (data.as_deref(), &assets, &mut materials));
        let mesh = flames(&mut commands, entity, &mut meshes, material);
        debug!(
            "dragon fires: {} breathes fire of the variant {}",
            unmade.dragon, unmade.variant
        );
        commands.entity(entity).insert(Breath {
            fire: DragonFire::new(unmade.variant, &mut dice.0),
            dragon: unmade.dragon,
            head,
            last: None,
            mesh,
            lit: false,
        });
    }
}

/// Runs each breath a frame on at its dragon's head (`0x77AAE0`): its flames leave along the
/// head's `−y` in the engine's frame, which mirrors the model's `y` (the FX frame is left-handed),
/// so the bone's own `+y`, which the converter's turn of each bone into glTF's frame makes its
/// `−z` (`opensacred-convert`'s `gltf`); it lights the night while it burns, and every 0.3 s it
/// lays a fire on the ground under the head; it ends at 4 s, or as its dragon goes.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries in groups"
)]
fn breathe_on(
    mut commands: Commands,
    time: Res<Time>,
    mut breaths: Query<(Entity, &mut Breath, &Transform)>,
    places: Query<&GlobalTransform>,
    (data, assets): (Option<Res<Data>>, Res<AssetServer>),
    (mut made, mut materials, mut meshes, mut dice): (
        ResMut<Materials>,
        ResMut<Assets<crate::fx::Effect>>,
        ResMut<Assets<Mesh>>,
        ResMut<Dice>,
    ),
    (dragons, rules): (
        Query<(&crate::ai::Mind, Option<&crate::npcs::Arms>)>,
        Res<crate::fight::Rules>,
    ),
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut breath, transform) in &mut breaths {
        let (Ok(head), Ok(dragon)) = (places.get(breath.head), places.get(breath.dragon)) else {
            commands.entity(entity).despawn();
            continue;
        };
        let origin = transform.translation;
        let to = bolt_looks::models(head.translation() - origin);
        let from = breath.last.unwrap_or(to);
        let along = Vec3::from(bolt_looks::models(
            head.affine().transform_vector3(Vec3::NEG_Z),
        ))
        .normalize_or_zero();
        let caster = breath.dragon;
        let Breath {
            fire, mesh, lit, ..
        } = &mut *breath;
        let lays = fire.update(dt, (from, to), along.to_array(), &mut dice.0);
        if fire.done() {
            commands.entity(entity).despawn();
            continue;
        }
        if fire.burns() && !*lit {
            *lit = true;
            commands
                .entity(entity)
                .insert(daynight::Light(particles::DRAGON_FIRE_LIGHT));
        }
        if lays {
            let point = head.translation().with_y(dragon.translation().y);
            // Its damages by the dragon's level and its skill 2 (`0x5AF75D`).
            let damages = dragons.get(caster).map_or([0.0; 4], |(mind, arms)| {
                let skill_2 = arms.map_or(0, |arms| arms.skill_level(mind.level, 2));
                rules.0.ground_fire_damages(mind.level, skill_2)
            });
            lay(
                &mut commands,
                (point, fire.variant),
                (caster, damages),
                (data.as_deref(), &assets),
                (&mut made, &mut materials, &mut meshes),
            );
        }
        let mut quads = Quads::default();
        let grid = particles::DRAGON_FIRE_GRID;
        for particle in &fire.system.particles {
            quads.push(
                bolt_looks::world(particle.place),
                particle.size * iso::MODEL_SCALE,
                particle.angle,
                frame(particles::age_frame(particle.life, grid), grid),
                [1.0, 1.0, 1.0, 0.0],
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&*mesh) {
            quads.into_mesh(&mut mesh);
        }
        breath.last = Some(to);
    }
}

/// The corner and the side of the frame `frame` of a `grid × grid` atlas.
fn frame(frame: u8, grid: u8) -> (Vec2, f32) {
    let side = grid.max(1);
    let corner = Vec2::new(f32::from(frame % side), f32::from(frame / side % side));
    (corner / f32::from(side), 1.0 / f32::from(side))
}

/// Lays a fire on the ground of the variant `variant` at `point` (the system's action 1,
/// `0x5B2600`), lit by 200, of the dragon `caster` and its damages `damages`.
fn lay(
    commands: &mut Commands,
    (point, variant): (Vec3, u8),
    (caster, damages): (Entity, [f32; 4]),
    (data, assets): (Option<&Data>, &AssetServer),
    (made, materials, meshes): (
        &mut Materials,
        &mut Assets<crate::fx::Effect>,
        &mut Assets<Mesh>,
    ),
) {
    let texture = particles::GROUND_FIRE_TEXTURES
        .get(usize::from(variant))
        .copied()
        .unwrap_or(particles::GROUND_FIRE_TEXTURES[0]);
    let entity = commands
        .spawn((
            Transform::from_translation(point),
            Visibility::default(),
            daynight::Light(particles::GROUND_FIRE_LIGHT),
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    let material = made.get(texture, (data, assets, materials));
    let mesh = flames(commands, entity, meshes, material);
    commands.entity(entity).insert(OnTheGround {
        fire: GroundFire::new(variant),
        ramp: GroundFire::ramp(),
        mesh,
        caster,
        damages,
        struck: false,
    });
}

/// Strikes, at each fire's first frame with targets about it (`0x5AF75D`, `+0x11C`), the hero and
/// the hero's followers alive nearer it than the sum of their sizes, once, by the dragon's damages
/// as a spell's (the event 5 of `0x51CB00`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn strike(
    mut fires: Query<(&mut OnTheGround, &Transform)>,
    targets: Query<
        (Entity, &GlobalTransform, Option<&crate::arrows::Size>),
        (
            Or<(With<crate::hero::Body>, With<crate::followers::Follower>)>,
            Without<crate::npcs::Dies>,
        ),
    >,
    size: Res<FireSize>,
    mut hits: MessageWriter<crate::fight::Hit>,
) {
    for (mut fire, at) in &mut fires {
        if fire.struck {
            continue;
        }
        let here = at.translation.xz();
        let within: Vec<Entity> = targets
            .iter()
            .filter(|(_, place, other)| {
                let other = other.map_or(opensacred_rules::arrows::size(0), |other| other.0);
                (place.translation().xz() - here).length() < size.0 + other
            })
            .map(|(target, ..)| target)
            .collect();
        if within.is_empty() {
            continue;
        }
        fire.struck = true;
        for target in within {
            debug!(
                "dragon fires: a fire of {} strikes {target} with {:?}",
                fire.caster, fire.damages
            );
            hits.write(crate::fight::Hit {
                attacker: fire.caster,
                target,
                share: 1.0,
                ranged: false,
                art: None,
                fixed: Some(fire.damages),
                sure: true,
                poison: false,
            });
        }
    }
}

/// Runs each fire on the ground a frame on (`0x7AB070`), its flames standing on their feet; it
/// goes with its last flame.
fn burn(
    mut commands: Commands,
    time: Res<Time>,
    mut fires: Query<(Entity, &mut OnTheGround, &GlobalTransform)>,
    (mut dice, mut meshes, orders): (ResMut<Dice>, ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut fire, place) in &mut fires {
        let OnTheGround {
            fire, ramp, mesh, ..
        } = &mut *fire;
        fire.update(dt, &mut dice.0);
        if fire.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut quads = Quads::default();
        let grid = particles::DRAGON_FIRE_GRID;
        for particle in &fire.system.particles {
            quads.push_footed(
                bolt_looks::world(particle.place),
                particle.size * iso::MODEL_SCALE,
                0.0,
                frame(particles::age_frame(particle.life, grid), grid),
                ramp.at(particle.life),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&*mesh) {
            // At its object's place in the order, as the original draws the system (#2188).
            let origin = place.translation();
            quads.in_order(orders.at(origin), origin);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The dragons' breaths and the fires they lay.
pub struct DragonFiresPlugin;

impl Plugin for DragonFiresPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Materials>()
            .init_resource::<FireSize>()
            .add_systems(Startup, read)
            .add_systems(
                Update,
                (make, breathe_on, burn, strike)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
