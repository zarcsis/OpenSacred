//! The guns' sights (#1092; docs/re/effects.md, "The guns' sights"): each shot of a gun
//! ([`Gunfire`]: a shot's clip, the Dwarf's gun arts) flashes at its muzzle, the gun's bone
//! `Canon_fx01` (48), along that bone's axis (1172, `cParticleSystem_gunfire`): two crossed flames
//! of one of its four textures at random, warm and narrow within grey and wide, added whole and
//! untinted by the night, of its gun's scale, for 0.1 s, lighting the night by 300 and the models
//! white 5 ahead of it (`opensacred_rules::particles::{gunfire_quads, gunfire_scale}`), kept at
//! the muzzle while it shows. Each
//! creature that the Dwarf's gun arts strike with a gun, and that his cannon strikes ([`Impact`]),
//! sprays red at its neck along the shot (1151, `cParticleSystem_shoot_impact`,
//! `particles::shoot_impact`), drawn by `puffs`, tinted by the night.
//!
//! Not followed: the impact's test of the struck object's `+0x14` bit `0x10` (not read), the
//! cannon's own flash (1017, `dwarf_shoot`).

use std::collections::HashMap;

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::arts::{DWARF_ATTACKE, DWARF_HARDHIT, DWARF_RUNDUM};
use opensacred_rules::particles::{
    self, GUNFIRE_LIFE, GUNFIRE_LIGHT, GUNFIRE_LIGHT_AHEAD, gunfire_quads, gunfire_scale,
};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::hit_sparks::Spark;
use crate::iso;
use crate::sfx::Gunfire;

/// The guns' muzzles' bone (48) and the struck creatures' neck (3).
const MUZZLE: [&str; 1] = ["Canon_fx01"];
const NECK: [&str; 1] = ["Bip01 Neck"];

/// The flash's Direct3D light (`0x7C3AC0`): white.
const WHITE: [f32; 3] = [1.0; 3];

/// A shot's impact at the struck creature `on` along the world's `direction`: the Dwarf's
/// cannon's (1087, `0x517A10`) at each hostile creature near its line, along the line's far end
/// less the creature.
#[derive(Message, Clone, Copy, Debug)]
pub struct Impact {
    pub on: Entity,
    pub direction: Vec3,
}

/// A flash that shows: its shooter, whose muzzle it keeps to, its gun's scale, the seconds left,
/// its mesh and its light.
#[derive(Component)]
struct Flash {
    shooter: Entity,
    scale: f32,
    left: f32,
    mesh: Handle<Mesh>,
    light: Entity,
}

/// The flashes' and the impacts' rolls.
#[derive(Resource)]
struct Rolls(Rng);

impl Default for Rolls {
    fn default() -> Self {
        Self(crate::seed::dice(0x1172))
    }
}

