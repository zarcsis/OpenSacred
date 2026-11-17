//! The sparks of a level-up (#2762; docs/re/fight.md, "The hero's levels"): as a creature rises a
//! level, `addExperience` (`0x57E160`), once the level's setter has run, makes the generic system
//! 856 at it of the variant 2 (`0x57F504`, the FX event `0x50`): its figure in sparks, one at every
//! `⌊V/500⌋`th vertex of its model as last drawn, still where they were made, shrinking away
//! within two seconds, added whole and untinted by the night; gold for the hero, a darker gold for
//! a follower, red for another creature that rises by its kills (`opensacred_rules::particles::
//! {Riser, vertex_sparks}`). `puffs` draws them over the creature's model, as the original draws
//! the system after the creature standing in its cell, its billboards depth tested against the
//! model's, so that the sparks before its surface cover it (`fx::Orders::over`). The hero's
//! level-up (`fight::level_up`) and a creature's (`npcs::earn`) mark it [`Rises`].

use bevy::prelude::*;
use opensacred_rules::particles::{self, Riser};

use crate::spawning::Rng;

/// A creature that rose a level, which [`burst`] bursts into its sparks.
#[derive(Component)]
pub(crate) struct Rises;

/// The sparks' dice (`rand()`).
#[derive(Resource)]
struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x2762))
    }
}

/// Bursts each creature that rose a level into its sparks, at the vertices of its model as last
/// posed, its worn items' left out, in its riser's colour.
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn burst(
    mut commands: Commands,
    risers: Query<
        (
            Entity,
            &GlobalTransform,
            Has<crate::hero::Body>,
            Has<crate::followers::Follower>,
        ),
        With<Rises>,
    >,
    (shapes, meshes): (crate::vertices::ModelPoints, Res<Assets<Mesh>>),
    mut dice: ResMut<Dice>,
) {
    for (entity, at, hero, follower) in &risers {
        commands.entity(entity).try_remove::<Rises>();
        let riser = Riser::of(hero, follower);
        let origin = at.translation();
        let points: Vec<[f32; 3]> = shapes
            .picked(entity, &meshes)
            .into_iter()
            .map(|point| crate::puffs::local(point, origin))
            .collect();
        debug!(
            "level sparks: {entity}, a {riser:?}, bursts into {} sparks",
            points.len()
        );
        if points.is_empty() {
            continue;
        }
        crate::puffs::spawn_over(
            &mut commands,
            particles::vertex_sparks(&points, riser.colour(), &mut dice.0),
            origin,
            entity,
        );
    }
}

/// The sparks of a level-up.
pub struct LevelSparksPlugin;

impl Plugin for LevelSparksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>().add_systems(
            Update,
            burst
                .after(crate::fight::level_up)
                .after(crate::npcs::earn)
                .in_set(crate::frame::Playing),
        );
    }
}

#[cfg(test)]
mod tests {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::PrimitiveTopology;
    use bevy::mesh::skinning::SkinnedMeshInverseBindposes;

    use super::*;

    #[test]
    fn a_creature_that_rises_bursts_into_sparks_at_its_vertices_once() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<SkinnedMeshInverseBindposes>>()
            .init_resource::<Dice>()
            .add_systems(Update, burst);
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all())
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_POSITION,
                vec![[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            );
        let mesh = app.world_mut().resource_mut::<Assets<Mesh>>().add(mesh);
        let place = GlobalTransform::from_translation(Vec3::new(10.0, 0.0, 20.0));
        // A creature of a model of three vertices, and one of none.
        let creature = app
            .world_mut()
            .spawn((Rises, place))
            .with_child((Mesh3d(mesh), place))
            .id();
        let bare = app.world_mut().spawn((Rises, place)).id();
        app.update();
        let mut bursts = app
            .world_mut()
            .query_filtered::<&Transform, With<crate::puffs::Unmade>>();
        let made: Vec<Vec3> = bursts
            .iter(app.world())
            .map(|at| at.translation)
            .collect();
        assert_eq!(made, [place.translation()]);
        assert!(app.world().get::<Rises>(creature).is_none());
        assert!(app.world().get::<Rises>(bare).is_none());
        // Once: the next frame makes none.
        app.update();
        assert_eq!(bursts.iter(app.world()).count(), 1);
    }
}
