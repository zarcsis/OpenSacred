//! The scripts' `Attack`s without an art (92, `0x4A2B40`; #1830; docs/re/netscript.md,
//! "`Attack`"): outside the anim mode each attacker, a creature or every member of a group, is
//! sent at its target, the hero, a creature, or one member of a group drawn for each attacker.
//! Where the pair is hostile neither way, the attacker hostile to the heroes, else the target
//! hostile to them (or always where a group attacks a group) turns against every side
//! (`opensacred_rules::minds::attack_sides`), for good; then the attacker fights the target where
//! it is hostile to it (the event `0x100`), and a pair that stays friendly fights not. The target
//! is sent nothing: it fights back by its own mind.

use bevy::prelude::*;
use opensacred_rules::minds::{self, AGAINST_EVERY_SIDE, Sided};
use opensacred_rules::spawning::Rng;

use opensacred_rules::spells;

use crate::ai::Mind;
use crate::frame::Playing;
use crate::hero::Body;
use crate::npcs::{Dies, Party, SCRIPT_GROUP, ScriptAttacks, ScriptName, Walk};
use crate::sfx::{Of, Play, Roll};
use crate::terrain::{Heights, Level};
use crate::{buildings, crowd, iso};

/// The sound of the teleport's cast at its caster (`0x7E7`).
const PHASE_SOUND: u32 = 2023;

/// A creature's teleport that a script's `Attack` of `ECS_TELEPORT` orders (`0x4A2B40`,
/// `PHASEN`; #1831): where to, on which level, whether it goes there with no test of the way
/// (a cell of negative numbers), and the seconds left until it is there once it casts.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Phase {
    to: Vec3,
    level: Level,
    direct: bool,
    left: Option<f32>,
}

impl Phase {
    /// To the place `to` on the level `level`.
    pub fn to(to: Vec3, level: Level) -> Self {
        Self {
            to,
            level,
            direct: false,
            left: None,
        }
    }

    /// To the cell `(x, y)` on the floor `floor`, as the record gives it: a cell of negative
    /// numbers is the cell `|c| − 1` along each axis, which the cell's point loses its sign at
    /// (`⌊(c + 0.5) · 53.6656⌋`), reached with no test of the way, on the ground (`0x522742`).
    pub fn to_cell((x, y, floor): (i32, i32, i32), heights: &Heights) -> Option<Self> {
        let direct = x < 0 && y < 0;
        let along = |c: i32| {
            if c < 0 {
                c.unsigned_abs() - 1
            } else {
                c.unsigned_abs()
            }
        };
        let (x, y) = (i32::try_from(along(x)).ok()?, i32::try_from(along(y)).ok()?);
        let floor = usize::try_from(floor)
            .ok()
            .filter(|&floor| floor > 0 && !direct);
        let level = floor.and_then(|floor| {
            let building = heights.building_on_floor((x, y), floor)?;
            heights.floor_level(building, floor)
        });
        Some(Self {
            direct,
            ..Self::to(iso::ground(x as f32 + 0.5, y as f32 + 0.5), level)
        })
    }
}

/// The dice of the members of a target group (`rand() % n`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x1830))
    }
}

/// The creatures that the attacks find, by their names and groups, and send.
type Creatures<'w, 's> = Query<
    'w,
    's,
    (Entity, Option<&'static ScriptName>, &'static mut Mind),
    (Without<Dies>, Without<Body>),
>;

/// An attacker's target: the hero, or a creature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Hero,
    Creature(Entity),
}

