//! The followers' entries of the portraits' column take a weapon or a shield from the cursor and
//! give theirs (`cUI_Mercenary`, its input `0x6D6C70`; #1771; docs/re/ai.md, "A hireling's
//! hands"). A left click on a follower's entry with the cursor empty takes the item of its right
//! hand, else of its left, onto the cursor. With an item held that the follower takes (a shield,
//! or a weapon whose kind's attack its model has, `0x6D6B90`) and may use (`0x54ABE0`), the item
//! goes into a hand by `opensacred_rules::equipment::handed`, and the item it replaces onto the
//! cursor (`0x6D6760`); its values, its clips and its fight are made anew as an archer's rearming
//! makes them (`npcs::Rearms`). An item that it may use, put on or not, makes it remark how the
//! item compares with its hands' old ones (the event (8, `0x10`), `remarks::Remark::Equipment`,
//! #1733); an item refused posts (8, `0x11`), which no case of the sound manager plays. A potion
//! held given to a follower to drink (`0x54AA80`) is not followed.

use bevy::prelude::*;
use opensacred_rules::drops::Dropped;
use opensacred_rules::equipment::{self, Hands, LEFT_HAND, RIGHT_HAND};

use crate::ai::Mind;
use crate::bar::{self, Entries};
use crate::drops::Tables;
use crate::fight::{Health, Rules};
use crate::followers::Follower;
use crate::game::AppState;
use crate::hud::Pointer;
use crate::npcs::{Arms, Npcs, Rearms, ScriptName};
use crate::pickup::Pack;
use crate::remarks::Remark;

/// The items that the hero gave a follower, by its hands' slots (12 the left, 13 the right), so
/// that one taken back is the item given, its own values, sockets and name.
#[derive(Component, Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Given([Option<Dropped>; 2]);

impl Given {
    /// The item given into the hand of the slot `slot`, taken out of the list.
    fn take(&mut self, slot: u8) -> Option<Dropped> {
        self.0
            .get_mut(usize::from(slot.checked_sub(LEFT_HAND)?))?
            .take()
    }

    /// Keeps `dropped` as the item given into the hand of the slot `slot`.
    fn put(&mut self, slot: u8, dropped: Dropped) {
        if let Some(hand) = slot
            .checked_sub(LEFT_HAND)
            .and_then(|hand| self.0.get_mut(usize::from(hand)))
        {
            *hand = Some(dropped);
        }
    }

    /// The item given into the hand of the slot `slot`.
    fn get(&self, slot: u8) -> Option<&Dropped> {
        self.0
            .get(usize::from(slot.checked_sub(LEFT_HAND)?))?
            .as_ref()
    }
}

/// The key of a creature's name (`sdk_REFKEY`, `0x44A1C0`): the number of its `CreateNPC`'s name
/// of the prefix `res:` (docs/re/netscript.md), else its type.
fn name_key(name: Option<&ScriptName>, creature: u32) -> u32 {
    name.and_then(|name| name.0.strip_prefix("res:")?.parse().ok())
        .unwrap_or(creature)
}

