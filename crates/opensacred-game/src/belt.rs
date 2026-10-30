//! The potions' belt (#1311; docs/re/inventory.md, §7, docs/re/interface.md, "The bar"): its
//! five slots lit in their kinds' colours while the backpack's grid holds a potion of the kind
//! (`0x5DA8C0`), else the empty flask. The left button on a slot drinks its potion, Space the
//! healing one of the kind 1 (whose key the original's tooltip names), and the right button on a
//! potion of the grid, the cursor empty, drinks that one (`0x6C0150`, the hero's event `0x10`).
//! With Shift, Space drinks the yellow potion of the kind 5; the right button on a slot of the
//! kinds 1, 2 or 4 gives its potion to the hero's follower of the lowest most health among those
//! below 99 % of their health (`0x612FA0`, #1313), as Shift with the right button on a potion of
//! the grid gives that one (`0x6C0150`, #1272).
//! A drink (`0x54AA80`, `cPotion3D`'s `0x5CAA60`) plays one of the three drinking sounds, and by
//! its kind (`opensacred_rules::potions::Drink`, #1353): a healing potion, where the drinker's
//! health is below its most, gives back its share of the most health; a purple or a blue one gives
//! the drinker its effect 33, one alone of its whole value; a green one cures its effects 30; a
//! yellow one waits for the combos (#1090). Each is then used up, with the sound of an item used
//! up (`0x5D8000`); a healing potion at full health is not.
//!
//! The slots' sparkles are `sparkles`' (#1272). Not followed: the hero's horse healed with it; the
//! slots' tooltips (#1273).

use bevy::prelude::*;
use opensacred_rules::potions::{self, Drink, choose, healed, missing};
use opensacred_rules::spawning::Rng;

use crate::backpack::{cell_under, on_grid};
use crate::bar::{self, EMPTY_FLASK, UiImages};
use crate::effects::Effects;
use crate::fight::Health;
use crate::followers::Follower;
use crate::frame::Unpaused;
use crate::game::{AppState, Data, Difficulty};
use crate::hero::Body;
use crate::hud::Pointer;
use crate::inventory::{Shown, screen_point};
use crate::pickup::Pack;
use crate::sfx;

/// The belt's lit flasks by their kinds 1 to 5: red, purple, blue, green and yellow
/// (`GUI_main_05`).
const FILLED: [u32; 5] = [176, 179, 177, 178, 180];

/// The drinking sounds, `DRINK_POTION01` to `03`, one at random (`0x5CAA60`), and the sound of an
/// item used up, `UI_INV_USEITEM01` (`0x5D8000`).
const DRINKS: [u32; 3] = [7003, 7001, 7008];
const USED_UP: u32 = 7006;

/// A flask of the belt: its kind, 1 to 5, and the image it shows.
#[derive(Component, Debug)]
pub struct Flask {
    kind: u8,
    shown: u32,
}

impl Flask {
    pub fn new(kind: u8) -> Self {
        Self {
            kind,
            shown: EMPTY_FLASK,
        }
    }
}

/// What the potions do on the game's difficulty.
#[derive(Resource, Debug, Default)]
pub(crate) struct Potions(pub(crate) potions::Potions);

/// Takes the potions' values of the balance on the game's difficulty.
fn begin(mut commands: Commands, data: Option<Res<Data>>, difficulty: Option<Res<Difficulty>>) {
    let balance = data.as_ref().and_then(|data| data.0.tables.balance().ok());
    let difficulty = difficulty.map(|d| d.0).unwrap_or_default();
    commands.insert_resource(Potions(potions::Potions::at(balance, difficulty)));
}

/// Lights each flask while the grid holds a potion of its kind.
fn light(
    pack: Res<Pack>,
    healing: Option<Res<Potions>>,
    (images, assets): (Res<UiImages>, Res<AssetServer>),
    mut flasks: Query<(&mut Flask, &mut ImageNode)>,
) {
    let healing = healing.map(|h| h.0).unwrap_or_default();
    let potions = pack.potions();
    for (mut flask, mut image) in &mut flasks {
        if !pack.is_changed() && !flask.is_added() {
            continue;
        }
        let lit = choose(potions.iter().copied(), flask.kind, None, &healing).is_some();
        let index = match (lit, usize::from(flask.kind).checked_sub(1)) {
            (true, Some(kind)) => FILLED.get(kind).copied().unwrap_or(EMPTY_FLASK),
            _ => EMPTY_FLASK,
        };
        if index != flask.shown {
            *image = bar::piece(&images, &assets, index, Vec2::ZERO, Vec2::ZERO).0;
            flask.shown = index;
        }
    }
}

/// The followers whose health over their most is below this share are given the belt's potions
/// (`0x612FA0`).
const HURT: f64 = 0.99;

