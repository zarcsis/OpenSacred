//! The tooltips of the bar and the portraits' column (#1681, a part of #1594; docs/re/interface.md,
//! "The tooltip"), each of the style `0x209` (`tooltip::spawn_tip_in`), as the pointer rests a
//! third of a second on its control: a button of the bar its text centred 20 above its top's middle
//! (`0x6E4560`), and a belt's slot its kind's text and key alike (#1684); a portrait's ring or picture the text 9960 with the creature's health and its
//! most, four wide each, centred 64 left of the ring's corner; the hero's experience bar the text
//! 9961 with the experience into the level and the level's span, six wide each, centred 24 left of
//! the bar's corner (`0x6D7B80`).
//!
//! Not followed yet (#1594): an art slot's description, the bonuses' "+", the horse's button and
//! the mount's bar, and the disc's and the compass's texts; the weapon sets' items' tooltips are
//! #1516's.

use bevy::prelude::*;

use crate::bar::{self, UiImages};
use crate::fight::{Experience, Health};
use crate::game::AppState;
use crate::hero::Body;
use crate::hud::Pointer;
use crate::inventory::screen_point;
use crate::lines::Texts;
use crate::tooltip::{Frame, Rest, TipRoot, spawn_tip_in};

/// The bar's buttons' texts, in the order of `bar::BUTTONS`: Inventory, Options, Map, Book and
/// Collect.
const BUTTON_TEXTS: [&str; 5] = ["9907", "9908", "9910", "9911", "9909"];
/// The belt's slots' texts, the slots 1 to 5 (red, purple, blue, green and yellow), each its
/// potion's name and its key (the names at `0x94C880`).
const BELT_TEXTS: [&str; 5] = [
    "POTION_RED_HINT",
    "POTION_BLACK_HINT",
    "POTION_BLUE_HINT",
    "POTION_GREEN_HINT",
    "POTION_YELLOW_HINT",
];
/// A button's or a belt slot's tooltip's centre above its top's middle.
const ABOVE: f32 = 20.0;
/// The health's text and its tooltip's centre left of the ring's corner; the experience's text and
/// its tooltip's centre left of the bar's corner.
const HEALTH: &str = "9960";
const HEALTH_LEFT: f32 = 64.0;
const EXPERIENCE: &str = "9961";
const EXPERIENCE_LEFT: f32 = 24.0;

/// A control whose tooltip the pointer rests on: a button of the bar by its index, a belt's slot,
/// 1 to 5, a creature's health by its entry in the column, or the hero's experience.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Control {
    Button(usize),
    Belt(u8),
    Health(usize),
    Experience,
}

impl Control {
    /// The control as a number for the rest's timer.
    fn number(self) -> u32 {
        let index = |i: usize| u32::try_from(i).unwrap_or(0);
        match self {
            Self::Button(i) => index(i),
            Self::Belt(slot) => 8 + u32::from(slot),
            Self::Health(k) => 16 + index(k),
            Self::Experience => 32,
        }
    }
}

/// The control of the bar under the point `at` of the original's screen, the bar's places centred
/// along the window: a button, else a belt's slot (`0x6E4560`'s order).
fn bar_control(images: &UiImages, at: Vec2) -> Option<Control> {
    bar::BUTTONS
        .iter()
        .position(|&(index, corner)| {
            Rect::from_corners(corner, corner + images.size(index)).contains(at)
        })
        .map(Control::Button)
        .or_else(|| bar::belt_slot(at).map(Control::Belt))
}

/// The control of the portraits' column under the point `at` of the original's screen, the
/// column's places at the window's right, of `entries` shown (the hero's first): the hero's
/// experience bar, else an entry's ring or picture (`0x6D7B80`'s order).
fn column_control(images: &UiImages, at: Vec2, entries: usize) -> Option<Control> {
    let (frame, corner) = bar::EXPERIENCE_FRAME;
    if Rect::from_corners(corner, corner + images.size(frame)).contains(at) {
        return Some(Control::Experience);
    }
    (0..entries)
        .find(|&k| {
            let step = Vec2::new(
                0.0,
                bar::ENTRY_STEP * f32::from(u8::try_from(k).unwrap_or(0)),
            );
            let ring =
                Rect::from_corners(bar::RING_AT + step, bar::RING_AT + step + bar::RING_SIZE);
            let picture = Rect::from_corners(
                bar::PORTRAIT_AT + step,
                bar::PORTRAIT_AT + step + bar::PORTRAIT_SIZE,
            );
            ring.contains(at) || picture.contains(at)
        })
        .map(Control::Health)
}

/// A text of `texts` by its name, the name where it has none.
fn named<'a>(texts: Option<&'a Texts>, name: &'a str) -> &'a str {
    texts.and_then(|texts| texts.named(name)).unwrap_or(name)
}

/// The health's line (`"%s: %4d/%4d"`, `0x9E1620`).
fn health_line(text: &str, health: &Health) -> String {
    format!("{text}: {:4}/{:4}", health.current, health.most)
}

