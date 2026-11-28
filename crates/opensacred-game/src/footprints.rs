//! The footprints (#3066; docs/re/drawing.md, "The footprints"): at each step of a creature on the
//! ground, the hero's and every other's whose type lays them
//! (`opensacred_rules::particles::lays_footprints`), a square of 26.9 world units lies flat at the
//! height it stands at, its middle 19 ahead of it, turned by its facing leaning toward the
//! screen's level (`footprint_axes`): the desert's print on `DESERT`, the snow's on `ARCTIC`, laid
//! over by their alpha for 8.3 s and gone at once, and on either water the ripples, their five
//! frames added at 7 a second; none on any other ground, the swamp's among them. At most 64 lie at
//! once, the oldest giving way, and one to a cell: a step into a cell where one lies lays none
//! (docs/decisions/drawing.md, "One footprint a cell, none waiting"). They lie in list 0 under
//! every sprite and model, tinted by the night as the ground (`marks`' order). None for a creature
//! aloft, the hero in a winged form, the Big Demon or the hero of six pieces of set 23, whose
//! steps leave fires (`fx`).
//!
//! Not followed: a footprint on a building's floor, whose cells' natures the steps do not read.

use std::collections::VecDeque;

use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;
use opensacred_rules::particles::{
    FOOTPRINT_AHEAD, FOOTPRINT_SIDE, FOOTPRINTS, PRINT_TEXTURES, Print, RIPPLE_FRAMES, RIPPLE_RATE,
    RIPPLE_TEXTURES, footprint_axes,
};

use crate::blended::Layer;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, layered, particle_material, particle_texture};
use crate::game::{AppState, Data};

/// A step that may lay a footprint: where its creature stands and the way it faces, on the ground.
#[derive(Message, Clone, Copy, Debug)]
pub struct Step {
    pub at: Vec3,
    pub facing: Vec3,
}

/// The materials, found at the first footprint: the two prints' and the ripples' five frames'.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    prints: [Option<Handle<Effect>>; 2],
    ripples: Vec<Option<Handle<Effect>>>,
}

/// The footprints lying, oldest first, each its cell and its entity.
#[derive(Resource, Default)]
struct Lying(VecDeque<((i32, i32), Entity)>);

/// A footprint: its look, how long it has lain, its square's corners about its middle, and its
/// square's child with its mesh.
#[derive(Component)]
struct Footprint {
    print: Print,
    age: f32,
    corners: [Vec3; 4],
    part: Entity,
    mesh: Handle<Mesh>,
}

/// Finds the materials at the first footprint (`0x6392AE`).
fn find_looks(looks: &mut Looks, data: &Data, server: &AssetServer, effects: &mut Assets<Effect>) {
    looks.made = true;
    let assets = &data.0.assets;
    let material = |name: &str, alpha: AlphaMode, effects: &mut Assets<Effect>| {
        let image = particle_texture(assets, server, name);
        if image.is_none() {
            warn!("footprints: no texture {name}; its footprints are not drawn");
        }
        image.map(|image| effects.add(layered(particle_material(image, alpha), Layer::Underlays)))
    };
    looks.prints = PRINT_TEXTURES.map(|name| material(name, AlphaMode::Blend, effects));
    looks.ripples = RIPPLE_TEXTURES
        .iter()
        .map(|name| material(name, AlphaMode::Add, effects))
        .collect();
}

/// The footsteps of the hero, which lay footprints but of the fires of set 23 (`0x639080`,
/// `0x63918C`); the Daemon's form of 1094 tells none (`sound::HeroStep`).
fn hero_steps(
    mut steps: MessageReader<crate::sound::HeroStep>,
    heroes: Query<(&Transform, Option<&crate::fight::Holding>), With<crate::hero::Body>>,
    mut prints: MessageWriter<Step>,
) {
    let Some((transform, holding)) = heroes.iter().next() else {
        steps.clear();
        return;
    };
    let fiery = holding.is_some_and(|holding| holding.0.sets.fiery_steps());
    for &crate::sound::HeroStep(at) in steps.read() {
        if !fiery {
            prints.write(Step {
                at,
                facing: transform.rotation * Vec3::X,
            });
        }
    }
}

