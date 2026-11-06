//! The plant cages' looks (849, `FX_PLANTCAGE`, #1092; docs/re/effects.md, "The plant cage"): each
//! cage about a held creature, the Wood Elf's spell 40 (`spells`), a creature's at the hero
//! (`creature_casts`) or the ten of the hero's terror (`ai`), is a vine of dots facing the camera
//! from the creature's neck down to a base point on the ground within 50 of it, growing from the
//! ground up over 0.667 s, swaying as its waves run down it, seven spurs from its bends and a glow
//! flat on the ground at its base, all added by their alpha in its colour, green a hero's and blue
//! a creature's, untinted by the night, and its light at its base
//! (`opensacred_rules::particles::PlantCage`); after the hold it sinks as it rose. Every fifth
//! cage of a cast plays `DRYADE_DORNEN` as it comes, the terror's none.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, PlantCage};
use opensacred_rules::spells::{THORNS_SOUND, THORNS_SOUNDED};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// How far over the ground the glow lies, in world units.
const GROUND_LIFT: f32 = 0.5;

/// The bone a cage stands at (3), else its creature's root.
const NECK: [&str; 1] = ["Bip01 Neck"];

/// A cage to make about its creature ([`make`]): whose it is, its hold's seconds, and whether it
/// sounds.
#[derive(Component)]
struct Unmade {
    target: Entity,
    hero: bool,
    seconds: f32,
    sounded: bool,
}

/// A cage that stands: its creature, its look, its base point in the world, and the meshes of its
/// dots and of its glow.
#[derive(Component)]
struct Cage {
    target: Entity,
    look: PlantCage,
    base: Vec3,
    meshes: [Handle<Mesh>; 2],
}

/// The cages' materials, made as first asked: the vine's dots' and the glow's.
#[derive(Resource, Default)]
struct Looks(Option<[Option<Handle<Effect>>; 2]>);

/// The cages' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x849))
    }
}

/// Makes `count` cages about `target` for its hold of `seconds`, a hero's or a creature's (the
/// event `0x49` each, `0x52634B`), every fifth sounding from the first.
pub(crate) fn cage(
    commands: &mut Commands,
    target: Entity,
    count: u32,
    (hero, seconds): (bool, f32),
) {
    cages(commands, target, count, (hero, seconds), true);
}

/// [`cage`] with no sound, as the hero's terror cages its creature (`0x4E26F1`).
pub(crate) fn cage_quietly(
    commands: &mut Commands,
    target: Entity,
    count: u32,
    (hero, seconds): (bool, f32),
) {
    cages(commands, target, count, (hero, seconds), false);
}

fn cages(
    commands: &mut Commands,
    target: Entity,
    count: u32,
    (hero, seconds): (bool, f32),
    sounds: bool,
) {
    debug!("plant cages: {count} about {target} for {seconds} s");
    for index in 0..count {
        commands.spawn((
            Unmade {
                target,
                hero,
                seconds,
                sounded: sounds && index % THORNS_SOUNDED == 0,
            },
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// Gives each new cage its look at its creature's place on the ground, its meshes, its light at
/// its base and its sound (`0x799720`, `0x79A610`).
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    places: Query<&GlobalTransform>,
    (data, assets, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
    mut sounds: MessageWriter<crate::sfx::PlayAt>,
) {
    for (entity, unmade) in &unmade {
        let Ok(at) = places.get(unmade.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        let [dots, glow] = looks
            .0
            .get_or_insert_with(|| {
                [particles::PLANT_TEXTURE, particles::PLANT_GLOW_TEXTURE].map(|name| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("plant cages: no texture {name}; it is not drawn");
                    }
                    image.map(|image| effects.add(particle_material(image, AlphaMode::Add)))
                })
            })
            .clone();
        let look = PlantCage::new(unmade.seconds, unmade.hero, &mut dice.0);
        let place = at.translation();
        let base = place + world(look.base);
        if unmade.sounded {
            sounds.write(crate::sfx::PlayAt {
                sound: crate::sfx::Of::Id(THORNS_SOUND),
                point: place,
                rate: (0.5, crate::sfx::Roll::None),
            });
        }
        let parts = [meshes.add(empty()), meshes.add(empty())];
        let mut made = commands.entity(entity);
        made.remove::<Unmade>()
            .insert((Transform::IDENTITY, Visibility::default()));
        for (mesh, material) in parts.iter().zip([dots, glow]) {
            if let Some(material) = material {
                made.with_child((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                ));
            }
        }
        // Its Direct3D light at its base, a hero's green, a creature's blue (`0x799E10`).
        let light = crate::fx_lights::PLANTS[usize::from(!unmade.hero)];
        made.commands().spawn(crate::fx_lights::light(
            light,
            base,
            entity,
            (None, Vec3::ZERO),
        ));
        debug!("plant cages: a cage about {} from {base}", unmade.target);
        made.insert(Cage {
            target: unmade.target,
            look,
            base,
            meshes: parts,
        });
    }
}

/// Runs each cage a frame on (`0x799D30`, `0x799E10`): its dots facing the camera from its
/// creature's neck to its base, and its glow flat on the ground at its base; it goes once it is
/// over, or with its creature.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut cages: Query<(Entity, &mut Cage)>,
    (places, children, bones, worn): (
        Query<&GlobalTransform>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let (across, down) = iso::fx_axes();
    for (entity, mut cage) in &mut cages {
        let cage = &mut *cage;
        let Ok(at) = places.get(cage.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        cage.look.update(dt);
        if cage.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let neck = crate::making::bone(cage.target, &NECK, &children, (&bones, &worn))
            .unwrap_or(at.translation());
        let colour = cage.look.colour;
        let mut dots = Quads::default();
        for (point, half) in cage.look.dots(models(cage.base - neck)) {
            dots.push(
                neck + world(point),
                half * s,
                0.0,
                (Vec2::ZERO, 1.0),
                colour,
            );
        }
        let mut glow = Quads::default();
        let half = cage.look.glow() * s;
        if half > 0.0 {
            let ground = cage.base + Vec3::Y * GROUND_LIFT;
            let (a, d) = (across * half, down * half);
            glow.push_corners(
                [
                    ground - a - d,
                    ground + a - d,
                    ground + a + d,
                    ground - a + d,
                ],
                [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                colour,
            );
        }
        for (handle, quads) in cage.meshes.iter().zip([dots, glow]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The plant cages' looks.
pub struct PlantCagesPlugin;

impl Plugin for PlantCagesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_systems(
                Update,
                (make, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
