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
