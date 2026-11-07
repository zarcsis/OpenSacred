//! The goblin mages' ball's impacts (786, `cParticleSystem_goblinimpact`, #2147; docs/re/effects.md,
//! "The goblin mages' ball"): a ball that lands on the hero cracks the ground under her with
//! lightning for 2 s, laid over by their alpha, a pale blue glow shrinking over them; one that
//! lands on no creature traces a ring of blue streaks once about its place, as `creature_casts`
//! summons its creature there (`opensacred_rules::particles::{GoblinImpact, GoblinLanding}`).
//! Each lights the night by 200 while it lasts, and none is tinted by it.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    GOBLIN_IMPACT_CRACKS, GOBLIN_IMPACT_GLOW, GOBLIN_IMPACT_GLOW_COLOUR, GOBLIN_IMPACT_LIGHT,
    GOBLIN_IMPACT_STREAKS, GOBLIN_STREAK_WIDTH, GoblinImpact, GoblinLanding, Particle,
};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;

/// How far over the ground the impacts lie, in world units.
const GROUND_LIFT: f32 = 0.5;

/// An impact to make at its entity's place on the ground ([`make`]).
#[derive(Component)]
struct Unmade(GoblinLanding);

/// Starts an impact of `landing` at `place`, a point on the ground.
pub fn spawn(commands: &mut Commands, place: Vec3, landing: GoblinLanding) {
    commands.spawn((
        Unmade(landing),
        crate::daynight::Light(GOBLIN_IMPACT_LIGHT),
        Transform::from_translation(place),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
}

/// An impact that lasts: its look and the meshes of its particles and of its glow.
#[derive(Component)]
struct Impact {
    look: GoblinImpact,
    meshes: [Handle<Mesh>; 2],
}

/// The impacts' materials, made as first asked: the cracks laid over by their alpha, the glow
/// added whole, the streaks added by their alpha; none where a texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<[Option<Handle<Effect>>; 3]>);

/// The impacts' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x786))
    }
}

/// Gives each new impact its look, its meshes and its materials, in the camera that draws after
/// the night's pass.
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, &Unmade(landing)) in &unmade {
        let materials = looks
            .0
            .get_or_insert_with(|| {
                let mut material = |name: &str, alpha: Option<AlphaMode>| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("goblin_impacts: no texture {name}; its part is not drawn");
                    }
                    image.map(|image| {
                        effects.add(alpha.map_or_else(
                            || whole_material(image.clone()),
                            |alpha| particle_material(image.clone(), alpha),
                        ))
                    })
                };
                [
                    material(GOBLIN_IMPACT_CRACKS, Some(AlphaMode::Blend)),
                    material(GOBLIN_IMPACT_GLOW, None),
                    material(GOBLIN_IMPACT_STREAKS, Some(AlphaMode::Add)),
                ]
            })
            .clone();
        let [cracks, glow, streaks] = materials;
        let particles = match landing {
            GoblinLanding::Hit => cracks,
            GoblinLanding::Summons(_) => streaks,
        };
        let parts = [meshes.add(empty()), meshes.add(empty())];
        let mut impact = commands.entity(entity);
        impact.remove::<Unmade>().insert(Impact {
            look: GoblinImpact::new(landing),
            meshes: parts.clone(),
        });
        for (mesh, material) in parts.into_iter().zip([particles, glow]) {
            if let Some(material) = material {
                impact.with_child((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                ));
            }
        }
    }
}

/// A crack flat on the ground (`0x761B00` with the identity matrix and the flags `0x18C`): from
/// its particle's place out to twice its size along its angle, its size to each side, of its
/// frame of the 2 × 2 grid.
fn crack(quads: &mut Quads, particle: &Particle, colour: [f32; 4]) {
    let (sin, cos) = particle.angle.sin_cos();
    let [x, y, _] = particle.place;
    let size = particle.size;
    let corner = |(across, out): (f32, f32)| {
        world([
            x + across * cos - out * sin,
            y + across * sin + out * cos,
            0.0,
        ]) + Vec3::Y * GROUND_LIFT
    };
    let cell = Vec2::new(
        f32::from(particle.frame % 2),
        f32::from(particle.frame / 2 % 2),
    ) * 0.5;
    quads.push_corners(
        [
            (-size, 0.0),
            (size, 0.0),
            (size, 2.0 * size),
            (-size, 2.0 * size),
        ]
        .map(corner),
        [
            cell + Vec2::new(0.0, 0.5),
            cell + Vec2::new(0.5, 0.5),
            cell + Vec2::new(0.5, 0.0),
            cell,
        ],
        colour,
    );
}

/// A streak upright in the frame's `x`–`z` plane (`0x761B00` turned about `x` by π/2, the flags
/// `0x89`): from its particle's place up to twice its size, an eighth of its size to each side.
fn streak(quads: &mut Quads, particle: &Particle, colour: [f32; 4]) {
    let [x, y, z] = particle.place;
    let (half, high) = (GOBLIN_STREAK_WIDTH * particle.size, 2.0 * particle.size);
    let corner = |(across, up): (f32, f32)| world([x + across, y, z + up]);
    quads.push_corners(
        [(-half, 0.0), (half, 0.0), (half, high), (-half, high)].map(corner),
        [Vec2::Y, Vec2::ONE, Vec2::X, Vec2::ZERO],
        colour,
    );
}

/// Runs each impact a frame on (`0x774730`, `0x774910`): its particles and its glow, a square
/// flat on the ground of its half side while it is above 0; it goes once it is over.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut impacts: Query<(Entity, &mut Impact, &GlobalTransform)>,
    (mut meshes, mut dice, orders): (ResMut<Assets<Mesh>>, ResMut<Dice>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut impact, at) in &mut impacts {
        impact.look.update(dt, &mut dice.0);
        if impact.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let ramp = impact.look.ramp();
        let mut particles = Quads::default();
        for particle in &impact.look.system.particles {
            let colour = ramp.at(particle.life);
            match impact.look.landing {
                GoblinLanding::Hit => crack(&mut particles, particle, colour),
                GoblinLanding::Summons(_) => streak(&mut particles, particle, colour),
            }
        }
        let mut glow = Quads::default();
        let half = impact.look.glow;
        if impact.look.landing == GoblinLanding::Hit && half > 0.0 {
            let corner =
                |(x, y): (f32, f32)| world([half * x, half * y, 0.0]) + Vec3::Y * GROUND_LIFT;
            glow.push_corners(
                [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(corner),
                [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                GOBLIN_IMPACT_GLOW_COLOUR,
            );
        }
        // At its object's place in the order, as the original draws the system (#2188).
        let origin = at.translation();
        let order = orders.at(origin);
        for (handle, mut quads) in impact.meshes.iter().zip([particles, glow]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                quads.in_order(order, origin);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The goblin mages' ball's impacts.
pub struct GoblinImpactsPlugin;

impl Plugin for GoblinImpactsPlugin {
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
