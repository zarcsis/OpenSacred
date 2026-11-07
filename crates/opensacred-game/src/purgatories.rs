//! The look of the purgatory (#1092; docs/re/effects.md, "The purgatory"): with its object
//! (`spells::purgatory`), `cParticleSystem_hellFire` (820, `0x776390`) leaves flames standing on
//! the ground about it, added by their alpha (`opensacred_rules::particles::hellfire`), and while
//! it burns draws its column of light of its vertices' colours alone, added by their alpha
//! (`particles::hellfire_column`, `holy_lights::column_quads`), over a glow on the ground added
//! whole; all untinted by the night, on the ground under its object, its light of 200 with it. Its
//! object gone, its column and glow go and its flames burn out. A creature's is blue, its flames
//! white of `PARTICLE_FIRE09` (`+0xA9`).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, Column, System, hellfire, hellfire_column};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::models;
use crate::frame::Playing;
use crate::fx::{
    Effect, Orders, Quads, empty, particle_material, particle_texture, plain_material,
    whole_material,
};
use crate::game::{AppState, Data};
use crate::terrain::{Heights, Level};
use crate::{daynight, iso};

/// How far over the ground the glow lies, in world units, so that the ground's own faces do not
/// cut it.
const GROUND_LIFT: f32 = 0.5;

/// A purgatory's object to give its look, a hero's or a creature's, on its level (`spells`).
#[derive(Component)]
pub(crate) struct Look {
    pub hero: bool,
    pub level: Level,
}

/// A purgatory's look, which outlives its object while its flames burn out: its object while it
/// lives, its kind (0 a hero's, 1 a creature's), its level, its flames, their dice, its place on
/// the ground and the place before, its lamp, and its parts' meshes, the flames', the column's
/// and the glow's.
#[derive(Component)]
struct Shown {
    object: Option<Entity>,
    kind: usize,
    level: Level,
    flames: System,
    dice: Rng,
    place: Vec3,
    before: Option<Vec3>,
    lamp: Entity,
    parts: [Handle<Mesh>; 3],
}

/// The flames' systems and the columns, a hero's and a creature's, and the flames', the column's
/// and the glow's materials, made as the first purgatory shows; none where a texture is missing.
#[derive(Resource)]
struct Looks {
    fires: [particles::Fire; 2],
    columns: [Column; 2],
    materials: Option<[Option<Handle<Effect>>; 4]>,
}

impl Default for Looks {
    fn default() -> Self {
        Self {
            fires: [hellfire(true), hellfire(false)],
            columns: [hellfire_column(true), hellfire_column(false)],
            materials: None,
        }
    }
}

/// The seed of the looks' `rand()` (**estimate**: any).
const SEED: usize = 0x334;

/// The ground under `place` on `level`, else its own height.
fn on_ground(heights: &Heights, place: Vec3, level: Level) -> Vec3 {
    place.with_y(heights.standing(place.x, place.z, level).unwrap_or(place.y))
}

