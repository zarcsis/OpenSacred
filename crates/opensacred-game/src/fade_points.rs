//! The points that the trees and the walls fade about (#2942; docs/re/drawing.md, "The trees about
//! the heroes"): as `0x62FF60` passes the creatures in its lists it adds a point for the local
//! hero, the horse it rides, its followers, a creature hostile to it with a ranged weapon in hand
//! while the hero lives, and any creature that fights, each the middle of its rectangle on the
//! screen (`0x44CF20`), up to 100; a sprite of a fading type drawn after a point fades about it
//! (`statics::Fade`, `sprites.wesl`). The game keeps the 100 nearest the hero, in the order the
//! lists draw them.

use bevy::prelude::*;
use bevy::render::storage::ShaderBuffer;
use opensacred_rules::pick;

use crate::frame::Step;
use crate::iso;
use crate::statics::{FADE_POINTS, Fade, TreeFade};

/// The point that a creature of the type `kind` standing at `at` fades the sprites about, on the
/// original's screen in its pixels at zoom 1: the middle of its rectangle there (`0x44CF20`,
/// [`pick::creature_rect`]), a ridden horse's halved about its middle.
fn point(at: Vec3, kind: u32, ridden: bool) -> Vec2 {
    let feet = Vec2::new(at.dot(iso::right()), -at.dot(iso::up())) * iso::PIXELS_PER_UNIT;
    Vec2::from(pick::creature_rect(feet.to_array(), kind, 1.0, ridden).middle())
}

/// The creatures that may fade the sprites: their place, their mind, their health, the master
/// they follow, the rider they carry, what they hold and whether they are shown.
type Creatures<'w, 's> = Query<
    'w,
    's,
    (
        &'static Transform,
        &'static crate::ai::Mind,
        &'static crate::fight::Health,
        Option<&'static crate::followers::Follower>,
        Option<&'static crate::riding::Ridden>,
        Option<&'static crate::npcs::Arms>,
        Option<&'static InheritedVisibility>,
    ),
    Without<crate::hero::Body>,
>;

/// Keeps the points in the fade's buffer (`0x62FF60`'s tests, `0x6314A4` to `0x6315EA`): the
/// hero; a horse whose rider is the hero (`0x4266F0`, `+0x1EC`); a creature that follows the hero
/// (`+0x251`, the flags `0x104` of `+0x1F4`, in the hero's list of followers); a creature hostile
/// to the hero holding a ranged weapon (`0x549000`, `0x54A2A0`), while the hero lives; and a
/// creature that fights (the AI's state 4 or 8, `+0x104`, or its other states of a fight with a
/// target, `+0x158`). Of them the 100 nearest the hero, in the order of drawing of their places.
fn gather(
    heroes: Query<(Entity, &Transform, &crate::fight::Health), With<crate::hero::Body>>,
    creatures: Creatures,
    (arts, size): (Option<Res<crate::arts::HeroArts>>, Option<Res<TreeFade>>),
    (mut fade, mut buffers): (ResMut<Fade>, ResMut<Assets<ShaderBuffer>>),
) {
    let Some(size) = size else {
        return;
    };
    let Some((hero, at, health)) = heroes.iter().next() else {
        fade.keep(*size, &[], &mut buffers);
        return;
    };
    let here = at.translation;
    let alive = health.current > 0;
    let kind = arts.as_deref().map_or(1, |arts| arts.kind);
    let mut points = vec![(0.0, here, point(here, kind, false))];
    for (at, mind, health, follower, ridden, arms, visibility) in &creatures {
        if health.current == 0 || visibility.is_some_and(|visibility| !visibility.get()) {
            continue;
        }
        let ridden_by_hero = ridden.is_some_and(|ridden| ridden.rider == hero);
        let fades = ridden_by_hero
            || follower.is_some_and(|follower| follower.master == hero)
            || (alive && mind.hostile() && arms.is_some_and(crate::npcs::Arms::ranged))
            || mind.fighting();
        if fades {
            let at = at.translation;
            points.push((
                at.distance_squared(here),
                at,
                point(at, mind.creature, ridden.is_some()),
            ));
        }
    }
    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    points.truncate(FADE_POINTS);
    let mut placed: Vec<(Vec2, Vec4)> = points
        .into_iter()
        .map(|(_, at, point)| (point, crate::models::place_of(at)))
        .collect();
    placed.sort_by(|a, b| {
        crate::statics::nearness(a.1.to_array())
            .total_cmp(&crate::statics::nearness(b.1.to_array()))
    });
    fade.keep(*size, &placed, &mut buffers);
}

/// The points that the sprites fade about.
pub struct FadePointsPlugin;

impl Plugin for FadePointsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, gather.in_set(Step::Models));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heros_point_is_the_middle_of_its_rectangle() {
        // The heroes' size (20, 170, 0): its box from 146 pixels above its feet to 24 below.
        let feet = iso::ground(30.5, 20.5);
        let at = point(feet, 1, false);
        let on_screen =
            Vec2::new(feet.dot(iso::right()), -feet.dot(iso::up())) * iso::PIXELS_PER_UNIT;
        assert!((at.x - on_screen.x).abs() < 1e-3, "{at} {on_screen}");
        assert!((on_screen.y - at.y - 60.8).abs() < 0.1, "{at} {on_screen}");
        // A ridden horse's box halved about its middle keeps the middle.
        assert!(point(feet, 1, true).distance(at) < 1e-3);
    }
}