/// The experience's line (`"%s: %6d/%6d"`, `0x9E1638`): the experience into the level and the
/// level's span.
fn experience_line(text: &str, experience: &opensacred_rules::fight::Experience) -> String {
    let level = experience.level;
    let (start, end) = (
        opensacred_rules::fight::level_start(level),
        opensacred_rules::fight::level_end(level),
    );
    format!(
        "{text}: {:6}/{:6}",
        experience.points.saturating_sub(start),
        end.saturating_sub(start)
    )
}

/// Shows the tooltip of the control that the pointer rests on, and takes it away as it leaves.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments, grouped"
)]
fn tips(
    mut commands: Commands,
    time: Res<Time<Real>>,
    (pointer, ui_scale, windows): (Res<Pointer>, Res<UiScale>, Query<&Window>),
    (images, assets, texts): (Res<UiImages>, Res<AssetServer>, Option<Res<Texts>>),
    (entries, experience): (Option<Res<bar::Entries>>, Res<Experience>),
    bars: Query<(), With<bar::BarRoot>>,
    (heroes, healths): (Query<&Health, With<Body>>, Query<&Health>),
    tips: Query<Entity, With<TipRoot>>,
    mut rest: Local<Rest>,
    mut showing: Local<Option<Control>>,
) {
    let entries = entries.as_deref().map_or(&[][..], |entries| &entries.0[..]);
    let over = if bars.is_empty() {
        None
    } else {
        screen_point(&pointer, &ui_scale, &windows)
            .and_then(|at| bar_control(&images, at))
            .or_else(|| {
                let at = bar::column_point(&pointer, &ui_scale, &windows)?;
                column_control(&images, at, entries.len() + 1)
            })
    };
    let (gone, due) = rest.step(over.map(Control::number), time.elapsed_secs());
    if gone && showing.take().is_some() {
        for tip in &tips {
            commands.entity(tip).despawn();
        }
    }
    let (Some(control), Some(_)) = (over, due) else {
        return;
    };
    let texts = texts.as_deref();
    let (text, centre, frame) = match control {
        Control::Button(i) => {
            let (index, corner) = bar::BUTTONS[i];
            let size = images.size(index);
            (
                named(texts, BUTTON_TEXTS[i]).to_owned(),
                Vec2::new(corner.x + size.x / 2.0, corner.y - ABOVE),
                Frame::Screen,
            )
        }
        Control::Belt(slot) => {
            let index = usize::from(slot.saturating_sub(1));
            let corner = bar::BELT[index.min(bar::BELT.len() - 1)];
            (
                named(texts, BELT_TEXTS[index.min(BELT_TEXTS.len() - 1)]).to_owned(),
                Vec2::new(corner.x + bar::BELT_SIDE / 2.0, corner.y - ABOVE),
                Frame::Screen,
            )
        }
        Control::Health(k) => {
            let health = match k {
                0 => heroes.iter().next(),
                k => entries
                    .get(k - 1)
                    .and_then(|&entity| healths.get(entity).ok()),
            };
            let Some(health) = health else {
                return;
            };
            let step = bar::ENTRY_STEP * f32::from(u8::try_from(k).unwrap_or(0));
            (
                health_line(named(texts, HEALTH), health),
                bar::RING_AT + Vec2::new(-HEALTH_LEFT, step),
                Frame::Column,
            )
        }
        Control::Experience => {
            let (_, corner) = bar::EXPERIENCE_FRAME;
            (
                experience_line(named(texts, EXPERIENCE), &experience.0),
                corner - Vec2::new(EXPERIENCE_LEFT, 0.0),
                Frame::Column,
            )
        }
    };
    spawn_tip_in(&mut commands, &assets, (&text, centre), frame);
    *showing = Some(control);
}

/// The tooltips of the bar and the portraits' column.
pub struct BarTipsPlugin;

impl Plugin for BarTipsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, tips.run_if(in_state(AppState::InGame)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lines_write_their_numbers_as_wide_as_the_original() {
        let health = Health {
            current: 119,
            most: 119,
            level: 1,
        };
        assert_eq!(health_line("Health", &health), "Health:  119/ 119");
        let experience = opensacred_rules::fight::Experience::default();
        let line = experience_line("Experience", &experience);
        assert!(line.starts_with("Experience:      0/"), "{line}");
    }

    #[test]
    fn a_belts_slot_has_its_tooltip_after_the_buttons() {
        let images = UiImages::default();
        assert_eq!(
            bar_control(&images, Vec2::new(450.0, 680.0)),
            Some(Control::Belt(1))
        );
        assert_eq!(
            bar_control(&images, Vec2::new(575.0, 680.0)),
            Some(Control::Belt(5))
        );
        assert_eq!(bar_control(&images, Vec2::new(300.0, 680.0)), None);
    }

    #[test]
    fn a_portraits_ring_and_picture_go_before_the_entries_below() {
        let images = UiImages::default();
        // Without the images' table the experience bar has no size; the ring is the column's.
        assert_eq!(
            column_control(&images, Vec2::new(940.0, 20.0), 1),
            Some(Control::Health(0))
        );
        assert_eq!(
            column_control(&images, Vec2::new(940.0, 160.0), 2),
            Some(Control::Health(1))
        );
        assert_eq!(column_control(&images, Vec2::new(940.0, 160.0), 1), None);
    }
}
