//! A creature's hands under the Seraphim's effects 16 and 17 (#3030; docs/re/arts.md, "The
//! Seraphim's", "The hands as the effects come and go"): the ice druid that orders itself her
//! 1018 and 1019, or any creature given the effects. As the effect 16 comes, the staff 3809 goes
//! into its right hand (slot 13), and as it goes the staff is taken out and destroyed; as the
//! effect 17 comes, each item in its hands bursts into blue sparks of its shape (the object 856
//! of the variant 2, drawn by `puffs`) and both hands are emptied. Nothing comes back to the
//! creature's hands: the original's takers restore nothing but a player's weapon set
//! (`0x582D50`, `0x586D80`, `0x556A30`). The BFG's bolt leaves only while the staff is in its
//! hand ([`BoltHands::staff`], `0x4DE38C`), the hero's own being `arsenal`'s.

use bevy::prelude::*;
use opensacred_rules::arts::{self, BFG_ITEM};

use crate::effects::Effects;
use crate::frame::{Playing, Unpaused};
use crate::game::AppState;
use crate::hero::Body;

/// What a creature's hands hold of the effects 16 and 17: whether it held each at its last
/// change, and whether the staff of the effect 16 is in its right hand.
#[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoltHands {
    bfg: bool,
    railgun: bool,
    pub staff: bool,
}

/// What a creature's hands do as its effects change from `was` to holding the effect 16 (`bfg`)
/// and 17 (`railgun`): its new state, whether its hands burst and empty (the effect 17 come),
/// whether the staff goes in (the effect 16 come) and whether the staff goes out (the effect 16
/// gone with the staff still held).
fn changes(was: BoltHands, (bfg, railgun): (bool, bool)) -> (BoltHands, [bool; 3]) {
    let empties = railgun && !was.railgun;
    let arms = bfg && !was.bfg;
    let mut staff = was.staff && !empties;
    let drops = !bfg && was.bfg && staff;
    if drops {
        staff = false;
    }
    if arms {
        staff = true;
    }
    (
        BoltHands {
            bfg,
            railgun,
            staff,
        },
        [empties, arms, drops],
    )
}

/// The sparks' dice.
#[derive(Resource)]
struct Sparks(crate::spawning::Rng);

impl Default for Sparks {
    fn default() -> Self {
        Self(crate::seed::dice(0x3030))
    }
}

/// Changes each creature's hands as its effects 16 and 17 come and go ([`changes`]).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn hands(
    mut commands: Commands,
    mut creatures: Query<
        (Entity, &Effects, Option<&mut BoltHands>, &GlobalTransform),
        (With<crate::ai::Mind>, Without<Body>, Changed<Effects>),
    >,
    weapons: Query<(Entity, &crate::models::OrderedWith), With<crate::npcs::Wielded>>,
    (shapes, meshes): (crate::vertices::ModelPoints, Res<Assets<Mesh>>),
    mut dice: ResMut<Sparks>,
) {
    for (creature, effects, hands, at) in &mut creatures {
        let was = hands.as_deref().copied().unwrap_or_default();
        let (now, [empties, arms, drops]) = changes(
            was,
            (effects.holds(arts::BFG), effects.holds(arts::RAILGUN)),
        );
        if now == was {
            continue;
        }
        match hands {
            Some(mut hands) => *hands = now,
            None => {
                commands.entity(creature).insert(now);
            }
        }
        if empties {
            let origin = at.translation();
            for (weapon, _) in weapons.iter().filter(|(_, with)| with.0 == creature) {
                let points: Vec<[f32; 3]> = shapes
                    .picked(weapon, &meshes)
                    .into_iter()
                    .map(|point| crate::puffs::local(point, origin))
                    .collect();
                if points.is_empty() {
                    continue;
                }
                crate::puffs::spawn(
                    &mut commands,
                    opensacred_rules::particles::vertex_sparks(
                        &points,
                        opensacred_rules::particles::WEAPON_SPARKS,
                        &mut dice.0,
                    ),
                    origin,
                    None,
                );
            }
            debug!("bolt hands: {creature}'s hands burst and empty");
        }
        let rearms = match (empties, arms, drops) {
            (_, true, _) => Some(crate::npcs::Rearms::right(BFG_ITEM)),
            (true, false, _) => Some(crate::npcs::Rearms::empty_hands()),
            (false, false, true) => Some(crate::npcs::Rearms::empty_right()),
            (false, false, false) => None,
        };
        if let Some(rearms) = rearms {
            debug!("bolt hands: {creature} holds the staff {}", now.staff);
            commands.entity(creature).insert(rearms);
        }
    }
}

/// The creatures' hands under the Seraphim's effects.
pub struct BoltHandsPlugin;

impl Plugin for BoltHandsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Sparks>().add_systems(
            Update,
            hands
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
    fn the_staff_comes_with_the_effect_16_and_goes_with_it_or_the_effect_17() {
        let none = BoltHands::default();
        // The effect 16: the staff in.
        let (bfg, [empties, arms, drops]) = changes(none, (true, false));
        assert!(bfg.staff && arms && !empties && !drops);
        // Its end: the staff out.
        let (gone, [_, _, drops]) = changes(bfg, (false, false));
        assert!(!gone.staff && drops);
        // The effect 17 on empty hands: they empty all the same, nothing in them.
        let (railgun, [empties, arms, _]) = changes(none, (false, true));
        assert!(empties && !arms && !railgun.staff);
        // The effect 17 while 16 holds: the staff bursts and goes for good, and 16's end takes
        // nothing out.
        let (both, [empties, _, _]) = changes(bfg, (true, true));
        assert!(empties && !both.staff);
        let (sixteen, _) = changes(both, (true, false));
        assert!(!sixteen.staff);
        let (_, [_, _, drops]) = changes(sixteen, (false, false));
        assert!(!drops);
        // The effect 16 while 17 holds: a new staff all the same.
        let (again, [_, arms, _]) = changes(railgun, (true, true));
        assert!(arms && again.staff);
    }
}
