//! `--screenshot`: once the hero animates and every sector near it is in place, render a few
//! more frames, save one to a PNG and exit. The first step toward the screenshot tests of M2.
//! The command `screenshot` saves the next frame at once ([`save`]), and `record` records as
//! `--record` does, going on after it (`command`). A recording into a path ending in `.mkv` goes
//! into one Matroska file through FFmpeg, its frames' times with them (`mkv`, #1226).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use bevy::diagnostic::FrameCount;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::window::PrimaryWindow;
use opensacred_data::world::SIDE;

use crate::frame::Playing;
use crate::hero::{Body, Ready};
use crate::mkv::{self, Encoder, Frame, SoundTrack, Times};
use crate::sectors::Streamer;
use crate::{game, iso};

#[derive(Resource)]
pub struct Request {
    pub path: PathBuf,
    /// Frames to render after the scene is ready, so that the animation and the textures settle.
    settle: u32,
    frames: u32,
    taken: bool,
}

impl Request {
    pub fn new(path: PathBuf, settle: u32) -> Self {
        Self {
            path,
            settle,
            frames: 0,
            taken: false,
        }
    }
}

pub fn take(
    mut commands: Commands,
    mut request: ResMut<Request>,
    ready: Res<Ready>,
    streamer: Res<Streamer>,
    bodies: Query<&Transform, With<Body>>,
) {
    let settled = bodies
        .iter()
        .next()
        .is_some_and(|hero| streamer.settled(hero.translation));
    if request.taken || !ready.0 || !settled {
        return;
    }
    request.frames += 1;
    if request.frames < request.settle {
        return;
    }
    request.taken = true;
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(request.path.clone()))
        .observe(
            |_: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
                exit.write(AppExit::Success);
            },
        );
}

/// The screenshots of [`save`] asked for and not written yet.
#[derive(Resource, Default)]
pub struct Saving(Arc<AtomicU32>);

impl Saving {
    /// Whether a screenshot asked for is still to be written.
    pub fn pending(&self) -> bool {
        self.0.load(Ordering::Acquire) > 0
    }
}

/// Saves the next frame of the window to `path`, as it is; [`Saving`] counts it until its file
/// is written, a frame or two later, so that a script waits for it (#202).
pub fn save(world: &mut World, path: PathBuf) {
    let saving = Arc::clone(&world.get_resource_or_init::<Saving>().0);
    saving.fetch_add(1, Ordering::AcqRel);
    let mut write = save_to_disk(path);
    world
        .spawn(Screenshot::primary_window())
        .observe(move |captured: On<ScreenshotCaptured>| {
            write(captured);
            saving.fetch_sub(1, Ordering::AcqRel);
        });
}

/// `--record`: from the same moment as a screenshot, save every frame to a numbered PNG for a
/// number of frames, then exit, to look at flicker or a stutter frame by frame; the command
/// `record` goes on after its frames, and in a menu records from the next frame (#2491). Into a
/// path ending in `.mkv`, the frames go into one Matroska file instead (`mkv`, #1226).
#[derive(Resource)]
pub struct Recording {
    dir: PathBuf,
    /// The recording into Matroska, from its first frame on.
    encoder: Option<Encoder>,
    settle: u32,
    frames: u32,
    /// Whether it waits for the game's scene to be ready before its first frame.
    waits: bool,
    /// Frames rendered since the scene was ready.
    rendered: u32,
    /// Frames saved, which the last one's capture counts down.
    saved: std::sync::Arc<std::sync::atomic::AtomicU32>,
    /// Whether the process ends with the recording.
    exit: bool,
}

impl Recording {
    pub fn new(dir: PathBuf, settle: u32, frames: u32, exit: bool) -> Self {
        Self {
            dir,
            encoder: None,
            settle,
            frames,
            waits: true,
            rendered: 0,
            saved: std::sync::Arc::default(),
            exit,
        }
    }

    /// The recording from the next frame, with no scene to wait for: a menu's.
    pub fn now(self) -> Self {
        Self {
            waits: false,
            ..self
        }
    }
}

