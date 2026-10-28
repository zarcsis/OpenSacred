//! The weather (docs/re/weather.md, #1072): rain, fog and snow, chosen by the weather entries of
//! the sector the camera is in on timers of real time that every sector shares (`0x625290`),
//! their levels moving toward what the choice wants at about three steps a second (`0x41A4F0`),
//! as the renderer's events set them (`0x419A70`). It is drawn and heard only: no rule of the
//! play reads it.
//!
//! Two slips of the original's choice are repaired (#1083, docs/decisions/weather.md, "Weather
//! times span their range"): its time is `shortest + (longest − shortest) / b` for the roll `b`,
//! nearly always the shortest and infinite for a roll of 0, where ours is
//! `shortest + (longest − shortest) · b / 255`; and it takes the snow for on by the rain's flag,
//! where ours takes the snow's own.

use opensacred_data::world::{SectorWeather, WeatherKind};

use crate::spawning::Dice;

/// The renderer's flags (`+0x10`): the rain, the fog, a storm (its lightning armed), the snow and
/// a quake of the screen.
pub const RAIN: u32 = 0x1;
pub const FOG: u32 = 0x2;
pub const STORM: u32 = 0x4;
pub const SNOW: u32 = 0x10;
pub const QUAKE: u32 = 0x80;

/// The fliers over the camera: the crow's and the bat's creature types (`CROW.GRN`, `BAT.GRN`).
pub const CROW: u32 = 513;
pub const BAT: u32 = 514;

/// The most of a value of the weather's events (`0x419A70`).
const MOST: u32 = 100;
/// The rain's drops and the snow's flakes of a value of 1.
const DROPS: u32 = 30;
/// The most snow flakes (`+0x9158`), and the most drops, past which the count is taken for 0
/// (`0x41A4F0`).
const FLAKES: u32 = 3_000;
const RAIN_MOST: u32 = 20_000;
/// The value above which a rain arms the storm.
const STORM_ABOVE: u32 = 75;
/// The steps of the levels a second (`0xAA4548`).
const STEPS: f32 = 3.0;
/// What a step moves the rain's drops, the fog's alpha and the snow's flakes by.
const RAIN_STEP: u32 = 10;
const FOG_STEP: u32 = 1;
const SNOW_STEP: u32 = 20;
/// How fast a quake's amplitude falls, in the screen's pixels a second.
const QUAKE_FALLS: f32 = 10.0;
/// How fast a jolt falls, in the screen's pixels a second.
const JOLT_FALLS: f32 = 40.0;

/// A level of the weather: what the choice wants and what it is now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Level {
    pub wanted: u32,
    pub now: u32,
}

impl Level {
    /// The level moved by `step` toward what is wanted, held there; whether it is at 0 and wanted
    /// at 0, its kind done.
    fn ramp(&mut self, step: u32) -> bool {
        if self.now < self.wanted {
            self.now = self.now.saturating_add(step).min(self.wanted);
        } else if self.wanted < self.now {
            self.now = self.now.saturating_sub(step).max(self.wanted);
        } else if self.wanted == 0 {
            return true;
        }
        false
    }
}

/// An event of the weather (`cEventWeather`, its kinds 0, 1, 2, 3, 5 and 7; docs/re/weather.md,
/// "`cEventWeather` and its kinds"), a value of 0 stopping its kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Every flag off at once (0).
    Clear,
    /// Every level wanted at 0, so that the weather fades (1).
    Fade,
    Rain(u32),
    Fog(u32),
    Snow(u32),
    /// A quake of the screen (7), which the arts' landings and the spells send.
    Quake(u32),
    /// A jolt of the screen (12), which the Big Demon's steps send.
    Jolt(u32),
}

