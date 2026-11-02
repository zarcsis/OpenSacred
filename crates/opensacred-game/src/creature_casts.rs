//! The creatures' casts (docs/re/ai.md, "The creatures' spells"; #1852): a caster's decision in
//! its melee (`ai`, `opensacred_rules::minds::Mind::cast`) orders a cast, whose clip of its spell
//! and type (`opensacred_rules::casts::clip`) plays at the creature's speed, and whose spell acts
//! at its share of the clip (`npcs::ClipActs`), at the level a creature makes a spell it lacks
//! (`fight::Rules::creature_spell_level`), its damage times `MagieAll`'s factor
//! (`fight::Rules::creature_spell_damage`), and whose clip's end gives the creature its melee
//! again from its start (`opensacred_rules::minds::Mind::cast_ends`). The game follows the mark
//! 1's spells as the hero's make them, striking the hero: the magic skeletons' fireball and the
//! mummies' whirlwind (`bolts`), the mummy 03's lightning and the orc shaman's meteors (`spells`),
//! and their flesh to stone, which holds the hero (`hero_held`).

use bevy::prelude::*;
use opensacred_rules::arts::bolt_damages;
use opensacred_rules::{casts, spells};

use crate::ai::Mind;
use crate::crowd::InTheWay;
use crate::frame::Playing;
use crate::npcs::{ClipActs, Dies, ScriptClip};

/// The spells of the casts that the game follows.
const FOLLOWED: [u16; 4] = [
    spells::FIREBALL,
    spells::WHIRLWIND,
    spells::LIGHTNING,
    spells::METEORS,
];

/// Whether a creature of type `creature` is a caster whose spell the game follows.
pub fn casts(creature: u32) -> bool {
    casts::mark_1(creature).is_some_and(|(spell, _)| FOLLOWED.contains(&spell))
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

/// Makes each cast's spell act once its clip passes its share, at the creature's spell's level,
/// its damage `MagieAll`'s share of the hero's, as the hero's spell acts (`spells`): the
/// fireball a bolt toward its target's place, the whirlwind a fan of bolts along the line to it,
/// the lightning a strike on its target, the meteors a hail about its target's place as the cast
/// was ordered, the stone the effect 15 on the hero for its value 0 in seconds (`0x52494F`),
/// refused on a hero that an effect of the stone holds already (`0x582B00`).
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
            (&Mind, &InTheWay),
            Option<&crate::fight::Ranked>,
            Option<&crate::fight::Combat>,
        ),
        With<ClipActs>,
    >,
    places: Query<&GlobalTransform>,
    held: Query<Option<&crate::effects::Effects>, With<crate::hero::Body>>,
    (arts, rules, looks, mut dice): (
        Option<Res<crate::arts::HeroArts>>,
        Res<crate::fight::Rules>,
        Option<Res<crate::spells::Looks>>,
        ResMut<crate::spells::Dice>,
    ),
    (mut launches, mut hits): (
        MessageWriter<crate::bolts::Launch>,
        MessageWriter<crate::fight::Hit>,
    ),
) {
    for (entity, cast, at, (mind, way), rank, combat) in &acting {
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
        let multipliers = combat.map_or([1.0; 4], |combat| combat.spells);
        let damages = bolt_damages(record, i32::try_from(level).unwrap_or(1), multipliers)
            .map(|damage| scaled(damage, factor));
        let here = at.translation;
        let target = places.get(cast.target).map(GlobalTransform::translation);
        let toward = target.unwrap_or(cast.point);
        let line = (toward - here).xz();
        let direction = line
            .try_normalize()
            .unwrap_or_else(|| (at.rotation * Vec3::X).xz().normalize_or(Vec2::X));
        info!(
            "creature_casts: {entity} casts {} at {toward} with {damages:?}",
            cast.spell
        );
        match cast.spell {
            spells::FIREBALL => {
                launches.write(crate::bolts::Launch {
                    shooter: entity,
                    direction,
                    course: crate::bolts::Course::fireball(level),
                    damages,
                    wait: 0.0,
                });
            }
            spells::WHIRLWIND => {
                // The hero's fan (`0x524EA2`), without its knockback, which strikes creatures.
                let course = crate::bolts::Course::passing(spells::WHIRLWIND_COURSE, 4);
                for angle in spells::fan(cast.spell, spells::count(record, level), line.length()) {
                    launches.write(crate::bolts::Launch {
                        shooter: entity,
                        direction: Vec2::from_angle(angle).rotate(direction),
                        course,
                        damages,
                        wait: 0.0,
                    });
                }
            }
            spells::LIGHTNING => {
                let Ok(place) = target else {
                    continue;
                };
                hits.write(crate::fight::Hit {
                    attacker: entity,
                    target: cast.target,
                    share: 1.0,
                    ranged: false,
                    art: None,
                    fixed: Some(damages),
                    sure: true,
                    poison: false,
                });
                if let Some(looks) = &looks {
                    crate::spells::lightning(&mut commands, looks, place);
                }
            }
            spells::STONE => {
                let Ok(effects) = held.get(cast.target) else {
                    continue;
                };
                if effects.is_some_and(|effects| effects.holds_any(&[spells::STONE])) {
                    continue;
                }
                let seconds =
                    f32::from(u16::try_from(spells::value(record, 0, level)).unwrap_or(u16::MAX));
                let effect = opensacred_rules::fight::Effect {
                    id: spells::HELD,
                    whole: 0,
                    value: 0.0,
                    art: u32::from(spells::STONE),
                    level: 0,
                };
                commands
                    .entity(cast.target)
                    .entry::<crate::effects::Effects>()
                    .or_default()
                    .and_modify(move |mut effects| effects.add(effect, seconds));
            }
            spells::METEORS => {
                crate::spells::meteors(
                    &mut commands,
                    &mut dice,
                    (record, level),
                    (entity, way.level),
                    cast.point,
                    damages,
                );
            }
            _ => {}
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
