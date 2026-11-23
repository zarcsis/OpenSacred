//! The butterflies over a head (#3023; docs/re/effects.md, "The butterflies over a head";
//! docs/re/arts.md, "The effects"): the effect 31, which a `SetNPCState`'s `0x8C` and the potions
//! 6081 and 6094 give for 10 s ([`KnockOut`]), added or renewed as `0x582D50` does
//! (`effects::Effects::knock_out`), each time with an object 874 that circles its bearer's head
//! with five blue butterflies for 150 ms a unit of the effect's art; a knock-down's, white for
//! 6 s (`knock_down`); and those of a hero's follower that falls unconscious, white for its 20 s
//! of lying (`followers`, #3087). Their system runs by `opensacred_rules::particles::Knockout`; it follows
//! the bearer's `Bip01 Head`, laid over its model as the original draws it after its bearer
//! (`fx::Orders::over`), and the night tints it.

use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;
use opensacred_rules::fight::Effect;
use opensacred_rules::fight::effects::{KNOCKOUT, KNOCKOUT_SECONDS};
use opensacred_rules::particles::{
    KNOCKED_DOWN_COLOUR, KNOCKED_DOWN_SECONDS, KNOCKOUT_COLOUR, KNOCKOUT_GRID, KNOCKOUT_TEXTURE,
    Knockout, knockout_seconds,
};

use crate::bolt_looks::world;
use crate::effects::Effects;
use crate::frame::{Playing, Unpaused};
use crate::fx::{
    Effect as EffectMaterial, Orders, Quads, empty, particle_material, particle_texture,
};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// The effect 31 given a creature, the hero where none is named, of the art `art`: a
/// `SetNPCState`'s `0x8C` of `art` above 0 (`0x46217B`), a potion 6081 or 6094 of
/// `rand() % 601 + 400` (`0x5CB89A`).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct KnockOut {
    pub creature: Option<Entity>,
    pub art: u32,
}

/// The art of a potion 6081's or 6094's effect 31 (`rand() % 601 + 400`, `0x5CB87D`).
pub(crate) fn potion_art(dice: &mut Rng) -> u32 {
    dice.below(POTION_ARTS) + POTION_ART_LEAST
}

const POTION_ARTS: u32 = 601;
const POTION_ART_LEAST: u32 = 400;

/// The butterflies of a creature knocked down, white for 6 s, at its head (`0x56E7D2`).
pub(crate) fn knocked_down(commands: &mut Commands, bearer: Entity) {
    spawn(commands, bearer, KNOCKED_DOWN_SECONDS, KNOCKED_DOWN_COLOUR);
}

/// The butterflies of a hero's follower that falls unconscious to rise again, white for its 20 s
/// of lying, at its head (`0x53EA1E`–`0x53EAB3`, #3087).
pub(crate) fn fainted(commands: &mut Commands, bearer: Entity) {
    spawn(
        commands,
        bearer,
        opensacred_rules::particles::FAINTED_SECONDS,
        KNOCKED_DOWN_COLOUR,
    );
}

