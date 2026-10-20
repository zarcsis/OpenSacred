//! `--benchmark SECONDS`: renders without vsync for that long once the hero is in place, prints
//! the frame rate and exits. For the M2 criterion of 60 frames a second at 3840 × 2160. The
//! command `benchmark` measures the same way and goes on after, at the window's present mode.
//!
//! A thread of its own sleeps a millisecond at a time and counts the times it woke more than
//! 20 ms late: stalls of the whole process, not of a frame's work, which came with the slow
//! frames of #2.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use bevy::prelude::*;

use crate::frame::Playing;
use crate::hero::Ready;

#[derive(Resource)]
pub struct Benchmark {
    length: Duration,
    elapsed: Duration,
    frames: u32,
    slowest: Duration,
    stalls: Arc<Stalls>,
    /// Whether the process ends with the benchmark.
    exit: bool,
}

/// The process's stalls that the watching thread saw.
#[derive(Default)]
struct Stalls {
    count: AtomicU32,
    /// The longest, in microseconds.
    longest: AtomicU32,
    /// Set when the benchmark is over, for the thread to end.
    over: AtomicBool,
}

/// How late the watching thread may wake before it counts a stall.
const STALL: Duration = Duration::from_millis(20);

impl Benchmark {
    pub fn new(seconds: f32, exit: bool) -> Self {
        let stalls = Arc::new(Stalls::default());
        let seen = Arc::clone(&stalls);
        std::thread::spawn(move || {
            let mut last = Instant::now();
            while !seen.over.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(1));
                let now = Instant::now();
                let gap = now - last;
                if gap > STALL {
                    seen.count.fetch_add(1, Ordering::Relaxed);
                    let micros = u32::try_from(gap.as_micros()).unwrap_or(u32::MAX);
                    seen.longest.fetch_max(micros, Ordering::Relaxed);
                }
                last = now;
            }
        });
        Self {
            length: Duration::from_secs_f32(seconds),
            elapsed: Duration::ZERO,
            frames: 0,
            slowest: Duration::ZERO,
            stalls,
            exit,
        }
    }
}

impl Drop for Benchmark {
    fn drop(&mut self) {
        self.stalls.over.store(true, Ordering::Relaxed);
    }
}

pub fn measure(
    mut commands: Commands,
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
        let result = format!(
            "benchmark: {} frames in {seconds:.1} s, {:.1} frames a second, the slowest {:.1} ms; \
             {} stalls of the process, the longest {:.1} ms",
            benchmark.frames,
            f64::from(benchmark.frames) / seconds,
            benchmark.slowest.as_secs_f64() * 1000.0,
            benchmark.stalls.count.load(Ordering::Relaxed),
            f64::from(benchmark.stalls.longest.load(Ordering::Relaxed)) / 1000.0
        );
        // On the standard output for the scripts that read it, and in the log.
        println!("{result}");
        info!("{result}");
        if benchmark.exit {
            exit.write(AppExit::Success);
        } else {
            commands.remove_resource::<Benchmark>();
        }
    }
}

/// The benchmark, when asked for.
pub struct BenchmarkPlugin;

impl Plugin for BenchmarkPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            measure.run_if(resource_exists::<Benchmark>).in_set(Playing),
        );
    }
}