/// Carries out the scripts' `Attack`s.
fn attack(
    mut attacks: ResMut<ScriptAttacks>,
    mut dice: ResMut<Dice>,
    heroes: Query<(), With<Body>>,
    mut creatures: Creatures,
) {
    if attacks.0.is_empty() {
        return;
    }
    for attack in std::mem::take(&mut attacks.0) {
        let party = |party: &Party| -> Vec<Entity> {
            let named = |name: &str| {
                creatures
                    .iter()
                    .find(|(_, named, _)| named.is_some_and(|named| named.0 == name))
                    .map(|(entity, ..)| entity)
            };
            match party {
                Party::Hero => Vec::new(),
                Party::Named(name) => named(name).into_iter().collect(),
                Party::Group(group) => creatures
                    .iter()
                    .filter(|(.., mind)| mind.group == Some(SCRIPT_GROUP | group))
                    .map(|(entity, ..)| entity)
                    .collect(),
            }
        };
        let attackers = party(&attack.attacker);
        let members = party(&attack.target);
        let groups = matches!(
            (&attack.attacker, &attack.target),
            (Party::Group(_), Party::Group(_))
        );
        for attacker in attackers {
            let target = match &attack.target {
                Party::Hero if !heroes.is_empty() => Target::Hero,
                Party::Hero => continue,
                Party::Named(_) => match members.first() {
                    Some(&member) => Target::Creature(member),
                    None => continue,
                },
                Party::Group(_) => {
                    let Ok(n) = u32::try_from(members.len()) else {
                        continue;
                    };
                    let drawn = usize::try_from(dice.0.below(n)).unwrap_or(0);
                    match members.get(drawn) {
                        Some(&member) => Target::Creature(member),
                        None => continue,
                    }
                }
            };
            if Target::Creature(attacker) == target {
                continue;
            }
            send(attacker, target, groups, &mut creatures);
        }
    }
}

/// Turns one of the pair `attacker`, `target` against every side where the pair is hostile
/// neither way, then sends the attacker at its target where it is hostile to it.
fn send(attacker: Entity, target: Target, groups: bool, creatures: &mut Creatures) {
    let sides = |creatures: &Creatures, entity: Entity| {
        creatures
            .get(entity)
            .ok()
            .map(|(.., mind)| (mind.side, mind.creature))
    };
    let Some(own) = sides(creatures, attacker) else {
        return;
    };
    let other = match target {
        Target::Hero => None,
        Target::Creature(entity) => match sides(creatures, entity) {
            Some(other) => Some(other),
            None => return,
        },
    };
    let turned = match minds::attack_sides(own, other, groups) {
        Sided::Attacker => Some(attacker),
        Sided::Target => match target {
            Target::Creature(entity) => Some(entity),
            Target::Hero => None,
        },
        Sided::Neither => None,
    };
    if let Some(turned) = turned
        && let Ok((.., mut mind)) = creatures.get_mut(turned)
    {
        info!(
            "script_attacks: {turned}, a {}, turns against every side",
            mind.creature
        );
        mind.side = AGAINST_EVERY_SIDE;
    }
    let Some((side, creature)) = sides(creatures, attacker) else {
        return;
    };
    let hostile = match target {
        Target::Hero => minds::hostile_to_heroes(side, creature),
        Target::Creature(entity) => sides(creatures, entity)
            .is_some_and(|(other_side, other)| minds::hostile(side, creature, other_side, other)),
    };
    if !hostile {
        debug!("script_attacks: {attacker} is no foe of its target");
        return;
    }
    let Ok((.., mut mind)) = creatures.get_mut(attacker) else {
        return;
    };
    let goes = match target {
        Target::Hero => mind.send_at_hero(),
        Target::Creature(entity) => mind.send_at(entity),
    };
    info!("script_attacks: {attacker} sent at {target:?}: {goes}");
}

