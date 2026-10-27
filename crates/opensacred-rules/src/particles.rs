//! The particle systems' machinery (docs/re/effects.md, "The particle systems"), with no engine:
//! the particles of 64 bytes, the emitter layers of their rates, the emission blocks, the colour
//! ramps of 256 entries over a particle's life, the update of `0x7640C0` and the emission of
//! `0x764760`; and the fires of `cParticleSystem_smoke` (#1092, docs/re/effects.md, "The fires and
//! smokes"), its variants 4 to 6 and 10 to 12, a flame's layer and a smoke's.
//!
//! Places, sizes and speeds are in the models' units, the third coordinate up.

use crate::spawning::Dice;

/// A particle (`0x7640C0`, `0x764760`): its place, velocity, buoyancy (each second its upward
/// velocity falls by it, so a negative one rises), size (the half side of its quad), life (255 at
/// its birth, dying at 0), angle and its speed, and its layer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    pub place: [f32; 3],
    pub velocity: [f32; 3],
    pub buoyancy: f32,
    pub size: f32,
    pub life: f32,
    pub angle: f32,
    pub spin: f32,
    pub layer: u8,
}

impl Particle {
    /// Whether it lives: its life and its size above 0.
    pub fn alive(&self) -> bool {
        self.life > 0.0 && self.size > 0.0
    }
}

/// An emitter layer's rates a second (32 bytes): of the buoyancy, the life (−200 lives 1.275 s),
/// the size and the angle; the wind its buoyancy carries it by.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Layer {
    pub buoyancy: f32,
    pub life: f32,
    pub size: f32,
    pub angle: f32,
    pub wind: [f32; 3],
}

/// A value and its spread, which adds a uniform `[−spread, spread]`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spread<T> {
    pub value: T,
    pub spread: T,
}

const fn spread<T>(value: T, spread: T) -> Spread<T> {
    Spread { value, spread }
}

/// An emission block (100 bytes a layer): a particle's buoyancy, size, angle, its speed, its
/// velocity and its offset from the system's place, each with its spread, and the interval
/// between two particles in seconds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Block {
    pub buoyancy: Spread<f32>,
    pub size: Spread<f32>,
    pub angle: Spread<f32>,
    pub spin: Spread<f32>,
    pub velocity: Spread<[f32; 3]>,
    pub offset: Spread<[f32; 3]>,
    pub interval: f32,
}

/// A colour ramp over a particle's life (`0x763F50`): 256 entries of red, green, blue and alpha in
/// 0 to 1, the entry 255 the colour at its birth and the entry 0 at its death.
#[derive(Clone, Debug, PartialEq)]
pub struct Ramp(pub [[f32; 4]; 256]);

impl Ramp {
    /// The ramp from `start` through `keys`, each a colour reached linearly over its count of
    /// entries, from the entry 255 down; the counts sum to 256.
    pub fn new(start: [f32; 4], keys: &[([f32; 4], u16)]) -> Self {
        let mut entries = [[0.0; 4]; 256];
        let mut at = 255_usize;
        let mut from = start;
        let mut first = true;
        for &(to, count) in keys {
            for step in 0..count {
                let share = (f32::from(step) + 1.0) / f32::from(count);
                let colour = if first && step == 0 {
                    from
                } else {
                    std::array::from_fn(|k| from[k] + (to[k] - from[k]) * share)
                };
                if let Some(entry) = entries.get_mut(at) {
                    *entry = colour;
                }
                at = at.saturating_sub(1);
            }
            first = false;
            from = to;
        }
        Self(entries)
    }

    /// The colour at the life `life`, its entry rounded down.
    pub fn at(&self, life: f32) -> [f32; 4] {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a life of 0 to 255, rounded down to its entry"
        )]
        let entry = life.clamp(0.0, 255.0) as usize;
        self.0[entry.min(255)]
    }
}

