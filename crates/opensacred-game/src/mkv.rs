//! Recording into one Matroska file (#1225, #1226): `--record FILE.mkv` and `record FILE.mkv N`.
//! The game writes a Matroska stream of its own into FFmpeg's standard input, its frames as raw
//! BGRA in Matroska's VFW mode, each at the real time it was made at, and a text track with each
//! frame's times; FFmpeg encodes the frames losslessly in FFV1 (`bgr0`, a variable frame rate,
//! none dropped or repeated), keeps the text track and tags the file with the run's details. The
//! frames go through a thread of their own, so that neither the pipe nor FFmpeg holds up a frame
//! of the game. FFmpeg is found as the converter finds it: beside the game, else `FFMPEG`, else
//! `ffmpeg` from `PATH`; without it the recording fails with an error that says so, and the game
//! goes on. FFmpeg's Matroska muxer rounds the blocks' times to 1 ms, so that once it ends the
//! game remuxes its file into the one asked for at a microsecond a unit ([`remux`], #1227): each
//! frame's block and its times' event at the frame's exact time, the packets copied as they are
//! (docs/10, the command `record`; docs/05, "Looking at frames").
//!
//! The game's whole mix goes with them as a third track (#1230): the taps' frames (`sound_tap`) as
//! float PCM at the device's rate, set against the frames' `Time<Real>` by when the mix's first
//! chunk was summed, from the recording's first frame to the end of its last, which FFmpeg encodes
//! into FLAC of 24 bits; without a sound device the recording has none, and the log says so.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::sound_tap::Sound;

/// The Matroska stream's time scale: nanoseconds a unit of its timestamps, a microsecond.
const TIMESTAMP_SCALE: u64 = 1_000;
/// The tracks: the frames, then their times.
const VIDEO: u64 = 1;
const TEXT: u64 = 2;
const AUDIO: u64 = 3;
/// The sound's frames a block: some 21 ms at 48 kHz, well within a cluster's 16-bit offset.
const SOUND_BLOCK: usize = 1024;
/// How long the end of a recording waits for the mix to reach its last frame's end.
const SOUND_WAIT: Duration = Duration::from_millis(500);
/// FFV1's slices: the frame split for its threads.
const SLICES: &str = "16";

/// A frame's times, as its screenshot was taken: the text track's event over it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Times {
    /// The frame's number in the recording, from 0.
    pub n: u32,
    /// Bevy's count of the frames since the start.
    pub frame: u32,
    /// `Time<Real>`'s elapsed time and delta, in seconds.
    pub real: f64,
    pub real_delta: f64,
    /// `Time<Virtual>`'s, in seconds.
    pub r#virtual: f64,
    pub virtual_delta: f64,
    /// Whether the world was paused.
    pub paused: bool,
}

/// A frame of a recording: its times and its pixels, BGRA rows from the top.
pub struct Frame {
    pub times: Times,
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

// EBML (RFC 8794) and Matroska's elements, by their IDs, which carry their own length marker.
const EBML: u32 = 0x1A45_DFA3;
const EBML_VERSION: u32 = 0x4286;
const EBML_READ_VERSION: u32 = 0x42F7;
const EBML_MAX_ID_LENGTH: u32 = 0x42F2;
const EBML_MAX_SIZE_LENGTH: u32 = 0x42F3;
const DOC_TYPE: u32 = 0x4282;
const DOC_TYPE_VERSION: u32 = 0x4287;
const DOC_TYPE_READ_VERSION: u32 = 0x4285;
const SEGMENT: u32 = 0x1853_8067;
const INFO: u32 = 0x1549_A966;
const TIMESTAMP_SCALE_ID: u32 = 0x2A_D7B1;
const MUXING_APP: u32 = 0x4D80;
const WRITING_APP: u32 = 0x5741;
const TRACKS: u32 = 0x1654_AE6B;
const TRACK_ENTRY: u32 = 0xAE;
const TRACK_NUMBER: u32 = 0xD7;
const TRACK_UID: u32 = 0x73C5;
const TRACK_TYPE: u32 = 0x83;
const CODEC_ID: u32 = 0x86;
const CODEC_PRIVATE: u32 = 0x63A2;
const VIDEO_SETTINGS: u32 = 0xE0;
const AUDIO_SETTINGS: u32 = 0xE1;
const SAMPLING_FREQUENCY: u32 = 0xB5;
const CHANNELS: u32 = 0x9F;
const BIT_DEPTH: u32 = 0x6264;
const PIXEL_WIDTH: u32 = 0xB0;
const PIXEL_HEIGHT: u32 = 0xBA;
const CLUSTER: u32 = 0x1F43_B675;
const CLUSTER_TIMESTAMP: u32 = 0xE7;
const SIMPLE_BLOCK: u32 = 0xA3;
const BLOCK_GROUP: u32 = 0xA0;
const BLOCK: u32 = 0xA1;
const BLOCK_DURATION: u32 = 0x9B;
const DURATION: u32 = 0x4489;
const CRC_32: u32 = 0xBF;
const VOID: u32 = 0xEC;
const SEEK_HEAD: u32 = 0x114D_9B74;
const SEEK: u32 = 0x4DBB;
const SEEK_ID: u32 = 0x53AB;
const SEEK_POSITION: u32 = 0x53AC;
const TAGS: u32 = 0x1254_C367;
const CUES: u32 = 0x1C53_BB6B;
const CUE_POINT: u32 = 0xBB;
const CUE_TIME: u32 = 0xB3;
const CUE_TRACK_POSITIONS: u32 = 0xB7;
const CUE_TRACK: u32 = 0xF7;
const CUE_CLUSTER_POSITION: u32 = 0xF1;
/// A track's types: video and subtitles.
const TRACK_VIDEO: u64 = 1;
const TRACK_AUDIO: u64 = 2;
const TRACK_SUBTITLE: u64 = 0x11;
/// The size of an element whose end the stream does not tell: the segment's and the clusters'.
const UNKNOWN_SIZE: [u8; 8] = [0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

/// The bytes of an element's ID, its leading zero bytes left out.
fn id(out: &mut Vec<u8>, id: u32) {
    let bytes = id.to_be_bytes();
    let first = bytes.iter().position(|&b| b != 0).unwrap_or(3);
    out.extend_from_slice(&bytes[first..]);
}

/// An element of `body`, its size in eight bytes.
fn element(out: &mut Vec<u8>, element: u32, body: &[u8]) {
    id(out, element);
    let size = u64::try_from(body.len()).unwrap_or(u64::MAX) & 0x00FF_FFFF_FFFF_FFFF;
    out.extend_from_slice(&(size | 0x0100_0000_0000_0000).to_be_bytes());
    out.extend_from_slice(body);
}

/// An unsigned integer element, in its fewest bytes.
fn uint(out: &mut Vec<u8>, element_id: u32, value: u64) {
    let bytes = value.to_be_bytes();
    let first = bytes.iter().position(|&b| b != 0).unwrap_or(7);
    element(out, element_id, &bytes[first..]);
}

fn text(out: &mut Vec<u8>, element_id: u32, value: &str) {
    element(out, element_id, value.as_bytes());
}

/// A `BITMAPINFOHEADER` of BGRA frames of `width` × `height` pixels, their rows from the top
/// (a negative height), as Matroska's VFW mode takes one for its codec's private data.
fn bitmap_info(width: u32, height: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(40);
    let height = i32::try_from(height).unwrap_or(i32::MAX);
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&i32::try_from(width).unwrap_or(i32::MAX).to_le_bytes());
    out.extend_from_slice(&(-height).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    // BI_RGB, its image's size, no resolution, no palette.
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(
        &width
            .saturating_mul(height.unsigned_abs())
            .saturating_mul(4)
            .to_le_bytes(),
    );
    out.extend_from_slice(&[0; 16]);
    out
}

/// The stream's start: the EBML header, the segment, its info and its tracks, for frames of
/// `width` × `height` pixels and, with a sample rate, the sound's stereo float PCM.
pub fn head(width: u32, height: u32, sound: Option<u32>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut header = Vec::new();
    uint(&mut header, EBML_VERSION, 1);
    uint(&mut header, EBML_READ_VERSION, 1);
    uint(&mut header, EBML_MAX_ID_LENGTH, 4);
    uint(&mut header, EBML_MAX_SIZE_LENGTH, 8);
    text(&mut header, DOC_TYPE, "matroska");
    uint(&mut header, DOC_TYPE_VERSION, 4);
    uint(&mut header, DOC_TYPE_READ_VERSION, 2);
    element(&mut out, EBML, &header);
    id(&mut out, SEGMENT);
    out.extend_from_slice(&UNKNOWN_SIZE);
    let mut info = Vec::new();
    uint(&mut info, TIMESTAMP_SCALE_ID, TIMESTAMP_SCALE);
    text(&mut info, MUXING_APP, "opensacred-game");
    text(&mut info, WRITING_APP, "opensacred-game");
    element(&mut out, INFO, &info);
    let mut video = Vec::new();
    uint(&mut video, TRACK_NUMBER, VIDEO);
    uint(&mut video, TRACK_UID, VIDEO);
    uint(&mut video, TRACK_TYPE, TRACK_VIDEO);
    text(&mut video, CODEC_ID, "V_MS/VFW/FOURCC");
    element(&mut video, CODEC_PRIVATE, &bitmap_info(width, height));
    let mut size = Vec::new();
    uint(&mut size, PIXEL_WIDTH, u64::from(width));
    uint(&mut size, PIXEL_HEIGHT, u64::from(height));
    element(&mut video, VIDEO_SETTINGS, &size);
    let mut times = Vec::new();
    uint(&mut times, TRACK_NUMBER, TEXT);
    uint(&mut times, TRACK_UID, TEXT);
    uint(&mut times, TRACK_TYPE, TRACK_SUBTITLE);
    text(&mut times, CODEC_ID, "S_TEXT/UTF8");
    let mut tracks = Vec::new();
    element(&mut tracks, TRACK_ENTRY, &video);
    element(&mut tracks, TRACK_ENTRY, &times);
    if let Some(rate) = sound {
        let mut audio = Vec::new();
        uint(&mut audio, TRACK_NUMBER, AUDIO);
        uint(&mut audio, TRACK_UID, AUDIO);
        uint(&mut audio, TRACK_TYPE, TRACK_AUDIO);
        text(&mut audio, CODEC_ID, "A_PCM/FLOAT/IEEE");
        let mut settings = Vec::new();
        element(
            &mut settings,
            SAMPLING_FREQUENCY,
            &f64::from(rate).to_be_bytes(),
        );
        uint(&mut settings, CHANNELS, 2);
        uint(&mut settings, BIT_DEPTH, 32);
        element(&mut audio, AUDIO_SETTINGS, &settings);
        element(&mut tracks, TRACK_ENTRY, &audio);
    }
    element(&mut out, TRACKS, &tracks);
    out
}

/// A cluster of the sound's `frames` at `at` microseconds from the recording's start, their
/// samples as little-endian floats, left and right.
fn sound_cluster(at: u64, frames: &[kira::Frame]) -> Vec<u8> {
    let mut block = block_head(AUDIO, true);
    for frame in frames {
        block.extend_from_slice(&frame.left.to_le_bytes());
        block.extend_from_slice(&frame.right.to_le_bytes());
    }
    let mut body = Vec::with_capacity(block.len() + 32);
    uint(&mut body, CLUSTER_TIMESTAMP, at);
    element(&mut body, SIMPLE_BLOCK, &block);
    let mut out = Vec::with_capacity(body.len() + 12);
    id(&mut out, CLUSTER);
    out.extend_from_slice(&UNKNOWN_SIZE);
    out.extend_from_slice(&body);
    out
}

/// A recording's sound: the mix's frames as they come, set against the recording's times, the
/// first sample at its first frame's.
pub struct SoundTrack {
    sound: Sound,
    /// `Time<Real>`'s first update, which the frames' real times count from.
    base: Instant,
    /// The frames come and not written yet.
    waiting: Vec<kira::Frame>,
    /// Frames to leave out before the first that counts, when the mix began before the
    /// recording's first frame.
    skip: u64,
    /// Whether the first frames are set against the recording's start yet.
    placed: bool,
    /// The frames written, from the recording's start.
    written: u64,
    /// When the mix's first frame was summed, once it was: [`Sound::started`].
    started: fn() -> Option<Instant>,
}

impl SoundTrack {
    /// The sound of `sound`, whose times count from `base`, `Time<Real>`'s first update.
    pub fn new(sound: Sound, base: Instant) -> Self {
        Self {
            sound,
            base,
            waiting: Vec::new(),
            skip: 0,
            placed: false,
            written: 0,
            started: || Sound::started().0,
        }
    }

    /// The frames of `seconds`, rounded.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0 and rounded: a recording's frames, far within u64"
    )]
    fn frames_of(&self, seconds: f64) -> u64 {
        (seconds.max(0.0) * f64::from(self.sound.sample_rate)).round() as u64
    }

