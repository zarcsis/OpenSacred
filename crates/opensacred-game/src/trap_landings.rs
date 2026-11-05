//! The looks of what the Dark Elf's trap balls make where they land (#1092; docs/re/effects.md,
//! "The traps' landings"): the pentagram 782 of 1050, a glow on the ground under a ring and a
//! star of twinkling sparks that draw themselves, chased by two comets once armed, bursting into
//! a column of fire or fading out (`opensacred_rules::particles::PentagramKiller`); the
//! confusion 871 of 1051, a turning rune circle, a small one flying up from it and a helix
//! sliding up about it (`ConfusionCircle`); and the ground's poison 792 of 1052, a green stain
//! with a circle turning under it and green smoke rising (`GroundPoison`). The glows and the
//! smoke are added and untinted by the night; the stain lies over the ground in the world's
//! camera.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, ConfusionCircle, GroundPoison, PentagramKiller};

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// How far over the ground the flat parts lie, in world units.
const GROUND_LIFT: f32 = 0.5;

/// What a trap's ball made where it landed, which takes its look: the pentagram of its seconds,
/// the confusion or the ground's poison (`spells`).
#[derive(Component, Debug, Clone, Copy)]
pub(crate) enum Landing {
    Pentagram(f32),
    Confusion,
    Poison,
}

/// The looks' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x1050))
    }
}

/// The materials the looks take, found at the first landing, by their textures: the pentagram's
/// glow, sparks and puffs; the confusion's circle, spinner and helix; the poison's circle, stain
/// and smoke.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    pentagram: [Option<Handle<Effect>>; 3],
    confusion: [Option<Handle<Effect>>; 3],
    poison: [Option<Handle<Effect>>; 3],
}

impl Looks {
    /// Finds the looks' textures at the first look asked.
    fn make(&mut self, data: Option<&Data>, assets: &AssetServer, effects: &mut Assets<Effect>) {
        if self.made {
            return;
        }
        self.made = true;
        let Some(data) = data else {
            return;
        };
        let mut material = |name: &str, make: fn(Handle<Image>) -> Effect| {
            let image = particle_texture(&data.0.assets, assets, name);
            if image.is_none() {
                warn!("trap landings: no texture {name}; it is not drawn");
            }
            image.map(|image| effects.add(make(image)))
        };
        let added = |image| particle_material(image, AlphaMode::Add);
        let blended = |image| particle_material(image, AlphaMode::Blend);
        self.pentagram = [
            material(particles::PENTAGRAM_GLOW, whole_material),
            material(particles::PENTAGRAM_SPARK, added),
            material(particles::PENTAGRAM_PUFF, added),
        ];
        self.confusion = [
            material(particles::CONFUSION_CIRCLE, whole_material),
            material(particles::CONFUSION_SPINNER, whole_material),
            material(particles::CONFUSION_HELIX, added),
        ];
        self.poison = [
            material(particles::POISON_CIRCLE, whole_material),
            material(particles::POISON_STAIN, blended),
            material(particles::POISON_SMOKE, added),
        ];
    }
}

/// A look's systems.
enum System {
    Pentagram(PentagramKiller),
    Confusion(ConfusionCircle),
    Poison(GroundPoison),
}

/// A landing's look, the pentagram's outliving its object while it bursts or fades: its object
/// while it lasts, its system and its parts' meshes.
#[derive(Component)]
struct Shown {
    landing: Option<Entity>,
    system: System,
    meshes: [Handle<Mesh>; 3],
}

