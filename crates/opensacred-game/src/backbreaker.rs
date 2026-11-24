//! The Gladiator's backbreaker 1010 carries its target and throws it down (#3127; docs/re/arts.md,
//! "The Gladiator's"): as `making`'s arm takes the target up it hangs fastened to his `Bip01 Neck`,
//! else his `Bip01 R Hand`, its model's root on the bone's place and orientation as the client's
//! attach fastens a model to a bone (`0x44C670`), playing its first hit clip, the slot 175, once
//! at the rate 1.0 (`0x502EFC`–`0x50317B`); as the arm lets it go it is put at his right hand's
//! point, its heading back, plays its dying clip, the slot 173, once at the rate 1.0, and flies
//! the throw's flight (`arts::backbreaker_throw`, `0x5014C7`–`0x501769`) until the ground takes
//! it. A taker who makes the art no more, or is gone, drops it where it hangs onto the ground.

use bevy::prelude::*;
use opensacred_rules::arts;

use crate::crowd::InTheWay;
use crate::frame::{Step, Unpaused};
use crate::game::AppState;
use crate::making::{Playing, Worn};
use crate::npcs::ScriptClip;
use crate::terrain::{Heights, Level};

/// The bones the target hangs at, the first its taker's model has (`0x44C670`).
const CARRIED_AT: [&str; 2] = ["Bip01 Neck", "Bip01 R Hand"];
/// The bone whose point the target is let go at (`0x44CA30`).
pub(crate) const LET_GO_AT: [&str; 1] = ["Bip01 R Hand"];
/// The target's first hit clip (`HIT_A`), which it plays as it hangs (`0x50317B`).
const HANGS: u16 = 175;
/// The target's dying clip (`*_DYING_A`), which it plays as it is thrown (`0x5016D2`).
const THROWN: u16 = 173;

/// A creature that the backbreaker has taken up: its taker, and its heading as it was taken up,
/// which it takes back as it is let go.
#[derive(Component, Clone, Copy, Debug)]
struct TakenUp {
    by: Entity,
    rotation: Quat,
}

/// A creature that the backbreaker has let go, in its flight.
#[derive(Component, Clone, Copy, Debug)]
struct Flung(arts::Flight);

/// Takes the creature `target` up into the hands of `by`, its heading `rotation`: it hangs at
/// his neck, its hit clip playing.
pub(crate) fn take_up(commands: &mut Commands, target: Entity, by: Entity, rotation: Quat) {
    commands
        .entity(target)
        .insert((TakenUp { by, rotation }, ScriptClip::fall(HANGS)));
}

/// Lets the creature `target` go at the point `hand`, its heading back, along `flight`, its
/// dying clip playing.
pub(crate) fn throw(commands: &mut Commands, target: Entity, hand: Vec3, flight: arts::Flight) {
    commands.queue(move |world: &mut World| {
        let Ok(mut entity) = world.get_entity_mut(target) else {
            return;
        };
        let rotation = entity.take::<TakenUp>().map(|taken| taken.rotation);
        if let Some(mut transform) = entity.get_mut::<Transform>() {
            transform.translation = hand;
            if let Some(rotation) = rotation {
                transform.rotation = rotation;
            }
        }
        entity.insert((Flung(flight), ScriptClip::fall(THROWN)));
    });
}

/// Moves a thrown creature by its flight `flight` for `dt` seconds (`0x44DFE0`): on the ground
/// where it may stand on its level `level`, else its velocity on the ground is gone; over the
/// ground by the flight's height. The ground's height under it.
pub(crate) fn fly(
    transform: &mut Transform,
    flight: &mut arts::Flight,
    heights: &Heights,
    level: Level,
    dt: f32,
) -> f32 {
    let [dx, dz] = flight.step(dt);
    let to = transform.translation + Vec3::new(dx, 0.0, dz);
    if heights.walkable_on(crate::paths::cell_of(to.x, to.z), level) {
        transform.translation.x = to.x;
        transform.translation.z = to.z;
    } else {
        flight.velocity = [0.0, 0.0];
    }
    let ground = heights
        .standing(transform.translation.x, transform.translation.z, level)
        .unwrap_or(transform.translation.y);
    transform.translation.y =
        ground + crate::iso::height(flight.height.max(0.0) * crate::iso::PIXELS_PER_MODEL_HEIGHT);
    ground
}

/// The height of the point `at` over the ground `ground` in the models' units, a flight's.
pub(crate) fn model_height(at: f32, ground: f32) -> f32 {
    (at - ground).max(0.0) / crate::iso::height(crate::iso::PIXELS_PER_MODEL_HEIGHT)
}

