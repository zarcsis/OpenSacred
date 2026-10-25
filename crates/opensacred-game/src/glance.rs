//! The Gladiator's 1016's effect (#755; docs/re/arts.md, "The Gladiator's"): while the effect 18
//! holds on the hero, each of its entries counts milliseconds from 1,000, and each time its count
//! passes 1,000 with a hostile creature about, the hero strikes one hostile creature alive within
//! `(l + 20)·10` of it (`opensacred_rules::arts::glance_due`, `glance_reach`): the first so near,
//! each later one taking its place by a roll of one in three, with the effect's whole value as
//! the damage of one of the four kinds by a roll (the fight's hit of fixed damages, the event 5
//! of `0x562730` at `0x563310`), which carries no attacker and always lands (seen). The hero
//! alone: the original runs it for the local player's creature.

use bevy::prelude::*;
use opensacred_rules::arts::{GLANCE, GLANCE_EVERY, glance_due, glance_reach};

use crate::ai::Mind;
use crate::effects::Effects;
use crate::fight::{self, Experience, Health};
use crate::frame::{Playing, Step};
use crate::hero::Body;
use crate::npcs::Dies;
use crate::spawning::Rng;

/// The counts of the hero's effects 18, each in the entry's first word (`+8`), in their order.
#[derive(Component, Default)]
struct Counts(Vec<u32>);

/// The dice of the strikes' targets and kinds (`0x562730`'s `rand`).
#[derive(Resource)]
struct Dice(Rng);

/// Counts each effect 18 of the hero on, and strikes as each count comes due.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its queries with their filters"
)]
fn strike(
    mut commands: Commands,
    time: Res<Time>,
    experience: Res<Experience>,
    mut dice: ResMut<Dice>,
    mut heroes: Query<
        (Entity, &Transform, &Effects, Option<&mut Counts>),
        (With<Body>, Without<Dies>),
    >,
    creatures: Query<(Entity, &Transform, &Mind, &Health), (Without<Body>, Without<Dies>)>,
    mut hits: MessageWriter<fight::Hit>,
) {
    let seconds = time.delta_secs();
    for (hero, at, effects, counts) in &mut heroes {
        let wholes: Vec<i32> = effects
            .timed()
            .iter()
            .filter(|timed| timed.effect.id == GLANCE)
            .map(|timed| timed.effect.whole)
            .collect();
        let Some(mut counts) = counts else {
            if !wholes.is_empty() {
                commands.entity(hero).insert(Counts::default());
            }
            continue;
        };
        // A new effect's count starts at 1,000 (`0x582D50`); the first to go is the oldest.
        let counts = &mut counts.0;
        while counts.len() > wholes.len() {
            counts.remove(0);
        }
        counts.resize(wholes.len(), GLANCE_EVERY);
        if wholes.is_empty() {
            commands.entity(hero).remove::<Counts>();
            continue;
        }
        let here = at.translation.xz();
        let reach = glance_reach(experience.level);
        let hostile: Vec<(Entity, f32)> = creatures
            .iter()
            .filter(|(_, _, mind, health)| mind.hostile() && health.current > 0)
            .map(|(e, at, ..)| (e, (at.translation.xz() - here).length()))
            .collect();
        for (count, &whole) in counts.iter_mut().zip(&wholes) {
            if !glance_due(count, seconds, !hostile.is_empty()) {
                continue;
            }
            // The first within the reach, each later one in its place by one in three.
            let mut target = None;
            for &(creature, distance) in &hostile {
                if distance < reach && (target.is_none() || dice.0.below(3) == 0) {
                    target = Some(creature);
                }
            }
            let Some(target) = target else {
                continue;
            };
            let mut damages = [0.0; 4];
            let kind = usize::try_from(dice.0.below(4)).unwrap_or(0);
            let whole = u16::try_from(whole.clamp(0, 65_534)).unwrap_or(0);
            damages[kind] = f32::from(whole);
            debug!("glance: {hero} strikes {target} for {damages:?}");
            hits.write(fight::Hit {
                attacker: hero,
                target,
                share: 1.0,
                ranged: false,
                art: None,
                fixed: Some(damages),
                sure: true,
            });
        }
    }
}

/// The Gladiator's 1016's strikes.
pub struct GlancePlugin;

impl Plugin for GlancePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Dice(Rng::of(0, 0, 4)))
            .add_systems(Update, strike.after(Step::Npcs).in_set(Playing));
    }
}