/// Gives each new landing its look (`0x7727E0`, `0x778E80`, `0x778760`).
fn show(
    mut commands: Commands,
    landings: Query<(Entity, &Landing, &Transform), Added<Landing>>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes, mut dice): (
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
        ResMut<Dice>,
    ),
) {
    for (entity, landing, place) in &landings {
        looks.make(data.as_deref(), &assets, &mut effects);
        let (system, materials, stain) = match *landing {
            Landing::Pentagram(seconds) => (
                System::Pentagram(PentagramKiller::new(seconds)),
                looks.pentagram.clone(),
                None,
            ),
            Landing::Confusion => (
                System::Confusion(ConfusionCircle::new(&mut dice.0)),
                looks.confusion.clone(),
                None,
            ),
            Landing::Poison => (
                System::Poison(GroundPoison::new()),
                looks.poison.clone(),
                Some(1),
            ),
        };
        let parts: [Handle<Mesh>; 3] = std::array::from_fn(|_| meshes.add(empty()));
        let mut shown = commands.spawn((
            Transform::from_translation(place.translation),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        // The ground poison's Direct3D light at its place (`0x778760`, `0x778980`).
        if matches!(*landing, Landing::Poison) {
            let owner = shown.id();
            shown
                .commands()
                .spawn(crate::fx_lights::child(crate::fx_lights::POISON, owner));
        }
        for (k, (mesh, material)) in parts.iter().zip(materials).enumerate() {
            let Some(material) = material else {
                continue;
            };
            let part = (
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
            );
            // The poison's stain darkens the ground under it, as the ground takes the night.
            if stain == Some(k) {
                shown.with_child(part);
            } else {
                shown.with_child((part, RenderLayers::layer(iso::UNTINTED)));
            }
        }
        shown.insert(Shown {
            landing: Some(entity),
            system,
            meshes: parts,
        });
    }
}

/// A square of the half side `half` flat on the ground about the look's place, turned by `turn`
/// in the models' frame, `height` over it, the whole texture.
fn flat(quads: &mut Quads, (half, turn, height): (f32, f32, f32), colour: [f32; 4]) {
    let (sin, cos) = turn.sin_cos();
    let corner = |(x, y): (f32, f32)| {
        world([
            half * (x * cos - y * sin),
            half * (x * sin + y * cos),
            height,
        ]) + Vec3::Y * GROUND_LIFT
    };
    quads.push_corners(
        [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(corner),
        [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
        colour,
    );
}

/// Runs each landing's look a frame on (`0x772C40` and `0x773020`, `0x778F40` and `0x779020`,
/// `0x7788F0` and `0x778980`); the confusion and the poison go with their objects, the pentagram
/// once it has burst or faded.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown)>,
    landings: Query<(), With<Landing>>,
    (mut dice, mut meshes): (ResMut<Dice>, ResMut<Assets<Mesh>>),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut look) in &mut shown {
        let look = &mut *look;
        let gone = look
            .landing
            .is_none_or(|landing| !landings.contains(landing));
        if gone {
            look.landing = None;
        }
        let mut quads: [Quads; 3] = std::array::from_fn(|_| Quads::default());
        match &mut look.system {
            System::Pentagram(pentagram) => {
                pentagram.update(dt, gone, &mut dice.0);
                if pentagram.done() {
                    commands.entity(entity).despawn();
                    continue;
                }
                flat(
                    &mut quads[0],
                    (particles::PENTAGRAM_GLOW_HALF * s, 0.0, 0.0),
                    particles::PENTAGRAM_GLOW_COLOUR,
                );
                for spark in pentagram.sparks(&mut dice.0) {
                    quads[1].push(
                        world(spark),
                        particles::PENTAGRAM_SPARK_HALF * s,
                        0.0,
                        (Vec2::ZERO, 1.0),
                        particles::PENTAGRAM_SPARK_COLOUR,
                    );
                }
                let ramp = PentagramKiller::ramp();
                for puff in &pentagram.system.particles {
                    quads[2].push(
                        world(puff.place),
                        puff.size * s,
                        puff.angle,
                        (Vec2::ZERO, 1.0),
                        ramp.at(puff.life),
                    );
                }
            }
            System::Confusion(confusion) => {
                if gone {
                    commands.entity(entity).despawn();
                    continue;
                }
                confusion.update(dt, &mut dice.0);
                let (big, small, rise) = confusion.circles();
                flat(
                    &mut quads[0],
                    (particles::CONFUSION_CIRCLE_HALF * s, big, 0.0),
                    [1.0; 4],
                );
                flat(
                    &mut quads[1],
                    (particles::CONFUSION_SPINNER_HALF * s, small, rise),
                    [1.0; 4],
                );
                let [r, g, b] = particles::CONFUSION_HELIX_COLOUR;
                crate::beacons::ribbon_quads(
                    &mut quads[2],
                    &confusion.helix(),
                    [r, g, b, confusion.alpha()],
                );
            }
            System::Poison(poison) => {
                if gone {
                    commands.entity(entity).despawn();
                    continue;
                }
                poison.update(dt, &mut dice.0);
                flat(
                    &mut quads[0],
                    (particles::POISON_CIRCLE_HALF * s, poison.turn(), 0.0),
                    particles::POISON_CIRCLE_COLOUR,
                );
                flat(
                    &mut quads[1],
                    (particles::POISON_STAIN_HALF * s, 0.0, 0.0),
                    [1.0; 4],
                );
                let ramp = GroundPoison::ramp();
                for puff in &poison.system.particles {
                    let corner =
                        Vec2::new(f32::from(puff.frame % 2), f32::from(puff.frame / 2 % 2));
                    quads[2].push(
                        world(puff.place),
                        puff.size * s,
                        0.0,
                        (corner * 0.5, 0.5),
                        ramp.at(puff.life),
                    );
                }
            }
        }
        for (mesh, quads) in look.meshes.iter().zip(quads) {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The traps' landings' looks.
pub struct TrapLandingsPlugin;

impl Plugin for TrapLandingsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (show, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
