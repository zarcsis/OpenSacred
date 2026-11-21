//! The iron frame of the merchant's and the combat master's windows (#2940; docs/re/trade.md, "The
//! merchant's window (`cUI_Merchant`)"): their layouts' pieces (`0x6DC010`, `0x6D1460`) over the
//! window of 640 × 336 at the screen's corner, each placed by its image's anchors as the original
//! places them: down the panel's left side ten of the image 34 and down its right nine more and
//! the corner 57, along its top the images 54, 21, 21 and 55, along the window's foot ten of the
//! images 15 to 19, a roll each once a game, and over the pages' tabs sixteen of the image 21.

use bevy::prelude::*;
use opensacred_rules::spawning::Rng;

use crate::bar::{self, UiImages};
use crate::game::AppState;

/// The window's size.
const WINDOW: Vec2 = Vec2::new(640.0, 336.0);
/// The side's bar, its corner at the foot of the right side, the top's two ends and its middle,
/// the foot's first image of five, the pieces along each side and along the foot, and the bars
/// over the tabs.
const SIDE: u32 = 34;
const CORNER: u32 = 57;
const TOP: [u32; 4] = [54, 21, 21, 55];
const FOOT_FIRST: u32 = 15;
const FOOT_KINDS: usize = 5;
const SIDES: usize = 10;
const FOOT: usize = 10;
const TABS_BAR: u32 = 21;
const TABS_BARS: usize = 16;

/// The foot's images, rolled once a game (`0x6DC3A0`: `15 + ⌊5·r⌋`).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Foot([u32; FOOT]);

impl Default for Foot {
    fn default() -> Self {
        Self([FOOT_FIRST; FOOT])
    }
}

impl Foot {
    /// The foot's images of the rolls of `dice`.
    fn rolled(dice: &mut Rng) -> Self {
        let kinds = u32::try_from(FOOT_KINDS).unwrap_or(1);
        Self(std::array::from_fn(|_| FOOT_FIRST + dice.below(kinds)))
    }
}

/// The frame's pieces, each an image and its corner on the window, the window's foot `foot`, as
/// the layout places them by `anchors` (an image's `x0`, `y0`, `x1`, `y1`) and `size`: the left
/// side's bars at 510 plus their `x1` from 22 down, the right's at 640 less theirs from 20 down,
/// each the next lower by the right one's height; the top's from 511 along at 18 less their `y0`;
/// the foot's from 0 along at 340 less theirs; the tabs' bars from 0 along at 48 less theirs.
fn pieces(
    anchors: &dyn Fn(u32) -> [f32; 4],
    size: &dyn Fn(u32) -> Vec2,
    foot: &Foot,
) -> Vec<(u32, Vec2)> {
    let mut pieces = Vec::new();
    let mut y = 8.0;
    for side in 0..SIDES {
        pieces.push((SIDE, Vec2::new(anchors(SIDE)[2] + 510.0, y + 14.0)));
        let right = if side + 1 == SIDES { CORNER } else { SIDE };
        pieces.push((right, Vec2::new(WINDOW.x - anchors(right)[2], y + 12.0)));
        y += size(right).y;
    }
    let mut x = 511.0;
    for image in TOP {
        pieces.push((image, Vec2::new(x, 18.0 - anchors(image)[1])));
        x += size(image).x;
    }
    let mut x = 0.0;
    for &image in &foot.0 {
        pieces.push((image, Vec2::new(x, WINDOW.y - anchors(image)[1] + 4.0)));
        x += size(image).x;
    }
    let mut x = 0.0;
    for _ in 0..TABS_BARS {
        pieces.push((TABS_BAR, Vec2::new(x, 48.0 - anchors(TABS_BAR)[1])));
        x += size(TABS_BAR).x;
    }
    pieces
}

/// Spawns the frame's pieces into `parent`, the window's corner the screen's.
pub(crate) fn spawn(
    commands: &mut Commands,
    parent: Entity,
    (images, assets): (&UiImages, &AssetServer),
    foot: &Foot,
) {
    for (image, at) in pieces(&|i| images.anchors(i), &|i| images.size(i), foot) {
        let piece = commands
            .spawn((
                bar::piece(images, assets, image, at, Vec2::ZERO),
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(parent).add_child(piece);
    }
}

/// A new game's foot, of the game's seed.
fn roll(mut commands: Commands, game: Option<Res<crate::seed::Game>>) {
    let seed = game.map_or_else(crate::seed::Game::default, |game| *game);
    commands.insert_resource(Foot::rolled(&mut seed.dice(2940)));
}

/// The trade windows' frame.
pub struct TradeFramePlugin;

impl Plugin for TradeFramePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Foot>()
            .add_systems(OnEnter(AppState::InGame), roll);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pieces_lie_where_the_layout_puts_them() {
        // The images' anchors and sizes as the installation's table holds them.
        let anchors = |image: u32| match image {
            15 | 18 => [0.0, 4.0, 0.0, 0.0],
            16 | 17 | 19 => [0.0, 8.0, 0.0, 0.0],
            21 => [0.0, 1.0, 0.0, 0.0],
            34 => [0.0, 0.0, 1.0, 0.0],
            54 => [37.0, 7.0, 1.0, 19.0],
            55 => [0.0, 8.0, 0.0, 0.0],
            57 => [0.0, 32.0, 33.0, 0.0],
            _ => [0.0; 4],
        };
        let size = |image: u32| match image {
            15..=19 => Vec2::new(64.0, 20.0),
            21 => Vec2::new(32.0, 6.0),
            34 => Vec2::new(6.0, 32.0),
            54 | 55 => Vec2::new(38.0, 20.0),
            57 => Vec2::new(38.0, 37.0),
            _ => Vec2::ZERO,
        };
        let foot = Foot([15, 16, 17, 18, 19, 15, 16, 17, 18, 19]);
        let pieces = pieces(&anchors, &size, &foot);
        assert_eq!(pieces.len(), 2 * SIDES + TOP.len() + FOOT + TABS_BARS);
        // The sides: the left's first at (511, 22), the right's at (639, 20), the corner last.
        assert_eq!(pieces[0], (34, Vec2::new(511.0, 22.0)));
        assert_eq!(pieces[1], (34, Vec2::new(639.0, 20.0)));
        assert_eq!(pieces[18], (34, Vec2::new(511.0, 310.0)));
        assert_eq!(pieces[19], (57, Vec2::new(607.0, 308.0)));
        // The top: 54, 21, 21, 55 from 511 along.
        assert_eq!(
            pieces[20..24],
            [
                (54, Vec2::new(511.0, 11.0)),
                (21, Vec2::new(549.0, 17.0)),
                (21, Vec2::new(581.0, 17.0)),
                (55, Vec2::new(613.0, 10.0)),
            ]
        );
        // The foot: each at 340 less its `y0`.
        assert_eq!(pieces[24], (15, Vec2::new(0.0, 336.0)));
        assert_eq!(pieces[25], (16, Vec2::new(64.0, 332.0)));
        // The tabs' bars at 47, 32 apart.
        assert_eq!(pieces[34], (21, Vec2::new(0.0, 47.0)));
        assert_eq!(pieces[49], (21, Vec2::new(480.0, 47.0)));
    }

    #[test]
    fn the_foot_rolls_the_five_images() {
        let foot = Foot::rolled(&mut Rng::of(1, 2, 3));
        assert!(foot.0.iter().all(|image| (15..=19).contains(image)));
    }
}
