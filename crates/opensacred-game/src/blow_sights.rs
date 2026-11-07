//! The blows' sights (#1092; docs/re/effects.md, "The bleed and the parade"): at each strike that
//! does a fiftieth of its target's most health at least, `cParticleSystem_bleed` (772) sprays
//! fifty droplets of the target's blood sideways from about its neck, laid over by their alpha;
//! at each strike that does no damage, a miss or a damage resisted whole,
//! `cParticleSystem_parade` (773) bursts ten thin golden streaks from its neck, gone in a quarter
//! of a second. Both untinted by the night and drawn just behind the struck creature's model, as
//! the original draws the system before the creature it follows (`fx::Orders`, #2178)
//! (`opensacred_rules::particles::{Bleed, Parade}`).
//!
//! Not followed: the blood's puddle (903) that the bleed leaves under the violence, and the
//! parade's frame, taken as the camera's (**hypothesis**).

use std::collections::HashMap;

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    BLEED_TEXTURE, Bleed, PARADE_COLOURS, Parade, blood_colour, blow_bleeds,
};

use crate::bolt_looks::world;
use crate::fight::Struck;
use crate::frame::{Playing, Unpaused};
use crate::fx::{
    Effect, Orders, Quads, empty, particle_material, particle_texture, plain_blend_material,
};
use crate::game::{AppState, Data};
use crate::iso;
use crate::spawning::Rng;

/// The bone the sights stand at, else the root (the handler's cases 8 and `0xA`: the bone 3).
const NECK: [&str; 1] = ["Bip01 Neck"];

/// How wide a parade's streak is, in world units: a line of a pixel at the zoom 1.
const STREAK_HALF: f32 = 0.4;

/// The sights' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x772))
    }
}

/// Each creature type's blood (`items.pak`'s category and gib set), read at the start; one
/// without, the heroes', bleeds red.
#[derive(Resource, Default)]
struct Bloods(HashMap<u32, [f32; 3]>);

/// The sights' materials, made as the first asks: the droplets laid over by their alpha, the
/// streaks of their vertices' colours laid over; none for the droplets where their texture is
/// missing.
#[derive(Resource, Default)]
struct Looks(Option<(Option<Handle<Effect>>, Handle<Effect>)>);

/// A bleed that shows: its creature, its look, its blood and its mesh.
#[derive(Component)]
struct Bleeding {
    target: Entity,
    look: Bleed,
    colour: [f32; 3],
    mesh: Handle<Mesh>,
}

/// A parade that shows: its creature, its look and its mesh.
#[derive(Component)]
struct Parrying {
    target: Entity,
    look: Parade,
    mesh: Handle<Mesh>,
}

/// Reads each creature type's blood off the assets' items at the start.
fn read(mut commands: Commands, data: Option<Res<Data>>) {
    let Some(data) = data else {
        return;
    };
    match data.0.tables.items() {
        Ok(items) => commands.insert_resource(Bloods(
            items
                .iter()
                .map(|item| (item.slot, blood_colour(item.category(), item.gib_set())))
                .collect(),
        )),
        Err(e) => warn!("blow sights: no bloods: {e:#}"),
    }
}

