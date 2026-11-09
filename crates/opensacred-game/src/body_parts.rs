//! The body parts of a creature's burst (800, `cParticleSystem_bodyparts`, #1935;
//! docs/re/effects.md, "The body parts"): under `[gameplay] violence`, the original's `VIOLENCE`,
//! a creature that bursts (`undead`'s skeleton warriors, zombies and ghouls, and the creatures
//! whose type bursts) throws its gib set's parts about its place (`0x4E8DA0`): five of the parts
//! 0 to 4 and six of the part 5 for the creatures whose type bursts, six of the part 5 for the
//! undead, and lays a blood decal of its set's at its place (none for the bones).
//! Each part hops away from it along the ground (`opensacred_rules::particles::Hop`), turned back
//! by a cell that may not be walked on, bouncing lower and lower and laying a smaller decal at
//! each bounce, its model at twice its size turning about two axes and trailing puffs of
//! `PARTICLE_CLOUD01` in the burst's colour, laid over by their alpha and tinted by the night;
//! landed, it lies until 30 to 60 s from its start, then goes.

use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;
use opensacred_rules::particles::{self, BodyPart, PART_SCALE, PARTICLE_CLOUD};
use opensacred_rules::undead::{BURST_BLOOD, PART_BLOOD, blood_texture, part_type};

use crate::bolt_looks::{models, world};
use crate::frame::{Playing, Unpaused};
use crate::fx::{Effect, Orders, Quads, empty, particle_material, particle_texture};
use crate::game::{AppState, Data};
use crate::npcs::Retexture;
use crate::settings::Settings;
use crate::spawning::Rng;
use crate::terrain::{Heights, Level};
use crate::{iso, models as model_looks, paths};

/// A creature that bursts (`0x4E8DA0`): its place, its level, its gib set, and its parts' count:
/// above 1 the parts 0 to 4 fly, and above 0 six of the part 5.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub(crate) struct Burst {
    pub at: Vec3,
    pub level: Level,
    pub set: u8,
    pub parts: u8,
}

/// The parts' dice (`rand()`; **estimate**: any seed).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x800))
    }
}

/// The trails' material, laid over by its alpha; made at the first burst, none where the texture
/// is missing.
#[derive(Resource, Default)]
struct Looks {
    made: bool,
    trail: Option<Handle<Effect>>,
}

/// A part on its way or lying: its system, its gib set, its trail's colour, its level, its place
/// in the world, its model and its trail's mesh.
#[derive(Component)]
struct Part {
    part: BodyPart,
    set: u8,
    colour: [f32; 3],
    level: Level,
    place: Vec3,
    model: Option<Entity>,
    mesh: Handle<Mesh>,
}

/// The parts of a burst of `parts` (`0x4E8DA0`): above 1 the parts 0 to 4, then above 0 six of
/// the part 5.
fn kinds(parts: u8) -> Vec<u8> {
    let mut kinds = Vec::new();
    if parts > 1 {
        kinds.extend(0..5);
    }
    if parts > 0 {
        kinds.extend([5; 6]);
    }
    kinds
}

/// Throws the parts of each burst and lays its decal, with the violence on (`0x4E8DA0`): each
/// part made at the creature's place and thrown by the FX event 9 (`0x5A3840`), its model the
/// item of its type that the set rolls (`0x427650`); then where the set has a blood texture
/// (`0x4271E0`), the decal at the place.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources as arguments, grouped"
)]
fn throw(
    mut commands: Commands,
    mut bursts: MessageReader<Burst>,
    settings: Option<Res<Settings>>,
    heights: Res<Heights>,
    (data, assets, tables): (
        Option<Res<Data>>,
        Res<AssetServer>,
        Option<Res<crate::drops::Tables>>,
    ),
    (mut dice, mut looks, mut materials, mut meshes): (
        ResMut<Dice>,
        ResMut<Looks>,
        ResMut<Assets<Effect>>,
        ResMut<Assets<Mesh>>,
    ),
) {
    if !settings.is_some_and(|settings| settings.gameplay.violence) {
        bursts.clear();
        return;
    }
    for &Burst {
        at,
        level,
        set,
        parts,
    } in bursts.read()
    {
        if !looks.made {
            looks.made = true;
            let image = data
                .as_deref()
                .and_then(|data| particle_texture(&data.0.assets, &assets, PARTICLE_CLOUD));
            if image.is_none() {
                warn!("body parts: no texture {PARTICLE_CLOUD}; their trails are not drawn");
            }
            looks.trail =
                image.map(|image| materials.add(particle_material(image, AlphaMode::Blend)));
        }
        let dice = &mut dice.0;
        let ground = heights.standing(at.x, at.z, level).unwrap_or(at.y);
        let place = at.with_y(ground);
        let [r, g, b, _] = particles::argb(particles::burst_colour(set));
        let mut thrown = Vec::new();
        for kind in kinds(parts) {
            let Some(item) = part_type(set, kind, dice) else {
                continue;
            };
            let model = tables.as_deref().and_then(|tables| tables.look(item)).map(
                |(model, texture, _)| {
                    let mut model = commands.spawn((
                        model_looks::scene(&assets, model),
                        Transform::from_translation(place).with_scale(Vec3::ZERO),
                        model_looks::Ordered::default(),
                        DespawnOnExit(AppState::InGame),
                    ));
                    if let Some(texture) = texture {
                        model.insert(Retexture::load(&assets, texture));
                    }
                    model.id()
                },
            );
            let mesh = meshes.add(empty());
            let mut part = commands.spawn((
                Transform::IDENTITY,
                Visibility::default(),
                DespawnOnExit(AppState::InGame),
            ));
            if let Some(material) = looks.trail.clone() {
                part.with_child((
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::IDENTITY,
                    NoFrustumCulling,
                ));
            }
            part.insert(Part {
                part: BodyPart::new(0.0, dice),
                set,
                colour: [r, g, b],
                level,
                place,
                model,
                mesh,
            });
            thrown.push(item);
        }
        debug!("body parts: a burst of the set {set} at {place} throws {thrown:?}");
        if let Some(texture) = blood_texture(set, dice) {
            crate::marks::lay(
                &mut commands,
                place,
                (texture, BURST_BLOOD.0, BURST_BLOOD.1),
            );
        }
    }
}