    /// The microseconds of `frames` from the recording's start.
    fn micros_of(&self, frames: u64) -> u64 {
        let rate = u128::from(self.sound.sample_rate.max(1));
        u64::try_from((u128::from(frames) * 1_000_000 + rate / 2) / rate).unwrap_or(u64::MAX)
    }

    /// Takes the mix's new frames; the first, once the mix tells when it began, set against the
    /// recording's first frame at `first` seconds of `Time<Real>`: silence before them if they
    /// began after it, the frames before it left out if before.
    fn gather(&mut self, first: f64) {
        let mut came = Vec::new();
        self.sound.take(&mut came);
        if !self.placed
            && let Some(started) = (self.started)()
        {
            let offset = started.saturating_duration_since(self.base).as_secs_f64() - first;
            if offset >= 0.0 {
                let silence = usize::try_from(self.frames_of(offset)).unwrap_or(0);
                self.waiting
                    .splice(0..0, std::iter::repeat_n(kira::Frame::ZERO, silence));
            } else {
                self.skip = self.frames_of(-offset);
            }
            self.placed = true;
        }
        let skipped = usize::try_from(self.skip)
            .unwrap_or(usize::MAX)
            .min(came.len());
        self.skip -= u64::try_from(skipped).unwrap_or(0);
        self.waiting.extend_from_slice(&came[skipped..]);
    }

    /// The clusters of the whole blocks that begin before `until` microseconds, or of every frame
    /// waiting with `all`.
    fn blocks(&mut self, until: u64, all: bool) -> Vec<u8> {
        let mut out = Vec::new();
        if !self.placed {
            return out;
        }
        loop {
            let enough = self.waiting.len() >= SOUND_BLOCK || (all && !self.waiting.is_empty());
            let at = self.micros_of(self.written);
            if !enough || (!all && at >= until) {
                return out;
            }
            let length = self.waiting.len().min(SOUND_BLOCK);
            out.extend_from_slice(&sound_cluster(at, &self.waiting[..length]));
            self.waiting.drain(..length);
            self.written += u64::try_from(length).unwrap_or(0);
        }
    }