/// Makes the sight of each strike resolved (`0x532635`, `0x5329FB`): a bleed where it did a
/// fiftieth of its target's most health, a parade where it did none.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn make(
    mut commands: Commands,
    mut blows: MessageReader<Struck>,
    (places, minds, children, bones, worn): (
        Query<&GlobalTransform>,
        Query<&crate::ai::Mind>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (data, assets, bloods, mut looks, mut dice): (
        Option<Res<Data>>,
        Res<AssetServer>,
        Option<Res<Bloods>>,
        ResMut<Looks>,
        ResMut<Dice>,
    ),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for blow in blows.read() {
        let parade = blow.damage == 0;
        if !parade && !blow_bleeds(blow.damage, blow.most) {
            continue;
        }
        let Ok(root) = places.get(blow.target).map(GlobalTransform::translation) else {
            continue;
        };
        let neck =
            crate::making::bone(blow.target, &NECK, &children, (&bones, &worn)).unwrap_or(root);
        let (droplets, streaks) = looks
            .0
            .get_or_insert_with(|| {
                let image = data
                    .as_deref()
                    .and_then(|data| particle_texture(&data.0.assets, &assets, BLEED_TEXTURE));
                if image.is_none() {
                    warn!("blow sights: no texture {BLEED_TEXTURE}; the bleeds are not drawn");
                }
                (
                    image.map(|image| effects.add(particle_material(image, AlphaMode::Blend))),
                    effects.add(plain_blend_material()),
                )
            })
            .clone();
        let material = if parade { Some(streaks) } else { droplets };
        let Some(material) = material else {
            continue;
        };
        let mesh = meshes.add(empty());
        let mut sight = commands.spawn((
            Transform::from_translation(neck),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        sight.with_child((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            NoFrustumCulling,
            RenderLayers::layer(iso::UNTINTED),
        ));
        if parade {
            sight.insert(Parrying {
                target: blow.target,
                look: Parade::new(&mut dice.0),
                mesh,
            });
        } else {
            let colour = minds
                .get(blow.target)
                .ok()
                .and_then(|mind| bloods.as_ref()?.0.get(&mind.creature).copied())
                .unwrap_or_else(|| blood_colour(2, 0));
            sight.insert(Bleeding {
                target: blow.target,
                look: Bleed::new(&mut dice.0),
                colour,
                mesh,
            });
        }
    }
}

/// Runs each bleed a frame on (`0x76AEF0`) and draws it (`0x76AFF0`): its droplets facing the
/// camera, turned by their angles, in its blood at the alpha of their life, where they flew from
/// its creature's neck; it goes with its last droplet.
fn bleed(
    mut commands: Commands,
    time: Res<Time>,
    mut sights: Query<(Entity, &mut Bleeding, &Transform)>,
    (mut dice, mut meshes): (ResMut<Dice>, ResMut<Assets<Mesh>>),
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut sight, at) in &mut sights {
        let sight = &mut *sight;
        sight.look.update(dt, &mut dice.0);
        if sight.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let [r, g, b] = sight.colour;
        let mut quads = Quads::default();
        for droplet in &sight.look.system.particles {
            quads.push(
                world(droplet.place),
                droplet.size * iso::MODEL_SCALE,
                droplet.angle,
                (Vec2::ZERO, 1.0),
                [r, g, b, droplet.life.clamp(0.0, 255.0) / 255.0],
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&sight.mesh) {
            quads.in_order(orders.behind(sight.target), at.translation);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// Runs each parade a frame on (`0x76C2F0`) and draws it (`0x76C630`): at its creature's neck,
/// each streak a line through its five points in the camera's plane (**hypothesis**: the draw's
/// matrix the camera's), from its bright head to its faint tail; it goes once they are gone.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn parry(
    mut commands: Commands,
    time: Res<Time>,
    mut sights: Query<(Entity, &mut Parrying, &mut Transform)>,
    (places, children, bones, worn): (
        Query<&GlobalTransform>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    mut meshes: ResMut<Assets<Mesh>>,
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let (right, up) = (iso::right(), iso::up());
    let s = iso::MODEL_SCALE;
    for (entity, mut sight, mut transform) in &mut sights {
        let sight = &mut *sight;
        sight.look.update(dt);
        if sight.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        if let Ok(root) = places.get(sight.target).map(GlobalTransform::translation) {
            transform.translation =
                crate::making::bone(sight.target, &NECK, &children, (&bones, &worn))
                    .unwrap_or(root);
        }
        let on_screen = |[x, y, _]: [f32; 3]| (right * x + up * y) * s;
        let mut quads = Quads::default();
        for streak in &sight.look.streaks {
            for (k, pair) in streak.windows(2).enumerate() {
                let (a, b) = (on_screen(pair[0]), on_screen(pair[1]));
                let across = (b - a).cross(right.cross(up)).normalize_or_zero() * STREAK_HALF;
                let (head, tail) = (PARADE_COLOURS[k], PARADE_COLOURS[k + 1]);
                quads.push_corners_shaded(
                    [a - across, b - across, b + across, a + across],
                    [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                    [head, tail, tail, head],
                );
            }
        }
        if let Some(mut mesh) = meshes.get_mut(&sight.mesh) {
            quads.in_order(orders.behind(sight.target), transform.translation);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The blows' sights.
pub struct BlowSightsPlugin;

impl Plugin for BlowSightsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_systems(OnEnter(AppState::InGame), read)
            .add_systems(
                Update,
                (make, bleed, parry)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