/// The weather as the renderer keeps it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Weather {
    /// The flags ([`RAIN`], [`FOG`], [`STORM`], [`SNOW`]).
    pub flags: u32,
    /// The rain's drops (`+0x20`, `+0x1C`), the fog's alpha (`+0x178`, `+0x17C`) and the snow's
    /// flakes (`+0x9158`, `+0x9154`).
    pub rain: Level,
    pub fog: Level,
    pub snow: Level,
    /// The seconds to the storm's next lightning (`+0x428`).
    pub storm: f32,
    /// A quake's amplitude, in the screen's pixels (`+0x9DB8`).
    pub quake: f32,
    /// A jolt's offset down the screen, in its pixels (docs/re/weather.md, "The jolt").
    pub jolt: f32,
    /// The steps' accumulator (`0xAA4548`).
    steps: f32,
    /// When the rain's, the fog's and the snow's timers end, in milliseconds of real time
    /// (`0xAD77B4`, `0xAD77B0`, `0xAD77B8`), none until a sector of their kind is first seen
    /// (`0xAD77AC`).
    deadlines: [Option<f64>; 3],
    /// The rolls' index (`0xAD77C0`).
    index: u8,
    /// The view's sector last seen (`0x95F930`).
    sector: Option<u32>,
    /// When the next flier is due over the camera, in seconds of real time (`0xAD77BC`), none
    /// until a sector of fliers is first seen.
    fliers: Option<f64>,
}

/// A kind's place among the timed ones.
fn timed(kind: WeatherKind) -> Option<usize> {
    match kind {
        WeatherKind::Rain => Some(0),
        WeatherKind::Fog => Some(1),
        WeatherKind::Snow => Some(2),
        WeatherKind::Butterflies => None,
    }
}

/// A time of the range `range` in seconds for the roll `roll`, in milliseconds:
/// `shortest + (longest − shortest) · roll / 255`, from the shortest for a roll of 0 to the
/// longest for 255 (#1083; the original's `(longest − shortest) / roll` is nearly always the
/// shortest and infinite for 0).
fn period(range: [u32; 2], roll: u8) -> f64 {
    let [shortest, longest] = range.map(|seconds| f64::from(seconds) * 1_000.0);
    shortest + (longest - shortest) * f64::from(roll) / 255.0
}

impl Weather {
    /// The next of the rolls `rolls` (`0x95FA84`), the index first moved on.
    fn roll(&mut self, rolls: &[u8]) -> u8 {
        self.index = self.index.wrapping_add(1);
        rolls.get(usize::from(self.index)).copied().unwrap_or(0)
    }

    /// One frame's choice of the weather (`0x625290`) at the time `now` (milliseconds of real
    /// time), the camera in the sector `sector` of the entries `entries`, the rolls `rolls`: a
    /// change of sector fades it; each entry of rain, fog or snow arms its kind's timer the first
    /// time it is seen, and at the timer's end starts its kind, of an intensity rolled between its
    /// least and its most, for a time on, or stops it, for a time off.
    pub fn choose(
        &mut self,
        now: f64,
        sector: u32,
        entries: &[SectorWeather],
        rolls: &[u8],
        dice: &mut impl Dice,
    ) {
        if self.sector != Some(sector) {
            self.sector = Some(sector);
            self.event(Event::Fade, dice);
        }
        for entry in entries {
            let Some(k) = timed(entry.kind) else {
                continue;
            };
            let deadline = match self.deadlines[k] {
                Some(deadline) => deadline,
                None => {
                    let roll = self.roll(rolls);
                    let deadline = now + period(entry.off, roll);
                    self.deadlines[k] = Some(deadline);
                    deadline
                }
            };
            // Each kind on by its own flag (#1083; the original's `0x625459` takes the snow's
            // from the rain's).
            let on = match entry.kind {
                WeatherKind::Fog => self.flags & FOG != 0,
                WeatherKind::Snow => self.flags & SNOW != 0,
                _ => self.flags & RAIN != 0,
            };
            if deadline >= now {
                continue;
            }
            let (value, range) = if on {
                (0, entry.off)
            } else {
                let [least, most] = entry.intensity.map(u32::from);
                let roll = u32::from(self.roll(rolls));
                let span = (most + 1).saturating_sub(least).max(1);
                (least + roll % span, entry.on)
            };
            self.event(
                match entry.kind {
                    WeatherKind::Fog => Event::Fog(value),
                    WeatherKind::Snow => Event::Snow(value),
                    _ => Event::Rain(value),
                },
                dice,
            );
            let roll = self.roll(rolls);
            self.deadlines[k] = Some(now + period(range, roll));
        }
    }

