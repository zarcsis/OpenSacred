//! The hero's remarks outside the fight (#1176; docs/re/sound.md, "The hero's comments"): lines
//! by name that the sound manager's event 9 and its `0x696060` play as the scripts and the player
//! give them cause (`0x693FE0`), each stopping the line by name that plays, at the hero in the
//! middle, at the sample's own rate, the comments' clock left alone: `GETNQ` as a side quest is
//! taken in a talk (`0x46C160`), `REJECTNQ` at the scripts' `RejectQuest`, and `CANCELMYIDLE` once
//! the play has run 120 s without the player's input (`0x424A60`, #1246).

use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::{MouseButtonInput, MouseWheel};
use bevy::log::debug;
use bevy::prelude::{
    App, AssetServer, Assets, Commands, CursorMoved, Entity, GlobalTransform, IntoScheduleConfigs,
    Message, MessageReader, MessageWriter, OnEnter, Plugin, Query, Res, ResMut, Resource, Time,
    Update, With,
};
use bevy_kira_audio::prelude::{Audio, AudioControl, AudioInstance};
use opensacred_rules::spawning::Rng;

use crate::frame::{Playing, Unpaused};
use crate::game::AppState;
use crate::hero::{self, Body};
use crate::sfx::{self, Sfx};
use crate::voices::{self, NamedLine, Speaking};

/// A remark of the hero's, which a module asks for.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Remark {
    /// `GETNQ` (9, `0x40`): a side quest entered in a talk.
    TakesQuest,
    /// `REJECTNQ`: the scripts' `RejectQuest` (126, `0x696060`).
    RejectsQuest,
    /// `CANCELMYIDLE` (9, `0x100`): 120 s of play without the player's input.
    Idles,
}

impl Remark {
    /// How many lines a class has, one of which `rand() %` this picks.
    fn count(self) -> u32 {
        match self {
            Self::TakesQuest | Self::RejectsQuest => 11,
            Self::Idles => 14,
        }
    }

    /// The name of the line `n`, from 0, of the hero of the type `creature`: none of
    /// `REJECTNQ` for the Dwarf and the Daemon, whose lines `0x696060`'s table of seven classes
    /// leaves out.
    fn name(self, creature: u32, n: u32) -> Option<String> {
        let class = voices::class(creature)?;
        let n = n + 1;
        match self {
            Self::TakesQuest => Some(format!("SOUND_FX_GETNQ{class}{n:02}")),
            Self::RejectsQuest => {
                (creature <= 7).then(|| format!("SOUND_FX_REJECTNQ{class}{n:02}"))
            }
            Self::Idles => Some(format!("SOUND_FX_CANCELMYIDLE_{n:02}_{class}")),
        }
    }
}

/// How long the play runs without the player's input before the hero remarks on it, in seconds
/// (`0x424A60`).
const IDLE: f64 = 120.0;

/// The seconds of play since the player's last input or the last idle remark (`cStatsManager`'s
/// clock `+0x56F8` against its stamp `+0x5714`), and the remarks' rolls.
#[derive(Resource)]
struct Remarks {
    idle: f64,
    dice: Rng,
}

impl Default for Remarks {
    fn default() -> Self {
        Self {
            idle: 0.0,
            dice: Rng::of(0, 0, 0x1246),
        }
    }
}

/// Sets the idle clock back at every key, mouse button, wheel and motion of the cursor
/// (`0x423FD0`, which the engine's events of the kinds 1 to 4 and two input handlers call;
/// **hypothesis**: the keys and the mouse, its motion among them).
fn input(
    mut remarks: ResMut<Remarks>,
    mut keys: MessageReader<KeyboardInput>,
    mut buttons: MessageReader<MouseButtonInput>,
    mut wheel: MessageReader<MouseWheel>,
    mut moved: MessageReader<CursorMoved>,
) {
    let any = keys.read().count() + buttons.read().count() + wheel.read().count();
    if any + moved.read().count() > 0 {
        remarks.idle = 0.0;
    }
}

