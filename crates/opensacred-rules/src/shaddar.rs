//! The last fight's slaves and Shaddar's ward (#2919; docs/re/effects.md, "Shaddar's slaves"):
//! the thirteen FX objects 921 that the last priestess's death makes about the hall's pentagram.
//! A slave wanders as the butterflies do, a star of orange rays; it marks a player's hero it
//! touches for 5 s (the effect 32), and only a hero so marked harms Shaddar ([`wards`]); one that
//! touches Shaddar turns red and hunts the local hero at three times its speed, strikes the first
//! creature but Shaddar it touches, and wanders again ([`Slave`]).
//!
//! A slave's heading, turn and speed are the effects' frame's, as their flight's (`x` along the
//! screen's horizontal on the ground, `y` toward the viewer; docs/re/effects.md, "Their
//! flight"), its rays in that frame about its place with `z` up; its reaches are the world's on
//! the ground, in its units.

use crate::particles::{Block, Layer, Ramp, Spread, System};
use crate::spawning::Dice;

/// Shaddar's type (`TYPE_NPC_SHARIFO`), the slaves' and the hall's pentagram's FX types.
pub const SHADDAR: u32 = 700;
pub const SLAVE: u32 = 921;
pub const PENTAGRAM: u32 = 920;
/// The effect that a slave's touch gives a player's hero (`0x582D50(0x20, …, 5.0)`), one alone,
/// a touch renewing its seconds; it changes no stat.
pub const MARK: u32 = 32;
pub const MARK_SECONDS: f32 = 5.0;
/// A slave's size, by which it touches a creature within its size and the creature's
/// (`0x5AB490`).
pub const SLAVE_SIZE: f32 = 50.0;
/// How far a hunter seeks a creature once its target is gone, squared (`0x600570`): about 316.
pub const HUNT_REACH_SQUARED: f32 = 100_000.0;
/// A hunter's most turn toward its target a second (`+0xE0`).
const HUNT_TURN: f32 = 3.0;
/// A hunter's speed's factor, and the cooldown of a slave's touch after its strike.
const HUNT_FACTOR: f32 = 3.0;
const STRIKE_COOLDOWN: f32 = 1.0;

/// Whether Shaddar's ward takes a strike's whole damage away (`0x56B130` at `0x56DAD2`): a
/// strike at Shaddar from nobody, or from an attacker that does not hold the [`MARK`].
pub fn wards(defender: u32, attacker_marked: bool) -> bool {
    defender == SHADDAR && !attacker_marked
}

/// The turn of a marked hero's emblem over its head at the real clock's `millis` (`0x40A690`,
/// `0x6413E0`): its milliseconds' low 16 bits over 512, a turn in some 3.2 s.
pub fn mark_turn(millis: u64) -> f32 {
    f32::from(u16::try_from(millis % 0x1_0000).unwrap_or(0)) / 512.0
}

/// The marked hero's emblem (`FX_SACREDLOGO`): an upright square from 15 to each side, 12 to 42
/// over its head's bone, white, added whole.
pub const MARK_TEXTURE: &str = "FX_SACREDLOGO";
pub const MARK_HALF: f32 = 15.0;
pub const MARK_FROM: f32 = 12.0;
pub const MARK_TO: f32 = 42.0;

/// The slave's rays' texture (`0x7AA250`), its Direct3D light's colour (of the priority 1), its
/// particles, its flash's seconds, and its rays' half width and length for a ray's size (the
/// billboards' flags `0x8D` of the factor 0.125: each a streak standing on its foot).
pub const SLAVE_TEXTURE: &str = "PARTICLE_LINE01";
pub const SLAVE_LIGHT: [f32; 3] = [1.0, 0.8, 0.7];
const SLAVE_COUNT: usize = 400;
const FLASH: f32 = 0.5;
pub const RAY_HALF_WIDTH: f32 = 0.125;
pub const RAY_LENGTH: f32 = 2.0;

/// Whom a slave touches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Touched {
    /// A player's hero (an id up to 16).
    Hero,
    Shaddar,
    /// Any other creature.
    Other,
}

