//! The Dark Elf's casts' helices (#1092; docs/re/effects.md, "The Dark Elf's casts"): as the clip
//! of his slaughter (1047) or his death's mark (1048) starts, `cParticleSystem_dark_sm_cast` (865)
//! goes with his root, and 3 s on a coil of `FX_HELIX01` of radius 20 winds up from it, red for
//! the slaughter and yellow for the death's mark, spinning and stretching as it rises, fading in
//! and out over 1.67 s, added by its alpha, untinted by the night and drawn just behind his model,
//! as the original draws the system before the creature it follows (`fx::Orders`, #2178)
//! (`opensacred_rules::particles::DarkHelix`). Its `DELF_CAST` is `art_sounds`'.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{DarkHelix, HELIX_TEXTURE};

use crate::frame::{Playing as Running, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::iso;
use crate::making::{Making, Playing};
use crate::spawning::Rng;

/// The helices' dice, for their first steps' angles (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x865))
    }
}

/// The helices' material, made as the first asks, `None` within where its texture is missing:
/// added by its alpha (`SRCALPHA`, `ONE`).
#[derive(Resource, Default)]
struct Looks(Option<Option<Handle<Effect>>>);

/// A helix that shows: its Dark Elf, its look and its mesh.
#[derive(Component)]
struct Helix {
    caster: Entity,
    look: DarkHelix,
    mesh: Handle<Mesh>,
}

/// Makes the helix of each slaughter and death's mark whose clip starts (`0x50A986`, `0x50AA4A`:
/// the event `0x58` at his root).
fn start(
    mut commands: Commands,
    started: Query<(Entity, &Making, &GlobalTransform), Added<Playing>>,
    (data, assets, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (caster, making, at) in &started {
        let roll = f32::from(u16::try_from(dice.0.below(0x8000)).unwrap_or(0)) / 32_767.0;
        let Some(look) = DarkHelix::new(making.art, roll * std::f32::consts::TAU) else {
            continue;
        };
        let material = looks
            .0
            .get_or_insert_with(|| {
                let image = data
                    .as_deref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, HELIX_TEXTURE));
                if image.is_none() {
                    warn!("dark helices: no texture {HELIX_TEXTURE}; the helices are not drawn");
                }
                image.map(|image| effects.add(particle_material(image, AlphaMode::Add)))
            })
            .clone();
        let mesh = meshes.add(empty());
        let mut helix = commands.spawn((
            Transform::from_translation(at.translation()),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(material) = material {
            helix.with_child((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
                RenderLayers::layer(iso::UNTINTED),
            ));
        }
        helix.insert(Helix { caster, look, mesh });
    }
}

/// Runs each helix a frame on (`0x793E50`) and draws it (`0x793FD0`, `0x762C30`): at its Dark
/// Elf's root, its two ribbons in its colour once it shows; it goes once its phase is done, or
/// with him.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut helices: Query<(Entity, &mut Helix, &mut Transform)>,
    places: Query<&GlobalTransform>,
    mut meshes: ResMut<Assets<Mesh>>,
    orders: Orders,
) {
    let dt = time.delta_secs();
    for (entity, mut helix, mut transform) in &mut helices {
        let Ok(root) = places.get(helix.caster).map(GlobalTransform::translation) else {
            commands.entity(entity).despawn();
            continue;
        };
        helix.look.update(dt);
        if helix.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation = root;
        let mut quads = Quads::default();
        if helix.look.shows() {
            crate::beacons::ribbon_quads(&mut quads, &helix.look.ribbons(), helix.look.colour());
        }
        if let Some(mut mesh) = meshes.get_mut(&helix.mesh) {
            quads.in_order(orders.behind(helix.caster), root);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The Dark Elf's casts' helices.
pub struct DarkHelicesPlugin;

impl Plugin for DarkHelicesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_systems(
                Update,
                (start, run)
                    .chain()
                    .in_set(Running)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
