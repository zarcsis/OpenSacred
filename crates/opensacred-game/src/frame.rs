//! The order of a frame's systems across the modules: each module's plugin puts its systems of
//! `Update` that others depend on in one of these steps, which run in this order, each step's
//! commands applied before the next. Every step, and every other system of a game, is
//! [`Playing`]: it runs only while a game is played.

use bevy::prelude::{App, IntoScheduleConfigs, Plugin, SystemSet, Update, in_state};

use crate::game::AppState;

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
    /// The models' shadows (`shadows`).
    Shadows,
}

/// The systems of a game, which run only while one is played (`game::AppState::InGame`): the
/// steps, and the systems of a game outside them.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Playing;

/// The systems of a game that stop while it is paused (`pause`): the player's keys and mouse on
/// the world, and what starts a sound. The rest goes on, standing still on the paused time.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Unpaused;

/// The player's keys and mouse on the world, the pause and the menus, which the open console
/// holds back (`console`).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Control;

/// The order of the steps.
pub struct FramePlugin;

impl Plugin for FramePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(Update, Playing.run_if(in_state(AppState::InGame)))
            .configure_sets(
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
                    .chain()
                    .in_set(Playing),
            );
    }
}
