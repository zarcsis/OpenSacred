//! The potions' belt (#1311; docs/re/inventory.md, §7, docs/re/interface.md, "The bar"): its
//! five slots lit in their kinds' colours while the backpack's grid holds a potion of the kind
//! (`0x5DA8C0`), else the empty flask. The left button on a slot drinks its potion, Space the
//! healing one of the kind 1 (whose key the original's tooltip names), and the right button on a
//! potion of the grid, the cursor empty, drinks that one (`0x6C0150`, the hero's event `0x10`).
//! With Shift, Space drinks the yellow potion of the kind 5; the right button on a slot of the
//! kinds 1, 2 or 4 gives its potion to the hero's follower of the lowest most health among those
//! below 99 % of their health (`0x612FA0`, #1313), as Shift with the right button on a potion of
//! the grid gives that one (`0x6C0150`, #1272).
//! The keys Q, W, E and R drink the purple, blue, green and yellow potions, and B gives the healing
//! one to that follower (`cEngine_receiveEvent`, `0x618090`, #1536): on their characters, which
//! repeat while a key is held, where Space drinks once a press (its latch `+0x1C8`, which the
//! key's release clears). The open console holds them back with the clicks (#1554).
//! A drink (`0x54AA80`, `cPotion3D`'s `0x5CAA60`) plays one of the three drinking sounds, and by
//! its kind (`opensacred_rules::potions::Drink`, #1353): a healing potion, where the drinker's
//! health is below its most, gives back its share of the most health; a purple or a blue one gives
//! the drinker its effect 33, one alone of its whole value; a green one cures its effects 30; a
//! yellow one sets every combo's regeneration to its cocktail's factor of the combo's time
//! (#1531). Each is then used up, with the sound of an item used up (`0x5D8000`); a healing
//! potion at full health is not.
//!
//! The slots' sparkles are `sparkles`' (#1272), their tooltips `bar_tips`' (#1684). Not followed:
//! the hero's horse healed with it.

use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use opensacred_rules::potions::{self, Drink, choose, healed, missing};
use opensacred_rules::spawning::Rng;

use crate::backpack::{cell_under, on_grid};
use crate::bar::{self, EMPTY_FLASK, UiImages};
use crate::controls::{Action, Keys};
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

/// The potions' keys by their characters, without Control: the kind each drinks, and whether
/// it gives it to a follower (`0x618090`).
const LETTERS: [(Action, u8, bool); 5] = [
    (Action::PurplePotion, 2, false),
    (Action::BluePotion, 3, false),
    (Action::GreenPotion, 4, false),
    (Action::YellowPotion, 5, false),
    (Action::FollowerPotion, 1, true),
];

