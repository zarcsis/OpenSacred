//! The followers' entries of the portraits' column take a weapon or a shield from the cursor and
//! give theirs (`cUI_Mercenary`, its input `0x6D6C70`; #1771; docs/re/ai.md, "A hireling's
//! hands"). A left click on a follower's entry with the cursor empty takes the item of its right
//! hand, else of its left, onto the cursor. With an item held that the follower takes (a shield,
//! or a weapon whose kind's attack its model has, `0x6D6B90`) and may use (`0x54ABE0`), the item
//! goes into a hand by `opensacred_rules::equipment::handed`, and the item it replaces onto the
//! cursor (`0x6D6760`); its values, its clips and its fight are made anew as an archer's rearming
//! makes them (`npcs::Rearms`). The remarks of a hireling given an item and of one refused (the
//! events (8, `0x10`) and (8, `0x11`)) are #1733's; a potion held given to a follower to drink
//! (`0x54AA80`) is not followed.

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
use crate::npcs::{Arms, Npcs, Rearms};
use crate::pickup::Pack;

/// The items that the hero gave a follower, by its hands' slots (12 the left, 13 the right), so
/// that one taken back is the item given, its own values, sockets and name.
#[derive(Component, Clone, Debug, Default)]
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
}

/// A left click on a follower's entry of the column (`0x6D6C70`'s entries 1 to 4).
fn press(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    entries: Option<Res<Entries>>,
    mut pack: ResMut<Pack>,
    (npcs, tables, rules): (Option<Res<Npcs>>, Option<Res<Tables>>, Res<Rules>),
    followers: Query<(&Arms, &Mind, &Health, Option<&Given>), With<Follower>>,
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
    let Ok((arms, mind, health, given)) = followers.get(follower) else {
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
    if !equipment::may_use(mind.creature, health.level, &asks) {
        info!(
            "mercenaries: {follower} of type {} at level {} may not use {item}",
            mind.creature, health.level
        );
        return;
    }
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
