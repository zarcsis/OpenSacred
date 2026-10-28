//! The runes read from the backpack (#1315; docs/re/inventory.md, §3 and §7): the right button on
//! a rune of the grid, the cursor empty, teaches the hero the rune's combat art or raises it a
//! level (`cItemUpgrade`, `0x5CD100`), where the hero may use the rune (`0x54ABE0`: its classes,
//! its weapon's kind and the level it asks); the rune is then used up, with the sound of an item
//! used up (`0x5D8000`). A rune the hero may not use, or of no art, is left as it is.
//!
//! A book or a scroll (the category 16) read the same way (#1358, the book's handler at
//! `0x5CBFC0`) joins the hero's books read, once (`pickup::Pack::read`, `0x5DD5D0`), and is used
//! up with the same sound, read before or not; the sound event `(9, 0x11)` of a book new to the
//! list plays nothing (`0x67A7A0`).
//!
//! Not followed: the spells' page told of the art (`UI_TABPAGE_SPELLS`), the taskbar's book button
//! told of a new book (`UI_TB_BOOK`), the cell's sparkle, and the event (7, 2) of an art that
//! neither table holds.

use bevy::prelude::*;
use opensacred_rules::equipment::may_use;

use crate::arts::HeroArts;
use crate::backpack::{cell_under, on_grid};
use crate::drops::Tables;
use crate::fight::{Experience, Rules};
use crate::frame::Unpaused;
use crate::game::AppState;
use crate::hero::Hero;
use crate::hud::Pointer;
use crate::inventory::Shown;
use crate::pickup::Pack;
use crate::sfx;

/// `UI_INV_USEITEM01`, an item used up (`0x5D8000`).
const USED_UP: u32 = 7006;

/// The category of the books and the scrolls (`items.pak +0x2E`).
const BOOK: u8 = 16;

/// The right button on a rune or a book of the grid, the cursor empty: the hero reads it.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system's parameters are its inputs"
)]
fn read(
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    shown: Res<Shown>,
    mut pack: ResMut<Pack>,
    (tables, rules, experience): (Option<Res<Tables>>, Res<Rules>, Res<Experience>),
    hero: Option<Res<Hero>>,
    arts: Option<ResMut<HeroArts>>,
    mut sounds: MessageWriter<sfx::Play>,
) {
    if !shown.0 || pack.held.is_some() || !buttons.just_pressed(MouseButton::Right) {
        return;
    }
    let Some(tables) = tables else {
        return;
    };
    let used_up = |sounds: &mut MessageWriter<sfx::Play>| {
        sounds.write(sfx::Play {
            sound: sfx::Of::Id(USED_UP),
            at: None,
            rate: (0.5, sfx::Roll::None),
        });
    };
    let Some(placed) = on_grid(&pointer, &ui_scale, &windows)
        .and_then(cell_under)
        .and_then(|cell| pack.grid.at(cell).copied())
    else {
        return;
    };
    let Some(dropped) = pack
        .items
        .iter()
        .find(|(key, _)| *key == placed.key)
        .map(|(_, dropped)| *dropped)
    else {
        return;
    };
    if tables.category(dropped.item) == BOOK {
        let new = pack.read(dropped.item);
        pack.use_up(placed.key);
        used_up(&mut sounds);
        let before = if new { "" } else { ", read before" };
        info!("runes: the hero reads the book {}{before}", dropped.item);
        return;
    }
    let (Some(hero), Some(mut arts)) = (hero, arts) else {
        return;
    };
    let Some(art) = tables.rune_art(dropped.item) else {
        return;
    };
    let asks = tables.asks(&dropped, &rules);
    if !may_use(hero.creature, experience.level, &asks) {
        debug!("runes: the hero may not read {}", dropped.item);
        return;
    }
    match arts.learn(art, 1) {
        Ok(()) => {
            pack.use_up(placed.key);
            used_up(&mut sounds);
            info!("runes: the hero reads {}: the art {art}", dropped.item);
        }
        Err(e) => debug!("runes: {}: {e}", dropped.item),
    }
}

/// The runes and the books read.
pub struct RunesPlugin;

impl Plugin for RunesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            read.in_set(Unpaused).run_if(in_state(AppState::InGame)),
        );
    }
}
