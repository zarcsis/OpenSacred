//! Day and night (docs/re/daynight.md): the game's clock, its phases and the night's darkness.
//!
//! The clock starts at 08:00 and goes 24 seconds of the day a second, as the original's does
//! (**hypothesis** for the unit of the frame's time: a day lasts an hour). By the minutes of the
//! day, the dawn comes at 03:20, the day at 05:20, the dusk at 20:20 and the night at 22:20; the
//! night's level is 0 by day, 255 by night, and goes between them over the two hours of the dusk
//! and of the dawn.
//!
//! The original colours every point of the world by the light around it and the night's level:
//! by night a ramp of the light's level, built from the setting `NIGHT_DARKNESS` (50), and
//! `255 − level` added to each channel, so that the day is white. The game draws the frame first
//! and then multiplies it by that colour (`night.wesl`, a fullscreen pass): the same for the art,
//! which the original multiplies by its vertices' colour, if not for its blending. **Not yet**: the
//! light sources (#59), so that the light's level is the dark's everywhere (the original's sources and
//! their radii are not read), and the dungeons' grey ramp.

use bevy::asset::embedded_asset;
use bevy::core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin};
use bevy::math::Vec4;
use bevy::prelude::{App, Component, Plugin, Query, Res, ResMut, Resource, Time};
use bevy::render::RenderApp;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_resource::ShaderType;
use bevy::shader::ShaderRef;

/// The shader of [`Night`], embedded in the binary.
pub const SHADER: &str = "embedded://opensacred_game/night.wesl";

/// Seconds of the day that a second of the frames' time adds (the clock's `+0x0C`).
const SPEED: f64 = 24.0;
const DAY_SECONDS: f64 = 86_400.0;
/// The renderer's times, in minutes of the day: the dawn's start, the day's, the dusk's, the
/// night's.
const DAWN: f64 = 200.0;
const DAY: f64 = 320.0;
const DUSK: f64 = 1220.0;
const NIGHT: f64 = 1340.0;
/// The night's level per minute of the dusk and of the dawn (255 over 120 minutes).
const PER_MINUTE: f64 = 2.125;
/// `NIGHT_DARKNESS` of the original's settings (`Settings.cfg`, 50), as a fraction.
const DARKNESS: f32 = 0.5;
/// The light's level at a point that no light source reaches.
const DARK: f32 = 32.0;

/// The phase of the day, as the renderer tells the sound manager (`+0xA34`: 0 to 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Day,
    Dusk,
    Night,
    Dawn,
}

/// The game's clock: the seconds of the day.
#[derive(Resource, Clone)]
pub struct Clock {
    seconds: f64,
}

impl Clock {
    /// A clock at `hours:minutes`.
    pub fn at(hours: u32, minutes: u32) -> Self {
        Self {
            seconds: f64::from(hours * 3600 + minutes * 60) % DAY_SECONDS,
        }
    }

    /// The minutes of the day, whole, as the original counts them (hours · 60 + minutes).
    pub fn minutes(&self) -> f64 {
        (self.seconds / 60.0).floor()
    }

    pub fn phase(&self) -> Phase {
        phase(self.minutes())
    }

    /// The clock after a frame's time of `seconds`.
    fn advance(&mut self, seconds: f64) {
        self.seconds = (self.seconds + seconds * SPEED).rem_euclid(DAY_SECONDS);
    }
}

impl Default for Clock {
    /// 08:00, the original's start.
    fn default() -> Self {
        Self::at(8, 0)
    }
}

/// The phase at a minute of the day (`0x41A4F0`).
pub fn phase(minutes: f64) -> Phase {
    if DAY < minutes && minutes < DUSK {
        Phase::Day
    } else if (DUSK..=NIGHT).contains(&minutes) {
        Phase::Dusk
    } else if !(DAWN..=NIGHT).contains(&minutes) {
        Phase::Night
    } else {
        Phase::Dawn
    }
}

