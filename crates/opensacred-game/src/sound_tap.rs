//! The game's whole mix for a recording's sound track (#1230, #1228's answer): a tap at the end of
//! every channel's track, after its volume control (`volumes`), and on the movies' track. Kira's
//! main track sums its sub tracks and has no effects of its own here, and a sub track runs its
//! effects before its own volume, which stays 0 dB, so that the sum of the taps is what goes to
//! the device, sample for sample.
//!
//! Kira renders in chunks of up to its internal buffer's frames, every track one after another in
//! the audio thread, and every track's effects see every chunk while the track plays, which every
//! channel's does: each tap adds its chunk into a shared sum, and the last to give to a chunk hands
//! it to the recording through a lock-free ring buffer (`rtrb`), from the chunk after the one the
//! recording armed it in. The sum's lock is the audio thread's own but for the moments a recording
//! arms or disarms it; the taps pass their samples on unchanged and allocate nothing as they run.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use bevy_kira_audio::AudioEffect;
use kira::Frame;
use kira::effect::{Effect, EffectBuilder};
use kira::info::Info;
use rtrb::{Consumer, Producer, RingBuffer};

/// The device's sample rate, as Kira gives it to the taps; 0 until one is on a track.
static SAMPLE_RATE: AtomicU32 = AtomicU32::new(0);

/// The taps' sum and where it goes.
static MIX: Mutex<Mix> = Mutex::new(Mix::new());

/// The sum of the chunk being rendered, and the recording's end of it.
struct Mix {
    sum: Vec<Frame>,
    /// The chunk's frames.
    length: usize,
    /// The chunk's number since the first tap.
    chunk: u64,
    /// The taps that gave to it so far, and the taps on tracks.
    given: usize,
    taps: usize,
    /// Where the sums go while a recording takes them.
    out: Option<Producer<Frame>>,
    /// A recording's, from the next chunk on.
    pending: Option<Producer<Frame>>,
    /// When the first chunk handed to the recording was summed.
    started: Option<Instant>,
    /// Frames the ring buffer had no room for.
    lost: u64,
}

impl Mix {
    const fn new() -> Self {
        Self {
            sum: Vec::new(),
            length: 0,
            chunk: 0,
            given: 0,
            taps: 0,
            out: None,
            pending: None,
            started: None,
            lost: 0,
        }
    }

    /// Ends the chunk: its sum to the recording, if one takes it, and a pending recording's from
    /// the next.
    fn finish(&mut self) {
        if let Some(out) = &mut self.out {
            self.started.get_or_insert_with(Instant::now);
            for &frame in &self.sum[..self.length] {
                if out.push(frame).is_err() {
                    self.lost += 1;
                }
            }
        }
        if let Some(pending) = self.pending.take() {
            self.out = Some(pending);
            self.started = None;
            self.lost = 0;
        }
        self.sum[..self.length].fill(Frame::ZERO);
        self.length = 0;
        self.given = 0;
        self.chunk += 1;
    }
}

/// A tap on a track: it adds the track's chunks into the sum and leaves them as they are.
struct Tap {
    /// The chunk it gives to next.
    next: u64,
}

impl Effect for Tap {
    fn init(&mut self, sample_rate: u32, internal_buffer_size: usize) {
        SAMPLE_RATE.store(sample_rate, Ordering::Release);
        if let Ok(mut mix) = MIX.lock() {
            mix.taps += 1;
            if mix.sum.len() < internal_buffer_size {
                mix.sum.resize(internal_buffer_size, Frame::ZERO);
            }
            self.next = mix.chunk;
        }
    }

    fn on_change_sample_rate(&mut self, sample_rate: u32) {
        SAMPLE_RATE.store(sample_rate, Ordering::Release);
    }

    fn process(&mut self, input: &mut [Frame], _dt: f64, _info: &Info) {
        let Ok(mut mix) = MIX.lock() else {
            return;
        };
        // A tap that gave to this chunk already: the chunk is over without the rest's.
        if self.next > mix.chunk {
            mix.finish();
        }
        if mix.given == 0 {
            mix.length = input.len().min(mix.sum.len());
        }
        let length = mix.length.min(input.len());
        for (sum, &frame) in mix.sum[..length].iter_mut().zip(input.iter()) {
            *sum += frame;
        }
        mix.given += 1;
        self.next = mix.chunk + 1;
        if mix.given >= mix.taps {
            mix.finish();
        }
    }
}

impl Drop for Tap {
    fn drop(&mut self) {
        if let Ok(mut mix) = MIX.lock() {
            mix.taps = mix.taps.saturating_sub(1);
        }
    }
}

