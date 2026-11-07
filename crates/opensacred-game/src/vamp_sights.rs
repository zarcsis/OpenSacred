//! The Vampiress's domination's ring and her raising's pentagram (#1092; docs/re/effects.md, "The
//! Vampiress's domination and raising"): the domination (1024, 1025) shows
//! `cParticleSystem_beherrschung` (857) about the dominated creature's neck, a band of runes of its
//! size's radius laid over in white, their red glow added twice, brightest in a band of twelve
//! columns that sweeps round with a red glow and a red light at its head; the raising (1034)
//! `cParticleSystem_liveundead` (866) under the rising dead, a pentagram and a ring of runes flat
//! on the ground, laid over and glowing red, turning against each other. Both untinted by the
//! night and drawn just behind their creature's model, as the original draws the system before
//! the creature it follows (`fx::Orders`, #2178) (`opensacred_rules::particles::{Domination,
//! Raising}`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{
    DOMINATION_GLOW, DOMINATION_GLOW_HALF, DOMINATION_HALF_HEIGHT, DOMINATION_LIGHT,
    DOMINATION_SEGMENTS, DOMINATION_TEXTURES, Domination, RAISING_HALVES, RAISING_TEXTURES,
    Raising,
};

use crate::frame::{Playing, Unpaused};
use crate::fx::{
    Effect, InOrder, Orders, Quads, empty, particle_material, particle_texture, repeated_texture,
    whole_material,
};
use crate::game::{AppState, Data};
use crate::iso;

/// The bone the domination's ring stands at, else the root (`0x5A4D63`: the bone 3).
const NECK: [&str; 1] = ["Bip01 Neck"];

/// A sight to make about its creature ([`make`]).
#[derive(Component)]
enum Unmade {
    Domination { target: Entity, seconds: f32 },
    Raising { target: Entity, place: Vec3 },
}

/// A domination's ring that shows: its creature, its look, its meshes (the runes', their glow's
/// and the head's glow's) and its light.
#[derive(Component)]
struct Ring {
    target: Entity,
    look: Domination,
    parts: [Handle<Mesh>; 3],
    light: Entity,
}

/// A raising's pentagram that shows: its creature, its look, its place, the depth of its place
/// in the order while its creature is there, and its meshes (the pentagram's, the ring's, and
/// their glows').
#[derive(Component)]
struct Pentagram {
    target: Entity,
    look: Raising,
    place: Vec3,
    order: Option<InOrder>,
    parts: [Handle<Mesh>; 4],
}

/// The sights' materials, made as first asked, none where a texture is missing: the ring's runes
/// laid over by their alpha, their glow and the head's glow added whole; the raising's pentagram
/// and ring laid over, their glows added whole.
#[derive(Resource, Default)]
struct Looks(Option<Materials>);

/// [`Looks`]' materials.
struct Materials {
    ring: [Option<Handle<Effect>>; 3],
    raising: [Option<Handle<Effect>>; 4],
}

/// Starts the domination's ring about `target` for its `seconds` (`0x5124CE`, the event `0x51`).
pub(crate) fn dominate(commands: &mut Commands, target: Entity, seconds: f32) {
    commands.spawn((
        Unmade::Domination { target, seconds },
        DespawnOnExit(AppState::InGame),
    ));
}

/// Starts the raising's pentagram under `target`, a corpse lying at `place` (`0x51127E`, the event
/// `0x5A`): the game makes the risen creature anew in its place (`followers::raise`), so the
/// pentagram stays there once the corpse is gone, before a creature standing on it.
pub(crate) fn raise(commands: &mut Commands, target: Entity, place: Vec3) {
    commands.spawn((
        Unmade::Raising { target, place },
        DespawnOnExit(AppState::InGame),
    ));
}

