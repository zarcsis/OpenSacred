//! The zhadum (1165, #2920; docs/re/effects.md, "The zhadum"): the guard that the `Enter` of the
//! sector 91,35 makes beside the sword that the start lays in its cave (`npcs`). Once a second,
//! while any living creature on the ground stands within its reach (its size 1,000 and the
//! creature's), the hero among them, it takes one of them at random, lays a laser beam of the
//! variant 5 from itself through the creature's neck and on, some 2,800 units (`beams`), and
//! kills it outright (`fight::Slay`), its glow of `PARTICLE_FLARE02` flashing white for a second
//! and fading over two thirds of one, added whole, untinted by the night
//! (`opensacred_rules::zhadum::Zhadum`). An immortal creature lives through it.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::ZHADUM_BEAM;
use opensacred_rules::spawning::Rng;
use opensacred_rules::zhadum::{self, GLOW_HALF, GLOW_TEXTURE, SIZE};

use crate::beams::End;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;

/// A zhadum that a script made, at its record's point raised by its height.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Zhadum;

/// A zhadum on guard: its rules and its glow's mesh.
#[derive(Component)]
struct Guarding {
    zhadum: zhadum::Zhadum,
    mesh: Handle<Mesh>,
}

/// The glow's material, made as the first zhadum guards; none where its texture is missing.
#[derive(Resource, Default)]
struct Made(Option<Option<Handle<Effect>>>);

/// The zhadums' dice (`rand()`; **estimate**: any seed).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x48D))
    }
}

/// Gives each new zhadum its rules and its glow (`0x7C2E50`, `0x7C3000`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn make(
    mut commands: Commands,
    zhadums: Query<Entity, (With<Zhadum>, Without<Guarding>)>,
    (data, assets, mut made, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Made>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for entity in &zhadums {
        let material = made
            .0
            .get_or_insert_with(|| {
                let image = data
                    .as_ref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, GLOW_TEXTURE));
                if image.is_none() {
                    warn!("zhadums: no texture {GLOW_TEXTURE}; the glow is not drawn");
                }
                image.map(|image| materials.add(whole_material(image)))
            })
            .clone();
        let mesh = meshes.add(empty());
        let mut guard = commands.entity(entity);
        if let Some(material) = material {
            guard.with_child((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
        guard.insert(Guarding {
            zhadum: zhadum::Zhadum::default(),
            mesh,
        });
        debug!("zhadums: {entity} guards");
    }
}

/// Where a zhadum at `from` lays its beam at a creature whose neck stands at `neck`: through the
/// neck and on, `BEAM_REACH` of the effects' frame along the ground (`0x5B2144`…`0x5B21B3`), its
/// height on the line through the neck.
fn beam_end(from: Vec3, neck: Vec3) -> Vec3 {
    let toward = neck - from;
    let along = iso::flight(zhadum::Zhadum::beam(iso::in_flight(toward)));
    let near = toward.xz().length();
    let rise = if near > 0.0 {
        toward.y * along.length() / near
    } else {
        0.0
    };
    from + along + Vec3::Y * rise
}

/// Runs each zhadum a frame on (`0x5A6680`, `0x5ABD80`, `0x7C2EE0`): of the living creatures on
/// the ground within its size and theirs, it takes one at random once a second, lays its beam at
/// it and slays it.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn guard(
    mut commands: Commands,
    (time, mut dice): (Res<Time>, ResMut<Dice>),
    mut zhadums: Query<(Entity, &mut Guarding, &GlobalTransform)>,
    creatures: Query<
        (
            Entity,
            (&Transform, &GlobalTransform),
            &crate::crowd::InTheWay,
            &crate::fight::Health,
            Option<&crate::arrows::Size>,
        ),
        (Without<crate::hero::Body>, Without<crate::npcs::Dies>),
    >,
    heroes: Query<
        (
            Entity,
            (&Transform, &GlobalTransform),
            &crate::hero::Body,
            &crate::fight::Health,
            Option<&crate::arrows::Size>,
        ),
        Without<crate::npcs::Dies>,
    >,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    mut slays: MessageWriter<crate::fight::Slay>,
) {
    let dt = time.delta_secs().min(0.1);
    let own = opensacred_rules::arrows::size(0);
    // Each living creature on the ground: its place now, its root's drawn place, which its bones'
    // follow a frame late, and its size.
    let living: Vec<(Entity, [Vec3; 2], f32)> = heroes
        .iter()
        .filter(|(_, _, body, health, _)| body.level().is_none() && health.current > 0)
        .map(|(entity, (at, root), _, _, size)| {
            let places = [at.translation, root.translation()];
            (entity, places, size.map_or(own, |s| s.0))
        })
        .chain(
            creatures
                .iter()
                .filter(|(_, _, way, health, _)| {
                    way.creature && way.level.is_none() && health.current > 0
                })
                .map(|(entity, (at, root), _, _, size)| {
                    let places = [at.translation, root.translation()];
                    (entity, places, size.map_or(own, |s| s.0))
                }),
        )
        .collect();
    for (entity, mut guarding, place) in &mut zhadums {
        let here = place.translation();
        let near: Vec<(Entity, [Vec3; 2])> = living
            .iter()
            .filter(|(_, [at, _], size)| (at.xz() - here.xz()).length() < SIZE + size)
            .map(|&(target, places, _)| (target, places))
            .collect();
        if let Some(&(target, [at, root])) = guarding
            .zhadum
            .strike(near.len(), &mut dice.0)
            .and_then(|index| near.get(index))
        {
            // Its neck, else its root, where it stands now.
            let neck = crate::making::bone(
                target,
                &[crate::arrows::NECK, "__Root"],
                &children,
                (&bones, &worn),
            )
            .map_or(at, |bone| at + bone - root);
            let to = beam_end(here, neck);
            debug!("zhadums: {entity} slays {target}, its beam to {to}");
            crate::beams::lay(&mut commands, (End::At(here), End::At(to)), ZHADUM_BEAM);
            slays.write(crate::fight::Slay { target });
        }
        guarding.zhadum.update(dt);
    }
}

