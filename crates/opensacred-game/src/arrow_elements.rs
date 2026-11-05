//! The arrows' elements (#2061; docs/re/effects.md, "The arrows' elements"): an arrow of a weapon
//! whose fire, poison or magic passes a fifth of its physical damage shows it
//! (`opensacred_rules::arrows::element`): a line of glows along its shaft, its bones `weapon_fx01`
//! and `weapon_fx02` (`0x5BEE90`), and for the fire and the poison sparks along it that ride with
//! it (`opensacred_rules::particles::ArrowSparks`), the fire lighting the night; all added and
//! untinted by the night, while the arrow lives. A weapon in hand shows its own element likewise
//! (#2089, `opensacred_rules::arrows::weapon_element`), while its model is in the hand.

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;
use opensacred_rules::arrows::{self, Element};
use opensacred_rules::particles::{self, ArrowSparks};

use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::spawning::Rng;
use crate::{bolt_looks, daynight, iso};

/// The bones of an arrow's shaft or a weapon's line (15, 16 and 17).
const SHAFT: [&str; 3] = ["weapon_fx01", "weapon_fx02", "weapon_fx03"];

/// An arrow's element and its share, which its shot gives it (`arrows`, `0x5BD2D0`), or a
/// weapon's in hand, which its values give it ([`wield`]).
#[derive(Component, Clone, Copy, Debug)]
pub struct Elemental {
    pub element: Element,
    pub share: f32,
}

/// The sparks' dice.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x796))
    }
}

/// The glows' material, made at the first arrow that shows; none where the texture is missing.
#[derive(Resource, Default)]
struct Material(Option<Option<Handle<Effect>>>);

/// An arrow's element shown: its arrow, its sparks, its shaft's bones that its model has from the
/// first on, once it has the first, where the shaft was a frame ago in the frame of its arrow's
/// place, and the meshes of its sparks and its line.
#[derive(Component)]
struct Shown {
    arrow: Entity,
    elemental: Elemental,
    sparks: Option<ArrowSparks>,
    shaft: Option<Vec<Entity>>,
    last: Option<[[f32; 3]; 2]>,
    meshes: [Handle<Mesh>; 2],
}

/// Gives each weapon put into a hand the element of its values (`cWeapon3D::toggleVisuals`,
/// `0x5CDB60`): a creature's of its arms, the hero's of its weapon set's half for that hand.
fn wield(
    mut commands: Commands,
    wielded: Query<
        (Entity, &crate::npcs::Wielded, &crate::models::OrderedWith),
        Added<crate::npcs::Wielded>,
    >,
    holders: Query<&crate::npcs::Arms>,
    sets: Option<Res<crate::sets::WeaponSets>>,
    (tables, rules): (
        Option<Res<crate::drops::Tables>>,
        Option<Res<crate::fight::Rules>>,
    ),
) {
    let (Some(tables), Some(rules)) = (tables, rules) else {
        return;
    };
    for (weapon, hand, holder) in &wielded {
        // A creature's of its arms, its type's record's where the item maker gave it none; the
        // hero's, which has none, of its set's half for that hand, else the other half's, where a
        // weapon of both hands lies in one half and its model in the other hand.
        let values = match holders.get(holder.0) {
            Ok(arms) => arms
                .in_hand(hand.slot())
                .map(|(item, values)| tables.own_values(&tables.held_item(item, 1, values))),
            Err(_) => sets.as_ref().and_then(|sets| {
                let halves = sets.halves.get(sets.selected)?;
                let [this, other] = [hand.half(), 1 - hand.half()]
                    .map(|half| halves[half].as_ref().map(|dropped| tables.values(dropped)));
                this.or(other)
            }),
        };
        if let Some((element, share)) =
            values.and_then(|v| rules.0.weapon_element(v.damage.map(|[_, most]| most)))
        {
            commands.entity(weapon).insert(Elemental { element, share });
        }
    }
}

