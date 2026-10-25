//! The Dark Elf's soulcatcher 1044 (#757; docs/re/arts.md, "The Dark Elf's"): while its effect 35
//! holds on the hero, each creature that dies within 550 of him sends him a soul (the objects 869
//! and 780, `0x5FF7C0`), which flies toward him at `⌊10·t⌋` units a second, `t` its age, and as it
//! reaches him, nearer than his size and its own, heals him `10 + 30·L` up to his most health
//! with the effect 30 for 1 s (`opensacred_rules::arts::soul_speed`, `soul_heals`); a soul ends at
//! a cell that may not be walked on or without him. A glowing ball stands in for its particles,
//! as the Seraphim's bolts' (docs/decisions/2026-10-24-bolts-look.md).

use bevy::prelude::*;
use opensacred_rules::arts::{SOUL_REACH, SOUL_SIZE, SOULCATCHER, soul_heals, soul_speed};
use opensacred_rules::fight::Effect;

use crate::arts::HeroArts;
use crate::effects::Effects;
use crate::fight::{Experience, Health};
use crate::frame::{Playing, Step};
use crate::game::AppState;
use crate::hero::Body;
use crate::npcs::Dies;
use crate::terrain::{Heights, Level};
use crate::{arrows, paths};

/// How high over the ground a soul flies, in world units (**hypothesis**: about the Dark Elf's
/// chest, his bone 1).
const HEIGHT: f32 = 90.0;

/// A soul on its way to the Dark Elf: his entity, its level, its age in seconds and the health it
/// gives him.
#[derive(Component)]
struct Soul {
    to: Entity,
    level: Level,
    age: f32,
    heals: u32,
}

/// The souls' look.
#[derive(Resource)]
struct Look(Handle<Mesh>, Handle<StandardMaterial>);

/// Makes the souls' look: a small pale ball (**estimate**).
fn make_look(
    mut commands: Commands,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };
    commands.insert_resource(Look(
        meshes.add(Sphere::new(12.0)),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.8, 0.95, 0.9),
            unlit: true,
            ..default()
        }),
    ));
}

/// Sends a soul from each creature that dies within [`SOUL_REACH`] of the hero while his
/// effect 35 holds, of the health that his 1044 at its level in play gives.
fn catch(
    mut commands: Commands,
    look: Option<Res<Look>>,
    arts: Option<Res<HeroArts>>,
    experience: Res<Experience>,
    heroes: Query<(Entity, &Transform, &Body, &Effects)>,
    dying: Query<&Transform, (Added<Dies>, Without<Body>)>,
) {
    let (Some(look), Some(arts)) = (look, arts) else {
        return;
    };
    let Some((hero, at, body, _)) = heroes
        .iter()
        .find(|(.., effects)| effects.holds(SOULCATCHER))
    else {
        return;
    };
    let level = arts.arts.level_in_play(
        1044,
        opensacred_rules::arts::Creature {
            kind: arts.kind,
            level: experience.level,
            player: true,
        },
    );
    let Some(spell) = arts.tables.spells.iter().find(|spell| spell.id == 47) else {
        return;
    };
    let heals = soul_heals(spell, level.unwrap_or(1));
    for place in &dying {
        if (place.translation - at.translation).xz().length() >= SOUL_REACH {
            continue;
        }
        debug!(
            "souls: a soul for {hero} of {heals} from {:?}",
            place.translation
        );
        commands.spawn((
            Mesh3d(look.0.clone()),
            MeshMaterial3d(look.1.clone()),
            Transform::from_translation(place.translation + Vec3::Y * HEIGHT),
            Soul {
                to: hero,
                level: body.level(),
                age: 0.0,
                heals,
            },
            DespawnOnExit(AppState::InGame),
        ));
    }
}

/// Flies each soul toward the hero and heals him as it reaches him (`0x5A7BB2`, `0x5ABD80`'s case
/// `0x30C`).
fn home(
    mut commands: Commands,
    time: Res<Time>,
    heights: Res<Heights>,
    mut souls: Query<(Entity, &mut Soul, &mut Transform), Without<Body>>,
    mut heroes: Query<(&Transform, &mut Health, Option<&mut Effects>), With<Body>>,
) {
    let dt = time.delta_secs();
    for (entity, mut soul, mut transform) in &mut souls {
        let Ok((at, mut health, effects)) = heroes.get_mut(soul.to) else {
            commands.entity(entity).despawn();
            continue;
        };
        soul.age += dt;
        let here = transform.translation.xz();
        let offset = at.translation.xz() - here;
        // He reaches it within his size and its own (the heroes' 50).
        if offset.length() < arrows::size(0) + SOUL_SIZE {
            health.current = (health.current + soul.heals).min(health.most);
            if let Some(mut effects) = effects {
                effects.add(
                    Effect {
                        id: 30,
                        whole: 0,
                        value: 0.0,
                        art: 100,
                        level: 0,
                    },
                    1.0,
                );
            }
            debug!(
                "souls: {entity} heals {} by {}, health {}/{}",
                soul.to, soul.heals, health.current, health.most
            );
            commands.entity(entity).despawn();
            continue;
        }
        let step = (soul_speed(soul.age) * dt).min(offset.length());
        let ground = here + offset.normalize_or_zero() * step;
        if !heights.walkable_on(paths::cell_of(ground.x, ground.y), soul.level) {
            commands.entity(entity).despawn();
            continue;
        }
        let height = heights
            .standing(ground.x, ground.y, soul.level)
            .unwrap_or(transform.translation.y - HEIGHT);
        transform.translation = Vec3::new(ground.x, height + HEIGHT, ground.y);
    }
}

/// The Dark Elf's souls.
pub struct SoulsPlugin;

impl Plugin for SoulsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, make_look).add_systems(
            Update,
            (catch, home).chain().after(Step::Npcs).in_set(Playing),
        );
    }
}