/// The tap to add last to a track's effects.
pub struct TapBuilder;

impl AudioEffect for TapBuilder {
    type Handle = ();

    fn build_effect(self) -> (Box<dyn Effect>, ()) {
        (Box::new(Tap { next: 0 }), ())
    }
}

impl EffectBuilder for TapBuilder {
    type Handle = ();

    fn build(self) -> (Box<dyn Effect>, ()) {
        self.build_effect()
    }
}

/// A recording's end of the mix: the frames summed from the chunk after it was armed.
pub struct Sound {
    frames: Consumer<Frame>,
    /// The device's sample rate.
    pub sample_rate: u32,
}

impl Sound {
    /// Starts taking the mix with room for `seconds` of it waiting, if a sound device plays the
    /// taps' tracks; `None` without one.
    pub fn arm(seconds: f64) -> Option<Self> {
        let sample_rate = SAMPLE_RATE.load(Ordering::Acquire);
        let mut mix = MIX.lock().ok()?;
        if sample_rate == 0 || mix.taps == 0 {
            return None;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a few seconds of frames, far within usize"
        )]
        let room = (seconds.max(1.0) * f64::from(sample_rate)) as usize;
        let (producer, frames) = RingBuffer::new(room);
        mix.pending = Some(producer);
        Some(Self {
            frames,
            sample_rate,
        })
    }

    /// A recording's end of `frames` at `sample_rate`, for the tests of what takes it.
    #[cfg(test)]
    pub(crate) fn of(frames: Consumer<Frame>, sample_rate: u32) -> Self {
        Self {
            frames,
            sample_rate,
        }
    }

    /// The frames summed since the last call, into `into`.
    pub fn take(&mut self, into: &mut Vec<Frame>) {
        while let Ok(frame) = self.frames.pop() {
            into.push(frame);
        }
    }

    /// When the first frame handed over was summed, once one was, and the frames lost so far.
    pub fn started() -> (Option<Instant>, u64) {
        MIX.lock().map_or((None, 0), |mix| (mix.started, mix.lost))
    }

    /// Stops taking the mix.
    pub fn disarm() {
        if let Ok(mut mix) = MIX.lock() {
            mix.out = None;
            mix.pending = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kira::backend::mock::{MockBackend, MockBackendSettings};
    use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};
    use kira::track::TrackBuilder;
    use kira::{AudioManager, AudioManagerSettings};

    use super::*;

    /// A known signal on each of two tracks comes out of the ring buffer as their sum, sample for
    /// sample, from the chunk after the recording armed it, with the tracks' samples unchanged.
    #[test]
    fn the_taps_hand_over_the_sum_of_their_tracks() {
        let mut manager = AudioManager::<MockBackend>::new(AudioManagerSettings {
            backend_settings: MockBackendSettings { sample_rate: 1000 },
            ..AudioManagerSettings::default()
        })
        .unwrap();
        let mut left = manager
            .add_sub_track(TrackBuilder::new().with_effect(TapBuilder))
            .unwrap();
        let mut right = manager
            .add_sub_track(TrackBuilder::new().with_effect(TapBuilder))
            .unwrap();
        let signal = |k: u32| -> Arc<[Frame]> {
            (0..4000)
                .map(|n| {
                    let v = f32::from(u16::try_from((n * k) % 1000).unwrap()) / 1000.0;
                    Frame { left: v, right: -v }
                })
                .collect()
        };
        let sound = |frames: Arc<[Frame]>| StaticSoundData {
            sample_rate: 1000,
            frames,
            settings: StaticSoundSettings::default(),
            slice: None,
        };
        let (a, b) = (signal(7), signal(13));
        left.play(sound(a.clone())).unwrap();
        right.play(sound(b.clone())).unwrap();
        let mut chunk = || {
            manager.backend_mut().on_start_processing();
            manager.backend_mut().process();
        };
        chunk();
        chunk();
        let mut recorded = Sound::arm(10.0).unwrap();
        assert_eq!(recorded.sample_rate, 1000);
        // The chunk it was armed in goes by; the next two are handed over.
        chunk();
        chunk();
        chunk();
        let mut frames = Vec::new();
        recorded.take(&mut frames);
        Sound::disarm();
        // The chunks of 128 frames from the fourth, the sounds' frames from 384.
        assert_eq!(frames.len(), 256);
        for (n, frame) in frames.iter().enumerate() {
            let at = 384 + n;
            assert_eq!(frame.left, a[at].left + b[at].left, "frame {n}");
            assert_eq!(frame.right, a[at].right + b[at].right, "frame {n}");
        }
        assert!(Sound::started().0.is_some());
    }
}