/// Gives each new purgatory its look (`0x776860`): an entity of its own, its lamp and its parts.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn show(
    mut commands: Commands,
    purgatories: Query<(Entity, &Look, &GlobalTransform), Added<Look>>,
    mut looks: ResMut<Looks>,
    heights: Res<Heights>,
    (data, assets, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (object, look, place) in &purgatories {
        let textures = looks.fires.each_ref().map(|fire| fire.texture);
        let made = looks
            .materials
            .get_or_insert_with(|| {
                let image = |name: &str| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("purgatories: no texture {name}; its part is not drawn");
                    }
                    image
                };
                let [hero, creature] = textures.map(|name| {
                    image(name).map(|image| materials.add(particle_material(image, AlphaMode::Add)))
                });
                let column = Some(materials.add(plain_material()));
                let glow = image(particles::HELLFIRE_GLOW_TEXTURE)
                    .map(|image| materials.add(whole_material(image)));
                [hero, creature, column, glow]
            })
            .clone();
        let kind = usize::from(!look.hero);
        let place = on_ground(&heights, place.translation(), look.level);
        let lamp = commands
            .spawn((
                Transform::from_translation(place.with_y(0.0)),
                daynight::Light(looks.fires[kind].light),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let shown = commands
            .spawn((
                Transform::IDENTITY,
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let parts = [
            meshes.add(empty()),
            meshes.add(empty()),
            meshes.add(empty()),
        ];
        let [hero, creature, column, glow] = made;
        let flames = if look.hero { hero } else { creature };
        for (mesh, material) in parts.iter().zip([flames, column, glow]) {
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
        commands.entity(shown).insert(Shown {
            object: Some(object),
            kind,
            level: look.level,
            flames: System::new(looks.fires[kind].count),
            dice: crate::seed::dice(SEED ^ index),
            place,
            before: None,
            lamp,
            parts,
        });
    }
}

/// Runs each purgatory's look a frame on (`0x7762D0`, `0x776390`): while its object lives, flames
/// at its place, its column and its glow, all drawn at its object's place in the order of
/// drawing, before a creature standing in its cell, as the original draws the system before the
/// creature (`fx::Orders`, #2178), but in a building, where they keep their own depths; then its
/// flames burn out and it goes, its lamp with it.
fn burn(
    mut commands: Commands,
    (time, looks, heights): (Res<Time>, Res<Looks>, Res<Heights>),
    mut shown: Query<(Entity, &mut Shown)>,
    objects: Query<&GlobalTransform, With<Look>>,
    mut lamps: Query<&mut Transform, With<daynight::Light>>,
    mut meshes: ResMut<Assets<Mesh>>,
    orders: Orders,
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut look) in &mut shown {
        let look = &mut *look;
        let fire = &looks.fires[look.kind];
        let burning = match look.object.map(|object| objects.get(object)) {
            Some(Ok(place)) => {
                look.place = on_ground(&heights, place.translation(), look.level);
                true
            }
            _ => {
                look.object = None;
                false
            }
        };
        look.flames.update(dt, &fire.layers);
        if burning {
            let before = look.before.unwrap_or(look.place);
            look.flames.emit_along(
                &fire.blocks,
                &fire.layers,
                fire.count,
                (models(before), models(look.place)),
                dt,
                &mut look.dice,
            );
        }
        look.before = Some(look.place);
        if !burning && look.flames.particles.is_empty() {
            commands.entity(look.lamp).despawn();
            commands.entity(entity).despawn();
            continue;
        }
        if let Ok(mut lamp) = lamps.get_mut(look.lamp) {
            lamp.translation = look.place.with_y(0.0);
        }
        let (mut column, mut glow) = (Quads::default(), Quads::default());
        if burning {
            let c = &looks.columns[look.kind];
            let (across, depth) = iso::fx_axes();
            let place = look.place;
            let at = |x: f32, y: f32, z: f32| place + (across * x + depth * y + Vec3::Y * z) * s;
            column = crate::holy_lights::column_quads(c, c.alphas, at);
            let half = particles::HELLFIRE_GLOW_HALF * s;
            let ground = place + Vec3::Y * GROUND_LIFT;
            let [r, g, b] = particles::HELLFIRE_COLOURS[look.kind];
            glow.push_corners(
                [
                    ground + (-across - depth) * half,
                    ground + (across - depth) * half,
                    ground + (across + depth) * half,
                    ground + (-across + depth) * half,
                ],
                [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                [r, g, b, 1.0],
            );
        }
        let quads = [crate::breath::wall_quads(fire, &look.flames), column, glow];
        let order = look
            .level
            .is_none()
            .then(|| orders.at(look.place))
            .flatten();
        for (handle, mut part) in look.parts.iter().zip(quads) {
            if let Some(mut mesh) = meshes.get_mut(handle) {
                part.in_order(order, Vec3::ZERO);
                part.into_mesh(&mut mesh);
            }
        }
    }
}

/// The purgatories' looks.
pub struct PurgatoriesPlugin;

impl Plugin for PurgatoriesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Looks>().add_systems(
            Update,
            (show, burn)
                .chain()
                .in_set(Playing)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
