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

use crate::ai::Mind;
use crate::frame::Playing;
use crate::hero::Body;
use crate::npcs::{Dies, Party, SCRIPT_GROUP, ScriptAttacks, ScriptName};

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

/// The scripts' `Attack`s.
pub struct ScriptAttacksPlugin;

impl Plugin for ScriptAttacksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>().add_systems(
            Update,
            attack.after(crate::npcs::script_records).in_set(Playing),
        );
    }
}