/// A uniform number of `[−1, 1]` by `dice`, as the client's `rand()` / 32767 · 2 − 1.
fn signed(dice: &mut impl Dice) -> f32 {
    let roll = f32::from(u16::try_from(dice.below(0x8000)).unwrap_or(0)) / 32_767.0;
    roll * 2.0 - 1.0
}

fn spread_f32(value: Spread<f32>, dice: &mut impl Dice) -> f32 {
    value.value + value.spread * signed(dice)
}

fn spread_3(value: Spread<[f32; 3]>, dice: &mut impl Dice) -> [f32; 3] {
    std::array::from_fn(|k| value.value[k] + value.spread[k] * signed(dice))
}

/// A system's particles, its clock and when it last emitted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct System {
    pub particles: Vec<Particle>,
    pub clock: f32,
    last: f32,
}

impl System {
    /// A system of up to `count` particles.
    pub fn new(count: usize) -> Self {
        Self {
            particles: Vec::with_capacity(count),
            clock: 0.0,
            last: 0.0,
        }
    }

    /// One frame of `dt` seconds of each living particle by its layer's rates (`0x7640C0`): its
    /// place moves by its velocity, its upward velocity falls by `dt` times its buoyancy and its
    /// velocity takes that much of the wind, its angle turns by its speed, and its buoyancy, life,
    /// size and angle change by the layer's rates; the dead are dropped.
    pub fn update(&mut self, dt: f32, layers: &[Layer]) {
        self.clock += dt;
        for particle in &mut self.particles {
            let Some(layer) = layers.get(usize::from(particle.layer)) else {
                particle.life = 0.0;
                continue;
            };
            age(particle, dt, layer);
        }
        self.particles.retain(Particle::alive);
    }

    /// Emits, at the system's place (the origin), as many particles as whole intervals of the
    /// first block have passed since the last emission, each of a block that `pick` chooses and
    /// aged by its share of the time (`0x764760`), up to `count` alive.
    pub fn emit(&mut self, blocks: &[Block], layers: &[Layer], count: usize, dice: &mut impl Dice) {
        let Some(first) = blocks.first() else {
            return;
        };
        if first.interval <= 0.0 {
            return;
        }
        while self.last + first.interval <= self.clock {
            self.last += first.interval;
            if self.particles.len() >= count {
                continue;
            }
            let layer = pick(blocks.len(), dice);
            let (Some(block), Some(rates)) = (blocks.get(layer), layers.get(layer)) else {
                continue;
            };
            let mut particle = Particle {
                place: spread_3(block.offset, dice),
                velocity: spread_3(block.velocity, dice),
                buoyancy: spread_f32(block.buoyancy, dice),
                size: spread_f32(block.size, dice),
                life: 255.0,
                angle: spread_f32(block.angle, dice),
                spin: spread_f32(block.spin, dice),
                layer: u8::try_from(layer).unwrap_or(0),
            };
            age(&mut particle, self.clock - self.last, rates);
            if particle.alive() {
                self.particles.push(particle);
            }
        }
    }
}

/// A particle aged by `dt` seconds under the layer `layer`.
fn age(particle: &mut Particle, dt: f32, layer: &Layer) {
    for k in 0..3 {
        particle.place[k] += dt * particle.velocity[k];
    }
    let fall = dt * particle.buoyancy;
    particle.velocity[2] -= fall;
    for k in 0..3 {
        particle.velocity[k] += fall * layer.wind[k];
    }
    particle.angle += dt * particle.spin;
    particle.buoyancy += dt * layer.buoyancy;
    particle.life += dt * layer.life;
    particle.size += dt * layer.size;
    particle.angle += dt * layer.angle;
}

/// The layer that a particle of `layers` blocks takes (`0x7687F0`): the first for one, the first
/// four times in five for two.
fn pick(layers: usize, dice: &mut impl Dice) -> usize {
    match layers {
        2 => usize::from(dice.below(5) == 0),
        _ => 0,
    }
}