/// What a slave does at its touches of a frame ([`Slave::touch`]): the indices of the touched
/// that it marks and of those that it strikes, and whether it starts hunting the local hero.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Deeds {
    pub marks: Vec<usize>,
    pub strikes: Vec<usize>,
    pub hunts_hero: bool,
}

/// A signal to a slave's system (`0x7CBA10` writes its word whole, so that a frame's last
/// stands).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Signal {
    /// `8`: its new rays twice as long for half a second.
    Flash,
    /// `0x10`: from orange and wandering to red and hunting, or back.
    Toggle,
}

/// A slave (`cParticleSystem_shaddar_slave` and its object): its rays, its flash, whether its
/// rays are red (its ramp's index, `+0x28B0`), whether it hunts (its flags `0x10002`, else
/// `0x8002`), whether it fades out (its signal 2), its heading and turn, and its touch's
/// cooldown (`+0x11C`).
#[derive(Clone, Debug, PartialEq)]
pub struct Slave {
    pub system: System,
    pub flash: f32,
    pub red: bool,
    pub hunting: bool,
    pub fading: bool,
    pub heading: [f32; 2],
    pub turn: f32,
    pub cooldown: f32,
}

/// A uniform number of `[0, 1]` by `dice`, as the client's `rand()` / 32767.
fn unit(dice: &mut impl Dice) -> f32 {
    f32::from(u16::try_from(dice.below(0x8000)).unwrap_or(0)) / 32_767.0
}

impl Slave {
    /// A slave as its event makes it (`0x5A4E53`, `0x7AA6F0`): its heading `(2r₂ − 1, 2r₁ − 1)`
    /// normalised (never of no length: a roll over 32,767 is never a half) and its turn
    /// `0.1 + 0.4 r₃` radians a second, its rays orange.
    pub fn new(dice: &mut impl Dice) -> Self {
        let [r1, r2, r3] = [unit(dice), unit(dice), unit(dice)];
        let [x, y] = [2.0 * r2 - 1.0, 2.0 * r1 - 1.0];
        let length = (x * x + y * y).sqrt().max(f32::EPSILON);
        let heading = [x / length, y / length];
        Self {
            system: System::new(SLAVE_COUNT),
            flash: 0.0,
            red: false,
            hunting: false,
            fading: false,
            heading,
            turn: 0.1 + 0.4 * r3,
            cooldown: 0.0,
        }
    }

    /// Its rays' layer (`+0x28B4`): the life −200 and the size −25 a second.
    pub fn layers() -> [Layer; 1] {
        [Layer {
            life: -200.0,
            size: -25.0,
            ..Layer::default()
        }]
    }

    /// Its rays' block by its flash `flash` (`+0x28D4`, `0x7AA330`): the size `30·(2f + 1)` ± 7,
    /// any angle, 40 over its place ± 1, one every 0.0066 s at the detail 2.
    pub fn block(flash: f32) -> Block {
        Block {
            size: Spread {
                value: 30.0 * (2.0 * flash + 1.0),
                spread: 7.0,
            },
            angle: Spread {
                value: std::f32::consts::PI,
                spread: std::f32::consts::PI,
            },
            offset: Spread {
                value: [0.0, 0.0, 40.0],
                spread: [1.0; 3],
            },
            interval: 0.0066,
            ..Block::default()
        }
    }

    /// Its ramps (`+0x20B0`, `+0x24B0`): orange, or red, each fading in over 76 entries, dimming
    /// over 66 and fading to a grey nothing over 114.
    pub fn ramp(red: bool) -> Ramp {
        let c = |r: f32, g: f32, b: f32, a: f32| [r / 255.0, g / 255.0, b / 255.0, a / 255.0];
        if red {
            Ramp::new(
                c(254.0, 1.0, 1.0, 0.0),
                &[
                    (c(253.0, 76.0, 76.0, 254.0), 76),
                    (c(167.0, 67.0, 67.0, 254.0), 66),
                    (c(55.0, 55.0, 54.0, 0.0), 114),
                ],
            )
        } else {
            Ramp::new(
                c(254.0, 96.0, 1.0, 0.0),
                &[
                    (c(253.0, 131.0, 76.0, 254.0), 76),
                    (c(167.0, 98.0, 67.0, 254.0), 66),
                    (c(55.0, 54.0, 55.0, 0.0), 114),
                ],
            )
        }
    }

