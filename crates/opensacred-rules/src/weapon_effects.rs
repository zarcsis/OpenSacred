//! The effects of a weapon in hand (#2922; docs/re/effects.md, "776, a weapon's glow" and "The
//! unique weapons' effects"): what `cWeapon3D`'s switch (`0x5CDB60`) makes as a model is put on.
//!
//! - A weapon whose model has the bone `stdfx_bone01` glows at its bone `weapon_gl01` in the
//!   colour of its bonuses' style ([`bonus_style`], [`WeaponGlow`], 776).
//! - The Seraphim's whip 3073 trails a whip of blue glows ([`MagicWhip`], 824).
//! - Her wings 4010 and 4092 trail a ribbon from each streak bone ([`MagicStreak`], 837).
//! - The Vampiress's sword 1771 sheds red sparks ([`MagicWorms`], 858).
//!
//! The places are the models' frame's (`x` along the screen's horizontal on the ground, `y`
//! toward the viewer, `z` up), the systems' own.

use crate::particles::{Block, Layer, Ramp, Spread, System};
use crate::spawning::Dice;

/// The types of the unique weapons that have an effect of their own.
pub const WHIP: u32 = 3073;
pub const WINGS: [u32; 2] = [4010, 4092];
pub const WORMS_SWORD: u32 = 1771;

/// The bone that a weapon's model must have for its bonus's glow, and the bone the glow sits at
/// (the 18th).
pub const GLOW_BONE: &str = "stdfx_bone01";
pub const GLOW_AT: &str = "weapon_gl01";

/// The style of the glow of a weapon whose bonus slots are `slots`, each its condition and its
/// effect, in their order (`0x5CEA50`): the first slot of a condition or an effect whose effect's
/// low 16 bits give a style; none where none does.
pub fn bonus_style(slots: impl IntoIterator<Item = (u16, u32)>) -> Option<u8> {
    slots
        .into_iter()
        .filter(|&(condition, effect)| condition != 0 || effect != 0)
        .find_map(|(_, effect)| style_of(effect & 0xFFFF))
}

/// The style of a bonus's effect (the byte table `0x5CEBA8`): 1 red, 2 green, 3 grey, 4 blue,
/// 5 violet.
fn style_of(effect: u32) -> Option<u8> {
    match effect {
        5 | 11 | 24 | 25 => Some(1),
        2 | 16 | 26 | 27 => Some(2),
        1 | 29 | 30 | 34 => Some(3),
        4 | 28 | 35 | 36 => Some(4),
        3 | 7..=10 | 13 | 15 | 17..=23 | 31..=33 | 37..=41 | 43 | 44 => Some(5),
        _ => None,
    }
}

/// The glow's texture, its flare's, its particles, its world light and its flare's half side.
pub const GLOW_TEXTURE: &str = "PARTICLE_GLOW01";
pub const GLOW_FLARE: &str = "PARTICLE_FLARE02";
const GLOW_COUNT: usize = 300;
pub const GLOW_LIGHT: f32 = 250.0;

/// A weapon's glow (776, `cParticleSystem_fireball`, its start `0x76B5F0`, run `0x76B4C0` and
/// draw `0x76B540`): sparks of `PARTICLE_GLOW01` born at its point, white, turning to its style's
/// colour and fading out in a quarter second, added, and a flare of `PARTICLE_FLARE02` there,
/// pulsing.
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponGlow {
    pub system: System,
    pub style: u8,
}

impl WeaponGlow {
    /// A glow of the style `style`.
    pub fn new(style: u8) -> Self {
        Self {
            system: System::new(GLOW_COUNT),
            style,
        }
    }

    /// Its layer: the life −1000 and the size −2.7 a second.
    pub fn layers() -> [Layer; 1] {
        [Layer {
            life: -1000.0,
            size: -2.7,
            ..Layer::default()
        }]
    }

    /// Its block: the size 3 at its point, one every 0.005 s at the detail 2.
    pub fn block() -> Block {
        Block {
            size: Spread {
                value: 3.0,
                spread: 0.0,
            },
            interval: 0.005,
            ..Block::default()
        }
    }

