//! The town stash's window (#1411; docs/re/containers.md, "The town stash (5200)"): the original's
//! `UI_WND_CHEST` (`cUI_Chest`, `0x6ACB40`) at (0, 0) of its screen, 640 × 320, shown as the hero
//! opens a town stash, with the inventory (`containers`): its frame (`0x6AD540`), its grid of 20 ×
//! 10 cells from (0, 20) with the items in it, drawn as the backpack's (`backpack`), and its
//! clicks (`0x6ACFC0`): the cursor's item put in where it fits or in place of the one item under
//! it, an item taken onto the empty cursor, and Shift with a click sending an item to the
//! backpack. Its grid is one for every town stash, kept while the game runs.
//!
//! As the inventory hides, the window hides with it (`0x6ACD80`): the stash is no more emptied
//! and its lid closes with its sound.
//!
//! Not followed yet: the hovered item's tooltip (`0x75B390`), the window's own opening and closing
//! sounds over the inventory's, the saves (#1095), and Shift with a click on the backpack's item
//! while the window shows.

use bevy::prelude::*;
use opensacred_rules::drops::Dropped;
use opensacred_rules::inventory::{Grid, Put, held_at};
use opensacred_rules::spawning::Rng;

use crate::backpack;
use crate::bar::{self, UiImages};
use crate::choice::Layer;
use crate::drops::Tables;
use crate::game::AppState;
use crate::hud::Pointer;
use crate::inventory::{Shown, screen_point};
use crate::pickup::{Pack, lift_from, put_into};
use crate::sfx;

/// The window on the original's screen (`0x759AF0` at `0x75A046`) and the clicks it takes, 32
/// below its rectangle too (`0x6ACFC0`).
pub(crate) const WINDOW: Rect = Rect {
    min: Vec2::new(0.0, 0.0),
    max: Vec2::new(640.0, 352.0),
};
/// The grid: its top left on the screen and its columns and rows (`0x6ACB40`: the inventory
/// entry's `+0x115C`, `+0x115E`; **seen** from (0, 20) in the original's frames).
const GRID: Vec2 = Vec2::new(0.0, 20.0);
const COLUMNS: u8 = 20;
const ROWS: u8 = 10;
/// The frame (`0x6AD540`): the right edge's ten pieces from 20 down, the image 34 and the last 57,
/// each standing at its third anchor from the window's right; the bottom edge's ten pieces of the
/// images 15 to 19 at random from the left, each at its second anchor above 340; the top edge's
/// twenty of the image 21, each at its second anchor above 16.
const RIGHT: (u32, u32) = (34, 57);
const RIGHT_PIECES: u8 = 10;
const RIGHT_FROM: f32 = 20.0;
const BOTTOM: (u32, u32) = (15, 5);
const BOTTOM_PIECES: u8 = 10;
const BOTTOM_AT: f32 = 340.0;
const TOP: u32 = 21;
const TOP_PIECES: u8 = 20;
const TOP_AT: f32 = 16.0;
/// The render layer of the items' models.
const ITEMS: usize = 14;

/// The items in the town stashes, one store for all of them (the window's own inventory entry,
/// `0x6ACB40`): each by its key and its place in the grid.
#[derive(Resource)]
pub struct Chest {
    pub items: Vec<(u32, Dropped)>,
    pub grid: Grid,
}

impl Default for Chest {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            grid: Grid::new(COLUMNS, ROWS),
        }
    }
}

/// The town stash whose window shows (`+0x154`), none while it does not.
#[derive(Resource, Default)]
pub struct Opened(pub Option<Entity>);

/// The window's container.
#[derive(Component)]
struct Root;

/// The items' camera, light and models.
#[derive(Component, Clone)]
struct Scene;

/// A cell's tint, at its column and row.
#[derive(Component)]
struct Tint((u8, u8));

/// An item's model, by its key in the store.
#[derive(Component)]
struct Model(u32);

/// Whether the window shows: a town stash opened and the inventory shown.
fn showing(shown: &Shown, opened: &Opened) -> bool {
    shown.0 && opened.0.is_some()
}

