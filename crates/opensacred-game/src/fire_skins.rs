//! The look of the fire skin and of the fire spiral's flames (#1092; docs/re/effects.md, "The fire
//! skin", "The fire spiral"): on its bearer for its seconds (`spells`), `cParticleSystem_fireskin`
//! (821, `0x78BBA0`) leaves flames standing on the ground about it, added by their alpha
//! (`opensacred_rules::particles::fire_skin`), over a glow on the ground added whole, both untinted
//! by the night, its light of 200 with it; its seconds over, its flames burn out. A creature's is
//! blue, of `PARTICLE_FIRE09` (`+0xA9`). Each of the spiral's five objects leaves the wider orange
//! flames of `cParticleSystem_fireCircle` (790, `0x775BC0`; `particles::fire_circle`) where it
//! goes, with no glow, its light of 300.
//! Its bearer's skin of lava is `lava_skins`'.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::particles::{self, System, fire_circle, fire_skin};
use opensacred_rules::spawning::Rng;

use crate::bolt_looks::models;
use crate::frame::Playing;
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture, whole_material};
use crate::game::{AppState, Data};
use crate::{daynight, iso};

/// How far over the ground the glow lies, in world units, so that the ground's own faces do not
/// cut it.
const GROUND_LIFT: f32 = 0.5;

/// An object to give its look (`spells`): a fire skin, a hero's or a creature's, or a flame of
/// the fire spiral.
#[derive(Component, Clone, Copy)]
pub(crate) enum Look {
    Skin { hero: bool },
    Spiral,
}

impl Look {
    /// Its kind, the index of its fire: 0 a hero's skin, 1 a creature's, 2 the spiral's flame.
    fn kind(self) -> usize {
        match self {
            Self::Skin { hero: true } => 0,
            Self::Skin { hero: false } => 1,
            Self::Spiral => 2,
        }
    }
}

/// A look, which outlives its object while its flames burn out: its object while it lives, its
/// kind ([`Look::kind`]), its flames, their dice, its place and the place before, its lamp, and
/// its parts' meshes, the flames' and the glow's.
#[derive(Component)]
struct Skin {
    object: Option<Entity>,
    kind: usize,
    flames: System,
    dice: Rng,
    place: Vec3,
    before: Option<Vec3>,
    lamp: Entity,
    parts: [Handle<Mesh>; 2],
}

/// The flames' systems by their kind, and their materials and the glow's (the fire skin's alone),
/// made as the first look shows; none where a texture is missing.
#[derive(Resource)]
struct Looks {
    fires: [particles::Fire; 3],
    materials: Option<[Option<Handle<Effect>>; 4]>,
}

impl Default for Looks {
    fn default() -> Self {
        Self {
            fires: [fire_skin(true), fire_skin(false), fire_circle()],
            materials: None,
        }
    }
}

/// The index of the glow's material among the looks' materials.
const GLOW: usize = 3;

/// The seed of the looks' `rand()` (**estimate**: any).
const SEED: usize = 0x335;

