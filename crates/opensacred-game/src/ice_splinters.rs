//! The look of the ice splinters (839, `FX_ICEBULLETS`, #1092; docs/re/effects.md, "The ice
//! splinters"): each splinter bolt shows five icicles about its place, laid along its flight and
//! turning about it, in the hero's pale texture or another caster's, each trailing four rings
//! that grow behind it, a hero's added in grey, another's laid over in black; at its time's end
//! or a cell that stops it the icicles burst into blue-white sparks, a creature's grey, that fall
//! and bounce off the ground (`opensacred_rules::particles::IceSplinters`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, IceSplinters};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{
    Effect, Quads, empty, particle_material, particle_texture, texture_path, whole_material,
};
use crate::game::{AppState, Data};
use crate::iso;
use crate::models as model_looks;
use crate::npcs::Retexture;
use crate::spawning::Rng;
use crate::terrain::Heights;

/// A splinter bolt, which takes its look ([`show`]): whether a hero cast it, and its direction
/// on the ground.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Splinters {
    pub(crate) hero: bool,
    pub(crate) direction: Vec2,
}

/// The splinters' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x839))
    }
}

/// What the looks take, found at the first splinters: the icicle's model and the others'
/// texture, relative to the asset directory, the rings' materials, a hero's and another's, and
/// the sparks'.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    model: Option<String>,
    others: Option<String>,
    rings: Option<[Handle<Effect>; 2]>,
    sparks: Option<Handle<Effect>>,
}

impl Looks {
    /// Finds the looks' files at the first look asked.
    fn make(&mut self, data: Option<&Data>, assets: &AssetServer, effects: &mut Assets<Effect>) {
        if self.made {
            return;
        }
        self.made = true;
        let Some(data) = data else {
            return;
        };
        let shared = &data.0;
        self.model = crate::npcs::model_files(&shared.assets)
            .ok()
            .and_then(|files| files.get(particles::SPLINTER_MODEL).cloned());
        if self.model.is_none() {
            warn!(
                "ice splinters: no {}; their icicles are not drawn",
                particles::SPLINTER_MODEL
            );
        }
        self.others = texture_path(&shared.assets, particles::SPLINTER_OTHERS_TEXTURE);
        let texture = |name: &str| {
            let image = particle_texture(&shared.assets, assets, name);
            if image.is_none() {
                warn!("ice splinters: no texture {name}; its part is not drawn");
            }
            image
        };
        // A hero's rings added whole (`ONE`, `ONE`), another's laid over by their alpha.
        self.rings = texture(particles::SPLINTER_RING_TEXTURE).map(|image| {
            [
                effects.add(whole_material(image.clone())),
                effects.add(particle_material(image, AlphaMode::Blend)),
            ]
        });
        self.sparks = texture(particles::SPLINTER_SPARK_TEXTURE)
            .map(|image| effects.add(whole_material(image)));
    }
}

/// A splinters' look, which outlives its bolt while its sparks live: its bolt while it flies,
/// its system, its icicles, its place, its direction in the world, and its meshes of the rings
/// and of the sparks.
#[derive(Component)]
struct Shown {
    bolt: Option<Entity>,
    look: IceSplinters,
    icicles: Vec<Entity>,
    place: Vec3,
    along: Vec3,
    meshes: [Handle<Mesh>; 2],
}