    /// Its ramp by its style: white, its colour over 8 entries, a darker one at the alpha 0.8
    /// over 192 and to nothing over 56.
    pub fn ramp(style: u8) -> Ramp {
        let (bright, dark) = match style {
            1 => ([0.5, 0.2, 0.2], [0.3, 0.1, 0.1]),
            2 => ([0.2, 0.5, 0.2], [0.1, 0.3, 0.1]),
            3 => ([0.3, 0.3, 0.3], [0.2, 0.2, 0.2]),
            4 => ([0.2, 0.2, 0.5], [0.1, 0.1, 0.3]),
            _ => ([0.4, 0.2, 0.5], [0.3, 0.2, 0.3]),
        };
        let [r, g, b] = bright;
        let [dr, dg, db] = dark;
        Ramp::new(
            [1.0; 4],
            &[
                ([r, g, b, 1.0], 8),
                ([dr, dg, db, 0.8], 192),
                ([0.0; 4], 56),
            ],
        )
    }

    /// Its sparks `dt` seconds on: aged and emitted.
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) {
        let layers = Self::layers();
        self.system.update(dt, &layers);
        self.system
            .emit(&[Self::block()], &layers, GLOW_COUNT, dice);
    }

    /// Its flare's half side at its clock: `17 + 3·(sin 2t + 1)/2`.
    pub fn flare(&self) -> f32 {
        17.0 + 3.0 * ((2.0 * self.system.clock).sin() + 1.0) / 2.0
    }
}

/// A trail's points: its whip's or its streak's fifty, each a place and a velocity.
const TRAIL_POINTS: usize = 50;
/// The share of a point's move that it keeps as its velocity (`13.5 · 0.05`).
const KEEP: f32 = 13.5 * 0.05;

/// One step of a trail's points after its first (`0x787330`, `0x787AC0`): each the `step` from
/// the one before it along its own offset from it, plus its velocity and `pull(k)`, its velocity
/// its move times [`KEEP`].
fn follow(points: &mut [([f32; 3], [f32; 3])], step: f32, pull: impl Fn(usize) -> [f32; 3]) {
    for k in 1..points.len() {
        let (before, _) = points[k - 1];
        let (place, velocity) = points[k];
        let tug = pull(k - 1);
        let way: [f32; 3] = std::array::from_fn(|i| place[i] - before[i] + velocity[i] + tug[i]);
        let length = way.iter().map(|v| v * v).sum::<f32>().sqrt();
        let unit = if length > 0.0 {
            way.map(|v| v / length)
        } else {
            [0.0; 3]
        };
        let new: [f32; 3] = std::array::from_fn(|i| before[i] + unit[i] * step);
        points[k] = (new, std::array::from_fn(|i| (new[i] - place[i]) * KEEP));
    }
}

/// The whip's and the streak's texture.
pub const TRAIL_TEXTURE: &str = "PARTICLE_GLOW01";

/// The whip 3073's effect (824, `cParticleSystem_magicwhip`, its constructor `0x787120`, start
/// `0x787760`, run `0x787330` and draw `0x7874A0`): fifty points 1.5 apart trailing from its
/// place, each drawn as a camera-facing quad of the half side 4 of `PARTICLE_GLOW01`, its corners
/// blue and violet.
#[derive(Clone, Debug, PartialEq)]
pub struct MagicWhip {
    pub points: Vec<([f32; 3], [f32; 3])>,
}

impl Default for MagicWhip {
    fn default() -> Self {
        Self::new()
    }
}

impl MagicWhip {
    /// Its quad's half side and its corners' colours, from the bottom left, the top left, the
    /// bottom right and the top right.
    pub const HALF: f32 = 4.0;
    pub const CORNERS: [[f32; 4]; 4] = [
        [0.0, 0.0, 128.0 / 255.0, 1.0],
        [64.0 / 255.0, 64.0 / 255.0, 128.0 / 255.0, 1.0],
        [0.0, 64.0 / 255.0, 128.0 / 255.0, 1.0],
        [64.0 / 255.0, 0.0, 128.0 / 255.0, 1.0],
    ];

    /// A whip hanging down from its place, its points 1.5 apart along −y.
    pub fn new() -> Self {
        let points = (0..TRAIL_POINTS)
            .map(|i| {
                let i = f32::from(u8::try_from(i).unwrap_or(0));
                ([0.0, -1.5 * i, 0.0], [0.0; 3])
            })
            .collect();
        Self { points }
    }

    /// A frame on, its first point at `place` (`0x787330`): each frame whatever its time.
    pub fn update(&mut self, place: [f32; 3]) {
        if let Some(first) = self.points.first_mut() {
            first.0 = place;
        }
        follow(&mut self.points, 1.5, |_| [0.0; 3]);
    }
}