    /// The rest, up to `end` microseconds, the last frame's end: the mix waited for as long as
    /// [`SOUND_WAIT`] to come so far, silence for what it did not, the frames past it left out.
    fn rest(&mut self, end: u64, first: f64) -> Vec<u8> {
        let rate = u128::from(self.sound.sample_rate);
        let total = u64::try_from(u128::from(end) * rate / 1_000_000).unwrap_or(u64::MAX);
        let waited = Instant::now();
        loop {
            self.gather(first);
            let have = self.written + u64::try_from(self.waiting.len()).unwrap_or(0);
            if have >= total || waited.elapsed() >= SOUND_WAIT {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if !self.placed {
            self.placed = true;
        }
        let keep = usize::try_from(total.saturating_sub(self.written)).unwrap_or(0);
        self.waiting.resize(keep, kira::Frame::ZERO);
        self.blocks(end, true)
    }
}

/// A block's head: its track, its time from its cluster's (0), and its flags.
fn block_head(track: u64, keyframe: bool) -> Vec<u8> {
    // The track's number as a one-byte EBML size.
    let number = u8::try_from(track).unwrap_or(0x7F) | 0x80;
    vec![number, 0, 0, if keyframe { 0x80 } else { 0 }]
}

/// A cluster of one frame at `at` microseconds from the recording's start: the frame's pixels and
/// its times' event, which lasts `lasts` microseconds.
pub fn cluster(at: u64, lasts: u64, frame: &Frame) -> Vec<u8> {
    let mut body = Vec::with_capacity(frame.bgra.len() + 256);
    uint(&mut body, CLUSTER_TIMESTAMP, at);
    let mut block = block_head(VIDEO, true);
    block.extend_from_slice(&frame.bgra);
    element(&mut body, SIMPLE_BLOCK, &block);
    let mut event = block_head(TEXT, false);
    event.truncate(3);
    event.push(0);
    event.extend_from_slice(
        serde_json::to_string(&frame.times)
            .unwrap_or_default()
            .as_bytes(),
    );
    let mut group = Vec::new();
    element(&mut group, BLOCK, &event);
    uint(&mut group, BLOCK_DURATION, lasts.max(1));
    element(&mut body, BLOCK_GROUP, &group);
    let mut out = Vec::with_capacity(body.len() + 12);
    id(&mut out, CLUSTER);
    out.extend_from_slice(&UNKNOWN_SIZE);
    out.extend_from_slice(&body);
    out
}

/// Microseconds from `seconds`, none below 0.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to 0 and rounded: a recording's seconds, far below u64's range"
)]
fn micros(seconds: f64) -> u64 {
    (seconds.max(0.0) * 1e6).round() as u64
}

/// FFmpeg, as the converter finds it: beside the game, as a release carries it, else `FFMPEG`,
/// else `ffmpeg` from `PATH`.
fn ffmpeg() -> PathBuf {
    let beside = std::env::current_exe().ok().and_then(|exe| {
        let path = exe.with_file_name(format!("ffmpeg{}", std::env::consts::EXE_SUFFIX));
        path.is_file().then_some(path)
    });
    beside.unwrap_or_else(|| {
        std::env::var_os("FFMPEG").map_or_else(|| PathBuf::from("ffmpeg"), PathBuf::from)
    })
}

/// FFmpeg's command for `out`, tagged with `tags`, reading the stream from its standard input.
fn command(out: &Path, tags: &[(String, String)], sound: bool) -> Command {
    let mut command = Command::new(ffmpeg());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        /// `CREATE_NO_WINDOW` of `CreateProcess`, as the converter's (#1495).
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "matroska",
        "-i",
        "pipe:0",
    ]);
    command.args([
        "-map", "0", "-c:v", "ffv1", "-level", "3", "-g", "1", "-slices", SLICES,
    ]);
    command.args([
        "-pix_fmt",
        "bgr0",
        "-c:s",
        "copy",
        "-fps_mode",
        "passthrough",
    ]);
    // The encoder at the stream's own time base rather than one of a frame rate guessed from the
    // first frames, which rounds uneven times to its frames (#2616).
    command.args(["-enc_time_base:v", "demux"]);
    if sound {
        // FLAC is whole numbers: 24 bits of the float samples, a peak over 1 clipped as the
        // device clips it.
        command.args(["-c:a", "flac", "-sample_fmt", "s32"]);
    }
    for (key, value) in tags {
        command.arg("-metadata").arg(format!("{key}={value}"));
    }
    command
        .arg("-y")
        .arg(out)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    command
}

/// How a recording into Matroska stands, shared with its thread.
#[derive(Default)]
struct Shared {
    /// Whether FFmpeg has ended, or the recording failed without it.
    done: AtomicBool,
    /// Why it failed, if it did.
    error: Mutex<Option<String>>,
}

/// A recording into one Matroska file: the frames go to its thread, which writes them in their
/// order into FFmpeg; [`Encoder::finish`] closes the stream after the last.
pub struct Encoder {
    frames: Option<Sender<Frame>>,
    shared: Arc<Shared>,
}

impl Encoder {
    /// A recording into `path`, tagged with `tags`, with `sound`, the mix's, if there is a sound
    /// device; FFmpeg starts with its first frame.
    pub fn new(path: PathBuf, tags: Vec<(String, String)>, sound: Option<SoundTrack>) -> Self {
        let (frames, received) = channel();
        let shared = Arc::new(Shared::default());
        let thread_shared = Arc::clone(&shared);
        std::thread::spawn(move || {
            let mut sound = sound;
            let result = write(&path, &tags, &received, &mut sound);
            if sound.is_some() {
                Sound::disarm();
            }
            if let Err(error) = result
                && let Ok(mut slot) = thread_shared.error.lock()
            {
                *slot = Some(error);
            }
            thread_shared.done.store(true, Ordering::Release);
        });
        Self {
            frames: Some(frames),
            shared,
        }
    }

    /// Hands `frame` to the thread.
    #[cfg(test)]
    fn send(&self, frame: Frame) {
        if let Some(frames) = &self.frames {
            // The thread may have ended on an error, which `done` tells.
            frames.send(frame).ok();
        }
    }

    /// What hands frames to the thread, for a screenshot's observer.
    pub fn sender(&self) -> Option<Sender<Frame>> {
        self.frames.clone()
    }

    /// Ends the stream after the frames handed over.
    pub fn finish(&mut self) {
        self.frames = None;
    }

    /// Whether FFmpeg has ended, and why the recording failed if it did.
    pub fn done(&self) -> Option<Result<(), String>> {
        if !self.shared.done.load(Ordering::Acquire) {
            return None;
        }
        let error = self.shared.error.lock().ok().and_then(|slot| slot.clone());
        Some(error.map_or(Ok(()), Err))
    }
}

/// The thread's work: the frames from `received` in their order into FFmpeg, until the channel
/// closes; then FFmpeg's end waited for.
fn write(
    out: &Path,
    tags: &[(String, String)],
    received: &Receiver<Frame>,
    sound: &mut Option<SoundTrack>,
) -> Result<(), String> {
    // FFmpeg's file, at 1 ms a unit, beside the one asked for, which the remux makes.
    let encoded = encoded_path(out);
    let mut timeline = Vec::new();
    let result = encode(&encoded, tags, received, &mut timeline, sound);
    if let Err(error) = result {
        std::fs::remove_file(&encoded).ok();
        return Err(error);
    }
    let rate = sound.as_ref().map(|sound| sound.sound.sample_rate);
    match remux(&encoded, out, &timeline, rate) {
        Ok(()) => {
            std::fs::remove_file(&encoded).ok();
            Ok(())
        }
        Err(error) => {
            // FFmpeg's file, at its milliseconds, under the name asked for.
            std::fs::rename(&encoded, out).ok();
            Err(format!(
                "remuxing {} at microseconds: {error}",
                out.display()
            ))
        }
    }
}

/// The file that FFmpeg writes for a recording into `out`, which the remux makes again.
fn encoded_path(out: &Path) -> PathBuf {
    let mut name = out.file_stem().unwrap_or_default().to_os_string();
    name.push(".ffmpeg.mkv");
    out.with_file_name(name)
}