/// The left button on a slot of the belt, Space (with Shift the yellow potion), Q, W, E or R, or
/// the right button on a potion of the grid: the hero drinks the potion (`0x612FA0`, `0x54AA80`).
/// The right button on a slot of the kinds 1, 2 or 4, or B for the kind 1: its potion, chosen as
/// for the hero, goes to the hero's follower of the lowest most health among those below 99 % of
/// their health, who drinks it (#1313). What the drink does goes by the potion's kind (#1353).
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system's parameters are its inputs"
)]
fn drink(
    mut commands: Commands,
    (keys, mut letters, console): (
        Keys,
        MessageReader<KeyboardInput>,
        Option<Res<crate::console::Console>>,
    ),
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (shown, page): (Res<Shown>, Res<crate::inventory::Page>),
    mut pack: ResMut<Pack>,
    (healing, mut combos): (
        Option<Res<Potions>>,
        Option<ResMut<crate::combos::HeroCombos>>,
    ),
    mut heroes: Query<(Entity, &mut Health), With<Body>>,
    mut followers: Query<(Entity, &Follower, &mut Health), Without<Body>>,
    mut sounds: MessageWriter<sfx::Play>,
    mut dice: Local<Option<Rng>>,
) {
    // A character's press or its repeat, the first of the frame's (`0x618090`).
    let held = keys.modifiers();
    let lettered = letters
        .read()
        .filter(|key| key.state == ButtonState::Pressed && !held.control)
        .filter_map(|key| {
            LETTERS
                .iter()
                .find(|&&(action, ..)| keys.controls().is(action, key, held))
        })
        .map(|&(_, kind, given)| (kind, given))
        .reduce(|first, _| first);
    // The open console takes the keys and the clicks (#1554).
    if console.is_some_and(|console| console.is_open()) {
        return;
    }
    let Ok((hero, health)) = heroes.single_mut() else {
        return;
    };
    if health.current == 0 {
        return;
    }
    let healing = healing.map(|h| h.0).unwrap_or_default();
    let wound = Some(missing(health.current, health.most));
    let slot = || screen_point(&pointer, &ui_scale, &windows).and_then(bar::belt_slot);
    // The belt's key, Space, drinks the kind 6: the healing potion, with Shift the yellow
    // (`0x612FA0` reads Shift itself).
    let (kind, given) = if keys.just_pressed(Action::BeltPotion) {
        (Some(if held.shift { 5 } else { 1 }), false)
    } else if let Some((kind, given)) = lettered {
        (Some(kind), given)
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
            let key = on_grid(&pointer, &ui_scale, &windows, &page)
                .and_then(cell_under)
                .and_then(|cell| pack.grid.at(cell))
                .map(|placed| placed.key)
                .filter(|key| pack.potions().iter().any(|(k, _)| k == key));
            (key, keys.pressed(Action::SendItem))
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
    let drinking = (follower.unwrap_or(hero), follower.is_none());
    let dice = dice.get_or_insert_with(|| crate::seed::dice(1311));
    if drunk(
        &mut commands,
        drinking,
        &mut drinker,
        (item, &healing),
        (combos.as_deref_mut(), &mut sounds, dice),
    ) {
        pack.use_up(key);
        play(&mut sounds, USED_UP);
    }
}

/// Plays the sound `id` at the listener.
fn play(sounds: &mut MessageWriter<sfx::Play>, id: u32) {
    sounds.write(sfx::Play {
        sound: sfx::Of::Id(id),
        at: None,
        rate: (0.5, sfx::Roll::None),
    });
}

/// `drinker`, the hero or a creature (`hero`), of the health `health`, drinks the potion `item`
/// (`0x54AA80`, the potion's event `0x10`): a drink's sound by a roll of three, and what the drink
/// does by the potion's kind (#1353), the combos refilled for the hero alone; whether it did
/// anything, so that the potion is used up, a heal of one at its whole health doing nothing.
pub(crate) fn drunk(
    commands: &mut Commands,
    (drinker, hero): (Entity, bool),
    health: &mut Health,
    (item, healing): (u32, &potions::Potions),
    (combos, sounds, dice): (
        Option<&mut crate::combos::HeroCombos>,
        &mut MessageWriter<sfx::Play>,
        &mut Rng,
    ),
) -> bool {
    let roll = usize::try_from(dice.below(3)).unwrap_or(0);
    play(sounds, DRINKS.get(roll).copied().unwrap_or(DRINKS[0]));
    let who = if hero {
        "the hero".to_owned()
    } else {
        format!("{drinker}")
    };
    match healing.drink(item) {
        Some(Drink::Heals(share)) if health.current < health.most => {
            health.current = healed(health.current, health.most, share);
            info!(
                "belt: {who} drinks {item}: health {} of {}",
                health.current, health.most
            );
        }
        Some(Drink::Lasts(effect, seconds)) => {
            affect(commands, drinker, move |effects| {
                effects.add_alone(effect, seconds);
            });
            info!(
                "belt: {who} drinks {item}: the effect {} of the whole value {} for {seconds} s",
                effect.id, effect.whole
            );
        }
        Some(Drink::Cures(per_cent)) => {
            affect(commands, drinker, move |effects| effects.cure(per_cent));
            info!("belt: {who} drinks {item}: the poison cured by {per_cent} %");
        }
        // The combos' regeneration set to the cocktail's factor of their times (#1531).
        Some(Drink::Refills(factor)) if hero => {
            if let Some(combos) = combos {
                opensacred_rules::combos::refilled(&mut combos.0, factor);
            }
            info!("belt: {who} drinks {item}: the combos refilled by {factor} times");
        }
        _ => {
            debug!("belt: {who} drinks {item} to no effect");
            return false;
        }
    }
    true
}

/// The sound of a potion used up (`0x5DEBC0`).
pub(crate) fn used_up(sounds: &mut MessageWriter<sfx::Play>) {
    play(sounds, USED_UP);
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

    #[derive(Resource, Default)]
    struct Drunk(Vec<bool>);

    /// Each of the potions `items` drunk in turn by a follower of the health `health`.
    fn drinks(items: Vec<u32>, health: Health) -> (Vec<bool>, Health) {
        let mut app = App::new();
        app.add_message::<sfx::Play>().init_resource::<Drunk>();
        let follower = app.world_mut().spawn(health).id();
        app.add_systems(
            Update,
            move |mut commands: Commands,
                  mut healths: Query<&mut Health>,
                  mut sounds: MessageWriter<sfx::Play>,
                  mut done: ResMut<Drunk>| {
                let mut health = healths.get_mut(follower).unwrap();
                let mut dice = Rng::of(1, 2, 3);
                for &item in &items {
                    let did = drunk(
                        &mut commands,
                        (follower, false),
                        &mut health,
                        (item, &potions::Potions::default()),
                        (None, &mut sounds, &mut dice),
                    );
                    done.0.push(did);
                }
            },
        );
        app.update();
        let health = *app.world().get::<Health>(follower).unwrap();
        (app.world().resource::<Drunk>().0.clone(), health)
    }

    #[test]
    fn a_follower_drinks_a_potion_given_it_and_no_yellow_one() {
        let wounded = Health {
            current: 10,
            most: 48,
            level: 1,
        };
        // The small healing potion heals the novice; the yellow one does nothing for a follower.
        let (did, health) = drinks(vec![5171, 5172], wounded);
        assert_eq!(did, vec![true, false]);
        assert!(health.current > 10 && health.current <= 48, "{health:?}");
        // At its whole health a healing potion does nothing, so that it is not used up.
        let whole = Health {
            current: 48,
            ..wounded
        };
        assert_eq!(drinks(vec![5171], whole).0, vec![false]);
    }
}
