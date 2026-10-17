//! `--benchmark SECONDS`: renders without vsync for that long once the hero is in place, prints
//! the frame rate and exits. For the M2 criterion of 60 frames a second at 3840 × 2160.

use std::time::Duration;

use bevy::prelude::*;

use crate::hero::Ready;

#[derive(Resource)]
pub struct Benchmark {
    length: Duration,
    elapsed: Duration,
    frames: u32,
    slowest: Duration,
}

impl Benchmark {
    pub fn new(seconds: f32) -> Self {
        Self {
            length: Duration::from_secs_f32(seconds),
            elapsed: Duration::ZERO,
            frames: 0,
            slowest: Duration::ZERO,
        }
    }
}

pub fn measure(
    time: Res<Time<Real>>,
    ready: Res<Ready>,
    mut benchmark: ResMut<Benchmark>,
    mut exit: MessageWriter<AppExit>,
) {
    if !ready.0 {
        return;
    }
    let delta = time.delta();
    benchmark.elapsed += delta;
    benchmark.frames += 1;
    benchmark.slowest = benchmark.slowest.max(delta);
    if delta > Duration::from_millis(30) {
        info!(
            "slow frame at {:.2} s: {:.1} ms",
            benchmark.elapsed.as_secs_f64(),
            delta.as_secs_f64() * 1000.0
        );
    }
    if benchmark.elapsed >= benchmark.length {
        let seconds = benchmark.elapsed.as_secs_f64();
        println!(
            "benchmark: {} frames in {seconds:.1} s, {:.1} frames a second, the slowest {:.1} ms",
            benchmark.frames,
            f64::from(benchmark.frames) / seconds,
            benchmark.slowest.as_secs_f64() * 1000.0
        );
        exit.write(AppExit::Success);
    }
}
