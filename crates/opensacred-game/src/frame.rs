//! The order of a frame's systems across the modules: each module's plugin puts its systems of
//! `Update` that others depend on in one of these steps, which run in this order, each step's
//! commands applied before the next.

use bevy::prelude::{App, IntoScheduleConfigs, Plugin, SystemSet, Update};

/// The steps of a frame, in order.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    /// The hero's model spawned and its clip started (`hero`).
    HeroSpawn,
    /// The camera's zoom (`iso`).
    Zoom,
    /// The hero's gait, its target and its run (`hero`).
    HeroMove,
    /// The building the hero is in (`buildings`).
    Buildings,
    /// The footsteps (`sound`).
    Footsteps,
    /// The camera following the hero (`hero`).
    Follow,
    /// The sectors streamed in and out around the hero (`sectors`).
    Sectors,
    /// The start script's NPCs and objects placed, animated and equipped (`npcs`).
    Npcs,
    /// The models' materials and places among the sprites (`models`).
    Models,
    /// The characters' shadows (`shadows`).
    Shadows,
}

/// The order of the steps.
pub struct FramePlugin;

impl Plugin for FramePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (
                Step::HeroSpawn,
                Step::Zoom,
                Step::HeroMove,
                Step::Buildings,
                Step::Footsteps,
                Step::Follow,
                Step::Sectors,
                Step::Npcs,
                Step::Models,
                Step::Shadows,
            )
                .chain(),
        );
    }
}