/// The left button on a slot of the belt, Space (with Shift the yellow potion), or the right
/// button on a potion of the grid: the hero drinks the potion (`0x612FA0`, `0x54AA80`). The right
/// button on a slot of the kinds 1, 2 or 4: its potion, chosen as for the hero, goes to the
/// hero's follower of the lowest most health among those below 99 % of their health, who drinks
/// it (#1313). What the drink does goes by the potion's kind (#1353).
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system's parameters are its inputs"
)]
fn drink(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    shown: Res<Shown>,
    mut pack: ResMut<Pack>,
    healing: Option<Res<Potions>>,
    mut heroes: Query<(Entity, &mut Health), With<Body>>,
    mut followers: Query<(Entity, &Follower, &mut Health), Without<Body>>,
    mut sounds: MessageWriter<sfx::Play>,
    mut dice: Local<Option<Rng>>,
) {
    let Ok((hero, health)) = heroes.single_mut() else {
        return;
    };
    if health.current == 0 {
        return;
    }
    let healing = healing.map(|h| h.0).unwrap_or_default();
    let wound = Some(missing(health.current, health.most));
    let slot = || screen_point(&pointer, &ui_scale, &windows).and_then(bar::belt_slot);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let (kind, given) = if keys.just_pressed(KeyCode::Space) {
        (Some(if shift { 5 } else { 1 }), false)
    } else if buttons.just_pressed(MouseButton::Left) {
        (slot(), false)
    } else if buttons.just_pressed(MouseButton::Right) {
        (slot().filter(|kind| matches!(kind, 1 | 2 | 4)), true)
    } else {
        (None, false)
    };
    let (key, given) = match kind {
        Some(kind) => (choose(pack.potions(), kind, wound, &healing), given),
        // The right button on a potion of the grid, the cursor empty; with Shift, to the weakest
        // follower (`0x6C0150`).
        None if shown.0 && pack.held.is_none() && buttons.just_pressed(MouseButton::Right) => {
            let key = on_grid(&pointer, &ui_scale, &windows)
                .and_then(cell_under)
                .and_then(|cell| pack.grid.at(cell))
                .map(|placed| placed.key)
                .filter(|key| pack.potions().iter().any(|(k, _)| k == key));
            (key, shift)
        }
        None => (None, false),
    };
    let Some((key, item)) = key.and_then(|key| pack.potions().into_iter().find(|p| p.0 == key))
    else {
        return;
    };
    let follower = if given {
        let hers = followers
            .iter()
            .filter(|(_, of, _)| of.master == hero)
            .map(|(entity, _, health)| (entity, health.current, health.most));
        let Some(follower) = weakest(hers) else {
            return;
        };
        Some(follower)
    } else {
        None
    };
    let mut drinker = match follower.and_then(|f| followers.get_mut(f).ok()) {
        Some((_, _, health)) => health,
        None => health,
    };
    let dice = dice.get_or_insert_with(|| Rng::of(0, 0, 1311));
    let roll = usize::try_from(dice.below(3)).unwrap_or(0);
    let play = |sounds: &mut MessageWriter<sfx::Play>, id: u32| {
        sounds.write(sfx::Play {
            sound: sfx::Of::Id(id),
            at: None,
            rate: (0.5, sfx::Roll::None),
        });
    };
    play(&mut sounds, DRINKS.get(roll).copied().unwrap_or(DRINKS[0]));
    let who = follower.map_or_else(|| "the hero".to_owned(), |f| format!("{f}"));
    let drinking = follower.unwrap_or(hero);
    match healing.drink(item) {
        Some(Drink::Heals(share)) if drinker.current < drinker.most => {
            drinker.current = healed(drinker.current, drinker.most, share);
            info!(
                "belt: {who} drinks {item}: health {} of {}",
                drinker.current, drinker.most
            );
        }
        Some(Drink::Lasts(effect, seconds)) => {
            affect(&mut commands, drinking, move |effects| {
                effects.add_alone(effect, seconds);
            });
            info!(
                "belt: {who} drinks {item}: the effect {} of the whole value {} for {seconds} s",
                effect.id, effect.whole
            );
        }
        Some(Drink::Cures(per_cent)) => {
            affect(&mut commands, drinking, move |effects| {
                effects.cure(per_cent)
            });
            info!("belt: {who} drinks {item}: the poison cured by {per_cent} %");
        }
        // The combos' regeneration, which waits for the combos (#1090).
        Some(Drink::Refills(per_cent)) => {
            info!("belt: {who} drinks {item}: the combos refilled by {per_cent} %, none held");
        }
        _ => {
            debug!("belt: {who} drinks {item} to no effect");
            return;
        }
    }
    pack.use_up(key);
    play(&mut sounds, USED_UP);
}

/// Changes the effects of `drinker` by `change`, its list made where it has none.
fn affect(
    commands: &mut Commands,
    drinker: Entity,
    change: impl FnOnce(&mut Effects) + Send + Sync + 'static,
) {
    commands
        .entity(drinker)
        .entry::<Effects>()
        .or_default()
        .and_modify(move |mut effects| change(&mut effects));
}

/// The follower given a belt's potion, of `followers` by their healths and most healths
/// (`0x612FA0`): of those below 99 % of their health, the one of the lowest most health, the first
/// of equal ones, below 99,999.
fn weakest<T>(followers: impl IntoIterator<Item = (T, u32, u32)>) -> Option<T> {
    let mut lowest = 99_999;
    let mut weakest = None;
    for (follower, current, most) in followers {
        let hurt = f64::from(current) < HURT * f64::from(most);
        if most < lowest && hurt {
            lowest = most;
            weakest = Some(follower);
        }
    }
    weakest
}

/// The potions' belt.
pub struct BeltPlugin;

impl Plugin for BeltPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), begin)
            .add_systems(
                Update,
                (light, drink.in_set(Unpaused)).run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_potion_goes_to_the_weakest_follower_that_is_hurt() {
        // A wolf at full health, a unicorn of 166 at 20, a skeleton of 90 at 89 (98.9 %).
        let followers = [
            ("wolf", 120, 120),
            ("unicorn", 20, 166),
            ("skeleton", 89, 90),
        ];
        assert_eq!(weakest(followers), Some("skeleton"));
        // At 99 % and above, none.
        assert_eq!(weakest([("skeleton", 99, 100), ("wolf", 120, 120)]), None);
        // Of equal most healths the first.
        assert_eq!(weakest([("a", 1, 50), ("b", 2, 50)]), Some("a"));
        assert_eq!(weakest::<&str>([]), None);
    }
}