/// Holds each creature taken up at its taker's bone at each frame: its model's root on the
/// bone's place and orientation, its own scale kept. One whose taker makes the art on it no more
/// is dropped onto the ground where it hangs, its heading back.
fn carry(
    mut commands: Commands,
    mut taken: Query<(Entity, &mut Transform, &TakenUp, &InTheWay)>,
    makers: Query<&Playing>,
    children: Query<&Children>,
    (bones, worn): (Query<(&Name, &GlobalTransform)>, Worn),
    heights: Res<Heights>,
) {
    for (entity, mut transform, up, way) in &mut taken {
        let holds = makers
            .get(up.by)
            .is_ok_and(|playing| playing.target() == entity);
        if !holds {
            let at = transform.translation;
            transform.rotation = up.rotation;
            transform.translation.y = heights.standing(at.x, at.z, way.level).unwrap_or(at.y);
            commands.entity(entity).remove::<TakenUp>();
            debug!("backbreaker: {entity} dropped where it hung");
            continue;
        }
        let bone = crate::making::bone_entity(up.by, &CARRIED_AT, &children, (&bones, &worn))
            .and_then(|bone| bones.get(bone).ok());
        if let Some((_, at)) = bone {
            transform.translation = at.translation();
            transform.rotation = at.rotation();
        }
    }
}

/// Flies each creature let go along its flight until it lands, put on the ground then.
fn fling(
    mut commands: Commands,
    time: Res<Time>,
    heights: Res<Heights>,
    mut flung: Query<(Entity, &mut Transform, &mut Flung, &InTheWay)>,
) {
    for (entity, mut transform, mut flung, way) in &mut flung {
        let ground = fly(
            &mut transform,
            &mut flung.0,
            &heights,
            way.level,
            time.delta_secs(),
        );
        if flung.0.landed() {
            transform.translation.y = ground;
            commands.entity(entity).remove::<Flung>();
        }
    }
}

/// The backbreaker's carrying and throw.
pub struct BackbreakerPlugin;

impl Plugin for BackbreakerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (carry, fling)
                .in_set(Unpaused)
                .in_set(Step::Npcs)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sector (0, 0) all walkable on the ground, but for the cells' column 12 of the ground byte
    /// `0x41`, as the walls'.
    fn ground() -> Heights {
        use opensacred_data::world::{Cell, SIDE, Sector};
        let cell = |ground| Cell {
            tile: 0,
            overlays: Vec::new(),
            heights: [0; 4],
            light: [255; 4],
            heights2: [0; 4],
            offsets: [0; 2],
            flags: 0,
            ground,
        };
        let mut cells = vec![cell(0x40); SIDE * SIDE];
        for y in 0..SIDE {
            cells[y * SIDE + 12] = cell(0x41);
        }
        let mut heights = Heights::default();
        heights.add(&Sector {
            x: 0,
            y: 0,
            id: 0,
            liquids: [0; 2],
            cells,
            statics: Vec::new(),
            layers: Vec::new(),
            buildings: Vec::new(),
        });
        heights
    }

    /// Flies `flight` from `from` in frames of 1/60 s until it lands, at most 10 s: where it
    /// landed, and the highest it rose over the ground.
    fn flown(heights: &Heights, from: Vec3, mut flight: arts::Flight) -> (Vec3, f32) {
        let mut transform = Transform::from_translation(from);
        let mut highest: f32 = 0.0;
        for _ in 0..600 {
            let ground = fly(&mut transform, &mut flight, heights, None, 1.0 / 60.0);
            highest = highest.max(transform.translation.y - ground);
            if flight.landed() {
                return (transform.translation.with_y(ground), highest);
            }
        }
        panic!("the flight never landed");
    }

    #[test]
    fn a_thrown_creature_flies_on_along_its_line_and_lands() {
        let heights = ground();
        let from = crate::iso::ground(5.5, 5.5);
        let hand = crate::iso::height(120.0 * crate::iso::PIXELS_PER_MODEL_HEIGHT);
        let flight = arts::Flight {
            height: model_height(from.y + hand, from.y),
            ..arts::Flight::new([750.0, 0.0], arts::BACKBREAKER_RISE)
        };
        assert!((flight.height - 120.0).abs() < 1e-3, "{}", flight.height);
        let (at, highest) = flown(&heights, from, flight);
        // About a second in the air from the hand's height, some 70 units on along +x.
        let gone = at - from;
        assert!(gone.z.abs() < 1e-3, "{gone}");
        assert!((60.0..80.0).contains(&gone.x), "{gone}");
        assert!(highest >= hand, "{highest} {hand}");
    }

    #[test]
    fn a_cell_it_may_not_enter_stops_a_thrown_creature() {
        let heights = ground();
        let from = crate::iso::ground(11.5, 5.5);
        let wall = crate::iso::ground(12.0, 5.5);
        let along = (wall - from).xz().normalize() * 3_000.0;
        let flight = arts::Flight::new(along.into(), arts::BACKBREAKER_RISE);
        let (at, _) = flown(&heights, from, flight);
        assert_eq!(
            crate::paths::cell_of(at.x, at.z),
            crate::paths::cell_of(from.x, from.z)
        );
    }

    #[test]
    fn a_point_under_the_ground_is_no_height() {
        assert_eq!(model_height(1.0, 3.0), 0.0);
    }
}