    /// Its speed a second in the flight's units of the wandering speed `speed` (85): three times
    /// it, rounded down, while it hunts.
    pub fn speed(&self, speed: f32) -> f32 {
        if self.hunting {
            (HUNT_FACTOR * speed).floor()
        } else {
            speed
        }
    }

    /// Its rays `dt` seconds on (`0x7AA330`): the flash falling and, at its end, the rays orange
    /// again whatever it does; its rays aged and, unless it fades, emitted. Whether it lives: a
    /// fading slave while a ray lives.
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) -> bool {
        self.cooldown = (self.cooldown - dt).max(0.0);
        if self.flash > 0.0 {
            self.flash -= dt;
            if self.flash <= 0.0 {
                self.flash = 0.0;
                self.red = false;
            }
        }
        let layers = Self::layers();
        self.system.update(dt, &layers);
        if self.fading {
            return !self.system.particles.is_empty();
        }
        self.system
            .emit(&[Self::block(self.flash)], &layers, SLAVE_COUNT, dice);
        true
    }

    /// Turns its wandering heading by its turn over `dt` seconds (`0x5A8885`).
    pub fn wander(&mut self, dt: f32) {
        let (sin, cos) = (self.turn * dt).sin_cos();
        let [x, y] = self.heading;
        self.heading = [x * cos - y * sin, y * cos + x * sin];
    }

    /// Turns back from a cell that it may not cross while it wanders: its heading reversed, its
    /// turn `0.2 r`.
    pub fn bounce(&mut self, dice: &mut impl Dice) {
        self.heading = self.heading.map(|v| -v);
        self.turn = 0.2 * unit(dice);
    }

    /// Turns its hunting heading toward `toward` (any length) by at most 3 radians a second over
    /// `dt` seconds (`0x5A7DD0`).
    pub fn steer(&mut self, toward: [f32; 2], dt: f32) {
        let want = toward[1].atan2(toward[0]);
        let have = self.heading[1].atan2(self.heading[0]);
        let mut off = (want - have).rem_euclid(std::f32::consts::TAU);
        if off > std::f32::consts::PI {
            off -= std::f32::consts::TAU;
        }
        let most = HUNT_TURN * dt;
        let angle = have + off.clamp(-most, most);
        self.heading = [angle.cos(), angle.sin()];
    }

    /// What it does at its touches of a frame, `touched` in the order gathered and `homing` whether
    /// it has a target (`0x5ABD80`, the case `0x399`): with none, while its cooldown is out, it
    /// marks each player's hero and flashes, and at Shaddar turns to hunt the local hero; with
    /// one, it strikes each creature but Shaddar, turns back, drops its target and waits a second.
    /// A frame's last signal stands.
    pub fn touch(&mut self, touched: &[Touched], homing: bool) -> Deeds {
        let mut deeds = Deeds::default();
        let mut signal = None;
        if homing {
            for (index, &who) in touched.iter().enumerate() {
                if who != Touched::Shaddar {
                    deeds.strikes.push(index);
                    signal = Some(Signal::Toggle);
                    self.cooldown = STRIKE_COOLDOWN;
                }
            }
        } else if self.cooldown <= 0.0 {
            for (index, &who) in touched.iter().enumerate() {
                match who {
                    Touched::Hero => {
                        deeds.marks.push(index);
                        signal = Some(Signal::Flash);
                    }
                    Touched::Shaddar => signal = Some(Signal::Toggle),
                    Touched::Other => {}
                }
            }
        }
        match signal {
            Some(Signal::Flash) => self.flash = FLASH,
            Some(Signal::Toggle) => {
                self.red = !self.red;
                self.hunting = self.red;
                deeds.hunts_hero = self.hunting && !homing;
            }
            None => {}
        }
        deeds
    }

    /// Fades out (its signal 2): no more rays, it stands, and it ends as its last ray dies.
    pub fn fade(&mut self) {
        self.fading = true;
        self.hunting = false;
    }
}

#[cfg(test)]
mod tests;
