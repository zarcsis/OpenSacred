//! The doors that the hero opens and closes (#372; docs/re/fight.md, "Doors in the way"). The
//! client's left click sends the hero the event 7 with the object under the cursor (`0x6172C0`),
//! whose AI's state 1 (`0x53A170`) walks it to a door and, standing within 300 of it, sends the
//! door the event `0x21` (`0x44BCD0`): a closed door opens, an open one closes, its trigger's
//! cells with it, and the door keeps the hero's mark, so that no townsperson closes it again.
//!
//! The game picks the door nearest the cursor's line of sight as it picks a creature
//! (`attack::from_line`); the hero walks where the click sends it, beside the door's closed cells
//! (`hero::steer`), rather than to the door's outer point, which #273 has yet to give, and turns
//! the door once it stands within 300 of it.

use bevy::prelude::*;

use crate::attack::{self, Aim};
use crate::frame::{Control, Step, Unpaused};
use crate::hero::{self, Body};
use crate::hud::Pointer;
use crate::npcs::{Door, Object, Swing};

/// How near a door the hero turns it, standing, in world units (`0x53A170`).
const TURNS_WITHIN: f32 = 300.0;

/// The door that the hero was sent to by a click, which it turns as it stands near.
#[derive(Component)]
pub struct Opens(Entity);

/// On a press of the left button that went to no attack, the door whose model is nearest the
/// cursor's line of sight, if one is near it, becomes the hero's to turn; any press ends the last
/// such order.
pub(crate) fn pick(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    pointer: Res<Pointer>,
    aim: Res<Aim>,
    camera: Single<(&Camera, &GlobalTransform)>,
    doors: Query<(Entity, &GlobalTransform), With<Door>>,
    heroes: Query<Entity, With<Body>>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    for hero in &heroes {
        commands.entity(hero).remove::<Opens>();
    }
    let Some(cursor) = pointer.0 else {
        return;
    };
    let (camera, view) = *camera;
    let (false, Ok(ray)) = (aim.0, camera.viewport_to_world(view, cursor)) else {
        return;
    };
    let Some(door) = doors
        .iter()
        .map(|(door, at)| (door, attack::from_line(ray, at.translation())))
        .filter(|&(_, distance)| distance <= attack::PICKED_WITHIN)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(door, _)| door)
    else {
        return;
    };
    for hero in &heroes {
        debug!("doors: the hero goes to turn {door}");
        commands.entity(hero).insert(Opens(door));
    }
}

/// The hero that stands, once its way to a door it was sent to ends: within 300 of the door's
/// logical position (`npcs::Object`) it turns it, as a hero, else it gives up.
fn turn(
    mut commands: Commands,
    mut doors: Query<(&Object, &mut Door)>,
    heroes: Query<(Entity, &Transform, &Body, &Opens)>,
) {
    for (hero, transform, body, opens) in &heroes {
        if body.target().is_some() {
            continue;
        }
        commands.entity(hero).remove::<Opens>();
        let Ok((at, door)) = doors.get_mut(opens.0) else {
            continue;
        };
        if turns(transform.translation, at.at) {
            debug!("doors: the hero turns {}", opens.0);
            commands.entity(opens.0).insert(Swing::by_hero(!door.open));
        }
    }
}

/// Whether a hero standing at `hero` turns the door at `door`: within [`TURNS_WITHIN`] of it on
/// the ground.
fn turns(hero: Vec3, door: Vec3) -> bool {
    (door - hero).xz().length() <= TURNS_WITHIN
}

/// The doors that the hero opens and closes.
pub struct DoorsPlugin;

impl Plugin for DoorsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                pick.in_set(Control).after(attack::aim).before(hero::steer),
                turn.after(hero::run),
            )
                .in_set(Unpaused)
                .in_set(Step::HeroMove),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hero_turns_a_door_it_stands_within_300_of() {
        let door = Vec3::new(1000.0, 0.0, 1000.0);
        assert!(turns(door + Vec3::new(200.0, 50.0, 200.0), door));
        assert!(turns(door + Vec3::new(300.0, 0.0, 0.0), door));
        assert!(!turns(door + Vec3::new(250.0, 0.0, 250.0), door));
    }
}
