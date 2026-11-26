//! The `Hardhit`'s ray's sight with a bow (887, `cParticleSystem_hardhitbow`, #2279;
//! docs/re/effects.md, "The `Hardhit`'s ray"): at each step of the ray, 40 apart along the ground
//! from the shooter's left hand (`arrows`), a pale blue line of glows over the step, fading away
//! in a third of a second, and thirty faint green motes spiralling off it
//! (`opensacred_rules::particles::HardRay`); the line added by its alpha, the motes whole, all
//! facing the camera and untinted by the night.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{GLOW_LINE_TEXTURE, HARD_RAY_TEXTURE, HardRay};

use crate::frame::{Playing, Unpaused};
use crate::fx::{
    Effect, Orders, Quads, empty, particle_material, particle_texture, whole_material,
};
use crate::game::{AppState, Data};
use crate::iso;

/// How far each object's segment reaches along the ray either side of its point (`0x5A4F90`).
const HALF_STEP: f32 = 20.0;

/// A ray's step to show: its point and the ray's direction on the ground.
#[derive(Component)]
struct Unmade {
    at: Vec3,
    along: Vec3,
}

/// A step that shows: its look and the meshes of its line and its motes.
#[derive(Component)]
struct Shown {
    look: HardRay,
    parts: [Handle<Mesh>; 2],
}

/// The sights' materials, the line's added by its alpha and the motes' whole; none where a
/// texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<[Option<Handle<Effect>>; 2]>);

/// Lays the object 887 at the point `at` of the ray along `along` on the ground (`0x4DCB80` at
/// its case `0x3E`, the FX event `0x68`).
pub(crate) fn lay(commands: &mut Commands, at: Vec3, along: Vec3) {
    commands.spawn((Unmade { at, along }, DespawnOnExit(AppState::InGame)));
}

/// A ray's step as a save keeps it (#3774): its point and its look, none for one not made yet,
/// with the ray's direction then; a load makes it again there, going on as it went.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedRay {
    at: Vec3,
    along: Vec3,
    look: Option<HardRay>,
}

/// The rays' steps in `world` as a save keeps them (#3774).
pub(crate) fn saved(world: &mut World) -> Vec<SavedRay> {
    let mut shown = world.query::<(&Shown, &Transform)>();
    let mut saved: Vec<SavedRay> = shown
        .iter(world)
        .map(|(shown, at)| SavedRay {
            at: at.translation,
            along: Vec3::X,
            look: Some(shown.look.clone()),
        })
        .collect();
    let mut unmade = world.query::<&Unmade>();
    saved.extend(unmade.iter(world).map(|unmade| SavedRay {
        at: unmade.at,
        along: unmade.along,
        look: None,
    }));
    saved
}

/// Makes a loaded save's rays' steps again (#3774).
pub(crate) fn again(world: &mut World, rays: Vec<SavedRay>) {
    for saved in rays {
        let mut made = world.spawn((
            Unmade {
                at: saved.at,
                along: saved.along,
            },
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(look) = saved.look {
            made.insert(crate::fx::Resumes(look));
        }
    }
}

/// The sights' materials, made at the first.
fn materials(
    looks: &mut Looks,
    data: Option<&Data>,
    assets: &AssetServer,
    effects: &mut Assets<Effect>,
) -> [Option<Handle<Effect>>; 2] {
    looks
        .0
        .get_or_insert_with(|| {
            let texture = |name: &str| {
                let image = data.and_then(|data| particle_texture(&data.0.assets, assets, name));
                if image.is_none() {
                    warn!("hard rays: no texture {name}; its part is not drawn");
                }
                image
            };
            [
                texture(GLOW_LINE_TEXTURE)
                    .map(|image| effects.add(particle_material(image, AlphaMode::Add))),
                texture(HARD_RAY_TEXTURE).map(|image| effects.add(whole_material(image))),
            ]
        })
        .clone()
}

/// Gives each new step its look along its segment in the models' frame, its meshes and its
/// materials (`0x7A1320`, `0x7A0DF0` at its first run).
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade, Option<&crate::fx::Resumes<HardRay>>)>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    let (across, depth) = iso::fx_axes();
    for (entity, unmade, resumed) in &unmade {
        let ground =
            Vec2::new(unmade.along.dot(across), unmade.along.dot(depth)).normalize_or_zero();
        let half = [ground.x * HALF_STEP, ground.y * HALF_STEP, 0.0];
        // A loaded step goes on as it went (#3774).
        let look = match resumed {
            Some(crate::fx::Resumes(look)) => look.clone(),
            None => HardRay::new(half.map(|h| -h), half),
        };
        let materials = materials(&mut looks, data.as_deref(), &assets, &mut effects);
        let parts: [Handle<Mesh>; 2] = std::array::from_fn(|_| meshes.add(empty()));
        let mut made = commands.entity(entity);
        made.remove::<(Unmade, crate::fx::Resumes<HardRay>)>()
            .insert((
                Shown {
                    look,
                    parts: parts.clone(),
                },
                Transform::from_translation(unmade.at),
                Visibility::default(),
            ));
        for (mesh, material) in parts.into_iter().zip(materials) {
            if let Some(material) = material {
                made.with_child((
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

/// Runs each step a frame on (`0x7A0DF0`) and draws it (`0x7A1080`): its line while it fades,
/// its motes, at its place in the order of what stands there; it goes with its last mote.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &Transform)>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, Orders),
) {
    let s = iso::MODEL_SCALE;
    let (across, depth) = iso::fx_axes();
    let frame = |[x, y, z]: [f32; 3]| (across * x + depth * y + Vec3::Y * z) * s;
    let whole = (Vec2::ZERO, 1.0);
    for (entity, mut step, transform) in &mut shown {
        let step = &mut *step;
        step.look.update(time.delta_secs());
        if step.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut quads: [Quads; 2] = std::array::from_fn(|_| Quads::default());
        if let Some((line, from, to)) = step.look.line() {
            for point in line.points(from, to) {
                quads[0].push(frame(point), line.half * s, 0.0, whole, line.colour());
            }
        }
        for (place, size, colour) in step.look.motes() {
            quads[1].push(frame(place), size * s, 0.0, whole, colour);
        }
        let origin = transform.translation;
        let order = orders.at(origin);
        for (handle, mut part) in step.parts.iter().zip(quads) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, origin);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The `Hardhit`'s rays' sights.
pub struct HardRaysPlugin;

impl Plugin for HardRaysPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>().add_systems(
            Update,
            (make, run)
                .chain()
                .in_set(Playing)
                .in_set(Unpaused)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
