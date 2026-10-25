//! What the hero's spells do as they act (#872, #873; docs/re/arts.md, "The spells' cast", "The
//! objects' actions", "The Mage's spells"): `making` casts a spell and, once the share of its
//! clip still to play falls below the spell's, sends [`Acts`]. The fireball leaves for its
//! target's place as a bolt (`bolts`); the regeneration gives its bearer its record's effects
//! (`opensacred_rules::spells::effects`) at once, and the stone skin as its shield's clock passes
//! [`SHIELD_TIME`] at the cast's rate; the healing adds its health to its bearer's, its most at
//! most. A spell that lasts on its bearer gives it the effect `0x22` too.

use bevy::prelude::*;
use opensacred_rules::arts::bolt_damages;
use opensacred_rules::fight::Effect;
use opensacred_rules::spells::{self, FIREBALL, HEALING, REIKI, SHIELD_TIME, STONE_SKIN};

use crate::effects::Effects;
use crate::fight::{Combat, Health};
use crate::frame::{Playing, Step};
use crate::hero::Body;

/// A spell acting: its caster, the spell, its target (the caster for one on itself), the point
/// it was aimed at, and its level in play.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct Acts {
    pub caster: Entity,
    pub spell: u16,
    pub target: Entity,
    pub point: Vec3,
    pub level: u32,
}

/// The stone skin's effects waiting for its shield's clock: what is left of it in the world's
/// seconds.
#[derive(Component, Debug, Clone)]
struct Shielding {
    left: f32,
    effects: Vec<(Effect, f32)>,
}

/// Carries out each spell that acts.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn act(
    mut commands: Commands,
    mut acts: MessageReader<Acts>,
    arts: Option<Res<crate::arts::HeroArts>>,
    rules: Res<crate::fight::Rules>,
    casters: Query<(&Transform, Option<&Combat>), With<Body>>,
    places: Query<&GlobalTransform>,
    mut healths: Query<&mut Health>,
    mut launches: MessageWriter<crate::bolts::Launch>,
) {
    let Some(arts) = arts else {
        acts.clear();
        return;
    };
    for act in acts.read() {
        let Some(record) = arts.tables.spells.iter().find(|s| s.id == act.spell) else {
            warn!("spells: no record {} of the first table", act.spell);
            continue;
        };
        let Ok((at, combat)) = casters.get(act.caster) else {
            continue;
        };
        let here = at.translation;
        let mut given = Vec::new();
        if spells::lasts(act.spell) {
            given.push(spells::lasting(record, act.level));
        }
        match act.spell {
            FIREBALL => {
                // For its target's place, or the point where it is gone (`0x522480`).
                let toward = places
                    .get(act.target)
                    .ok()
                    .filter(|_| act.target != act.caster)
                    .map_or(act.point, GlobalTransform::translation);
                let direction = (toward - here)
                    .xz()
                    .try_normalize()
                    .unwrap_or_else(|| (at.rotation * Vec3::X).xz().normalize_or(Vec2::X));
                let multipliers = combat.map_or([1.0; 4], |combat| combat.spells);
                let level = i32::try_from(act.level).unwrap_or(1);
                let damages = bolt_damages(record, level, multipliers);
                info!("spells: the fireball leaves for {toward} with {damages:?}");
                launches.write(crate::bolts::Launch {
                    shooter: act.caster,
                    direction,
                    course: crate::bolts::Course::fireball(act.level),
                    damages,
                    wait: 0.0,
                });
            }
            REIKI => given.extend(spells::effects(record, act.level)),
            STONE_SKIN => {
                // The shield's clock runs at the cast's rate (`+0x3E`).
                let rate = combat.map_or(opensacred_rules::fight::CAST_SPEED, |c| c.cast) / 100.0;
                commands.entity(act.target).insert(Shielding {
                    left: SHIELD_TIME / rate.max(f32::EPSILON),
                    effects: spells::effects(record, act.level),
                });
            }
            HEALING => {
                let lore = opensacred_rules::fight::magic_lore(&arts.skills, &rules);
                let health = spells::healing(record, act.level, lore);
                if let Ok(mut bearer) = healths.get_mut(act.target) {
                    bearer.current = bearer.current.saturating_add(health).min(bearer.most);
                    info!("spells: the healing gives {health}, to {}", bearer.current);
                }
            }
            _ => {}
        }
        give(&mut commands, act.target, given);
    }
}

/// Gives `bearer` the effects `given`, each for its seconds.
fn give(commands: &mut Commands, bearer: Entity, given: Vec<(Effect, f32)>) {
    if given.is_empty() {
        return;
    }
    info!("spells: {bearer} takes {given:?}");
    commands
        .entity(bearer)
        .entry::<Effects>()
        .or_default()
        .and_modify(move |mut effects| {
            for (effect, seconds) in given {
                effects.add(effect, seconds);
            }
        });
}

/// Runs each stone skin's shield and gives its effects as its clock passes (its system's update
/// and the action 6).
fn shield(time: Res<Time>, mut commands: Commands, mut shields: Query<(Entity, &mut Shielding)>) {
    for (bearer, mut shielding) in &mut shields {
        shielding.left -= time.delta_secs();
        if shielding.left > 0.0 {
            continue;
        }
        let given = std::mem::take(&mut shielding.effects);
        commands.entity(bearer).remove::<Shielding>();
        give(&mut commands, bearer, given);
    }
}

/// The spells' acts.
pub struct SpellsPlugin;

impl Plugin for SpellsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Acts>().add_systems(
            Update,
            (act, shield).chain().after(Step::Npcs).in_set(Playing),
        );
    }
}
