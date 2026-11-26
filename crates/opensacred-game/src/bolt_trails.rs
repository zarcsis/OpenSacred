//! The looks of the Mage's whirlwind and of the fire bolt (#1092; docs/re/effects.md, "The
//! whirlwind and the fire bolt"): the whirlwind's (831, `cParticleSystem_windstrike`) dust left
//! in the world along its way in the colour of the ground under its caster, laid over by its
//! alpha and tinted by the light at it; the fire bolt's (922, `cParticleSystem_shaddar_magic`)
//! green smoke riding with it, added and untinted by the night
//! (`opensacred_rules::particles::BoltTrail`). A whirlwind that a cell stops vanishes at once, one
//! flown out its time leaves its dust to settle; a fire bolt's smoke fades where it ends.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, BoltTrail, TrailKind, argb};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;
use crate::terrain::Heights;

/// A bolt's trail, which takes its look ([`show`]).
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Trail(pub(crate) TrailKind);

/// The trails' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x33F))
    }
}

/// The trails' materials by their kind, each made at its first trail; none where its texture is
/// missing.
#[derive(Resource, Default)]
struct Looks {
    dust: Option<Option<Handle<Effect>>>,
    smoke: Option<Option<Handle<Effect>>>,
}

/// A trail's look, which outlives its bolt while its particles live: its bolt while it flies,
/// its particles, its bolt's place and the place before, and its mesh.
#[derive(Component)]
struct Shown {
    bolt: Option<Entity>,
    trail: BoltTrail,
    place: Vec3,
    before: Option<Vec3>,
    mesh: Handle<Mesh>,
}

/// A trail as a save keeps it (#3774): its particles, its place and the place before, and
/// whether its bolt still flies; a load gives a flying bolt's trail to its bolt made again
/// (`bolts::SavedBolt`), and makes one that outlives its bolt again on its own.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedTrail {
    trail: BoltTrail,
    place: Vec3,
    before: Option<Vec3>,
    flies: bool,
}

/// The trails in `world` as a save keeps them (#3774): of each flying bolt by its entity, and
/// those that outlive their bolts.
pub(crate) fn saved(world: &mut World) -> (Vec<(Entity, SavedTrail)>, Vec<SavedTrail>) {
    let mut shown = world.query::<&Shown>();
    let mut flying = Vec::new();
    let mut alone = Vec::new();
    for shown in shown.iter(world) {
        let kept = SavedTrail {
            trail: shown.trail.clone(),
            place: shown.place,
            before: shown.before,
            flies: shown.bolt.is_some(),
        };
        match shown.bolt {
            Some(bolt) => flying.push((bolt, kept)),
            None => alone.push(kept),
        }
    }
    (flying, alone)
}