/// The night's level at a minute of the day, 0 to 255 (`0x41C2D0`).
pub fn level(minutes: f64) -> f64 {
    match phase(minutes) {
        Phase::Day => 0.0,
        Phase::Dusk => (minutes - DUSK) * PER_MINUTE,
        Phase::Night => 255.0,
        Phase::Dawn => (DAY - minutes) * PER_MINUTE,
    }
}

/// The night ramp's red and green, and its blue, at a light's level, out of 255 (the renderer's
/// tables, `0x417F00`).
fn ramp(light: f32) -> (f32, f32) {
    let base = 64.0 + 64.0 * DARKNESS;
    (base + light / 4.0, base + light / 3.0)
}

/// The night's pass and its shader.
pub struct DayNightPlugin;

impl Plugin for DayNightPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "night.wesl");
        app.add_plugins(FullscreenMaterialPlugin::<Night>::default());
    }
}

/// `HH:MM` of a time of day.
pub fn parse_time(text: &str) -> Result<Clock, String> {
    let (h, m) = text.split_once(':').ok_or("expected HH:MM")?;
    let number = |s: &str| s.trim().parse::<u32>().map_err(|e| format!("{s}: {e}"));
    let (hours, minutes) = (number(h)?, number(m)?);
    if hours > 23 || minutes > 59 {
        return Err(format!("{text}: not a time of day"));
    }
    Ok(Clock::at(hours, minutes))
}

/// Advances the clock by the frame's time.
pub fn tick(time: Res<Time>, mut clock: ResMut<Clock>) {
    clock.advance(f64::from(time.delta_secs()));
}

/// The frame's colour by the night: the ramp at the dark's level, and the part of the day's
/// white that the night's level leaves, added to it.
#[derive(Component, ExtractComponent, Clone, Copy, ShaderType, Default)]
#[extract_app(RenderApp)]
pub struct Night {
    /// Red, green and blue of the ramp, and the day's part, each out of 1.
    colour: Vec4,
}

impl FullscreenMaterial for Night {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
}

/// Keeps the camera's night by the clock.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a level from 0 to 255 in a float of the shader"
)]
pub fn light(clock: Res<Clock>, mut nights: Query<&mut Night>) {
    let level = level(clock.minutes()) as f32;
    let (red_green, blue) = ramp(DARK);
    let colour = Vec4::new(
        red_green / 255.0,
        red_green / 255.0,
        blue / 255.0,
        (255.0 - level) / 255.0,
    );
    for mut night in &mut nights {
        if night.colour != colour {
            night.colour = colour;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_phases_and_the_nights_level_follow_the_renderers_times() {
        let at = |h: f64, m: f64| h * 60.0 + m;
        assert_eq!(phase(at(8.0, 0.0)), Phase::Day);
        assert_eq!(phase(at(20.0, 20.0)), Phase::Dusk);
        assert_eq!(phase(at(23.0, 0.0)), Phase::Night);
        assert_eq!(phase(at(1.0, 0.0)), Phase::Night);
        assert_eq!(phase(at(4.0, 0.0)), Phase::Dawn);
        assert_eq!(level(at(12.0, 0.0)), 0.0);
        assert_eq!(level(at(21.0, 20.0)), 127.5);
        assert_eq!(level(at(22.0, 20.0)), 255.0);
        assert_eq!(level(at(0.0, 0.0)), 255.0);
        assert_eq!(level(at(4.0, 20.0)), 127.5);
        assert_eq!(level(at(5.0, 20.0)), 0.0);
    }

    #[test]
    fn the_clock_starts_at_eight_and_a_day_lasts_an_hour() {
        let mut clock = Clock::default();
        assert_eq!(clock.minutes(), 480.0);
        clock.advance(1800.0);
        assert_eq!(clock.minutes(), 480.0 + 720.0);
        clock.advance(1800.0);
        assert_eq!(clock.minutes(), 480.0);
        assert_eq!(Clock::at(22, 30).phase(), Phase::Night);
    }

    #[test]
    fn the_night_ramp_is_a_bluish_grey() {
        let (rg, b) = ramp(DARK);
        assert_eq!((rg, b.round()), (104.0, 107.0));
        let (rg, b) = ramp(255.0);
        assert_eq!((rg.round(), b), (160.0, 181.0));
    }
}