/// Lays a footprint at each step on a ground of a print, one to a cell, the oldest of 64 giving
/// way (`0x41EA30`, `0x41EAF0`).
fn lay(
    mut commands: Commands,
    mut steps: MessageReader<Step>,
    heights: Res<crate::terrain::Heights>,
    (mut lying, mut looks, data, server): (
        ResMut<Lying>,
        ResMut<Looks>,
        Option<Res<Data>>,
        Res<AssetServer>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for step in steps.read() {
        let Some(print) = heights
            .step_nature(step.at.x, step.at.z)
            .and_then(Print::of)
        else {
            continue;
        };
        let cell = crate::paths::cell_of(step.at.x, step.at.z);
        if lying.0.iter().any(|&(lies, _)| lies == cell) {
            continue;
        }
        let Some((forward, right)) = footprint_axes([step.facing.x, step.facing.z]) else {
            continue;
        };
        if !looks.made {
            let Some(data) = data.as_deref() else {
                return;
            };
            find_looks(&mut looks, data, &server, &mut effects);
        }
        let material = match print {
            Print::Desert => looks.prints[0].clone(),
            Print::Snow => looks.prints[1].clone(),
            Print::Ripple => looks.ripples.first().cloned().flatten(),
        };
        let Some(material) = material else {
            continue;
        };
        if lying.0.len() >= FOOTPRINTS
            && let Some((_, oldest)) = lying.0.pop_front()
        {
            commands.entity(oldest).try_despawn();
        }
        // Its square about its middle ahead of the creature, the texture's top its far edge and
        // its `u` from the creature's left.
        let forward = Vec3::new(forward[0], 0.0, forward[1]);
        let right = Vec3::new(right[0], 0.0, right[1]);
        let middle = step.at + forward * FOOTPRINT_AHEAD;
        let half = FOOTPRINT_SIDE / 2.0;
        let corners = [
            (forward - right) * half,
            (forward + right) * half,
            (right - forward) * half,
            (-forward - right) * half,
        ];
        let mesh = meshes.add(empty());
        let part = commands
            .spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
            ))
            .id();
        let entity = commands
            .spawn((
                Footprint {
                    print,
                    age: 0.0,
                    corners,
                    part,
                    mesh,
                },
                Transform::from_translation(middle),
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ))
            .add_child(part)
            .id();
        lying.0.push_back((cell, entity));
    }
}

/// Ages each footprint (`0x41EC49`): a print goes after its 8.3 s, the ripples turn their frames
/// at 7 a second and go after the last; each lies in list 0 at its cell's place
/// (`fx::Orders::in_list_0`).
fn age(
    mut commands: Commands,
    time: Res<Time>,
    mut footprints: Query<(Entity, &mut Footprint, &GlobalTransform)>,
    mut parts: Query<&mut MeshMaterial3d<Effect>>,
    (mut lying, looks, orders, mut assets): (
        ResMut<Lying>,
        Res<Looks>,
        Orders,
        ResMut<Assets<Mesh>>,
    ),
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut footprint, at) in &mut footprints {
        footprint.age += dt;
        if footprint.age >= footprint.print.seconds() {
            commands.entity(entity).despawn();
            lying.0.retain(|&(_, lies)| lies != entity);
            continue;
        }
        if footprint.print == Print::Ripple
            && let Ok(mut material) = parts.get_mut(footprint.part)
        {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a frame of 0 to 4 by the ripples' age of less than 0.72 s"
            )]
            let frame =
                ((footprint.age * RIPPLE_RATE) as usize).min(usize::from(RIPPLE_FRAMES) - 1);
            if let Some(Some(next)) = looks.ripples.get(frame)
                && material.0 != *next
            {
                material.0 = next.clone();
            }
        }
        if let Some(mut mesh) = assets.get_mut(&footprint.mesh) {
            let origin = at.translation();
            let mut quads = Quads::default();
            quads.push_corners(
                footprint.corners,
                [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                [1.0; 4],
            );
            quads.at_order(orders.in_list_0(origin), origin);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The footprints.
pub struct FootprintsPlugin;

impl Plugin for FootprintsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Step>()
            .init_resource::<Looks>()
            .init_resource::<Lying>()
            .add_systems(
                Update,
                (hero_steps, lay, age)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
