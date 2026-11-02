//! The creatures' casts (docs/re/ai.md, "The creatures' spells"; #1852): a caster's decision in
//! its melee (`ai`, `opensacred_rules::minds::Mind::cast`) orders a cast, whose clip of its spell
//! and type (`opensacred_rules::casts::clip`) plays at the creature's speed, and whose spell acts
//! at its share of the clip (`npcs::ClipActs`), at the level a creature makes a spell it lacks
//! (`fight::Rules::creature_spell_level`), its damage times `MagieAll`'s factor
//! (`fight::Rules::creature_spell_damage`), and whose clip's end gives the creature its melee
//! again from its start (`opensacred_rules::minds::Mind::cast_ends`). The game follows the magic
//! skeletons' fireball first: a bolt at the hero as the hero's fireball flies (`bolts`).

use bevy::prelude::*;
use opensacred_rules::arts::bolt_damages;
use opensacred_rules::{casts, spells};

use crate::ai::Mind;
use crate::frame::Playing;
use crate::npcs::{ClipActs, Dies, ScriptClip};

/// The spells of the casts that the game follows.
const FOLLOWED: [u16; 1] = [spells::FIREBALL];

/// Whether the game follows a creature's cast of the spell `spell`.
pub fn follows(spell: u16) -> bool {
    FOLLOWED.contains(&spell)
}

/// Whether a creature of type `creature` is a caster whose spell the game follows.
pub fn casts(creature: u32) -> bool {
    casts::mark_1(creature).is_some_and(|(spell, _)| follows(spell))
}

/// A creature's cast in course, until its clip ends: its spell and its variant, its target and
/// the target's place as the cast was ordered, and whether its clip has begun.
#[derive(Component, Debug)]
pub struct Casting {
    spell: u16,
    variant: u8,
    target: Entity,
    point: Vec3,
    begun: bool,
}

impl Casting {
    pub fn new(spell: u16, variant: u8, target: Entity, point: Vec3) -> Self {
        Self {
            spell,
            variant,
            target,
            point,
            begun: false,
        }
    }
}

/// Begins each cast's clip: its slot by spell and type, its spell acting at the spell's share
/// (`0x521D60`).
fn begin(mut commands: Commands, mut casting: Query<(Entity, &mut Casting, &Mind), Without<Dies>>) {
    for (entity, mut cast, mind) in &mut casting {
        if cast.begun {
            continue;
        }
        cast.begun = true;
        let slot = casts::clip(cast.spell, mind.creature, false);
        let share = spells::share(cast.spell).unwrap_or(1.0);
        debug!(
            "creature_casts: {entity} casts {} (variant {}) by its clip {slot}",
            cast.spell, cast.variant
        );
        commands
            .entity(entity)
            .insert(ScriptClip::casting(slot, share));
    }
}

/// Makes each cast's spell act once its clip passes its share: the fireball a bolt toward its
/// target's place, at the creature's spell's level, its damage `MagieAll`'s share of the hero's.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn act(
    mut commands: Commands,
    acting: Query<
        (
            Entity,
            &Casting,
            &Transform,
            &Mind,
            Option<&crate::fight::Ranked>,
            Option<&crate::fight::Combat>,
        ),
        With<ClipActs>,
    >,
    places: Query<&GlobalTransform>,
    (arts, rules): (Option<Res<crate::arts::HeroArts>>, Res<crate::fight::Rules>),
    mut launches: MessageWriter<crate::bolts::Launch>,
) {
    for (entity, cast, at, mind, rank, combat) in &acting {
        commands.entity(entity).remove::<ClipActs>();
        let Some(record) = arts
            .as_ref()
            .and_then(|arts| arts.tables.spells.iter().find(|s| s.id == cast.spell))
        else {
            continue;
        };
        let rank = rank.map_or(crate::fight::Rank::Common, |rank| rank.0);
        let level = rules.0.creature_spell_level(mind.level, rank);
        let factor = rules.0.creature_spell_damage(level);
        let here = at.translation;
        if cast.spell == spells::FIREBALL {
            let toward = places
                .get(cast.target)
                .map_or(cast.point, GlobalTransform::translation);
            let direction = (toward - here)
                .xz()
                .try_normalize()
                .unwrap_or_else(|| (at.rotation * Vec3::X).xz().normalize_or(Vec2::X));
            let multipliers = combat.map_or([1.0; 4], |combat| combat.spells);
            let damages = bolt_damages(record, i32::try_from(level).unwrap_or(1), multipliers)
                .map(|damage| scaled(damage, factor));
            info!("creature_casts: {entity} casts its fireball at {toward} with {damages:?}");
            launches.write(crate::bolts::Launch {
                shooter: entity,
                direction,
                course: crate::bolts::Course::fireball(level),
                damages,
                wait: 0.0,
            });
        }
    }
}

/// Ends each cast whose clip has ended (`0x51D150`'s sub-state 4), the creature taking its melee
/// up again from its start.
fn end(
    mut commands: Commands,
    mut ended: Query<(Entity, &Casting, &mut Mind), Without<ScriptClip>>,
) {
    for (entity, cast, mut mind) in &mut ended {
        if cast.begun {
            commands.entity(entity).remove::<Casting>();
            mind.cast_ends();
        }
    }
}

/// A damage times a factor, a float as the strikes' damages are.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a spell's damage, far within the floats' range"
)]
fn scaled(damage: f32, factor: f64) -> f32 {
    (f64::from(damage) * factor) as f32
}

/// The creatures' casts.
pub struct CreatureCastsPlugin;

impl Plugin for CreatureCastsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (begin, act, end).chain().in_set(Playing));
    }
}