    /// The fliers over the camera at the time `now` (seconds of real time) in a sector whose
    /// fliers' byte is `byte` (`0x625290`'s end): in a sector of any byte but 0 their timer runs,
    /// first armed `rand() % 161 + 20` seconds on by the rolls `rolls`, which the weather's
    /// timers share; once it is passed, the flier of the byte is due, a crow (513) for `0x10` or
    /// a bat (514) for `0x80`, none for the rest, and the timer is armed anew. The caller sends
    /// it but by night (`0x41BAB0`) and while a place of its pool of eight is free (`0x637060`).
    pub fn fliers(&mut self, now: f64, byte: u8, rolls: &[u8]) -> Option<u32> {
        if byte == 0 {
            return None;
        }
        let wait = |weather: &mut Self| f64::from(weather.roll(rolls) % 161 + 20);
        let due = match self.fliers {
            Some(due) => due,
            None => {
                let due = now + wait(self);
                self.fliers = Some(due);
                due
            }
        };
        if now <= due {
            return None;
        }
        self.fliers = Some(now + wait(self));
        match byte {
            0x10 => Some(CROW),
            0x80 => Some(BAT),
            _ => None,
        }
    }

    /// The renderer's handling of the event `event` (`0x419A70`): a value is held at 100; a rain
    /// of more than 75 arms the storm's lightning, 20 to 40 s on, by `dice`.
    pub fn event(&mut self, event: Event, dice: &mut impl Dice) {
        match event {
            Event::Clear => self.flags = 0,
            Event::Fade => {
                self.rain.wanted = 0;
                self.fog.wanted = 0;
                self.snow.wanted = 0;
                self.quake = 0.0;
            }
            Event::Rain(0) => {
                self.rain.wanted = 0;
                self.calm();
            }
            Event::Rain(value) => {
                let value = value.min(MOST);
                self.flags |= RAIN;
                self.rain.wanted = value * DROPS;
                if value > STORM_ABOVE {
                    if self.flags & STORM == 0 {
                        self.flags |= STORM;
                        self.storm = lightning_in(dice);
                    }
                } else {
                    self.calm();
                }
            }
            Event::Fog(0) => self.fog.wanted = 0,
            Event::Fog(value) => {
                self.flags |= FOG;
                self.fog.wanted = value.min(MOST) * 2;
            }
            Event::Snow(0) => self.snow.wanted = 0,
            Event::Snow(value) => {
                self.flags |= SNOW;
                self.snow.wanted = (value.min(MOST) * DROPS).min(FLAKES);
            }
            Event::Quake(0) => {
                self.quake = 0.0;
                self.flags &= !QUAKE;
            }
            Event::Quake(value) => {
                self.flags |= QUAKE;
                self.quake = small(value.min(MOST)) / 3.0;
            }
            Event::Jolt(value) => self.jolt = small(value.min(MOST)) / 3.0,
        }
    }

    /// The storm's lightning disarmed.
    fn calm(&mut self) {
        self.storm = 0.0;
        self.flags &= !STORM;
    }

    /// One frame's update of the levels (`0x41A4F0`), `dt` its seconds: the steps of the frame,
    /// about three a second, move each level that its flag holds toward what is wanted, and a
    /// level at 0 and wanted at 0 takes its flag off; the storm's lightning comes as its time
    /// ends, 20 to 40 s on again; a quake's amplitude falls by 10 a second, and ends at 0. How
    /// many lightnings came (the event of kind 9, which only the drawing takes, #1073).
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) -> u32 {
        while self.steps > 1.0 {
            self.steps -= 1.0;
        }
        self.steps += dt * STEPS;
        self.jolt = (self.jolt - dt * JOLT_FALLS).max(0.0);
        if self.flags == 0 {
            return 0;
        }
        let steps = whole(self.steps);
        let mut lightnings = 0;
        if self.flags & RAIN != 0 {
            if self.rain.ramp(steps * RAIN_STEP) {
                self.flags &= !RAIN;
            }
            if self.rain.now > RAIN_MOST {
                self.rain.now = 0;
            }
        }
        if self.flags & FOG != 0 && self.fog.ramp(steps * FOG_STEP) {
            self.flags &= !FOG;
        }
        if self.flags & STORM != 0 {
            self.storm -= dt;
            if self.storm <= 0.0 {
                self.storm = lightning_in(dice);
                lightnings += 1;
            }
        }
        if self.flags & SNOW != 0 {
            if self.snow.ramp(steps * SNOW_STEP) {
                self.flags &= !SNOW;
            }
            if self.snow.now > FLAKES {
                self.snow.now = 0;
            }
        }
        if self.flags & QUAKE != 0 {
            self.quake -= dt * QUAKE_FALLS;
            if self.quake <= 0.0 {
                self.event(Event::Quake(0), dice);
            }
        }
        lightnings
    }
}

