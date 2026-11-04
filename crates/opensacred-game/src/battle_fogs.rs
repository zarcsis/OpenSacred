//! The look of the battle fog (#1092; docs/re/effects.md, "The battle fog"): where a fog trap
//! lies (`spells`) and where a dryad druid blinks (`creature_casts`), `cParticleSystem_battlefog`
//! (868, `0x7707D0`) puffs `PARTICLE_SMOKE03` for its seconds, laid over by its alpha and tinted
//! by the light at it (`opensacred_rules::particles::battle_fog`), its puffs living out after.
//!
//! Not followed: the renderer's vector in its layer's wind, as the world's smokes'.

use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;
use opensacred_rules::particles::{System, battle_fog};
use opensacred_rules::spawning::Rng;

use crate::frame::Playing;
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;

/// A fog to make at the place `at` for `seconds`.
#[derive(Component)]
struct Unmade {
    at: Vec3,
    seconds: f32,
}

/// A fog: its puffs, their dice, its seconds, and its mesh.
#[derive(Component)]
struct Fog {
    puffs: System,
    dice: Rng,
    seconds: f32,
    mesh: Handle<Mesh>,
}

/// The fogs' system and material, made as the first fog rises; no material where its texture
/// is missing.
#[derive(Resource)]
struct Looks {
    fog: opensacred_rules::particles::Fire,
    material: Option<Option<Handle<Effect>>>,
}

impl Default for Looks {
    fn default() -> Self {
        Self {
            fog: battle_fog(0),
            material: None,
        }
    }
}

/// The seed of the fogs' `rand()` (**estimate**: any).
const SEED: usize = 0x364;

/// A battle fog rising at the place `at` on the ground for `seconds` (`spells`, `creature_casts`).
pub(crate) fn rise(commands: &mut Commands, at: Vec3, seconds: f32) {
    commands.spawn((Unmade { at, seconds }, DespawnOnExit(AppState::InGame)));
}

/// Makes each fog asked for (`0x7709F0`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    mut looks: ResMut<Looks>,
    (data, assets, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (entity, &Unmade { at, seconds }) in &unmade {
        let texture = looks.fog.texture;
        let material = looks
            .material
            .get_or_insert_with(|| {
                let image = data
                    .as_ref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, texture));
                if image.is_none() {
                    warn!("battle fogs: no texture {texture}; the fog is not drawn");
                }
                image.map(|image| materials.add(particle_material(image, AlphaMode::Blend)))
            })
            .clone();
        commands.entity(entity).despawn();
        let mesh = meshes.add(empty());
        let index = usize::try_from(entity.index_u32()).unwrap_or(0);
        let mut fog = commands.spawn((
            Fog {
                puffs: System::new(looks.fog.count),
                dice: crate::seed::dice(SEED ^ index),
                seconds,
                mesh: mesh.clone(),
            },
            Transform::from_translation(at),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        // Tinted by the light at it: drawn with the world under the night's pass.
        if let Some(material) = material {
            fog.with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
            ));
        }
    }
}

/// Runs each fog a frame on (`0x770700`): its puffs aged, and new ones for its seconds; then it
/// goes with its last.
fn drift(
    mut commands: Commands,
    time: Res<Time>,
    looks: Res<Looks>,
    mut fogs: Query<(Entity, &mut Fog)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().min(0.1);
    let fog = &looks.fog;
    for (entity, mut puffs) in &mut fogs {
        let puffs = &mut *puffs;
        puffs.puffs.update(dt, &fog.layers);
        if puffs.puffs.clock < puffs.seconds {
            puffs
                .puffs
                .emit(&fog.blocks, &fog.layers, fog.count, &mut puffs.dice);
        } else if puffs.puffs.particles.is_empty() {
            commands.entity(entity).despawn();
            continue;
        }
        if let Some(mut mesh) = meshes.get_mut(&puffs.mesh) {
            quads(fog, &puffs.puffs).into_mesh(&mut mesh);
        }
    }
}

/// The quads of a fog's puffs in the systems' frame (docs/re/effects.md, "Their flight"): its
/// spread of 30 along the screen's horizontal and 80 down it, each puff its frame of the 2 × 2,
/// turned, in the ramp's colour at its life.
fn quads(fog: &opensacred_rules::particles::Fire, puffs: &System) -> Quads {
    let s = iso::MODEL_SCALE;
    let (across, down) = iso::fx_axes();
    let grid = fog.grid.max(1);
    let side = 1.0 / f32::from(grid);
    let mut quads = Quads::default();
    for puff in &puffs.particles {
        let [x, y, z] = puff.place;
        let frame = Vec2::new(
            f32::from(puff.frame % grid),
            f32::from(puff.frame / grid % grid),
        ) * side;
        quads.push(
            (across * x + down * y + Vec3::Y * z) * s,
            puff.size * s,
            puff.angle,
            (frame, side),
            fog.ramps[0].at(puff.life),
        );
    }
    quads
}

/// The battle fogs' looks.
pub struct BattleFogsPlugin;

impl Plugin for BattleFogsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>().add_systems(
            Update,
            (make, drift)
                .chain()
                .in_set(Playing)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