/// Gives each new sight its look at its creature, its meshes and its materials (`0x79D480`,
/// `0x79DBF0`; `0x79E180`, `0x79E5D0`), and the ring its light.
fn make(
    mut commands: Commands,
    unmade: Query<(Entity, &Unmade)>,
    (places, sizes): (Query<&GlobalTransform>, Query<&crate::arrows::Size>),
    (data, assets, mut looks): (Option<Res<Data>>, Res<AssetServer>, ResMut<Looks>),
    (mut effects, mut meshes): (ResMut<Assets<Effect>>, ResMut<Assets<Mesh>>),
) {
    for (entity, unmade) in &unmade {
        let at = match *unmade {
            Unmade::Domination { target, .. } => {
                places.get(target).map(GlobalTransform::translation)
            }
            Unmade::Raising { target, place } => Ok(places
                .get(target)
                .map_or(place, GlobalTransform::translation)),
        };
        let Ok(at) = at else {
            commands.entity(entity).despawn();
            continue;
        };
        let materials = looks.0.get_or_insert_with(|| {
            let mut material = |name: &str, whole: bool, repeated: bool| {
                let image = data.as_ref().and_then(|data| {
                    if repeated {
                        repeated_texture(&data.0.assets, &assets, name)
                    } else {
                        particle_texture(&data.0.assets, &assets, name)
                    }
                });
                if image.is_none() {
                    warn!("vamp sights: no texture {name}; its part is not drawn");
                }
                image.map(|image| {
                    effects.add(if whole {
                        whole_material(image)
                    } else {
                        particle_material(image, AlphaMode::Blend)
                    })
                })
            };
            let [runes, glow, head] = DOMINATION_TEXTURES;
            let [penta, ring, penta_glow, ring_glow] = RAISING_TEXTURES;
            Materials {
                ring: [
                    material(runes, false, true),
                    material(glow, true, true),
                    material(head, true, false),
                ],
                raising: [
                    material(penta, false, false),
                    material(ring, false, false),
                    material(penta_glow, true, false),
                    material(ring_glow, true, false),
                ],
            }
        });
        let chosen: Vec<Option<Handle<Effect>>> = match unmade {
            Unmade::Domination { .. } => materials.ring.to_vec(),
            Unmade::Raising { .. } => materials.raising.to_vec(),
        };
        let parts: Vec<Handle<Mesh>> = chosen.iter().map(|_| meshes.add(empty())).collect();
        let mut made = commands.entity(entity);
        made.remove::<Unmade>()
            .insert((Transform::from_translation(at), Visibility::default()));
        for (mesh, material) in parts.iter().zip(chosen) {
            if let Some(material) = material {
                made.with_child((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                ));
            }
        }
        match *unmade {
            Unmade::Domination { target, seconds } => {
                let size = sizes.get(target).map_or(50.0, |size| size.0);
                // Its Direct3D light, at the band's head (`0x642B90`, `0x642EF0`).
                let light = commands
                    .spawn((
                        crate::fx_lights::FxLight {
                            colour: DOMINATION_LIGHT,
                            owner: entity,
                            follows: None,
                            offset: Vec3::ZERO,
                        },
                        Transform::IDENTITY,
                        Visibility::default(),
                        ChildOf(entity),
                    ))
                    .id();
                commands.entity(entity).insert(Ring {
                    target,
                    look: Domination::new(seconds, size),
                    parts: [parts[0].clone(), parts[1].clone(), parts[2].clone()],
                    light,
                });
            }
            Unmade::Raising { target, .. } => {
                commands.entity(entity).insert(Pentagram {
                    target,
                    look: Raising::default(),
                    place: at,
                    order: None,
                    parts: [
                        parts[0].clone(),
                        parts[1].clone(),
                        parts[2].clone(),
                        parts[3].clone(),
                    ],
                });
            }
        }
    }
}

/// The point of the models' frame `[x, y, z]` turned by `turn` about the upright, which takes the
/// band's point at the angle `a`, `(sin a, cos a)`, to the angle `a + turn`, in world units.
fn turned([x, y, z]: [f32; 3], turn: f32) -> Vec3 {
    let (sin, cos) = turn.sin_cos();
    let (across, depth) = iso::fx_axes();
    (across * (x * cos + y * sin) + depth * (y * cos - x * sin) + Vec3::Y * z) * iso::MODEL_SCALE
}

