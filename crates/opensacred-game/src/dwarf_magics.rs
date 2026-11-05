//! The look of the dwarves' magic (832 and 833, `FX_DWARFMAGIC`, #1092; docs/re/effects.md, "The
//! dwarves' magic"): the scripts' lamps of the dwarves' halls and the caves (`npcs`) twinkle,
//! stars of `PARTICLE_SPARK04` rising 100 over their place, added whole and untinted by the night,
//! fading through pale yellow (832) or pale blue (833); each lights the ground about it by night
//! or in a dungeon (`opensacred_rules::particles::DwarfMagic`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, DwarfMagic};

use crate::bolt_looks::world;
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::spawning::Rng;
use crate::{daynight, iso};

/// A dwarves' magic's object, its variant (1 of 832, 2 of 833), which takes its look ([`show`]).
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct Magic(pub(crate) u8);

/// The magics' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x832))
    }
}

/// The stars' material, made at the first magic; none where its texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<Option<Handle<Effect>>>);

/// A magic's look on its object: its system and its mesh.
#[derive(Component)]
struct Shown {
    magic: DwarfMagic,
    mesh: Handle<Mesh>,
}

/// Gives each new magic its look (`0x78A2D0`, `0x78A5D0`): its stars' mesh and its light.
fn show(
    mut commands: Commands,
    magics: Query<(Entity, &Magic), Added<Magic>>,
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, &Magic(variant)) in &magics {
        let material = looks
            .0
            .get_or_insert_with(|| {
                let image = data.as_deref().and_then(|data| {
                    particle_texture(&data.0.assets, &assets, particles::DWARF_MAGIC_TEXTURE)
                });
                if image.is_none() {
                    warn!(
                        "dwarf magics: no texture {}; their stars are not drawn",
                        particles::DWARF_MAGIC_TEXTURE
                    );
                }
                image.map(|image| effects.add(whole_material(image)))
            })
            .clone();
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
        // Its light in the world's table, at its place on the ground (`+0x9C`, `0x5A9C50`).
        commands.spawn((
            daynight::Light(particles::dwarf_magic_light(variant)),
            Transform::IDENTITY,
            ChildOf(entity),
        ));
        commands.entity(entity).insert(Shown {
            magic: DwarfMagic::new(variant),
            mesh,
        });
    }
}

/// Twinkles each magic a frame on (`0x78A360`, `0x78A3B0`): its stars about its place.
fn twinkle(
    time: Res<Time>,
    mut magics: Query<&mut Shown>,
    (mut dice, mut meshes): (ResMut<Dice>, ResMut<Assets<Mesh>>),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for mut look in &mut magics {
        let look = &mut *look;
        look.magic.update(dt, &mut dice.0);
        let ramp = look.magic.ramp();
        let mut quads = Quads::default();
        for star in &look.magic.system.particles {
            quads.push(
                world(star.place),
                star.size * s,
                star.angle,
                (Vec2::ZERO, 1.0),
                ramp.at(star.life),
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&look.mesh) {
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The dwarves' magics' looks.
pub struct DwarfMagicsPlugin;

impl Plugin for DwarfMagicsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (show, twinkle)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