/// The flashes' materials by their texture, made as they are first asked; none where the texture
/// is missing.
#[derive(Resource, Default)]
struct Looks(HashMap<&'static str, Option<Handle<Effect>>>);

/// The world point and the unit axis of `shooter`'s gun's muzzle, the bone's `z` (the original's
/// `y`, the barrel's way in the guns' models).
fn muzzle(
    shooter: Entity,
    children: &Query<&Children>,
    (bones, worn): (&Query<(&Name, &GlobalTransform)>, &crate::making::Worn),
) -> Option<(Vec3, Vec3)> {
    let bone = crate::making::bone_entity(shooter, &MUZZLE, children, (bones, worn))?;
    let (_, at) = bones.get(bone).ok()?;
    let axis = at.affine().transform_vector3(Vec3::Z).try_normalize()?;
    Some((at.translation(), axis))
}

/// Flashes each shot of a gun at its shooter's muzzle.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn fire(
    mut commands: Commands,
    mut fires: MessageReader<Gunfire>,
    (hero, npcs, heroes, arms): (
        Option<Res<crate::hero::Hero>>,
        Option<Res<crate::npcs::Npcs>>,
        Query<(), With<crate::hero::Body>>,
        Query<&crate::npcs::Arms>,
    ),
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
    mut rolls: ResMut<Rolls>,
) {
    for &Gunfire { shooter } in fires.read() {
        let Some((place, axis)) = muzzle(shooter, &children, (&bones, &worn)) else {
            continue;
        };
        let armed = crate::sfx::armed_of(shooter, hero.as_deref(), &heroes, &arms, npcs.as_deref());
        let scale = gunfire_scale(armed.kind);
        let texture = particles::gunfire_texture(&mut rolls.0);
        let material = looks
            .0
            .entry(texture)
            .or_insert_with(|| {
                let image = data
                    .as_ref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, texture));
                if image.is_none() {
                    warn!("gun_fires: no texture {texture}; its flashes are not drawn");
                }
                image.map(|image| effects.add(whole_material(image)))
            })
            .clone();
        let mut mesh = empty();
        let (origin, lamp) = lay((place, axis), scale, &mut mesh);
        let mesh = meshes.add(mesh);
        debug!("gun_fires: {shooter} fires {texture} at {place} along {axis}");
        let mut flash = commands.spawn((
            crate::daynight::Light(GUNFIRE_LIGHT),
            Transform::from_translation(origin),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(material) = material {
            flash.with_child((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
        let owner = flash.id();
        let light = commands
            .spawn(crate::fx_lights::light(
                WHITE,
                lamp,
                owner,
                (None, Vec3::ZERO),
            ))
            .id();
        commands.entity(owner).insert(Flash {
            shooter,
            scale,
            left: GUNFIRE_LIFE,
            mesh,
            light,
        });
    }
}

/// Lays a flash of the gun's `scale` out at the muzzle `place` along its `axis` into `mesh`, from
/// the place on the ground under it, which it answers with its light's place. Its frame turns its
/// `y` onto the shot (**estimate**: the shortest turn), in the models' frame, its third coordinate
/// up.
fn lay((place, axis): (Vec3, Vec3), scale: f32, mesh: &mut Mesh) -> (Vec3, Vec3) {
    let along = Vec3::from(models(axis)).normalize_or(Vec3::Y);
    let turn = Quat::from_rotation_arc(Vec3::Y, along);
    let origin = place.with_y(0.0);
    let mut quads = Quads::default();
    for quad in gunfire_quads() {
        let corners = quad
            .corners
            .map(|corner| place + world((turn * Vec3::from(corner) * scale).to_array()) - origin);
        quads.push_corners(corners, quad.uvs.map(Vec2::from), quad.colour);
    }
    quads.into_mesh(mesh);
    (
        origin,
        place + world((along * GUNFIRE_LIGHT_AHEAD).to_array()),
    )
}

/// Sprays each impact at its struck creature's neck, else its place: the Dwarf's gun arts' with
/// a gun along his muzzle's axis, his cannon's along its line.
fn impact(
    mut commands: Commands,
    mut sparks: MessageReader<Spark>,
    mut impacts: MessageReader<Impact>,
    hero: Option<Res<crate::hero::Hero>>,
    places: Query<&GlobalTransform>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    mut rolls: ResMut<Rolls>,
) {
    let gun = hero.is_some_and(|hero| hero.armed.gun);
    let shots = sparks
        .read()
        .filter(|spark| gun && matches!(spark.art, DWARF_RUNDUM | DWARF_HARDHIT | DWARF_ATTACKE))
        .filter_map(|spark| {
            let (_, axis) = muzzle(spark.by, &children, (&bones, &worn))?;
            Some((spark.on, axis))
        })
        .collect::<Vec<_>>();
    let cannon = impacts.read().map(|impact| (impact.on, impact.direction));
    for (on, direction) in shots.into_iter().chain(cannon) {
        let Some(point) = crate::making::bone(on, &NECK, &children, (&bones, &worn))
            .or_else(|| places.get(on).ok().map(GlobalTransform::translation))
        else {
            continue;
        };
        debug!("gun_fires: an impact at {on} along {direction}");
        let burst = particles::shoot_impact(models(direction), &mut rolls.0);
        crate::puffs::spawn(&mut commands, burst, point, None);
    }
}

/// Keeps each flash at its shooter's muzzle, its light ahead of it, and takes it off once its time
/// is over: the original places it once at the muzzle as the shot's clip starts, where the game's
/// skeleton still holds the pose that the clip blends in from (docs/decisions/drawing.md).
fn show(
    mut commands: Commands,
    time: Res<Time>,
    mut flashes: Query<(Entity, &mut Flash, &mut Transform)>,
    mut lights: Query<&mut Transform, (With<crate::fx_lights::FxLight>, Without<Flash>)>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (entity, mut flash, mut transform) in &mut flashes {
        flash.left -= time.delta_secs();
        if flash.left <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let Some(muzzle) = muzzle(flash.shooter, &children, (&bones, &worn)) else {
            continue;
        };
        let Some(mut mesh) = meshes.get_mut(&flash.mesh) else {
            continue;
        };
        let (origin, lamp) = lay(muzzle, flash.scale, &mut mesh);
        transform.translation = origin;
        if let Ok(mut light) = lights.get_mut(flash.light) {
            light.translation = lamp;
        }
    }
}

/// The guns' flashes and their shots' impacts.
pub struct GunFiresPlugin;

impl Plugin for GunFiresPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Impact>()
            .init_resource::<Rolls>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (
                    (fire, impact)
                        .in_set(Playing)
                        .after(crate::frame::Step::Npcs),
                    show.in_set(Playing)
                        .in_set(Unpaused)
                        .after(crate::frame::Step::Npcs),
                )
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