/// The images and places of the window's frame (`0x6AD540`), by `anchors` and `size` of an image.
fn frame(
    dice: &mut Rng,
    anchors: impl Fn(u32) -> [f32; 4],
    size: impl Fn(u32) -> Vec2,
) -> Vec<(u32, Vec2)> {
    let mut pieces = Vec::new();
    let mut y = RIGHT_FROM;
    for step in 0..RIGHT_PIECES {
        let piece = if step + 1 == RIGHT_PIECES {
            RIGHT.1
        } else {
            RIGHT.0
        };
        pieces.push((piece, Vec2::new(WINDOW.width() - anchors(piece)[2], y)));
        y += size(piece).y;
    }
    let mut x = 0.0;
    for _ in 0..BOTTOM_PIECES {
        let piece = BOTTOM.0 + dice.below(BOTTOM.1);
        pieces.push((piece, Vec2::new(x, BOTTOM_AT - anchors(piece)[1])));
        x += size(piece).x;
    }
    let mut x = 0.0;
    for _ in 0..TOP_PIECES {
        pieces.push((TOP, Vec2::new(x, TOP_AT - anchors(TOP)[1])));
        x += size(TOP).x;
    }
    pieces
}

/// Makes the window, its frame, its grid and the items' camera as it shows, and takes them away as
/// it hides.
#[expect(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
fn show(
    mut commands: Commands,
    (shown, opened): (Res<Shown>, Res<Opened>),
    olds: Query<Entity, Or<(With<Root>, With<Scene>)>>,
    (images, assets): (Res<UiImages>, Res<AssetServer>),
    mut targets: ResMut<Assets<Image>>,
    mut layers: ResMut<Assets<Layer>>,
    window: Option<Single<&Window>>,
    mut dice: Local<Option<Rng>>,
) {
    if !shown.is_changed() && !opened.is_changed() {
        return;
    }
    for old in &olds {
        commands.entity(old).despawn();
    }
    if !showing(&shown, &opened) {
        return;
    }
    let root = commands.spawn((Root, backpack::screen())).id();
    let dice = dice.get_or_insert_with(|| crate::seed::dice(1411));
    let pieces = frame(
        dice,
        |index| images.anchors(index),
        |index| images.size(index),
    );
    for (index, at) in pieces {
        let piece = commands
            .spawn((
                bar::piece(&images, &assets, index, at, Vec2::ZERO),
                ZIndex(1),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(root).add_child(piece);
    }
    let scale = window.map_or(1.0, |window| window.height() / bar::SCREEN.y);
    backpack::spawn_grid(
        &mut commands,
        (&images, &assets),
        (&mut targets, &mut layers),
        scale,
        (root, GRID, (COLUMNS, ROWS)),
        (ITEMS, Tint, Scene),
    );
}

/// Lays the items' models out as the store holds them while the window shows.
fn lay_out(
    mut commands: Commands,
    (shown, opened): (Res<Shown>, Res<Opened>),
    chest: Res<Chest>,
    (tables, assets): (Option<Res<Tables>>, Res<AssetServer>),
    mut models: Query<(Entity, &Model, &mut Transform)>,
    added: Query<(), Added<Tint>>,
) {
    if !showing(&shown, &opened) || !(chest.is_changed() || !added.is_empty()) {
        return;
    }
    let Some(tables) = tables else {
        return;
    };
    let laid = models
        .iter_mut()
        .map(|(entity, model, transform)| (entity, model.0, transform))
        .collect();
    backpack::lay_models(
        &mut commands,
        (&tables, &assets),
        (&chest.items, &chest.grid),
        laid,
        (ITEMS, |key| (Scene, Model(key))),
    );
}

/// The pointer on the original's screen from the grid's top left.
fn on_grid(pointer: &Pointer, ui_scale: &UiScale, windows: &Query<&Window>) -> Option<Vec2> {
    Some(screen_point(pointer, ui_scale, windows)? - GRID)
}

/// Tints the cells while the window shows, as the backpack's (`backpack::cell_tint`).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn tint(
    (shown, opened): (Res<Shown>, Res<Opened>),
    (chest, pack): (Res<Chest>, Res<Pack>),
    (pointer, ui_scale): (Res<Pointer>, Res<UiScale>),
    windows: Query<&Window>,
    (hero, experience, rules, tables): (
        Option<Res<crate::hero::Hero>>,
        Res<crate::fight::Experience>,
        Res<crate::fight::Rules>,
        Option<Res<Tables>>,
    ),
    mut tints: Query<(&Tint, &mut BackgroundColor)>,
) {
    if !showing(&shown, &opened) {
        return;
    }
    let held = pack.held.and_then(|held| {
        let at = on_grid(&pointer, &ui_scale, &windows)?;
        backpack::cell_in(at, (COLUMNS, ROWS))?;
        let cell = held_at((at.x, at.y), held.size);
        Some((cell, held.size, chest.grid.highlight(cell, held.size)))
    });
    let refused = |key: u32| {
        chest
            .items
            .iter()
            .find(|(k, _)| *k == key)
            .is_some_and(|(_, dropped)| {
                backpack::refuses(
                    (hero.as_deref(), &experience, &rules, tables.as_deref()),
                    dropped,
                )
            })
    };
    for (tint, mut colour) in &mut tints {
        let wanted = backpack::cell_tint(&chest.grid, held, tint.0, refused);
        colour.set_if_neq(BackgroundColor(wanted));
    }
}

/// A left click on the grid while the window shows (`0x6ACFC0`): with Shift on an item, the item
/// into the backpack where it fits; with the cursor empty, the item onto the cursor; with an item,
/// put where it fits, with its sound, or in place of the one item under it.
fn click(
    (shown, opened): (Res<Shown>, Res<Opened>),
    (buttons, keys): (Res<ButtonInput<MouseButton>>, crate::controls::Keys),
    (pointer, ui_scale): (Res<Pointer>, Res<UiScale>),
    windows: Query<&Window>,
    (mut chest, mut pack): (ResMut<Chest>, ResMut<Pack>),
    tables: Option<Res<Tables>>,
    mut sounds: MessageWriter<sfx::Play>,
) {
    if !showing(&shown, &opened) || !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(at) = on_grid(&pointer, &ui_scale, &windows) else {
        return;
    };
    let Some(cell) = backpack::cell_in(at, (COLUMNS, ROWS)) else {
        return;
    };
    let chest = &mut *chest;
    let shift = keys.pressed(crate::controls::Action::SendItem);
    if shift && pack.held.is_none() {
        // Into the backpack (`0x5DEBC0`, `0x5DECC0`): out of the store where the backpack takes it.
        let Some(placed) = chest.grid.at(cell).copied() else {
            return;
        };
        let Some(index) = chest.items.iter().position(|(key, _)| *key == placed.key) else {
            return;
        };
        if pack.take(chest.items[index].1, placed.size) {
            chest.grid.remove(placed.key);
            let (_, dropped) = chest.items.remove(index);
            debug!("town stash: {} into the backpack", dropped.item);
        }
        return;
    }
    let Some(held) = pack.held else {
        if lift_from((&mut chest.items, &mut chest.grid), &mut pack.held, cell) {
            debug!("town stash: the item at {cell:?} onto the cursor");
        }
        return;
    };
    let put = put_into(
        (&mut chest.items, &mut chest.grid),
        &mut pack.held,
        held_at((at.x, at.y), held.size),
    );
    debug!("town stash: {} put at {cell:?}: {put:?}", held.dropped.item);
    if put == Put::Placed {
        let category = tables.map_or(0, |tables| tables.category(held.dropped.item));
        let (id, rate) = crate::held::put_sound(category);
        sounds.write(sfx::Play {
            sound: sfx::Of::Id(id),
            at: None,
            rate,
        });
    }
}

/// The town stash's window.
pub struct TownStashPlugin;

impl Plugin for TownStashPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Chest>()
            .init_resource::<Opened>()
            .add_systems(
                Update,
                (show, lay_out, click, tint)
                    .chain()
                    .after(crate::inventory::Opened)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_runs_round_the_grid() {
        let mut dice = Rng::of(1, 0, 0);
        let pieces = frame(
            &mut dice,
            |_| [0.0, 4.0, 8.0, 0.0],
            |_| Vec2::new(32.0, 32.0),
        );
        assert_eq!(pieces.len(), 40);
        // The right edge: 34 nine times from 20 down, then 57, each at 640 less its anchor.
        assert_eq!(pieces[0], (34, Vec2::new(632.0, 20.0)));
        assert_eq!(pieces[9], (57, Vec2::new(632.0, 308.0)));
        // The bottom edge of 15 to 19, the top of 21.
        assert!(
            pieces[10..20]
                .iter()
                .all(|&(i, at)| (15..=19).contains(&i) && at.y == 336.0)
        );
        assert!(pieces[20..].iter().all(|&(i, at)| i == 21 && at.y == 12.0));
    }
}
