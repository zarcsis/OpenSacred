//! The weather (docs/re/weather.md, #1072): rain, fog and snow, chosen by the weather entries of
//! the sector the camera is in on timers of real time that every sector shares (`0x625290`),
//! their levels moving toward what the choice wants at about three steps a second (`0x41A4F0`),
//! as the renderer's events set them (`0x419A70`). It is drawn and heard only: no rule of the
//! play reads it.
//!
//! The choice follows the original's code as it is, two points that look like slips among it
//! (docs/re/weather.md): a time is `shortest + (longest − shortest) / b` for the roll `b`, so
//! nearly always the shortest and infinite for a roll of 0, and the snow is taken for on by the
//! rain's flag.

use opensacred_data::world::{SectorWeather, WeatherKind};

use crate::spawning::Dice;

/// The renderer's flags (`+0x10`): the rain, the fog, a storm (its lightning armed) and the snow.
pub const RAIN: u32 = 0x1;
pub const FOG: u32 = 0x2;
pub const STORM: u32 = 0x4;
pub const SNOW: u32 = 0x10;

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

/// An event of the weather (`cEventWeather`, its kinds 0, 1, 2, 3 and 5; docs/re/weather.md,
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
/// `shortest + (longest − shortest) / roll`, infinite for a roll of 0.
fn period(range: [u32; 2], roll: u8) -> f64 {
    let [shortest, longest] = range.map(|seconds| f64::from(seconds) * 1_000.0);
    (longest - shortest) * (1.0 / f64::from(roll)) + shortest
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
            // The snow taken for on by the rain's flag (`0x625459`).
            let on = match entry.kind {
                WeatherKind::Fog => self.flags & FOG != 0,
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

    /// The renderer's handling of the event `event` (`0x419A70`): a value is held at 100; a rain
    /// of more than 75 arms the storm's lightning, 20 to 40 s on, by `dice`.
    pub fn event(&mut self, event: Event, dice: &mut impl Dice) {
        match event {
            Event::Clear => self.flags = 0,
            Event::Fade => {
                self.rain.wanted = 0;
                self.fog.wanted = 0;
                self.snow.wanted = 0;
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
    /// ends, 20 to 40 s on again. How many lightnings came (the event of kind 9, which only the
    /// drawing takes, #1073).
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) -> u32 {
        while self.steps > 1.0 {
            self.steps -= 1.0;
        }
        self.steps += dt * STEPS;
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
        lightnings
    }
}

/// The seconds to a storm's next lightning, 20 to 40 (`rand() / 32767 · 20 + 20`).
fn lightning_in(dice: &mut impl Dice) -> f32 {
    let roll = u16::try_from(dice.below(32_768)).unwrap_or(0);
    (f32::from(roll) * 3.051_851e-5 + 1.0) * 20.0
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
        rolls[1] = 120;
        rolls[2] = 7;
        rolls[3] = 120;
        let mut weather = Weather::default();
        weather.choose(0.0, 2579, &[rain()], &rolls, &mut dice);
        // Armed: 480 s + 120 s / 120 off.
        assert_eq!(weather.deadlines[0], Some(481_000.0));
        assert_eq!(weather.flags, 0);
        weather.choose(481_000.0, 2579, &[rain()], &rolls, &mut dice);
        assert_eq!(weather.flags, 0);
        // Its end passed: the rain starts at 20 + 7 % 21, 27, 810 drops wanted, for 481 s.
        weather.choose(481_001.0, 2579, &[rain()], &rolls, &mut dice);
        assert_eq!(weather.flags, RAIN);
        assert_eq!(weather.rain.wanted, 810);
        assert_eq!(weather.deadlines[0], Some(481_001.0 + 481_000.0));
        // About three steps a second, of 10 drops each: none in the first quarter.
        for _ in 0..4 {
            weather.update(0.25, &mut dice);
        }
        assert_eq!(weather.rain.now, 30);
        // The time on passes: the rain stops, and fades to nothing, its flag off.
        weather.choose(962_002.0, 2579, &[rain()], &rolls, &mut dice);
        assert_eq!(weather.rain.wanted, 0);
        assert_eq!(weather.flags, RAIN);
        for _ in 0..5 {
            weather.update(0.25, &mut dice);
        }
        assert_eq!(weather.rain.now, 0);
        assert_eq!(weather.flags, 0);
    }

    #[test]
    fn a_change_of_sector_fades_the_weather_and_a_roll_of_0_freezes_its_kind() {
        let mut dice = Rolls(vec![0]);
        let mut weather = Weather::default();
        weather.event(Event::Fog(30), &mut dice);
        assert_eq!((weather.flags, weather.fog.wanted), (FOG, 60));
        weather.choose(0.0, 1, &[], &[], &mut dice);
        assert_eq!(weather.fog.wanted, 0);
        assert_eq!(weather.flags, FOG);
        // A roll of 0 makes a time infinite.
        let fog = SectorWeather {
            kind: WeatherKind::Fog,
            ..rain()
        };
        let mut rolls = vec![0_u8; 256];
        rolls[1] = 0;
        weather.choose(10.0, 1, &[fog], &rolls, &mut dice);
        assert_eq!(weather.deadlines[1], Some(f64::INFINITY));
    }

    #[test]
    fn the_snow_is_taken_for_on_by_the_rains_flag() {
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
        // Its time on passes without the rain: it starts again rather than stopping.
        weather.choose(1_400_000.0, 7, &[snow], &rolls, &mut dice);
        assert_eq!(weather.flags, SNOW);
        assert!(weather.snow.wanted > 0);
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
}