/// Draws each zhadum's glow (`0x7C2F90`): a camera-facing quad of a half side of 60 at its place,
/// grey by its flash and its fade, at its place in the order.
fn draw(
    zhadums: Query<(&Guarding, &GlobalTransform)>,
    orders: Orders,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (guarding, place) in &zhadums {
        let grey = guarding.zhadum.glow();
        let mut quads = Quads::default();
        if grey > 0.0 {
            quads.push_facing(
                Vec3::ZERO,
                GLOW_HALF * iso::MODEL_SCALE,
                [[grey, grey, grey, 1.0]; 4],
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&guarding.mesh) {
            quads.in_order(orders.at(place.translation()), place.translation());
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The zhadum.
pub struct ZhadumsPlugin;

impl Plugin for ZhadumsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Made>()
            .add_systems(
                Update,
                (make, guard, draw)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_beam_goes_through_the_neck_and_some_2800_units_on() {
        let from = Vec3::new(1000.0, 30.0, 2000.0);
        // A neck to the west on the ground, 100 higher, 500 away.
        let neck = from + Vec3::new(-500.0, 100.0, 0.0);
        let to = beam_end(from, neck);
        let ground = (to - from).xz();
        assert!((ground.normalize() - Vec2::new(-1.0, 0.0)).length() < 1e-4);
        assert!(
            (ground.length() - 2860.0).abs() < 2.0,
            "{}",
            ground.length()
        );
        // On the line through the neck.
        assert!((to.y - from.y - 100.0 * ground.length() / 500.0).abs() < 1e-2);
        // Along the screen's vertical, foreshortened: some 2,710.
        let down = beam_end(from, from + Vec3::new(300.0, 0.0, 300.0));
        assert!(((down - from).length() - 2712.0).abs() < 2.0);
        // A neck right over it: the beam stays level.
        assert_eq!(beam_end(from, from + Vec3::Y * 50.0), from);
    }
}