/// The directory that a recording into `path` needs: the PNGs', or a Matroska file's parent.
pub fn recording_dir(path: &std::path::Path) -> &std::path::Path {
    if mkv::is_matroska(path) {
        path.parent().unwrap_or(std::path::Path::new("."))
    } else {
        path
    }
}

/// The file of frame `n` of a recording.
fn frame_file(dir: &std::path::Path, n: u32) -> PathBuf {
    dir.join(format!("frame-{n:04}.png"))
}

/// The number in the recording of the frame rendered `rendered`-th since the scene was ready
/// (from 1): from 0 at the `settle`-th, the frame a screenshot takes ([`take`]), or at the first
/// when `settle` is 0; `None` before it.
fn frame_number(rendered: u32, settle: u32) -> Option<u32> {
    rendered.checked_sub(settle.max(1))
}

/// Seconds of the mix that may wait for a recording's thread, which takes it as it writes each
/// frame.
const SOUND_ROOM: f64 = 10.0;

/// The clocks whose times go with a frame recorded into Matroska.
type Clocks<'w> = (
    Res<'w, Time<Real>>,
    Res<'w, Time<Virtual>>,
    Option<Res<'w, FrameCount>>,
);

/// The tags of a recording into Matroska: the game's version, the hero's class and sector, the
/// window's size, the frames of settling and the command line.
fn tags(
    class: Option<game::Class>,
    hero: Option<Vec3>,
    window: Option<(u32, u32)>,
    settle: u32,
) -> Vec<(String, String)> {
    let mut tags = vec![("version".to_owned(), env!("OPENSACRED_VERSION").to_owned())];
    tags.push((
        "class".to_owned(),
        format!("{:?}", class.unwrap_or_default()).to_lowercase(),
    ));
    if let Some(at) = hero {
        let side = f32::from(u16::try_from(SIDE).unwrap_or(u16::MAX)) * iso::CELL;
        let sector = ((at.x / side).floor(), (at.z / side).floor());
        tags.push(("sector".to_owned(), format!("{},{}", sector.0, sector.1)));
    }
    if let Some((width, height)) = window {
        tags.push(("window".to_owned(), format!("{width}x{height}")));
    }
    tags.push(("settle".to_owned(), settle.to_string()));
    let line: Vec<String> = std::env::args().collect();
    tags.push(("command_line".to_owned(), line.join(" ")));
    tags
}

/// The pixels of a frame's capture as BGRA rows from the top, if its format is one of the
/// window's.
fn bgra(image: &Image) -> Option<Vec<u8>> {
    let mut data = image.data.clone()?;
    match image.texture_descriptor.format {
        TextureFormat::Bgra8UnormSrgb | TextureFormat::Bgra8Unorm => {}
        TextureFormat::Rgba8UnormSrgb | TextureFormat::Rgba8Unorm => {
            for texel in data.as_chunks_mut::<4>().0 {
                texel.swap(0, 2);
            }
        }
        _ => return None,
    }
    Some(data)
}

