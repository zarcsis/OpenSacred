//! The potions' belt (#1311; docs/re/inventory.md, §7, docs/re/interface.md, "The bar"): its
//! five slots lit in their kinds' colours while the backpack's grid holds a potion of the kind
//! (`0x5DA8C0`), else the empty flask. The left button on a slot drinks its potion, Space the
//! healing one of the kind 1 (whose key the original's tooltip names), and the right button on a
//! potion of the grid, the cursor empty, drinks that one (`0x6C0150`, the hero's event `0x10`).
//! A drink (`0x54AA80`, `cPotion3D`'s `0x5CAA60`) plays one of the three drinking sounds; a
//! healing potion, where the hero's health is below its most, gives back its share of the most
//! health (`opensacred_rules::potions`) and is used up, with the sound of an item used up
//! (`0x5D8000`).
//!
//! Not followed: the other kinds' effects (the creature events 6, 7, 8 and 10), whose drink plays
//! its sound and uses nothing; the right button's potion to the weakest follower; the hero's horse
//! healed with it; the slots' sparkles and tooltips (#1272).

use bevy::prelude::*;
use opensacred_rules::potions::{self, choose, healed, missing};
use opensacred_rules::spawning::Rng;

use crate::backpack::{cell_under, on_grid};
use crate::bar::{self, EMPTY_FLASK, UiImages};
use crate::fight::Health;
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

/// What the healing potions give back on the game's difficulty.
#[derive(Resource, Debug, Default)]
struct Healing(potions::Healing);

/// Takes the healing potions' shares of the balance on the game's difficulty.
fn begin(mut commands: Commands, data: Option<Res<Data>>, difficulty: Option<Res<Difficulty>>) {
    let balance = data.as_ref().and_then(|data| data.0.tables.balance().ok());
    let difficulty = difficulty.map(|d| d.0).unwrap_or_default();
    commands.insert_resource(Healing(potions::Healing::at(balance, difficulty)));
}

/// Lights each flask while the grid holds a potion of its kind.
fn light(
    pack: Res<Pack>,
    healing: Option<Res<Healing>>,
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

/// The left button on a slot of the belt, Space, or the right button on a potion of the grid:
/// the hero drinks the potion (`0x612FA0`, `0x54AA80`).
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system's parameters are its inputs"
)]
fn drink(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    shown: Res<Shown>,
    mut pack: ResMut<Pack>,
    healing: Option<Res<Healing>>,
    mut heroes: Query<&mut Health, With<Body>>,
    mut sounds: MessageWriter<sfx::Play>,
    mut dice: Local<Option<Rng>>,
) {
    let Ok(mut health) = heroes.single_mut() else {
        return;
    };
    if health.current == 0 {
        return;
    }
    let healing = healing.map(|h| h.0).unwrap_or_default();
    let wound = Some(missing(health.current, health.most));
    let kind = if keys.just_pressed(KeyCode::Space) {
        Some(1)
    } else if buttons.just_pressed(MouseButton::Left) {
        screen_point(&pointer, &ui_scale, &windows).and_then(bar::belt_slot)
    } else {
        None
    };
    let key = match kind {
        Some(kind) => choose(pack.potions(), kind, wound, &healing),
        // The right button on a potion of the grid, the cursor empty.
        None if shown.0 && pack.held.is_none() && buttons.just_pressed(MouseButton::Right) => {
            on_grid(&pointer, &ui_scale, &windows)
                .and_then(cell_under)
                .and_then(|cell| pack.grid.at(cell))
                .map(|placed| placed.key)
                .filter(|key| pack.potions().iter().any(|(k, _)| k == key))
        }
        None => None,
    };
    let Some((key, item)) = key.and_then(|key| pack.potions().into_iter().find(|p| p.0 == key))
    else {
        return;
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
    match healing.heals(item) {
        Some(share) if health.current < health.most => {
            health.current = healed(health.current, health.most, share);
            pack.use_up(key);
            play(&mut sounds, USED_UP);
            info!(
                "belt: the hero drinks {item}: health {} of {}",
                health.current, health.most
            );
        }
        _ => debug!("belt: the hero drinks {item} to no effect"),
    }
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
