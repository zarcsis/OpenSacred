//! The look of the stargates (802, `FX_STARGATE`, and the add-on's 1180, `FX_STARGATE_UW`; #1092,
//! docs/re/effects.md, "The stargates"): the scripts' gate objects (`npcs`) stand an upright
//! window facing the viewer 95 over their point, a world map's picture in it, its edges fading in
//! a capsule and ripples running out from its middle, its picture changing every 5 s; two whirls
//! turn about it, drawn with their depth not tested; it grows from nothing as it is made, and
//! lights the ground about it by night (`opensacred_rules::particles::Stargate`). None darkens by
//! night. All lie at the gate's place in the original's order of drawing (`fx::Orders::at`,
//! #2220): the window no nearer than a little behind it, the whirls at it whole and after it
//! (#2539), over what the order draws before
//! the gate and under what it draws after (`fx::Quads::at_order`).

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use opensacred_rules::particles::{self, STARGATE_HIGH, STARGATE_WHIRL_ALPHAS, Stargate};

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, linear, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::spawning::Rng;
use crate::{daynight, iso};

/// A stargate's object, which takes its look ([`show`]): whether it is the add-on's.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Gate {
    underworld: bool,
}

impl Gate {
    /// The gate of an FX object of the type `kind`, if it is one.
    pub(crate) fn of(kind: u32) -> Option<Self> {
        match kind {
            802 => Some(Self { underworld: false }),
            1180 => Some(Self { underworld: true }),
            _ => None,
        }
    }
}

/// How far behind the gate's depth in the order its window lies at the nearest, in world units:
/// where its ripples bring it toward the viewer, the window clamped onto the whirls' own depth,
/// written over its whole rectangle, hid them in a band about its picture once in each ripple's
/// 1.26 s (#2539). The depth of a vertex some 150,000 units out is good to some 0.05 units, so
/// that 0.05 still left a frame or two of each ripple; 0.5 left none in 240 frames, and is
/// still a hundredth of a cell's depth.
const UNDER_WHIRLS: f32 = 0.5;

/// How much nearer the whirls sort than the window, and the added whirl than the laid one, in
/// world units of the blended meshes' distance (`StandardMaterial::depth_bias`; it also biases
/// their depth by as many of its least steps, which changes nothing that shows).
const WHIRLS_AFTER: f32 = 1.0;

/// The gates' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x802))
    }
}

/// The materials of a kind of gate: its sixteen pictures', laid over by their alpha, and its
/// whirls', the first laid over and the second added, both at the gate's depth in the order.
#[derive(Clone, Default)]
struct Materials {
    pictures: Vec<Option<Handle<Effect>>>,
    whirls: [Option<Handle<Effect>>; 2],
}

/// The gates' materials, a gate's and an add-on gate's, made at the first of each.
#[derive(Resource, Default)]
struct Looks([Option<Materials>; 2]);

/// A gate's look on its object: its system, its window's two copies' meshes and their entities,
/// and its whirls' mesh.
#[derive(Component)]
struct Shown {
    gate: Stargate,
    underworld: bool,
    windows: [(Handle<Mesh>, Entity); 2],
    whirls: [Handle<Mesh>; 2],
}

/// A window's mesh of no triangles yet, kept in the main world to be rewritten each frame.
fn window_mesh() -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0_f32; 3]; 3]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0_f32; 2]; 3]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0_f32; 4]; 3]);
    mesh.insert_indices(Indices::U32(vec![0, 1, 2]));
    mesh
}