/// Makes a loaded save's trails that outlive their bolts again (#3774), each on its own.
pub(crate) fn again(world: &mut World, trails: Vec<SavedTrail>) {
    for saved in trails {
        world.spawn((
            Trail(saved.trail.kind),
            Transform::from_translation(saved.place),
            crate::fx::Resumes(saved),
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// Gives each new trail its look (`0x789E10`, `0x78A170`; `0x7AA980`, `0x7AAC20`): the
/// whirlwind's dust takes the colour of the ground where it leaves (`0x52507C`, `0x41BA00`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn show(
    mut commands: Commands,
    trails: Query<
        (
            Entity,
            &Trail,
            &Transform,
            Option<&crate::fx::Resumes<SavedTrail>>,
        ),
        Added<Trail>,
    >,
    heights: Res<Heights>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, trail, place, resumed) in &trails {
        // A loaded trail goes on as it went, one that outlived its bolt on its own (#3774).
        let resumed = resumed.map(|crate::fx::Resumes(saved)| saved);
        let alone = resumed.is_some_and(|saved| !saved.flies);
        let (made, texture, alpha_mode) = match trail.0 {
            TrailKind::Windstrike => (
                &mut looks.dust,
                particles::WINDSTRIKE_TEXTURE,
                AlphaMode::Blend,
            ),
            TrailKind::Shaddar => (&mut looks.smoke, particles::SHADDAR_TEXTURE, AlphaMode::Add),
        };
        let material = made
            .get_or_insert_with(|| {
                let image = data
                    .as_deref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, texture));
                if image.is_none() {
                    warn!("bolt trails: no texture {texture}; its trails are not drawn");
                }
                // The dust, which the world's camera draws, among the objects (#3459).
                image.map(|image| {
                    let material = particle_material(image, alpha_mode);
                    effects.add(match trail.0 {
                        TrailKind::Windstrike => crate::fx::among_objects(material),
                        TrailKind::Shaddar => material,
                    })
                })
            })
            .clone();
        let at = place.translation;
        let colour = argb(
            heights
                .nature(at.x, at.z)
                .and_then(|nature| particles::GROUND_COLOURS.get(usize::from(nature)))
                .copied()
                .unwrap_or(particles::NO_GROUND_COLOUR),
        );
        let mesh = meshes.add(empty());
        let mut shown = commands.spawn((
            Transform::IDENTITY,
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(material) = material {
            let drawn = (
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
            );
            // The dust tinted by the light at it (the flag `0x40`), drawn with the world under
            // the night's pass; the smoke untinted.
            match trail.0 {
                TrailKind::Windstrike => shown.with_child(drawn),
                TrailKind::Shaddar => shown.with_child((drawn, RenderLayers::layer(iso::UNTINTED))),
            };
        }
        // The fire bolt's Direct3D light, at its place while it flies (`0x7AA980`, `0x7AAB90`).
        if matches!(trail.0, TrailKind::Shaddar) && !alone {
            let owner = shown.id();
            shown.commands().spawn(crate::fx_lights::light(
                crate::fx_lights::FIRE,
                crate::fx_lights::LOW,
                at,
                owner,
                (Some(entity), Vec3::ZERO),
            ));
        }
        let [r, g, b, _] = colour;
        shown.insert(Shown {
            bolt: (!alone).then_some(entity),
            trail: resumed.map_or_else(
                || BoltTrail::new(trail.0).with_ground([r, g, b]),
                |saved| saved.trail.clone(),
            ),
            place: resumed.map_or(at, |saved| saved.place),
            before: resumed.and_then(|saved| saved.before),
            mesh,
        });
        if alone {
            commands.entity(entity).despawn();
        } else {
            commands
                .entity(entity)
                .remove::<crate::fx::Resumes<SavedTrail>>();
        }
    }
}

/// Runs each trail a frame on (`0x789EA0`, `0x789F50`; `0x7AAA60`, `0x7AAB90`): its particles
/// along its bolt's way, its end with its bolt's, and its own end with its last particle.
fn blow(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown)>,
    bolts: Query<&Transform, With<Trail>>,
    mut ends: MessageReader<crate::bolt_looks::Ended>,
    (mut dice, mut meshes, orders): (ResMut<Dice>, ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let ramp = BoltTrail::ramp();
    let ended: Vec<crate::bolt_looks::Ended> = ends.read().copied().collect();
    for (entity, mut look) in &mut shown {
        let look = &mut *look;
        match look.bolt.map(|bolt| bolts.get(bolt)) {
            Some(Ok(place)) => look.place = place.translation,
            _ => {
                if let Some(bolt) = look.bolt.take() {
                    // A cell that stops a whirlwind ends its dust at once (`+0x74` bit 2); a fire
                    // bolt's hit or a cell that stops it bursts its last puff, its 2 s run out none
                    // (`0x7AAAC0`, #3457).
                    let end = ended.iter().find(|end| end.bolt == bolt);
                    let at_once =
                        end.is_some_and(|end| end.at_once || (!end.timed && end.hit.is_none()));
                    let timed = end.is_none_or(|end| end.timed);
                    debug!("bolt trails: {bolt} ends, at once {at_once}, timed {timed}");
                    look.trail.end(at_once, timed, &mut dice.0);
                }
            }
        }
        let before = look.before.unwrap_or(look.place);
        look.trail
            .update(dt, (models(before), models(look.place)), &mut dice.0);
        look.before = Some(look.place);
        if look.trail.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut quads = Quads::default();
        for particle in &look.trail.system.particles {
            match look.trail.kind {
                // Its own colour, rolled about the ground's (#3155), at the alpha of its life
                // (the flags `0x64`), the grid 1.
                TrailKind::Windstrike => {
                    let [r, g, b] = particle.colour;
                    quads.push(
                        world(particle.place),
                        particle.size * s,
                        particle.angle,
                        (Vec2::ZERO, 1.0),
                        [r, g, b, particle.life / 255.0],
                    );
                }
                // The ramp's colour, riding with its bolt (the flags `0x10D`, the system flag 2),
                // the first frame of the grid 2.
                TrailKind::Shaddar => quads.push(
                    look.place + world(particle.place),
                    particle.size * s,
                    particle.angle,
                    (Vec2::ZERO, 0.5),
                    ramp.at(particle.life),
                ),
            }
        }
        if let Some(mut mesh) = meshes.get_mut(&look.mesh) {
            // At its place in the order as it flies, which a moving object heads in its cell (#2188).
            quads.in_order(orders.at(look.place), Vec3::ZERO);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The whirlwinds' and the fire bolts' looks.
pub struct BoltTrailsPlugin;

impl Plugin for BoltTrailsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (show, blow)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