/// The seconds to a storm's next lightning, 20 to 40 (`rand() / 32767 · 20 + 20`).
fn lightning_in(dice: &mut impl Dice) -> f32 {
    let roll = u16::try_from(dice.below(32_768)).unwrap_or(0);
    (f32::from(roll) * 3.051_851e-5 + 1.0) * 20.0
}

/// A value of an event, at most 100, as a float.
fn small(value: u32) -> f32 {
    f32::from(u16::try_from(value).unwrap_or(u16::MAX))
}

/// The whole steps of the accumulator `steps`, as the C library truncates a float.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the accumulator stays between 0 and 1 plus three times a frame's seconds"
)]
fn whole(steps: f32) -> u32 {
    steps.max(0.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flier_comes_on_its_timer_by_the_sectors_byte() {
        let rolls: Vec<u8> = (0..=255).collect();
        let mut weather = Weather::default();
        assert_eq!(weather.fliers(0.0, 0, &rolls), None);
        assert_eq!(weather.fliers(10.0, 0x10, &rolls), None);
        // Armed by the roll 1: 21 s on.
        assert_eq!(weather.fliers, Some(31.0));
        assert_eq!(weather.fliers(31.0, 0x10, &rolls), None);
        assert_eq!(weather.fliers(31.5, 0x10, &rolls), Some(CROW));
        // Armed anew by the roll 2.
        assert_eq!(weather.fliers, Some(53.5));
        assert_eq!(weather.fliers(60.0, 0x30, &rolls), None);
        assert_eq!(weather.fliers, Some(83.0));
        assert_eq!(weather.fliers(90.0, 0x80, &rolls), Some(BAT));
    }

    /// Dice that roll their numbers in turn.
    struct Rolls(Vec<u32>);

    impl Dice for Rolls {
        fn below(&mut self, n: u32) -> u32 {
            self.0.rotate_left(1);
            self.0.last().copied().unwrap_or(0) % n.max(1)
        }
    }

    fn rain() -> SectorWeather {
        SectorWeather {
            kind: WeatherKind::Rain,
            off: [480, 600],
            on: [480, 600],
            intensity: [20, 40],
        }
    }

    #[test]
    fn a_rain_starts_and_stops_on_its_timer_and_ramps_three_steps_a_second() {
        let mut dice = Rolls(vec![0]);
        // The rolls: the first arms the time off, the second the intensity, the third the time on.
        let mut rolls = vec![1_u8; 256];
        rolls[1] = 255;
        rolls[2] = 7;
        rolls[3] = 0;
        let mut weather = Weather::default();
        weather.choose(0.0, 2579, &[rain()], &rolls, &mut dice);
        // Armed: 480 s + 120 s · 255 / 255 off, the longest.
        assert_eq!(weather.deadlines[0], Some(600_000.0));
        assert_eq!(weather.flags, 0);
        weather.choose(600_000.0, 2579, &[rain()], &rolls, &mut dice);
        assert_eq!(weather.flags, 0);
        // Its end passed: the rain starts at 20 + 7 % 21, 27, 810 drops wanted, for the shortest
        // 480 s of a roll of 0.
        weather.choose(600_001.0, 2579, &[rain()], &rolls, &mut dice);
        assert_eq!(weather.flags, RAIN);
        assert_eq!(weather.rain.wanted, 810);
        assert_eq!(weather.deadlines[0], Some(600_001.0 + 480_000.0));
        // About three steps a second, of 10 drops each: none in the first quarter.
        for _ in 0..4 {
            weather.update(0.25, &mut dice);
        }
        assert_eq!(weather.rain.now, 30);
        // The time on passes: the rain stops, and fades to nothing, its flag off.
        weather.choose(1_080_002.0, 2579, &[rain()], &rolls, &mut dice);
        assert_eq!(weather.rain.wanted, 0);
        assert_eq!(weather.flags, RAIN);
        for _ in 0..5 {
            weather.update(0.25, &mut dice);
        }
        assert_eq!(weather.rain.now, 0);
        assert_eq!(weather.flags, 0);
    }

    #[test]
    fn a_change_of_sector_fades_the_weather_and_a_roll_of_0_takes_the_shortest_time() {
        let mut dice = Rolls(vec![0]);
        let mut weather = Weather::default();
        weather.event(Event::Fog(30), &mut dice);
        assert_eq!((weather.flags, weather.fog.wanted), (FOG, 60));
        weather.choose(0.0, 1, &[], &[], &mut dice);
        assert_eq!(weather.fog.wanted, 0);
        assert_eq!(weather.flags, FOG);
        // A roll of 0 takes the shortest time, where the original's is infinite (#1083).
        let fog = SectorWeather {
            kind: WeatherKind::Fog,
            ..rain()
        };
        let rolls = vec![0_u8; 256];
        weather.choose(10.0, 1, &[fog], &rolls, &mut dice);
        assert_eq!(weather.deadlines[1], Some(480_010.0));
    }

    #[test]
    fn the_snow_starts_and_stops_by_its_own_flag() {
        let mut dice = Rolls(vec![0]);
        let snow = SectorWeather {
            kind: WeatherKind::Snow,
            ..rain()
        };
        let rolls = vec![1_u8; 256];
        let mut weather = Weather::default();
        weather.choose(0.0, 7, &[snow], &rolls, &mut dice);
        weather.choose(700_000.0, 7, &[snow], &rolls, &mut dice);
        assert_eq!(weather.flags, SNOW);
        let wanted = weather.snow.wanted;
        assert!(wanted > 0);
        // Its time on passes: it stops, where the original's, taken for on by the rain's flag,
        // starts again (#1083).
        weather.choose(1_400_000.0, 7, &[snow], &rolls, &mut dice);
        assert_eq!(weather.flags, SNOW);
        assert_eq!(weather.snow.wanted, 0);
    }

    #[test]
    fn a_heavy_rain_arms_the_storm_and_its_lightning_comes_in_20_to_40_seconds() {
        let mut dice = Rolls(vec![16_383]);
        let mut weather = Weather::default();
        weather.event(Event::Rain(80), &mut dice);
        assert_eq!(weather.flags, RAIN | STORM);
        assert_eq!(weather.rain.wanted, 2_400);
        assert!((weather.storm - 30.0).abs() < 0.01);
        let mut lightnings = 0;
        for _ in 0..(31 * 10) {
            lightnings += weather.update(0.1, &mut dice);
        }
        assert_eq!(lightnings, 1);
        // A lighter rain disarms it.
        weather.event(Event::Rain(50), &mut dice);
        assert_eq!(weather.flags & STORM, 0);
        // The snow is held at 3,000 flakes.
        weather.event(Event::Snow(100), &mut dice);
        assert_eq!(weather.snow.wanted, 3_000);
    }

    #[test]
    fn a_jolt_moves_by_a_third_of_its_value_and_falls_by_forty_a_second() {
        let mut dice = Rolls(vec![0]);
        let mut weather = Weather::default();
        weather.event(Event::Jolt(18), &mut dice);
        assert!((weather.jolt - 6.0).abs() < 1e-4);
        weather.update(0.1, &mut dice);
        assert!((weather.jolt - 2.0).abs() < 1e-4);
        weather.update(0.1, &mut dice);
        assert_eq!(weather.jolt, 0.0);
    }

    #[test]
    fn a_quake_shakes_by_a_third_of_its_value_and_falls_by_ten_a_second() {
        let mut dice = Rolls(vec![0]);
        let mut weather = Weather::default();
        weather.event(Event::Quake(45), &mut dice);
        assert_eq!(weather.flags & QUAKE, QUAKE);
        assert!((weather.quake - 15.0).abs() < 1e-4);
        weather.update(1.0, &mut dice);
        assert!((weather.quake - 5.0).abs() < 1e-4);
        weather.update(0.6, &mut dice);
        assert_eq!((weather.flags & QUAKE, weather.quake), (0, 0.0));
        // At most 100.
        weather.event(Event::Quake(300), &mut dice);
        assert!((weather.quake - 100.0 / 3.0).abs() < 1e-4);
    }
}