/// Gives each new gate its look (`0x77FBE0`, `0x780BB0`): its window's copies, its whirls and its
/// light on the ground.
fn show(
    mut commands: Commands,
    gates: Query<(Entity, &Gate), Added<Gate>>,
    (data, assets, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, gate) in &gates {
        let kind = usize::from(gate.underworld);
        let made = looks.0[kind]
            .get_or_insert_with(|| {
                let texture = |name: &str| {
                    let image = data
                        .as_deref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("stargates: no texture {name}; its part is not drawn");
                    }
                    image
                };
                let pictures = particles::STARGATE_PICTURES[kind]
                    .iter()
                    .map(|name| {
                        texture(name)
                            .map(|image| effects.add(particle_material(image, AlphaMode::Blend)))
                    })
                    .collect();
                // The whirls after the window, the laid one first, as the original draws them
                // (`0x780790`): all of a gate stand at its one place, so that the blended meshes'
                // sort by their distance ties and drew them in either order, the window over the
                // whirls once in a while (#2539).
                let [laid, added] = particles::STARGATE_WHIRLS[kind];
                let mut whirl = |name, alpha_mode, nearer| {
                    texture(name).map(|image| {
                        let mut material = particle_material(image, alpha_mode);
                        material.base.depth_bias = nearer;
                        effects.add(material)
                    })
                };
                let whirls = [
                    whirl(laid, AlphaMode::Blend, WHIRLS_AFTER),
                    whirl(added, AlphaMode::Add, 2.0 * WHIRLS_AFTER),
                ];
                Materials { pictures, whirls }
            })
            .clone();
        let look = Stargate::new(&mut dice.0);
        // Its two copies of the window, the shown picture and the next over it, and its whirls,
        // all untinted by the night (`+0xA0` unread).
        let windows = look.pictures.map(|picture| {
            let mesh = meshes.add(window_mesh());
            let mut drawn = commands.spawn((
                Mesh3d(mesh.clone()),
                Transform::IDENTITY,
                Visibility::Hidden,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
                ChildOf(entity),
            ));
            if let Some(Some(material)) = made.pictures.get(picture) {
                drawn.insert(MeshMaterial3d(material.clone()));
            }
            (mesh, drawn.id())
        });
        let whirls = made.whirls.clone().map(|material| {
            let mesh = meshes.add(empty());
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
            mesh
        });
        // Its light in the world's table, on the ground at its point (`+0x9C`, `0x5A9C50`).
        commands.spawn((
            daynight::Light(particles::STARGATE_LIGHT),
            Transform::IDENTITY,
            ChildOf(entity),
        ));
        commands.entity(entity).insert(Shown {
            gate: look,
            underworld: gate.underworld,
            windows,
            whirls,
        });
    }
}

/// Runs each gate a frame on (`0x780450`, `0x780790`): its window's ripples and its pictures'
/// levels, each copy shown while its level is above 0, and its whirls, all at its scale about its
/// middle, 95 over its point, at its place in the order (#2220).
fn turn(
    time: Res<Time>,
    mut gates: Query<(&mut Shown, &Transform)>,
    mut windows: Query<(&mut Visibility, Option<&mut MeshMaterial3d<Effect>>)>,
    (looks, mut dice, mut meshes, orders): (Res<Looks>, ResMut<Dice>, ResMut<Assets<Mesh>>, Orders),
) {
    let dt = time.delta_secs();
    let s = iso::MODEL_SCALE;
    let indices = Stargate::triangles();
    for (mut look, place) in &mut gates {
        let look = &mut *look;
        let origin = place.translation;
        let order = orders.at(origin);
        let before = look.gate.pictures;
        look.gate.update(dt, &mut dice.0);
        let scale = look.gate.scale();
        let middle = Vec3::Y * STARGATE_HIGH * s;
        let at = |place: [f32; 3]| middle + world(place) * scale;
        let vertices = look.gate.window();
        for (k, (mesh, entity)) in look.windows.iter().enumerate() {
            let level = look.gate.levels[k];
            if let Ok((mut visibility, material)) = windows.get_mut(*entity) {
                *visibility = if level > 0 && scale > 0.0 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                // A copy's picture changed: its material the new one's.
                let picture = look.gate.pictures[k];
                if picture != before[k]
                    && let Some(mut material) = material
                    && let Some(Some(made)) = looks.0[usize::from(look.underworld)]
                        .as_ref()
                        .and_then(|made| made.pictures.get(picture))
                {
                    material.0 = made.clone();
                }
            }
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                mesh.insert_attribute(
                    Mesh::ATTRIBUTE_POSITION,
                    vertices
                        .iter()
                        .map(|vertex| {
                            let point = at(vertex.place);
                            order
                                .map_or(point, |order| order.behind(point, origin, UNDER_WHIRLS))
                                .to_array()
                        })
                        .collect::<Vec<_>>(),
                );
                mesh.insert_attribute(
                    Mesh::ATTRIBUTE_UV_0,
                    vertices.iter().map(|vertex| vertex.uv).collect::<Vec<_>>(),
                );
                mesh.insert_attribute(
                    Mesh::ATTRIBUTE_COLOR,
                    vertices
                        .iter()
                        .map(|vertex| linear(vertex.colours[k]))
                        .collect::<Vec<_>>(),
                );
                mesh.insert_indices(Indices::U32(indices.clone()));
            }
        }
        for (k, mesh) in look.whirls.iter().enumerate() {
            let mut quads = Quads::default();
            quads.push_corners(
                look.gate.whirl(k).map(at),
                [
                    Vec2::new(0.0, 1.0),
                    Vec2::new(0.0, 0.0),
                    Vec2::new(1.0, 0.0),
                    Vec2::new(1.0, 1.0),
                ],
                [1.0, 1.0, 1.0, STARGATE_WHIRL_ALPHAS[k]],
            );
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                quads.at_order(order, origin);
                quads.into_mesh(&mut mesh);
            }
        }
    }
}

/// The stargates' looks.
pub struct StargatesPlugin;

impl Plugin for StargatesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (show, turn)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