/// Carries out the scripts' teleports: a creature drawn on the screen casts `PHASEN`, its sound
/// at it, and stands at its end 3.35 s over its cast's rate later, as the hero's teleport takes it
/// (`spells::teleport`): along the line to the place in steps, as far as the spell's value at the
/// level that a creature lacking it casts it at, `⌊level / 5⌋ + 1`, or onto a cell of negative
/// numbers with no test; one not drawn (no timer listener, `+8` bit 7, which the draw registers)
/// is put on the place at once (`sdk_ENGINE_TP`). The objects 775 and 774 are not drawn.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their components"
)]
fn phase(
    mut commands: Commands,
    (time, heights, arts): (Res<Time>, Res<Heights>, Option<Res<crate::arts::HeroArts>>),
    (children, drawn): (Query<&Children>, Query<&ViewVisibility>),
    mut phasing: Query<
        (
            Entity,
            &mut Phase,
            &mut Transform,
            Option<&Mind>,
            Option<&mut crowd::InTheWay>,
            Option<&mut buildings::Layered>,
        ),
        Without<Dies>,
    >,
    mut sounds: MessageWriter<Play>,
) {
    for (entity, mut phase, mut transform, mind, way, layered) in &mut phasing {
        let here = transform.translation;
        let mut to = phase.to;
        match phase.left {
            None => {
                let seen = children
                    .iter_descendants(entity)
                    .any(|child| drawn.get(child).is_ok_and(|seen| seen.get()));
                if seen {
                    sounds.write(Play {
                        sound: Of::Id(PHASE_SOUND),
                        at: Some(entity),
                        rate: (0.5, Roll::None),
                    });
                    let rate = opensacred_rules::fight::CAST_SPEED / 100.0;
                    phase.left = Some(spells::TELEPORT_TIME / rate);
                    commands.entity(entity).remove::<Walk>();
                    continue;
                }
            }
            Some(left) => {
                let left = left - time.delta_secs();
                if left > 0.0 {
                    phase.left = Some(left);
                    continue;
                }
                if !phase.direct {
                    let level = way.as_ref().and_then(|way| way.level);
                    let spell_level = mind.map_or(1, |mind| mind.level / 5 + 1);
                    let points = arts
                        .as_ref()
                        .and_then(|arts| {
                            arts.tables.spells.iter().find(|s| s.id == spells::TELEPORT)
                        })
                        .map_or(0, |record| spells::value(record, 0, spell_level));
                    let passes =
                        |[x, z]: [f32; 2]| heights.walkable_on(crate::paths::cell_of(x, z), level);
                    let stands = |[x, z]: [f32; 2]| heights.standing(x, z, level).is_some();
                    let [x, z] =
                        spells::teleport([here.x, here.z], [to.x, to.z], points, passes, stands);
                    to = Vec3::new(x, here.y, z);
                }
            }
        }
        let level = if phase.left.is_some() && !phase.direct {
            way.as_ref().and_then(|way| way.level)
        } else {
            phase.level
        };
        if let Some(y) = heights.standing(to.x, to.z, level) {
            to.y = y;
        }
        info!("script_attacks: {entity} teleports to {to}");
        transform.translation = to;
        if let Some(mut way) = way {
            way.level = level;
        }
        if let Some(mut layered) = layered {
            *layered = buildings::Layered::at(&heights, to.x, to.z, level);
        }
        commands.entity(entity).remove::<(Phase, Walk)>();
    }
}

/// The scripts' `Attack`s.
pub struct ScriptAttacksPlugin;

impl Plugin for ScriptAttacksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>().add_systems(
            Update,
            (attack, phase)
                .after(crate::npcs::script_records)
                .in_set(Playing),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_teleport_to_a_cell_of_negative_numbers_lands_a_cell_short_with_no_test() {
        let heights = Heights::house();
        // Quest 1002's (−1655, −2409, 0): on 1654, 2408 of the ground, straight there.
        let phase = Phase::to_cell((-1655, -2409, 0), &heights).unwrap();
        assert!(phase.direct);
        assert_eq!(phase.to, iso::ground(1654.5, 2408.5));
        assert_eq!(phase.level, None);
        // A cell of its own, by the way's tests.
        let phase = Phase::to_cell((3252, 2583, 0), &heights).unwrap();
        assert!(!phase.direct && phase.left.is_none());
        assert_eq!(phase.to, iso::ground(3252.5, 2583.5));
    }
}