#[expect(
    clippy::too_many_arguments,
    reason = "a system's parameters: the recording, the scene's state and the run's details"
)]
pub fn record(
    mut commands: Commands,
    mut recording: ResMut<Recording>,
    (ready, streamer): (Option<Res<Ready>>, Option<Res<Streamer>>),
    bodies: Query<&Transform, With<Body>>,
    mut exit: MessageWriter<AppExit>,
    (real, virtual_time, count): Clocks,
    request: Option<Res<game::Request>>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    if recording.saved.load(std::sync::atomic::Ordering::Acquire) >= recording.frames {
        // Into Matroska: the stream closed after the last frame, and FFmpeg's end waited for.
        if let Some(encoder) = recording.encoder.as_mut() {
            encoder.finish();
            match encoder.done() {
                None => return,
                Some(Ok(())) => info!(
                    "recorded {} frames to {}",
                    recording.frames,
                    recording.dir.display()
                ),
                Some(Err(error)) => {
                    error!("the recording into {}: {error}", recording.dir.display())
                }
            }
        }
        if recording.exit {
            exit.write(AppExit::Success);
        } else {
            if recording.encoder.is_none() {
                info!(
                    "recorded {} frames to {}",
                    recording.frames,
                    recording.dir.display()
                );
            }
            commands.remove_resource::<Recording>();
        }
        return;
    }
    if recording.waits {
        let settled = bodies.iter().next().is_some_and(|hero| {
            streamer
                .as_deref()
                .is_some_and(|streamer| streamer.settled(hero.translation))
        });
        if !ready.is_some_and(|ready| ready.0) || (recording.rendered == 0 && !settled) {
            return;
        }
    }
    recording.rendered += 1;
    let Some(n) = frame_number(recording.rendered, recording.settle) else {
        return;
    };
    if n >= recording.frames {
        return;
    }
    let saved = recording.saved.clone();
    if !mkv::is_matroska(&recording.dir) {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(frame_file(&recording.dir, n)))
            .observe(move |_: On<ScreenshotCaptured>| {
                saved.fetch_add(1, std::sync::atomic::Ordering::Release);
            });
        return;
    }
    // The frame's times as its screenshot is taken, which go with its pixels.
    let times = Times {
        n,
        frame: count.map_or(0, |count| count.0),
        real: real.elapsed_secs_f64(),
        real_delta: real.delta_secs_f64(),
        r#virtual: virtual_time.elapsed_secs_f64(),
        virtual_delta: virtual_time.delta_secs_f64(),
        paused: virtual_time.is_paused(),
    };
    if recording.encoder.is_none() {
        let window = windows
            .iter()
            .next()
            .map(|window| (window.physical_width(), window.physical_height()));
        let hero = bodies.iter().next().map(|body| body.translation);
        let class = request.and_then(|request| request.0.class);
        let tags = tags(class, hero, window, recording.settle);
        // The game's whole mix with them, from the chunk after this frame's (#1230).
        let sound = crate::sound_tap::Sound::arm(SOUND_ROOM).map(|sound| {
            // Where `Time<Real>`'s elapsed time counts from: its first update, not its start.
            SoundTrack::new(sound, real.first_update().unwrap_or_else(|| real.startup()))
        });
        if sound.is_none() {
            info!(
                "the recording into {} has no sound: no sound device plays the game's",
                recording.dir.display()
            );
        }
        recording.encoder = Some(Encoder::new(recording.dir.clone(), tags, sound));
    }
    let Some(frames) = recording.encoder.as_ref().and_then(Encoder::sender) else {
        return;
    };
    commands.spawn(Screenshot::primary_window()).observe(
        move |captured: On<ScreenshotCaptured>| {
            match bgra(&captured.image) {
                Some(pixels) => {
                    let frame = Frame {
                        times,
                        width: captured.image.width(),
                        height: captured.image.height(),
                        bgra: pixels,
                    };
                    frames.send(frame).ok();
                }
                None => error!(
                    "a frame of {:?} is not recorded",
                    captured.image.texture_descriptor.format
                ),
            }
            saved.fetch_add(1, std::sync::atomic::Ordering::Release);
        },
    );
}

/// The screenshot of `--screenshot` and the recordings, in a game, when asked for.
pub struct ScreenshotPlugin;

impl Plugin for ScreenshotPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Saving>().add_systems(
            Update,
            (
                take.run_if(resource_exists::<Request>).in_set(Playing),
                // In a menu too, a recording of the command's (#2491).
                record.run_if(resource_exists::<Recording>),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_numbered_in_order() {
        let dir = std::path::Path::new("frames");
        assert_eq!(frame_file(dir, 7), dir.join("frame-0007.png"));
        let mut names: Vec<_> = [10, 9, 100].map(|n| frame_file(dir, n)).to_vec();
        names.sort();
        assert_eq!(names, [9, 10, 100].map(|n| frame_file(dir, n)));
    }

    #[test]
    fn a_recording_starts_at_frame_0() {
        // With the 30 frames of settling the default, the 30th is frame 0; with none, the first
        // is, so that the frames asked for are all saved and the recording ends (#150).
        let numbers = |settle| [1, 29, 30, 31].map(|rendered| frame_number(rendered, settle));
        assert_eq!(numbers(30), [None, None, Some(0), Some(1)]);
        assert_eq!(numbers(0), [Some(0), Some(28), Some(29), Some(30)]);
        assert_eq!(numbers(1), numbers(0));
    }
}
