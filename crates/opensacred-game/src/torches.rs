//! The torches' flames (#1092's torches family; docs/re/effects.md, "The torches' flames"): a lit
//! torch in a creature's hand, the hero's or an NPC's (the types `0x1600` and `0x1601`,
//! `TORCH_SMALL_ON` and `TORCH_LARGE_ON`, known by their models' files), burns as the original's
//! `cWeapon3D` makes it burn (`0x5CDB60`, `FX_TORCHSMOKE` 787): flames of `PARTICLE_FIRE01` along
//! the way of its bone `weapon_fx01`, each playing its 4 × 4 frames once
//! (`opensacred_rules::particles::Torchsmoke`), riding with the torch's holder (the system's flag
//! 4: its frame's origin the holder's place on the ground, `0x5AA134`), and a glow of `PARTICLE_GLOW01` at the bone, both
//! added to the frame and not tinted by the night (`iso::UNTINTED`), with the loop 219 at the bone
//! at 0.4 of the volume (`hums`). The torch gone, its flames burn out.
//!
//! Not followed: the torch's light, which is the hero's rule (`daynight`, #61).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, Torchsmoke};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::{flames, models};
use crate::frame::{Playing, Step};
use crate::fx::{Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::hums::{Hum, Loudness};
use crate::iso;
use crate::models::OrderedWith;
use crate::npcs::{Piece, Wielded};

/// The lit torches' models by their files' names, the types `0x1600` and `0x1601`'s
/// (`0x428150`).
const LIT: [&str; 2] = ["torch_small_on", "torch_large_on"];

/// The bone that the flame burns at (bone 15).
const BONE: [&str; 1] = ["weapon_fx01"];

/// Whether the model `model`, a file `<number>-<name>.glb`, is a lit torch's ([`LIT`]).
pub(crate) fn lit(model: &str) -> bool {
    std::path::Path::new(model)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(|stem| stem.split_once('-'))
        .is_some_and(|(_, name)| LIT.iter().any(|lit| name.eq_ignore_ascii_case(lit)))
}

/// A torch's flame: the torch's model while it burns, its holder, its system, its frame's origin
/// in the world (the holder's place on the ground), the bone's point in that frame the frame
/// before, its loop's entity at the bone, and its parts' meshes, the flames' and the glow's.
#[derive(Component)]
struct Flame {
    torch: Option<Entity>,
    holder: Option<Entity>,
    system: Torchsmoke,
    origin: Vec3,
    before: Option<Vec3>,
    crackle: Entity,
    parts: [Handle<Mesh>; 2],
}

/// The flames' and the glow's materials, made as the first torch burns; none where a texture is
/// missing.
#[derive(Resource, Default)]
struct Materials(Option<[Option<Handle<crate::fx::Effect>>; 2]>);

/// The flames' rolls.
#[derive(Resource)]
struct Rolls(Rng);

impl Default for Rolls {
    fn default() -> Self {
        Self(crate::seed::dice(0x787))
    }
}

/// Lights each torch taken into a hand: a flame of its own, with its loop.
fn light(
    mut commands: Commands,
    torches: Query<(Entity, &Piece, Option<&OrderedWith>), Added<Wielded>>,
    data: Option<Res<Data>>,
    assets: Res<AssetServer>,
    (mut made, mut materials, mut meshes): (
        ResMut<Materials>,
        ResMut<Assets<crate::fx::Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (torch, piece, holder) in &torches {
        if !lit(piece.model()) {
            continue;
        }
        let made = made.0.get_or_insert_with(|| {
            [particles::TORCH_TEXTURE, particles::TORCH_GLOW_TEXTURE].map(|texture| {
                let image = data
                    .as_ref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, texture));
                if image.is_none() {
                    warn!("torches: no texture {texture}; its part is not drawn");
                }
                image.map(|image| materials.add(particle_material(image, AlphaMode::Add)))
            })
        });
        let flame = commands
            .spawn((
                Transform::IDENTITY,
                Visibility::Hidden,
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let crackle = commands
            .spawn((
                Transform::IDENTITY,
                Hum(particles::TORCH_LOOP),
                Loudness(particles::TORCH_LOOP_VOLUME),
                ChildOf(flame),
            ))
            .id();
        let parts = [meshes.add(empty()), meshes.add(empty())];
        for (mesh, material) in parts.iter().zip(made.iter()) {
            if let Some(material) = material {
                commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                    ChildOf(flame),
                ));
            }
        }
        commands.entity(flame).insert(Flame {
            torch: Some(torch),
            holder: holder.map(|&OrderedWith(holder)| holder),
            system: Torchsmoke::new(),
            origin: Vec3::ZERO,
            before: None,
            crackle,
            parts,
        });
        debug!("torches: {torch} burns");
    }
}