/// Gives each arrow or weapon of an element its look, in the camera that draws after the night's
/// pass.
fn show(
    mut commands: Commands,
    arrows: Query<(Entity, &Elemental, &GlobalTransform), Added<Elemental>>,
    (data, assets): (Option<Res<Data>>, Res<AssetServer>),
    (mut made, mut materials, mut meshes): (
        ResMut<Material>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    for (arrow, elemental, at) in &arrows {
        let material = made
            .0
            .get_or_insert_with(|| {
                let image = data.as_ref().and_then(|data| {
                    particle_texture(&data.0.assets, &assets, particles::ARROW_SPARKS_TEXTURE)
                });
                if image.is_none() {
                    warn!(
                        "arrow elements: no texture {}; they are not drawn",
                        particles::ARROW_SPARKS_TEXTURE
                    );
                }
                image.map(|image| materials.add(particle_material(image, AlphaMode::Add)))
            })
            .clone();
        let sparks = ArrowSparks::new(elemental.element, elemental.share);
        debug!(
            "arrow elements: {arrow} shows {:?} at {:.2}",
            elemental.element, elemental.share
        );
        let parts: [Handle<Mesh>; 2] = std::array::from_fn(|_| meshes.add(empty()));
        let mut shown = commands.spawn((
            Transform::from_translation(at.translation()),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
        ));
        if let Some(light) = sparks.as_ref().map(ArrowSparks::light).filter(|&r| r > 0.0) {
            shown.insert(daynight::Light(light));
        }
        if let Some(material) = material {
            shown.with_children(|children| {
                for mesh in &parts {
                    children.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        Transform::IDENTITY,
                        NoFrustumCulling,
                        RenderLayers::layer(iso::UNTINTED),
                    ));
                }
            });
        }
        shown.insert(Shown {
            arrow,
            elemental: *elemental,
            sparks,
            shaft: None,
            last: None,
            meshes: parts,
        });
    }
}

/// Runs each arrow's or weapon's element a frame on at it: its sparks along the shaft the frame
/// swept between its first two bones (`0x77A650`), riding with its place (the system's flag 4),
/// and its line of glows over its bones (`0x40DA80`, `particles::glow_segments`); it goes with
/// its arrow or weapon.
fn run(
    mut commands: Commands,
    time: Res<Time>,
    mut shown: Query<(Entity, &mut Shown, &mut Transform)>,
    places: Query<&GlobalTransform>,
    (children, bones, worn): (
        Query<&Children>,
        Query<(&Name, &GlobalTransform)>,
        crate::making::Worn,
    ),
    (mut dice, mut meshes): (ResMut<Dice>, ResMut<Assets<Mesh>>),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut look, mut transform) in &mut shown {
        let Ok(arrow) = places.get(look.arrow) else {
            commands.entity(entity).despawn();
            continue;
        };
        let origin = arrow.translation();
        transform.translation = origin;
        if look.shaft.is_none() {
            let found: Vec<Entity> = SHAFT
                .iter()
                .map_while(|&bone| {
                    crate::making::bone_entity(look.arrow, &[bone], &children, (&bones, &worn))
                })
                .collect();
            if !found.is_empty() {
                look.shaft = Some(found);
            }
        }
        let Some(points) = look.shaft.as_ref().and_then(|shaft| {
            shaft
                .iter()
                .map(|&bone| {
                    let bone = places.get(bone).ok()?;
                    Some(bolt_looks::models(bone.translation() - origin))
                })
                .collect::<Option<Vec<[f32; 3]>>>()
        }) else {
            continue;
        };
        // The sparks between its first two bones, at its one bone for a model of one.
        let ends = [points[0], points.get(1).copied().unwrap_or(points[0])];
        let from = look.last.unwrap_or(ends);
        let Shown {
            elemental,
            sparks,
            meshes: [spark_mesh, line_mesh],
            ..
        } = &mut *look;
        let mut quads = Quads::default();
        if let Some(sparks) = sparks {
            sparks.update(dt, (from, ends), &mut dice.0);
            for particle in &sparks.system.particles {
                quads.push(
                    bolt_looks::world(particle.place),
                    particle.size * s,
                    0.0,
                    (Vec2::ZERO, 1.0),
                    sparks.colour(particle.life),
                );
            }
        }
        if let Some(mut mesh) = meshes.get_mut(&*spark_mesh) {
            quads.into_mesh(&mut mesh);
        }
        let line = arrows::element_line(elemental.element, elemental.share);
        let mut quads = Quads::default();
        for &(a, b) in particles::glow_segments(points.len()) {
            for at in line.points(points[a], points[b]) {
                quads.push(
                    bolt_looks::world(at),
                    line.half * s,
                    0.0,
                    (Vec2::ZERO, 1.0),
                    line.colour(),
                );
            }
        }
        if let Some(mut mesh) = meshes.get_mut(&*line_mesh) {
            quads.into_mesh(&mut mesh);
        }
        look.last = Some(ends);
    }
}

/// The arrows' elements.
pub struct ArrowElementsPlugin;

impl Plugin for ArrowElementsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>()
            .init_resource::<Material>()
            .add_systems(
                Update,
                (wield, show, run)
                    .chain()
                    .in_set(Playing)
                    .in_set(Unpaused)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}
