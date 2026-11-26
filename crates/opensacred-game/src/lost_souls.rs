//! The look of the Wood Elf's lost souls (#1092; docs/re/effects.md, "The lost souls"): each soul
//! (`spells`), `cParticleSystem_lostsouls` (850, `0x79CFB0`), leaves green dots of
//! `PARTICLE_DOTS01` behind it as it flies, added by their alpha, from a point that wobbles about
//! it, over a glow of `PARTICLE_GLOW03` beating once a second at that point, added whole
//! (`opensacred_rules::particles::LostSoul`); untinted by the night. Its object gone, its dots
//! fade out and it goes. Its green Direct3D point light follows it on the models (`fx_lights`,
//! #2106).
//!
//! A dryad ghost (`dryad_ghosts`, #1964) looks as a soul does, with blue dots, for as long as its
//! object lives (`Ghostly`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, LostSoul, soul_glow, soul_wobble};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::{models, world};
use crate::frame::Playing;
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::iso;

/// A soul's object to give its look, for its seconds (`spells`).
#[derive(Component)]
pub(crate) struct Look(pub f32);

/// A dryad ghost's object, which looks as a soul does with blue dots for as long as it lives
/// (`dryad_ghosts`, `particles::LostSoul::dryad_ghost`).
#[derive(Component)]
pub(crate) struct Ghostly;

/// A soul's look, which outlives its object while its dots fade: its object while it lives, its
/// dots, their dice, its place and the place before, and its parts' meshes, the dots' and the
/// glow's.
#[derive(Component)]
struct Shown {
    object: Option<Entity>,
    soul: LostSoul,
    dice: Rng,
    place: Vec3,
    before: Option<Vec3>,
    parts: [Handle<Mesh>; 2],
}

/// The dots' and the glow's materials, made as the first soul shows; none where a texture is
/// missing.
#[derive(Resource, Default)]
struct Materials(Option<[Option<Handle<Effect>>; 2]>);

/// The seed of the souls' `rand()` (**estimate**: any).
const SEED: usize = 0x352;

/// A lost soul's look as a save keeps it with its disk (#3776, `spells::SpellLook`): its seconds,
/// and its soul, dice, place and the place before, none for one not shown yet.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedLook {
    seconds: f32,
    shown: Option<(LostSoul, Rng, Vec3, Option<Vec3>)>,
}

/// The look of the disk `object` as a save keeps it, if it has one.
pub(crate) fn saved_look(world: &World, object: Entity) -> Option<SavedLook> {
    let Look(seconds) = *world.get::<Look>(object)?;
    let shown = world.try_query::<&Shown>().and_then(|mut shown| {
        shown
            .iter(world)
            .find(|shown| shown.object == Some(object))
            .map(|shown| {
                (
                    shown.soul.clone(),
                    shown.dice.clone(),
                    shown.place,
                    shown.before,
                )
            })
    });
    Some(SavedLook { seconds, shown })
}

/// Gives the loaded disk `object` its look as the save kept it.
pub(crate) fn look_again(object: &mut EntityWorldMut<'_>, saved: SavedLook) {
    object.insert(Look(saved.seconds));
    if let Some(shown) = saved.shown {
        object.insert(crate::fx::Resumes(shown));
    }
}

/// Gives each new soul its look (`0x79D310`): an entity of its own and its parts.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn show(
    mut commands: Commands,
    souls: Query<
        (
            Entity,
            &Look,
            &GlobalTransform,
            Has<Ghostly>,
            Option<&crate::fx::Resumes<(LostSoul, Rng, Vec3, Option<Vec3>)>>,
        ),
        Added<Look>,
    >,
    (data, assets, mut made, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Materials>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (object, &Look(seconds), place, ghostly, resumed) in &souls {
        let [dots, glow] = made
            .0
            .get_or_insert_with(|| {
                let image = |name: &str| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("lost souls: no texture {name}; its part is not drawn");
                    }
                    image
                };
                [
                    image(particles::SOUL_TEXTURE)
                        .map(|image| materials.add(particle_material(image, AlphaMode::Add))),
                    image(particles::SOUL_GLOW_TEXTURE)
                        .map(|image| materials.add(whole_material(image))),
                ]
            })
            .clone();
        let shown = commands
            .spawn((
                Transform::IDENTITY,
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        // Its Direct3D light, following it (`0x79CDD0`, `0x79CFB0`; the dryad ghost's alike,
        // `0x7BF100`).
        commands.spawn(crate::fx_lights::light(
            crate::fx_lights::SOUL_GREEN,
            crate::fx_lights::LOW,
            place.translation(),
            shown,
            (Some(object), Vec3::ZERO),
        ));
        let parts = [meshes.add(empty()), meshes.add(empty())];
        for (mesh, material) in parts.iter().zip([dots, glow]) {
            if let Some(material) = material {
                commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                    ChildOf(shown),
                ));
            }
        }
        let index = usize::try_from(shown.index_u32()).unwrap_or(0);
        // A loaded look shows on as it showed (#3776).
        let (soul, dice, at, before) = match resumed {
            Some(crate::fx::Resumes(shown)) => shown.clone(),
            None => (
                if ghostly {
                    LostSoul::dryad_ghost()
                } else {
                    LostSoul::new(seconds)
                },
                crate::seed::dice(SEED ^ index),
                place.translation(),
                None,
            ),
        };
        commands
            .entity(object)
            .remove::<crate::fx::Resumes<(LostSoul, Rng, Vec3, Option<Vec3>)>>();
        commands.entity(shown).insert(Shown {
            object: Some(object),
            soul,
            dice,
            place: at,
            before,
            parts,
        });
    }
}

/// Runs each soul's look a frame on (`0x79CEB0`, `0x79CFB0`): while its object lives, dots along
/// its way; its glow at its wobbling point; then its dots fade out and it goes.
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown)>,
    objects: Query<&GlobalTransform, With<Look>>,
    (mut meshes, orders): (ResMut<Assets<Mesh>>, crate::fx::Orders),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut look) in &mut shown {
        let look = &mut *look;
        let ramp = look.soul.ramp();
        match look.object.map(|object| objects.get(object)) {
            Some(Ok(place)) => look.place = place.translation(),
            _ => {
                if look.object.take().is_some() {
                    look.soul.end();
                }
            }
        }
        let before = look.before.unwrap_or(look.place);
        look.soul
            .update(dt, (models(before), models(look.place)), &mut look.dice);
        look.before = Some(look.place);
        if look.soul.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let mut dots = Quads::default();
        for dot in &look.soul.system.particles {
            dots.push(
                world(dot.place),
                dot.size * s,
                dot.angle,
                (Vec2::ZERO, 1.0),
                ramp.at(dot.life),
            );
        }
        let clock = look.soul.system.clock;
        let (half, colour) = soul_glow(clock);
        let mut glow = Quads::default();
        glow.push(
            look.place + world(soul_wobble(clock)),
            half * s,
            0.0,
            (Vec2::ZERO, 1.0),
            colour,
        );
        // At its place in the order as it flies, which a moving object heads in its cell (#2188).
        let order = orders.at(look.place);
        for (handle, mut part) in look.parts.iter().zip([dots, glow]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, Vec3::ZERO);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The lost souls' looks.
pub struct LostSoulsPlugin;

impl Plugin for LostSoulsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Materials>().add_systems(
            Update,
            (show, fly)
                .chain()
                .in_set(Playing)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