/// The frames from `received` in their order into FFmpeg writing `out`, until the channel closes;
/// then FFmpeg's end waited for. `timeline` takes each frame's time and its event's length, in
/// microseconds from the first.
fn encode(
    out: &Path,
    tags: &[(String, String)],
    received: &Receiver<Frame>,
    timeline: &mut Vec<(u64, u64)>,
    sound: &mut Option<SoundTrack>,
) -> Result<(), String> {
    let mut waiting: BTreeMap<u32, Frame> = BTreeMap::new();
    let mut next = 0;
    let mut pipe: Option<(Child, ChildStdin)> = None;
    // The frame written last waits for the next's time, its event's end.
    let mut pending: Option<Frame> = None;
    let mut start = None;
    let mut failed = None;
    for frame in received {
        waiting.insert(frame.times.n, frame);
        while let Some(frame) = waiting.remove(&next) {
            next += 1;
            if let Some(previous) = pending.take()
                && failed.is_none()
            {
                let lasts = frame.times.real - previous.times.real;
                match put(
                    out,
                    tags,
                    (&mut start, &mut pipe, sound),
                    previous,
                    Some(lasts),
                ) {
                    Ok(time) => timeline.push(time),
                    Err(error) => failed = Some(error),
                }
            }
            pending = Some(frame);
        }
    }
    if let Some(previous) = pending.take()
        && failed.is_none()
    {
        match put(out, tags, (&mut start, &mut pipe, sound), previous, None) {
            Ok(time) => timeline.push(time),
            Err(error) => failed = Some(error),
        }
    }
    // The sound to the last frame's end.
    if let (Some(track), Some((_, stdin)), Some(&(at, lasts)), Some(first)) =
        (sound.as_mut(), pipe.as_mut(), timeline.last(), start)
        && failed.is_none()
        && let Err(e) = stdin.write_all(&track.rest(at + lasts, first))
    {
        failed = Some(format!("writing to FFmpeg: {e}"));
    }
    let Some((mut child, stdin)) = pipe else {
        return failed.map_or(Ok(()), Err);
    };
    drop(stdin);
    let mut stderr = String::new();
    if let Some(mut err) = child.stderr.take() {
        err.read_to_string(&mut stderr).ok();
    }
    let status = child
        .wait()
        .map_err(|e| format!("waiting for FFmpeg: {e}"))?;
    if let Some(error) = failed {
        return Err(format!("{error}; FFmpeg: {}", stderr.trim()));
    }
    if !status.success() {
        return Err(format!("FFmpeg ended with {status}: {}", stderr.trim()));
    }
    Ok(())
}

/// Writes `frame` into FFmpeg, started with the stream's head at the first, its event lasting
/// `lasts` seconds, else its own delta; `start` is the first frame's real time. Returns the
/// frame's time and its event's length in microseconds.
fn put(
    out: &Path,
    tags: &[(String, String)],
    (start, pipe, sound): (
        &mut Option<f64>,
        &mut Option<(Child, ChildStdin)>,
        &mut Option<SoundTrack>,
    ),
    frame: Frame,
    lasts: Option<f64>,
) -> Result<(u64, u64), String> {
    if pipe.is_none() {
        let rate = sound.as_ref().map(|sound| sound.sound.sample_rate);
        let mut child = command(out, tags, rate.is_some()).spawn().map_err(|e| {
            format!(
                "starting {} for the recording into {}: {e} (FFmpeg is found beside the game, else \
                 by FFMPEG, else on PATH)",
                ffmpeg().display(),
                out.display()
            )
        })?;
        let mut stdin = child.stdin.take().ok_or("no FFmpeg's standard input")?;
        stdin
            .write_all(&head(frame.width, frame.height, rate))
            .map_err(|e| format!("writing to FFmpeg: {e}"))?;
        *pipe = Some((child, stdin));
    }
    let first = *start.get_or_insert(frame.times.real);
    let at = micros(frame.times.real - first);
    let lasts = micros(lasts.unwrap_or(frame.times.real_delta));
    if let Some((_, stdin)) = pipe.as_mut() {
        // The sound's blocks that begin before the frame, then the frame.
        if let Some(sound) = sound.as_mut() {
            sound.gather(first);
            stdin
                .write_all(&sound.blocks(at, false))
                .map_err(|e| format!("writing to FFmpeg: {e}"))?;
        }
        stdin
            .write_all(&cluster(at, lasts, &frame))
            .map_err(|e| format!("writing to FFmpeg: {e}"))?;
    }
    Ok((at, lasts.max(1)))
}

/// An EBML variable-size integer's length from its first byte, 1 to 8.
fn vint_length(first: u8) -> Option<usize> {
    (first != 0).then(|| usize::try_from(first.leading_zeros()).map_or(8, |zeros| zeros + 1))
}

/// An element's ID from `input`, its length marker kept; `None` at the end of the input.
fn read_id(input: &mut impl Read) -> io::Result<Option<u32>> {
    let mut first = [0];
    if input.read(&mut first)? == 0 {
        return Ok(None);
    }
    let length = vint_length(first[0])
        .filter(|&n| n <= 4)
        .ok_or_else(|| invalid("an ID"))?;
    let mut id = u32::from(first[0]);
    for _ in 1..length {
        let mut byte = [0];
        input.read_exact(&mut byte)?;
        id = (id << 8) | u32::from(byte[0]);
    }
    Ok(Some(id))
}

/// An element's size from `input`: `None` for an unknown size.
fn read_size(input: &mut impl Read) -> io::Result<Option<u64>> {
    let mut first = [0];
    input.read_exact(&mut first)?;
    let length = vint_length(first[0]).ok_or_else(|| invalid("a size"))?;
    let marker = 1u64 << (7 * length);
    let mut size = u64::from(first[0]) & ((1 << (8 - length)) - 1);
    for _ in 1..length {
        let mut byte = [0];
        input.read_exact(&mut byte)?;
        size = (size << 8) | u64::from(byte[0]);
    }
    Ok((size != marker - 1).then_some(size))
}

fn invalid(what: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("not a Matroska file: {what}"),
    )
}

/// An element's body of `size` bytes from `input`.
fn read_body(input: &mut impl Read, size: u64) -> io::Result<Vec<u8>> {
    let mut body = vec![0; usize::try_from(size).map_err(|_| invalid("a size"))?];
    input.read_exact(&mut body)?;
    Ok(body)
}

/// The elements of `body`, each its ID and its own body.
fn children(body: &[u8]) -> io::Result<Vec<(u32, &[u8])>> {
    let mut out = Vec::new();
    let mut rest = body;
    while !rest.is_empty() {
        let mut reader = rest;
        let id = read_id(&mut reader)?.ok_or_else(|| invalid("an element cut short"))?;
        let size = read_size(&mut reader)?.ok_or_else(|| invalid("an unknown size inside"))?;
        let size = usize::try_from(size).map_err(|_| invalid("a size"))?;
        let inside = reader
            .get(..size)
            .ok_or_else(|| invalid("an element cut short"))?;
        out.push((id, inside));
        rest = &reader[size..];
    }
    Ok(out)
}

/// An unsigned integer element's value.
fn uint_of(body: &[u8]) -> u64 {
    body.iter()
        .fold(0, |value, &byte| (value << 8) | u64::from(byte))
}

/// A block's track's number, its body's first variable-size integer, and that integer's length.
fn block_track(block: &[u8]) -> io::Result<(u64, usize)> {
    let first = *block.first().ok_or_else(|| invalid("an empty block"))?;
    let length = vint_length(first).ok_or_else(|| invalid("a block's track"))?;
    let bytes = block
        .get(..length)
        .ok_or_else(|| invalid("a block cut short"))?;
    let marker_bits = u64::from(first) & ((1 << (8 - length)) - 1);
    let track = bytes[1..]
        .iter()
        .fold(marker_bits, |value, &byte| (value << 8) | u64::from(byte));
    Ok((track, length))
}

/// `block` with its time from its cluster's set to `offset`.
fn retimed(block: &[u8], offset: i16) -> io::Result<Vec<u8>> {
    let (_, length) = block_track(block)?;
    let mut out = block.to_vec();
    out.get_mut(length..length + 2)
        .ok_or_else(|| invalid("a block cut short"))?
        .copy_from_slice(&offset.to_be_bytes());
    Ok(out)
}

/// A cluster of the remux: its time, and its blocks, each a `SimpleBlock` or a `BlockGroup`
/// element whole, at their microseconds; whether it holds a frame, which the cues point at.
struct Cluster {
    at: u64,
    elements: Vec<u8>,
    video: bool,
}

