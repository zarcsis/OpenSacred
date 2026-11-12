//! Recording into one Matroska file (#1225, #1226): `--record FILE.mkv` and `record FILE.mkv N`.
//! The game writes a Matroska stream of its own into FFmpeg's standard input, its frames as raw
//! BGRA in Matroska's VFW mode, each at the real time it was made at, and a text track with each
//! frame's times; FFmpeg encodes the frames losslessly in FFV1 (`bgr0`, a variable frame rate,
//! none dropped or repeated), keeps the text track and tags the file with the run's details. The
//! frames go through a thread of their own, so that neither the pipe nor FFmpeg holds up a frame
//! of the game. FFmpeg is found as the converter finds it: beside the game, else `FFMPEG`, else
//! `ffmpeg` from `PATH`; without it the recording fails with an error that says so, and the game
//! goes on. FFmpeg's Matroska muxer rounds the blocks' times to 1 ms; the text track keeps them
//! exactly (docs/10, the command `record`; docs/05, "Looking at frames").

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use serde::Serialize;

/// The Matroska stream's time scale: nanoseconds a unit of its timestamps, a microsecond.
const TIMESTAMP_SCALE: u64 = 1_000;
/// The tracks: the frames, then their times.
const VIDEO: u64 = 1;
const TEXT: u64 = 2;
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
const PIXEL_WIDTH: u32 = 0xB0;
const PIXEL_HEIGHT: u32 = 0xBA;
const CLUSTER: u32 = 0x1F43_B675;
const CLUSTER_TIMESTAMP: u32 = 0xE7;
const SIMPLE_BLOCK: u32 = 0xA3;
const BLOCK_GROUP: u32 = 0xA0;
const BLOCK: u32 = 0xA1;
const BLOCK_DURATION: u32 = 0x9B;
/// A track's types: video and subtitles.
const TRACK_VIDEO: u64 = 1;
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
/// `width` × `height` pixels.
pub fn head(width: u32, height: u32) -> Vec<u8> {
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
    element(&mut out, TRACKS, &tracks);
    out
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
fn command(out: &Path, tags: &[(String, String)]) -> Command {
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
    /// A recording into `path`, tagged with `tags`; FFmpeg starts with its first frame.
    pub fn new(path: PathBuf, tags: Vec<(String, String)>) -> Self {
        let (frames, received) = channel();
        let shared = Arc::new(Shared::default());
        let thread_shared = Arc::clone(&shared);
        std::thread::spawn(move || {
            let result = write(&path, &tags, &received);
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
fn write(out: &Path, tags: &[(String, String)], received: &Receiver<Frame>) -> Result<(), String> {
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
                if let Err(error) = put(out, tags, &mut start, &mut pipe, previous, Some(lasts)) {
                    failed = Some(error);
                }
            }
            pending = Some(frame);
        }
    }
    if let Some(previous) = pending.take()
        && failed.is_none()
        && let Err(error) = put(out, tags, &mut start, &mut pipe, previous, None)
    {
        failed = Some(error);
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
/// `lasts` seconds, else its own delta; `start` is the first frame's real time.
fn put(
    out: &Path,
    tags: &[(String, String)],
    start: &mut Option<f64>,
    pipe: &mut Option<(Child, ChildStdin)>,
    frame: Frame,
    lasts: Option<f64>,
) -> Result<(), String> {
    if pipe.is_none() {
        let mut child = command(out, tags).spawn().map_err(|e| {
            format!(
                "starting {} for the recording into {}: {e} (FFmpeg is found beside the game, else \
                 by FFMPEG, else on PATH)",
                ffmpeg().display(),
                out.display()
            )
        })?;
        let mut stdin = child.stdin.take().ok_or("no FFmpeg's standard input")?;
        stdin
            .write_all(&head(frame.width, frame.height))
            .map_err(|e| format!("writing to FFmpeg: {e}"))?;
        *pipe = Some((child, stdin));
    }
    let first = *start.get_or_insert(frame.times.real);
    let at = micros(frame.times.real - first);
    let lasts = micros(lasts.unwrap_or(frame.times.real_delta));
    if let Some((_, stdin)) = pipe.as_mut() {
        stdin
            .write_all(&cluster(at, lasts, &frame))
            .map_err(|e| format!("writing to FFmpeg: {e}"))?;
    }
    Ok(())
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

    #[test]
    fn a_path_ending_in_mkv_records_into_matroska() {
        assert!(is_matroska(Path::new("rec.mkv")));
        assert!(is_matroska(Path::new("C:/x/REC.MKV")));
        assert!(!is_matroska(Path::new("frames")));
    }

    /// Frames of known pixels and times through the writer and FFmpeg, and back: the pixels, the
    /// count and the text track's times (#1226).
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
        // Uneven times, as a game's frames are.
        let times = [0.0, 0.016, 0.050, 0.051, 0.120];
        let mut encoder = Encoder::new(path.clone(), vec![("class".into(), "mage".into())]);
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
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
