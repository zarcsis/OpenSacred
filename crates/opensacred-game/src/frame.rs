//! The order of a frame's systems across the modules: each module's plugin puts its systems of
//! `Update` that others depend on in one of these steps, which run in this order, each step's
//! commands applied before the next. Every step, and every other system of a game, is
//! [`Playing`]: it runs only while a game is played.

use bevy::input::InputSystems;
use bevy::prelude::{
    App, First, In, IntoScheduleConfigs, IntoSystem, Plugin, PreUpdate, Res, ResMut, Resource,
    SystemSet, Time, Update, in_state,
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
    /// The sectors streamed in and out around the hero, those in view of a hero that arrives
    /// somewhere at once (`sectors`).
    Sectors,
    /// The camera following the hero, which lands first where it arrived (`hero`).
    Follow,
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

/// The player's mouse on the world and the keys that go with it, which hold back what takes the
/// keyboard (the open console, a movie, the scripts' cinema, a question that waits and the world
/// map: `console`, `movies`, `cinema`, `purchase`, `world_map`), the pointer over the bar or a
/// window (`bar`, `inventory`, `quest_book`, `stables`) and a dialogue's window (`talk`): each
/// module's test of it ([`HoldsControl`]) taken as the frame begins ([`HeldBack`]).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Control;

/// The keys of the pause and the menus (Esc and P of a game, `pause` and `busy_dialog`; the main
/// menu's, `menu`), which only what takes the keyboard holds back
/// ([`HoldsControl::hold_control`]). The pointer over the bar or a window and a dialogue's
/// window leave them (#2877): the original's menu over a game opens and its pause stands with
/// the cursor over the taskbar and through a talk (seen; docs/re/netscript.md, "The talk's own
/// end, and the world meanwhile").
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Menus;

/// The player's clicks on the world, which the scripts' cinema refuses (`0x6172ED`,
/// `scripts::no_cinema`, #1892): the hero's moves, its attacks, its arts, its talks and its
/// pick-ups, the doors, the containers and the stashes it opens.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OnWorld;

/// Whether [`Control`] and [`Menus`] are held back in this frame, by the tests of
/// [`HoldsControl`] as the windows and the console stood when it began (#1593): taken in
/// `PreUpdate` ([`Hold`]) before any key or click of the frame shows or hides one, so that a
/// press that hides its window (a close button) does not reach the world in its frame, whichever
/// of its systems runs first. The original's button takes the press it lies under, whatever it
/// does with it (`0x72B560`).
#[derive(Resource, Debug, Default)]
pub struct HeldBack {
    /// [`Control`], by any test.
    clicks: bool,
    /// [`Menus`] too, by a test of what takes the keyboard.
    keys: bool,
}

/// The step of `PreUpdate` that takes [`HeldBack`]: after Bevy reads the input and the pointer
/// (`hud`), before the console's keys and the windows' toggles, which run after them.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hold;

/// A module's test of whether the player's keys and mouse go to the world, true where they do.
pub trait HoldsControl {
    /// Holds [`Control`] and [`Menus`] back in the frames that begin with `free` false: what
    /// takes the keyboard.
    fn hold_control<M>(&mut self, free: impl IntoSystem<(), bool, M> + 'static) -> &mut Self;

    /// Holds [`Control`] alone back in the frames that begin with `free` false: the pointer over
    /// a window, or a dialogue's window, under which the pause's and the menus' keys go on.
    fn hold_clicks<M>(&mut self, free: impl IntoSystem<(), bool, M> + 'static) -> &mut Self;
}

impl HoldsControl for App {
    fn hold_control<M>(&mut self, free: impl IntoSystem<(), bool, M> + 'static) -> &mut Self {
        self.add_systems(PreUpdate, free.pipe(hold).in_set(Hold))
    }

    fn hold_clicks<M>(&mut self, free: impl IntoSystem<(), bool, M> + 'static) -> &mut Self {
        self.add_systems(PreUpdate, free.pipe(hold_clicks).in_set(Hold))
    }
}

impl HeldBack {
    /// Whether a window or the console held the world's controls back as the frame began: the
    /// cursor then shows nothing of the world under it (`cursors`, #2507).
    pub fn held(&self) -> bool {
        self.clicks
    }
}

/// A test that found the world's controls and the menus' keys held back.
fn hold(In(free): In<bool>, mut held: ResMut<HeldBack>) {
    if !free {
        held.clicks = true;
        held.keys = true;
    }
}

/// A test that found the world's controls alone held back.
fn hold_clicks(In(free): In<bool>, mut held: ResMut<HeldBack>) {
    if !free {
        held.clicks = true;
    }
}

/// A new frame, held back by none yet.
fn release(mut held: ResMut<HeldBack>) {
    *held = HeldBack::default();
}

/// Whether no test held [`Control`] back as the frame began.
fn free(held: Res<HeldBack>) -> bool {
    !held.clicks
}

/// Whether no test held [`Menus`] back as the frame began.
fn keys_free(held: Res<HeldBack>) -> bool {
    !held.keys
}

/// [`HeldBack`], its taking each frame in its step and the conditions of [`Control`] and
/// [`Menus`].
pub(crate) fn held_back(app: &mut App) {
    app.init_resource::<HeldBack>()
        .add_systems(First, release)
        .configure_sets(
            PreUpdate,
            Hold.after(InputSystems)
                .after(crate::hud::point)
                .before(crate::console::keys),
        )
        .configure_sets(Update, (Control.run_if(free), Menus.run_if(keys_free)));
}

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

    /// Stands the world in this frame: no tick, and the time gathered toward the next dropped.
    pub fn stand(&mut self) {
        self.gathered = 0.0;
        self.seconds = None;
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
        held_back(app);
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
                    Step::Sectors,
                    Step::Follow,
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
        // Stood, it ticks not in that frame, and gathers anew from nothing after it.
        world.gather(0.09);
        world.gather(0.25);
        world.stand();
        assert_eq!(world.seconds, None);
        world.gather(0.09);
        assert_eq!(world.seconds, None);
        world.gather(0.02);
        assert!(
            world
                .seconds
                .is_some_and(|seconds| (seconds - 0.11).abs() < 1e-6)
        );
    }

    #[test]
    fn a_window_hidden_in_its_frame_holds_the_world_back_until_the_next() {
        use bevy::prelude::{IntoScheduleConfigs as _, MinimalPlugins};
        #[derive(Resource)]
        struct Shown(bool);
        #[derive(Resource, Default)]
        struct Steered(u32);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        held_back(&mut app);
        app.insert_resource(Shown(true))
            .init_resource::<Steered>()
            .hold_control(|shown: Res<Shown>| !shown.0)
            // A close button that hides its window, then the world's controls.
            .add_systems(
                Update,
                (
                    |mut shown: ResMut<Shown>| shown.0 = false,
                    (|mut steered: ResMut<Steered>| steered.0 += 1).in_set(Control),
                )
                    .chain(),
            );
        app.update();
        assert_eq!(app.world().resource::<Steered>().0, 0);
        app.update();
        assert_eq!(app.world().resource::<Steered>().0, 1);
    }

    #[test]
    fn a_window_under_the_pointer_holds_the_clicks_back_and_leaves_the_menus_keys() {
        use bevy::prelude::{IntoScheduleConfigs as _, MinimalPlugins};
        #[derive(Resource, Default)]
        struct Over(bool);
        #[derive(Resource, Default)]
        struct Typing(bool);
        #[derive(Resource, Default)]
        struct Ran {
            clicks: u32,
            keys: u32,
        }
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        held_back(&mut app);
        app.init_resource::<Over>()
            .init_resource::<Typing>()
            .init_resource::<Ran>()
            .hold_clicks(|over: Res<Over>| !over.0)
            .hold_control(|typing: Res<Typing>| !typing.0)
            .add_systems(
                Update,
                (
                    (|mut ran: ResMut<Ran>| ran.clicks += 1).in_set(Control),
                    (|mut ran: ResMut<Ran>| ran.keys += 1).in_set(Menus),
                ),
            );
        let ran = |app: &mut App| {
            app.update();
            let ran = app.world().resource::<Ran>();
            (ran.clicks, ran.keys)
        };
        // Nothing holds: both run.
        assert_eq!(ran(&mut app), (1, 1));
        // The pointer over a window (#2877): the clicks wait, Esc and P go on, and the cursor
        // shows nothing of the world.
        app.world_mut().resource_mut::<Over>().0 = true;
        assert_eq!(ran(&mut app), (1, 2));
        assert!(app.world().resource::<HeldBack>().held());
        // What takes the keyboard holds both, with the pointer over a window or off it.
        app.world_mut().resource_mut::<Typing>().0 = true;
        assert_eq!(ran(&mut app), (1, 2));
        app.world_mut().resource_mut::<Over>().0 = false;
        assert_eq!(ran(&mut app), (1, 2));
        assert!(app.world().resource::<HeldBack>().held());
        // And both go on once it lets go.
        app.world_mut().resource_mut::<Typing>().0 = false;
        assert_eq!(ran(&mut app), (2, 3));
        assert!(!app.world().resource::<HeldBack>().held());
    }
}
