//! The look of the gust 28 (#1092; docs/re/effects.md, "The gust"): about its bearer for its
//! seconds (`spells`), `cParticleSystem_twister` (826, `0x788B80`) circles 250 puffs of
//! `PARTICLE_CLOUD01` in the colour of the ground under it (`0x524DC2`, `0x41BA00`), laid over by
//! their alpha and tinted by the light at them, their column rising to 100, and once its seconds
//! are over spinning faster and climbing for 2 s more (`opensacred_rules::particles::Twister`).

use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;
use opensacred_rules::particles::{self, Twister, argb};

use crate::frame::Playing;
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;

/// The puffs' texture.
const TEXTURE: &str = "PARTICLE_CLOUD01";

/// A gust's object to give its look: its bearer and its seconds (`spells::strikes_about`).
#[derive(Component)]
pub(crate) struct Look {
    pub bearer: Option<Entity>,
    pub seconds: f32,
}

/// A gust: its puffs and column, its bearer, its place, and its mesh.
#[derive(Component)]
struct Gust {
    twister: Twister,
    bearer: Option<Entity>,
    place: Vec3,
    mesh: Handle<Mesh>,
}

/// The puffs' material, made as the first gust blows; none where its texture is missing.
#[derive(Resource, Default)]
struct Material(Option<Option<Handle<Effect>>>);

/// The gusts' dice.
#[derive(Resource)]
struct Rolls(opensacred_rules::spawning::Rng);

impl Default for Rolls {
    fn default() -> Self {
        Self(crate::seed::dice(0x33A))
    }
}

/// Gives each new gust its look (`0x788D90`), an entity of its own that outlives its object.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn show(
    mut commands: Commands,
    looks: Query<(&Look, &GlobalTransform), Added<Look>>,
    mut rolls: ResMut<Rolls>,
    (data, assets, mut made, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Material>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (look, place) in &looks {
        let material = made
            .0
            .get_or_insert_with(|| {
                let image = data
                    .as_ref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, TEXTURE));
                if image.is_none() {
                    warn!("twisters: no texture {TEXTURE}; the gust is not drawn");
                }
                image.map(|image| materials.add(particle_material(image, AlphaMode::Blend)))
            })
            .clone();
        let mesh = meshes.add(empty());
        let mut gust = commands.spawn((
            Gust {
                twister: Twister::new(look.seconds, &mut rolls.0),
                bearer: look.bearer,
                place: place.translation(),
                mesh: mesh.clone(),
            },
            Transform::IDENTITY,
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        // Tinted by the light at it: drawn with the world under the night's pass.
        if let Some(material) = material {
            gust.with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
            ));
        }
    }
}

/// Runs each gust a frame on (`0x7888E0`, `0x788B80`) about its bearer, in the colour of the
/// ground under it; once it has gone it ends.
fn blow(
    mut commands: Commands,
    time: Res<Time>,
    heights: Res<crate::terrain::Heights>,
    mut gusts: Query<(Entity, &mut Gust)>,
    bearers: Query<&GlobalTransform>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let (across, down) = iso::fx_axes();
    for (entity, mut gust) in &mut gusts {
        let gust = &mut *gust;
        if let Some(place) = gust.bearer.and_then(|bearer| bearers.get(bearer).ok()) {
            gust.place = place.translation();
        }
        gust.twister.update(dt);
        if gust.twister.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let colour = argb(
            heights
                .nature(gust.place.x, gust.place.z)
                .and_then(|nature| particles::GROUND_COLOURS.get(usize::from(nature)))
                .copied()
                .unwrap_or(particles::NO_GROUND_COLOUR),
        );
        let mut quads = Quads::default();
        for puff in &gust.twister.puffs {
            let ([x, y, z], half) = puff.at(gust.twister.height);
            quads.push(
                gust.place + (across * x + down * y + Vec3::Y * z) * s,
                half * s,
                puff.turn,
                (Vec2::ZERO, 1.0),
                colour,
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&gust.mesh) {
            // Just behind its bearer, else at its place, as the original draws the system before
            // the creature (#2188).
            let order = gust
                .bearer
                .map_or_else(|| orders.at(gust.place), |bearer| orders.behind(bearer));
            quads.in_order(order, Vec3::ZERO);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The gusts' looks.
pub struct TwistersPlugin;

impl Plugin for TwistersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Material>()
            .init_resource::<Rolls>()
            .add_systems(
                Update,
                (show, blow)
                    .chain()
                    .in_set(Playing)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
