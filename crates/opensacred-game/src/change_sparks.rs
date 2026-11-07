//! The look of the Wood Elf's changeling (36, 845, `cParticleSystem_changeling`; #1092;
//! docs/re/effects.md, "The Wood Elf's sights"): as a creature changes into an animal, its figure
//! in pale green sparks, one at every `⌊V/500⌋`th vertex of its model as last posed, falling and
//! bouncing on the ground under it as they shrink; as it changes back, the animal's figure the
//! same, rising ever faster. The sparks stay where they were made, added whole, untinted by the
//! night (`opensacred_rules::particles::changeling_sparks`, drawn by `puffs`).

use bevy::prelude::*;
use opensacred_rules::particles;

use crate::spawning::Rng;

/// The sparks' dice (`rand()`).
#[derive(Resource)]
pub(crate) struct Dice(Rng);

impl Default for Dice {
    fn default() -> Self {
        Self(crate::seed::dice(0x845))
    }
}

/// Bursts `creature`'s figure into the changeling's sparks, falling at the change or rising at
/// the change back (`back`), from the ground under it: the spell's object at the change
/// (`0x525E5A`), and the effect 20's taker's at the change back, whose start takes the animal's
/// figure before the creature is changed back (`0x587F91`).
pub(crate) fn burst(
    In((creature, back)): In<(Entity, bool)>,
    mut commands: Commands,
    (shapes, meshes): (crate::vertices::ModelPoints, Res<Assets<Mesh>>),
    places: Query<&GlobalTransform>,
    mut dice: ResMut<Dice>,
) {
    let Ok(origin) = places.get(creature).map(GlobalTransform::translation) else {
        return;
    };
    let points: Vec<[f32; 3]> = shapes
        .picked(creature, &meshes)
        .into_iter()
        .map(|point| crate::puffs::local(point, origin))
        .collect();
    debug!(
        "change sparks: {creature} bursts into {} sparks, back {back}",
        points.len()
    );
    if points.is_empty() {
        return;
    }
    crate::puffs::spawn(
        &mut commands,
        particles::changeling_sparks(&points, back, &mut dice.0),
        origin,
        None,
    );
}

/// The changeling's sparks.
pub struct ChangeSparksPlugin;

impl Plugin for ChangeSparksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dice>();
    }
}