/// The number in a FLAC frame's header (RFC 9639 §9.1), its first sample's for a stream of
/// variable blocks, its frame's for one of a fixed block, and the frame's samples: `(number,
/// variable, samples)`.
fn flac_frame(packet: &[u8]) -> Option<(u64, bool, u64)> {
    let (&sync, rest) = packet.split_first()?;
    let (&code, rest) = rest.split_first()?;
    if sync != 0xFF || code & 0xFE != 0xF8 {
        return None;
    }
    let variable = code & 1 == 1;
    let (&sizes, rest) = rest.split_first()?;
    let (_, rest) = rest.split_first()?;
    // The number, coded as UTF-8 codes its characters, to 36 bits.
    let (&lead, mut rest) = rest.split_first()?;
    let more = usize::try_from(lead.leading_ones()).ok()?.saturating_sub(1);
    let mut number = u64::from(lead & (0x7F >> more.min(7)));
    for _ in 0..more {
        let (&byte, after) = rest.split_first()?;
        number = (number << 6) | u64::from(byte & 0x3F);
        rest = after;
    }
    let samples = match sizes >> 4 {
        1 => 192,
        n @ 2..=5 => 576 << (n - 2),
        6 => u64::from(*rest.first()?) + 1,
        7 => u64::from(u16::from_be_bytes([*rest.first()?, *rest.get(1)?])) + 1,
        n @ 8..=15 => 256 << (n - 8),
        _ => return None,
    };
    Some((number, variable, samples))
}

/// Makes again the Matroska file `encoded`, FFmpeg's at a millisecond a unit, as `out` at a
/// microsecond a unit (#1227): `timeline` holds each frame's exact time and its event's length,
/// in microseconds; the video track's blocks take them in their order, and so do the text track's
/// events. The info's scale and duration, the clusters (each block within a signed 16-bit offset
/// of its cluster's time), the cues and the seek head are written anew, the packets copied as
/// they are; the tracks and the tags are kept, the CRCs and the voids left out.
pub fn remux(
    encoded: &Path,
    out: &Path,
    timeline: &[(u64, u64)],
    sound: Option<u32>,
) -> io::Result<()> {
    let mut input = BufReader::with_capacity(1 << 20, File::open(encoded)?);
    let mut output = BufWriter::with_capacity(1 << 20, File::create(out)?);
    // The EBML header, as it is.
    let header_id = read_id(&mut input)?.ok_or_else(|| invalid("no header"))?;
    let header_size = read_size(&mut input)?.ok_or_else(|| invalid("the header's size"))?;
    if header_id != EBML {
        return Err(invalid("no EBML header"));
    }
    element_to(&mut output, EBML, &read_body(&mut input, header_size)?)?;
    if read_id(&mut input)? != Some(SEGMENT) {
        return Err(invalid("no segment"));
    }
    let segment_size = read_size(&mut input)?;
    let segment_start = input.stream_position()?;
    // The segment's own elements: its size and the seek head filled in at the end.
    let mut written: Vec<u8> = Vec::new();
    id(&mut written, SEGMENT);
    output.write_all(&written)?;
    let size_at = output.stream_position()?;
    output.write_all(&UNKNOWN_SIZE)?;
    let start = output.stream_position()?;
    let reserved = seek_head(&[(INFO, 0), (TRACKS, 0), (TAGS, 0), (CUES, 0)]);
    output.write_all(&reserved)?;
    let mut positions: Vec<(u32, u64)> = Vec::new();
    let mut tracks: BTreeMap<u64, u64> = BTreeMap::new();
    let mut counts: BTreeMap<u64, usize> = BTreeMap::new();
    let mut cues: Vec<(u64, u64)> = Vec::new();
    let mut current: Option<Cluster> = None;
    // FFmpeg's scale, nanoseconds a unit of its timestamps: 1 ms unless its info says otherwise.
    let mut scale = 1_000_000u64;
    // The sound's FLAC frames' fixed samples, the first's.
    let mut fixed: Option<u64> = None;
    let video = |tracks: &BTreeMap<u64, u64>| {
        tracks
            .iter()
            .find(|&(_, &kind)| kind == TRACK_VIDEO)
            .map(|(&number, _)| number)
    };
    loop {
        let read = input.stream_position()? - segment_start;
        if segment_size.is_some_and(|size| read >= size) {
            break;
        }
        let Some(element_id) = read_id(&mut input)? else {
            break;
        };
        let size = read_size(&mut input)?.ok_or_else(|| invalid("an unknown size"))?;
        let body = read_body(&mut input, size)?;
        match element_id {
            SEEK_HEAD | VOID | CUES | CRC_32 => {}
            INFO => {
                let at = output.stream_position()? - start;
                positions.push((INFO, at));
                let mut info = Vec::new();
                let total = timeline.last().map_or(0, |&(at, lasts)| at + lasts);
                for (child, value) in children(&body)? {
                    match child {
                        TIMESTAMP_SCALE_ID => {
                            scale = uint_of(value).max(1);
                            uint(&mut info, TIMESTAMP_SCALE_ID, TIMESTAMP_SCALE);
                        }
                        DURATION => {
                            #[expect(
                                clippy::cast_precision_loss,
                                reason = "a recording's microseconds, exact in an f64"
                            )]
                            let duration = total as f64;
                            element(&mut info, DURATION, &duration.to_be_bytes());
                        }
                        CRC_32 => {}
                        _ => element(&mut info, child, value),
                    }
                }
                element_to(&mut output, INFO, &info)?;
            }
            TRACKS => {
                let at = output.stream_position()? - start;
                positions.push((TRACKS, at));
                for (child, entry) in children(&body)? {
                    if child != TRACK_ENTRY {
                        continue;
                    }
                    let fields = children(entry)?;
                    let field = |wanted: u32| {
                        fields
                            .iter()
                            .find(|&&(field, _)| field == wanted)
                            .map(|&(_, value)| uint_of(value))
                    };
                    if let (Some(number), Some(kind)) = (field(TRACK_NUMBER), field(TRACK_TYPE)) {
                        tracks.insert(number, kind);
                    }
                }
                element_to(&mut output, TRACKS, &body)?;
            }
            TAGS => {
                let at = output.stream_position()? - start;
                positions.push((TAGS, at));
                element_to(&mut output, TAGS, &body)?;
            }
            CLUSTER => {
                // FFmpeg's time of the cluster, which its blocks' are from.
                let mut from = 0i128;
                for (child, value) in children(&body)? {
                    if child == CLUSTER_TIMESTAMP {
                        from = i128::from(uint_of(value));
                        continue;
                    }
                    let block = match child {
                        SIMPLE_BLOCK => value,
                        BLOCK_GROUP => children(value)?
                            .into_iter()
                            .find(|&(field, _)| field == BLOCK)
                            .map(|(_, block)| block)
                            .ok_or_else(|| invalid("a block group without its block"))?,
                        _ => continue,
                    };
                    let (track, length) = block_track(block)?;
                    let (at, lasts) = if tracks.get(&track) == Some(&TRACK_AUDIO) {
                        // A FLAC frame at its first sample's time.
                        let rate = u128::from(sound.ok_or_else(|| invalid("a sound unasked"))?);
                        let payload = block.get(length + 3..).unwrap_or_default();
                        let (number, variable, samples) =
                            flac_frame(payload).ok_or_else(|| invalid("a FLAC frame"))?;
                        let first = if variable {
                            number
                        } else {
                            number * *fixed.get_or_insert(samples)
                        };
                        let at = (u128::from(first) * 1_000_000 + rate / 2) / rate.max(1);
                        (u64::try_from(at).unwrap_or(u64::MAX), 0)
                    } else {
                        let index = counts.entry(track).or_insert(0);
                        let &time = timeline
                            .get(*index)
                            .ok_or_else(|| invalid("more blocks than frames"))?;
                        *index += 1;
                        time
                    };
                    // FFmpeg's time of the block, rounded: within a unit of the frame's own.
                    let (_, length) = block_track(block)?;
                    let offset = block
                        .get(length..length + 2)
                        .ok_or_else(|| invalid("a block cut short"))?;
                    let rounded = (from + i128::from(i16::from_be_bytes([offset[0], offset[1]])))
                        * i128::from(scale)
                        / 1_000;
                    if (rounded - i128::from(at)).unsigned_abs() > u128::from(scale / 1_000) {
                        return Err(invalid("a block far from its frame's time"));
                    }
                    // A cluster a frame, at the frame's time: its video's block opens it.
                    let opens = current.as_ref().is_none_or(|cluster| {
                        Some(track) == video(&tracks) || at.abs_diff(cluster.at) > 32_767
                    });
                    if opens {
                        if let Some(cluster) = current.take() {
                            write_cluster(&mut output, start, cluster, &mut cues, &tracks)?;
                        }
                        current = Some(Cluster {
                            at,
                            elements: Vec::new(),
                            video: false,
                        });
                    }
                    let Some(cluster) = current.as_mut() else {
                        continue;
                    };
                    let offset = i16::try_from(i128::from(at) - i128::from(cluster.at))
                        .map_err(|_| invalid("a block far from its cluster"))?;
                    cluster.video |= Some(track) == video(&tracks);
                    match child {
                        SIMPLE_BLOCK => {
                            element(
                                &mut cluster.elements,
                                SIMPLE_BLOCK,
                                &retimed(value, offset)?,
                            );
                        }
                        _ => {
                            let mut group = Vec::new();
                            for (field, inside) in children(value)? {
                                match field {
                                    BLOCK => element(&mut group, BLOCK, &retimed(inside, offset)?),
                                    BLOCK_DURATION => uint(&mut group, BLOCK_DURATION, lasts),
                                    CRC_32 => {}
                                    _ => element(&mut group, field, inside),
                                }
                            }
                            element(&mut cluster.elements, BLOCK_GROUP, &group);
                        }
                    }
                }
            }
            _ => element_to(&mut output, element_id, &body)?,
        }
    }
    if let Some(cluster) = current.take() {
        write_cluster(&mut output, start, cluster, &mut cues, &tracks)?;
    }
    // The cues: each frame's time and its cluster.
    positions.push((CUES, output.stream_position()? - start));
    let track = video(&tracks).unwrap_or(VIDEO);
    let mut points = Vec::new();
    for (time, position) in cues {
        let mut place = Vec::new();
        uint(&mut place, CUE_TRACK, track);
        uint(&mut place, CUE_CLUSTER_POSITION, position);
        let mut point = Vec::new();
        uint(&mut point, CUE_TIME, time);
        element(&mut point, CUE_TRACK_POSITIONS, &place);
        element(&mut points, CUE_POINT, &point);
    }
    element_to(&mut output, CUES, &points)?;
    let end = output.stream_position()?;
    // The seek head and the segment's size, now known; a void fills the place of an element
    // missing, the tags'.
    let mut head = seek_head(&positions);
    let room = reserved.len().saturating_sub(head.len());
    if room > 0 {
        element(&mut head, VOID, &vec![0; room.saturating_sub(9)]);
    }
    output.seek(SeekFrom::Start(start))?;
    output.write_all(&head)?;
    output.seek(SeekFrom::Start(size_at))?;
    output.write_all(&((end - start) | 0x0100_0000_0000_0000).to_be_bytes())?;
    output.flush()?;
    Ok(())
}