/// How a layer of particles is drawn (docs/re/effects.md, "The billboards"): added to the frame
/// by its alpha (`SRCALPHA`, `ONE`), or laid over it by its alpha (`SRCALPHA`, `INVSRCALPHA`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    Add,
    Over,
}

/// A fire of `cParticleSystem_smoke`: its texture's name, its layers, blocks, ramps and blends,
/// its count of particles and the radius of its light.
#[derive(Clone, Debug, PartialEq)]
pub struct Fire {
    pub texture: &'static str,
    pub layers: Vec<Layer>,
    pub blocks: Vec<Block>,
    pub ramps: Vec<Ramp>,
    pub blends: Vec<Blend>,
    pub count: usize,
    pub light: f32,
}

/// The smoke class's particles (`0x76E200`).
pub const SMOKE_PARTICLES: usize = 200;

/// The flame's ramp A: from (254, 243, 131) at an alpha of 90 to (254, 80, 40) opaque over 22
/// entries, to (232, 67, 20) at 229 over 29, to black at 139 over 85, to transparent black over 120.
fn flame_ramp() -> Ramp {
    let c = |r: f32, g: f32, b: f32, a: f32| [r / 255.0, g / 255.0, b / 255.0, a / 255.0];
    Ramp::new(
        c(254.0, 243.0, 131.0, 90.0),
        &[
            (c(254.0, 80.0, 40.0, 255.0), 22),
            (c(232.0, 67.0, 20.0, 229.0), 29),
            (c(0.0, 0.0, 0.0, 139.0), 85),
            (c(0.0, 0.0, 0.0, 0.0), 120),
        ],
    )
}

/// The fire smoke's ramp B, its alpha `alpha`: from a grey of 60 at an alpha of 0 to a grey of 33
/// at `alpha` over 110 entries, to transparent black over 146.
fn fire_smoke_ramp(alpha: f32) -> Ramp {
    let grey = |g: f32, a: f32| [g / 255.0, g / 255.0, g / 255.0, a];
    Ramp::new(
        grey(60.0, 0.0),
        &[(grey(33.0, alpha), 110), (grey(0.0, 0.0), 146)],
    )
}

/// The fire's smoke's ramp C: from a grey of 159 at an alpha of 0 to a grey of 133 at 80 over
/// 110 entries, to a grey of 100 at 0 over 146.
fn smoke_ramp() -> Ramp {
    let grey = |g: f32, a: f32| [g / 255.0, g / 255.0, g / 255.0, a / 255.0];
    Ramp::new(
        grey(159.0, 0.0),
        &[(grey(133.0, 80.0), 110), (grey(100.0, 0.0), 146)],
    )
}

