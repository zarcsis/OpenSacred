//! `--screenshot`: once the hero animates and every sector near it is in place, render a few
//! more frames, save one to a PNG and exit. The first step toward the screenshot tests of M2.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};

use crate::hero::Ready;
use crate::sectors::Streamer;

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
) {
    if request.taken || !ready.0 || !streamer.idle() {
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

/// `--record`: from the same moment as a screenshot, save every frame to a numbered PNG for a
/// number of frames, then exit, to look at flicker or a stutter frame by frame.
#[derive(Resource)]
pub struct Recording {
    dir: PathBuf,
    settle: u32,
    frames: u32,
    /// Frames rendered since the scene was ready.
    rendered: u32,
    /// Frames saved, which the last one's capture counts down.
    saved: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

impl Recording {
    pub fn new(dir: PathBuf, settle: u32, frames: u32) -> Self {
        Self {
            dir,
            settle,
            frames,
            rendered: 0,
            saved: std::sync::Arc::default(),
        }
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

pub fn record(
    mut commands: Commands,
    mut recording: ResMut<Recording>,
    ready: Res<Ready>,
    streamer: Res<Streamer>,
    mut exit: MessageWriter<AppExit>,
) {
    if recording.saved.load(std::sync::atomic::Ordering::Acquire) >= recording.frames {
        exit.write(AppExit::Success);
        return;
    }
    if !ready.0 || (recording.rendered == 0 && !streamer.idle()) {
        return;
    }
    recording.rendered += 1;
    let Some(n) = frame_number(recording.rendered, recording.settle) else {
        return;
    };
    if n >= recording.frames {
        return;
    }
    let saved = recording.saved.clone();
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(frame_file(&recording.dir, n)))
        .observe(move |_: On<ScreenshotCaptured>| {
            saved.fetch_add(1, std::sync::atomic::Ordering::Release);
        });
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