/// An element of `body` written to `output`.
fn element_to(output: &mut impl Write, element_id: u32, body: &[u8]) -> io::Result<()> {
    let mut out = Vec::with_capacity(body.len() + 12);
    element(&mut out, element_id, body);
    output.write_all(&out)
}

/// A cluster of the remux written to `output`, its video's time and place among the cues.
fn write_cluster(
    output: &mut (impl Write + Seek),
    start: u64,
    cluster: Cluster,
    cues: &mut Vec<(u64, u64)>,
    tracks: &BTreeMap<u64, u64>,
) -> io::Result<()> {
    let position = output.stream_position()? - start;
    let mut body = Vec::with_capacity(cluster.elements.len() + 16);
    uint(&mut body, CLUSTER_TIMESTAMP, cluster.at);
    body.extend_from_slice(&cluster.elements);
    if cluster.video && tracks.values().any(|&kind| kind == TRACK_VIDEO) {
        cues.push((cluster.at, position));
    }
    element_to(output, CLUSTER, &body)
}

/// A seek head of `positions`, the elements' places from the segment's start, of a size that
/// the places do not change: each in eight bytes.
fn seek_head(positions: &[(u32, u64)]) -> Vec<u8> {
    let mut seeks = Vec::new();
    for &(element_id, position) in positions {
        let mut seek = Vec::new();
        let mut target = Vec::new();
        id(&mut target, element_id);
        element(&mut seek, SEEK_ID, &target);
        element(&mut seek, SEEK_POSITION, &position.to_be_bytes());
        element(&mut seeks, SEEK, &seek);
    }
    let mut out = Vec::new();
    element(&mut out, SEEK_HEAD, &seeks);
    out
}

