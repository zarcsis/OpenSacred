//! The peace stream's alternation (docs/re/sound.md, "The peace stream's alternation"; #1177):
//! by day the region's music and the ambient loop of the place take the one stream in turns of
//! 180 s, crossfading over 7 s. The timer (`+0xB7B8`) counts by day and dawn alone; past 180 s it
//! switches the stream to the other phase and starts again, a crossfade that runs taking 20 s off
//! it first. The hero's death and the dusk and the night ask for the ambient loop now, setting the
//! timer to 200 s in the music's phase; a sector without music cannot open the music, so that the
//! ambient loop, once it plays, stays. The scripts' control (`SetScriptSoundControle`, #1243)
//! refuses the switches while it holds, until the scripts give the stream back or 300 s pass.
//! `opensacred-game`'s `music` and `atmosphere` play what the phase holds.

/// How long a phase lasts, in seconds of the timer (`0x8961F0`).
pub const TURN: f64 = 180.0;
/// How much later a switch due while a crossfade runs is tried again (`0x896220`).
pub const WAITS: f64 = 20.0;
/// The timer that a request for the ambient loop sets, past [`TURN`] (`0x691310`).
pub const NOW: f64 = 200.0;
/// How long a crossfade lasts, in seconds (`0x69C4BB`).
pub const CROSSFADE: f32 = 7.0;
/// How long the scripts' control holds after their last `SetScriptSoundControle`, in seconds
/// (`+0xB7C8`, `0x6791C0`).
pub const CONTROL: f64 = 300.0;

/// What the peace stream plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The region's music (`+0x698` 3).
    Music,
    /// The ambient loop of the place's nature (`+0x698` 2).
    Ambient,
}

/// The peace stream: its phase, none before the first region's event, its timer, and what is
/// left of the scripts' control, none when they do not hold it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stream {
    phase: Option<Phase>,
    timer: f64,
    control: Option<f64>,
}

impl Stream {
    /// The phase it plays, none before [`Self::begin`].
    pub fn phase(&self) -> Option<Phase> {
        self.phase
    }

    /// Its timer, in seconds.
    pub fn timer(&self) -> f64 {
        self.timer
    }

    /// Whether the scripts hold the stream (`+0x6B1`): no music but theirs changes it.
    pub fn controlled(&self) -> bool {
        self.control.is_some()
    }

    /// The scripts' `SetScriptSoundControle` (129, `0x6791C0`): `on` takes the stream for
    /// [`CONTROL`] from now, asking for the ambient loop in the music's phase as [`Self::now`]
    /// does; off gives it back, the caller crossfading to what the phase holds.
    pub fn control(&mut self, on: bool) {
        if on {
            self.control = Some(CONTROL);
            self.now();
        } else {
            self.control = None;
        }
    }

    /// Its first phase, as the first region's event rolls it (`0x6822E5`): the ambient loop for an
    /// odd `roll` (`rand() & 1`), else the music; the stream begun once.
    pub fn begin(&mut self, roll: u32) -> Phase {
        *self.phase.get_or_insert(if roll & 1 == 1 {
            Phase::Ambient
        } else {
            Phase::Music
        })
    }

    /// A frame of `seconds`: the scripts' control runs out at [`CONTROL`] whatever the hour
    /// (`0x69B85C`); the timer counts where the day `counts` (the day and the dawn, not the dusk
    /// nor the night); past [`TURN`], unless the place `holds` the cycle (the cemetery's and the
    /// indoor arena's music, a spot loop of their own), a crossfade that runs (`crossfading`)
    /// takes [`WAITS`] off it, and a timer still past it switches to the other phase, the music
    /// only where the place has one (`music`) and nothing while the scripts hold the stream, and
    /// starts again (`0x69BD5E`). The phase it switched to, if it did.
    pub fn frame(
        &mut self,
        seconds: f64,
        (counts, holds): (bool, bool),
        (crossfading, music): (bool, bool),
    ) -> Option<Phase> {
        if let Some(left) = self.control {
            self.control = Some(left - seconds).filter(|&left| left > 0.0);
        }
        if counts {
            self.timer += seconds;
        }
        let phase = self.phase?;
        if holds || self.timer <= TURN {
            return None;
        }
        if crossfading {
            self.timer -= WAITS;
            if self.timer <= TURN {
                return None;
            }
        }
        // A switch that the control refuses, tried again at each frame (`0x694616`).
        if self.controlled() {
            return None;
        }
        let next = match phase {
            Phase::Music => Phase::Ambient,
            // The music fails to open in a place without one, and is tried again at each frame.
            Phase::Ambient if !music => return None,
            Phase::Ambient => Phase::Music,
        };
        self.phase = Some(next);
        self.timer = 0.0;
        Some(next)
    }

