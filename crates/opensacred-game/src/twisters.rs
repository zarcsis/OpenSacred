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

/// A gust as a save keeps it (#3775): its puffs and column, its bearer by its name, and its
/// place; a load makes it again there, blowing on as it blew.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedGust {
    twister: Twister,
    #[serde(with = "crate::names::optional")]
    bearer: Option<Entity>,
    place: Vec3,
}

impl bevy::ecs::entity::MapEntities for SavedGust {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.bearer = self.bearer.map(|bearer| mapper.get_mapped(bearer));
    }
}

/// The gusts in `world` as a save keeps them (#3775).
pub(crate) fn saved(world: &mut World) -> Vec<SavedGust> {
    let mut gusts = world.query::<&Gust>();
    gusts
        .iter(world)
        .map(|gust| SavedGust {
            twister: gust.twister.clone(),
            bearer: gust.bearer,
            place: gust.place,
        })
        .collect()
}

/// Makes a loaded save's gusts again (#3775).
pub(crate) fn again(world: &mut World, gusts: Vec<SavedGust>) {
    if gusts.is_empty() {
        return;
    }
    if let Err(e) = world.run_system_cached_with(gusts_again, gusts) {
        warn!("twisters: the saved gusts are not made: {e}");
    }
}

/// Makes the saved gusts `gusts` again ([`again`]).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn gusts_again(
    In(gusts): In<Vec<SavedGust>>,
    mut commands: Commands,
    (data, assets, mut made, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Material>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    let material = material(&mut made, data.as_deref(), &assets, &mut materials);
    for saved in gusts {
        spawn_gust(
            &mut commands,
            (material.clone(), meshes.add(empty())),
            (
                saved.twister,
                crate::names::found(saved.bearer),
                saved.place,
            ),
        );
    }
}

/// The gusts' material, made as the first gust shows; none where its texture is missing.
fn material(
    made: &mut Material,
    data: Option<&Data>,
    assets: &AssetServer,
    materials: &mut Assets<Effect>,
) -> Option<Handle<Effect>> {
    made.0
        .get_or_insert_with(|| {
            let image = data.and_then(|data| particle_texture(&data.0.assets, assets, TEXTURE));
            if image.is_none() {
                warn!("twisters: no texture {TEXTURE}; the gust is not drawn");
            }
            image.map(|image| {
                materials.add(crate::fx::among_objects(particle_material(
                    image,
                    AlphaMode::Blend,
                )))
            })
        })
        .clone()
}

/// Makes a gust of `twister` about `bearer` at `place`, of `material` in `mesh`, tinted by the
/// light at it: drawn with the world under the night's pass.
fn spawn_gust(
    commands: &mut Commands,
    (material, mesh): (Option<Handle<Effect>>, Handle<Mesh>),
    (twister, bearer, place): (Twister, Option<Entity>, Vec3),
) {
    let mut gust = commands.spawn((
        Gust {
            twister,
            bearer,
            place,
            mesh: mesh.clone(),
        },
        Transform::IDENTITY,
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
    if let Some(material) = material {
        gust.with_child((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            NoFrustumCulling,
        ));
    }
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
        let material = material(&mut made, data.as_deref(), &assets, &mut materials);
        spawn_gust(
            &mut commands,
            (material, meshes.add(empty())),
            (
                Twister::new(look.seconds, &mut rolls.0),
                look.bearer,
                place.translation(),
            ),
        );
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
                .map_or_else(|| orders.at(gust.place), |bearer| orders.borne_by(bearer));
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