/// Whether `path` names a Matroska file, to record into rather than a directory of PNGs.
pub fn is_matroska(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mkv"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_elements_id_and_size_are_its_bytes() {
        let mut out = Vec::new();
        uint(&mut out, TRACK_NUMBER, 2);
        assert_eq!(out, [0xD7, 0x01, 0, 0, 0, 0, 0, 0, 1, 2]);
        let mut out = Vec::new();
        uint(&mut out, TIMESTAMP_SCALE_ID, 1_000);
        assert_eq!(&out[..3], [0x2A, 0xD7, 0xB1]);
        assert_eq!(&out[3..], [0x01, 0, 0, 0, 0, 0, 0, 2, 0x03, 0xE8]);
    }

    #[test]
    fn a_frames_bitmap_header_tells_its_rows_from_the_top() {
        let header = bitmap_info(4, 3);
        assert_eq!(header.len(), 40);
        assert_eq!(i32::from_le_bytes(header[8..12].try_into().unwrap()), -3);
        assert_eq!(u16::from_le_bytes(header[14..16].try_into().unwrap()), 32);
        assert_eq!(u32::from_le_bytes(header[20..24].try_into().unwrap()), 48);
    }

    #[test]
    fn a_cluster_holds_its_frame_and_its_times() {
        let frame = Frame {
            times: Times {
                n: 3,
                frame: 90,
                real: 1.5,
                real_delta: 0.016,
                r#virtual: 1.25,
                virtual_delta: 0.016,
                paused: false,
            },
            width: 1,
            height: 1,
            bgra: vec![1, 2, 3, 255],
        };
        let out = cluster(1_500_000, 16_000, &frame);
        assert_eq!(&out[..4], CLUSTER.to_be_bytes());
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("\"n\":3"), "{text}");
        assert!(text.contains("\"virtual\":1.25"), "{text}");
        // The frame's pixels in its block, after the track's number, the time and the flags.
        let pixels = [0x81, 0, 0, 0x80, 1, 2, 3, 255];
        assert!(out.windows(pixels.len()).any(|w| w == pixels));
    }

    /// A Matroska file as FFmpeg writes one: a millisecond a unit, a CRC in every cluster, a seek
    /// head, a void and cues of its own; three frames whose exact times `exact` round to its
    /// milliseconds, each a video block of `pixels(n)` and an event of `event(n)`.
    fn ffmpeg_like(
        exact: &[u64],
        pixels: impl Fn(usize) -> Vec<u8>,
        event: impl Fn(usize) -> Vec<u8>,
    ) -> Vec<u8> {
        let mut segment = Vec::new();
        element(&mut segment, SEEK_HEAD, &[0xAA; 12]);
        element(&mut segment, VOID, &[0; 20]);
        let mut info = Vec::new();
        element(&mut info, CRC_32, &[1, 2, 3, 4]);
        uint(&mut info, TIMESTAMP_SCALE_ID, 1_000_000);
        element(&mut info, DURATION, &120.0f64.to_be_bytes());
        text(&mut info, MUXING_APP, "Lavf");
        element(&mut segment, INFO, &info);
        let mut tracks = Vec::new();
        for (number, kind) in [(1, TRACK_VIDEO), (2, TRACK_SUBTITLE)] {
            let mut entry = Vec::new();
            uint(&mut entry, TRACK_NUMBER, number);
            uint(&mut entry, TRACK_TYPE, kind);
            element(&mut tracks, TRACK_ENTRY, &entry);
        }
        element(&mut segment, TRACKS, &tracks);
        element(&mut segment, TAGS, b"tags as they are");
        for (n, &at) in exact.iter().enumerate() {
            let ms = (at + 500) / 1_000;
            let mut cluster = Vec::new();
            element(&mut cluster, CRC_32, &[9, 9, 9, 9]);
            uint(&mut cluster, CLUSTER_TIMESTAMP, ms);
            let mut video = vec![0x81, 0, 0, 0x80];
            video.extend_from_slice(&pixels(n));
            element(&mut cluster, SIMPLE_BLOCK, &video);
            let mut block = vec![0x82, 0, 0, 0];
            block.extend_from_slice(&event(n));
            let mut group = Vec::new();
            element(&mut group, BLOCK, &block);
            uint(&mut group, BLOCK_DURATION, 16);
            element(&mut cluster, BLOCK_GROUP, &group);
            element(&mut segment, CLUSTER, &cluster);
        }
        element(&mut segment, CUES, &[0xCC; 8]);
        let mut file = Vec::new();
        let mut header = Vec::new();
        text(&mut header, DOC_TYPE, "matroska");
        element(&mut file, EBML, &header);
        element(&mut file, SEGMENT, &segment);
        file
    }

    #[test]
    fn the_remux_puts_each_block_at_its_frames_microsecond() {
        let dir = std::env::temp_dir().join(format!("opensacred-remux-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (encoded, out) = (dir.join("rec.ffmpeg.mkv"), dir.join("rec.mkv"));
        let exact = [0u64, 16_412, 33_901];
        let lasts = [16_412u64, 17_489, 16_000];
        let pixels = |n: usize| vec![u8::try_from(n).unwrap(); 64];
        let event = |n: usize| format!("{{\"n\":{n}}}").into_bytes();
        std::fs::write(&encoded, ffmpeg_like(&exact, pixels, event)).unwrap();
        let timeline: Vec<(u64, u64)> = exact.iter().copied().zip(lasts).collect();
        remux(&encoded, &out, &timeline, None).unwrap();
        let file = std::fs::read(&out).unwrap();
        let top = children(&file).unwrap();
        assert_eq!(
            top.iter().map(|&(id, _)| id).collect::<Vec<_>>(),
            [EBML, SEGMENT]
        );
        let segment_body = top[1].1;
        let segment = children(segment_body).unwrap();
        let ids: Vec<u32> = segment.iter().map(|&(id, _)| id).collect();
        assert_eq!(
            ids,
            [
                SEEK_HEAD, INFO, TRACKS, TAGS, CLUSTER, CLUSTER, CLUSTER, CUES
            ]
        );
        // The scale a microsecond, the duration in it, the CRC left out.
        let info = children(segment[1].1).unwrap();
        assert!(info.contains(&(TIMESTAMP_SCALE_ID, &[0x03, 0xE8][..])));
        let duration = info.iter().find(|&&(id, _)| id == DURATION).unwrap().1;
        assert_eq!(f64::from_be_bytes(duration.try_into().unwrap()), 49_901.0);
        assert!(info.iter().all(|&(id, _)| id != CRC_32));
        assert_eq!(segment[3].1, b"tags as they are");
        // Each frame in a cluster of its own at its exact time, its packets as they were.
        for (n, &(_, cluster)) in segment[4..7].iter().enumerate() {
            let parts = children(cluster).unwrap();
            assert_eq!(parts[0].0, CLUSTER_TIMESTAMP);
            assert_eq!(uint_of(parts[0].1), exact[n]);
            let video = parts.iter().find(|&&(id, _)| id == SIMPLE_BLOCK).unwrap().1;
            assert_eq!(&video[..4], [0x81, 0, 0, 0x80]);
            assert_eq!(video[4..], pixels(n)[..]);
            let group = parts.iter().find(|&&(id, _)| id == BLOCK_GROUP).unwrap().1;
            let fields = children(group).unwrap();
            let block = fields.iter().find(|&&(id, _)| id == BLOCK).unwrap().1;
            assert_eq!(&block[1..3], [0, 0]);
            assert_eq!(block[4..], event(n)[..]);
            let lasting = fields
                .iter()
                .find(|&&(id, _)| id == BLOCK_DURATION)
                .unwrap()
                .1;
            assert_eq!(uint_of(lasting), lasts[n]);
        }
        // The cues point at the clusters, and the seek head at the elements it names.
        let start = segment_body.as_ptr() as usize;
        let cues = children(segment[7].1).unwrap();
        assert_eq!(cues.len(), 3);
        for (n, &(_, point)) in cues.iter().enumerate() {
            let fields = children(point).unwrap();
            assert_eq!(uint_of(fields[0].1), exact[n]);
            let place = children(fields[1].1).unwrap();
            let position = usize::try_from(uint_of(place[1].1)).unwrap();
            let at = segment[4 + n].1.as_ptr() as usize - start;
            // The cluster's head, its ID and its eight bytes of size, before its body.
            assert_eq!(position + 12, at);
        }
        for (_, seek) in children(segment[0].1).unwrap() {
            let fields = children(seek).unwrap();
            let target = u32::try_from(uint_of(fields[0].1)).unwrap();
            let position = usize::try_from(uint_of(fields[1].1)).unwrap();
            let mut at = &segment_body[position..];
            assert_eq!(read_id(&mut at).unwrap(), Some(target));
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_block_far_from_its_frames_time_fails_the_remux() {
        let dir = std::env::temp_dir().join(format!("opensacred-remux-far-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (encoded, out) = (dir.join("rec.ffmpeg.mkv"), dir.join("rec.mkv"));
        let file = ffmpeg_like(&[0, 16_000, 33_000], |_| vec![0; 4], |_| vec![b'x']);
        std::fs::write(&encoded, file).unwrap();
        // A frame 5 ms off the one FFmpeg wrote: the blocks are not the frames'.
        let timeline = [(0, 16_000), (21_000, 12_000), (33_000, 16_000)];
        assert!(remux(&encoded, &out, &timeline, None).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_flac_frames_header_tells_its_number_and_samples() {
        // A fixed block of 4,608 samples, frame 1.
        assert_eq!(
            flac_frame(&[0xFF, 0xF8, 0x59, 0x08, 0x01, 0]),
            Some((1, false, 4608))
        );
        // A variable one, its first sample 128 in two bytes, its 300 samples after the number.
        let header = [0xFF, 0xF9, 0x79, 0x08, 0xC2, 0x80, 0x01, 0x2B];
        assert_eq!(flac_frame(&header), Some((128, true, 300)));
        assert_eq!(flac_frame(&[0x12, 0x34, 0, 0, 0]), None);
    }

    /// `Time<Real>`'s start for the tests of the sound's placing.
    static BASE: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

    /// A track of the sound `frames` at 1,000 frames a second whose mix began `began` seconds after
    /// [`BASE`].
    fn track_of(frames: &[kira::Frame], began: fn() -> Option<Instant>) -> SoundTrack {
        let (mut producer, consumer) = rtrb::RingBuffer::new(frames.len().max(1));
        for &frame in frames {
            producer.push(frame).unwrap();
        }
        let mut track = SoundTrack::new(Sound::of(consumer, 1000), *BASE.get_or_init(Instant::now));
        track.started = began;
        track
    }

    #[test]
    fn the_sound_starts_with_the_recordings_first_frame() {
        let frames: Vec<kira::Frame> = (1..=50u16)
            .map(|n| kira::Frame::from_mono(f32::from(n)))
            .collect();
        // The mix began 10 ms after the first frame, at 10 s: 10 frames of silence before it.
        let mut later = track_of(&frames, || {
            Some(*BASE.get_or_init(Instant::now) + Duration::from_millis(10_010))
        });
        later.gather(10.0);
        assert_eq!(later.waiting.len(), 60);
        assert!(
            later.waiting[..10]
                .iter()
                .all(|&frame| frame == kira::Frame::ZERO)
        );
        assert_eq!(later.waiting[10].left, 1.0);
        // It began 5 ms before: its first 5 frames left out.
        let mut earlier = track_of(&frames, || {
            Some(*BASE.get_or_init(Instant::now) + Duration::from_millis(9_995))
        });
        earlier.gather(10.0);
        assert_eq!(earlier.waiting.len(), 45);
        assert_eq!(earlier.waiting[0].left, 6.0);
        // To the last frame's end, 40 ms: the rest left out, each block at its first frame's time.
        let blocks = earlier.rest(40_000, 10.0);
        assert_eq!(earlier.written, 40);
        assert_eq!(&blocks[..4], CLUSTER.to_be_bytes());
    }

    #[test]
    fn a_path_ending_in_mkv_records_into_matroska() {
        assert!(is_matroska(Path::new("rec.mkv")));
        assert!(is_matroska(Path::new("C:/x/REC.MKV")));
        assert!(!is_matroska(Path::new("frames")));
    }

    /// `ffprobe`'s lines for `path` with `args` and the entries `entries`.
    fn probe(path: &Path, args: &[&str], entries: &str) -> String {
        let probe = ffmpeg().with_file_name(format!("ffprobe{}", std::env::consts::EXE_SUFFIX));
        let out = Command::new(probe)
            .args(["-v", "error"])
            .args(args)
            .arg(entries)
            .arg(path)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n")
    }

    /// Frames of known pixels and times through the writer and FFmpeg, and back: the pixels, the
    /// count and the text track's times (#1226), and a sound with them (#1230).
    #[test]
    #[ignore = "needs FFmpeg"]
    fn frames_come_back_from_ffmpeg_as_they_went_in() {
        let dir = std::env::temp_dir().join(format!("opensacred-mkv-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rec.mkv");
        let (width, height) = (64, 48);
        let pixels = |n: u32| -> Vec<u8> {
            (0..width * height)
                .flat_map(|i| {
                    let (x, y) = (i % width, i / width);
                    let v = |k: u32| u8::try_from((k * 37 + n * 11) % 256).unwrap();
                    [v(x), v(y), v(x + y), 255]
                })
                .collect()
        };
        // Uneven times to the microsecond, as a game's frames are.
        let times = [0.0, 0.016_312, 0.050_407, 0.051_123, 0.120_996];
        // A sound at 48 kHz, its mix begun with the first frame, at 10 s of `Time<Real>`.
        let rate = 48_000u32;
        let sound: Vec<kira::Frame> = (0..rate / 5)
            .map(|n| {
                let t = f64::from(n) / f64::from(rate);
                #[expect(clippy::cast_possible_truncation, reason = "a sample in f32")]
                let v = (t * 440.0 * std::f64::consts::TAU).sin() as f32 * 0.5;
                kira::Frame { left: v, right: -v }
            })
            .collect();
        let (mut producer, consumer) = rtrb::RingBuffer::new(sound.len());
        for &frame in &sound {
            producer.push(frame).unwrap();
        }
        let mut track = SoundTrack::new(Sound::of(consumer, rate), *BASE.get_or_init(Instant::now));
        track.started = || Some(*BASE.get_or_init(Instant::now) + Duration::from_secs(10));
        let mut encoder = Encoder::new(
            path.clone(),
            vec![("class".into(), "mage".into())],
            Some(track),
        );
        // Out of their order, as captures may come.
        for n in [1u32, 0, 2, 4, 3] {
            let at = times[usize::try_from(n).unwrap()];
            encoder.send(Frame {
                times: Times {
                    n,
                    frame: 100 + n,
                    real: 10.0 + at,
                    real_delta: 0.016,
                    r#virtual: at,
                    virtual_delta: 0.016,
                    paused: false,
                },
                width,
                height,
                bgra: pixels(n),
            });
        }
        encoder.finish();
        let done = loop {
            if let Some(done) = encoder.done() {
                break done;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        assert_eq!(done, Ok(()));
        // The frames back as BGRA, every one.
        let raw = Command::new(ffmpeg())
            .args(["-v", "error", "-i"])
            .arg(&path)
            .args(["-map", "0:v", "-fps_mode", "passthrough", "-f", "rawvideo"])
            .args(["-pix_fmt", "bgra", "-"])
            .output()
            .unwrap();
        assert!(
            raw.status.success(),
            "{}",
            String::from_utf8_lossy(&raw.stderr)
        );
        let size = usize::try_from(width * height * 4).unwrap();
        assert_eq!(raw.stdout.len(), 5 * size);
        for n in 0..5u32 {
            let at = usize::try_from(n).unwrap() * size;
            assert!(raw.stdout[at..at + size] == pixels(n)[..], "frame {n}");
        }
        // The text track's events, one a frame, in their order, with the times as they were.
        let events = Command::new(ffmpeg())
            .args(["-v", "error", "-i"])
            .arg(&path)
            .args(["-map", "0:s", "-f", "srt", "-"])
            .output()
            .unwrap();
        let srt = String::from_utf8_lossy(&events.stdout);
        assert_eq!(srt.matches("\"n\":").count(), 5, "{srt}");
        assert!(srt.contains("\"real\":10.051"), "{srt}");
        let tags = Command::new(ffmpeg())
            .args(["-v", "error", "-i"])
            .arg(&path)
            .args(["-f", "ffmetadata", "-"])
            .output()
            .unwrap();
        // Matroska's tags' names in capitals, as FFmpeg writes them.
        assert!(String::from_utf8_lossy(&tags.stdout).contains("CLASS=mage"));
        // The frames' packets at their times to the microsecond, after the remux (#1227, #2616).
        let packets = Command::new(
            ffmpeg().with_file_name(format!("ffprobe{}", std::env::consts::EXE_SUFFIX)),
        )
        .args([
            "-v",
            "error",
            "-select_streams",
            "v",
            "-show_entries",
            "packet=pts_time",
        ])
        .args(["-of", "csv=p=0"])
        .arg(&path)
        .output()
        .unwrap();
        let read: Vec<f64> = String::from_utf8_lossy(&packets.stdout)
            .lines()
            .map(|line| line.trim().parse().unwrap())
            .collect();
        assert_eq!(read, times);
        assert!(!encoded_path(&path).exists());
        // The sound in FLAC at its rate, from the first frame to the end of the last (its
        // 0.016 s), its samples as they went in to 24 bits, its packets at their first samples.
        let stream = probe(
            &path,
            &["-select_streams", "a", "-show_entries"],
            "stream=codec_name,sample_rate,channels",
        );
        assert!(stream.contains("codec_name=flac"), "{stream}");
        assert!(stream.contains("sample_rate=48000"), "{stream}");
        let decoded = Command::new(ffmpeg())
            .args(["-v", "error", "-i"])
            .arg(&path)
            .args(["-map", "0:a", "-f", "f32le", "-"])
            .output()
            .unwrap();
        let samples: Vec<f32> = decoded
            .stdout
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&bytes| f32::from_le_bytes(bytes))
            .collect();
        let total = usize::try_from((120_996 + 16_000) * u64::from(rate) / 1_000_000).unwrap();
        assert_eq!(samples.len(), 2 * total);
        for (n, pair) in samples.as_chunks::<2>().0.iter().enumerate() {
            assert!((pair[0] - sound[n].left).abs() < 1e-6, "sample {n}");
            assert!((pair[1] - sound[n].right).abs() < 1e-6, "sample {n}");
        }
        let packets = probe(
            &path,
            &["-select_streams", "a", "-show_entries"],
            "packet=pts_time",
        );
        let first: Vec<&str> = packets
            .lines()
            .filter(|line| line.starts_with("pts_time="))
            .take(2)
            .collect();
        // FFmpeg's FLAC frames as long as the blocks that came in, 1,024 samples.
        assert_eq!(
            first,
            ["pts_time=0.000000", "pts_time=0.021333"],
            "{packets}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
