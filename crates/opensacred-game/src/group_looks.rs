//! The sights of a script's `SetGroupState` (#2893; docs/re/netscript.md, "`SetGroupState`,
//! `DelNPC`, `SetOnKill` and `SetOnCollect`"): a group that its `0x51` removes vanishes in the
//! soul's sparks, an object 780 (`FX_PUZZLE_SOLVED`) by the FX event `0xC` at each member
//! (`0x49ED51`), its figure in pale green sparks rising where it stood as a body that has lain its
//! time leaves it (`soul_sparks`); a member whose side its `0x08` or `0x0E` changes flashes in the
//! generic sparks of its figure, an object 856 of the variant 0 by the FX event `0x50`
//! (`0x49F172`, `0x49F276`), red as it turns hostile and near white as it turns peaceful
//! (`opensacred_rules::minds::grouped_flash`), as an undead goblin burns away (`puffs`). Each
//! takes the member's figure as the record runs, before the member goes. The flash serves the
//! black magician 271's blink too, its pale green sparks where it stood (`0x4D72E8`, #2898).

use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use opensacred_rules::particles;

use crate::game::AppState;
use crate::spawning::Rng;
use crate::vertices::ModelPoints;

/// The dice of the flashes' sparks.
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x2893))
    }
}

/// The place of `member` and the points of its figure (`0x767FB0`), none where it has no place.
fn figure(world: &mut World, member: Entity) -> Option<(Vec3, Vec<Vec3>)> {
    let at = world.get::<GlobalTransform>(member)?.translation();
    let mut state = SystemState::<(ModelPoints, Res<Assets<Mesh>>)>::new(world);
    let (shapes, meshes) = state.get(world).ok()?;
    Some((at, shapes.picked(member, &meshes)))
}

/// The removed `member`'s figure rising in the soul's sparks where it stood (780 by the FX event
/// `0xC`), taken now, before it goes.
pub(crate) fn vanish(world: &mut World, member: Entity) {
    let Some((at, points)) = figure(world, member) else {
        return;
    };
    debug!("group looks: {member} vanishes in {} sparks", points.len());
    world.spawn((
        Transform::from_translation(at),
        crate::soul_sparks::Sparks {
            from: member,
            rising: true,
        },
        crate::soul_sparks::Figure(points),
        DespawnOnExit(AppState::InGame),
    ));
}

/// `member`'s flash of the colour `colour` as its side changes: the generic sparks of the
/// variant 0 at its figure (856 by the FX event `0x50`).
pub(crate) fn flash(world: &mut World, member: Entity, colour: u32) {
    let Some((at, points)) = figure(world, member) else {
        return;
    };
    let mut points: Vec<[f32; 3]> = points
        .into_iter()
        .map(|point| crate::puffs::local(point, at))
        .collect();
    if points.is_empty() {
        points.push([0.0; 3]);
    }
    let Some(mut dice) = world.get_resource_mut::<Dice>() else {
        return;
    };
    let burst = particles::generic_sparks(&points, colour, &mut dice.0);
    debug!("group looks: {member} flashes {colour:#010x}");
    crate::puffs::spawn_in(world, burst, at, None);
}

/// The groups' sights' dice.
pub struct GroupLooksPlugin;

impl Plugin for GroupLooksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_member_vanishes_and_flashes_where_it_stands() {
        let mut world = World::new();
        world.init_resource::<Assets<Mesh>>();
        world.init_resource::<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>();
        let member = world.spawn(GlobalTransform::from_xyz(10.0, 0.0, 5.0)).id();
        // Its soul's sparks of the figure taken now, rising where it stands.
        vanish(&mut world, member);
        let mut souls = world.query::<(
            &Transform,
            &crate::soul_sparks::Sparks,
            &crate::soul_sparks::Figure,
        )>();
        let (at, sparks, figure) = souls.single(&world).unwrap();
        assert_eq!(at.translation, Vec3::new(10.0, 0.0, 5.0));
        assert!(sparks.from == member && sparks.rising);
        assert!(figure.0.is_empty());
        // A flash wants the dice; with them, a burst at it.
        let mut bursts = world.query::<(&Transform, &crate::puffs::Unmade)>();
        flash(&mut world, member, particles::TURNED_HOSTILE);
        assert_eq!(bursts.iter(&world).count(), 0);
        world.init_resource::<Dice>();
        flash(&mut world, member, particles::TURNED_HOSTILE);
        let (at, _) = bursts.single(&world).unwrap();
        assert_eq!(at.translation, Vec3::new(10.0, 0.0, 5.0));
        // None for a member gone.
        let gone = world.spawn_empty().id();
        vanish(&mut world, gone);
        flash(&mut world, gone, particles::TURNED_PEACEFUL);
        assert_eq!(souls.iter(&world).count(), 1);
        assert_eq!(bursts.iter(&world).count(), 1);
    }
}
