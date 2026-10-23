//! The order of a frame's systems across the modules: each module's plugin puts its systems of
//! `Update` that others depend on in one of these steps, which run in this order, each step's
//! commands applied before the next. Every step, and every other system of a game, is
//! [`Playing`]: it runs only while a game is played.

use bevy::prelude::{
    App, IntoScheduleConfigs, Plugin, Res, ResMut, Resource, SystemSet, Time, Update, in_state,
};

use crate::game::AppState;

/// The steps of a frame, in order.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    /// The world's tick gathered (`frame`).
    Tick,
    /// The hero's model spawned and its clip started (`hero`).
    HeroSpawn,
    /// The camera's zoom (`iso`).
    Zoom,
    /// The cells the creatures stand on (`crowd`).
    Crowd,
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
    /// The scripts' and the fills' NPCs and objects placed, animated and equipped (`npcs`).
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

/// The world's tick (`cEngine::renderThread::run`, `0x60E350`): the render loop adds each
/// frame's time to `0xAD6EFC` and, once the sum is above 0.1 s, runs the creatures' updates
/// (`0x5375C0`) with it and starts again from 0, so every 6 or 7 frames at 60 a second (#616).
/// The systems that the original runs in those updates run when [`ticked`], on its seconds.
#[derive(Resource, Debug, Default)]
pub struct WorldTick {
    gathered: f32,
    /// This frame's tick, its seconds, if one falls in it.
    pub seconds: Option<f32>,
}

/// The time that the world's tick gathers: it runs once the sum is above it (`0x60E350`).
const TICK: f32 = 0.1;

impl WorldTick {
    /// Gathers a frame's `seconds`; the tick runs when their sum is above [`TICK`].
    pub fn gather(&mut self, seconds: f32) {
        self.gathered += seconds;
        self.seconds = (self.gathered > TICK).then(|| std::mem::take(&mut self.gathered));
    }
}

/// Gathers the frame's time of the game, which stands while it is paused.
fn tick(time: Res<Time>, mut world: ResMut<WorldTick>) {
    world.gather(time.delta_secs());
}

/// Whether the world's tick runs in this frame.
pub fn ticked(world: Res<WorldTick>) -> bool {
    world.seconds.is_some()
}

/// The order of the steps.
pub struct FramePlugin;

impl Plugin for FramePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldTick>()
            .configure_sets(Update, Playing.run_if(in_state(AppState::InGame)))
            .configure_sets(
                Update,
                (
                    Step::Tick,
                    Step::HeroSpawn,
                    Step::Zoom,
                    Step::Crowd,
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
            )
            .add_systems(Update, tick.in_set(Step::Tick));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_world_ticks_once_its_frames_gather_more_than_a_tenth_of_a_second() {
        // At 60 frames a second, every 6 or 7 frames as the sum's rounding falls (#616).
        let mut world = WorldTick::default();
        let mut ticks = Vec::new();
        for frame in 1..=40 {
            world.gather(1.0 / 60.0);
            if let Some(seconds) = world.seconds {
                ticks.push((frame, seconds));
            }
        }
        let frames: Vec<u32> = ticks.iter().map(|&(frame, _)| frame).collect();
        assert!(
            frames
                .windows(2)
                .all(|pair| (6..=7).contains(&(pair[1] - pair[0]))),
            "{frames:?}"
        );
        assert!(
            ticks
                .iter()
                .all(|&(_, seconds)| seconds > TICK && seconds < 0.12)
        );
        // A frame of more than a tenth ticks at once, with what was gathered before it.
        world.gather(0.25);
        assert!(world.seconds.is_some_and(|seconds| seconds >= 0.25));
    }
}