/// Gives each new splinter bolt its look (`0x78CA30`, `0x78D640`): its five icicles, its rings
/// and its sparks.
fn show(
    mut commands: Commands,
    bolts: Query<(Entity, &Splinters, &Transform), Added<Splinters>>,
    (data, assets, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (bolt, splinters, place) in &bolts {
        looks.make(data.as_deref(), &assets, &mut effects);
        let at = place.translation;
        let look = IceSplinters::new(models(at), splinters.hero, &mut dice.0);
        let along =
            Vec3::new(splinters.direction.x, 0.0, splinters.direction.y).normalize_or(Vec3::X);
        let icicles = looks
            .model
            .as_ref()
            .map(|model| {
                look.offsets
                    .iter()
                    .map(|&offset| {
                        let mut icicle = commands.spawn((
                            model_looks::scene(&assets, model),
                            icicle_transform(at + world(offset), along, 0.0),
                            model_looks::Ordered::default(),
                            DespawnOnExit(AppState::InGame),
                        ));
                        // Another caster's in its own texture (`0x401BD0`).
                        if !splinters.hero
                            && let Some(texture) = &looks.others
                        {
                            icicle.insert(Retexture::load(&assets, texture));
                        }
                        icicle.id()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let parts = [meshes.add(empty()), meshes.add(empty())];
        let mut shown = commands.spawn((
            Transform::IDENTITY,
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        // Unlit and untinted by the night, as the beams' quads.
        let ring = looks
            .rings
            .as_ref()
            .map(|[hero, other]| if splinters.hero { hero } else { other }.clone());
        for (mesh, material) in parts.iter().zip([ring, looks.sparks.clone()]) {
            if let Some(material) = material {
                shown.with_child((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                ));
            }
        }
        shown.insert(Shown {
            bolt: Some(bolt),
            look,
            icicles,
            place: at,
            along,
            meshes: parts,
        });
    }
}

/// An icicle's transform at `at` (`0x78CF50`): its point, which the model's `+Z` holds, along
/// `along`, turned `turn` radians about it, at the models' scale.
fn icicle_transform(at: Vec3, along: Vec3, turn: f32) -> Transform {
    Transform::from_translation(at)
        .looking_to(-along, Vec3::Y)
        .with_scale(Vec3::splat(iso::MODEL_SCALE))
        * Transform::from_rotation(Quat::from_rotation_z(turn))
}

/// Runs each splinters' look a frame on (`0x78CD10`, `0x78CF50`): its icicles along its bolt's
/// way and their rings, its burst where its bolt ends, its sparks, and its end with its last.
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    heights: Res<Heights>,
    mut shown: Query<(Entity, &mut Shown)>,
    bolts: Query<&Transform, (With<Splinters>, Without<Shown>)>,
    mut icicles: Query<&mut Transform, (Without<Splinters>, Without<Shown>)>,
    (mut dice, mut meshes, orders): (ResMut<Dice>, ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut look) in &mut shown {
        let look = &mut *look;
        match look.bolt.map(|bolt| bolts.get(bolt)) {
            Some(Ok(place)) => look.place = place.translation,
            _ => {
                if look.bolt.take().is_some() {
                    // Their time's end or a cell that stops them (`+0x74` bit 2): they burst.
                    let along = Vec3::from_array(models(look.along)).normalize_or_zero();
                    look.look
                        .burst(models(look.place), along.to_array(), &mut dice.0);
                    for icicle in look.icicles.drain(..) {
                        commands.entity(icicle).despawn();
                    }
                }
            }
        }
        let ground = heights
            .at(look.place.x, look.place.z)
            .unwrap_or(look.place.y - crate::bolts::HEIGHT);
        look.look.update(dt, models(look.place), ground / s);
        if look.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        for (icicle, offset) in look.icicles.iter().zip(look.look.offsets) {
            if let Ok(mut transform) = icicles.get_mut(*icicle) {
                *transform =
                    icicle_transform(look.place + world(offset), look.along, look.look.clock);
            }
        }
        // The rings in the plane across the flight, turned with their icicles (the quad's local
        // `x` and `z`, `+0x2660`).
        let turn = icicle_transform(Vec3::ZERO, look.along, look.look.clock).rotation;
        let (a, b) = (turn * Vec3::X, turn * Vec3::Y);
        let mut rings = Quads::default();
        for (centre, half, share) in look.look.rings() {
            let (centre, h) = (world(centre), half * s);
            let colour = if look.look.hero {
                let grey = (128.0 * share).floor() / 255.0;
                [grey, grey, grey, 1.0]
            } else {
                [0.0, 0.0, 0.0, (128.0 * share).floor() / 255.0]
            };
            rings.push_corners(
                [
                    centre - a * h - b * h,
                    centre + a * h - b * h,
                    centre + a * h + b * h,
                    centre - a * h + b * h,
                ],
                [
                    Vec2::new(0.0, 1.0),
                    Vec2::new(1.0, 1.0),
                    Vec2::new(1.0, 0.0),
                    Vec2::new(0.0, 0.0),
                ],
                colour,
            );
        }
        let ramp = look.look.ramp();
        let mut sparks = Quads::default();
        for spark in &look.look.system.particles {
            sparks.push(
                world(spark.place),
                spark.size * s,
                spark.angle,
                (Vec2::ZERO, 1.0),
                ramp.at(spark.life),
            );
        }
        // At its object's place in the order, which a moving object heads in its cell (#2188).
        let order = orders.at(look.place);
        for (mut quads, mesh) in [rings, sparks].into_iter().zip(&look.meshes) {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.in_order(order, Vec3::ZERO);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The ice splinters' looks.
pub struct IceSplintersPlugin;

impl Plugin for IceSplintersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (show, fly)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
