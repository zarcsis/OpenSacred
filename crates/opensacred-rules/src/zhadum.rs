//! The zhadum (1165, `FX_ZHADUM`, `cParticleSystem_zhadum`; #2920; docs/re/effects.md, "The
//! zhadum"): the guard that the `Enter` of the sector 91,35 makes beside the sword that the start
//! lays there. Once a second, while any living creature stands within its reach, the hero among
//! them, it takes one of them at random, lays a laser beam of the variant 5 at it and on
//! ([`crate::particles::ZHADUM_BEAM`]), and kills it outright; its glow flashes white for a second
//! and fades over two thirds of one ([`Zhadum`]).
//!
//! Its beam's direction and reach are the effects' frame's on the ground (`x` along the screen's
//! horizontal, `y` toward the viewer; docs/re/effects.md, "Their flight"); its reach the world's.

use crate::spawning::Dice;

/// Its FX type.
pub const ZHADUM: u32 = 1165;
/// Its size (`items.pak +0x14`), by which it reaches a creature within it and the creature's own.
pub const SIZE: f32 = 1000.0;
/// How far its beam goes toward its target in the effects' frame: 500 of the frame that
/// `0x623AD0` takes with its flag 1, four times the effects' frame (some 2,700 to 2,860 of the
/// world's units, by the direction).
pub const BEAM_REACH: f32 = 2000.0;
/// Its glow (`0x762E10` of the style 0): `PARTICLE_FLARE02`, of a half side of 60, added whole.
pub const GLOW_TEXTURE: &str = "PARTICLE_FLARE02";
pub const GLOW_HALF: f32 = 60.0;
/// The seconds between its kills (`+0x11C`), its glow's hold and its fade's fall a second.
const PERIOD: f32 = 1.0;
const HOLD: f32 = 1.0;
const FADE: f32 = 1.5;

/// A zhadum's state: its kills' cooldown (the object's `+0x11C`), its glow's hold (`+0x20A8`)
/// and its fade (`+0x20AC`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Zhadum {
    pub cooldown: f32,
    pub hold: f32,
    pub fade: f32,
}

impl Zhadum {
    /// Its run of `dt` seconds (`0x7C2EE0`), then its cooldown's (`0x5A6680`): the hold falls and,
    /// below 0, ends and starts the fade, which falls by 1.5 a second to 0.
    pub fn update(&mut self, dt: f32) {
        if self.hold > 0.0 {
            self.hold -= dt;
            if self.hold < 0.0 {
                self.hold = 0.0;
                self.fade = 1.0;
            }
        }
        self.fade = (self.fade - FADE * dt).max(0.0);
        self.cooldown = (self.cooldown - dt).max(0.0);
    }

    /// Which of its `targets` living creatures within its reach it kills now (`0x5ABD80`, the
    /// case `0x48D`): none while its cooldown runs; else one at random (`0x443B20(0, n − 1)`), its
    /// cooldown a second and its glow held (its signal 8).
    pub fn strike(&mut self, targets: usize, dice: &mut impl Dice) -> Option<usize> {
        if targets == 0 || self.cooldown != 0.0 {
            return None;
        }
        self.cooldown = PERIOD;
        self.hold = HOLD;
        let count = u32::try_from(targets).unwrap_or(u32::MAX);
        usize::try_from(dice.below(count)).ok()
    }

    /// Its glow's grey (`0x7C2F90`): `⌊255·b⌋/255`, `b` 1 while it holds, else its fade.
    pub fn glow(&self) -> f32 {
        let b = if self.hold > 0.0 { 1.0 } else { self.fade };
        (255.0 * b).floor() / 255.0
    }

    /// Its beam's far end about its place, toward `toward` (any length) in the effects' frame:
    /// [`BEAM_REACH`] along it (`0x5B216B`); toward nothing, its place.
    pub fn beam(toward: [f32; 2]) -> [f32; 2] {
        let length = toward[0].hypot(toward[1]);
        if length == 0.0 {
            return [0.0; 2];
        }
        toward.map(|v| v / length * BEAM_REACH)
    }
}

#[cfg(test)]
mod tests;