/// Gives each new object its look (`0x78BE10`, `0x775DF0`): an entity of its own, its lamp and
/// its parts.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments"
)]
fn show(
    mut commands: Commands,
    skins: Query<(Entity, &Look, &GlobalTransform), Added<Look>>,
    mut looks: ResMut<Looks>,
    (data, assets, mut materials, mut meshes): (
        Option<Res<Data>>,
        Res<AssetServer>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (object, look, place) in &skins {
        let textures = looks.fires.each_ref().map(|fire| fire.texture);
        let made = looks
            .materials
            .get_or_insert_with(|| {
                let image = |name: &str| {
                    let image = data
                        .as_ref()
                        .and_then(|data| particle_texture(&data.0.assets, &assets, name));
                    if image.is_none() {
                        warn!("fire skins: no texture {name}; its part is not drawn");
                    }
                    image
                };
                let [hero, creature, spiral] = textures.map(|name| {
                    image(name).map(|image| materials.add(particle_material(image, AlphaMode::Add)))
                });
                let glow = image(particles::FIRE_SKIN_GLOW_TEXTURE)
                    .map(|image| materials.add(whole_material(image)));
                [hero, creature, spiral, glow]
            })
            .clone();
        let kind = look.kind();
        let place = place.translation();
        let lamp = commands
            .spawn((
                Transform::from_translation(place.with_y(0.0)),
                daynight::Light(looks.fires[kind].light),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let skin = commands
            .spawn((
                Transform::IDENTITY,
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        let parts = [meshes.add(empty()), meshes.add(empty())];
        let glow = made[GLOW]
            .clone()
            .filter(|_| matches!(look, Look::Skin { .. }));
        for (mesh, material) in parts.iter().zip([made[kind].clone(), glow]) {
            if let Some(material) = material {
                commands.spawn((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                    RenderLayers::layer(iso::UNTINTED),
                    ChildOf(skin),
                ));
            }
        }
        let index = usize::try_from(skin.index_u32()).unwrap_or(0);
        commands.entity(skin).insert(Skin {
            object: Some(object),
            kind,
            flames: System::new(looks.fires[kind].count),
            dice: crate::seed::dice(SEED ^ index),
            place,
            before: None,
            lamp,
            parts,
        });
    }
}

/// Runs each look a frame on (`0x78BB00`, `0x78BBA0`; `0x775A70`, `0x775BC0`): while its object
/// lives, flames at its place and, for a fire skin, the glow under it; then its flames burn out
/// and it goes, its lamp with it.
fn burn(
    mut commands: Commands,
    time: Res<Time>,
    looks: Res<Looks>,
    mut skins: Query<(Entity, &mut Skin)>,
    objects: Query<&GlobalTransform, With<Look>>,
    mut lamps: Query<&mut Transform, With<daynight::Light>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().min(0.1);
    for (entity, mut skin) in &mut skins {
        let skin = &mut *skin;
        let fire = &looks.fires[skin.kind];
        let burning = match skin.object.map(|object| objects.get(object)) {
            Some(Ok(place)) => {
                skin.place = place.translation();
                true
            }
            _ => {
                skin.object = None;
                false
            }
        };
        skin.flames.update(dt, &fire.layers);
        if burning {
            let before = skin.before.unwrap_or(skin.place);
            skin.flames.emit_along(
                &fire.blocks,
                &fire.layers,
                fire.count,
                (models(before), models(skin.place)),
                dt,
                &mut skin.dice,
            );
        }
        skin.before = Some(skin.place);
        if !burning && skin.flames.particles.is_empty() {
            commands.entity(skin.lamp).despawn();
            commands.entity(entity).despawn();
            continue;
        }
        if let Ok(mut lamp) = lamps.get_mut(skin.lamp) {
            lamp.translation = skin.place.with_y(0.0);
        }
        if let Some(mut mesh) = meshes.get_mut(&skin.parts[0]) {
            crate::breath::wall_quads(fire, &skin.flames).into_mesh(&mut mesh);
        }
        if let Some(mut mesh) = meshes.get_mut(&skin.parts[1]) {
            let mut glow = Quads::default();
            if burning && skin.kind < 2 {
                let s = iso::MODEL_SCALE;
                let (across, down) = iso::fx_axes();
                let half = particles::FIRE_SKIN_GLOW_HALF * s;
                let ground = skin.place + Vec3::Y * GROUND_LIFT;
                glow.push_corners(
                    [
                        ground + (-across - down) * half,
                        ground + (across - down) * half,
                        ground + (across + down) * half,
                        ground + (-across + down) * half,
                    ],
                    [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y],
                    particles::FIRE_SKIN_GLOW_COLOURS[skin.kind],
                );
            }
            glow.into_mesh(&mut mesh);
        }
    }
}

/// The fire skins' and the fire spiral's looks.
pub struct FireSkinsPlugin;

impl Plugin for FireSkinsPlugin {
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