/// A system of butterflies for `seconds` in `colour` over the head of `bearer`.
fn spawn(commands: &mut Commands, bearer: Entity, seconds: f32, colour: [f32; 4]) {
    commands.spawn((
        Unmade {
            bearer,
            seconds,
            colour,
        },
        Transform::default(),
        Visibility::default(),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Butterflies to make: their bearer, their seconds and their colour.
#[derive(Component)]
pub(crate) struct Unmade {
    pub(crate) bearer: Entity,
    seconds: f32,
    colour: [f32; 4],
}

/// Butterflies that fly: their bearer, their system, their colour and their mesh.
#[derive(Component)]
struct Shown {
    bearer: Entity,
    system: Knockout,
    colour: [f32; 4],
    mesh: Handle<Mesh>,
}

/// The butterflies' material, made as the first fly; none where the texture is missing.
#[derive(Resource, Default)]
struct Material(Option<Option<Handle<EffectMaterial>>>);

/// The butterflies' dice (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x874))
    }
}

/// The name of the bone the butterflies circle.
const HEAD: &str = "Bip01 Head";
/// How high above its root a bearer's head stands where its model has no such bone, in world
/// units (**estimate**: a man's head).
const HEAD_HEIGHT: f32 = 85.0;

/// Gives each creature knocked out its effect 31 of 10 s as `0x582D50` adds it (at `0x58437C`),
/// renewing one that holds, and its butterflies for 150 ms a unit of the art its effect then
/// holds, blue.
fn knock_out(
    mut commands: Commands,
    mut knocks: MessageReader<KnockOut>,
    heroes: Query<Entity, With<crate::hero::Body>>,
    mut held: Query<&mut Effects>,
) {
    for knock in knocks.read() {
        let Some(creature) = knock.creature.or_else(|| heroes.iter().next()) else {
            continue;
        };
        let effect = Effect {
            art: knock.art,
            ..Effect::new(KNOCKOUT, 0.0)
        };
        let art = match held.get_mut(creature) {
            Ok(mut effects) => effects.knock_out(effect, KNOCKOUT_SECONDS),
            Err(_) => {
                let mut effects = Effects::default();
                let art = effects.knock_out(effect, KNOCKOUT_SECONDS);
                commands.entity(creature).insert(effects);
                art
            }
        };
        info!(
            "knockouts: {creature} takes the effect 31 of the art {}, butterflies for {} s",
            knock.art,
            knockout_seconds(art)
        );
        spawn(
            &mut commands,
            creature,
            knockout_seconds(art),
            KNOCKOUT_COLOUR,
        );
    }
}

/// Gives each new system its mesh and its material.
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    (data, assets, mut made): (Option<Res<Data>>, Res<AssetServer>, ResMut<Material>),
    (mut materials, mut meshes): (ResMut<Assets<EffectMaterial>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade) in &unmade {
        let material = made
            .0
            .get_or_insert_with(|| {
                let image = data
                    .as_ref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, KNOCKOUT_TEXTURE));
                if image.is_none() {
                    warn!("knockouts: no texture {KNOCKOUT_TEXTURE}");
                }
                image.map(|image| {
                    materials.add(crate::fx::among_objects(particle_material(
                        image,
                        AlphaMode::Blend,
                    )))
                })
            })
            .clone();
        let mesh = meshes.add(empty());
        let mut made = commands.entity(entity);
        made.remove::<Unmade>().insert(Shown {
            bearer: unmade.bearer,
            system: Knockout::new(unmade.seconds),
            colour: unmade.colour,
            mesh: mesh.clone(),
        });
        if let Some(material) = material {
            made.with_child((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                NoFrustumCulling,
            ));
        }
    }
}

/// Runs each system a frame on at its bearer's head (`0x79B920`) and draws its butterflies
/// (`0x79BBB0`, `0x761B00` with the flags `0x40` and the grid 8): each a quad of its half side,
/// of its frame of the grid, in the system's colour, over its bearer's model where it lies before
/// it; each goes once its seconds ran out or its bearer is gone.
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &mut Transform)>,
    places: Query<&GlobalTransform>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut meshes, orders, mut dice): (ResMut<Assets<Mesh>>, Orders, ResMut<Dice>),
) {
    let dt = time.delta_secs().min(0.1);
    let grid = f32::from(KNOCKOUT_GRID);
    for (entity, mut look, mut transform) in &mut shown {
        let look = &mut *look;
        let Ok(root) = places.get(look.bearer) else {
            commands.entity(entity).despawn();
            continue;
        };
        transform.translation =
            crate::making::bone(look.bearer, &[HEAD], &children, (&bones, &worn))
                .unwrap_or(root.translation() + Vec3::Y * HEAD_HEIGHT);
        look.system.update(dt, &mut dice.0);
        if look.system.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let Some(mut mesh) = meshes.get_mut(&look.mesh) else {
            continue;
        };
        let mut quads = Quads::default();
        for butterfly in look.system.butterflies() {
            let frame = butterfly.frame();
            let cell = Vec2::new(
                f32::from(frame % KNOCKOUT_GRID),
                f32::from(frame / KNOCKOUT_GRID),
            );
            quads.push(
                world(butterfly.offset),
                butterfly.size * iso::MODEL_SCALE,
                0.0,
                (cell / grid, 1.0 / grid),
                look.colour,
            );
        }
        quads.as_drawn(orders.over(look.bearer), transform.translation);
        quads.into_mesh(&mut mesh);
    }
}

/// The butterflies over a head.
pub struct KnockoutsPlugin;

impl Plugin for KnockoutsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<KnockOut>()
            .init_resource::<Material>()
            .init_resource::<Dice>()
            .add_systems(
                Update,
                (knock_out, make, fly)
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
    fn a_potions_art_is_400_to_1000() {
        let mut dice = Rng::of(1, 2, 3);
        for _ in 0..1000 {
            let art = potion_art(&mut dice);
            assert!((400..=1000).contains(&art), "{art}");
        }
    }
}