/// The wings' streaks (837, `cParticleSystem_magicstreak`, its constructor `0x7877C0`, run
/// `0x787AC0` and draw `0x787E70`): fifty points 0.75 apart trailing from its place, drawn out
/// along its bone's `x` axis, moved in steps of 1/200 s, each drawn as a small blue quad and a
/// large faint one.
#[derive(Clone, Debug, PartialEq)]
pub struct MagicStreak {
    pub points: Vec<([f32; 3], [f32; 3])>,
}

impl Default for MagicStreak {
    fn default() -> Self {
        Self::new()
    }
}

impl MagicStreak {
    /// Its small quad's half side and corners, and its large one's.
    pub const HALF: f32 = 1.5;
    pub const CORNERS: [[f32; 4]; 4] = [
        [0.0, 0.0, 192.0 / 255.0, 1.0],
        [128.0 / 255.0, 128.0 / 255.0, 192.0 / 255.0, 1.0],
        [0.0, 128.0 / 255.0, 192.0 / 255.0, 1.0],
        [128.0 / 255.0, 0.0, 192.0 / 255.0, 1.0],
    ];
    pub const HAZE: f32 = 10.0;
    pub const HAZE_CORNERS: [[f32; 4]; 4] = [
        [0.0, 0.0, 64.0 / 255.0, 64.0 / 255.0],
        [32.0 / 255.0, 32.0 / 255.0, 64.0 / 255.0, 64.0 / 255.0],
        [0.0, 32.0 / 255.0, 64.0 / 255.0, 64.0 / 255.0],
        [32.0 / 255.0, 0.0, 64.0 / 255.0, 64.0 / 255.0],
    ];

    /// A streak of all its points at its place, none yet placed.
    pub fn new() -> Self {
        Self {
            points: vec![([0.0; 3], [0.0; 3]); TRAIL_POINTS],
        }
    }

    /// `dt` seconds on, its place now `place` and its bone's `x` axis `axis` (`0x787AC0`): its
    /// time held at 0.5 s and cut into `⌊200·t⌋` steps, in each its first point moved on toward
    /// its place and the others after it, pulled along the axis by a weight that falls from 0.7
    /// by 0.014 a point.
    pub fn update(&mut self, dt: f32, place: [f32; 3], axis: [f32; 3]) {
        let Some(&(first, _)) = self.points.first() else {
            return;
        };
        let from = if first[0] == 0.0 && first[1] == 0.0 {
            place
        } else {
            first
        };
        let steps = crate::fight::truncated(f64::from(200.0 * dt.min(0.5)));
        let n = u16::try_from(steps).unwrap_or(0);
        for s in 0..n {
            let share = f32::from(s) / f32::from(n);
            self.points[0].0 = std::array::from_fn(|i| from[i] + (place[i] - from[i]) * share);
            follow(&mut self.points, 0.75, |k| {
                let k = f32::from(u8::try_from(k).unwrap_or(0));
                axis.map(|v| v * (0.7 - 0.014 * k))
            });
        }
    }
}

/// The sword's sparks' texture, their particles and their colour.
pub const WORMS_TEXTURE: &str = "PARTICLE_GLOW01";
const WORMS_COUNT: usize = 100;
pub const WORMS_COLOUR: [f32; 3] = [1.0, 68.0 / 255.0, 68.0 / 255.0];

/// The sword 1771's sparks (858, `cParticleSystem_magicworms`, its start `0x77C870`, run
/// `0x77C590` and draw `0x77C600`): red sparks of `PARTICLE_GLOW01` born within 40 of its place
/// each way, living half a second, their alpha their life's, added.
#[derive(Clone, Debug, PartialEq)]
pub struct MagicWorms {
    pub system: System,
}

impl Default for MagicWorms {
    fn default() -> Self {
        Self {
            system: System::new(WORMS_COUNT),
        }
    }
}

impl MagicWorms {
    /// Its layer: the life −500 a second.
    pub fn layers() -> [Layer; 1] {
        [Layer {
            life: -500.0,
            ..Layer::default()
        }]
    }

    /// Its block: the size 1 within 40 of its place each way, one every 0.02 s at the detail 2.
    pub fn block() -> Block {
        Block {
            size: Spread {
                value: 1.0,
                spread: 0.0,
            },
            offset: Spread {
                value: [0.0; 3],
                spread: [40.0; 3],
            },
            interval: 0.02,
            ..Block::default()
        }
    }

    /// Its sparks `dt` seconds on: aged and emitted.
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) {
        let layers = Self::layers();
        self.system.update(dt, &layers);
        self.system
            .emit(&[Self::block()], &layers, WORMS_COUNT, dice);
    }
}

#[cfg(test)]
mod tests;
