//! The purchase window (#1619; docs/re/trade.md, "The purchase window"): `UI_WND_PURCHASE`
//! (`cUI_Purchase`, `0x6DCDD0`), which asks the hero to buy or to sell an item at a merchant's,
//! or tells it that it cannot pay or carry one (`0x6DCED0`): a dark panel with the tooltip's
//! border, at the screen's `(320 − w/2, 114)`, as wide as its text and 32 and 92 high; its text on
//! two lines, the question and the price (`%s: %d %s` of the texts 1055 and 1004), and its buttons, Yes
//! and No (1034, 1035) at a quarter and three quarters of its width, or OK (1028) alone in the
//! middle for what it tells. While it shows it takes the interface's input to itself (`0x6DD450`):
//! the world's clicks are held back, and the windows' clicks wait for its answer, which goes to the
//! window that asked (`0x6DD4A0`, `0x6DD700`).
//!
//! Not followed: the buttons' click sound (`0x7325C0(5)`) and their looks under the pointer, and
//! the blacksmith's and the master's questions.

use bevy::prelude::*;

use crate::bar::{self, UiImages};
use crate::frame::HoldsControl;
use crate::game::{AppState, FONT};
use crate::hud::Pointer;
use crate::lines::Texts;
use crate::tooltip::{PANEL, WHITE, argb};

/// The window's centre along the screen, its top, its height, and the room about its text
/// (`0x6DD0F8`).
const CENTRE: f32 = 320.0;
const TOP: f32 = 114.0;
const HEIGHT: f32 = 92.0;
const ROOM: f32 = 32.0;
/// The text's top within the window, its lines' height and the buttons' line (**seen**: the
/// lines at 12 and 32 below the top, 16 in, the buttons at 61).
const TEXT_TOP: f32 = 12.0;
const LINE: f32 = 20.0;
const BUTTONS_AT: f32 = 58.0;
const BUTTON: Vec2 = Vec2::new(60.0, 24.0);
const FONT_SIZE: f32 = 20.0;
/// The texts: the price's word and the gold's, and the buttons'.
const PRICE: &str = "1055";
const GOLD: &str = "1004";
const YES: &str = "1034";
const NO: &str = "1035";
const OK: &str = "1028";

/// What the window asks or tells (its kind, `+0x154`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
    /// Whether to buy an item, at its price (1, the text 1000).
    Buy,
    /// That the hero's gold is short of its price (2, the text 1002).
    Poor,
    /// Whether to sell an item, at its price (3, the text 1001).
    Sell,
    /// That the item fits nowhere in the backpack (4, the text 1190).
    Full,
}

impl Ask {
    /// Its first line's text.
    fn text(self) -> &'static str {
        match self {
            Self::Buy => "1000",
            Self::Poor => "1002",
            Self::Sell => "1001",
            Self::Full => "1190",
        }
    }

    /// Whether it asks, with Yes and No, rather than tells, with OK.
    fn asks(self) -> bool {
        matches!(self, Self::Buy | Self::Sell)
    }
}

/// The window that shows: what it asks or tells, and the price.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Asking {
    pub ask: Ask,
    pub price: i64,
}

/// The window's answer to the window that asked: what it asked, and whether Yes (OK's is No).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Answered {
    pub ask: Ask,
    pub yes: bool,
}

/// The window's container.
#[derive(Component)]
struct Root;

/// A button of the window, and whether it answers Yes.
#[derive(Component)]
struct Button(bool);