/// Runs each part a frame on (`0x5A6680`'s flag `0x800`, `0x783900`): it hops along the ground,
/// turned back where the cell ahead may not be walked on, flown over aside (`0x636D20`), at its
/// height over the ground there; at each bounce where its set has a blood texture, a decal at it
/// (`0x5AA560`); its model at its place turned by its two turns at twice its size (`0x7839C0`),
/// its trail's puffs at their places, the whole at its place in the order; past its end it goes.
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    mut parts: Query<(Entity, &mut Part)>,
    mut shown: Query<&mut Transform, Without<Part>>,
    (heights, orders): (Res<Heights>, Orders),
    (mut meshes, mut dice): (ResMut<Assets<Mesh>>, ResMut<Dice>),
) {
    let dt = time.delta_secs().min(0.1);
    let s = iso::MODEL_SCALE;
    for (entity, mut part) in &mut parts {
        let Part {
            part,
            set,
            colour,
            level,
            place,
            model,
            mesh,
        } = &mut *part;
        let before = *place;
        let step = part.update(dt);
        if !step.lies {
            let here = before.xz();
            let mut at = here + Vec2::from(step.moved);
            let cell = paths::cell_of(at.x, at.y);
            if !heights.walkable_at(cell, *level) && !heights.flown_over(cell, *level) {
                part.hop.turn_about();
                at = here;
            }
            let ground = heights.standing(at.x, at.y, *level).unwrap_or(before.y);
            *place = Vec3::new(at.x, ground + step.lift, at.y);
            if step.bounced
                && let Some(texture) = blood_texture(*set, &mut dice.0)
            {
                crate::marks::lay(
                    &mut commands,
                    place.with_y(ground),
                    (texture, PART_BLOOD.0, PART_BLOOD.1),
                );
            }
        }
        part.trail_on(dt, (models(before), models(*place)), &mut dice.0);
        if part.done() {
            if let Some(model) = model.take() {
                commands.entity(model).try_despawn();
            }
            commands.entity(entity).despawn();
            continue;
        }
        if let Some(model) = *model
            && let Ok(mut transform) = shown.get_mut(model)
        {
            let [a, b] = part.turns();
            *transform = Transform::from_translation(*place)
                .with_rotation(Quat::from_rotation_z(b) * Quat::from_rotation_x(a))
                .with_scale(Vec3::splat(PART_SCALE * s));
        }
        let [r, g, b] = *colour;
        let mut quads = Quads::default();
        for puff in &part.trail.particles {
            quads.push(
                world(puff.place),
                puff.size * s,
                puff.angle,
                (Vec2::ZERO, 1.0),
                [r, g, b, puff.life / 255.0],
            );
        }
        if let Some(mut mesh) = meshes.get_mut(&*mesh) {
            quads.in_order(orders.at(*place), Vec3::ZERO);
            quads.into_mesh(&mut mesh);
        }
    }
}

/// The body parts of the creatures' bursts.
pub struct BodyPartsPlugin;

impl Plugin for BodyPartsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Burst>()
            .init_resource::<Dice>()
            .init_resource::<Looks>()
            .add_systems(
                Update,
                (throw, fly)
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
    fn a_burst_throws_its_parts_by_its_count() {
        assert!(kinds(0).is_empty());
        assert_eq!(kinds(1), vec![5; 6]);
        assert_eq!(kinds(2), vec![0, 1, 2, 3, 4, 5, 5, 5, 5, 5, 5]);
    }
}