/// A left click on a follower's entry of the column (`0x6D6C70`'s entries 1 to 4).
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn press(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    entries: Option<Res<Entries>>,
    mut pack: ResMut<Pack>,
    (npcs, tables, rules): (Option<Res<Npcs>>, Option<Res<Tables>>, Res<Rules>),
    followers: Query<
        (
            (&Arms, &Mind, &Health, &crate::fight::Combat),
            Option<&Given>,
            Option<&ScriptName>,
        ),
        With<Follower>,
    >,
    mut remarks: MessageWriter<Remark>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let (Some(entries), Some(npcs), Some(tables)) = (entries, npcs, tables) else {
        return;
    };
    let Some(k) = bar::column_point(&pointer, &ui_scale, &windows)
        .and_then(|at| bar::entry_under(at, entries.0.len() + 1))
    else {
        return;
    };
    let Some(&follower) = k.checked_sub(1).and_then(|k| entries.0.get(k)) else {
        return;
    };
    let Ok(((arms, mind, health, combat), given, name)) = followers.get(follower) else {
        return;
    };
    debug!(
        "mercenaries: {follower} of type {} pressed, the cursor holding {:?}",
        mind.creature,
        pack.held.as_ref().map(|held| held.dropped.item)
    );
    let mut given = given.cloned().unwrap_or_default();
    // An item off a hand onto the cursor: the one given, else the hand's own as it was made.
    let hand_back = |slot: u8, pack: &mut Pack, given: &mut Given| {
        let dropped = given.take(slot).or_else(|| {
            arms.in_hand(slot)
                .map(|(item, values)| tables.held_item(item, health.level, values))
        });
        if let Some(dropped) = dropped {
            info!("mercenaries: {follower} gives {} back", dropped.item);
            let size = tables.size(dropped.item);
            pack.hold(dropped, size);
        }
    };
    let Some(held) = pack.held.as_ref() else {
        // The right hand's item, else the left's (`0x556A30(13)`, then `(12)`).
        let Some(slot) = [RIGHT_HAND, LEFT_HAND]
            .into_iter()
            .find(|&slot| arms.in_hand(slot).is_some())
        else {
            return;
        };
        hand_back(slot, &mut pack, &mut given);
        commands
            .entity(follower)
            .insert((Rearms::handing(&[slot], None), given));
        return;
    };
    let item = held.dropped.item;
    if !npcs.takes_through_portrait(arms, item) {
        debug!("mercenaries: {follower} takes no {item} through its portrait");
        return;
    }
    let asks = tables.asks(&held.dropped, &rules.0);
    // Its own attributes with its items' and its skills as its fight counts them (`0x54ABE0` on
    // the follower, #2411).
    let skills = arms.skills(health.level);
    let wearer = equipment::Wearer {
        creature: mind.creature,
        level: health.level,
        attributes: Some(combat.0.attributes),
        skills: Some(&skills),
    };
    if !equipment::may_use(&wearer, &asks) {
        info!(
            "mercenaries: {follower} of type {} at level {} may not use {item}",
            mind.creature, health.level
        );
        return;
    }
    // How it compares with the hands' old items, as the remark tells (`0x6D6660`).
    let worth = |slot: u8| {
        let old = given
            .get(slot)
            .map(|dropped| tables.values(dropped))
            .or_else(|| {
                arms.in_hand(slot).map(|(item, values)| {
                    tables.values(&tables.held_item(item, health.level, values))
                })
            });
        old.map_or(0, |values| equipment::hand_worth(&values))
    };
    let value = equipment::compared(
        equipment::hand_worth(&tables.values(&held.dropped)),
        worth(LEFT_HAND),
        worth(RIGHT_HAND),
    );
    remarks.write(Remark::Equipment {
        key: name_key(name, mind.creature),
        value,
    });
    let Some(grip) = npcs.grip(item) else {
        return;
    };
    let both = |slot: u8| {
        arms.in_hand(slot)
            .map(|(item, _)| npcs.grip(item).is_some_and(|grip| grip.both))
    };
    let hands = Hands {
        left: both(LEFT_HAND),
        right: both(RIGHT_HAND),
    };
    let Some(handing) = equipment::handed(grip, hands, arms.two_weapons(health.level)) else {
        info!("mercenaries: {follower}'s hands are full for {item}, which takes both");
        return;
    };
    let Some(held) = pack.held.take() else {
        return;
    };
    let values = tables.values(&held.dropped);
    if let Some(off) = handing.off {
        hand_back(off, &mut pack, &mut given);
    }
    info!(
        "mercenaries: {follower} takes {item} into its hand {}, {:?} off",
        handing.into, handing.off
    );
    given.put(handing.into, held.dropped);
    let off: Vec<u8> = handing.off.into_iter().collect();
    commands.entity(follower).insert((
        Rearms::handing(&off, Some((item, handing.into, values))),
        given,
    ));
}

/// The followers' entries' clicks.
pub struct MercenariesPlugin;

impl Plugin for MercenariesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, press.run_if(in_state(AppState::InGame)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_creatures_name_key_is_its_res_number_else_its_type() {
        let named = ScriptName("res:17095".to_owned());
        assert_eq!(name_key(Some(&named), 322), 17095);
        let scripted = ScriptName("merc_1".to_owned());
        assert_eq!(name_key(Some(&scripted), 322), 322);
        assert_eq!(name_key(None, 77), 77);
    }
}
