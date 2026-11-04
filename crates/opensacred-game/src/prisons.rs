//! The scripts' magic bars and barriers (#2063; docs/re/effects.md, "The magic prison"): an
//! object of `FX_MAGICPRISON` (854) that a script's `CreateObj` makes (`npcs`) draws, as
//! `cParticleSystem_magicprison` (`0x79CA40`), lilac glows of `PARTICLE_GLOW03` rising in a
//! column from its place, added whole, over a glow on the ground
//! (`opensacred_rules::particles::magic_prison`); untinted by the night. It sounds
//! `SPELL_BANNKREIS` once as it is made, and stands until its `DelNPC`.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, System, magic_prison};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::world;
use crate::frame::Playing;
use crate::fx::{Effect, Quads, empty, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;

/// How far over its place its ground glow lies, in world units, so that the ground's own faces do
/// not cut it.
const GROUND_LIFT: f32 = 0.5;

/// A magic prison's object, which `npcs` makes for a script's `CreateObj` of 854.
#[derive(Component)]
pub(crate) struct Prison;

/// A prison's look: its glows, their dice, and its parts' meshes, the glows' and the ground's.
#[derive(Component)]
struct Shown {
    glows: System,
    dice: Rng,
    parts: [Handle<Mesh>; 2],
}

/// The prisons' system and material, made as the first prison shows; none where the texture is
/// missing.
#[derive(Resource)]
struct Looks {
    fire: particles::Fire,
    material: Option<Option<Handle<Effect>>>,
}

impl Default for Looks {
    fn default() -> Self {
        Self {
            fire: magic_prison(),
            material: None,
        }
    }
}

/// The seed of the prisons' `rand()` (**estimate**: any).
const SEED: usize = 0x356;

/// Gives each new prison its look and its sound (`0x79CC80`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn show(
    mut commands: Commands,
    prisons: Query<Entity, Added<Prison>>,
    mut sounds: MessageWriter<crate::sfx::Play>,
    (data, assets, mut looks, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for prison in &prisons {
        let material = looks
            .material
            .get_or_insert_with(|| {
                let image = data.as_ref().and_then(|data| {
                    particle_texture(&data.0.assets, &assets, particles::PRISON_TEXTURE)
                });
                if image.is_none() {
                    warn!("prisons: no texture {}", particles::PRISON_TEXTURE);
                }
                image.map(|image| materials.add(whole_material(image)))
            })
            .clone();
        let parts = [meshes.add(empty()), meshes.add(empty())];
        if let Some(material) = material {
            for mesh in &parts {
                commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                    ChildOf(prison),
                ));
            }
        }
        let index = usize::try_from(prison.index_u32()).unwrap_or(0);
        commands.entity(prison).insert(Shown {
            glows: System::new(looks.fire.count),
            dice: crate::seed::dice(SEED ^ index),
            parts,
        });
        sounds.write(crate::sfx::Play {
            sound: crate::sfx::Of::Id(particles::PRISON_SOUND),
            at: Some(prison),
            rate: (0.5, crate::sfx::Roll::None),
        });
    }
}

/// Runs each prison a frame on (`0x79C9F0`, `0x79CA40`): its glows rise and fade, new ones every
/// 0.05 s, over its glow on the ground.
fn glow(
    time: Res<Time>,
    looks: Res<Looks>,
    mut shown: Query<&mut Shown>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let fire = &looks.fire;
    let (across, depth) = iso::fx_axes();
    for mut look in &mut shown {
        let look = &mut *look;
        look.glows.update(dt, &fire.layers);
        look.glows
            .emit(&fire.blocks, &fire.layers, fire.count, &mut look.dice);
        let mut glows = Quads::default();
        for glow in &look.glows.particles {
            glows.push(
                world(glow.place),
                glow.size * s,
                0.0,
                (Vec2::ZERO, 1.0),
                fire.ramps[0].at(glow.life),
            );
        }
        let mut ground = Quads::default();
        let half = particles::PRISON_GLOW_HALF * s;
        let lift = Vec3::Y * GROUND_LIFT;
        ground.push_corners(
            [
                lift + (-across - depth) * half,
                lift + (across - depth) * half,
                lift + (across + depth) * half,
                lift + (-across + depth) * half,
            ],
            [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
            particles::PRISON_GLOW_COLOUR,
        );
        for (handle, part) in look.parts.iter().zip([glows, ground]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The magic prisons.
pub struct PrisonsPlugin;

impl Plugin for PrisonsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>().add_systems(
            Update,
            (show, glow)
                .chain()
                .in_set(Playing)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
