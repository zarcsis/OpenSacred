//! The look of the Daemon's fear (#1092; docs/re/effects.md, "The fear"): at each of its five
//! objects (`spells`), `cParticleSystem_hellscream` (1012, `0x7B0870`) lays a glow on the ground
//! added whole, a ring spreading over and over added whole, and a column of camera-facing glows
//! of `PARTICLE_GLOW01` stacked up as it fades in, added by their alpha
//! (`opensacred_rules::particles::HellScream`); untinted by the night, its light of 350 with it.
//! Its object gone, it fades out.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, HellScream};

use crate::frame::Playing;
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::terrain::{Heights, Level};
use crate::{daynight, iso};

/// How far over the ground the glow and the ring lie, in world units, so that the ground's own
/// faces do not cut them.
const GROUND_LIFT: f32 = 0.5;

/// An object of the fear to give its look, for its seconds, on its level (`spells`).
#[derive(Component)]
pub(crate) struct Look {
    pub seconds: f32,
    pub level: Level,
}

/// A fear object's look: its object while it lives, its state, its lamp, and its parts' meshes,
/// the glow's, the ring's and the column's.
#[derive(Component)]
struct Shown {
    object: Option<Entity>,
    scream: HellScream,
    lamp: Entity,
    parts: [Handle<Mesh>; 3],
}

/// The materials of the glow, the ring and the column, made as the first fear shows; none where
/// a texture is missing.
#[derive(Resource, Default)]
struct Materials(Option<[Option<Handle<Effect>>; 3]>);

/// Gives each new fear object its look (`0x7B0AF0`): an entity of its own on the ground under
/// it, its lamp and its parts.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn show(
    mut commands: Commands,
    objects: Query<(Entity, &Look, &GlobalTransform), Added<Look>>,
    heights: Res<Heights>,
    (data, assets, mut made, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Materials>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (object, look, place) in &objects {
        let parts_materials = made
            .0
            .get_or_insert_with(|| {
                let image = |name: &str| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("hell screams: no texture {name}; its part is not drawn");
                    }
                    image
                };
                [
                    image(particles::FEAR_GLOW_TEXTURE).map(|i| materials.add(whole_material(i))),
                    image(particles::FEAR_RING_TEXTURE).map(|i| materials.add(whole_material(i))),
                    image(particles::FEAR_COLUMN_TEXTURE)
                        .map(|i| materials.add(particle_material(i, AlphaMode::Add))),
                ]
            })
            .clone();
        let at = place.translation();
        let ground = heights.standing(at.x, at.z, look.level).unwrap_or(at.y);
        let lamp = commands
            .spawn((
                Transform::from_translation(at.with_y(0.0)),
                daynight::Light(particles::FEAR_LIGHT),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let shown = commands
            .spawn((
                Transform::from_translation(at.with_y(ground)),
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let parts: [Handle<Mesh>; 3] = std::array::from_fn(|_| meshes.add(empty()));
        for (mesh, material) in parts.iter().zip(parts_materials) {
            if let Some(material) = material {
                commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                    ChildOf(shown),
                ));
            }
        }
        commands.entity(shown).insert(Shown {
            object: Some(object),
            scream: HellScream::new(look.seconds),
            lamp,
            parts,
        });
    }
}

/// Runs each fear object's look a frame on (`0x7B07B0`, `0x7B0870`); faded out, it goes, its
/// lamp with it.
fn scream(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &GlobalTransform)>,
    objects: Query<(), With<Look>>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let (across, depth) = iso::fx_axes();
    let (right, up) = (iso::right(), iso::up());
    for (entity, mut look, at) in &mut shown {
        let look = &mut *look;
        if look.object.is_some_and(|object| !objects.contains(object)) {
            look.object = None;
            look.scream.end();
        }
        look.scream.update(dt);
        if look.scream.done() {
            commands.entity(look.lamp).despawn();
            commands.entity(entity).despawn();
            continue;
        }
        let ground = Vec3::Y * GROUND_LIFT;
        let mut glow = Quads::default();
        let half = particles::FEAR_GLOW_HALF * s;
        glow.push_corners(
            [
                ground + (-across - depth) * half,
                ground + (across - depth) * half,
                ground + (across + depth) * half,
                ground + (-across + depth) * half,
            ],
            [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
            particles::FEAR_GLOW_COLOUR,
        );
        let mut ring = Quads::default();
        let (inner, outer, colour) = look.scream.ring();
        ring.push_ring(
            ground,
            (inner * s, outer * s),
            particles::FEAR_RING_SEGMENTS,
            colour,
        );
        let mut column = Quads::default();
        for (height, half) in look.scream.column() {
            let centre = Vec3::Y * height * s;
            let (r, u) = (right * half * s, up * half * s);
            column.push_corners(
                [
                    centre - r - u,
                    centre + r - u,
                    centre + r + u,
                    centre - r + u,
                ],
                [Vec2::Y, Vec2::ONE, Vec2::X, Vec2::ZERO],
                particles::FEAR_COLUMN_COLOUR,
            );
        }
        // At its object's place in the order, as the original draws the system (#2188).
        let origin = at.translation();
        let order = orders.at(origin);
        for (handle, mut part) in look.parts.iter().zip([glow, ring, column]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, origin);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The fear's looks.
pub struct HellScreamsPlugin;

impl Plugin for HellScreamsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Materials>().add_systems(
            Update,
            (show, scream)
                .chain()
                .in_set(Playing)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
