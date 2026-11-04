//! The look of the Seraphim's blades of light (#1092; docs/re/effects.md, "The blades of light"):
//! about its caster for its seconds (`spells`), `cParticleSystem_holywall` draws (814, `0x7846C0`)
//! a glow on the ground under it, laid over by its alpha, a ring of dots of light at its size and
//! two spirals of them climbing from the ring and turning (`opensacred_rules::particles::Blades`),
//! and a spark at each spiral's top, the dots and sparks facing the camera and added whole; all
//! untinted by the night.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, Blades};
use opensacred_rules::spawning::Rng;

use crate::frame::Playing;
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;

/// How far over the ground the glow lies, in world units, so that the ground's own faces do not
/// cut it.
const GROUND_LIFT: f32 = 0.5;

/// A blades of light's object to give its look, of its caster (`spells::strikes_about`).
#[derive(Component)]
pub(crate) struct Look(pub Entity);

/// A blades of light's look: its turn and height, its radius in its frame (its caster's size,
/// `0x428CE0`), its dice, and its parts' meshes, the glow's, the dots' and the sparks'.
#[derive(Component)]
struct Shown {
    blades: Blades,
    radius: f32,
    dice: Rng,
    parts: [Handle<Mesh>; 3],
}

/// The glow's, the dots' and the sparks' materials, made as the first blades show; none where a
/// texture is missing.
#[derive(Resource, Default)]
struct Materials(Option<[Option<Handle<Effect>>; 3]>);

/// The seed of the looks' `rand()` (**estimate**: any).
const SEED: usize = 0x32E;

/// Gives each new blades of light its look (`0x784C90`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn show(
    mut commands: Commands,
    looks: Query<(Entity, &Look), Added<Look>>,
    sizes: Query<&crate::arrows::Size>,
    (data, assets, mut made, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Materials>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (entity, &Look(caster)) in &looks {
        let parts_materials = made
            .0
            .get_or_insert_with(|| {
                let image = |name: &str| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("light blades: no texture {name}; its part is not drawn");
                    }
                    image
                };
                let disc = image(particles::BLADES_DISC_TEXTURE)
                    .map(|image| materials.add(particle_material(image, AlphaMode::Blend)));
                let [dots, sparks] = [
                    particles::BLADES_DOT_TEXTURE,
                    particles::BLADES_SPARK_TEXTURE,
                ]
                .map(|name| image(name).map(|image| materials.add(whole_material(image))));
                [disc, dots, sparks]
            })
            .clone();
        let parts = [
            meshes.add(empty()),
            meshes.add(empty()),
            meshes.add(empty()),
        ];
        for (mesh, material) in parts.iter().zip(parts_materials) {
            if let Some(material) = material {
                commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                    ChildOf(entity),
                ));
            }
        }
        let radius = sizes
            .get(caster)
            .map_or(crate::arrows::size(0), |size| size.0);
        let index = usize::try_from(entity.index_u32()).unwrap_or(0);
        commands.entity(entity).insert(Shown {
            blades: Blades::default(),
            radius,
            dice: crate::seed::dice(SEED ^ index),
            parts,
        });
    }
}

/// Runs each blades of light's look a frame on (`0x784630`, `0x7846C0`), about its object's place,
/// which goes with its caster.
fn run(time: Res<Time>, mut shown: Query<&mut Shown>, mut meshes: ResMut<Assets<Mesh>>) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let (across, down) = iso::fx_axes();
    // A point of its frame about its place, at the world's scale.
    let at = |[x, y, z]: [f32; 3]| (across * x + down * y + Vec3::Y * z) * s;
    for mut shown in &mut shown {
        let shown = &mut *shown;
        shown.blades.update(dt);
        let mut disc = Quads::default();
        let half = (shown.radius + particles::BLADES_DISC_BEYOND) * s;
        let ground = Vec3::Y * GROUND_LIFT;
        disc.push_corners(
            [
                ground + (-across - down) * half,
                ground + (across - down) * half,
                ground + (across + down) * half,
                ground + (-across + down) * half,
            ],
            [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
            particles::BLADES_DISC_COLOUR,
        );
        let mut dots = Quads::default();
        for dot in shown.blades.dots(shown.radius) {
            dots.push_facing(
                at(dot),
                particles::BLADES_DOT_HALF * s,
                particles::BLADES_DOT_COLOURS,
            );
        }
        let (least, more) = particles::BLADES_SPARK_HALF;
        // The spark's half side rolled at each draw (`rand()` / 32767 · 10 + 15).
        #[expect(clippy::cast_precision_loss, reason = "a roll below 32768")]
        let roll = shown.dice.below(32_768) as f32 / 32_767.0;
        let spark = (least + more * roll) * s;
        let mut sparks = Quads::default();
        for at_top in shown.blades.sparks(shown.radius) {
            sparks.push_facing(at(at_top), spark, [[1.0; 4]; 4]);
        }
        for (handle, quads) in shown.parts.iter().zip([disc, dots, sparks]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The blades of light's looks.
pub struct LightBladesPlugin;

impl Plugin for LightBladesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Materials>().add_systems(
            Update,
            (show, run)
                .chain()
                .in_set(Playing)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
