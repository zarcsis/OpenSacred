//! The light strikes (#1092; docs/re/effects.md, "The light strikes"): the sights of the Mage's
//! stone skin (769), water form (783) and quickening (825) and of the Seraphim's crusader's
//! strength (818) as `spells` casts them, until they give their effects. A strike of light 60
//! wide flickers down onto its bearer's neck, its frame of eight rolled anew every 0.07 s, opaque
//! at its foot and clear at its top, added by its alpha, over a glow added whole; it stands in
//! the models' upright plane, facing the viewer, and is untinted by the night; its light of 200
//! stands where its object was made (`opensacred_rules::particles::LightStrike`).
//!
//! Not followed: the strike and its glow over all else (their depth test off, `0x770F30`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    ICE_STRIKES, LIGHT_STRIKE_GLOW, LIGHT_STRIKE_LIGHT, LIGHT_STRIKE_QUAD, LightStrike, StrikeKind,
};

use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::spawning::Rng;
use crate::{daynight, iso};

/// The bone the strikes stand at (`+0x70`, 3), else the root.
const NECK: [&str; 1] = ["Bip01 Neck"];

/// The four kinds in the order of their glows' materials.
const KINDS: [StrikeKind; 4] = [
    StrikeKind::StoneSkin,
    StrikeKind::WaterForm,
    StrikeKind::Crusader,
    StrikeKind::Quickening,
];

/// A strike to make: its kind, its bearer, the creature at whose place its object is made, and
/// the cast's rate.
#[derive(Component)]
struct Unmade {
    kind: StrikeKind,
    bearer: Entity,
    made_at: Entity,
    rate: f32,
}

/// A strike that shows: its bearer, its look, whether its bearer is no hero, its lamp, and its
/// meshes, the strike's by its four textures and its glow's.
#[derive(Component)]
struct Shown {
    bearer: Entity,
    look: LightStrike,
    creature: bool,
    lamp: Entity,
    parts: [Handle<Mesh>; 5],
}

/// The strikes' materials, made as the first asks: the strike's four textures added by their
/// alpha, then the four kinds' glows added whole; none where a texture is missing.
#[derive(Resource, Default)]
struct Looks(Option<[Option<Handle<Effect>>; 8]>);

/// The strikes' dice (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x301))
    }
}

/// Starts the strike of `kind` on `bearer`, its object made at `made_at`'s place (the stone
/// skin's at its bearer, the others' at their caster), its clock at the cast's rate `rate`.
pub(crate) fn strike(
    commands: &mut Commands,
    kind: StrikeKind,
    (bearer, made_at): (Entity, Entity),
    rate: f32,
) {
    commands.spawn((
        Unmade {
            kind,
            bearer,
            made_at,
            rate,
        },
        DespawnOnExit(AppState::InGame),
    ));
}

/// Gives each new strike its look at its bearer, its lamp, its meshes and its materials
/// (`0x770C70`, `0x7712E0` and their copies).
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    (places, heroes): (Query<&GlobalTransform>, Query<(), With<crate::hero::Body>>),
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade) in &unmade {
        let Ok(at) = places.get(unmade.bearer) else {
            commands.entity(entity).despawn();
            continue;
        };
        let made_at = places
            .get(unmade.made_at)
            .map_or(at.translation(), GlobalTransform::translation);
        let materials = looks
            .0
            .get_or_insert_with(|| {
                let mut material = |name: &str, whole: bool| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("light strikes: no texture {name}; its part is not drawn");
                    }
                    image.map(|image| {
                        effects.add(if whole {
                            whole_material(image)
                        } else {
                            particle_material(image, AlphaMode::Add)
                        })
                    })
                };
                let [a, b, c, d] = ICE_STRIKES.map(|name| material(name, false));
                let [e, f, g, h] = KINDS.map(|kind| material(kind.glow(), true));
                [a, b, c, d, e, f, g, h]
            })
            .clone();
        let glow = KINDS
            .iter()
            .position(|&kind| kind == unmade.kind)
            .map_or(4, |kind| 4 + kind);
        let lamp = commands
            .spawn((
                Transform::from_translation(made_at.with_y(0.0)),
                daynight::Light(LIGHT_STRIKE_LIGHT),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let parts: [Handle<Mesh>; 5] = std::array::from_fn(|_| meshes.add(empty()));
        let mut made = commands.entity(entity);
        made.remove::<Unmade>().insert((
            Shown {
                bearer: unmade.bearer,
                look: LightStrike::new(unmade.kind, unmade.rate),
                creature: !heroes.contains(unmade.bearer),
                lamp,
                parts: parts.clone(),
            },
            Transform::from_translation(at.translation()),
            Visibility::default(),
        ));
        let chosen = [0, 1, 2, 3, glow].map(|index| materials[index].clone());
        for (mesh, material) in parts.into_iter().zip(chosen) {
            if let Some(material) = material {
                made.with_child((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                ));
            }
        }
    }
}

/// Runs each strike a frame on (`0x770EA0`, `0x770F30` and their copies): at its bearer's neck,
/// else its place, its frame's texture and breadth, its glow 2 under the neck; it goes with its
/// lamp once its time is over, or with its bearer.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut strikes: Query<(Entity, &mut Shown, &mut Transform)>,
    (places, children, bones, worn): (
        Query<&GlobalTransform>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut meshes, mut dice): (ResMut<Assets<Mesh>>, ResMut<Dice>),
) {
    let dt = time.delta_secs();
    let s = iso::MODEL_SCALE;
    let (across, depth) = iso::fx_axes();
    let frame = |[x, y, z]: [f32; 3]| (across * x + depth * y + Vec3::Y * z) * s;
    for (entity, mut shown, mut transform) in &mut strikes {
        let shown = &mut *shown;
        shown.look.update(dt, &mut dice.0);
        let place = places.get(shown.bearer).map(GlobalTransform::translation);
        if shown.look.done() || place.is_err() {
            commands.entity(shown.lamp).despawn();
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation =
            crate::making::bone(shown.bearer, &NECK, &children, (&bones, &worn))
                .or(place.ok())
                .unwrap_or(transform.translation);
        let mut quads: [Quads; 5] = std::array::from_fn(|_| Quads::default());
        let (texture, [left, right]) = shown.look.strike();
        let [x0, x1, z0, z1] = LIGHT_STRIKE_QUAD;
        let [r, g, b, _] = shown.look.kind.colour(shown.creature);
        let (foot, top) = ([r, g, b, 1.0], [r, g, b, 0.0]);
        if let Some(strike) = quads.get_mut(texture) {
            strike.push_corners_shaded(
                [
                    frame([x0, 0.0, z0]),
                    frame([x1, 0.0, z0]),
                    frame([x1, 0.0, z1]),
                    frame([x0, 0.0, z1]),
                ],
                [
                    Vec2::new(0.0, left),
                    Vec2::new(0.0, right),
                    Vec2::new(1.0, right),
                    Vec2::new(1.0, left),
                ],
                [foot, foot, top, top],
            );
        }
        let [half, height] = LIGHT_STRIKE_GLOW;
        quads[4].push(
            frame([0.0, 0.0, height]),
            half * s,
            0.0,
            (Vec2::ZERO, 1.0),
            shown.look.kind.glow_colour(shown.creature),
        );
        for (handle, part) in shown.parts.iter().zip(quads) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The light strikes.
pub struct LightStrikesPlugin;

impl Plugin for LightStrikesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>()
            .init_resource::<Dice>()
            .add_systems(
                Update,
                (make, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