/// Makes the window as it shows, and takes it away as it goes: centred on 320 of the screen, as
/// wide as its text and 32 by its room on either side.
fn show(
    mut commands: Commands,
    asking: Option<Res<Asking>>,
    (images, assets, texts): (Res<UiImages>, Res<AssetServer>, Option<Res<Texts>>),
    roots: Query<Entity, With<Root>>,
) {
    let changed = asking.as_ref().is_some_and(|asking| asking.is_changed());
    if !changed && (asking.is_some() || roots.is_empty()) {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    let Some(asking) = asking else {
        return;
    };
    let said = |key: &str| {
        texts
            .as_deref()
            .and_then(|texts| texts.named(key))
            .unwrap_or(key)
            .to_owned()
    };
    let mut lines = vec![said(asking.ask.text())];
    if asking.ask.asks() {
        lines.push(format!("{}: {} {}", said(PRICE), asking.price, said(GOLD)));
    }
    let font: Handle<Font> = assets.load(FONT);
    let root = commands
        .spawn((Root, crate::backpack::screen(), GlobalZIndex(3)))
        .id();
    // A row across twice 320, which centres the panel on 320.
    let mut row = bar::placed(Vec2::new(0.0, TOP), Vec2::new(2.0 * CENTRE, HEIGHT));
    row.justify_content = JustifyContent::Center;
    let row = commands.spawn((row, Pickable::IGNORE)).id();
    commands.entity(root).add_child(row);
    let panel = commands
        .spawn((
            Node {
                height: Val::Px(HEIGHT),
                padding: UiRect {
                    left: Val::Px(ROOM / 2.0),
                    right: Val::Px(ROOM / 2.0),
                    top: Val::Px(TEXT_TOP),
                    bottom: Val::Px(0.0),
                },
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(PANEL),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(row).add_child(panel);
    crate::tooltip::add_border(&mut commands, (&images, &assets), panel);
    let words = |text: String| {
        (
            Text::new(text),
            TextFont {
                font: font.clone().into(),
                font_size: FontSize::Px(FONT_SIZE),
                ..default()
            },
            TextLayout {
                linebreak: LineBreak::NoWrap,
                ..default()
            },
            TextColor(argb(WHITE)),
            Pickable::IGNORE,
        )
    };
    for line in lines {
        let child = commands
            .spawn((
                Node {
                    height: Val::Px(LINE),
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_child(words(line))
            .id();
        commands.entity(panel).add_child(child);
    }
    // The buttons at a quarter and three quarters of the width, or one in the middle.
    let buttons = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(BUTTONS_AT),
                height: Val::Px(BUTTON.y),
                justify_content: JustifyContent::SpaceAround,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(panel).add_child(buttons);
    let answers: Vec<(String, bool)> = if asking.ask.asks() {
        vec![(said(YES), true), (said(NO), false)]
    } else {
        vec![(said(OK), false)]
    };
    for (text, yes) in answers {
        let button = commands
            .spawn((
                Button(yes),
                Node {
                    width: Val::Px(BUTTON.x),
                    height: Val::Px(BUTTON.y),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .with_child(words(text))
            .id();
        commands.entity(buttons).add_child(button);
    }
}

/// A left press on a button answers (`0x6DD970`); a press elsewhere does nothing.
fn press(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    asking: Option<Res<Asking>>,
    (pointer, windows): (Res<Pointer>, Query<&Window>),
    nodes: Query<(&Button, &ComputedNode, &UiGlobalTransform)>,
    mut answers: MessageWriter<Answered>,
) {
    let Some(asking) = asking else {
        return;
    };
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let (Some(at), Ok(window)) = (pointer.0, windows.single()) else {
        return;
    };
    let point = at * window.scale_factor();
    let Some(yes) = nodes
        .iter()
        .find(|(_, node, transform)| node.contains_point(**transform, point))
        .map(|(button, ..)| button.0)
    else {
        return;
    };
    debug!("purchase: {:?} answered {yes}", asking.ask);
    answers.write(Answered {
        ask: asking.ask,
        yes,
    });
    commands.remove_resource::<Asking>();
}

/// Whether no question waits, so that the player's keys and mouse go to the world.
fn answered(asking: Option<Res<Asking>>) -> bool {
    asking.is_none()
}

/// Takes the window away as a game ends.
fn forget(mut commands: Commands) {
    commands.remove_resource::<Asking>();
}

/// The purchase window.
pub struct PurchasePlugin;

impl Plugin for PurchasePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Answered>()
            .hold_control(answered)
            .add_systems(OnExit(AppState::InGame), forget)
            .add_systems(
                Update,
                (press, show)
                    .chain()
                    .before(crate::inventory::Opened)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_asks_to_buy_or_sell_and_tells_the_rest() {
        assert!(Ask::Buy.asks() && Ask::Sell.asks());
        assert!(!Ask::Poor.asks() && !Ask::Full.asks());
        assert_eq!(
            [Ask::Buy, Ask::Poor, Ask::Sell, Ask::Full].map(Ask::text),
            ["1000", "1002", "1001", "1190"]
        );
    }
}