/// Runs each domination's ring a frame on (`0x79D610`) and draws it (`0x79D770`): at its
/// creature's neck, else its root, the band of runes laid over in white, then their glow added
/// twice in the band's greys, and the glow at the band's head with its light; it goes once it has
/// shrunk away, or with its creature.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn ring_on(
    mut commands: Commands,
    time: Res<Time>,
    mut rings: Query<(Entity, &mut Ring, &mut Transform)>,
    mut lights: Query<&mut Transform, (With<crate::fx_lights::FxLight>, Without<Ring>)>,
    (places, children, bones, worn): (
        Query<&GlobalTransform>,
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    mut meshes: ResMut<Assets<Mesh>>,
    orders: Orders,
) {
    let dt = time.delta_secs();
    for (entity, mut ring, mut transform) in &mut rings {
        let ring = &mut *ring;
        let Ok(at) = places.get(ring.target) else {
            commands.entity(entity).despawn();
            continue;
        };
        ring.look.update(dt);
        if ring.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        let neck = crate::making::bone(ring.target, &NECK, &children, (&bones, &worn))
            .unwrap_or(at.translation());
        transform.translation = neck;
        let (radius, growth, turn) = (ring.look.radius(), ring.look.growth(), ring.look.turn());
        let half = DOMINATION_HALF_HEIGHT * growth;
        let point = |column: u8, z: f32| {
            let a = std::f32::consts::TAU * f32::from(column) / f32::from(DOMINATION_SEGMENTS);
            turned([radius * a.sin(), radius * a.cos(), z], turn)
        };
        let (mut runes, mut glows) = (Quads::default(), Quads::default());
        for column in 0..DOMINATION_SEGMENTS {
            let next = column + 1;
            let corners = [
                point(column, -half),
                point(next, -half),
                point(next, half),
                point(column, half),
            ];
            let (u0, u1) = (f32::from(column) / 15.0, f32::from(next) / 15.0);
            let uvs = [
                Vec2::new(u0, 0.0),
                Vec2::new(u1, 0.0),
                Vec2::new(u1, 1.0),
                Vec2::new(u0, 1.0),
            ];
            runes.push_corners(corners, uvs, [1.0; 4]);
            let grey = |c: u8| {
                let g = ring.look.grey(c);
                [g, g, g, 1.0]
            };
            let colours = [grey(column), grey(next), grey(next), grey(column)];
            // Added twice (`0x79D770`).
            for _ in 0..2 {
                glows.push_corners_shaded(corners, uvs, colours);
            }
        }
        let angle = ring.look.glow_angle();
        let head = turned([radius * angle.sin(), radius * angle.cos(), 0.0], 0.0);
        let mut glow = Quads::default();
        glow.push(
            head,
            DOMINATION_GLOW_HALF * growth * iso::MODEL_SCALE,
            0.0,
            (Vec2::ZERO, 1.0),
            DOMINATION_GLOW,
        );
        if let Ok(mut light) = lights.get_mut(ring.light) {
            light.translation = head;
        }
        let order = orders.behind(ring.target);
        for (handle, mut part) in ring.parts.iter().zip([runes, glows, glow]) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, neck);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// Runs each raising's pentagram a frame on (`0x79E250`) and draws it (`0x79E300`): at its
/// creature's root, the pentagram and the ring of runes flat on the ground, laid over in white at
/// its fade's alpha, then their glows added in its fade's grey, the pentagram turned by its turn
/// and the ring by minus it; it goes once it has faded out.
fn pentagram_on(
    mut commands: Commands,
    time: Res<Time>,
    mut pentagrams: Query<(Entity, &mut Pentagram, &mut Transform)>,
    places: Query<&GlobalTransform>,
    mut meshes: ResMut<Assets<Mesh>>,
    orders: Orders,
) {
    let dt = time.delta_secs();
    for (entity, mut sight, mut transform) in &mut pentagrams {
        let sight = &mut *sight;
        // At its creature while it is there, else at its place, before a creature standing on it.
        if let Ok(at) = places.get(sight.target) {
            sight.place = at.translation();
            sight.order = orders.behind(sight.target);
        } else {
            sight.order = orders.at(sight.place);
        }
        sight.look.update(dt);
        if sight.look.done() {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation = sight.place;
        let (fade, turn) = (sight.look.fade(), sight.look.turn());
        let flat = |half: f32, angle: f32, colour: [f32; 4]| {
            let mut quads = Quads::default();
            quads.push_corners(
                [
                    turned([-half, -half, 0.0], angle),
                    turned([half, -half, 0.0], angle),
                    turned([half, half, 0.0], angle),
                    turned([-half, half, 0.0], angle),
                ],
                [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                colour,
            );
            quads
        };
        let [penta, ring] = RAISING_HALVES;
        let (white, grey) = ([1.0, 1.0, 1.0, fade], [fade, fade, fade, 1.0]);
        let quads = [
            flat(penta, turn, white),
            flat(ring, -turn, white),
            flat(penta, turn, grey),
            flat(ring, -turn, grey),
        ];
        for (handle, mut part) in sight.parts.iter().zip(quads) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(sight.order, sight.place);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The Vampiress's domination's ring and her raising's pentagram.
pub struct VampSightsPlugin;

impl Plugin for VampSightsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>().add_systems(
            Update,
            (make, ring_on, pentagram_on)
                .chain()
                .in_set(Playing)
                .in_set(Unpaused)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
