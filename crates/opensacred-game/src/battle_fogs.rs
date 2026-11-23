//! The look of the battle fog (#1092; docs/re/effects.md, "The battle fog"): where a fog trap
//! lies (`spells`), where a dryad druid blinks (`creature_casts`) and where a minimummy blinks
//! from and to (`ai`, #2565), `cParticleSystem_battlefog` (868, `0x7707D0`) puffs
//! `PARTICLE_SMOKE03` for its seconds, laid over by its alpha and tinted by the light at it
//! (`opensacred_rules::particles::battle_fog`), its puffs living out after; a blink's two of their
//! variants, the warm one where it leaves and the cool one where it lands (the FX event `0x5C`'s
//! `+0x3C`, #3157), a fog trap's of none.
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

/// A fog to make at the place `at` for `seconds`, of its variant.
#[derive(Component)]
struct Unmade {
    at: Vec3,
    seconds: f32,
    variant: Variant,
}

/// A fog: its puffs, their dice, its seconds, its variant, and its mesh.
#[derive(Component)]
struct Fog {
    puffs: System,
    dice: Rng,
    seconds: f32,
    variant: Variant,
    mesh: Handle<Mesh>,
}

/// A battle fog's variant, its event's `+0x3C` (`0x7709F0`): a fog trap's of none, a blink's warm
/// one where it leaves (1) and its cool one where it lands (2) (`0x4D9166`, `0x4D929A`; the dryad
/// druid's `0x4D6533`, `0x4D6663`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Variant {
    Plain = 0,
    Leaves = 1,
    Lands = 2,
}

/// The fogs' systems by their variants and their material, made as the first fog rises; no
/// material where its texture is missing.
#[derive(Resource)]
struct Looks {
    fogs: [opensacred_rules::particles::Fire; 3],
    material: Option<Option<Handle<Effect>>>,
}

impl Looks {
    /// The system of the variant `variant`.
    fn fog(&self, variant: Variant) -> &opensacred_rules::particles::Fire {
        &self.fogs[variant as usize]
    }
}

impl Default for Looks {
    fn default() -> Self {
        Self {
            fogs: [battle_fog(0), battle_fog(1), battle_fog(2)],
            material: None,
        }
    }
}

/// The seed of the fogs' `rand()` (**estimate**: any).
const SEED: usize = 0x364;

/// A battle fog of `variant` rising at the place `at` on the ground for `seconds` (`spells`,
/// `creature_casts`, `ai`).
pub(crate) fn rise(commands: &mut Commands, at: Vec3, seconds: f32, variant: Variant) {
    commands.spawn((
        Unmade {
            at,
            seconds,
            variant,
        },
        DespawnOnExit(AppState::InGame),
    ));
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
    for (
        entity,
        &Unmade {
            at,
            seconds,
            variant,
        },
    ) in &unmade
    {
        let texture = looks.fog(variant).texture;
        let material = looks
            .material
            .get_or_insert_with(|| {
                let image = data
                    .as_ref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, texture));
                if image.is_none() {
                    warn!("battle fogs: no texture {texture}; the fog is not drawn");
                }
                image.map(|image| {
                    materials.add(crate::fx::among_objects(particle_material(
                        image,
                        AlphaMode::Blend,
                    )))
                })
            })
            .clone();
        commands.entity(entity).despawn();
        let mesh = meshes.add(empty());
        let index = usize::try_from(entity.index_u32()).unwrap_or(0);
        let mut fog = commands.spawn((
            Fog {
                puffs: System::new(looks.fog(variant).count),
                dice: crate::seed::dice(SEED ^ index),
                seconds,
                variant,
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
    mut fogs: Query<(Entity, &mut Fog, &GlobalTransform)>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut puffs, place) in &mut fogs {
        let puffs = &mut *puffs;
        let fog = looks.fog(puffs.variant);
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
            // At its object's place in the order, as the original draws the system (#2188).
            let origin = place.translation();
            let mut quads = quads(fog, &puffs.puffs);
            quads.in_order(orders.at(origin), origin);
            quads.into_mesh(&mut mesh);
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