/// Starts the idle clock with each game.
fn begin(mut remarks: ResMut<Remarks>) {
    remarks.idle = 0.0;
}

/// Runs the idle clock while the play runs with a hero, and past [`IDLE`] asks for the remark
/// and starts it again (`0x424A60`).
fn idle(
    mut remarks: ResMut<Remarks>,
    time: Res<Time>,
    heroes: Query<(), With<Body>>,
    mut posts: MessageWriter<Remark>,
) {
    if !heroes.is_empty() && idles(&mut remarks.idle, f64::from(time.delta_secs())) {
        posts.write(Remark::Idles);
    }
}

/// A frame of `seconds` on the idle clock `idle`: whether it passed [`IDLE`], the clock then
/// started again.
fn idles(idle: &mut f64, seconds: f64) -> bool {
    *idle += seconds;
    if *idle > IDLE {
        *idle = 0.0;
        return true;
    }
    false
}

/// Plays the remarks asked for: one of the class's lines by `rand()`, by name, at the hero, the
/// line by name that plays stopped (`0x693FE0`).
fn remark(
    mut commands: Commands,
    mut posts: MessageReader<Remark>,
    mut remarks: ResMut<Remarks>,
    (hero, mut sfx, heard): (Option<Res<hero::Hero>>, ResMut<Sfx>, Res<sfx::Listener>),
    heroes: Query<(Entity, &GlobalTransform), With<Body>>,
    (assets, audio, mut instances, mut named): (
        Res<AssetServer>,
        Res<Audio>,
        ResMut<Assets<AudioInstance>>,
        ResMut<NamedLine>,
    ),
) {
    for &post in posts.read() {
        let n = remarks.dice.below(post.count());
        let (Some(hero), Ok((entity, at))) = (hero.as_ref(), heroes.single()) else {
            continue;
        };
        let Some(name) = post.name(hero.creature, n) else {
            continue;
        };
        let Some((id, file)) = sfx.named(&name) else {
            debug!("remarks: no line {name}");
            continue;
        };
        let at = at.translation();
        let (volume, pan) = sfx::placed_at(at, Some(at), *heard);
        if volume <= 0.0 {
            continue;
        }
        debug!("remarks: the hero says {name}");
        let sample = sfx.sample(id, file, &assets);
        let instance = audio
            .play(sample)
            .with_volume(sfx::decibels(volume))
            .with_panning(pan)
            .handle();
        named.name(&mut instances, Some(instance.clone()));
        commands.entity(entity).insert(Speaking(instance, true));
    }
}

/// The hero's remarks and the idle clock.
pub struct RemarksPlugin;

impl Plugin for RemarksPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Remark>()
            .init_resource::<Remarks>()
            .add_systems(OnEnter(AppState::InGame), begin)
            .add_systems(Update, input.in_set(Playing))
            .add_systems(Update, (idle, remark).chain().after(input).in_set(Unpaused));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_idle_clock_speaks_past_120_s_and_starts_again() {
        let mut idle = 0.0;
        let frames = |idle: &mut f64, count: u32| (0..count).filter(|_| idles(idle, 0.5)).count();
        assert_eq!(frames(&mut idle, 240), 0);
        assert_eq!(frames(&mut idle, 1), 1);
        assert_eq!(frames(&mut idle, 240), 0);
        assert_eq!(frames(&mut idle, 1), 1);
    }

    #[test]
    fn a_remark_names_the_line_of_the_heros_class() {
        assert_eq!(
            Remark::TakesQuest.name(1, 0).as_deref(),
            Some("SOUND_FX_GETNQSERA01")
        );
        assert_eq!(
            Remark::Idles.name(7, 13).as_deref(),
            Some("SOUND_FX_CANCELMYIDLE_14_VAMP")
        );
        assert_eq!(
            Remark::RejectsQuest.name(5, 10).as_deref(),
            Some("SOUND_FX_REJECTNQHELF11")
        );
        // The Dwarf and the Daemon reject in silence.
        assert_eq!(Remark::RejectsQuest.name(8, 0), None);
        assert_eq!(Remark::RejectsQuest.name(9, 0), None);
    }
}