/// Burns each flame a frame on at its torch's bone (`0x774FE0`), hidden while its torch is, and
/// puts it out as its torch goes, its flames burning out and the flame going with its last.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn burn(
    mut commands: Commands,
    time: Res<Time>,
    mut burning: Query<(Entity, &mut Flame, &mut Visibility)>,
    torches: Query<Option<&InheritedVisibility>, With<Wielded>>,
    holders: Query<&GlobalTransform>,
    mut crackles: Query<&mut Transform, With<Hum>>,
    (children, bones): (Query<&Children>, Query<(&Name, &GlobalTransform)>),
    mut meshes: ResMut<Assets<Mesh>>,
    mut rolls: ResMut<Rolls>,
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut flame, mut visibility) in &mut burning {
        let flame = &mut *flame;
        let mut bone = None;
        if let Some(torch) = flame.torch {
            if let Ok(shown) = torches.get(torch) {
                let at = crate::making::bone(torch, &BONE, &children, &bones);
                let Some(at) = at.filter(|_| shown.is_none_or(|shown| shown.get())) else {
                    // Hidden, or its bone not yet in place: nothing drawn, nothing left behind.
                    *visibility = Visibility::Hidden;
                    flame.before = None;
                    continue;
                };
                bone = Some(at);
            } else {
                flame.torch = None;
                flame.system.put_out();
            }
        }
        *visibility = Visibility::Inherited;
        if let Some(holder) = flame.holder.and_then(|holder| holders.get(holder).ok()) {
            flame.origin = holder.translation() * Vec3::new(1.0, 0.0, 1.0);
        }
        if let Some(at) = bone {
            let here = at - flame.origin;
            let before = flame.before.unwrap_or(here);
            flame.before = Some(here);
            flame
                .system
                .update(dt, (models(before), models(here)), &mut rolls.0);
            if let Ok(mut crackle) = crackles.get_mut(flame.crackle) {
                crackle.translation = at;
            }
        } else {
            flame.system.update(dt, ([0.0; 3], [0.0; 3]), &mut rolls.0);
        }
        if flame.system.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut parts = [Quads::default(), Quads::default()];
        flames(
            &flame.system.system,
            particles::TORCH_GRID,
            flame.origin,
            &mut parts[0],
        );
        if let Some(at) = bone {
            parts[1].push(
                at,
                particles::TORCH_GLOW_HALF * iso::MODEL_SCALE,
                0.0,
                (Vec2::ZERO, 1.0),
                particles::TORCH_GLOW_COLOUR,
            );
        }
        for (handle, part) in flame.parts.iter().zip(parts) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The torches' flames.
pub struct TorchesPlugin;

impl Plugin for TorchesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Materials>()
            .init_resource::<Rolls>()
            .add_systems(
                Update,
                (light, burn)
                    .chain()
                    .after(Step::Npcs)
                    .in_set(Playing)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lit_torches_are_known_by_their_models() {
        assert!(lit("models/models/1541-torch_small_on.glb"));
        assert!(lit("models/models/1540-TORCH_LARGE_ON.glb"));
        // The wall torch and a torch put out burn not.
        assert!(!lit("models/models/1486-m_torch.glb"));
        assert!(!lit("models/models/1542-torch_small_off.glb"));
        assert!(!lit("torch_small_on.glb"));
    }
}