    /// A request for the ambient loop now (`0x691310`; the hero's death `0x6853F7`, the dusk's
    /// and the night's events, a music that fails to open): in the music's phase the timer to
    /// [`NOW`], so that the next frame switches to the ambient loop; in the ambient phase nothing.
    pub fn now(&mut self) {
        if self.phase == Some(Phase::Music) {
            self.timer = NOW;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Frames of `seconds` each, `count` of them, by day, nothing held nor fading, with music.
    fn run(stream: &mut Stream, seconds: f64, count: u32) -> Vec<Phase> {
        (0..count)
            .filter_map(|_| stream.frame(seconds, (true, false), (false, true)))
            .collect()
    }

    #[test]
    fn by_day_the_music_and_the_ambient_loop_take_turns_of_180_s() {
        let mut stream = Stream::default();
        // Nothing before the first region's event.
        assert_eq!(stream.frame(200.0, (true, false), (false, true)), None);
        assert_eq!(stream.begin(1), Phase::Ambient);
        // Begun once: a later roll changes nothing.
        assert_eq!(stream.begin(0), Phase::Ambient);
        let mut stream = Stream::default();
        stream.begin(1);
        // Seen: ATMO_WOOD at 20 s, the music 180 s later, the loop 180 s after it.
        assert!(run(&mut stream, 0.5, 360).is_empty());
        assert_eq!(run(&mut stream, 0.5, 1), [Phase::Music]);
        assert!(run(&mut stream, 0.5, 360).is_empty());
        assert_eq!(run(&mut stream, 0.5, 1), [Phase::Ambient]);
    }

    #[test]
    fn the_dusk_and_the_night_hold_the_timer_and_ask_for_the_ambient_loop() {
        let mut stream = Stream::default();
        stream.begin(0);
        assert_eq!(stream.phase(), Some(Phase::Music));
        // The dusk asks for the loop: the next frame switches, whatever the timer.
        stream.now();
        assert_eq!(
            stream.frame(0.1, (false, false), (false, true)),
            Some(Phase::Ambient)
        );
        // The night counts nothing: no music all night.
        for _ in 0..10_000 {
            assert_eq!(stream.frame(0.1, (false, false), (false, true)), None);
        }
        assert!(stream.timer() < 1.0);
        // A request in the ambient phase changes nothing.
        stream.now();
        assert_eq!(stream.frame(0.1, (false, false), (false, true)), None);
        // The dawn counts again: the music 180 s into it.
        assert!(run(&mut stream, 0.5, 360).is_empty());
        assert_eq!(run(&mut stream, 0.5, 1), [Phase::Music]);
    }

    #[test]
    fn the_scripts_control_holds_the_stream_until_given_back_or_300_s_pass() {
        let mut stream = Stream::default();
        stream.begin(0);
        // Taken in the music's phase: the loop asked for, but refused while held.
        stream.control(true);
        assert!(stream.controlled());
        assert!((stream.timer() - NOW).abs() < 1e-9);
        assert_eq!(stream.frame(0.5, (true, false), (false, true)), None);
        assert_eq!(stream.phase(), Some(Phase::Music));
        // Given back: the switch goes through at the next frame.
        stream.control(false);
        assert_eq!(
            stream.frame(0.5, (true, false), (false, true)),
            Some(Phase::Ambient)
        );
        // The control runs out 300 s after it was taken, the switch due then going through.
        stream.control(true);
        assert_eq!(stream.phase(), Some(Phase::Ambient));
        assert!(run(&mut stream, 0.5, 599).is_empty());
        assert!(stream.controlled());
        assert_eq!(run(&mut stream, 0.5, 1), [Phase::Music]);
        assert!(!stream.controlled());
    }

    #[test]
    fn a_switch_waits_for_a_crossfade_and_a_place_without_music() {
        let mut stream = Stream::default();
        stream.begin(0);
        stream.now();
        // A crossfade running: 20 s off, the switch where the timer is still past 180 s.
        assert_eq!(stream.frame(0.0, (true, false), (true, true)), None);
        assert!((stream.timer() - (NOW - WAITS)).abs() < 1e-9);
        assert_eq!(stream.frame(0.0, (true, false), (false, true)), None);
        assert_eq!(
            stream.frame(0.5, (true, false), (false, true)),
            Some(Phase::Ambient)
        );
        let mut stream = Stream::default();
        stream.begin(0);
        stream.now();
        assert_eq!(
            stream.frame(0.5, (true, false), (true, true)),
            Some(Phase::Ambient)
        );
        // No music where the hero stands: the loop stays, tried again at each frame.
        let mut stream = Stream::default();
        stream.begin(1);
        assert_eq!(stream.frame(181.0, (true, false), (false, false)), None);
        assert_eq!(stream.frame(0.1, (true, false), (false, false)), None);
        assert_eq!(
            stream.frame(0.1, (true, false), (false, true)),
            Some(Phase::Music)
        );
        // The cemetery holds the cycle.
        let mut stream = Stream::default();
        stream.begin(1);
        assert_eq!(stream.frame(500.0, (true, true), (false, true)), None);
        assert_eq!(stream.phase(), Some(Phase::Ambient));
    }
}
