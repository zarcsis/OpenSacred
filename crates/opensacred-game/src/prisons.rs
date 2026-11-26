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

/// A magic prison's look as a save keeps it (#3814, the epic #3753): its object by its name and its look's
/// state; a load gives it to its object made again, going on as it went.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename = "SavedPrison")]
pub struct SavedLook {
    #[serde(with = "crate::names::required")]
    object: Entity,
    shown: (System, Rng),
}

impl bevy::ecs::entity::MapEntities for SavedLook {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.object = mapper.get_mapped(self.object);
    }
}

/// The looks in `world` as a save keeps them (#3814).
pub(crate) fn saved(world: &mut World) -> Vec<SavedLook> {
    let mut shown = world.query::<(Entity, &Shown)>();
    shown
        .iter(world)
        .map(|(object, shown)| SavedLook {
            object,
            shown: (shown.glows.clone(), shown.dice.clone()),
        })
        .collect()
}

/// Gives a loaded save's looks to their objects made again (#3814): one that shows already takes
/// its state, another takes it as it shows; one whose object the load finds not is left out.
pub(crate) fn again(world: &mut World, looks: Vec<SavedLook>) {
    for saved in looks {
        let Some(mut object) = crate::names::found(Some(saved.object))
            .and_then(|object| world.get_entity_mut(object).ok())
        else {
            continue;
        };
        match object.get_mut::<Shown>() {
            Some(mut shown) => {
                let (glows, dice) = saved.shown;
                shown.glows = glows;
                shown.dice = dice;
            }
            None => {
                object.insert(crate::fx::Resumes(saved.shown));
            }
        }
    }
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
    prisons: Query<(Entity, Option<&crate::fx::Resumes<(System, Rng)>>), Added<Prison>>,
    mut sounds: MessageWriter<crate::sfx::Play>,
    (data, assets, mut looks, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Looks>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (prison, resumed) in &prisons {
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
        // A loaded prison goes on as it went, its sound not made again (#3814): no load sends an
        // FX object's start.
        if let Some(crate::fx::Resumes((glows, dice))) = resumed {
            commands
                .entity(prison)
                .remove::<crate::fx::Resumes<(System, Rng)>>()
                .insert(Shown {
                    glows: glows.clone(),
                    dice: dice.clone(),
                    parts,
                });
            continue;
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
    mut shown: Query<(&mut Shown, &GlobalTransform)>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    let fire = &looks.fire;
    let (across, depth) = iso::fx_axes();
    for (mut look, place) in &mut shown {
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
        // At its object's place in the order, as the original draws the system (#2188).
        let origin = place.translation();
        let order = orders.at(origin);
        for (handle, mut part) in look.parts.iter().zip([glows, ground]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, origin);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prisons_look_goes_back_to_its_object_shown_or_not() {
        let shown = |count: usize, seed: usize| Shown {
            glows: System::new(count),
            dice: crate::seed::dice(seed),
            parts: std::array::from_fn(|_| Handle::default()),
        };
        let mut world = World::new();
        let prison = world.spawn(shown(4, 7)).id();
        let kept = saved(&mut world);
        let bytes = postcard::to_stdvec(&kept).unwrap();
        let kept: Vec<SavedLook> = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].object, prison);
        let mut loaded = World::new();
        let showing = loaded.spawn(shown(9, 1)).id();
        let unshown = loaded.spawn_empty().id();
        let looks = [showing, unshown].map(|object| SavedLook {
            object,
            shown: kept[0].shown.clone(),
        });
        again(&mut loaded, looks.to_vec());
        let now = loaded.get::<Shown>(showing).unwrap();
        assert_eq!((now.glows.clone(), now.dice.clone()), kept[0].shown);
        assert_eq!(
            loaded
                .get::<crate::fx::Resumes<(System, Rng)>>(unshown)
                .map(|crate::fx::Resumes(shown)| shown),
            Some(&kept[0].shown)
        );
    }
}
