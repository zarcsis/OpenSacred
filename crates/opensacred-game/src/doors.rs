//! The doors that the hero opens and closes (#372; docs/re/fight.md, "Doors in the way"). The
//! client's left click sends the hero the event 7 with the object under the cursor (`0x6172C0`),
//! whose AI's state 1 (`0x53A170`) walks it to a door and, standing within 300 of it, sends the
//! door the event `0x21` (`0x44BCD0`): a closed door opens, an open one closes, its trigger's
//! cells with it, and the door keeps the hero's mark, so that no townsperson closes it again.
//!
//! The door is the one that the cursor picks by its model's rectangle (`under_cursor`, #2873); the
//! hero walks by a way to the door's point on its own side, as a creature does
//! (`opensacred_rules::ways::DoorPoints`, `ai::door_point`, #3831), in place of the click's
//! ground, which over a door's tall model lies in the room behind it, and turns the door once it
//! stands within 300 of it.

use bevy::prelude::*;

use crate::attack::{self, Aim};
use crate::frame::{Control, Step, Unpaused};
use crate::hero::{self, Body};
use crate::hud::Pointer;
use crate::npcs::{Door, Object, Swing};

/// How near a door the hero turns it, standing, in world units (`0x53A170`).
const TURNS_WITHIN: f32 = 300.0;

/// How near the door's point the hero stands rather than walks, along each axis, in world units
/// (`0x53A170`).
const STANDS_WITHIN: f32 = 25.0;

/// The door that the hero was sent to by a click, which it turns as it stands near.
#[derive(Component)]
pub struct Opens(Entity);

/// The hero's walk to open a door as a save keeps it (#3763): the door by its name.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
// Named apart from the containers' of its name, which the saves' layout test traces by name
// (#2306); postcard writes no names.
#[serde(rename = "SavedDoorOpens")]
pub struct SavedOpens {
    #[serde(with = "crate::names::required")]
    target: Entity,
}

impl bevy::ecs::entity::MapEntities for SavedOpens {
    fn map_entities<E: bevy::ecs::entity::EntityMapper>(&mut self, mapper: &mut E) {
        self.target = mapper.get_mapped(self.target);
    }
}

impl crate::doings::Saves for Opens {
    type Saved = SavedOpens;

    fn saved(&self, _: &crate::doings::Moment) -> Option<SavedOpens> {
        Some(SavedOpens { target: self.0 })
    }

    fn again(saved: SavedOpens, _: &crate::doings::Moment) -> Option<Self> {
        Some(Self(crate::names::found(Some(saved.target))?))
    }
}

/// On a press of the left button that went to no attack, the door that the cursor picks
/// (`under_cursor`) becomes the hero's to turn, and the hero goes by a way to the door's point
/// on its own side on its level, its outer point on the ground and its inner on a building's
/// floor (`0x53A170`, `0x43B290`; docs/re/fight.md, "The hero's click"), or stands where it is
/// within 25 of it along each axis, in place of the click's ground (`hero::steer`); any press
/// ends the last such order.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system takes its resources and queries as arguments"
)]
pub(crate) fn pick(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    (pointer, under): (Res<Pointer>, Res<crate::under_cursor::UnderCursor>),
    aim: Res<Aim>,
    doors: Query<(&Door, &Object)>,
    mut heroes: Query<(Entity, &Transform, &mut Body)>,
    npcs: Option<Res<crate::npcs::Npcs>>,
    heights: Option<Res<crate::terrain::Heights>>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    for (hero, ..) in &heroes {
        commands.entity(hero).remove::<Opens>();
    }
    if pointer.0.is_none() || aim.0 {
        return;
    }
    let Some((entity, door, object)) = under
        .0
        .and_then(|entity| doors.get(entity).ok().map(|(door, at)| (entity, door, at)))
    else {
        return;
    };
    let (Some(npcs), Some(heights)) = (npcs, heights) else {
        return;
    };
    let points = crate::ai::door_points(door, npcs.door_cell(door), &heights);
    for (hero, transform, mut body) in &mut heroes {
        let here = transform.translation;
        let level = body.level();
        let point = crate::ai::door_point(points, object.at, here, level, &heights);
        debug!("doors: the hero goes to turn {entity} from {point}");
        if stands(here, point) {
            body.stop();
        } else {
            body.send_onto(point, level);
        }
        commands.entity(hero).insert(Opens(entity));
    }
}

/// Whether a hero at `hero` stands rather than walks to a door's point at `point`: within
/// [`STANDS_WITHIN`] of it along each axis (`0x53A170`).
fn stands(hero: Vec3, point: Vec3) -> bool {
    let off = (point - hero).abs();
    off.x <= STANDS_WITHIN && off.z <= STANDS_WITHIN
}

/// The hero that stands, once its way to a door it was sent to ends: within 300 of the door's
/// logical position (`npcs::Object`) it turns it, as a hero, else it gives up (`0x53BE8C`).
fn turn(
    mut commands: Commands,
    mut doors: Query<(&Object, &mut Door)>,
    heroes: Query<(Entity, &Transform, &Body, &Opens)>,
) {
    for (hero, transform, body, opens) in &heroes {
        // Its walk there, ordered for the world's next tick or begun (`hero::Body`).
        if body.target().is_some() || body.ordered() {
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
                pick.in_set(Control)
                    .in_set(crate::frame::OnWorld)
                    .after(attack::aim)
                    .after(hero::steer),
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

    #[test]
    fn the_hero_stands_within_25_of_the_doors_point_along_each_axis() {
        let point = Vec3::new(1000.0, 0.0, 1000.0);
        assert!(stands(point + Vec3::new(25.0, 40.0, -25.0), point));
        assert!(stands(point + Vec3::new(-24.0, 0.0, 7.0), point));
        assert!(!stands(point + Vec3::new(26.0, 0.0, 0.0), point));
        assert!(!stands(point + Vec3::new(0.0, 0.0, -30.0), point));
    }
}
