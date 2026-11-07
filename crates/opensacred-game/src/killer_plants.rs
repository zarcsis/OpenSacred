//! The look of the thorns' plants (848, `FX_KILLERPLANTS`, #1092; docs/re/effects.md, "The
//! thorns' plants"): each thorn `spells` makes grows a vine of dots facing the camera from its
//! foot up over a second, sways as two waves climb it, sticks seven spurs straight out along its
//! bends and lays a glow flat on the ground under it, all added by their alpha in its colour,
//! green a hero's and blue a creature's, untinted by the night
//! (`opensacred_rules::particles::KillerPlant`); after its seconds it sinks as it rose, and goes
//! with its thorn.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, KillerPlant};

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;
use crate::spells::Thorn;

/// How far over the ground the glow lies, in world units.
const GROUND_LIFT: f32 = 0.5;

/// The plants' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x848))
    }
}

/// The materials the looks take, found at the first thorn: the vine's dots' and the glow's.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    vine: Option<Handle<Effect>>,
    glow: Option<Handle<Effect>>,
}

/// A thorn's plant: its growth and the meshes of its dots and of its glow.
#[derive(Component)]
struct Plant {
    plant: KillerPlant,
    dots: Handle<Mesh>,
    glow: Handle<Mesh>,
}

/// Gives each new thorn its plant (`0x798820`, `0x799640`).
fn grow(
    mut commands: Commands,
    thorns: Query<(Entity, &Thorn), Added<Thorn>>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes, mut dice): (
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
        ResMut<Dice>,
    ),
) {
    for (entity, thorn) in &thorns {
        if !looks.made {
            looks.made = true;
            if let Some(data) = data.as_deref() {
                let mut material = |name: &str| {
                    let image = particle_texture(&data.0.assets, &assets, name);
                    if image.is_none() {
                        warn!("killer plants: no texture {name}; it is not drawn");
                    }
                    image.map(|image| effects.add(particle_material(image, AlphaMode::Add)))
                };
                looks.vine = material(particles::PLANT_TEXTURE);
                looks.glow = material(particles::PLANT_GLOW_TEXTURE);
            }
        }
        let (dots, glow) = (meshes.add(empty()), meshes.add(empty()));
        let mut shown = commands.entity(entity);
        // Its Direct3D light at its place, a hero's green, a creature's blue (`0x799640`).
        shown.commands().spawn(crate::fx_lights::child(
            crate::fx_lights::PLANTS[usize::from(!thorn.hero)],
            entity,
        ));
        shown.insert(Plant {
            plant: KillerPlant::new(thorn.stands, thorn.hero, &mut dice.0),
            dots: dots.clone(),
            glow: glow.clone(),
        });
        for (mesh, material) in [(dots, looks.vine.clone()), (glow, looks.glow.clone())] {
            if let Some(material) = material {
                shown.with_child((
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

/// Runs each plant a frame on (`0x798E50`, `0x798F10`): its dots facing the camera about its
/// thorn, and its glow flat on the ground, of the half side its rise gives it.
fn run(
    time: Res<Time>,
    mut plants: Query<(Entity, &mut Plant, &GlobalTransform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    orders: crate::fx::Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let (across, down) = iso::fx_axes();
    let lift = Vec3::Y * GROUND_LIFT;
    for (entity, mut plant, place) in &mut plants {
        // Just behind the plant's model, as the original draws the system before the creature
        // (#2188).
        let (order, origin) = (orders.behind(entity), place.translation());
        let Plant { plant, dots, glow } = &mut *plant;
        plant.update(dt);
        let colour = plant.colour;
        let mut quads = Quads::default();
        for (at, half) in plant.dots() {
            quads.push(world(at), half * s, 0.0, (Vec2::ZERO, 1.0), colour);
        }
        if let Some(mut mesh) = meshes.get_mut(&*dots) {
            quads.in_order(order, origin);
            quads.into_mesh(&mut mesh);
        }
        let mut quads = Quads::default();
        let half = plant.glow() * s;
        if half > 0.0 {
            let (a, d) = (across * half, down * half);
            quads.push_corners(
                [lift - a - d, lift + a - d, lift + a + d, lift - a + d],
                [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                colour,
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&*glow) {
            quads.in_order(order, origin);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The thorns' plants' looks.
pub struct KillerPlantsPlugin;

impl Plugin for KillerPlantsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (grow, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