/// The fire of the smoke class's variant `variant` (the FX event's `+0x38`): 4, 5 and 6 the fire
/// smokes `FX_FI_SMOKE_S`, `_M` and `_L`, 10, 11 and 12 the fires `FX_FIRE_L` of the radii 40,
/// 70 and 100 (docs/re/effects.md, the table of the variants); none for the others.
pub fn fire(variant: u8) -> Option<Fire> {
    // The flame's and the smoke's life and growth a second, the flame's and the smoke's sizes, the
    // places' spread along x and y, the velocities' spreads, the smoke's ramp and the light.
    let (lives, sizes, flame_size, smoke_size, place, velocities, smoke, light) = match variant {
        4 => (
            [-200.0, -180.0],
            [15.0, 10.0],
            spread(0.5, 0.2),
            spread(1.0, 0.5),
            [4.0, 0.0],
            [0.2, 0.3],
            fire_smoke_ramp(0.5),
            280.0,
        ),
        5 => (
            [-150.0, -130.0],
            [17.5, 10.0],
            spread(0.75, 0.35),
            spread(2.0, 0.75),
            [9.0, 0.0],
            [0.35, 0.4],
            fire_smoke_ramp(0.77),
            370.0,
        ),
        6 => (
            [-100.0, -80.0],
            [20.0, 10.0],
            spread(1.0, 0.5),
            spread(3.0, 1.0),
            [9.0, 0.0],
            [0.5, 0.5],
            fire_smoke_ramp(1.0),
            530.0,
        ),
        10 => (
            [-200.0, -200.0],
            [8.0, 10.0],
            spread(0.5, 0.2),
            spread(1.0, 0.5),
            [3.5, 1.0],
            [0.2, 0.3],
            smoke_ramp(),
            280.0,
        ),
        11 => (
            [-160.0, -150.0],
            [10.0, 10.0],
            spread(0.75, 0.35),
            spread(2.0, 0.75),
            [7.0, 0.0],
            [0.35, 0.4],
            smoke_ramp(),
            370.0,
        ),
        12 => (
            [-100.0, -100.0],
            [12.0, 12.0],
            spread(1.0, 0.5),
            spread(3.0, 1.0),
            [9.0, 0.0],
            [0.5, 0.5],
            smoke_ramp(),
            530.0,
        ),
        _ => return None,
    };
    let layer = |k: usize| Layer {
        life: lives[k],
        size: sizes[k],
        ..Layer::default()
    };
    let block = |buoyancy: Spread<f32>, size: Spread<f32>, velocity: f32| Block {
        buoyancy,
        size,
        velocity: spread([0.0; 3], [velocity; 3]),
        offset: spread([0.0; 3], [place[0], place[1], 0.0]),
        interval: 0.02,
        ..Block::default()
    };
    Some(Fire {
        texture: "particle_fire03",
        layers: vec![layer(0), layer(1)],
        blocks: vec![
            block(spread(-35.0, 34.0), flame_size, velocities[0]),
            block(spread(-35.0, 5.0), smoke_size, velocities[1]),
        ],
        ramps: vec![flame_ramp(), smoke],
        blends: vec![Blend::Add, Blend::Over],
        count: SMOKE_PARTICLES,
        light,
    })
}

/// The smoke class's variant of the FX type `kind` that the scripts' `CreateObj` gives it
/// (`0x48907C`), for the fires: 812, 813 and 876 the fire smokes, 886, 885 and 884 the fires.
pub fn fire_variant(kind: u32) -> Option<u8> {
    match kind {
        812 => Some(4),
        813 => Some(5),
        876 => Some(6),
        886 => Some(10),
        885 => Some(11),
        884 => Some(12),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spawning::Rng;

    #[test]
    fn a_ramp_runs_from_its_birth_to_its_death() {
        let ramp = flame_ramp();
        let close = |a: f32, b: f32| (a - b).abs() < 1e-3;
        assert!(close(ramp.at(255.0)[3], 90.0 / 255.0));
        // The 22nd entry, opaque.
        assert!(close(ramp.at(255.0 - 21.0)[3], 1.0));
        assert_eq!(ramp.at(0.0), [0.0; 4]);
    }

    #[test]
    fn a_fire_rises_and_dies_by_its_layers() {
        let fire = fire(10).unwrap();
        let mut system = System::new(fire.count);
        let mut dice = Rng::of(0, 0, 7);
        for _ in 0..60 {
            system.update(1.0 / 60.0, &fire.layers);
            system.emit(&fire.blocks, &fire.layers, fire.count, &mut dice);
        }
        // About 50 a second, the flames four in five.
        let flames = system.particles.iter().filter(|p| p.layer == 0).count();
        let smokes = system.particles.len() - flames;
        assert!(
            (40..=60).contains(&system.particles.len()),
            "{}",
            system.particles.len()
        );
        assert!(flames > smokes);
        // The flames rise.
        assert!(system.particles.iter().all(|p| p.velocity[2] > -1.0));
        // A flame lives 1.275 s.
        for _ in 0..90 {
            system.update(1.0 / 60.0, &fire.layers);
        }
        assert!(system.particles.iter().all(|p| p.layer == 1));
        assert_eq!(fire_variant(884), Some(12));
    }
}
