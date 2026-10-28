//! The equipment window's slots (#1288; docs/re/inventory.md, §4): each class's frames of its
//! slots at their places in the window (`opensacred_rules::equipment::frames`), drawn white at the
//! alpha `0xAF`, and their glow while the cursor holds an item: green where the slot takes it and
//! the hero may use it, red otherwise (`0xAF00FF00`, `0xAFFF0000`; seen in the original with a
//! potion held, every slot red).
//!
//! No item is worn in the window yet: the classes' default items it holds at the start are not
//! drawn, as in the original's frames, and putting items on is #1289's.

use bevy::prelude::*;
use opensacred_rules::equipment::{Glow, frames, glow};

use crate::bar::{self, UiImages};
use crate::drops::Tables;
use crate::fight::{Experience, Rules};
use crate::game::AppState;
use crate::hero::Hero;
use crate::inventory::EquipmentWindow;
use crate::pickup::Pack;

/// The frames' alpha (`0xAF`).
const ALPHA: f32 = 175.0 / 255.0;

/// A slot's frame, of its slot.
#[derive(Component)]
struct SlotFrame(u8);

/// Gives the equipment window its class's frames as it is made.
fn show(
    mut commands: Commands,
    windows: Query<Entity, Added<EquipmentWindow>>,
    hero: Option<Res<Hero>>,
    (images, assets): (Res<UiImages>, Res<AssetServer>),
) {
    let Some(hero) = hero else {
        return;
    };
    for window in &windows {
        for frame in frames(hero.creature) {
            let at = Vec2::new(f32::from(frame.at.0), f32::from(frame.at.1));
            let (mut image, node) = bar::piece(&images, &assets, frame.image, at, Vec2::ZERO);
            image.color = Color::srgba(1.0, 1.0, 1.0, ALPHA);
            let child = commands
                .spawn((image, node, SlotFrame(frame.slot), Pickable::IGNORE))
                .id();
            commands.entity(window).add_child(child);
        }
    }
}

/// Tints each frame by its glow for the item the cursor holds, the hero's type and level.
fn tint(
    pack: Res<Pack>,
    hero: Option<Res<Hero>>,
    (experience, rules, tables): (Res<Experience>, Res<Rules>, Option<Res<Tables>>),
    mut frames: Query<(&SlotFrame, &mut ImageNode)>,
) {
    let Some(hero) = hero else {
        return;
    };
    let asks = pack
        .held
        .zip(tables.as_deref())
        .map(|(held, tables)| tables.asks(&held.dropped, &rules));
    for (frame, mut image) in &mut frames {
        let wanted = match glow(frame.0, asks.as_ref(), (hero.creature, experience.0.level)) {
            Glow::White => Color::srgba(1.0, 1.0, 1.0, ALPHA),
            Glow::Green => Color::srgba(0.0, 1.0, 0.0, ALPHA),
            Glow::Red => Color::srgba(1.0, 0.0, 0.0, ALPHA),
        };
        if image.color != wanted {
            image.color = wanted;
        }
    }
}

/// The equipment window's slots.
pub struct EquipmentPlugin;

impl Plugin for EquipmentPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (show, tint)
                .chain()
                .after(crate::inventory::Opened)
                .run_if(in_state(AppState::InGame)),
        );
    }
}
