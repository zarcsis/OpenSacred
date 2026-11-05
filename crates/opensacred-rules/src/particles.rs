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
    /// Its frame of the texture's grid.
    pub frame: u8,
}

impl Particle {
    /// Whether it lives: its life and its size above 0.
    pub fn alive(&self) -> bool {
        self.life > 0.0 && self.size > 0.0
    }
}

/// An emitter layer's rates a second (32 bytes): of the buoyancy, the life (−200 lives 1.275 s),
/// the size and the angle; the wind its buoyancy carries it by; and its pull toward the system's
/// place (`+0x1C`), by which its particles' velocity turns toward it each second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Layer {
    pub buoyancy: f32,
    pub life: f32,
    pub size: f32,
    pub angle: f32,
    pub wind: [f32; 3],
    pub pull: f32,
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
    /// A frame of the texture's grid at random below it, the first for 0 or 1, the particle's
    /// layer for [`FRAME_OF_LAYER`] (`+0x60`).
    pub frames: u8,
}

/// A block's frames that give each particle the frame of its layer's index (`0x764760`).
pub const FRAME_OF_LAYER: u8 = 0xFF;

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
        self.update_over(dt, layers, None);
    }

    /// [`Self::update`], each particle bouncing at the height `ground` of the system's frame
    /// without loss (`0x7645C0`, the mode 0): its vertical velocity turned about where its place
    /// lies at or below it, after its move and before its layer's rates.
    pub fn update_over(&mut self, dt: f32, layers: &[Layer], ground: Option<f32>) {
        self.run(dt, layers, ground.map(Ground::Bounces));
    }

    /// [`Self::update`], each particle dying at the height `ground` of the system's frame
    /// (`0x7645C0`, the mode 2): its life set to 0 where its place lies at or below it, after its
    /// move and before its layer's rates.
    pub fn update_to_ground(&mut self, dt: f32, layers: &[Layer], ground: f32) {
        self.run(dt, layers, Some(Ground::Stops(ground)));
    }

    /// [`Self::update`] of a system at `place`, which its layers' pull draws its particles to.
    pub fn update_toward(&mut self, dt: f32, layers: &[Layer], place: [f32; 3]) {
        self.clock += dt;
        for particle in &mut self.particles {
            let Some(layer) = layers.get(usize::from(particle.layer)) else {
                particle.life = 0.0;
                continue;
            };
            age(particle, dt, layer, None, Some(place));
        }
        self.particles.retain(Particle::alive);
    }

    fn run(&mut self, dt: f32, layers: &[Layer], ground: Option<Ground>) {
        self.clock += dt;
        for particle in &mut self.particles {
            let Some(layer) = layers.get(usize::from(particle.layer)) else {
                particle.life = 0.0;
                continue;
            };
            age(particle, dt, layer, ground, None);
        }
        self.particles.retain(Particle::alive);
    }

    /// Emits, at the system's place (the origin), as many particles as whole intervals of the
    /// first block have passed since the last emission, each of a block that `pick` chooses and
    /// aged by its share of the time (`0x764760`), up to `count` alive.
    pub fn emit(&mut self, blocks: &[Block], layers: &[Layer], count: usize, dice: &mut impl Dice) {
        self.emit_at(blocks, layers, count, dice, |_, _| [0.0; 3]);
    }

    /// [`Self::emit`] by a system whose place went from `from` to `to` in the frame of `dt`
    /// seconds that its clock ran last (no flag 2: the particles stay in the world): each
    /// particle born at its offset from the place the system passed at its moment, so that a
    /// trail lies along its way.
    pub fn emit_along(
        &mut self,
        blocks: &[Block],
        layers: &[Layer],
        count: usize,
        (from, to): ([f32; 3], [f32; 3]),
        dt: f32,
        dice: &mut impl Dice,
    ) {
        let start = self.clock - dt;
        self.emit_at(blocks, layers, count, dice, |moment, _| {
            let share = if dt > 0.0 {
                ((moment - start) / dt).clamp(0.0, 1.0)
            } else {
                1.0
            };
            std::array::from_fn(|k| from[k] + (to[k] - from[k]) * share)
        });
    }

    /// [`Self::emit_along`] by a system of two points that went from `from` to `to` (`0x766200`):
    /// each particle born at a point at random along the segment between them at its moment.
    pub fn emit_along_segment(
        &mut self,
        blocks: &[Block],
        layers: &[Layer],
        count: usize,
        (from, to): ([[f32; 3]; 2], [[f32; 3]; 2]),
        dt: f32,
        dice: &mut impl Dice,
    ) {
        let start = self.clock - dt;
        self.emit_at(blocks, layers, count, dice, |moment, dice| {
            let share = if dt > 0.0 {
                ((moment - start) / dt).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let [a, b]: [[f32; 3]; 2] = std::array::from_fn(|end| {
                std::array::from_fn(|k| from[end][k] + (to[end][k] - from[end][k]) * share)
            });
            let along = f32::from(u16::try_from(dice.below(0x8000)).unwrap_or(0)) / 32_767.0;
            std::array::from_fn(|k| a[k] + (b[k] - a[k]) * along)
        });
    }

    /// A burst of `count` particles of `block` at once, of the layer 0, while the system has room
    /// below `room` (`0x764760` of a block of the interval 0).
    pub fn burst(&mut self, block: &Block, count: usize, room: usize, dice: &mut impl Dice) {
        for _ in 0..count {
            if self.particles.len() >= room {
                break;
            }
            self.particles.push(born(block, 0, dice));
        }
    }

    /// [`Self::emit`], each particle born at its offset from `place` of its moment: the first
    /// block's offset, whatever the particle's block, and its own block's spread about it, as the
    /// steady emission of `0x764760` reads the first block's `+0x38` alone (#2070).
    fn emit_at<D: Dice>(
        &mut self,
        blocks: &[Block],
        layers: &[Layer],
        count: usize,
        dice: &mut D,
        mut place: impl FnMut(f32, &mut D) -> [f32; 3],
    ) {
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
            let layer = pick(blocks, dice);
            let (Some(block), Some(rates)) = (blocks.get(layer), layers.get(layer)) else {
                continue;
            };
            let mut particle = born(block, layer, dice);
            let at = place(self.last, dice);
            let shift: [f32; 3] =
                std::array::from_fn(|k| at[k] + first.offset.value[k] - block.offset.value[k]);
            for (axis, shift) in particle.place.iter_mut().zip(shift) {
                *axis += shift;
            }
            age(&mut particle, self.clock - self.last, rates, None, None);
            if particle.alive() {
                self.particles.push(particle);
            }
        }
    }
}

/// A particle of the block `block` of the layer `layer`, at its offset from the system's place,
/// each value rolled about its spread (`0x764760`).
fn born(block: &Block, layer: usize, dice: &mut impl Dice) -> Particle {
    Particle {
        place: spread_3(block.offset, dice),
        velocity: spread_3(block.velocity, dice),
        buoyancy: spread_f32(block.buoyancy, dice),
        size: spread_f32(block.size, dice),
        life: 255.0,
        angle: spread_f32(block.angle, dice),
        spin: spread_f32(block.spin, dice),
        layer: u8::try_from(layer).unwrap_or(0),
        frame: match block.frames {
            FRAME_OF_LAYER => u8::try_from(layer).unwrap_or(0),
            frames if frames > 1 => u8::try_from(dice.below(u32::from(frames))).unwrap_or(0),
            _ => 0,
        },
    }
}

/// What a particle does at its system's ground (`0x7645C0`'s mode): it bounces without loss (the
/// mode 0) or it dies (the mode 2).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Ground {
    Bounces(f32),
    Stops(f32),
}

/// A particle aged by `dt` seconds under the layer `layer`, at `ground` where given, drawn by the
/// layer's pull toward the system's place `centre` where given: its velocity less `dt` times the
/// pull along the unit way from the place to it.
fn age(
    particle: &mut Particle,
    dt: f32,
    layer: &Layer,
    ground: Option<Ground>,
    centre: Option<[f32; 3]>,
) {
    for k in 0..3 {
        particle.place[k] += dt * particle.velocity[k];
    }
    let fall = dt * particle.buoyancy;
    particle.velocity[2] -= fall;
    for k in 0..3 {
        particle.velocity[k] += fall * layer.wind[k];
    }
    particle.angle += dt * particle.spin;
    if let Some(centre) = centre.filter(|_| layer.pull != 0.0) {
        let away: [f32; 3] = std::array::from_fn(|k| particle.place[k] - centre[k]);
        let length = away.iter().map(|v| v * v).sum::<f32>().sqrt();
        if length >= 1e-7 {
            let pull = dt * layer.pull / length;
            for (velocity, away) in particle.velocity.iter_mut().zip(away) {
                *velocity -= pull * away;
            }
        }
    }
    match ground {
        Some(Ground::Bounces(ground)) if particle.place[2] <= ground => {
            particle.velocity[2] = -particle.velocity[2];
        }
        Some(Ground::Stops(ground)) if particle.place[2] <= ground => particle.life = 0.0,
        _ => {}
    }
    particle.buoyancy += dt * layer.buoyancy;
    particle.life += dt * layer.life;
    particle.size += dt * layer.size;
    particle.angle += dt * layer.angle;
}

/// The layer that a particle of the blocks `blocks` takes (`0x7687F0`): the first for one, the
/// first four times in five for two, and for three each by its share of their rates, the inverses
/// of their intervals, by one roll.
fn pick(blocks: &[Block], dice: &mut impl Dice) -> usize {
    match blocks {
        [_, _] => usize::from(dice.below(5) == 0),
        [a, b, c] => {
            let rate = |block: &Block| {
                if block.interval > 0.0 {
                    1.0 / block.interval
                } else {
                    0.0
                }
            };
            let rates = [rate(a), rate(b), rate(c)];
            let whole: f32 = rates.iter().sum();
            let roll = f32::from(u16::try_from(dice.below(0x8000)).unwrap_or(0)) / 32_767.0;
            if roll < rates[0] / whole {
                0
            } else if roll >= (rates[0] + rates[1]) / whole {
                2
            } else {
                1
            }
        }
        _ => 0,
    }
}

/// How a layer of particles is drawn (docs/re/effects.md, "The billboards"): added to the frame
/// by its alpha (`SRCALPHA`, `ONE`), added whole (`ONE`, `ONE`), or laid over it by its alpha
/// (`SRCALPHA`, `INVSRCALPHA`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Blend {
    Add,
    Whole,
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
    /// The texture's frames along each side: 1 whole, 2 a grid of 2 × 2.
    pub grid: u8,
    pub count: usize,
    pub light: f32,
    /// Whether the light at it tints its colours (the flags' bit `0x40`): the chimneys' smokes and
    /// the geyser's jet darken by night, the fires and the lavas' smokes keep their colours.
    pub tinted: bool,
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
        _ => return smoke(variant),
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
        grid: 1,
        count: SMOKE_PARTICLES,
        light,
        tinted: false,
    })
}

/// How long the steps' small fire emits, in seconds (`0x890918`; [`small_fire`]), after which it
/// lives on until its last particle dies.
pub const SMALL_FIRE_EMITS: f32 = 8.0;
/// The steps' small fire's burnt ground: its quad's half side in the models' units, its texture's
/// frames along each side, and the texture (`FX_GROUNDSTEP`, `0x79F7D0`).
pub const GROUND_HALF: f32 = 50.0;
pub const GROUND_GRID: u8 = 4;
pub const GROUND_TEXTURE: &str = "fx_groundstep";

/// The small fire of `cParticleSystem_smallfire` (`0x79F600`, the type 878, `FX_SMALLFIRE`) of the
/// variant 2, which six pieces of set 23 leave at each of the hero's steps (#1808; docs/re/
/// effects.md): the smoke class's `FX_FI_SMOKE_L` in its flame, its places spread ± 15 along
/// both axes of the ground, a shorter smoke that spins ± 2, and a light of 150 (`0x79F970`); it
/// emits for [`SMALL_FIRE_EMITS`] seconds.
pub fn small_fire() -> Fire {
    let block = |buoyancy: Spread<f32>, size: Spread<f32>, spin: f32| Block {
        buoyancy,
        size,
        angle: spread(std::f32::consts::PI, std::f32::consts::PI),
        spin: spread(0.0, spin),
        velocity: spread([0.0; 3], [0.5; 3]),
        offset: spread([0.0; 3], [15.0, 15.0, 0.0]),
        interval: 0.02,
        ..Block::default()
    };
    let layer = |life: f32, size: f32| Layer {
        life,
        size,
        ..Layer::default()
    };
    Fire {
        texture: "particle_fire03",
        layers: vec![layer(-100.0, 20.0), layer(-180.0, 10.0)],
        blocks: vec![
            block(spread(-35.0, 34.0), spread(1.0, 0.5), 0.0),
            block(spread(-35.0, 5.0), spread(3.0, 1.0), 2.0),
        ],
        ramps: vec![flame_ramp(), fire_smoke_ramp(0.996_093_75)],
        blends: vec![Blend::Add, Blend::Over],
        grid: 1,
        count: SMOKE_PARTICLES,
        light: 150.0,
        tinted: false,
    }
}

/// The frame of the steps' small fire's burnt ground at its clock `t` (`0x79F7D0`), of the
/// [`GROUND_GRID`]'s 16: the frames 4 to 0 over the first 0.3 s, as it spreads in, then the frame
/// 0 to 8 s, then the frames 0 to 15 over the ninth second, as it shrinks away; none after.
pub fn ground_frame(t: f32) -> Option<u8> {
    let frame = |x: f32| u8::try_from(crate::fight::whole_part(x * 16.0) % 16).unwrap_or(0);
    if t <= 0.3 {
        Some(frame(0.3 - t))
    } else if t <= SMALL_FIRE_EMITS {
        Some(0)
    } else if t <= SMALL_FIRE_EMITS + 1.0 {
        Some(frame(t - 3.0))
    } else {
        None
    }
}

/// The smokes of the smoke class's variants 1 to 3, the chimneys' `FX_HS_SMOKE_S`, `_M` and `_L`,
/// a grey of 224 at the alpha of the life (the flags `0x146`), and 8 and 9, the lavas'
/// `FX_LV_SMOKE_M` and `_L`, ramp B starting opaque (`0x10C`): one layer of `PARTICLE_SMOKE03`'s
/// 2 × 2 frames, a frame at random, turned and spinning, laid over by its alpha, no light.
fn smoke(variant: u8) -> Option<Fire> {
    let grey = 224.0 / 255.0;
    let fading = || Ramp::new([grey, grey, grey, 1.0], &[([grey, grey, grey, 0.0], 256)]);
    let lava = |alpha: f32| {
        let grey = |g: f32, a: f32| [g / 255.0, g / 255.0, g / 255.0, a];
        Ramp::new(
            grey(60.0, 1.0),
            &[(grey(33.0, alpha), 110), (grey(0.0, 0.0), 146)],
        )
    };
    // The life and the growth a second, the buoyancy, the size, the place's spread, the velocity
    // and its spread, the spin's spread, the interval and the ramp.
    let (life, growth, buoyancy, size, place, velocity, spin, interval, ramp) = match variant {
        1 => (
            -300.0,
            10.0,
            spread(-50.0, 0.0),
            spread(6.0, 2.0),
            [2.0; 3],
            spread([0.0, 0.0, 1.0], [0.3; 3]),
            3.0,
            0.04,
            fading(),
        ),
        2 => (
            -120.0,
            10.0,
            spread(1.5, 0.0),
            spread(5.0, 2.0),
            [2.0; 3],
            spread([0.0, 0.0, 23.0], [2.0; 3]),
            3.0,
            0.1,
            fading(),
        ),
        3 => (
            -75.0,
            8.0,
            spread(2.0, 0.0),
            spread(5.0, 2.0),
            [2.0; 3],
            spread([0.0, 0.0, 30.0], [2.0; 3]),
            3.0,
            0.1,
            fading(),
        ),
        8 => (
            -200.0,
            12.0,
            spread(-35.0, 5.0),
            spread(1.0, 0.75),
            [5.0, 0.0, 0.0],
            spread([0.0; 3], [0.4; 3]),
            1.0,
            0.03,
            lava(0.77),
        ),
        9 => (
            -150.0,
            14.0,
            spread(-35.0, 5.0),
            spread(1.5, 1.0),
            [20.0, 0.0, 0.0],
            spread([0.0; 3], [0.5; 3]),
            2.0,
            0.03,
            lava(1.0),
        ),
        7 => return Some(geyser()),
        13 => return Some(magic_fire()),
        _ => return None,
    };
    Some(Fire {
        texture: "particle_smoke03",
        layers: vec![Layer {
            life,
            size: growth,
            ..Layer::default()
        }],
        blocks: vec![Block {
            buoyancy,
            size,
            spin: spread(0.0, spin),
            velocity,
            offset: spread([0.0; 3], place),
            interval,
            frames: 4,
            ..Block::default()
        }],
        ramps: vec![ramp],
        blends: vec![Blend::Over],
        grid: 2,
        count: SMOKE_PARTICLES,
        light: 0.0,
        // The chimneys' flags `0x146`, the lavas' `0x10C`.
        tinted: variant <= 3,
    })
}

/// The magic fire of the variant 13 (`FX_FIRE_MAGIC`, 904; `0x76E980`, `0x76E2D0`): three layers
/// of `PARTICLE_MULTI02`'s 2 × 2 frames, each particle the frame of its layer, turned at random
/// and added whole (the flags `0x11D`), a light of 230; the system emits every 0.0265 s, each
/// particle of a block by its share of the three blocks' rates (`0x7687F0`):
/// - a flame rising at 35 ± 5, of 5, spinning ± 1.5, drifting ± 4 across, every 0.0265 s, living
///   1.4 s and growing by 11 a second, from white to pink, magenta and teal;
/// - a spark rising at 70, of 5, drifting ± 4 across, every 0.06545 s, living 1 s and growing by
///   1, from orange to blue;
/// - a glow of 8 ± 0.5, drifting ± 2, every 0.02222 s, living a third of a second, from pink to
///   purple.
///
/// The sparks' and the glows' blocks lie 20 up, which the steady emission does not read: all are
/// born at the system's place, the glows on the brazier's bowl ([`System::emit`]). The ramps'
/// values are out of 256.
fn magic_fire() -> Fire {
    let c = |r: f32, g: f32, b: f32, a: f32| [r / 256.0, g / 256.0, b / 256.0, a / 256.0];
    let clear = [0.0; 4];
    let layer = |life: f32, size: f32| Layer {
        life,
        size,
        ..Layer::default()
    };
    let block = |buoyancy: Spread<f32>, size: Spread<f32>, drift: [f32; 3], up: f32| Block {
        buoyancy,
        size,
        angle: spread(std::f32::consts::PI, std::f32::consts::PI),
        velocity: spread([0.0; 3], drift),
        offset: spread([0.0, 0.0, up], [0.0; 3]),
        frames: FRAME_OF_LAYER,
        ..Block::default()
    };
    let interval = |block: Block, interval: f32| Block { interval, ..block };
    Fire {
        texture: "particle_multi02",
        layers: vec![layer(-180.0, 11.0), layer(-256.0, 1.0), layer(-768.0, 0.0)],
        blocks: vec![
            Block {
                spin: spread(0.0, 1.5),
                ..interval(
                    block(spread(-35.0, 5.0), spread(5.0, 0.0), [4.0, 4.0, 0.0], 0.0),
                    0.0265,
                )
            },
            interval(
                block(spread(-70.0, 0.0), spread(5.0, 0.0), [4.0, 4.0, 0.0], 20.0),
                0.06545,
            ),
            interval(
                block(spread(0.0, 0.0), spread(8.0, 0.5), [2.0; 3], 20.0),
                0.02222,
            ),
        ],
        ramps: vec![
            Ramp::new(
                c(255.0, 255.0, 255.0, 0.0),
                &[
                    (c(255.0, 205.0, 248.0, 248.0), 20),
                    (c(254.0, 12.0, 223.0, 254.0), 82),
                    (c(39.0, 137.0, 162.0, 158.0), 65),
                    (clear, 45),
                    (clear, 44),
                ],
            ),
            Ramp::new(
                c(255.0, 184.0, 97.0, 255.0),
                &[(c(90.0, 52.0, 253.0, 255.0), 166), (clear, 44), (clear, 46)],
            ),
            Ramp::new(
                c(247.0, 167.0, 245.0, 247.0),
                &[
                    (c(172.0, 9.0, 142.0, 173.0), 110),
                    (clear, 105),
                    (clear, 41),
                ],
            ),
        ],
        blends: vec![Blend::Whole; 3],
        grid: 2,
        count: SMOKE_PARTICLES,
        light: 230.0,
        tinted: false,
    }
}

/// The geyser's jet at rest (the variant 7, `FX_GEYSIR`, the flags `0x46`): one layer of
/// `PARTICLE_WATER01` whole, a blue-white of (208, 212, 228) at the alpha of the life, laid over
/// by its alpha; its drops falling by 100 a second's buoyancy, of 5 ± 1, ± 1 about the system,
/// rising at 40 ± 5 every 0.1 s and living 0.73 s (`0x76E980`, `0x76E2D0`).
fn geyser() -> Fire {
    let [r, g, b] = [208.0 / 255.0, 212.0 / 255.0, 228.0 / 255.0];
    Fire {
        texture: "particle_water01",
        layers: vec![Layer {
            life: -350.0,
            size: 15.0,
            ..Layer::default()
        }],
        blocks: vec![Block {
            buoyancy: spread(100.0, 0.0),
            size: spread(5.0, 1.0),
            velocity: spread([0.0, 0.0, 40.0], [0.0, 0.0, 5.0]),
            offset: spread([0.0; 3], [1.0; 3]),
            interval: 0.1,
            ..Block::default()
        }],
        ramps: vec![Ramp::new([r, g, b, 1.0], &[([r, g, b, 0.0], 256)])],
        blends: vec![Blend::Over],
        grid: 1,
        count: SMOKE_PARTICLES,
        light: 0.0,
        tinted: true,
    }
}

/// A geyser's clock (`+0x2E38`): its jet bursts for a second as the clock runs from 1 to 0, then
/// rests 30 to 100 s (`0x76E2D0`'s case of the variant 7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geyser {
    pub clock: f32,
}

/// What a geyser's frame asks: its jet at rest, its jet bursting, and whether its burst's sound
/// is to start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Jet {
    Rests,
    Bursts { sounds: bool },
}

/// A share of 0 to 1 by `dice`, as the client's `rand()` / 32767.
fn share(dice: &mut impl Dice) -> f32 {
    f32::from(u16::try_from(dice.below(0x8000)).unwrap_or(0)) / 32_767.0
}

impl Geyser {
    /// A geyser as it starts, its first burst 3 to 20 s on (**estimate** for the roll's form).
    pub fn new(dice: &mut impl Dice) -> Self {
        Self {
            clock: 3.0 + 17.0 * share(dice),
        }
    }

    /// One frame of `dt` seconds of the geyser of the jet `fire`, `latch` the sound's latch that
    /// every geyser shares (`0x17EEBC8`): the clock run below 0 rests it again 30 to 100 s on and
    /// opens the latch; below 1 its jet bursts upward at 200 and lives longer, its drops every
    /// 0.035 s, its sound starting where the latch is open, which it then closes; else its jet
    /// rests as [`geyser`] makes it.
    pub fn update(
        &mut self,
        dt: f32,
        fire: &mut Fire,
        latch: &mut bool,
        dice: &mut impl Dice,
    ) -> Jet {
        self.clock -= dt;
        let (Some(layer), Some(block)) = (fire.layers.first_mut(), fire.blocks.first_mut()) else {
            return Jet::Rests;
        };
        if self.clock < 0.0 {
            *latch = false;
            self.clock = 30.0 + 70.0 * share(dice);
            return Jet::Rests;
        }
        block.velocity.value = [0.0, 0.0, 40.0];
        if self.clock < 1.0 {
            block.velocity.value[2] = 200.0;
            layer.life = -75.0;
            block.interval = 0.035;
            let sounds = !*latch;
            *latch = true;
            Jet::Bursts { sounds }
        } else {
            layer.life = -350.0;
            block.interval = 0.1;
            Jet::Rests
        }
    }
}

/// The smoke class's variant of the FX type `kind` that the scripts' `CreateObj` gives it
/// (`0x48907C`): 771, 810 and 811 the chimneys' smokes, 812, 813 and 876 the fire smokes, 879 the
/// geyser, 882 and 881 the lavas' smokes, 886, 885 and 884 the fires, 904 the magic fire
/// (`0x489C3F`).
pub fn fire_variant(kind: u32) -> Option<u8> {
    match kind {
        904 => Some(13),
        879 => Some(7),
        771 => Some(1),
        810 => Some(2),
        811 => Some(3),
        882 => Some(8),
        881 => Some(9),
        812 => Some(4),
        813 => Some(5),
        876 => Some(6),
        886 => Some(10),
        885 => Some(11),
        884 => Some(12),
        _ => None,
    }
}

/// A system that makes its particles once, as it starts, and lives while they do: the classes
/// whose update only ages them (as `cParticleSystem_puzzleSolved_v`'s, `0x76CF20`): its texture,
/// its grid, how it is drawn, its one layer's rates, its ramp, and its particles.
#[derive(Clone, Debug, PartialEq)]
pub struct Burst {
    pub texture: &'static str,
    pub grid: u8,
    pub blend: Blend,
    pub layer: Layer,
    pub ramp: Ramp,
    pub particles: Vec<Particle>,
    /// The height in its frame at which its particles bounce, where they do ([`System::update_over`]).
    pub ground: Option<f32>,
    /// The radius of its light while it lives, 0 for none.
    pub light: f32,
    /// Whether the light at it tints it (the draw's flag `0x40`), so that it darkens by night.
    pub tinted: bool,
    /// Particles it emits on from its start, where it does ([`Stream`]).
    pub stream: Option<Stream>,
}

/// A burst's emission from its start (`0x764760` of an interval above 0): its block, for how long
/// it emits, and how many of its particles may live at once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stream {
    pub block: Block,
    pub until: f32,
    pub count: usize,
}

/// The colour of a creature's burst by its gib set (`0x427160`, `items.pak +0x6F`): 0 red, 1
/// green, 2 black, 3 the bones' brown, 4 a translucent pale blue; another set black.
pub fn burst_colour(set: u8) -> u32 {
    match set {
        0 => 0xFFAA_0000,
        1 => 0xFF00_AA00,
        2 => 0xFF11_1111,
        3 => 0xFF73_5623,
        4 => 0x80AB_BECD,
        _ => 0xFF00_0000,
    }
}

/// The cloud of a creature's burst (809, `FX_CLOUD`, `cParticleSystem_cloud`, its start
/// `0x784220` and update `0x783F70`; #1264), in the colour `colour` (the FX event's `+0x38`, the
/// set's): for 1 s, a particle every 0.02 s (the `DETAILLEVEL` 2's; 1.5 and 2.2 times that below),
/// up to 100, within (10, 10, 25) of 50 over the place, of a velocity of ± 2 along each axis, a
/// buoyancy of 40 (they sink), a size of 10 ± 4 that grows by 20 a second and an angle of π ± π,
/// living 1.275 s (−200); `PARTICLE_CLOUD01` laid over the frame by its alpha and turned, of the
/// colour at the alpha of its life, tinted by the light at it (the flags `0x46`).
pub fn burst_cloud(colour: u32) -> Burst {
    let [_, r, g, b] = colour.to_be_bytes().map(|c| f32::from(c) / 255.0);
    Burst {
        texture: "PARTICLE_CLOUD01",
        grid: 1,
        blend: Blend::Over,
        layer: Layer {
            life: -200.0,
            size: 20.0,
            ..Layer::default()
        },
        ramp: Ramp::new([r, g, b, 1.0], &[([r, g, b, 0.0], 256)]),
        particles: Vec::new(),
        ground: None,
        light: 0.0,
        tinted: true,
        stream: Some(Stream {
            block: Block {
                buoyancy: spread(40.0, 0.0),
                size: spread(10.0, 4.0),
                angle: spread(std::f32::consts::PI, std::f32::consts::PI),
                velocity: spread([0.0; 3], [2.0, 2.0, 2.0]),
                offset: spread([0.0, 0.0, 50.0], [10.0, 10.0, 25.0]),
                interval: 0.02,
                ..Block::default()
            },
            until: 1.0,
            count: 100,
        }),
    }
}

/// The ring of an undead that rises again (781, `TYPE_FX_PUZZLE_SOLVED_V`, its variant 3,
/// `0x76D260` at `0x76D5A3`; #1261): 75 particles on a ring of 17.5 about the system's place,
/// each rising at 70, the even ones falling back by a buoyancy of 100, of a size of 10 that falls
/// by 8 a second, living 1.275 s (−200), from magenta through blue to a pale blue and to black;
/// `PARTICLE_GLOW03` added to the frame (the flags `0x19`, `0x76CF60`).
pub fn rise_ring() -> Burst {
    let particles = (0..75_u8)
        .map(|k| {
            let angle = f32::from(k) * std::f32::consts::TAU / 75.0;
            Particle {
                place: [angle.cos() * 17.5, angle.sin() * 17.5, 0.0],
                velocity: [0.0, 0.0, 70.0],
                buoyancy: if k % 2 == 0 { 100.0 } else { 0.0 },
                size: 10.0,
                life: 255.0,
                angle: 0.0,
                spin: 0.0,
                layer: 0,
                frame: 0,
            }
        })
        .collect();
    Burst {
        texture: "PARTICLE_GLOW03",
        grid: 1,
        blend: Blend::Add,
        layer: Layer {
            life: -200.0,
            size: -8.0,
            ..Layer::default()
        },
        ramp: Ramp::new(
            [0.898, 0.0195, 0.996, 1.0],
            &[
                ([0.0117, 0.422, 0.996, 1.0], 126),
                ([0.0664, 0.621, 0.9375, 0.949], 45),
                ([0.0, 0.0, 0.0, 0.0], 85),
            ],
        ),
        particles,
        ground: None,
        light: 0.0,
        tinted: false,
        stream: None,
    }
}

/// The dust of a corpse that vanishes (781, its variant 4, `0x76D260` at `0x76D704`; #1261):
/// 150 particles at once (`0x767FB0`) at the corpse's points `points` in the system's frame, each
/// at one of them on `dice`, of a buoyancy of −20 ± 8 that falls by 10 a second (they rise ever
/// faster), a size of 10 that grows by 8 a second, an angle of π ± π turning by 0.5 ± 0.2 and 1 a
/// second, a velocity of ± 3 along each axis and a frame of the 2 × 2 grid, living 2.55 s (−100),
/// a pale bone colour from an alpha of 0.5 to none; `PARTICLE_SMOKE03` laid over the frame by its
/// alpha (the flags `0x10C`, `0x76CF60`). The original takes the model's skinned vertices, the
/// game the points it is given; the wind that the variant takes from the renderer is left out, as
/// the smoke class's is (none without points).
pub fn dust(points: &[[f32; 3]], dice: &mut impl Dice) -> Burst {
    let block = Block {
        buoyancy: spread(-20.0, 8.0),
        size: spread(10.0, 0.0),
        angle: spread(std::f32::consts::PI, std::f32::consts::PI),
        spin: spread(0.5, 0.2),
        velocity: spread([0.0; 3], [3.0; 3]),
        offset: Spread::default(),
        interval: 0.0,
        frames: 4,
    };
    let count = u32::try_from(points.len()).unwrap_or(u32::MAX);
    let particles = if points.is_empty() {
        Vec::new()
    } else {
        (0..150)
            .map(|_| {
                let at = points[usize::try_from(dice.below(count)).unwrap_or(0)];
                Particle {
                    place: at,
                    velocity: spread_3(block.velocity, dice),
                    buoyancy: spread_f32(block.buoyancy, dice),
                    size: spread_f32(block.size, dice),
                    life: 255.0,
                    angle: spread_f32(block.angle, dice),
                    spin: spread_f32(block.spin, dice),
                    layer: 0,
                    frame: u8::try_from(dice.below(u32::from(block.frames))).unwrap_or(0),
                }
            })
            .collect()
    };
    let bone = [0.898, 0.836, 0.664];
    Burst {
        texture: "PARTICLE_SMOKE03",
        grid: 2,
        blend: Blend::Over,
        layer: Layer {
            buoyancy: -10.0,
            life: -100.0,
            size: 8.0,
            angle: 1.0,
            ..Layer::default()
        },
        ramp: Ramp::new(
            [bone[0], bone[1], bone[2], 0.5],
            &[([bone[0], bone[1], bone[2], 0.0], 256)],
        ),
        particles,
        ground: None,
        light: 0.0,
        tinted: false,
        stream: None,
    }
}

/// The particles of `cParticleSystem_generic` (`0x796150`, 856, `FX_GENERIC`).
pub const GENERIC_PARTICLES: usize = 500;
/// The colour of an undead goblin's sparks at its last death (`0x541B95`, the event's `+0x38`).
pub const BURN_SPARKS: u32 = 0xFFFF_4040;

/// The variant of the 856 that a creature of the type `creature` makes at its last death
/// (`0x53ECA0`'s cases 3 to 5): 0, the red sparks, for the undead goblins 142 and 143; 7, the
/// flames, for the fire ghoul 73; 8, the pale flames, for the magic skeletons 48, 80 and 81; none
/// for another.
pub fn burn_variant(creature: u32) -> Option<u8> {
    match creature {
        142 | 143 => Some(0),
        73 => Some(7),
        48 | 80 | 81 => Some(8),
        _ => None,
    }
}

/// The sparks of the generic system's variant 0 in the colour `colour` (`0x796E86`; #1262): up to
/// [`GENERIC_PARTICLES`] at the points `points` of a body in the system's frame, taken in turn
/// (the original's every `⌊V/500⌋`th vertex of its model, the game's points), each of a buoyancy of
/// 150 ± 30, a size of 5 ± 2 that falls by 2 a second, a velocity of ± (3, 3, 0), never ageing
/// otherwise, bouncing at the ground of the system's frame (0); `PARTICLE_SPARK06` added in the
/// colour, a light of 100 while they live.
pub fn generic_sparks(points: &[[f32; 3]], colour: u32, dice: &mut impl Dice) -> Burst {
    let [a, r, g, b] = colour.to_be_bytes().map(|c| f32::from(c) / 255.0);
    let block = Block {
        buoyancy: spread(150.0, 30.0),
        size: spread(5.0, 2.0),
        velocity: spread([0.0; 3], [3.0, 3.0, 0.0]),
        ..Block::default()
    };
    let particles = if points.is_empty() {
        Vec::new()
    } else {
        (0..GENERIC_PARTICLES)
            .map(|k| Particle {
                place: points[k % points.len()],
                velocity: spread_3(block.velocity, dice),
                buoyancy: spread_f32(block.buoyancy, dice),
                size: spread_f32(block.size, dice),
                life: 255.0,
                angle: 0.0,
                spin: 0.0,
                layer: 0,
                frame: 0,
            })
            .collect()
    };
    Burst {
        texture: "PARTICLE_SPARK06",
        grid: 1,
        blend: Blend::Add,
        layer: Layer {
            size: -2.0,
            ..Layer::default()
        },
        ramp: Ramp::new([r, g, b, a], &[([r, g, b, a], 256)]),
        particles,
        ground: Some(0.0),
        light: 100.0,
        tinted: false,
        stream: None,
    }
}

/// The variant of the generic system that a statue's power-up makes on the hero (`0x4A95B4`,
/// #1846), and its colour, a gold.
pub const STATUE_SPARKS: u8 = 2;
pub const STATUE_COLOUR: u32 = 0xFFD0_A527;

/// The sparks of the generic system's variant 2 in the colour `colour` (`0x796E10`'s case 2; #1846): up
/// to [`GENERIC_PARTICLES`] at the points `points` of a body in the system's frame, taken in turn,
/// still, of a size of 7 ± 3 that falls by 5 a second, so that they are gone within two seconds,
/// with no buoyancy, no velocity and no light; `PARTICLE_SPARK06` added in the colour, as the
/// variant 0's (`+0x2578`, which `0x796150` loads).
pub fn statue_sparks(points: &[[f32; 3]], colour: u32, dice: &mut impl Dice) -> Burst {
    let mut burst = generic_sparks(points, colour, dice);
    let size = spread(7.0, 3.0);
    for particle in &mut burst.particles {
        particle.velocity = [0.0; 3];
        particle.buoyancy = 0.0;
        particle.size = spread_f32(size, dice);
    }
    burst.layer.size = -5.0;
    burst.light = 0.0;
    burst
}

/// The variant of the generic system that a script's `Partikel` makes most (`0x796E10`, #1828):
/// a ring of sparks rising about its place.
pub const RISING_RING: u8 = 1;
/// The variant of a script's object or creature appearing in tight sparks, with its sound
/// (`SOUND_FX_SPAWN`).
pub const SPAWN_RING: u8 = 4;
/// The sound that the generic system's variant 4 plays as it starts (`0x693510(2050, …)`).
pub const SPAWN_SOUND: u32 = 2050;

/// The sparks of the generic system's variants 1 and 4 in the colour `colour` (`0x796E10`,
/// `0x7962F0`; #1828), none for another variant: at its first update a ring about the system's
/// place, of 150 particles of radius 25 and a size of 20 rising at 2 (variant 1), or of 500 of
/// radius 10 and a size of 7 rising at 1 (variant 4), each at an angle θ round the ring, of a
/// buoyancy of −(10 + 80·U) (they rise ever faster) and of a random angle; all living 2.45 s
/// (−104), shrinking by 8 a second and turning by 3 radians a second, bouncing at the ground of
/// the system's frame (0); `PARTICLE_SPARK02` added in the colour (the flags `0x05`), a light of
/// 150, or 100, while they live. The rolls' order within a particle is an **estimate**.
pub fn generic_ring(variant: u8, colour: u32, dice: &mut impl Dice) -> Option<Burst> {
    let (count, radius, rise, size, light) = match variant {
        RISING_RING => (150, 25.0, 2.0, 20.0, 150.0),
        SPAWN_RING => (GENERIC_PARTICLES, 10.0, 1.0, 7.0, 100.0),
        _ => return None,
    };
    let [a, r, g, b] = colour.to_be_bytes().map(|c| f32::from(c) / 255.0);
    let particles = (0..count)
        .map(|_| {
            let theta = share(dice) * std::f32::consts::TAU;
            let buoyancy = -(10.0 + 80.0 * share(dice));
            let angle = share(dice) * std::f32::consts::TAU;
            Particle {
                place: [theta.cos() * radius, theta.sin() * radius, 0.0],
                velocity: [0.0, 0.0, rise],
                buoyancy,
                size,
                life: 255.0,
                angle,
                spin: 0.0,
                layer: 0,
                frame: 0,
            }
        })
        .collect();
    Some(Burst {
        texture: "PARTICLE_SPARK02",
        grid: 1,
        blend: Blend::Add,
        layer: Layer {
            life: -104.0,
            size: -8.0,
            angle: 3.0,
            ..Layer::default()
        },
        ramp: Ramp::new([r, g, b, a], &[([r, g, b, a], 256)]),
        particles,
        ground: Some(0.0),
        light,
        tinted: false,
        stream: None,
    })
}

/// The variant of the generic system that puffs a smoke (`0x796E10`'s case 10; #1846), quests
/// 10251's and 10252's.
pub const SMOKE_PUFFS: u8 = 10;
/// The particles of the variant 10's burst (`+0x5C` of its block).
const PUFFS: u8 = 25;

/// The smoke of the generic system's variant 10 in the colour `colour` (`0x796E10`'s case 10,
/// `0x7962F0`; #1846): at its clock's 0 a burst of 25 particles (`0x764760` of the interval 0)
/// within ± (15, 15, 4) of the system's place, each of a velocity of (0, 0, 5) ± (15, 15, 0), a
/// buoyancy of −60 ± 10 (they rise ever faster), a size of 8 ± 1 that grows by 10 a second and
/// an angle of π ± π, living 1.275 s (−200); `PARTICLE_SMOKE04` laid over by its alpha and turned,
/// in the colour (its bytes over 256) from an alpha of 0.996 through 0.39 over 60 entries to 0
/// over 196, tinted by the light at it (the flags `0x4C`), with no light of its own.
pub fn generic_smoke(colour: u32, dice: &mut impl Dice) -> Burst {
    let [_, r, g, b] = colour.to_be_bytes().map(|c| f32::from(c) / 256.0);
    let block = Block {
        buoyancy: spread(-60.0, 10.0),
        size: spread(8.0, 1.0),
        angle: spread(std::f32::consts::PI, std::f32::consts::PI),
        velocity: spread([0.0, 0.0, 5.0], [15.0, 15.0, 0.0]),
        offset: spread([0.0; 3], [15.0, 15.0, 4.0]),
        ..Block::default()
    };
    Burst {
        texture: "PARTICLE_SMOKE04",
        grid: 1,
        blend: Blend::Over,
        layer: Layer {
            life: -200.0,
            size: 10.0,
            ..Layer::default()
        },
        ramp: Ramp::new(
            [r, g, b, 0.996],
            &[([r, g, b, 0.390_625], 60), ([r, g, b, 0.0], 196)],
        ),
        particles: (0..PUFFS).map(|_| born(&block, 0, dice)).collect(),
        ground: None,
        light: 0.0,
        tinted: true,
        stream: None,
    }
}

/// The variant of the generic system that marks a place with a gold arrow (`0x796E10`'s case 3;
/// #1846): the Gladiator's quest 30's two teleports.
pub const GOLD_ARROW: u8 = 3;
/// The arrow's model and texture and the helix's texture (`0x796150`).
pub const ARROW_MODEL: &str = "ARROW2.GRN";
pub const ARROW_TEXTURE: &str = "ARROW2_GOLD";
pub const HELIX_TEXTURE: &str = "FX_HELIX01";
/// The helix's segments, its radius, its half width, and its rise and turn a segment (`0x762C30`
/// of the parameters at `+0x252C`).
const HELIX_SEGMENTS: u8 = 50;
const HELIX_RADIUS: f32 = 17.0;
const HELIX_HALF: f32 = 2.0;
const HELIX_RISE: f32 = 0.25;
const HELIX_TURN: f32 = 0.2;
/// The helix's height over the system's place (`+0x254C`), and the arrow's (`+0x2560` less 5).
const HELIX_HEIGHT: f32 = 50.0;
const ARROW_HEIGHT: f32 = 45.0;

/// A ribbon of the helix: each segment's two edges, its texture's `u` running along it and its `v`
/// 0 at the first edge and 1 at the second.
pub type Ribbon = Vec<[[f32; 3]; 2]>;

/// The gold arrow of the generic system's variant 3 (`0x796E10`'s case 3, its update `0x7962F0`,
/// drawn by `0x7967B0`; #1846): [`ARROW_MODEL`] in [`ARROW_TEXTURE`], lit with its specular
/// highlights (the state `0x80`), standing 45 over the system's place and bobbing by 10 at 7.5
/// radians a second, turning about the upright at 2 radians a second; about it a helix of
/// [`HELIX_TEXTURE`] in the colour, added by its alpha (`0x762C30`), which climbs and turns once
/// in 3.33 s, its alpha `sin(πs)` over the cycle's share `s`, each cycle at a new angle; and a
/// light of `100 + 75 sin 5t` (`+0x9C`). It never ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beacon {
    clock: f32,
    spin: f32,
    bob: f32,
    cycle: f32,
    turn: f32,
    alpha: f32,
}

impl Beacon {
    /// A beacon at its start, its helix at an angle at random (`+0x2540`).
    pub fn new(dice: &mut impl Dice) -> Self {
        Self {
            clock: 0.0,
            spin: 0.0,
            bob: 0.0,
            cycle: 0.0,
            turn: share(dice) * std::f32::consts::TAU,
            alpha: 0.0,
        }
    }

    /// A frame of `dt` seconds on: the arrow turns and bobs, and the helix's cycle runs, a new
    /// angle at random as it ends, its alpha at the cycle's share otherwise.
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) {
        self.spin += 2.0 * dt;
        self.bob += 7.5 * dt;
        self.cycle += 0.3 * dt;
        if self.cycle < 1.0 {
            self.alpha = ((self.cycle * std::f32::consts::PI).sin() * 255.0).trunc() / 255.0;
        } else {
            self.cycle = 0.0;
            self.turn = share(dice) * std::f32::consts::TAU;
        }
        self.clock += dt;
    }

    /// The radius of its light, in the pixels of the original's screen.
    pub fn light(&self) -> f32 {
        ((5.0 * self.clock).sin() * 75.0 + 100.0).trunc()
    }

    /// The arrow's turn about the upright, in radians, and its height over the system's place.
    pub fn arrow(&self) -> (f32, f32) {
        (self.spin, ARROW_HEIGHT + self.bob.sin() * 10.0)
    }

    /// The helix's alpha, 0 to 1.
    pub fn alpha(&self) -> f32 {
        self.alpha
    }

    /// The helix's two ribbons in the system's frame: one flat, from the radius less the half
    /// width to the radius plus it, one upright, from the half width above to it below; both
    /// starting at the cycle's height and angle (a hundredth of the cycle a quarter of a unit up
    /// and a fifth of a radian on).
    pub fn helix(&self) -> [Ribbon; 2] {
        let t = self.cycle * 100.0;
        helix_ribbons(
            (t * HELIX_RISE + HELIX_HEIGHT, t * HELIX_TURN + self.turn),
            (HELIX_RISE, HELIX_TURN),
            (HELIX_RADIUS, HELIX_HALF),
            HELIX_SEGMENTS,
        )
    }
}

/// A helix's two ribbons of `segments` points (`0x762C30`): from the height and the angle
/// `(z, angle)`, each point `rise` higher and `turn` on, one flat from `radius − half` to
/// `radius + half`, one upright from `half` above to `half` below, at `(cos, sin)` of the angle.
pub fn helix_ribbons(
    (mut z, mut angle): (f32, f32),
    (rise, turn): (f32, f32),
    (radius, half): (f32, f32),
    segments: u8,
) -> [Ribbon; 2] {
    let mut flat = Vec::with_capacity(usize::from(segments));
    let mut upright = Vec::with_capacity(usize::from(segments));
    for _ in 0..segments {
        let (sin, cos) = angle.sin_cos();
        let at = |radius: f32, height: f32| [cos * radius, sin * radius, height];
        flat.push([at(radius - half, z), at(radius + half, z)]);
        upright.push([at(radius, z + half), at(radius, z - half)]);
        z += rise;
        angle += turn;
    }
    [flat, upright]
}

/// How long the generic system's variants 7 and 8 emit their flames, in seconds (`0x796660`).
pub const DEATH_FLAMES_EMIT: f32 = 1.0;
/// The burning's scorch: its quad's half side in the models' units, how long it holds whole and
/// how fast its alpha falls after, a second (`0x796A44`, `0x7966EF`).
pub const SCORCH_HALF: f32 = 50.0;
pub const SCORCH_HOLDS: f32 = 10.0;
pub const SCORCH_FADES: f32 = 0.5;

/// The flames of the generic system's variant 7 (the fire ghoul's) or 8 (the magic skeletons')
/// (`0x797394`, `0x79749F`; #1262): every 0.003 s for [`DEATH_FLAMES_EMIT`], `PARTICLE_CLOUD01`
/// of a half side of 20, turned at random, shot up at 350 ± 130 (± 15 across) from within 8 of the
/// place, living 0.34 s (−750 a second), added by their alpha: white to orange to a fading red for
/// 7, pale cyan to blue for 8; no light; none for another variant.
pub fn death_flames(variant: u8) -> Option<Fire> {
    let ramp = match variant {
        7 => Ramp::new(
            [1.0, 1.0, 1.0, 1.0],
            &[([1.0, 0.5, 0.2, 1.0], 60), ([1.0, 0.2, 0.0, 0.0], 196)],
        ),
        8 => Ramp::new(
            [0.7, 1.0, 1.0, 1.0],
            &[([0.5, 0.8, 1.0, 1.0], 60), ([0.3, 0.6, 1.0, 0.0], 196)],
        ),
        _ => return None,
    };
    Some(Fire {
        texture: "particle_cloud01",
        layers: vec![Layer {
            life: -750.0,
            ..Layer::default()
        }],
        blocks: vec![Block {
            size: spread(20.0, 0.0),
            angle: spread(std::f32::consts::PI, std::f32::consts::PI),
            velocity: spread([0.0, 0.0, 350.0], [15.0, 15.0, 130.0]),
            offset: spread([0.0; 3], [8.0, 8.0, 0.0]),
            interval: 0.003,
            ..Block::default()
        }],
        ramps: vec![ramp],
        blends: vec![Blend::Add],
        grid: 1,
        count: GENERIC_PARTICLES,
        light: 0.0,
        tinted: false,
    })
}

/// The scorch's alpha at the system's clock `t`: whole to [`SCORCH_HOLDS`], then falling by
/// [`SCORCH_FADES`] a second; none once it is gone, when the system ends.
pub fn scorch_alpha(t: f32) -> Option<f32> {
    let alpha = if t <= SCORCH_HOLDS {
        1.0
    } else {
        1.0 - SCORCH_FADES * (t - SCORCH_HOLDS)
    };
    (alpha > 0.0).then_some(alpha)
}

/// The scorch's texture by the coin of its making (`0x796150`'s end): `FX_STREAKS02` where
/// `rand()` is even, else `FX_STREAKS01`.
pub fn scorch_texture(even: bool) -> &'static str {
    if even { "fx_streaks02" } else { "fx_streaks01" }
}

/// The glitter's numbers (`0x8E808C`), whose sines move its sparks; the mouse events' sparks
/// take the sixteen after them (`0x8E80CC`), which hold the same numbers.
const GLITTER: [f32; 16] = [
    1.3452, 6.6458, 4.6646, 2.7472, 6.4337, 4.5351, 1.7371, 0.9962, 2.5993, 3.9645, 2.3992, 1.7379,
    2.8925, 0.0085, 6.7786, 5.5555,
];

/// The sparks of the glitter at each `DETAILLEVEL`, 0 to 2 (2 in the installation).
pub const GLITTER_SPARKS: [u8; 3] = [3, 5, 8];
/// A spark's coloured square's half side, and its grey square's colour, in the frame's values.
pub const SPARK_HALF: f32 = 4.0;
pub const SPARK_GREY: f32 = 187.0 / 255.0;

/// A spark of the glitter: its offset from the glitter's point (across, along, up), its coloured
/// square's colour, and its grey square's half side, in the models' units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spark {
    pub offset: [f32; 3],
    pub colour: [f32; 3],
    pub half: f32,
}

/// The glitter over an item that lies on the ground, at its time `t` (`sdk_GLOW`, `0x408B70`;
/// docs/re/items.md, "The glitter"): the half of the game's seconds plus its object's phase. Each
/// of its `count` sparks wanders within 10 of the point by the sines of [`GLITTER`], its coloured
/// square turning round the hues in two seconds, its grey square from 1 to 3 across.
pub fn glitter(t: f32, count: u8) -> Vec<Spark> {
    sparks(t, count, false)
}

/// The sparks of a mouse event that neither hides nor blinks, at its time `t` (`0x409D00`, the
/// cursed gold's; docs/re/containers.md, "The mouse events"): as the glitter's, but each rising
/// `25 |sin(3t + a[i + 6])|` above the point, its coloured square at half the glitter's value.
pub fn event_sparks(t: f32, count: u8) -> Vec<Spark> {
    sparks(t, count, true)
}

/// The glitter's sparks, or a mouse event's where `event`.
fn sparks(t: f32, count: u8, event: bool) -> Vec<Spark> {
    let a = |i: i32| GLITTER[usize::try_from(i.rem_euclid(16)).unwrap_or(0)];
    let wave = |i: i32, [p, q, r, s]: [i32; 4]| {
        let first = (t * a(i + p) + a(i + q)).sin();
        (first, first * ((t * a(i + r) + a(i + s)).sin() + 1.0) * 5.0)
    };
    let value = if event { 125.0 } else { 250.0 } / 255.0;
    (1..=i32::from(count))
        .map(|i| {
            let (_, across) = wave(i, [-6, -7, -5, 0]);
            let (_, along) = wave(i, [7, -8, 5, 1]);
            let (rise, up) = wave(i, [4, 6, 2, 3]);
            let up = if event {
                (3.0 * t + a(i + 6)).sin().abs() * 25.0
            } else {
                up
            };
            let hue = (a(i + 4) - t).rem_euclid(1.0);
            Spark {
                offset: [across, along, up],
                colour: hue_colour(hue, value),
                half: rise + 2.0,
            }
        })
        .collect()
}

/// The colour of the hue `hue` (0 to 1) at the value `value` and full saturation, round the wheel
/// from blue through cyan, green, yellow, red and magenta (`0x642420`).
fn hue_colour(hue: f32, value: f32) -> [f32; 3] {
    let at = hue.clamp(0.0, 1.0) * 6.0;
    let sextant = at.floor().min(5.0);
    let share = at - sextant;
    let (rising, falling) = (share * value, (1.0 - share) * value);
    match sextant {
        0.0 => [0.0, rising, value],
        1.0 => [0.0, value, falling],
        2.0 => [rising, value, 0.0],
        3.0 => [value, falling, 0.0],
        4.0 => [value, 0.0, rising],
        _ => [falling, 0.0, value],
    }
}

/// The frame of a grid's flip-book that a particle of the life `life` shows, played once over its
/// life (`0x761B00` with a grid and no flag `0x100`): `⌊(255 − life) · G² / 256⌋` of the `G × G`
/// frames, row by row.
pub fn age_frame(life: f32, grid: u8) -> u8 {
    let frames = f32::from(grid) * f32::from(grid);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a frame below the grid's count, at most 255"
    )]
    let frame = ((255.0 - life.clamp(0.0, 255.0)) * frames / 256.0) as u32;
    u8::try_from(frame.min(u32::from(grid) * u32::from(grid) - 1).min(255)).unwrap_or(0)
}

/// The railgun's bolt (801, `cParticleSystem_railgun`, docs/re/effects.md, "The bolts"): no
/// particles but two crossed quads of `FX_PLASMA_CENTER`, one flat and one upright, of these half
/// sides along the flight and across it (`0x77E7A0`), laid over by their alpha; and, for a hero's
/// bolt, a line of [`RAILGUN_GLOWS`] glows of `PARTICLE_GLOW01` facing the camera, of the half side
/// [`RAILGUN_GLOW_HALF`] and the colour [`RAILGUN_GLOW_COLOUR`], added, the first
/// [`RAILGUN_GLOW_BEHIND`] behind the bolt's centre and each [`RAILGUN_GLOW_STEP`] on (`0x40DA80`
/// from `0x77EBC0`); its light [`RAILGUN_LIGHT`].
pub const RAILGUN_HALF: (f32, f32) = (20.0, 15.0);
pub const RAILGUN_GLOWS: usize = 8;
pub const RAILGUN_GLOW_HALF: f32 = 15.0;
pub const RAILGUN_GLOW_BEHIND: f32 = 20.0;
pub const RAILGUN_GLOW_STEP: f32 = 5.0;
pub const RAILGUN_GLOW_COLOUR: [f32; 4] = [32.0 / 255.0, 32.0 / 255.0, 64.0 / 255.0, 1.0];

/// The bolts' lights, their systems' radii (`+0x9C`) in the pixels of the original's screen, which
/// their objects' draw adds at their places on the ground (`0x5AA26E`, docs/re/effects.md,
/// "Lights"): the railgun's, the BFG's and the fireball's; the Daemon's bolt has none.
pub const RAILGUN_LIGHT: f32 = 100.0;
pub const BFG_LIGHT: f32 = 120.0;
pub const FIRESTRIKE_LIGHT: f32 = 200.0;

/// The BFG's bolt (875, `cParticleSystem_bfg`): three glows of `FX_BFG_1` to `3` of this half
/// side, added, in greys that pulse ([`bfg_greys`]); and a flat quad of `PARTICLE_GLOW03` on the
/// ground under it, of this half side and colour, added (`0x77F290`); its light [`BFG_LIGHT`].
pub const BFG_GLOW_HALF: f32 = 35.0;
pub const BFG_GROUND_HALF: f32 = 60.0;
pub const BFG_GROUND_COLOUR: [f32; 4] = [72.0 / 255.0, 185.0 / 255.0, 1.0, 1.0];
/// The BFG's pulse, in radians a second of its clock (`0x897E5C`).
const BFG_PULSE: f32 = 4.21;

/// The greys of the BFG's three glows at its clock `t` (`0x77F1C0`): with `a = 4.21 t`, the first
/// `⌊127 (sin a + 1)⌋`, the second 255 less that, the third `⌊127 (cos a + 1)⌋`, as shares of 255.
pub fn bfg_greys(t: f32) -> [f32; 3] {
    let a = BFG_PULSE * t;
    let first = (127.0 * (a.sin() + 1.0)).floor();
    let third = (127.0 * (a.cos() + 1.0)).floor();
    [first, 255.0 - first, third].map(|grey| grey / 255.0)
}

/// What a fireball's system does (`cParticleSystem_firestrike`'s state, `+0x20A8`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flames {
    /// It flies, leaving its trail of flames.
    Flying,
    /// It hit or stopped: one flame more at its place, at its next run.
    Burst,
    /// It makes no more; it ends as its last flame dies.
    Out,
}

/// The Mage's fireball (778, `cParticleSystem_firestrike`, `0x76BAE0`; docs/re/effects.md, "The
/// bolts"): a trail of `FIRE01` flames left along its way in the world, one every 0.07 s, each of a
/// half side of 15 growing by 16 a second, turned at random, drifting by up to 4 a second along
/// each axis and living 0.73 s, added and turned, its 4 × 4 frames played once over its life
/// ([`age_frame`]); at a hit or a stop one flame more of the burst's block, then none, the system
/// ending with its last flame; at its time's end none more and no burst. Its light
/// [`FIRESTRIKE_LIGHT`] lasts as long as it does.
#[derive(Clone, Debug, PartialEq)]
pub struct Firestrike {
    pub system: System,
    pub state: Flames,
}

/// The fireball's texture and its grid.
pub const FIRESTRIKE_TEXTURE: &str = "PARTICLE_FIRE01";
pub const FIRESTRIKE_GRID: u8 = 4;
/// Its particles at most (`0x76BAE0`).
const FIRESTRIKE_COUNT: usize = 50;

impl Firestrike {
    pub fn new() -> Self {
        Self {
            system: System::new(FIRESTRIKE_COUNT),
            state: Flames::Flying,
        }
    }

    /// Its one layer (`+0x20C0`, written anew at each run).
    fn layer() -> Layer {
        Layer {
            life: -350.0,
            size: 16.0,
            ..Layer::default()
        }
    }

    /// The flying block (`+0x20E0`) at the detail 2.
    fn flying() -> Block {
        Block {
            size: spread(15.0, 0.0),
            angle: spread(std::f32::consts::PI, std::f32::consts::PI),
            velocity: spread([0.0; 3], [4.0; 3]),
            interval: 0.07,
            ..Block::default()
        }
    }

    /// The burst's block (`+0x2144`).
    fn burst() -> Block {
        Block {
            size: spread(10.0, 0.0),
            angle: spread(std::f32::consts::PI, std::f32::consts::PI),
            velocity: spread([0.0; 3], [20.0; 3]),
            offset: spread([0.0; 3], [15.0; 3]),
            ..Block::default()
        }
    }

    /// A run of `dt` seconds, its place gone from `from` to `to` in the models' frame
    /// (`0x76BBD0`): its flames age; flying, it leaves them along the way at their interval;
    /// burst, one at `to`.
    pub fn update(&mut self, dt: f32, (from, to): ([f32; 3], [f32; 3]), dice: &mut impl Dice) {
        let layers = [Self::layer()];
        self.system.update(dt, &layers);
        match self.state {
            Flames::Flying => {
                self.system.emit_along(
                    &[Self::flying()],
                    &layers,
                    FIRESTRIKE_COUNT,
                    (from, to),
                    dt,
                    dice,
                );
            }
            Flames::Burst => {
                let mut flame = born(&Self::burst(), 0, dice);
                for (axis, at) in flame.place.iter_mut().zip(to) {
                    *axis += at;
                }
                self.system.particles.push(flame);
                self.state = Flames::Out;
            }
            Flames::Out => {}
        }
    }

    /// Its hit or its stop at a cell: one flame more at its next run, then none.
    pub fn hit(&mut self) {
        if self.state == Flames::Flying {
            self.state = Flames::Burst;
        }
    }

    /// Its time's end: no flame more.
    pub fn out(&mut self) {
        if self.state == Flames::Flying {
            self.state = Flames::Out;
        }
    }

    /// Whether it ended: no flame more to make and none alive.
    pub fn done(&self) -> bool {
        self.state == Flames::Out && self.system.particles.is_empty()
    }
}

impl Default for Firestrike {
    fn default() -> Self {
        Self::new()
    }
}

/// The Daemon's bolt of his 1103 (1159, `cParticleSystem_emperor`, `0x7BD9B0`; docs/re/effects.md,
/// "The bolts"): rays of `LIGHTNING01`'s 2 × 2 frames riding with it, one every 0.005 s for its
/// first 1.14 s, each of a half side of 10 ± 7, turned at random about its foot at the bolt's
/// centre and living 0.21 s, added in a ramp from black to (220, 220, 255); and while it makes
/// them a glow of `PARTICLE_GLOW03` of (224, 224, 255) flickering at its centre. At a hit (on its
/// target's neck) or a stop it flares 2 s more, its rays and its glow 1.75 times as large.
#[derive(Clone, Debug, PartialEq)]
pub struct Emperor {
    pub system: System,
    /// Until when of its clock it makes rays, and its flare's scale.
    until: f32,
    pub scale: f32,
}

/// The emperor's rays' texture and grid, its glow's texture, colour and half side.
pub const EMPEROR_TEXTURE: &str = "PARTICLE_LIGHTNING01";
pub const EMPEROR_GRID: u8 = 2;
pub const EMPEROR_GLOW_TEXTURE: &str = "PARTICLE_GLOW03";
pub const EMPEROR_GLOW_COLOUR: [f32; 4] = [224.0 / 255.0, 224.0 / 255.0, 1.0, 1.0];
/// Its particles at most, how long it makes rays, its flare's time and scale.
const EMPEROR_COUNT: usize = 400;
const EMPEROR_EMITS: f32 = 1.14;
const EMPEROR_FLARE: f32 = 2.0;
const EMPEROR_FLARE_SCALE: f32 = 1.75;

impl Emperor {
    pub fn new() -> Self {
        Self {
            system: System::new(EMPEROR_COUNT),
            until: EMPEROR_EMITS,
            scale: 1.0,
        }
    }

    /// Its layer (`+0x20B0`) and its block (`+0x20D0`) at the detail 2, the block's size and its
    /// spread by its scale.
    fn layer() -> Layer {
        Layer {
            life: -1200.0,
            ..Layer::default()
        }
    }

    fn block(&self) -> Block {
        Block {
            size: spread(10.0 * self.scale, 7.0 * self.scale),
            angle: spread(std::f32::consts::PI, std::f32::consts::PI),
            interval: 0.005,
            frames: 4,
            ..Block::default()
        }
    }

    /// Its ramp (`+0x2134`, computed): the entry `i` of `(⌊220 i / 256⌋, ⌊220 i / 256⌋,
    /// ⌊255 i / 256⌋)`, opaque.
    pub fn ramp() -> Ramp {
        Ramp(std::array::from_fn(|i| {
            #[expect(clippy::cast_precision_loss, reason = "an entry below 256")]
            let i = i as f32;
            [
                (220.0 * i / 256.0).floor() / 255.0,
                (220.0 * i / 256.0).floor() / 255.0,
                (255.0 * i / 256.0).floor() / 255.0,
                1.0,
            ]
        }))
    }

    /// A run of `dt` seconds (`0x7BDA50`): its rays age, and it makes them about its centre while
    /// its clock is below their end.
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) {
        let layers = [Self::layer()];
        self.system.update(dt, &layers);
        if self.emits() {
            self.system
                .emit(&[self.block()], &layers, EMPEROR_COUNT, dice);
        }
    }

    /// Whether it makes rays still, and draws its glow.
    pub fn emits(&self) -> bool {
        self.system.clock < self.until
    }

    /// Its flare at a hit or a stop: 2 s more of rays, 1.75 times as large (`+0x74` 2).
    pub fn flare(&mut self) {
        self.until = self.system.clock + EMPEROR_FLARE;
        self.scale = EMPEROR_FLARE_SCALE;
    }

    /// The half side of its glow at a roll `roll` of `[0, 1)`: `(10 + 4 roll)` times its scale.
    pub fn glow_half(&self, roll: f32) -> f32 {
        (10.0 + 4.0 * roll) * self.scale
    }

    /// Whether it ended: no ray more to make and none alive.
    pub fn done(&self) -> bool {
        !self.emits() && self.system.particles.is_empty()
    }
}

impl Default for Emperor {
    fn default() -> Self {
        Self::new()
    }
}

/// A lit torch's flame (787 `FX_TORCHSMOKE`, `cParticleSystem_torchsmoke`, `0x774F00`;
/// docs/re/effects.md, "The torches' flames"), which the torch's `cWeapon3D` makes and ends: a
/// flame of `PARTICLE_FIRE01` every 0.03 s at its bone, of a half side of 7, turned at random,
/// rising at 50 ± 10 within 1.5 of the bone and living 0.32 s, riding with the torch's holder
/// (its flags `0x204`: the system's frame has its origin at the holder's place on the ground, so
/// that its flames trail the bone's swing about the holder alone), added and turned, its 4 × 4
/// frames played once over its life ([`age_frame`]); and a glow of `PARTICLE_GLOW01` at the bone ([`TORCH_GLOW_HALF`],
/// [`TORCH_GLOW_COLOUR`]), added by its alpha. Put out, it makes no more and ends with its last
/// flame.
#[derive(Clone, Debug, PartialEq)]
pub struct Torchsmoke {
    pub system: System,
    pub lit: bool,
}

/// The torch's flames' texture and grid, its glow's texture, half side and colour (the style 3,
/// `SRCALPHA`, `ONE`, `0x775050`), and the loop it plays at the torch while it burns
/// (`TORCHFIRE_LOOP` at the volume 0.4, `0x7753AC`).
pub const TORCH_TEXTURE: &str = "PARTICLE_FIRE01";
pub const TORCH_GRID: u8 = 4;
pub const TORCH_GLOW_TEXTURE: &str = "PARTICLE_GLOW01";
pub const TORCH_GLOW_HALF: f32 = 20.0;
pub const TORCH_GLOW_COLOUR: [f32; 4] = [1.0, 234.0 / 255.0, 59.0 / 255.0, 128.0 / 255.0];
pub const TORCH_LOOP: u32 = 219;
pub const TORCH_LOOP_VOLUME: f32 = 0.4;
/// Its particles at most (`0x774F00`).
const TORCH_COUNT: usize = 200;

impl Torchsmoke {
    pub fn new() -> Self {
        Self {
            system: System::new(TORCH_COUNT),
            lit: true,
        }
    }

    /// Its layer: the life −800 a second, no growth.
    fn layer() -> Layer {
        Layer {
            life: -800.0,
            ..Layer::default()
        }
    }

    /// Its block, at the detail 2.
    fn block() -> Block {
        Block {
            size: spread(7.0, 0.0),
            angle: spread(std::f32::consts::PI, std::f32::consts::PI),
            velocity: spread([0.0, 0.0, 50.0], [10.0; 3]),
            offset: spread([0.0; 3], [1.5; 3]),
            interval: 0.03,
            ..Block::default()
        }
    }

    /// A run of `dt` seconds, its bone gone from `from` to `to` in its frame, the models' about
    /// its holder's place (`0x774FE0`): its flames age and, while it burns, it leaves new ones
    /// along the way.
    pub fn update(&mut self, dt: f32, (from, to): ([f32; 3], [f32; 3]), dice: &mut impl Dice) {
        let layers = [Self::layer()];
        self.system.update(dt, &layers);
        if self.lit {
            self.system
                .emit_along(&[Self::block()], &layers, TORCH_COUNT, (from, to), dt, dice);
        }
    }

    /// Put out (`+0x20A8` 1): no flame more.
    pub fn put_out(&mut self) {
        self.lit = false;
    }

    /// Whether it ended: put out and no flame alive.
    pub fn done(&self) -> bool {
        !self.lit && self.system.particles.is_empty()
    }
}

impl Default for Torchsmoke {
    fn default() -> Self {
        Self::new()
    }
}

/// The particles of the Daemon's breath and of its wall's fires (`0x5A0C50`'s count for the types
/// 1006 and 1007).
const WALLOFFIRE_COUNT: usize = 250;

/// The look of the Daemon's breath of fire 1102, `cParticleSystem_walloffire` (`FX_WALLOFFIRE`,
/// 1006, its start `0x7AE5A0` and its run `0x7AE290`; docs/re/effects.md, "The breath of fire"):
/// a fire of the smoke class's make that it leaves along its way while it flies, a flame of
/// `PARTICLE_FIRE03` every 0.01 s, one in five a smoke, both rising at 35 within 9 of its place
/// along x, the flame of a half side of 5 growing by 12 a second for 2.55 s in the fires' ramp
/// ([`fire`]'s), added, the smoke of 3 growing by 15 for 1.96 s in their smoke's, laid over, both
/// turned at random and untinted; stopped, it makes no more. Its light is 100, and its glow
/// ([`WALLOFFIRE_GLOW_TEXTURE`]) lies at its place while it lives.
pub fn walloffire() -> Fire {
    let pi = std::f32::consts::PI;
    let layer = |life: f32, size: f32| Layer {
        life,
        size,
        ..Layer::default()
    };
    let block = |buoyancy: Spread<f32>, size: Spread<f32>, spin: f32| Block {
        buoyancy,
        size,
        angle: spread(pi, pi),
        spin: spread(0.0, spin),
        velocity: spread([0.0; 3], [0.5; 3]),
        offset: spread([0.0; 3], [9.0, 0.0, 0.0]),
        interval: 0.01,
        frames: 0,
    };
    Fire {
        texture: "particle_fire03",
        layers: vec![layer(-100.0, 12.0), layer(-130.0, 15.0)],
        blocks: vec![
            block(spread(-35.0, 34.0), spread(5.0, 0.5), 0.0),
            block(spread(-35.0, 5.0), spread(3.0, 1.0), 2.0),
        ],
        ramps: vec![flame_ramp(), fire_smoke_ramp(1.0)],
        blends: vec![Blend::Add, Blend::Over],
        grid: 1,
        count: WALLOFFIRE_COUNT,
        light: 100.0,
        tinted: false,
    }
}

/// The look of the Daemon's ring of poison 1101, `cParticleSystem_ringofpoison`
/// (`FX_RINGOFPOISON`, 1008, its start `0x7AF840`, its run `0x7AF550` and its draw `0x7AF650`;
/// docs/re/effects.md, "The ring of poison"), which lies where it was made for 3 s: its burst of
/// sparks ([`ring_sparks`]) and a screen's quake of 30 at its start, its ring spreading over the
/// ground ([`poison_ring`]) and its two twirls turning on it ([`poison_twirls`]), its light 200.
pub const POISON_RING_LIVES: f32 = 3.0;
pub const POISON_RING_LIGHT: f32 = 200.0;
pub const POISON_RING_QUAKE: u32 = 30;
/// The ring's texture, its segments, the twirls' texture and colour (`0xFF41FF42`).
pub const POISON_RING_TEXTURE: &str = "fx_ringofpoison01";
pub const POISON_RING_SEGMENTS: u16 = 50;
pub const POISON_TWIRL_TEXTURE: &str = "Fx_twirl_rays";
pub const POISON_TWIRL_COLOUR: [f32; 4] = [65.0 / 255.0, 1.0, 66.0 / 255.0, 1.0];

/// The ring of poison's ring at its clock `t` (`0x763530`): flat on the ground, 50 segments
/// between the radii `150 t` and 40 more, each segment the texture's width along it and its height
/// from the inner radius out, added whole, of the colour (34, 128, 34) times a share that holds
/// 1 to 0.45 s and falls to 0 at 0.75 s; none from then.
pub fn poison_ring(t: f32) -> Option<(f32, f32, [f32; 4])> {
    ring_band(t, 150.0, [34.0, 128.0, 34.0])
}

/// The Daemon's blast's ring at its clock `t` (1168, `FX_KAMIKAZE`, `cParticleSystem_kamikaze`,
/// its draw `0x7C1890`; #1092): the ring of poison's band but spreading at its reach `reach` a
/// second (`min(150 + 2·L, 400)`, the event's `+0x44`), of (100, 100, 128).
pub fn blast_ring(t: f32, reach: f32) -> Option<(f32, f32, [f32; 4])> {
    ring_band(t, reach, [100.0, 100.0, 128.0])
}

/// A ring's band at its clock `t` spreading at `rate` a second, of the bytes `colour` times its
/// share (`0x763530`).
fn ring_band(t: f32, rate: f32, colour: [f32; 3]) -> Option<(f32, f32, [f32; 4])> {
    if !(0.0..0.75).contains(&t) {
        return None;
    }
    let share = if t < 0.45 {
        1.0
    } else {
        1.0 - (t - 0.45) * (10.0 / 3.0)
    };
    // The bytes rounded toward 0, as `__ftol`.
    let byte = |value: f32| (value * share).trunc() / 255.0;
    let inner = rate * t;
    let [r, g, b] = colour;
    Some((inner, inner + 40.0, [byte(r), byte(g), byte(b), 1.0]))
}

/// The blast's twirls' colour (`0xFFE0E0FF`); its twirls are the ring of poison's otherwise
/// ([`poison_twirls`]).
pub const BLAST_TWIRL_COLOUR: [f32; 4] = [224.0 / 255.0, 224.0 / 255.0, 1.0, 1.0];

/// The ring of poison's twirls at its clock `t` (`0x7631E0` twice): flat quads at its place of
/// the half sides `100 (1 − (t/3)²)` and `90 (1 − (t/3)²)`, turned by `−1.7 t` and `−1.3 t`
/// radians, of [`POISON_TWIRL_TEXTURE`] in [`POISON_TWIRL_COLOUR`] added whole; none from 3 s.
pub fn poison_twirls(t: f32) -> Option<[(f32, f32); 2]> {
    let share = t / POISON_RING_LIVES;
    if !(0.0..1.0).contains(&share) {
        return None;
    }
    let shrink = 1.0 - share * share;
    Some([(100.0 * shrink, -1.7 * t), (90.0 * shrink, -1.3 * t)])
}

/// The ring of poison's sparks at its start (`0x7AF840`, `0x764760` of the interval 0): its 150
/// particles of `PARTICLE_SPARK06` at once within 7 of its place along x and y, shot up at
/// 150 ± 40 (± 40 across) and falling back at a buoyancy of 180 ± 40, of a half side of 7 ± 2
/// shrinking by 6 a second, living 1.5 s, white for their first 100 entries and fading to black
/// over the rest, added whole (the flags `0x19`), not turned.
pub fn ring_sparks(dice: &mut impl Dice) -> Burst {
    sparks_burst([254.0 / 255.0; 3], dice)
}

/// The blast's sparks at its start (`0x7C1AB0`, `0x7C1790`): the ring of poison's, turning pale
/// blue (0.820, 0.820, 0.996) over their first 100 entries rather than staying white.
pub fn blast_sparks(dice: &mut impl Dice) -> Burst {
    sparks_burst([0.820, 0.820, 0.996], dice)
}

/// A ring's burst of sparks whose colour goes from white to `middle` over 100 entries and to
/// black over the rest.
fn sparks_burst(middle: [f32; 3], dice: &mut impl Dice) -> Burst {
    let block = Block {
        buoyancy: spread(180.0, 40.0),
        size: spread(7.0, 2.0),
        velocity: spread([0.0, 0.0, 150.0], [40.0; 3]),
        offset: spread([0.0; 3], [7.0, 7.0, 0.0]),
        ..Block::default()
    };
    let particles = (0..RING_SPARKS)
        .map(|_| Particle {
            place: spread_3(block.offset, dice),
            velocity: spread_3(block.velocity, dice),
            buoyancy: spread_f32(block.buoyancy, dice),
            size: spread_f32(block.size, dice),
            life: 255.0,
            angle: 0.0,
            spin: 0.0,
            layer: 0,
            frame: 0,
        })
        .collect();
    let white = [254.0 / 255.0, 254.0 / 255.0, 254.0 / 255.0, 1.0];
    let [r, g, b] = middle;
    Burst {
        texture: "PARTICLE_SPARK06",
        grid: 1,
        blend: Blend::Add,
        layer: Layer {
            life: -170.0,
            size: -6.0,
            ..Layer::default()
        },
        // Its alpha is 0 in the original's, which its source `ONE` does not take.
        ramp: Ramp::new(white, &[([r, g, b, 1.0], 100), ([0.0, 0.0, 0.0, 1.0], 156)]),
        particles,
        ground: None,
        light: 0.0,
        tinted: false,
        stream: None,
    }
}

/// The ring of poison's particles (`0x5A0C50`'s count for the type 1008), all of its burst.
const RING_SPARKS: usize = 150;

/// The look of the Seraphim's holy light, `cParticleSystem_holylight` (`SERA_HEILIGES_LICHT`,
/// 804, its start `0x7816A0`, its run `0x781030` and its draw `0x781100`; docs/re/effects.md,
/// "The holy light"), standing at its point for its seconds: a column of light 1,000 high
/// ([`HOLY_COLUMN`]), its ground symbol turning ([`HOLY_SYMBOL_HALF`]), a glow on the ground
/// ([`HOLY_GLOW_HALF`]) and sparks rising ([`HolyLight`]), its column and sparks narrowing as it
/// fades, its light 200.
pub const HOLY_LIGHT_LIGHT: f32 = 200.0;
/// The column (`+0x214C`, `+0x248C`): a cylinder of 12 segments of the radius 7 and the height
/// 1,000, white at its foot and (160, 160, 255) at its top, and two bands across it from 7 out
/// to 17.5 along x, fading to nothing at their outer edges; its alphas, the cylinder's foot's
/// and the bands' inner foot's, its pulse times 112 and 128, the tops' 112 and 128.
pub const HOLY_COLUMN: Column = Column {
    radius: 7.0,
    height: 1_000.0,
    segments: 12,
    band: 17.5,
    foot: [1.0, 1.0, 1.0],
    top: [160.0 / 255.0, 160.0 / 255.0, 1.0],
    alphas: [112.0, 128.0],
    tops: [112.0, 128.0],
};
/// The ground symbol's texture, half side, grey and turn a second (`FX_GROUNDSYMBOL01`), and the
/// glow's texture, half side and colour, both flat quads added whole.
pub const HOLY_SYMBOL_TEXTURE: &str = "FX_GROUNDSYMBOL01";
pub const HOLY_SYMBOL_HALF: f32 = 50.0;
pub const HOLY_SYMBOL_GREY: f32 = 128.0 / 255.0;
pub const HOLY_SYMBOL_TURN: f32 = 0.5;
pub const HOLY_GLOW_TEXTURE: &str = "PARTICLE_GLOW03";
pub const HOLY_GLOW_HALF: f32 = 100.0;
pub const HOLY_GLOW_COLOUR: [f32; 4] = [75.0 / 255.0, 233.0 / 255.0, 247.0 / 255.0, 1.0];
/// The sparks' colour (`+0x2134`, `0x708080F0`), added whole (the flags `0x31`).
pub const HOLY_SPARK_COLOUR: [f32; 4] = [128.0 / 255.0, 128.0 / 255.0, 240.0 / 255.0, 1.0];

/// A column of light's measures, the holy light's ([`HOLY_COLUMN`]) and the purgatory's
/// ([`hellfire_column`]): a cylinder of its segments of its radius and height, and two bands
/// across it from its edge out to `band` along x, fading to nothing at their outer edges; its
/// colours at its foot and its top, and its alphas at its foot and its top (out of 255), the
/// cylinder's and the bands' inner edges'.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Column {
    pub radius: f32,
    pub height: f32,
    pub segments: u16,
    pub band: f32,
    pub foot: [f32; 3],
    pub top: [f32; 3],
    pub alphas: [f32; 2],
    pub tops: [f32; 2],
}

/// The holy light's pulse at its clock `t` (`+0x20AC`): `0.8 + 0.1 sin 4t`, which its column's
/// alphas and its glow's take.
pub fn holy_pulse(t: f32) -> f32 {
    0.8 + 0.1 * (4.0 * t).sin()
}

/// The holy light's sparks and its fade (`+0x20B0`): from 1 down by `dt / seconds` a run, its
/// column's and its sparks' breadth and its sparks' size; at 0.01 it ends.
#[derive(Clone, Debug, PartialEq)]
pub struct HolyLight {
    pub system: System,
    pub fade: f32,
    seconds: f32,
}

/// Its sparks at most (`0x5A0C50`'s count for the type 804).
const HOLY_SPARKS: usize = 50;

impl HolyLight {
    /// The look of a holy light of `seconds`.
    pub fn new(seconds: f32) -> Self {
        Self {
            system: System::new(HOLY_SPARKS),
            fade: 1.0,
            seconds,
        }
    }

    /// Its layer: the life −300 a second, its sparks living 0.85 s.
    fn layer() -> Layer {
        Layer {
            life: -300.0,
            ..Layer::default()
        }
    }

    /// A run of `dt` seconds (`0x781030`): its sparks aged, and new ones every 0.03 s at its place,
    /// rising at a buoyancy of −1,500, of the half side `3.5 ± 1.75` and within 7 along x and y,
    /// both times its fade; its fade down by `dt / seconds`.
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) {
        let breadth = 7.0 * self.fade;
        let block = Block {
            buoyancy: spread(-1_500.0, 0.0),
            size: spread(breadth * 0.5, breadth * 0.25),
            offset: spread([0.0; 3], [breadth, breadth, 0.0]),
            interval: 0.03,
            ..Block::default()
        };
        let layers = [Self::layer()];
        self.system.update(dt, &layers);
        self.system.emit(&[block], &layers, HOLY_SPARKS, dice);
        if self.seconds > 0.0 {
            self.fade -= dt / self.seconds;
        } else {
            self.fade = 0.0;
        }
    }

    /// Whether it ended: its fade at 0.01 or below.
    pub fn done(&self) -> bool {
        self.fade <= 0.01
    }
}

/// The look of the Seraphim's blades of light, `cParticleSystem_holywall` (814, its start
/// `0x784C90`, its run `0x784630` and its draw `0x7846C0`; docs/re/effects.md, "The blades of
/// light"), which goes with its caster for its seconds: a glow under it ([`BLADES_DISC_TEXTURE`]),
/// a ring of dots of light at its size about it and two spirals of them climbing from the ring
/// and turning ([`Blades`]), and a spark at each spiral's top.
pub const BLADES_DISC_TEXTURE: &str = "PARTICLE_GLOW01";
/// The glow under it: a flat quad of a half side 20 beyond its radius, of (175, 175, 255), laid
/// over by its alpha.
pub const BLADES_DISC_BEYOND: f32 = 20.0;
pub const BLADES_DISC_COLOUR: [f32; 4] = [175.0 / 255.0, 175.0 / 255.0, 1.0, 1.0];
/// Its dots (`+0x20B8`): quads facing the camera of `PARTICLE_GLOW03`, of a half side of 4, their
/// corners' colours (0, 0, 128), (64, 64, 128), (0, 64, 128) and (64, 0, 128) from the bottom
/// left, the top left, the bottom right and the top right, added whole.
pub const BLADES_DOT_TEXTURE: &str = "PARTICLE_GLOW03";
pub const BLADES_DOT_HALF: f32 = 4.0;
pub const BLADES_DOT_COLOURS: [[f32; 4]; 4] = [
    [0.0, 0.0, 128.0 / 255.0, 1.0],
    [64.0 / 255.0, 64.0 / 255.0, 128.0 / 255.0, 1.0],
    [0.0, 64.0 / 255.0, 128.0 / 255.0, 1.0],
    [64.0 / 255.0, 0.0, 128.0 / 255.0, 1.0],
];
/// Its sparks (`+0x2138`): white quads facing the camera of `PARTICLE_SPARK06`, of a half side of
/// 15 and up to 10 more at random at each draw, added whole.
pub const BLADES_SPARK_TEXTURE: &str = "PARTICLE_SPARK06";
pub const BLADES_SPARK_HALF: (f32, f32) = (15.0, 10.0);

/// The blades of light's turn and height (`+0x20A8`, `+0x20AC`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Blades {
    pub angle: f32,
    pub height: f32,
}

impl Blades {
    /// A run of `dt` seconds (`0x784630`): its turn on by 5 radians a second, its height up by 33
    /// a second to 100.
    pub fn update(&mut self, dt: f32) {
        self.angle = (self.angle + 5.0 * dt) % std::f32::consts::TAU;
        if self.height < 100.0 {
            self.height += 33.0 * dt;
        }
    }

    /// Its dots about its place at the radius `radius`, in its frame, the third coordinate up: 100
    /// on the ring, every 1/50 of a turn (each place twice), then each spiral's 200, from a
    /// quarter turn either side of its turn round twice, one up, one down, to its height.
    pub fn dots(&self, radius: f32) -> Vec<[f32; 3]> {
        let at = |angle: f32, height: f32| [angle.sin() * radius, angle.cos() * radius, height];
        let step = |k: u16| f32::from(k) * 0.005;
        let quarter = std::f32::consts::FRAC_PI_2;
        let turns = 4.0 * std::f32::consts::PI;
        let ring = (0..100_u16).map(|k| at(f32::from(k) * std::f32::consts::TAU / 50.0, 0.0));
        let up = (0..200_u16).map(|k| {
            at(
                step(k) * turns + self.angle + quarter,
                step(k) * self.height,
            )
        });
        let down = (0..200_u16).map(|k| {
            at(
                self.angle - step(k) * turns - quarter,
                step(k) * self.height,
            )
        });
        ring.chain(up).chain(down).collect()
    }

    /// Its two sparks about its place at the radius `radius`: a quarter turn either side of its
    /// turn, at its height.
    pub fn sparks(&self, radius: f32) -> [[f32; 3]; 2] {
        let quarter = std::f32::consts::FRAC_PI_2;
        [self.angle + quarter, self.angle - quarter]
            .map(|angle| [angle.sin() * radius, angle.cos() * radius, self.height])
    }
}

/// The look of the lightning 13, `cParticleSystem_lightningStrike` (798, its start `0x77D5C0`, its
/// run `0x77CF70` and its draw `0x77D000`; docs/re/effects.md, "The lightning"), at what it
/// strikes: three bolts rising from it out of the screen for 0.25 s ([`LightningBolt`]), a glow
/// for 0.1 s and a burst of sparks ([`lightning_sparks`]), its light 200; a hero's of
/// `FX_LIGHTSTRIKE05` in (160, 176, 255), a creature's of `FX_LIGHTSTRIKE06` in (231, 73, 255)
/// (`+0xA9`).
pub const LIGHTNING_LIGHT: f32 = 200.0;
pub const LIGHTNING_BOLTS: usize = 3;
pub const LIGHTNING_BOLTS_SHOW: f32 = 0.25;
pub const LIGHTNING_GLOW_SHOWS: f32 = 0.1;
pub const LIGHTNING_TEXTURES: [&str; 2] = ["FX_LIGHTSTRIKE05", "FX_LIGHTSTRIKE06"];
pub const LIGHTNING_COLOURS: [[f32; 4]; 2] = [
    [160.0 / 255.0, 176.0 / 255.0, 1.0, 1.0],
    [231.0 / 255.0, 73.0 / 255.0, 1.0, 1.0],
];
/// The glow: `PARTICLE_GLOW05` of a half side of 25 in its colour, added whole (the style 2).
pub const LIGHTNING_GLOW_TEXTURE: &str = "PARTICLE_GLOW05";
pub const LIGHTNING_GLOW_HALF: f32 = 25.0;
/// A bolt's segments: 20 wide and 250 high, each the next 40 times its slant along x
/// (`+0x2168`); and of the texture the strip along `u` of the height 1/8 of it.
pub const LIGHTNING_SEGMENT: (f32, f32, f32) = (20.0, 250.0, 40.0);
pub const LIGHTNING_STRIP: f32 = 0.125;

/// A bolt of the lightning: its slant in −1 to 1 (`+0x20BC`), which shifts each segment up from
/// the last by 40 times it along x, and the texture's `u` by it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightningBolt {
    pub slant: f32,
}

impl LightningBolt {
    /// The three bolts of a lightning, of slants at random (`2 rand() / 32767 − 1`).
    pub fn three(dice: &mut impl Dice) -> [Self; LIGHTNING_BOLTS] {
        std::array::from_fn(|_| Self {
            slant: signed(dice),
        })
    }

    /// Its segment `k` from its foot, in its frame, the third coordinate up, with the texture's
    /// points: its bottom left, bottom right, top right and top left corners, its strip turned
    /// over and shifted by half its width where `flip` (a roll of `rand() & 1` at each draw).
    pub fn segment(&self, k: u16, flip: bool) -> [([f32; 3], [f32; 2]); 4] {
        let (width, height, drift) = LIGHTNING_SEGMENT;
        let half = width * 0.5;
        let (low, high) = (f32::from(k), f32::from(k) + 1.0);
        let corner =
            |step: f32, side: f32| [drift * self.slant * step + side * half, 0.0, height * step];
        let o = self.slant;
        // The top's `u` the slant, the foot's one more; across, the strip's height.
        let (u, v) = if flip {
            ((o + 0.5, o + 1.5), (LIGHTNING_STRIP, 0.0))
        } else {
            ((o, o + 1.0), (0.0, LIGHTNING_STRIP))
        };
        [
            (corner(low, -1.0), [u.1, v.1]),
            (corner(low, 1.0), [u.1, v.0]),
            (corner(high, 1.0), [u.0, v.0]),
            (corner(high, -1.0), [u.0, v.1]),
        ]
    }
}

/// The lightning's burst of sparks at its start (`0x77D5C0`, `0x764760` of the interval 0): 150
/// of `PARTICLE_SPARK06` within 10 of its place, shot up at 50 ± (40, 40, 20) and falling back at
/// a buoyancy of 150 ± 40, of a half side of 7 ± 1 shrinking by 2 a second, living 1.02 s, in its
/// colour (`colour`, the flags `0x13`: its corners' colour, added whole), turned at random.
pub fn lightning_sparks(colour: [f32; 4], dice: &mut impl Dice) -> Burst {
    let pi = std::f32::consts::PI;
    let block = Block {
        buoyancy: spread(150.0, 40.0),
        size: spread(7.0, 1.0),
        angle: spread(pi, pi),
        velocity: spread([0.0, 0.0, 50.0], [40.0, 40.0, 20.0]),
        offset: spread([0.0; 3], [10.0; 3]),
        ..Block::default()
    };
    let particles = (0..LIGHTNING_SPARKS)
        .map(|_| born(&block, 0, dice))
        .collect();
    Burst {
        texture: "PARTICLE_SPARK06",
        grid: 1,
        blend: Blend::Add,
        layer: Layer {
            life: -250.0,
            size: -2.0,
            ..Layer::default()
        },
        ramp: Ramp::new(colour, &[(colour, 256)]),
        particles,
        ground: None,
        light: 0.0,
        tinted: false,
        stream: None,
    }
}

/// The lightning's particles (`0x5A0C50`'s count for the type 798), all of its burst.
const LIGHTNING_SPARKS: usize = 150;

/// A bolt of the chain lightning 33 (docs/re/effects.md, "The chain lightning"): its segments of
/// `cParticleSystem_freeze` (799, its start `0x77DE80`, its run `0x77DAC0` and its draw
/// `0x77DB20`) along the line from its start to its end, each 30 long but the last, which the
/// executor (`0x526CAA`) lays one after the other, at most 19; and their fade, whole for 0.4 s,
/// then falling by 20 a second, so that the bolt shows for 0.45 s.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainBolt {
    pub segments: Vec<ChainSegment>,
    pub fade: f32,
    clock: f32,
}

/// A segment of a chain lightning's bolt: its index along the bolt, its length, and its ends'
/// offsets across it (`+0x4C`, `+0x50` and `+0x58`, `+0x5C` of its event), along its frame's `y`
/// and `z`, its `x` along the bolt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChainSegment {
    pub index: u16,
    pub length: f32,
    pub near: [f32; 2],
    pub far: [f32; 2],
}

/// A segment's step along the bolt, its segments at most, its quads' half breadth (`+0x20F0`: one
/// in its frame's `x`–`y` plane, one in its `x`–`z`, each from −50 to 50 across), the texture's
/// `u` a unit along the bolt on each quad (`+0x2108`), the texture, the first segment's glow's
/// texture and half side (the style 6), and the lights of the first and the others.
pub const CHAIN_STEP: f32 = 30.0;
const CHAIN_SEGMENTS: u16 = 19;
pub const CHAIN_HALF: f32 = 50.0;
pub const CHAIN_U: [f32; 2] = [1.0 / 300.0, 0.02];
pub const CHAIN_TEXTURE: &str = "FX_LIGHTNING01";
pub const CHAIN_GLOW_TEXTURE: &str = "PARTICLE_GLOW06";
pub const CHAIN_GLOW_HALF: f32 = 20.0;
pub const CHAIN_LIGHTS: [f32; 2] = [150.0, 100.0];

/// An offset across a segment's end (`0x63FDE0`): along `y` and `z`, each 16 times a share at
/// random less 8, the `z` rolled first.
fn chain_offset(dice: &mut impl Dice) -> [f32; 2] {
    let z = share(dice) * 16.0 - 8.0;
    let y = share(dice) * 16.0 - 8.0;
    [y, z]
}

impl ChainBolt {
    /// A bolt over `total` (`0x526CAA`): segments of 30 while any of it is left, the last of its
    /// rest rounded down; each end's offset across it the offset rolled for it times
    /// `sin(π l / total)`, `l` what is left of the bolt from it, so that the bolt bulges at its
    /// middle and none at its ends; a segment's far offset rolled anew for each, its near one its
    /// predecessor's far one.
    pub fn new(total: f32, dice: &mut impl Dice) -> Self {
        let mut near = chain_offset(dice);
        let mut far = chain_offset(dice);
        let mut segments = Vec::new();
        let mut left = total;
        let mut index = 0;
        let bulge = |l: f32| {
            if total > 0.0 {
                (std::f32::consts::PI * l / total).sin()
            } else {
                0.0
            }
        };
        while left > 0.0 && index < CHAIN_SEGMENTS {
            segments.push(ChainSegment {
                index,
                length: if left < CHAIN_STEP {
                    left.trunc()
                } else {
                    CHAIN_STEP
                },
                near: near.map(|offset| offset * bulge(left)),
                far: far.map(|offset| offset * bulge(left - CHAIN_STEP)),
            });
            near = far;
            far = chain_offset(dice);
            left -= CHAIN_STEP;
            index += 1;
        }
        Self {
            segments,
            fade: 1.0,
            clock: 0.0,
        }
    }

    /// A run of `dt` seconds (`0x77DAC0`).
    pub fn update(&mut self, dt: f32) {
        self.clock += dt;
        if self.clock > 0.4 {
            self.fade = (self.fade - 20.0 * dt).max(0.0);
        }
    }

    /// Its segments' grey (`0x77DB20`): half of `⌊255 f⌋`, `f` its fade.
    pub fn grey(&self) -> f32 {
        (255.0 * self.fade).trunc() / 2.0 / 255.0
    }

    /// Whether it ended: faded out.
    pub fn done(&self) -> bool {
        self.clock > 0.4 && self.fade <= 0.0
    }
}

/// The impact of a chain lightning's bolt at its end, `cParticleSystem_freeze_impact` (872, its
/// start `0x77E630`, its run `0x77E2A0` and its draw `0x77E3A0`): a burst of 15 sparks of
/// `PARTICLE_SPARK06` at its start, within 3 of its place, of a half side of 4 ± 1, flying off
/// at up to 150 along each axis and drawn back toward its place by 150 a second, falling by
/// 50 ± 40 a second, living 0.64 s, from (127, 127, 255) to black, added whole; and a glow of the
/// half side 30 at its place, in the grey of its fade, which falls as the bolt's.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainImpact {
    pub sparks: System,
    pub fade: f32,
    clock: f32,
}

/// The impact's sparks' texture, their pull toward its place (its layer's `+0x1C`), its glow's
/// half side, and its light times its fade (`0x77E3BD`).
pub const IMPACT_TEXTURE: &str = "PARTICLE_SPARK06";
const IMPACT_PULL: f32 = 150.0;
pub const IMPACT_GLOW_HALF: f32 = 30.0;
pub const IMPACT_LIGHT: f32 = 200.0;
/// Its sparks (`0x5A0C50`'s count for the type 872), all of its burst.
const IMPACT_SPARKS: usize = 15;

impl ChainImpact {
    pub fn new(dice: &mut impl Dice) -> Self {
        let block = Block {
            buoyancy: spread(50.0, 40.0),
            size: spread(4.0, 1.0),
            velocity: spread([0.0; 3], [150.0; 3]),
            offset: spread([0.0; 3], [3.0; 3]),
            ..Block::default()
        };
        let mut sparks = System::new(IMPACT_SPARKS);
        sparks.particles = (0..IMPACT_SPARKS).map(|_| born(&block, 0, dice)).collect();
        Self {
            sparks,
            fade: 1.0,
            clock: 0.0,
        }
    }

    /// Its sparks' layer (`+0x20AC`): the life −400 a second.
    pub fn layer() -> Layer {
        Layer {
            life: -400.0,
            ..Layer::default()
        }
    }

    /// Its sparks' ramp (`+0x2130`): the entry `k` of `(k/2, k/2, k)`, from (127, 127, 255) at
    /// their birth to black.
    pub fn ramp() -> Ramp {
        Ramp(std::array::from_fn(|k| {
            let k = f32::from(u8::try_from(k).unwrap_or(u8::MAX));
            [
                (k / 2.0).trunc() / 255.0,
                (k / 2.0).trunc() / 255.0,
                k / 255.0,
                0.0,
            ]
        }))
    }

    /// A run of `dt` seconds (`0x77E2A0`, `0x7640C0`): its sparks age, each drawn toward its place
    /// by the pull, and after 0.4 s its fade falls by 20 a second.
    pub fn update(&mut self, dt: f32) {
        self.clock += dt;
        self.sparks.update(dt, &[Self::layer()]);
        for spark in &mut self.sparks.particles {
            let length = spark
                .place
                .iter()
                .map(|axis| axis * axis)
                .sum::<f32>()
                .sqrt();
            if length >= 1e-7 {
                for (velocity, axis) in spark.velocity.iter_mut().zip(spark.place) {
                    *velocity -= dt * IMPACT_PULL * axis / length;
                }
            }
        }
        if self.clock > 0.4 {
            self.fade = (self.fade - 20.0 * dt).max(0.0);
        }
    }

    /// Its glow's grey: `⌊255 f⌋`, `f` its fade.
    pub fn grey(&self) -> f32 {
        (255.0 * self.fade).trunc() / 255.0
    }

    /// Whether it ended: its last spark gone (`0x7640C0`'s answer).
    pub fn done(&self) -> bool {
        self.sparks.particles.is_empty()
    }
}

/// The look of the light 20, `cParticleSystem_maglite` (817, its start `0x7850D0` and its draw
/// `0x784E90`; docs/re/effects.md, "The light"), which goes with its caster for its seconds: a
/// ball of light of the radius 55 about its place, of 21 flat slices of `PARTICLE_GLOW03`
/// ([`maglite_slices`]), each of (175, 175, 255) at an alpha of 24, added by its alpha; its light
/// at night the spell's second value at its level.
pub const MAGLITE_TEXTURE: &str = "PARTICLE_GLOW03";
pub const MAGLITE_COLOUR: [f32; 4] = [175.0 / 255.0, 175.0 / 255.0, 1.0, 24.0 / 255.0];

/// The light's slices: at the heights `2.5 (k − 10)` for `k` of 0 to 20 about its place, each of
/// the half side `√(55² − h²)`.
pub fn maglite_slices() -> impl Iterator<Item = (f32, f32)> {
    (0..21_u8).map(|k| {
        let height = (f32::from(k) - 10.0) * 2.5;
        (height, (3_025.0 - height * height).sqrt())
    })
}

/// The look of a soul of the Wood Elf's lost souls 41, `cParticleSystem_lostsouls` (850, its
/// start `0x79D310`, its run `0x79CEB0` and its draw `0x79CFB0`; docs/re/effects.md, "The lost
/// souls"), which goes with its object for its seconds: green dots of `PARTICLE_DOTS01` left
/// behind it from a point that wobbles about its place ([`soul_wobble`]), one every 0.01 s, of a
/// half side of 7 shrinking by 4 a second and spinning at 6 radians a second, fading out over
/// 0.64 s, added by their alpha (the flags `0xD`); and a glow beating once a second at that point
/// ([`soul_glow`]). Its seconds over, it makes no more and ends with its last dot.
#[derive(Clone, Debug, PartialEq)]
pub struct LostSoul {
    pub system: System,
    seconds: f32,
    out: bool,
    colour: [f32; 3],
}

/// The dryad ghost (1162, `cParticleSystem_dryad_ghost`, its start `0x7BF3A0`, its run `0x7BF1E0`
/// and its draw `0x7BF2C0`; docs/re/effects.md, "The dryad ghost"), which a sector's fill makes
/// for its dummy 1249 (`0x480D40`): a lost soul's look with blue dots ([`LostSoul::dryad_ghost`]),
/// which flies at 120 a second (`+0x98`), turning by 3 radians a second at most (`+0xE0`), at the
/// nearest creature hostile to its dummy within a squared 100,000 (`0x600570`), at its bone 3
/// (`Bip01 Neck`), else hovers 10 over its place (`0x5A7F28`); once a second it strikes every such
/// creature within its size and theirs (`0x5B17AB`), and it loops `HELF_AHNEN_LOOP` (2058).
pub const DRYAD_GHOST: u32 = 1162;
pub const DRYAD_GHOST_DUMMY: u32 = 1249;
pub const DRYAD_GHOST_SPEED: f32 = 120.0;
pub const DRYAD_GHOST_TURN: f32 = 3.0;
pub const DRYAD_GHOST_LOOKS: f32 = 316.2;
pub const DRYAD_GHOST_HOVER: f32 = 10.0;
pub const DRYAD_GHOST_SIZE: f32 = 50.0;
pub const DRYAD_GHOST_BEAT: f32 = 1.0;
pub const DRYAD_GHOST_LOOP: u32 = 2058;

/// A soul's dots' texture, their particles at most (`0x5A0C50`'s count for the type 850), and its
/// glow's texture (the style 5).
pub const SOUL_TEXTURE: &str = "PARTICLE_DOTS01";
const SOUL_COUNT: usize = 200;
pub const SOUL_GLOW_TEXTURE: &str = "PARTICLE_GLOW03";

/// A beat of a second at the clock `t` (`0x768790`, `0x768600`): from 0 up to 1 over the first
/// 0.3 s of each second, down to 0 over the rest.
pub fn beat(t: f32) -> f32 {
    let phase = t - t.floor();
    if phase < 0.3 {
        phase / 0.3
    } else {
        (1.0 - phase) / 0.7
    }
}

/// The point a soul's dots leave from and its glow lies at, about its place at its clock `t`
/// (`+0x2508`): `5 sin` of 6.58, 6.87 and 7.49 times `t` along each axis.
pub fn soul_wobble(t: f32) -> [f32; 3] {
    [6.58, 6.87, 7.49].map(|rate: f32| 5.0 * (rate * t).sin())
}

/// A soul's glow at its clock `t` (`0x79CFB0`): its half side `20 + 8 b` and its colour
/// (128, 239, 128) plus `b` times (64, 16, 64), `b` its beat ([`beat`]), added whole.
pub fn soul_glow(t: f32) -> (f32, [f32; 4]) {
    let b = beat(t);
    let c = |base: f32, more: f32| (base + more * b).round() / 255.0;
    (
        20.0 + 8.0 * b,
        [c(128.0, 64.0), c(239.0, 16.0), c(128.0, 64.0), 1.0],
    )
}

/// The look of the Daemon's choir 66 on a creature it holds, `cParticleSystem_songsfromhell`
/// (1011, its start `0x7B0200`, its run `0x7B0040` and its draw `0x7B0110`; docs/re/effects.md,
/// "The choir"), which goes with its creature for its seconds: it fades in over its first second,
/// then black wisps of `PARTICLE_GLOW01` rise from within its base of the creature's size over 6
/// along each axis of the ground, one every 0.07 s, of a half side of 6 ± 3 shrinking by 2 a
/// second, rising ever faster by 20 ± 15 a second, fading over 2.55 s, laid over by their
/// alpha (the flags 2); and five black discs under it ([`Choir::discs`]). Its seconds over, it
/// makes no more and ends with its last wisp.
#[derive(Clone, Debug, PartialEq)]
pub struct Choir {
    pub system: System,
    pub clock: f32,
    /// Its fade (`+0x20B0`), from 0 up to 1 over its first second.
    pub fade: f32,
    /// Its base (`+0x20AC`): its event's `+0x38`, the creature's size (`0x428CE0`), over 6.
    pub base: f32,
    seconds: f32,
}

/// The choir's texture, its wisps at most (`0x5A0C50`'s count for the type 1011), and its discs.
pub const CHOIR_TEXTURE: &str = "PARTICLE_GLOW01";
const CHOIR_COUNT: usize = 150;
const CHOIR_DISCS: u8 = 5;

/// The look of a magic bar or barrier of the scripts, `cParticleSystem_magicprison`
/// (`FX_MAGICPRISON`, 854, its start `0x79CC80`, its run `0x79C9F0` and its draw `0x79CA40`;
/// docs/re/effects.md, "The magic prison"), which stands where its record puts it until its
/// `DelNPC`: glows of `PARTICLE_GLOW03` rising within 2 of its place along the ground and up to
/// 100 above it, one every 0.05 s, of a half side of 5 ± 2 growing by 10 a second, drifting up or
/// down by up to 20 a second, living 1.275 s, from (64, 64, 128) to black, added whole (the flags
/// `0x19`); over a glow of `PARTICLE_GLOW03` on the ground ([`PRISON_GLOW_HALF`]). No light.
pub fn magic_prison() -> Fire {
    Fire {
        texture: PRISON_TEXTURE,
        layers: vec![Layer {
            life: -200.0,
            size: 10.0,
            ..Layer::default()
        }],
        blocks: vec![Block {
            buoyancy: spread(0.0, 20.0),
            size: spread(5.0, 2.0),
            offset: spread([0.0, 0.0, 50.0], [2.0, 2.0, 50.0]),
            interval: 0.05,
            ..Block::default()
        }],
        ramps: vec![prison_ramp()],
        blends: vec![Blend::Add],
        grid: 1,
        count: PRISON_COUNT,
        light: 0.0,
        tinted: false,
    }
}

/// The magic prison's ramp (`+0x20A8`): the entry `k` of `⌊64 k / 256⌋` red and green and
/// `⌊128 k / 256⌋` blue, from (63, 63, 127) at a glow's birth to black.
fn prison_ramp() -> Ramp {
    Ramp(std::array::from_fn(|k| {
        let k = f32::from(u16::try_from(k).unwrap_or(0));
        let rg = (64.0 * k / 256.0).trunc() / 255.0;
        [rg, rg, (128.0 * k / 256.0).trunc() / 255.0, 1.0]
    }))
}

/// Its texture, its glows at most (`0x5A0C50`'s count for the type 854), and its glow on the
/// ground (`0x7631E0`): a half side of 40 in (128, 128, 255), added by its alpha over the opaque
/// texture.
pub const PRISON_TEXTURE: &str = "PARTICLE_GLOW03";
const PRISON_COUNT: usize = 100;
pub const PRISON_GLOW_HALF: f32 = 40.0;
pub const PRISON_GLOW_COLOUR: [f32; 4] = [128.0 / 255.0, 128.0 / 255.0, 1.0, 1.0];
/// Its sound as it is made (`0x79CC80`): `SPELL_BANNKREIS` once, at the rate's factor 0.5.
pub const PRISON_SOUND: u32 = 2029;

/// The look of an object of a big demon's summoning, `cParticleSystem_bigdemonsummon`
/// (`FX_BIG_DEMON_SUMMON`, 918, its start `0x7A92E0`, its run `0x7A8C10` and its draw
/// `0x7A9060`; docs/re/effects.md, "The big demon's summoning"), which the scripts make six at
/// once about one place (the record's `0x03` its variant): its emitter goes, a turn every 4 s,
/// about a circle of the radius 285 (the variant 0) or to and fro along a side of the pentagram
/// in it (1 to 5), and leaves glows of `PARTICLE_GLOW03` where it passes for 8 s, added whole,
/// so that a burning pentagram in a circle is drawn on the ground. The circle's light is 600.
#[derive(Clone, Debug, PartialEq)]
pub struct DemonSummon {
    pub system: System,
    pub variant: u8,
    /// Its emitter's share of its turn (`+0x20A8`), up by 0.25 a second.
    pub phase: f32,
    /// Its emitter's place in its frame (`+0x2508`).
    pub emitter: [f32; 3],
}

/// The summoning's texture, its glows at most (`0x5A0C50`'s count for the type 918), its circle's
/// radius, how long it emits, its circle's light and its sounds as each object is set up (the FX
/// event `0x7E`'s case, `0x5A48F0`): `BOSS_SAKKARA_DEMON_GROWL01` or `02` at even odds, at the
/// rate's factor 0.5.
pub const SUMMON_TEXTURE: &str = "PARTICLE_GLOW03";
const SUMMON_COUNT: usize = 400;
pub const SUMMON_RADIUS: f32 = 285.0;
const SUMMON_EMITS: f32 = 8.0;
pub const SUMMON_LIGHT: f32 = 600.0;
pub const SUMMON_GROWLS: [u32; 2] = [11426, 11427];

/// The pentagram's points (`0x7A8A60`): on the circle at the angles 0, 4π/5, 8π/5, 2π/5, 6π/5
/// and 0 again along its frame's `sin` and `cos`, so that each side joins two of its points.
fn summon_point(k: usize) -> [f32; 3] {
    let angle = std::f32::consts::TAU * 0.4 * f32::from(u8::try_from(k % 5).unwrap_or(0));
    [
        SUMMON_RADIUS * angle.sin(),
        SUMMON_RADIUS * angle.cos(),
        0.0,
    ]
}

impl DemonSummon {
    /// An object of the variant `variant` (`0x7A92E0`).
    pub fn new(variant: u8) -> Self {
        let mut summon = Self {
            system: System::new(SUMMON_COUNT),
            variant,
            phase: 0.0,
            emitter: [0.0; 3],
        };
        summon.emitter = summon.place();
        summon
    }

    /// Whether it draws the circle (0 and 10) rather than a side.
    fn circle(&self) -> bool {
        self.variant == 0 || self.variant == 10
    }

    /// Its light: the circle's alone.
    pub fn light(&self) -> f32 {
        if self.circle() { SUMMON_LIGHT } else { 0.0 }
    }

    /// Its layer (`+0x24B0`): the life −150 a second, the size −2.
    pub fn layer() -> Layer {
        Layer {
            life: -150.0,
            size: -2.0,
            ..Layer::default()
        }
    }

    /// Its ramp (`+0x20B0`): from black over 5 entries to (150, 150, 196) (a red (150, 100,
    /// 100) for the variants from 10), over 50 to (196, 105, 30) ((196, 38, 30)), and over 201 to
    /// black; the values out of 256, added whole.
    pub fn ramp(&self) -> Ramp {
        let c = |r: f32, g: f32, b: f32| [r / 256.0, g / 256.0, b / 256.0, 1.0];
        let (first, second) = if self.variant < 10 {
            (c(150.0, 150.0, 196.0), c(196.0, 105.0, 30.0))
        } else {
            (c(150.0, 100.0, 100.0), c(196.0, 38.0, 30.0))
        };
        let black = [0.0, 0.0, 0.0, 1.0];
        Ramp::new(black, &[(first, 5), (second, 50), (black, 201)])
    }

    /// Its block (`+0x24D0`), at its emitter: the size 10, one every 0.01 s for the circle and
    /// 0.02 s for a side, at the detail 2.
    fn block(&self) -> Block {
        Block {
            size: spread(10.0, 0.0),
            interval: if self.circle() { 0.01 } else { 0.02 },
            ..Block::default()
        }
    }

    /// Its emitter's place at its phase (`0x7A8C10`): about the circle from (285, 0) turned by a
    /// whole turn of its phase; on its side, from its point to the next over the first half of its
    /// phase and back over the second.
    fn place(&self) -> [f32; 3] {
        if self.circle() {
            let angle = self.phase * std::f32::consts::TAU;
            return [
                SUMMON_RADIUS * angle.cos(),
                SUMMON_RADIUS * angle.sin(),
                0.0,
            ];
        }
        let side = usize::from(self.variant % 10).saturating_sub(1);
        let (from, to) = (summon_point(side), summon_point(side + 1));
        let share = if self.phase < 0.5 {
            2.0 * self.phase
        } else {
            2.0 * (1.0 - self.phase)
        };
        std::array::from_fn(|k| from[k] + (to[k] - from[k]) * share)
    }

    /// A run of `dt` seconds: its glows age, its emitter goes on, and for its first 8 s it leaves
    /// glows along its way.
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) {
        let layers = [Self::layer()];
        self.system.update(dt, &layers);
        self.phase += 0.25 * dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let from = self.emitter;
        self.emitter = self.place();
        if self.system.clock < SUMMON_EMITS {
            let block = self.block();
            self.system.emit_along(
                &[block],
                &layers,
                SUMMON_COUNT,
                (from, self.emitter),
                dt,
                dice,
            );
        }
    }

    /// Whether it ended: past its 8 s and no glow alive.
    pub fn done(&self) -> bool {
        self.system.clock >= SUMMON_EMITS && self.system.particles.is_empty()
    }
}

/// The look of an object of the Daemon's fear 67, `cParticleSystem_hellscream` (1012, its start
/// `0x7B0AF0`, its run `0x7B07B0` and its draw `0x7B0870`; docs/re/effects.md, "The fear"), where
/// it stands for its seconds: a glow on the ground ([`FEAR_GLOW_HALF`]), a ring spreading from
/// 10 to 100 over and over ([`HellScream::ring`]), and a column of light of camera-facing glows
/// stacked up as it fades in ([`HellScream::column`]). It fades in at 1 a second and out at 0.3
/// a second after its seconds. Its light is 350.
#[derive(Clone, Debug, PartialEq)]
pub struct HellScream {
    pub clock: f32,
    /// Its fade (`+0x20AC`).
    pub fade: f32,
    /// Its ring's inner radius (`+0x20B0`).
    pub radius: f32,
    seconds: f32,
    out: bool,
}

/// The fear's glow (`+0x2138`, `PARTICLE_GLOW03`, added whole) and its colour (`0xFF8246`), its
/// ring's texture (`+0x213C`), breadth and segments, its column's texture (`+0x2134`, added by
/// its alpha), colour (`0xFFFFA266`) and glows, and its light.
pub const FEAR_GLOW_TEXTURE: &str = "PARTICLE_GLOW03";
pub const FEAR_GLOW_HALF: f32 = 60.0;
pub const FEAR_GLOW_COLOUR: [f32; 4] = [1.0, 130.0 / 255.0, 70.0 / 255.0, 1.0];
pub const FEAR_RING_TEXTURE: &str = "laserbeampart";
pub const FEAR_RING_BREADTH: f32 = 20.0;
pub const FEAR_RING_SEGMENTS: u16 = 50;
pub const FEAR_COLUMN_TEXTURE: &str = "PARTICLE_GLOW01";
pub const FEAR_COLUMN_COLOUR: [f32; 4] = [1.0, 162.0 / 255.0, 102.0 / 255.0, 1.0];
const FEAR_COLUMN_GLOWS: u8 = 32;
pub const FEAR_LIGHT: f32 = 350.0;

impl HellScream {
    /// An object of the fear for `seconds` (its record's value 0 at its level, `0x7B0AF0`).
    pub fn new(seconds: f32) -> Self {
        Self {
            clock: 0.0,
            fade: 0.0,
            radius: 10.0,
            seconds,
            out: false,
        }
    }

    /// A run of `dt` seconds (`0x7B07B0`).
    pub fn update(&mut self, dt: f32) {
        self.clock += dt;
        if self.clock <= self.seconds {
            self.fade = if self.fade >= 1.0 {
                1.0
            } else {
                self.fade + dt
            };
        } else if self.fade <= 0.0 {
            self.fade = 0.0;
            self.out = true;
        } else {
            self.fade -= 0.3 * dt;
        }
        self.radius += 70.0 * dt;
        if self.radius > 100.0 {
            self.radius = 10.0;
        }
    }

    /// Its ring (`0x763530`): flat between its radius and 20 more, of (255, 162, 102) times a
    /// share that holds 1 to the radius 75 and falls to 0 at 100, each byte taken whole, added
    /// whole.
    pub fn ring(&self) -> (f32, f32, [f32; 4]) {
        let share = if self.radius < 75.0 {
            1.0
        } else {
            1.0 - (self.radius - 75.0) * 0.04
        };
        let byte = |value: f32| (value * share).trunc().max(0.0) / 255.0;
        (
            self.radius,
            self.radius + FEAR_RING_BREADTH,
            [byte(255.0), byte(162.0), byte(102.0), 1.0],
        )
    }

    /// Its column's glows (`0x7B0A0D`): the `i`-th of 32 at the height `4 i f` over its place, of
    /// the half side `1.7 sin(0.3 i + t) + 1.6 (sin(i − 3t) + 1) + 6 + f`, `t` its clock and `f`
    /// its fade.
    pub fn column(&self) -> impl Iterator<Item = (f32, f32)> + '_ {
        let (t, f) = (self.clock, self.fade);
        (0..FEAR_COLUMN_GLOWS).map(move |i| {
            let i = f32::from(i);
            (
                4.0 * i * f,
                1.7 * (0.3 * i + t).sin() + 1.6 * ((i - 3.0 * t).sin() + 1.0) + 6.0 + f,
            )
        })
    }

    /// Its object's end: its seconds over, so that it fades out.
    pub fn end(&mut self) {
        self.seconds = self.seconds.min(self.clock);
    }

    /// Whether it ended: faded out after its seconds.
    pub fn done(&self) -> bool {
        self.out
    }
}

impl Choir {
    /// A choir on a creature of the size `size` for `seconds` (`0x7B0200`).
    pub fn new(size: f32, seconds: f32) -> Self {
        Self {
            system: System::new(CHOIR_COUNT),
            clock: 0.0,
            fade: 0.0,
            base: size / 6.0,
            seconds,
        }
    }

    /// Its wisps' layer (`+0x20C4`): the life −100 a second and the size −2.
    pub fn layer() -> Layer {
        Layer {
            life: -100.0,
            size: -2.0,
            ..Layer::default()
        }
    }

    /// Its block (`+0x20E4`): the buoyancy −20 ± 15, the size 6 ± 3, within its base along each
    /// axis of the ground, one every 0.07 s at the detail 2.
    fn block(&self) -> Block {
        Block {
            buoyancy: spread(-20.0, 15.0),
            size: spread(6.0, 3.0),
            offset: spread([0.0; 3], [self.base, self.base, 0.0]),
            interval: 0.07,
            ..Block::default()
        }
    }

    /// A run of `dt` seconds (`0x7B0040`): fading in, nothing more; whole, its wisps age and,
    /// while its seconds last, it makes new ones about its place.
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) {
        self.clock += dt;
        if self.fade < 1.0 {
            self.fade += dt;
            return;
        }
        self.fade = 1.0;
        let layers = [Self::layer()];
        self.system.update(dt, &layers);
        if self.clock < self.seconds {
            self.system
                .emit(&[self.block()], &layers, CHOIR_COUNT, dice);
        }
    }

    /// Its object's end: no wisp more, its seconds over.
    pub fn end(&mut self) {
        self.seconds = self.seconds.min(self.clock);
        self.fade = 1.0;
    }

    /// Its wisps' colour at the life `life`: black at the alpha of their life (the flags 2).
    pub fn wisp(life: f32) -> [f32; 4] {
        [0.0, 0.0, 0.0, life.clamp(0.0, 255.0) / 255.0]
    }

    /// Its discs (`0x7631E0`): the `k`-th of five `3 k` over its place, of the half side
    /// `(10 − k)(sin t + b) f`, `t` its clock, `b` its base and `f` its fade, black, laid over
    /// by their alpha.
    pub fn discs(&self) -> impl Iterator<Item = (f32, f32)> + '_ {
        (0..CHOIR_DISCS).map(|k| {
            let k = f32::from(k);
            (
                3.0 * k,
                (10.0 - k) * (self.clock.sin() + self.base) * self.fade,
            )
        })
    }

    /// Whether it ended: past its seconds and no wisp alive.
    pub fn done(&self) -> bool {
        self.fade >= 1.0 && self.clock >= self.seconds && self.system.particles.is_empty()
    }
}

impl LostSoul {
    /// A soul for `seconds` (its record's value 1 at its level, `0x79D310`).
    pub fn new(seconds: f32) -> Self {
        Self {
            system: System::new(SOUL_COUNT),
            seconds,
            out: false,
            colour: [60.0, 255.0, 60.0],
        }
    }

    /// A dryad ghost's look ([`DRYAD_GHOST`]): its dots (60, 60, 255) out of 256 (`0x7BF3A0`),
    /// with no seconds of its own, as long as its object lives.
    pub fn dryad_ghost() -> Self {
        Self {
            colour: [60.0, 60.0, 255.0],
            ..Self::new(f32::INFINITY)
        }
    }

    /// Its dots' layer (`+0x24B0`): the life −400 a second and the size −4.
    pub fn layer() -> Layer {
        Layer {
            life: -400.0,
            size: -4.0,
            ..Layer::default()
        }
    }

    /// Its dots' ramp (`+0x20B0`): its colour, (60, 255, 60) out of 256 a soul's, from opaque to
    /// nothing.
    pub fn ramp(&self) -> Ramp {
        let [r, g, b] = self.colour.map(|c| c / 256.0);
        let colour = |alpha: f32| [r, g, b, alpha];
        Ramp::new(colour(255.0 / 256.0), &[(colour(0.0), 256)])
    }

    /// Its block at the clock `t` (`+0x24D0`): the size 7, the angle's speed 6, from its wobbling
    /// point, one every 0.01 s at the detail 2.
    fn block(t: f32) -> Block {
        Block {
            size: spread(7.0, 0.0),
            spin: spread(6.0, 0.0),
            offset: spread(soul_wobble(t), [0.0; 3]),
            interval: 0.01,
            ..Block::default()
        }
    }

    /// A run of `dt` seconds, its place gone from `from` to `to` in the models' frame
    /// (`0x79CEB0`): its dots age, and while its seconds last it leaves new ones along its way.
    pub fn update(&mut self, dt: f32, (from, to): ([f32; 3], [f32; 3]), dice: &mut impl Dice) {
        let layers = [Self::layer()];
        self.system.update(dt, &layers);
        if self.out {
            return;
        }
        let block = Self::block(self.system.clock);
        self.system
            .emit_along(&[block], &layers, SOUL_COUNT, (from, to), dt, dice);
        if self.system.clock > self.seconds {
            self.out = true;
        }
    }

    /// Its object's end (`+0x74` bit 1): no dot more.
    pub fn end(&mut self) {
        self.out = true;
    }

    /// Whether it ended: out and no dot alive.
    pub fn done(&self) -> bool {
        self.out && self.system.particles.is_empty()
    }
}

/// The look of the fire skin, `cParticleSystem_fireskin` (821, its start `0x78BE10`, its run
/// `0x78BB00` and its draw `0x78BBA0`; docs/re/effects.md, "The fire skin"), on its bearer for its
/// seconds: a flame every 0.01 s within 15 of its place along x and y, of a half side of 20 ± 10
/// shrinking by 50 a second, standing on its foot, living 0.73 s and playing its 4 × 4 frames
/// once over it, white coming up over its first 32 entries and going to black over the rest,
/// added by its alpha (`0x761B00`'s flags `0x89`); a hero's of `PARTICLE_FIRE02`, a creature's of
/// `PARTICLE_FIRE09` (`+0xA9`; [`FIRE_SKIN_TEXTURES`]). Its light is 200.
pub fn fire_skin(hero: bool) -> Fire {
    let white = |alpha: f32| [1.0, 1.0, 1.0, alpha];
    Fire {
        texture: FIRE_SKIN_TEXTURES[usize::from(!hero)],
        layers: vec![Layer {
            life: -350.0,
            size: -50.0,
            ..Layer::default()
        }],
        blocks: vec![Block {
            size: spread(20.0, 10.0),
            offset: spread([0.0; 3], [15.0, 15.0, 0.0]),
            interval: 0.01,
            ..Block::default()
        }],
        ramps: vec![Ramp::new(
            white(0.0),
            &[(white(1.0), 32), ([0.0, 0.0, 0.0, 1.0], 224)],
        )],
        blends: vec![Blend::Add],
        grid: 4,
        count: FIRE_SKIN_COUNT,
        light: 200.0,
        tinted: false,
    }
}

/// The fire skin's flames' textures, a hero's and a creature's, and its particles at most.
pub const FIRE_SKIN_TEXTURES: [&str; 2] = ["PARTICLE_FIRE02", "PARTICLE_FIRE09"];
const FIRE_SKIN_COUNT: usize = 100;
/// The fire skin's glow under it while it burns (`0x7631E0`): `PARTICLE_GLOW03` of a half side of
/// 60, flat on the ground and added whole, a hero's (255, 192, 1), a creature's (64, 64, 255).
pub const FIRE_SKIN_GLOW_TEXTURE: &str = "PARTICLE_GLOW03";
pub const FIRE_SKIN_GLOW_HALF: f32 = 60.0;
pub const FIRE_SKIN_GLOW_COLOURS: [[f32; 4]; 2] = [
    [1.0, 192.0 / 255.0, 1.0 / 255.0, 1.0],
    [64.0 / 255.0, 64.0 / 255.0, 1.0, 1.0],
];

/// The look of a flame of the fire spiral 11, `cParticleSystem_fireCircle` (790, its start
/// `0x775DF0`, its run `0x775A70` and its draw `0x775BC0`; docs/re/effects.md, "The fire
/// spiral"), which goes with its object about the caster's place: a flame every 0.01 s within 7
/// of its place along x and y, of a half side of 40 ± 20 shrinking by 70 a second, standing on its
/// foot, living 0.73 s and playing its 4 × 4 frames once over it, an orange (255, 178, 127) coming
/// up over its first 32 entries and holding to its death, added by its alpha (`0x761B00`'s flags
/// `0x89`), of `PARTICLE_FIRE02` (a creature's `PARTICLE_FIRE09`, `+0xA9`). Its light is 300.
pub fn fire_circle() -> Fire {
    let orange = |alpha: f32| [1.0, 0.7, 0.5, alpha];
    Fire {
        texture: FIRE_SKIN_TEXTURES[0],
        layers: vec![Layer {
            life: -350.0,
            size: -70.0,
            ..Layer::default()
        }],
        blocks: vec![Block {
            size: spread(40.0, 20.0),
            offset: spread([0.0; 3], [7.0, 7.0, 0.0]),
            interval: 0.01,
            ..Block::default()
        }],
        ramps: vec![Ramp::new(
            orange(0.0),
            &[(orange(1.0), 32), (orange(1.0), 224)],
        )],
        blends: vec![Blend::Add],
        grid: 4,
        count: FIRE_CIRCLE_COUNT,
        light: 300.0,
        tinted: false,
    }
}

/// The fire spiral's flame's particles at most (`0x5A0C50`'s count for the type 790).
const FIRE_CIRCLE_COUNT: usize = 100;

/// The look of the purgatory 23, `cParticleSystem_hellFire` (820, its start `0x776860`, its run
/// `0x7762D0` and its draw `0x776390`; docs/re/effects.md, "The purgatory"), which goes with its
/// object for its seconds: its flames as the fire spiral's ([`fire_circle`]) but within 25 of its
/// place, a creature's white of `PARTICLE_FIRE09` (`+0xA9`); and while it burns its column of light
/// ([`hellfire_column`]) and its glow on the ground ([`HELLFIRE_GLOW_HALF`]) in its colour
/// ([`HELLFIRE_COLOURS`]). Its light is 200.
pub fn hellfire(hero: bool) -> Fire {
    let colour = |alpha: f32| {
        if hero {
            [1.0, 0.7, 0.5, alpha]
        } else {
            [1.0, 1.0, 1.0, alpha]
        }
    };
    Fire {
        texture: FIRE_SKIN_TEXTURES[usize::from(!hero)],
        blocks: vec![Block {
            size: spread(40.0, 20.0),
            offset: spread([0.0; 3], [25.0, 25.0, 0.0]),
            interval: 0.01,
            ..Block::default()
        }],
        ramps: vec![Ramp::new(
            colour(0.0),
            &[(colour(1.0), 32), (colour(1.0), 224)],
        )],
        count: HELLFIRE_COUNT,
        light: 200.0,
        ..fire_circle()
    }
}

/// The purgatory's particles at most (`0x5A0C50`'s count for the type 820).
const HELLFIRE_COUNT: usize = 100;
/// The purgatory's colour (`+0x24C0`), its column's and its glow's: a hero's (255, 192, 1), a
/// creature's (64, 64, 255).
pub const HELLFIRE_COLOURS: [[f32; 3]; 2] = [
    [1.0, 192.0 / 255.0, 1.0 / 255.0],
    [64.0 / 255.0, 64.0 / 255.0, 1.0],
];
/// Its glow (`0x7631E0`): `PARTICLE_GLOW03` of a half side of 75, flat on the ground, added whole.
pub const HELLFIRE_GLOW_TEXTURE: &str = "PARTICLE_GLOW03";
pub const HELLFIRE_GLOW_HALF: f32 = 75.0;

/// The purgatory's column of light (`+0x254C`, `+0x288C`; [`Column`]), of no texture, added by its
/// alpha (`SRCALPHA`, `ONE`) with no faces culled: a cylinder of 12 segments of the radius 7 and the
/// height 200, and its bands from 7 out to 17.5; all in its colour, of the alphas 160 at the
/// cylinder's foot and 128 at the bands', fading to nothing at their tops.
pub fn hellfire_column(hero: bool) -> Column {
    let colour = HELLFIRE_COLOURS[usize::from(!hero)];
    Column {
        radius: 7.0,
        height: 200.0,
        segments: 12,
        band: 17.5,
        foot: colour,
        top: colour,
        alphas: [160.0, 128.0],
        tops: [0.0, 0.0],
    }
}

/// The look of a meteor of the hail of meteors 26, `cParticleSystem_meteor` (823, its start
/// `0x786EF0`, its run `0x7866C0` and its draw `0x786AC0`; docs/re/effects.md, "The hail of
/// meteors"), at the place where it lands: after its wait its head falls from (−100, 0, 500) of
/// its frame at (160, 0, −800) a second, its rock ([`METEOR_MODEL`] in [`METEOR_TEXTURE`],
/// [`METEOR_SCALE`] times its size, turned at random) and a glow ([`METEOR_GLOW_HALF`]) at it,
/// leaving fire and smoke behind it; at the ground a quake of [`METEOR_QUAKE`] and a burst of 10
/// of them at its place, and its crater ([`METEOR_CRATERS`]) under them, which lies 15 s after
/// they are gone and fades over 3. No light.
#[derive(Clone, Debug, PartialEq)]
pub struct Meteor {
    pub system: System,
    pub phase: MeteorPhase,
    /// Its head's place in its frame (the blocks' offset, `+0x2924`).
    pub head: [f32; 3],
    /// Its rock's two angles (`+0x29B8`, `+0x29BC`), a whole turn at random each.
    pub turns: [f32; 2],
    /// Its crater's texture by the constructor's coin ([`METEOR_CRATERS`]).
    pub crater: usize,
}

/// A meteor's phase (`+0x29B4`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeteorPhase {
    /// Before it falls, its seconds left (its event's `+0x38` in milliseconds).
    Waiting(f32),
    Falling,
    /// Landed, its fire and smoke burning out.
    Burning,
    /// Its crater lying, its seconds left.
    Lying(f32),
    /// Its crater fading, its seconds left.
    Fading(f32),
    Done,
}

/// The meteor's rock, its texture and its scale (`0x6539D0` of 4), its particles' texture, its
/// craters' textures (`FX_KRATER02` where the coin is even), its crater's half side, its glow's
/// texture (the style 5), half side and colour (`0xFFB0750A`), and its quake.
pub const METEOR_MODEL: &str = "GIBS01.GRN";
pub const METEOR_TEXTURE: &str = "C_LAVA00";
pub const METEOR_SCALE: f32 = 4.0;
pub const METEOR_PARTICLES: &str = "PARTICLE_SMOKE04";
pub const METEOR_CRATERS: [&str; 2] = ["FX_KRATER02", "FX_KRATER01"];
pub const METEOR_CRATER_HALF: f32 = 50.0;
pub const METEOR_GLOW_TEXTURE: &str = "PARTICLE_GLOW03";
pub const METEOR_GLOW_HALF: f32 = 30.0;
pub const METEOR_GLOW_COLOUR: [f32; 4] = [176.0 / 255.0, 117.0 / 255.0, 10.0 / 255.0, 1.0];
pub const METEOR_QUAKE: u32 = 30;
/// Its particles at most (`0x5A0C50`'s count for the type 823).
const METEOR_COUNT: usize = 700;
/// Its head's start and velocity, how long its crater lies at the detail 2 (15 over the detail's
/// factor) and fades, and its burst's particles (the blocks' `+0x5C`).
const METEOR_FROM: [f32; 3] = [-100.0, 0.0, 500.0];
const METEOR_VELOCITY: [f32; 3] = [160.0, 0.0, -800.0];
const METEOR_LIES: f32 = 15.0;
const METEOR_FADES: f32 = 3.0;
const METEOR_BURST: usize = 10;

impl Meteor {
    /// A meteor that falls after `wait` seconds (`0x786EF0`); its crater's coin is rolled first, as
    /// its constructor rolls it (`0x786390`), then its rock's angles.
    pub fn new(wait: f32, dice: &mut impl Dice) -> Self {
        let crater = usize::from(dice.below(2) == 1);
        let turns = [
            share(dice) * std::f32::consts::TAU,
            share(dice) * std::f32::consts::TAU,
        ];
        Self {
            system: System::new(METEOR_COUNT),
            phase: MeteorPhase::Waiting(wait),
            head: METEOR_FROM,
            turns,
            crater,
        }
    }

    /// Its two layers (`+0x28AC`): the fire's, of the life −400 a second and the size −5, and the
    /// smoke's, of the life −150 and the size +8.
    pub fn layers() -> [Layer; 2] {
        [
            Layer {
                life: -400.0,
                size: -5.0,
                ..Layer::default()
            },
            Layer {
                life: -150.0,
                size: 8.0,
                ..Layer::default()
            },
        ]
    }

    /// Its ramps (`+0x20AC`, `+0x24AC`): the fire's from a grey of 86 to (134, 116, 20) at an
    /// alpha of 134 over 14 entries, to (233, 128, 47) at 152 over 13, to (152, 83, 31) at 230
    /// over 60, to nothing over 107 and 62; the smoke's from a grey of 43 to (120, 128, 128) over
    /// 24, to a grey of 60 at 203 over 44, to nothing over 188; both from an alpha of 255.
    pub fn ramps() -> [Ramp; 2] {
        let c = |r: f32, g: f32, b: f32, a: f32| [r / 256.0, g / 256.0, b / 256.0, a / 256.0];
        let none = [0.0; 4];
        [
            Ramp::new(
                c(86.0, 86.0, 86.0, 255.0),
                &[
                    (c(134.0, 116.0, 20.0, 134.0), 14),
                    (c(233.0, 128.0, 47.0, 152.0), 13),
                    (c(152.0, 83.0, 31.0, 230.0), 60),
                    (none, 107),
                    (none, 62),
                ],
            ),
            Ramp::new(
                c(43.0, 43.0, 43.0, 255.0),
                &[
                    (c(120.0, 128.0, 128.0, 255.0), 24),
                    (c(60.0, 60.0, 60.0, 203.0), 44),
                    (none, 188),
                ],
            ),
        ]
    }

    /// Its blocks as it falls (`+0x28EC`, `+0x2950`), at its head: the fire of the size 6 ± 1,
    /// drifting by up to 3 along each axis, and the smoke of the size 5 ± 1, its buoyancy 5 ± 10
    /// and its spin ± 3; both turned at random, one every 0.0012 s at the detail 2, the smoke one in
    /// five.
    fn falling() -> [Block; 2] {
        let pi = std::f32::consts::PI;
        [
            Block {
                size: spread(6.0, 1.0),
                angle: spread(pi, pi),
                velocity: spread([0.0; 3], [3.0; 3]),
                interval: 0.0012,
                ..Block::default()
            },
            Block {
                buoyancy: spread(5.0, 10.0),
                size: spread(5.0, 1.0),
                angle: spread(pi, pi),
                spin: spread(0.0, 3.0),
                interval: 0.0012,
                ..Block::default()
            },
        ]
    }

    /// Its blocks as it lands (`0x7867E5`), at its place: the fire of the size 15 ± 5 within 3 of
    /// it, the smoke of the size 15 ± 5 within 2, its buoyancy −5 ± 2; both flying off at
    /// (15, 15, 30) ± 5 along each axis, turned at random.
    fn burst() -> [Block; 2] {
        let pi = std::f32::consts::PI;
        let block = |offset: f32| Block {
            size: spread(15.0, 5.0),
            angle: spread(pi, pi),
            velocity: spread([15.0, 15.0, 30.0], [5.0; 3]),
            offset: spread([0.0; 3], [offset; 3]),
            ..Block::default()
        };
        [
            block(3.0),
            Block {
                buoyancy: spread(-5.0, 2.0),
                ..block(2.0)
            },
        ]
    }

    /// A run of `dt` seconds (`0x7866C0`); whether it lands in it, when its quake shakes the
    /// screen.
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) -> bool {
        let layers = Self::layers();
        match self.phase {
            MeteorPhase::Waiting(left) => {
                self.phase = if left - dt <= 0.0 {
                    MeteorPhase::Falling
                } else {
                    MeteorPhase::Waiting(left - dt)
                };
                false
            }
            MeteorPhase::Falling => {
                self.system.update(dt, &layers);
                let from = self.head;
                for (axis, speed) in self.head.iter_mut().zip(METEOR_VELOCITY) {
                    *axis += speed * dt;
                }
                self.system.emit_along(
                    &Self::falling(),
                    &layers,
                    METEOR_COUNT,
                    (from, self.head),
                    dt,
                    dice,
                );
                if self.head[2] > 0.0 {
                    return false;
                }
                let blocks = Self::burst();
                for _ in 0..METEOR_BURST {
                    let layer = pick(&blocks, dice);
                    if let Some(block) = blocks.get(layer)
                        && self.system.particles.len() < METEOR_COUNT
                    {
                        self.system.particles.push(born(block, layer, dice));
                    }
                }
                self.phase = MeteorPhase::Burning;
                true
            }
            MeteorPhase::Burning => {
                self.system.update(dt, &layers);
                if self.system.particles.is_empty() {
                    self.phase = MeteorPhase::Lying(METEOR_LIES);
                }
                false
            }
            MeteorPhase::Lying(left) => {
                self.phase = if left - dt <= 0.0 {
                    MeteorPhase::Fading(METEOR_FADES)
                } else {
                    MeteorPhase::Lying(left - dt)
                };
                false
            }
            MeteorPhase::Fading(left) => {
                self.phase = if left - dt <= 0.0 {
                    MeteorPhase::Done
                } else {
                    MeteorPhase::Fading(left - dt)
                };
                false
            }
            MeteorPhase::Done => false,
        }
    }

    /// Whether its rock and its glow show: while it falls.
    pub fn falling_now(&self) -> bool {
        self.phase == MeteorPhase::Falling
    }

    /// Its crater's alpha, from its landing: whole, then `⌊85 t⌋ / 255` over its last 3 s, `t`
    /// its seconds left; none before it lands or once it is gone.
    pub fn crater_alpha(&self) -> Option<f32> {
        match self.phase {
            MeteorPhase::Burning | MeteorPhase::Lying(_) => Some(1.0),
            MeteorPhase::Fading(left) => Some((85.0 * left).floor().clamp(0.0, 255.0) / 255.0),
            MeteorPhase::Waiting(_) | MeteorPhase::Falling | MeteorPhase::Done => None,
        }
    }

    /// Whether it ended: its crater faded out.
    pub fn done(&self) -> bool {
        self.phase == MeteorPhase::Done
    }
}

/// The colours of the grounds by their nature (the cell's `+0x1F` high nibble), in the
/// renderer's table at `+0x9DC8` (`0x418EF3`, read by `0x41BA00`): the dust that the gust raises
/// takes the colour of the ground its bearer stands on; the nature 0's entry is left 0, and a
/// place of no cell takes (128, 128, 128) (`0x524DDC`).
pub const GROUND_COLOURS: [u32; 16] = [
    0x0000_0000,
    0xFFEF_EFC0,
    0xFFE0_E0EF,
    0xFF30_4010,
    0xFF30_4010,
    0xFF86_9A26,
    0xFFEF_9019,
    0xFF80_8080,
    0xFFC0_C0C0,
    0xFF19_C1EF,
    0xFF19_4AEF,
    0xFF80_8080,
    0xFF80_8080,
    0xFFB5_B3B3,
    0xFFE3_752B,
    0xFFEC_CB30,
];
pub const NO_GROUND_COLOUR: u32 = 0xFF80_8080;

/// The colour of an ARGB word, its red, green, blue and alpha in 0 to 1.
pub fn argb(word: u32) -> [f32; 4] {
    let [a, r, g, b] = word.to_be_bytes().map(|byte| f32::from(byte) / 255.0);
    [r, g, b, a]
}

/// The look of the gust 28, `cParticleSystem_twister` (826, its start `0x788D90`, its run
/// `0x7888E0` and its draw `0x788B80`; docs/re/effects.md, "The gust"), on its bearer for its
/// seconds: 250 puffs of `PARTICLE_CLOUD01` circling it in the colour of the ground under it,
/// laid over by their alpha and tinted by the light at it, their column rising to 100 over its
/// first seconds, and once its seconds are over spinning faster and climbing for 2 s more.
#[derive(Clone, Debug, PartialEq)]
pub struct Twister {
    pub puffs: Vec<Puff>,
    /// Its column's height (`+0x20AC`), its phase (`+0x20B4`: 0 rising, 1 standing, 2 going),
    /// its clock and its seconds, and how far it has gone in its last phase (`+0x20B0`).
    pub height: f32,
    phase: u8,
    clock: f32,
    seconds: f32,
    gone: f32,
}

/// A puff of the gust: its angle about its bearer, its radius, its share of the column's height,
/// its turn's speed a second, and its quad's angle in the screen's plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Puff {
    pub angle: f32,
    pub radius: f32,
    pub share: f32,
    pub spin: f32,
    pub turn: f32,
}

impl Puff {
    /// Its place about its bearer at the column's height `height`, in the systems' frame, the
    /// third coordinate up, and its quad's half side: the radius widening by 0.6 of its share,
    /// the half side `7 (2 share + 1)`.
    pub fn at(&self, height: f32) -> ([f32; 3], f32) {
        let radius = (self.share * 0.6 + 1.0) * self.radius;
        (
            [
                self.angle.cos() * radius,
                self.angle.sin() * radius,
                height * self.share,
            ],
            (self.share * 2.0 + 1.0) * 7.0,
        )
    }
}

/// The gust's puffs (`0x5A0C50`'s count for the type 826).
const TWISTER_PUFFS: usize = 250;

impl Twister {
    /// A gust of `seconds`, its puffs rolled at its first run: their angles of a whole turn,
    /// their radii 30 to 35, their shares the cube of a roll of 0 to 1, their speeds 0.3 to 4
    /// radians a second.
    pub fn new(seconds: f32, dice: &mut impl Dice) -> Self {
        let roll = |dice: &mut _| (signed(dice) + 1.0) * 0.5;
        let tau = std::f32::consts::TAU;
        let puffs = (0..TWISTER_PUFFS)
            .map(|_| {
                let angle = roll(dice) * tau;
                let radius = roll(dice) * 5.0 + 30.0;
                let share = roll(dice).powi(3);
                let spin = roll(dice) * 3.7 + 0.3;
                let turn = roll(dice) * tau;
                Puff {
                    angle,
                    radius,
                    share,
                    spin,
                    turn,
                }
            })
            .collect();
        Self {
            puffs,
            height: 0.0,
            phase: 0,
            clock: 0.0,
            seconds,
            gone: 1.0,
        }
    }

    /// A run of `dt` seconds (`0x7888E0`): each puff on about its bearer by its speed, faster by
    /// 6 a second once it goes; its column rising at 30 a second to 100, standing to its
    /// seconds, then climbing at 50 a second for 2 s.
    pub fn update(&mut self, dt: f32) {
        self.clock += dt;
        let tau = std::f32::consts::TAU;
        for puff in &mut self.puffs {
            puff.angle += dt * puff.spin;
            if puff.angle > tau {
                puff.angle -= tau;
            }
            if self.phase == 2 {
                puff.spin += dt * 6.0;
            }
        }
        match self.phase {
            0 => {
                self.height += dt * 30.0;
                if self.height > 100.0 {
                    self.height = 100.0;
                    self.phase = 1;
                }
            }
            1 if self.clock > self.seconds => self.phase = 2,
            2 => {
                self.gone += dt * 0.5;
                self.height += dt * 50.0;
            }
            _ => {}
        }
    }

    /// Whether it ended: 2 s after its seconds.
    pub fn done(&self) -> bool {
        self.gone > 2.0
    }
}

/// The look of the battle fog, `cParticleSystem_battlefog` (868, its start `0x7709F0`, its run
/// `0x770700` and its draw `0x7707D0`; docs/re/effects.md, "The battle fog"), for its seconds: a
/// puff of `PARTICLE_SMOKE03` every 0.3 s within (30, 80) of its place and 50 above it, give or
/// take 50, of a half side of 60 ± 20, turned at random and spinning by up to 0.3 radians a
/// second, a frame of the 2 × 2 at random, living 5.1 s in its ramp, laid over by its alpha and
/// tinted by the light at it; its seconds over, its puffs live out. Its ramp's start by its
/// event's variant (`+0x3C`): 1 a warm grey, 2 a cool one, else a grey of 120 at no alpha.
pub fn battle_fog(variant: u32) -> Fire {
    let c = |r: u32, g: u32, b: u32, a: u32| {
        [r, g, b, a].map(|v| f32::from(u16::try_from(v).unwrap_or(0)) / 256.0)
    };
    let start = match variant {
        1 => c(193, 183, 173, 150),
        2 => c(173, 183, 193, 150),
        _ => c(120, 120, 120, 0),
    };
    let pi = std::f32::consts::PI;
    Fire {
        texture: "PARTICLE_SMOKE03",
        layers: vec![Layer {
            life: -50.0,
            ..Layer::default()
        }],
        blocks: vec![Block {
            buoyancy: spread(0.0, 3.0),
            size: spread(60.0, 20.0),
            angle: spread(pi, pi),
            spin: spread(0.0, 0.3),
            offset: spread([0.0, 0.0, 50.0], [30.0, 80.0, 50.0]),
            interval: 0.3,
            frames: 4,
            ..Block::default()
        }],
        ramps: vec![Ramp::new(
            start,
            &[(c(193, 193, 193, 150), 64), (c(150, 150, 150, 0), 192)],
        )],
        blends: vec![Blend::Over],
        grid: 2,
        count: BATTLE_FOG_COUNT,
        light: 0.0,
        tinted: true,
    }
}

/// The battle fog's particles (`0x5A0C50`'s count for the type 868).
const BATTLE_FOG_COUNT: usize = 100;

/// The look of a rune of the banishing circle 16, `cParticleSystem_groundsymbol` (805, its start
/// `0x7825E0`, its run `0x781D60` and its draw `0x781DC0`; docs/re/effects.md, "The banishing
/// circle's runes"), standing for its seconds: its rune `runeshield<n>` facing the camera, laid
/// over by its alpha, with its two glows added over it, its first pulsing by its clock; light
/// bars of `runeshield_gradient` rising above it in a warm colour; and tiny sparks rising.
#[derive(Clone, Debug, PartialEq)]
pub struct Rune {
    pub sparks: System,
    seconds: f32,
}

/// A rune's quads facing the camera, in its frame's screen's right and up about its place: the
/// rune and its glows (`+0x2144`), the bar (`+0x21C4`) and the three rays (`+0x2244`, `+0x22C4`,
/// `+0x2344`), each its left, right, bottom and top, and its texture's width across.
pub const RUNE_QUAD: [f32; 4] = [-32.0, 32.0, -16.0, 16.0];
pub const RUNE_BAR: [f32; 4] = [-6.0, 6.0, 0.0, 64.0];
pub const RUNE_RAYS: [([f32; 4], f32); 3] = [
    ([-8.0, 8.0, -4.0, 48.0], 1.0),
    ([-8.0, 8.0, -4.0, 48.0], 2.0),
    ([-8.0, 8.0, -4.0, 64.0], 1.0),
];
/// The rune's light (`+0x9C`), and its sparks' colour, (128, 64, 0) added whole (the flags
/// `0x13`).
pub const RUNE_LIGHT: f32 = 50.0;
pub const RUNE_SPARK_COLOUR: [f32; 4] = [128.0 / 255.0, 64.0 / 255.0, 0.0, 1.0];
/// The rune ring's place about its caster (`0x522DE3`): the rune `i` at the angle
/// `i·π/4 + π − 0.5`, 200 off, along the world's `sin` and `cos`.
pub const RUNES_APART: f32 = std::f32::consts::FRAC_PI_4;
pub const RUNES_START: f32 = std::f32::consts::PI - 0.5;

/// A rune's colours at its clock `t` (`0x781DC0`): its first glow's grey
/// `⌊128 (sin t + 1)⌋`, its bar's `(2c, c, 0)` for `c = ⌊16 (sin t + 1) + 32⌋`, and its rays'
/// the same of `c = ⌊4 (sin t + 1) sin²(k t) + 8⌋`, `k` 2.1 for the first ray and 1.5 for the
/// others, each a share of 255.
pub fn rune_colours(t: f32) -> ([f32; 4], [f32; 4], [[f32; 4]; 3]) {
    let s = t.sin() + 1.0;
    let byte = |value: f32| value.trunc().min(255.0) / 255.0;
    let grey = byte(128.0 * s);
    let warm = |c: f32| [byte(2.0 * c.trunc()), byte(c), 0.0, 1.0];
    let ray = |k: f32| warm(4.0 * s * (k * t).sin().powi(2) + 8.0);
    (
        [grey, grey, grey, 1.0],
        warm(16.0 * s + 32.0),
        [ray(2.1), ray(1.5), ray(1.5)],
    )
}

/// Its sparks at most (`0x5A0C50`'s count for the type 805).
const RUNE_SPARKS: usize = 50;

/// The look of an object of the ring of frost 35, `cParticleSystem_circleofice` (840, its start
/// `0x78E070`, its run `0x78DC10` and its draw `0x78DD00`; docs/re/effects.md, "The ring of
/// frost"), where it stands for its seconds: for its first 1.5 s a strike of light flickering
/// down on its place ([`ICE_STRIKE_QUAD`]), its frame of eight rolled anew every 0.07 s, over a
/// glow; then a burst of sparks falling about it, and its iceberg ([`ICE_MODELS`], turned at
/// random) standing on a wider glow to its end. Its light is 150.
#[derive(Clone, Debug, PartialEq)]
pub struct Iceberg {
    pub sparks: System,
    /// Its clock (`+0x20A0`).
    pub clock: f32,
    /// Its strike's frame (`+0x20B4`, 0 to 7): its texture the half of it, its half of the
    /// texture's breadth the rest ([`Iceberg::strike`]).
    pub frame: u8,
    /// Its iceberg's angle about the upright (`+0x20AC`) and its model of the four (`+0x21C4`).
    pub turn: f32,
    pub model: usize,
    seconds: f32,
    flicker: f32,
    stands: bool,
}

/// The strikes' textures (`+0x21D4`, two frames each), their quad (`+0x2144`: from −60 to 60
/// across and −40 to 400 up, the texture's `u` up it and its `v` across), the glow's texture and
/// its half sides while it strikes and while it stands, the colours of a hero's and a
/// creature's (`+0x20B8`, white and `0xFFFF4040`), the icebergs, their textures (a hero's and a
/// creature's), the sparks' texture and the light.
pub const ICE_STRIKES: [&str; 4] = [
    "FX_LIGHTSTRIKE01",
    "FX_LIGHTSTRIKE02",
    "FX_LIGHTSTRIKE03",
    "FX_LIGHTSTRIKE04",
];
pub const ICE_STRIKE_QUAD: [f32; 4] = [-60.0, 60.0, -40.0, 400.0];
pub const ICE_GLOW_TEXTURE: &str = "PARTICLE_GLOW03";
pub const ICE_GLOW_HALVES: [f32; 2] = [25.0, 50.0];
pub const ICE_COLOURS: [[f32; 4]; 2] = [[1.0; 4], [1.0, 64.0 / 255.0, 64.0 / 255.0, 1.0]];
pub const ICE_MODELS: [&str; 4] = [
    "ICEBERG01.GRN",
    "ICEBERG02.GRN",
    "ICEBERG03.GRN",
    "ICEBERG04.GRN",
];
pub const ICE_TEXTURES: [&str; 2] = ["ICEBERG", "ICEBERG2"];
pub const ICE_SPARK_TEXTURE: &str = "PARTICLE_SPARK02";
pub const ICE_LIGHT: f32 = 150.0;
/// How long it strikes, how often its strike's frame changes, and its sparks (`0x5A0C50`'s count
/// for the type 840, all of them in its burst).
const ICE_STRIKES_FOR: f32 = 1.5;
const ICE_FLICKER: f32 = 0.07;
const ICE_SPARKS: usize = 50;

impl Iceberg {
    /// An object of the ring of frost for `seconds` (`0x78D870`, `0x78E070`): its iceberg's model
    /// rolled first, as its constructor rolls it, then its angle.
    pub fn new(seconds: f32, dice: &mut impl Dice) -> Self {
        let model = usize::try_from(dice.below(4)).unwrap_or(0);
        let turn = share(dice) * std::f32::consts::TAU;
        Self {
            sparks: System::new(ICE_SPARKS),
            clock: 0.0,
            frame: 0,
            turn,
            model,
            seconds,
            flicker: 0.0,
            stands: false,
        }
    }

    /// Its sparks' layer (`+0x20BC`): the life −150 a second.
    pub fn layer() -> Layer {
        Layer {
            life: -150.0,
            ..Layer::default()
        }
    }

    /// Its sparks' ramp (`+0x21FC`): the grey of its entry, from white at their birth to black.
    pub fn ramp() -> Ramp {
        Ramp(std::array::from_fn(|k| {
            let grey = f32::from(u8::try_from(k).unwrap_or(u8::MAX)) / 255.0;
            [grey, grey, grey, 0.0]
        }))
    }

    /// Its sparks' block (`+0x20DC`): the size 10, 30 up within 30 of its place along each axis,
    /// flying at up to 10 along each, falling by 140 ± 20 a second.
    fn burst() -> Block {
        Block {
            buoyancy: spread(140.0, 20.0),
            size: spread(10.0, 0.0),
            velocity: spread([0.0; 3], [10.0; 3]),
            offset: spread([0.0, 0.0, 30.0], [30.0; 3]),
            ..Block::default()
        }
    }

    /// A run of `dt` seconds (`0x78DC10`): striking, its frame rolled anew every 0.07 s, and past
    /// 1.5 s its burst; standing, its sparks age, bouncing on the ground.
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) {
        self.clock += dt;
        if self.stands {
            self.sparks.update_over(dt, &[Self::layer()], Some(0.0));
            return;
        }
        if self.clock > ICE_STRIKES_FOR {
            let block = Self::burst();
            for _ in 0..ICE_SPARKS {
                self.sparks.particles.push(born(&block, 0, dice));
            }
            self.stands = true;
        }
        self.flicker += dt;
        if self.flicker > ICE_FLICKER {
            self.flicker -= ICE_FLICKER;
            self.frame = u8::try_from(dice.below(8)).unwrap_or(0);
        }
    }

    /// Whether it strikes yet, before its iceberg stands.
    pub fn striking(&self) -> bool {
        !self.stands
    }

    /// Its strike's texture (of [`ICE_STRIKES`]) and the texture's breadth it takes, from its
    /// left edge to its right: an even frame's from a half to nothing, an odd frame's from all of
    /// it to a half.
    pub fn strike(&self) -> (usize, [f32; 2]) {
        let texture = usize::from(self.frame / 2);
        if self.frame.is_multiple_of(2) {
            (texture, [0.5, 0.0])
        } else {
            (texture, [1.0, 0.5])
        }
    }

    /// Whether it ended: standing past its seconds.
    pub fn done(&self) -> bool {
        self.stands && self.clock > self.seconds
    }
}

impl Rune {
    /// A rune of `seconds`.
    pub fn new(seconds: f32) -> Self {
        Self {
            sparks: System::new(RUNE_SPARKS),
            seconds,
        }
    }

    /// A run of `dt` seconds (`0x781D60`): its sparks aged and a new one every 0.05 s, within 6
    /// of its place along x and y, rising at 30 ± 5 and lifted on at a buoyancy of −200, of a half
    /// side of 1 ± 0.25, living 0.51 s.
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) {
        let layers = [Layer {
            life: -500.0,
            ..Layer::default()
        }];
        let block = Block {
            buoyancy: spread(-200.0, 0.0),
            size: spread(1.0, 0.25),
            velocity: spread([0.0, 0.0, 30.0], [0.0, 0.0, 5.0]),
            offset: spread([0.0; 3], [6.0, 6.0, 0.0]),
            interval: 0.05,
            ..Block::default()
        };
        self.sparks.update(dt, &layers);
        self.sparks.emit(&[block], &layers, RUNE_SPARKS, dice);
    }

    /// Whether it ended: past its seconds.
    pub fn done(&self) -> bool {
        self.sparks.clock > self.seconds
    }
}

/// The breath's glow at its place (`0x7AE4F0`): `PARTICLE_GLOW03` of a half side of 20, white,
/// added whole (the style 5, `ONE`, `ONE`), its depth tested.
pub const WALLOFFIRE_GLOW_TEXTURE: &str = "PARTICLE_GLOW03";
pub const WALLOFFIRE_GLOW_HALF: f32 = 20.0;

/// The look of a fire of the breath's wall, `cParticleSystem_walloffire_line`
/// (`FX_WALLOFFIRE_LINE`, 1007, its start `0x7AEC60` and its run `0x7AEB80`): a flame of
/// `PARTICLE_FIRE02` every 0.08 s within 40 of its place on the ground along each axis, of a half
/// side of 40 ± 10 shrinking by 17 a second, standing on its foot, living 2.55 s and playing its
/// 4 × 4 frames once over it, in a grey of 199 that comes up over its first 10 entries and goes
/// over its last 46, added whole (`0x761B00`'s flags `0x99`: `ONE`, `ONE`), untinted; it makes
/// them for the seconds of its event (`+0x48`, 10: the art's [`crate::arts::WALL_BURNS`]) and
/// ends with its last. Its light is 200.
pub fn walloffire_line() -> Fire {
    let grey = |g: f32| [g / 255.0, g / 255.0, g / 255.0, 1.0];
    Fire {
        texture: "particle_fire02",
        layers: vec![Layer {
            life: -100.0,
            size: -17.0,
            ..Layer::default()
        }],
        blocks: vec![Block {
            size: spread(40.0, 10.0),
            offset: spread([0.0; 3], [40.0, 40.0, 0.0]),
            interval: 0.08,
            ..Block::default()
        }],
        // The ramp's alpha is 0 in the original's, which its source `ONE` does not take.
        ramps: vec![Ramp::new(
            grey(0.0),
            &[(grey(199.0), 10), (grey(199.0), 200), (grey(0.0), 46)],
        )],
        blends: vec![Blend::Add],
        grid: 4,
        count: WALLOFFIRE_COUNT,
        light: 200.0,
        tinted: false,
    }
}

/// The combat arts' hit sparks (docs/re/effects.md, "The hit sparks"): the five classes of the
/// heroes' strikes, `FX_GLAD_SM_HIT` (842), `FX_SERA_SM_HIT` (843), `FX_ELVE_SM_HIT` (860),
/// `FX_VAMP_SM_HIT` (862) and `FX_DARK_SM_HIT` (864), which differ in their count, their colour and
/// one texture alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sparks {
    Glad,
    Sera,
    Elve,
    Vamp,
    Dark,
}

/// Where a strike's sparks burst: at the struck creature's neck (bone 3, `Bip01 Neck`), else its
/// root (bone 0, where its model has no neck, `0x44CB80`); at its root (bone 0); or at the striker's
/// own foot (the bones 36, the right, and 37, the left), as the kicks make them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SparksAt {
    Neck,
    Root,
    RightFoot,
    LeftFoot,
}

/// The sparks that a strike of the art `art` makes and where (the heroes' executors' sites,
/// `0x4FF0B0` to `0x5192E0`, the FX events `0x42`, `0x44`, `0x55`, `0x57`, `0x59`), whether it lands
/// or not: the `Attacke`s, `Hardhit`s and `Rundum`s of every class, the kicks, the Gladiator's
/// 1012 and 1014, the Seraphim's 1017, 1020 and 1054, the Dwarf's 1084 and the Daemon's 1091;
/// none for the other arts.
pub fn sparks_of(art: u16) -> Option<(Sparks, SparksAt)> {
    let (sparks, at) = match art {
        // The Gladiator's, the Dwarf's and the Daemon's, and the Vampiress's kick.
        1012 | 1014 | 1057..=1059 | 1078..=1080 | 1084 | 1091 | 1092 => {
            (Sparks::Glad, SparksAt::Neck)
        }
        1060 | 1041 | 1076 => (Sparks::Glad, SparksAt::RightFoot),
        // The Seraphim's, and the Mage's `Hardhit` and `Rundum`.
        1000 | 1001 | 1020 | 1054 | 1069..=1071 => (Sparks::Sera, SparksAt::Neck),
        1072 => (Sparks::Sera, SparksAt::LeftFoot),
        1017 => (Sparks::Sera, SparksAt::Root),
        // The Wood Elf's, and the Mage's `Attacke`.
        1002 | 1061..=1063 => (Sparks::Elve, SparksAt::Neck),
        1038..=1040 | 1073..=1075 => (Sparks::Vamp, SparksAt::Neck),
        1065..=1068 => (Sparks::Dark, SparksAt::Neck),
        _ => return None,
    };
    Some((sparks, at))
}

impl Sparks {
    /// Its particles (the class's whole count, burst at once).
    pub fn count(self) -> usize {
        match self {
            Self::Glad => 600,
            Self::Sera => 151,
            Self::Elve => 152,
            Self::Vamp => 250,
            Self::Dark => 153,
        }
    }

    /// Its texture: `PARTICLE_SPARK06`, the Vampiress's `PARTICLE_GLOW07`.
    pub fn texture(self) -> &'static str {
        match self {
            Self::Vamp => "PARTICLE_GLOW07",
            _ => "PARTICLE_SPARK06",
        }
    }

    /// Its ramp's entry `i`, from the death (0) to the birth (255), with `h = ⌊i/2⌋`: the
    /// Gladiator's and the Wood Elf's (h, i, h) green, the Seraphim's (h, h, i) blue, the
    /// Vampiress's (i, h, h) red, the Dark Elf's (i, i, h) yellow; added whole, as the original's
    /// `ONE`, `ONE` adds it whatever its alpha (`0x78F323`, `0x79054B`, `0x791530`, `0x793A10`,
    /// `0x794E47`).
    pub fn ramp(self) -> Ramp {
        Ramp(std::array::from_fn(|i| {
            let full = u8::try_from(i).unwrap_or(u8::MAX);
            let (i, h) = (f32::from(full) / 255.0, f32::from(full / 2) / 255.0);
            match self {
                Self::Glad | Self::Elve => [h, i, h, 1.0],
                Self::Sera => [h, h, i, 1.0],
                Self::Vamp => [i, h, h, 1.0],
                Self::Dark => [i, i, h, 1.0],
            }
        }))
    }
}

/// The hit sparks' light, at the striker's place (`+0x9C`).
pub const SPARKS_LIGHT: f32 = 200.0;
/// How long the sparks live: their life's rate of −200 a second.
pub const SPARKS_LIFE: f32 = 255.0 / 200.0;

/// A strike's sparks of the class `sparks` (the default of the five classes' starts, `0x78F180`,
/// `0x790340`, `0x791410`, `0x7938D0`, `0x794CE0`): all at once about the point it bursts at, each
/// of a buoyancy of 180 ± 40, so that it falls, a size of 6 ± 1 shrinking by 4 a second, the
/// velocity (0, 0, 60) ± 40 and the place ± 10 on each axis, living 1.275 s, drawn added and not
/// turned (the flags `0x19`) in its class's ramp. Its light ([`SPARKS_LIGHT`]) is not the burst's:
/// it lies at the striker.
pub fn hit_sparks(sparks: Sparks, dice: &mut impl Dice) -> Burst {
    let block = Block {
        buoyancy: spread(180.0, 40.0),
        size: spread(6.0, 1.0),
        velocity: spread([0.0, 0.0, 60.0], [40.0; 3]),
        offset: spread([0.0; 3], [10.0; 3]),
        ..Block::default()
    };
    let particles = (0..sparks.count())
        .map(|_| Particle {
            place: spread_3(block.offset, dice),
            velocity: spread_3(block.velocity, dice),
            buoyancy: spread_f32(block.buoyancy, dice),
            size: spread_f32(block.size, dice),
            life: 255.0,
            angle: 0.0,
            spin: 0.0,
            layer: 0,
            frame: 0,
        })
        .collect();
    Burst {
        texture: sparks.texture(),
        grid: 1,
        blend: Blend::Add,
        layer: Layer {
            life: -200.0,
            size: -4.0,
            ..Layer::default()
        },
        ramp: sparks.ramp(),
        particles,
        ground: None,
        light: 0.0,
        tinted: false,
        stream: None,
    }
}

/// The blood fountain's texture and the sounds of its start (`BLEEDING01` and `02`, `0x77BD60`).
pub const BLOOD_TEXTURE: &str = "particle_dots03";
pub const BLOOD_SOUNDS: [u32; 2] = [710, 711];
/// Its particles at most (`0x5A0C50`).
const BLOOD_COUNT: usize = 400;
/// The blood's speed out of its bone at the fountain's whole strength, how long that strength
/// holds and how fast it falls after (`0x77BA90`).
const BLOOD_SPEED: f32 = 35.0;
const BLOOD_HOLDS: f32 = 5.0;
const BLOOD_FALLS: f32 = 0.5;
/// The factor of `rand()` / 32767 whose truncation chooses its sound (`0x897D58`, just below 2).
const BLOOD_SOUND_FACTOR: f32 = f32::from_bits(0x3FFF_DF3B);

/// The bones that a creature's blood fountain may spurt from, by the part that its making rolls
/// (`+0x574`, `rand() % 6`, `0x52DFA1`), of which the sixth makes none (`0x4E87E0`): the head, the
/// upper arms and the thighs (the bones 20, 43, 44, 6 and 7).
pub const BLOOD_BONES: [&str; 5] = [
    "Bip01 Head",
    "Bip01 L UpperArm",
    "Bip01 R UpperArm",
    "Bip01 L Thigh",
    "Bip01 R Thigh",
];
/// The parts that a making rolls among.
pub const BLOOD_PARTS: u32 = 6;

/// The types that bleed no blood whatever their class and gib set (`0x4281F0`), the dragons
/// among them.
const BLOODLESS: [u32; 23] = [
    1, 2, 3, 4, 5, 6, 7, 46, 52, 53, 55, 63, 74, 88, 100, 101, 102, 103, 125, 126, 130, 700, 706,
];

/// Whether a creature of the type `kind`, of the `Creature.pak` class `class` and the gib set
/// `set` (`items.pak +0x6F`), bleeds (`0x4281F0`): no undead (the class 5), no set of bones (3) nor
/// of none (4), none of [`BLOODLESS`].
pub fn bleeds(kind: u32, class: u8, set: u8) -> bool {
    class != 5 && !matches!(set, 3 | 4) && !BLOODLESS.contains(&kind)
}

/// Whether a creature that bleeds spurts blood as it dies, by a roll of `rand() % 101` on `dice`
/// below 10 (`0x53F38F`).
pub fn spurts_at_death(dice: &mut impl Dice) -> bool {
    dice.below(101) < 10
}

/// The sound of a blood fountain's start, by a roll of `rand()` on `dice` (`0x77BFB3`): each of
/// [`BLOOD_SOUNDS`] at about even odds.
pub fn blood_sound(dice: &mut impl Dice) -> u32 {
    let roll = f32::from(u16::try_from(dice.below(0x8000)).unwrap_or(0)) / 32_767.0;
    if roll * BLOOD_SOUND_FACTOR < 1.0 {
        BLOOD_SOUNDS[0]
    } else {
        BLOOD_SOUNDS[1]
    }
}

/// A creature's blood fountain, `cParticleSystem_bloodfontain` (`FX_BLOODFONTAIN`, 908, its start
/// `0x77BD60` and its update `0x77BA90`; docs/re/effects.md, "The blood fountain"), which keeps to
/// its bone: a drop of `particle_dots03` every 0.005 s at the bone, within 1 of it on each axis,
/// leaving along the bone's axis at 35 times the fountain's strength ± 3 on each axis, of a
/// buoyancy of 45 ± 10, so that it falls, a size of 5 ± 3 that grows by 12 a second and an angle
/// of π ± π, living 1.42 s (−180) unless it reaches the ground first, where it dies; its strength
/// holds at 1 for 5 s, then falls by 0.5 a second, and the fountain ends as it reaches 0. Its
/// drops stay in the world, laid over the frame by their alpha, turned, of their ramp's colour and
/// tinted by the light at them (the flags `0x4C`).
#[derive(Clone, Debug, PartialEq)]
pub struct BloodFountain {
    pub system: System,
    pub strength: f32,
    pub ramp: Ramp,
}

impl BloodFountain {
    /// A fountain of the blood of the gib set `set`.
    pub fn new(set: u8) -> Self {
        Self {
            system: System::new(BLOOD_COUNT),
            strength: 1.0,
            ramp: Self::blood(set),
        }
    }

    /// The blood's ramp by the gib set (the FX event's `+0x3C`, `0x427630`): from its colour at
    /// the whole alpha over 124 entries to a darker one at 165/256, over 61 to a darker still at
    /// 140/256 and over 71 to nothing; red, green for the set 1, near black for 2.
    pub fn blood(set: u8) -> Ramp {
        let [first, second, third]: [[u8; 2]; 3] = match set {
            1 => [[12, 255], [3, 165], [0, 138]],
            2 => [[12, 12], [3, 3], [0, 0]],
            _ => [[255, 12], [165, 3], [138, 0]],
        };
        let colour = |[red, green]: [u8; 2], alpha: f32| {
            [f32::from(red) / 256.0, f32::from(green) / 256.0, 0.0, alpha]
        };
        Ramp::new(
            colour(first, 1.0),
            &[
                (colour(second, 165.0 / 256.0), 124),
                (colour(third, 140.0 / 256.0), 61),
                ([0.0; 4], 71),
            ],
        )
    }

    /// Its layer: the life −180 a second, the size +12.
    pub fn layer() -> Layer {
        Layer {
            life: -180.0,
            size: 12.0,
            ..Layer::default()
        }
    }

    /// Its block at the detail 2, the drops' velocity `velocity`.
    fn block(velocity: [f32; 3]) -> Block {
        Block {
            buoyancy: spread(45.0, 10.0),
            size: spread(5.0, 3.0),
            angle: spread(std::f32::consts::PI, std::f32::consts::PI),
            velocity: spread(velocity, [3.0; 3]),
            offset: spread([0.0; 3], [1.0; 3]),
            interval: 0.005,
            ..Block::default()
        }
    }

    /// A run of `dt` seconds, its bone gone from `from` to `to` in its frame and its axis
    /// `along` (of length 1), the ground at the height `ground` (`0x77BA90`): its strength falls
    /// once 5 s have run, its drops age and die on the ground, and new ones leave along the way.
    pub fn update(
        &mut self,
        dt: f32,
        (from, to): ([f32; 3], [f32; 3]),
        (along, ground): ([f32; 3], f32),
        dice: &mut impl Dice,
    ) {
        if self.system.clock + dt > BLOOD_HOLDS {
            self.strength = (self.strength - dt * BLOOD_FALLS).max(0.0);
        }
        let velocity = along.map(|axis| axis * self.strength * BLOOD_SPEED);
        let layers = [Self::layer()];
        self.system.update_to_ground(dt, &layers, ground);
        self.system.emit_along(
            &[Self::block(velocity)],
            &layers,
            BLOOD_COUNT,
            (from, to),
            dt,
            dice,
        );
    }

    /// Whether it ended: its strength gone (`+0x20A8` 0), its drops with it.
    pub fn done(&self) -> bool {
        self.strength <= 0.0
    }
}

/// The dragons' breath's textures by its variant (`0x77AA20`), its grid, how long it waits before
/// it burns and when it ends, its flames' speed and its light while it burns (`0x77AAE0`).
pub const DRAGON_FIRE_TEXTURES: [&str; 3] =
    ["PARTICLE_FIRE01", "PARTICLE_FIRE06", "PARTICLE_FIRE07"];
pub const DRAGON_FIRE_GRID: u8 = 4;
const DRAGON_FIRE_WAITS: f32 = 1.5;
const DRAGON_FIRE_ENDS: f32 = 4.0;
const DRAGON_FIRE_SPEED: f32 = 1000.0;
pub const DRAGON_FIRE_LIGHT: f32 = 400.0;
/// Its particles at most (`0x5A0C50`).
const DRAGON_FIRE_COUNT: usize = 500;
/// How often it lays a fire on the ground while it burns (`0x77AB8C`).
const GROUND_FIRE_EVERY: f32 = 0.3;

/// A dragon's breath of fire, `cParticleSystem_dragonfire` (`FX_DRAGONFIRE`, 852, its start
/// `0x77AE90`, its run `0x77AAE0` and its draw `0x77AC20`; docs/re/effects.md, "The dragons'
/// fire"), which keeps to the dragon's head: for 1.5 s nothing, then until 4 s a flame of its
/// variant's texture every 0.001 s at the head, within 5 of it on each axis, leaving along the
/// head's own `−y` at 1,000 ± 50 on each axis, of a size of 7 ± 3 that grows by 80 a second and
/// an angle of π ± π, living 0.2 s (−1,300), added whole, turned, its 4 × 4 frames played once
/// over its life; it lights the night by 400 while it burns, and lays a [`GroundFire`] where its
/// head is above the ground every 0.3 s from a first moment at random below 0.3 s.
#[derive(Clone, Debug, PartialEq)]
pub struct DragonFire {
    pub system: System,
    pub variant: u8,
    clock: f32,
    next: f32,
    alive: bool,
}

impl DragonFire {
    /// A breath of the variant `variant` (the FX event's `+0x38`).
    pub fn new(variant: u8, dice: &mut impl Dice) -> Self {
        let roll = f32::from(u16::try_from(dice.below(0x8000)).unwrap_or(0)) / 32_767.0;
        Self {
            system: System::new(DRAGON_FIRE_COUNT),
            variant,
            clock: 0.0,
            next: roll * GROUND_FIRE_EVERY,
            alive: true,
        }
    }

    /// Its layer: the life −1,300 a second, the size +80.
    fn layer() -> Layer {
        Layer {
            life: -1300.0,
            size: 80.0,
            ..Layer::default()
        }
    }

    /// Its block at the detail 2, its flames' velocity `velocity`.
    fn block(velocity: [f32; 3]) -> Block {
        Block {
            size: spread(7.0, 3.0),
            angle: spread(std::f32::consts::PI, std::f32::consts::PI),
            velocity: spread(velocity, [50.0; 3]),
            offset: spread([0.0; 3], [5.0; 3]),
            interval: 0.001,
            ..Block::default()
        }
    }

    /// A run of `dt` seconds, the head gone from `from` to `to` in its frame and its `−y` axis
    /// `along` (of length 1): whether it lays a fire on the ground at `to` this run.
    pub fn update(
        &mut self,
        dt: f32,
        (from, to): ([f32; 3], [f32; 3]),
        along: [f32; 3],
        dice: &mut impl Dice,
    ) -> bool {
        self.clock += dt;
        if self.clock <= DRAGON_FIRE_WAITS {
            return false;
        }
        let layers = [Self::layer()];
        self.system.clock = self.clock - dt;
        self.system.update(dt, &layers);
        let velocity = along.map(|axis| axis * DRAGON_FIRE_SPEED);
        self.system.emit_along(
            &[Self::block(velocity)],
            &layers,
            DRAGON_FIRE_COUNT,
            (from, to),
            dt,
            dice,
        );
        if self.clock > DRAGON_FIRE_ENDS {
            self.alive = false;
            return false;
        }
        self.next -= dt;
        if self.next >= 0.0 {
            return false;
        }
        self.next = GROUND_FIRE_EVERY;
        true
    }

    /// Whether it burns: past its wait and before its end, so that it lights the night.
    pub fn burns(&self) -> bool {
        self.alive && self.clock > DRAGON_FIRE_WAITS
    }

    /// Whether it ended (`+0x20B4` 0), its flames with it.
    pub fn done(&self) -> bool {
        !self.alive
    }
}

/// The variant of the breath of a dragon of type `creature` (`0x4E3B6D`): 1 for the types 74 and
/// 100, 2 for 88, 0 for the others.
pub fn dragon_fire_variant(creature: u32) -> u8 {
    match creature {
        74 | 100 => 1,
        88 => 2,
        _ => 0,
    }
}

/// The fires on the ground's FX type (`FX_DRAGON_GROUNDFIRE`), their textures by their variant
/// (`0x7AAFB0`), and their light.
pub const GROUND_FIRE: u16 = 923;
pub const GROUND_FIRE_TEXTURES: [&str; 3] =
    ["PARTICLE_FIRE02", "PARTICLE_FIRE05", "PARTICLE_FIRE08"];
pub const GROUND_FIRE_LIGHT: f32 = 200.0;
/// How long a fire on the ground makes flames (`0x7AB070`).
const GROUND_FIRE_BURNS: f32 = 2.0;
const GROUND_FIRE_COUNT: usize = 100;

/// A fire that a dragon's breath lays on the ground, `cParticleSystem_dragon_groundfire`
/// (`FX_DRAGON_GROUNDFIRE`, 923, its start `0x7AB1A0`, its run `0x7AB070` and its draw
/// `0x7AB0E0`), of its breath's variant: for 2 s a flame every 0.04 s within 50 of its place on
/// the ground along each of its axes, of a size of 40 ± 10 that shrinks by 17 a second, living
/// 2.55 s (−100), standing on its foot, added whole in a grey of 200/256 that rises from nothing
/// over the first 10 entries of its ramp and falls to nothing over the last 46, its 4 × 4 frames
/// played once over its life; it lights the night by 200 while a flame lives.
#[derive(Clone, Debug, PartialEq)]
pub struct GroundFire {
    pub system: System,
    pub variant: u8,
    clock: f32,
}

impl GroundFire {
    /// A fire of the variant `variant`.
    pub fn new(variant: u8) -> Self {
        Self {
            system: System::new(GROUND_FIRE_COUNT),
            variant,
            clock: 0.0,
        }
    }

    /// Its ramp: from nothing over 10 entries to a grey of 200/256, held over 200 and to nothing
    /// over 46, at an alpha of 0 (it adds whole).
    pub fn ramp() -> Ramp {
        let grey = [200.0 / 256.0, 200.0 / 256.0, 200.0 / 256.0, 0.0];
        Ramp::new([0.0; 4], &[(grey, 10), (grey, 200), ([0.0; 4], 46)])
    }

    /// Its layer: the life −100 a second, the size −17.
    fn layer() -> Layer {
        Layer {
            life: -100.0,
            size: -17.0,
            ..Layer::default()
        }
    }

    /// Its block at the detail 2.
    fn block() -> Block {
        Block {
            size: spread(40.0, 10.0),
            offset: spread([0.0; 3], [50.0, 50.0, 0.0]),
            interval: 0.04,
            ..Block::default()
        }
    }

    /// A run of `dt` seconds (`0x7AB070`): its flames age, and before 2 s new ones rise.
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) {
        let layers = [Self::layer()];
        self.system.update(dt, &layers);
        if self.clock < GROUND_FIRE_BURNS {
            self.system.clock = self.clock;
            self.system
                .emit(&[Self::block()], &layers, GROUND_FIRE_COUNT, dice);
        }
        self.clock += dt;
    }

    /// Whether it ended: past its 2 s and no flame alive.
    pub fn done(&self) -> bool {
        self.clock >= GROUND_FIRE_BURNS && self.system.particles.is_empty()
    }
}

/// The lava bursts' sound (`LAVABURST`, `0x79F2E5`), the chance in per cent that a cell of deep
/// lava drawn makes one (`0x41AF60`), and the seconds after one before the next may come.
pub const LAVA_BURST_SOUND: u32 = 206;
const LAVA_BURST_CHANCE: f32 = 0.3;
pub const LAVA_BURST_COOLDOWN: f32 = 1.0;
/// A burst's glow at its rock (`0x79F12A`, the style 5 of [`METEOR_GLOW_TEXTURE`]) and its trail's
/// texture.
pub const LAVA_GLOW_HALF: f32 = 30.0;
pub const LAVA_GLOW_COLOUR: [f32; 4] = [128.0 / 255.0, 53.0 / 255.0, 10.0 / 255.0, 1.0];
/// Its particles at most (`0x5A0C50`).
const LAVA_BURST_COUNT: usize = 100;

/// Whether a liquid cell of the material `material` and the corner heights `heights` is deep
/// lava (`0x62B680`): of the materials 4 to 6 (`a_lava`, `b_lava`, `c_lava`) and every corner's
/// bed 2 or more below the surface; the cells about it must be liquid too.
pub fn deep_lava(material: u8, heights: [i8; 4]) -> bool {
    (4..=6).contains(&material) && heights.iter().all(|&height| height <= -2)
}

/// Whether a cell of deep lava drawn makes a burst, by a roll of `rand()` on `dice`
/// (`0x41AF60`): its share of 100 below 0.3.
pub fn lava_bursts(dice: &mut impl Dice) -> bool {
    let roll = f32::from(u16::try_from(dice.below(0x8000)).unwrap_or(0)) / 32_767.0;
    roll * 100.0 < LAVA_BURST_CHANCE
}

/// A lava burst, `cParticleSystem_lavaburst` (`FX_LAVABURST`, 877, its start `0x79F160`, its run
/// `0x79EEE0` and its draw `0x79EFB0`; docs/re/effects.md, "The lava bursts"), of the variant 0:
/// a rock of lava ([`METEOR_MODEL`] in [`METEOR_TEXTURE`], [`METEOR_SCALE`] times its size,
/// turned about two axes at 2 to 4 radians a second each) thrown from the lava in an arc (the
/// object's flag 8): toward a heading at random, 150 to 250 away, as high as 80 to 180 at its
/// middle, in 1.7 to 2.5 s (its share of the way rising by 0.4 to 0.6 a second); a glow at it and
/// a trail of glows every 0.02 s within 1 of it, of a size of 5 ± 2 shrinking by 10 a second,
/// living 1.275 s, added by their alpha in a dim orange fading to black. Landed, it lays a small
/// fire there ([`LavaFlames`]) and ends with its trail.
#[derive(Clone, Debug, PartialEq)]
pub struct LavaBurst {
    pub system: System,
    /// Its rock's two angles and their speeds (`+0x252C` to `+0x2538`).
    pub turns: [f32; 2],
    spins: [f32; 2],
    /// Its share of the way (`+0xA8`), its rate, the arc's height and the way's end in its frame.
    share: f32,
    rate: f32,
    height: f32,
    end: [f32; 3],
    /// Its place in its frame, set at each run for the next one's emission.
    place: [f32; 3],
    landing: bool,
    landed: bool,
}

impl LavaBurst {
    /// A burst from its frame's origin, its arc and its rock's spins rolled on `dice` (`0x5A3C88`,
    /// `0x79F1E7`).
    pub fn new(dice: &mut impl Dice) -> Self {
        let mut unit = || f32::from(u16::try_from(dice.below(0x8000)).unwrap_or(0)) / 32_767.0;
        let height = unit() * 100.0 + 80.0;
        let rate = unit() * 0.2 + 0.4;
        let heading = unit() * std::f32::consts::TAU;
        let distance = unit() * 100.0 + 150.0;
        let spins = [unit() * 2.0 + 2.0, unit() * 2.0 + 2.0];
        let (sin, cos) = heading.sin_cos();
        Self {
            system: System::new(LAVA_BURST_COUNT),
            turns: [0.0; 2],
            spins,
            share: 0.0,
            rate,
            height,
            end: [cos * distance, sin * distance, 0.0],
            place: [0.0; 3],
            landing: false,
            landed: false,
        }
    }

    /// Its trail's ramp (`0x79F180`): at the entry `k` the colour `⌊128·k/256⌋`, `⌊50·k/256⌋`,
    /// `⌊10·k/256⌋` of 255, opaque.
    pub fn ramp() -> Ramp {
        Ramp(std::array::from_fn(|k| {
            let share = f32::from(u16::try_from(k).unwrap_or(0)) / 256.0;
            let [red, green, blue] =
                [128.0_f32, 50.0, 10.0].map(|top| (share * top).floor() / 255.0);
            [red, green, blue, 1.0]
        }))
    }

    fn layer() -> Layer {
        Layer {
            life: -200.0,
            size: -10.0,
            ..Layer::default()
        }
    }

    fn block() -> Block {
        Block {
            size: spread(5.0, 2.0),
            offset: spread([0.0; 3], [1.0; 3]),
            interval: 0.02,
            ..Block::default()
        }
    }

    /// Its rock's place in its frame: along the way by its share, raised by the arc
    /// `(1 − 4(s − ½)²)` of its height (`0x5A7108`).
    pub fn place(&self) -> [f32; 3] {
        let s = self.share;
        let lift = (1.0 - (s - 0.5) * (s - 0.5) * 4.0) * self.height;
        [self.end[0] * s, self.end[1] * s, self.end[2] * s + lift]
    }

    /// A run of `dt` seconds (`0x79EEE0`, then the object's flag 8 of `0x5A6680`): its trail ages
    /// and, while it flies, leaves new glows from where its rock was; its rock turns; then its rock
    /// goes on along the arc, and once its share reaches 1 it lands. Whether it landed this run,
    /// where a small fire is to be laid at its place ([`Self::place`]).
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) -> bool {
        let layers = [Self::layer()];
        self.system.update(dt, &layers);
        let from = self.place;
        let mut lands = false;
        if !self.landed {
            if self.landing {
                self.landed = true;
                lands = true;
            } else {
                self.system.emit_along(
                    &[Self::block()],
                    &layers,
                    LAVA_BURST_COUNT,
                    (from, from),
                    dt,
                    dice,
                );
            }
        }
        for (turn, spin) in self.turns.iter_mut().zip(self.spins) {
            *turn += dt * spin;
        }
        if !self.landed {
            self.place = self.place();
            if self.share >= 1.0 {
                self.landing = true;
            }
            self.share += dt * self.rate;
        }
        lands
    }

    /// Whether its rock flies, drawn with its glow.
    pub fn flies(&self) -> bool {
        !self.landed
    }

    /// Whether it ended: landed and its trail gone.
    pub fn done(&self) -> bool {
        self.landed && self.system.particles.is_empty()
    }
}

/// The small fire that a lava burst lays where it comes from and where it lands, its texture, its
/// grid and its flames' colour (`0x79F9B5`, `0xFFFF701A`), and its light.
pub const LAVA_FLAMES_TEXTURE: &str = "PARTICLE_FIRE02";
pub const LAVA_FLAMES_GRID: u8 = 4;
pub const LAVA_FLAMES_COLOUR: [f32; 4] = [1.0, 112.0 / 255.0, 26.0 / 255.0, 1.0];
pub const LAVA_FLAMES_LIGHT: f32 = 100.0;
/// Its flames, made at once.
const LAVA_FLAMES: usize = 5;

/// The small fire of a lava burst, `cParticleSystem_smallfire` of the variant 0 (`FX_SMALLFIRE`,
/// 878, its start `0x79F970`, its run `0x79F6D0` and its draw `0x79F7D0`): five flames at once
/// within 5 of its place along the ground's axes, of a size of 20 ± 3 that shrinks by 12 a second,
/// living until they shrink away, standing on their feet, added by their alpha in their orange,
/// their 4 × 4 frames played once over their lives; it lights the night by 100 and strikes
/// nothing (its system's flags 0).
#[derive(Clone, Debug, PartialEq)]
pub struct LavaFlames {
    pub system: System,
}

impl LavaFlames {
    /// Its flames, rolled on `dice` (`0x764760` at its clock's 0, the block's burst of 5).
    pub fn new(dice: &mut impl Dice) -> Self {
        let block = Block {
            size: spread(20.0, 3.0),
            offset: spread([0.0; 3], [5.0, 5.0, 0.0]),
            ..Block::default()
        };
        let mut system = System::new(LAVA_FLAMES);
        system
            .particles
            .extend((0..LAVA_FLAMES).map(|_| born(&block, 0, dice)));
        Self { system }
    }

    /// A run of `dt` seconds: its flames age.
    pub fn update(&mut self, dt: f32) {
        let layer = Layer {
            life: -100.0,
            size: -12.0,
            ..Layer::default()
        };
        self.system.update(dt, &[layer]);
    }

    /// Whether its flames are gone.
    pub fn done(&self) -> bool {
        self.system.particles.is_empty()
    }
}

/// The sparks' texture of an arrow's fire and poison, and their particles at most (`0x5A0C50`).
pub const ARROW_SPARKS_TEXTURE: &str = "PARTICLE_GLOW01";
const ARROW_SPARKS_COUNT: usize = 500;

/// The sparks of an arrow's fire or poison, `cParticleSystem_magicfire` (`FX_MAGICFIRE`, 796, its
/// start `0x77A8E0`, its run `0x77A650` and its draw `0x77A6C0`) and `cParticleSystem_magicgift`
/// (`FX_MAGICGIFT`, 808, `0x77C3E0`), of the variant 1, at the share `share` that its event
/// carries (`+0x3C`, in thousandths): 0 for an arrow, its element's for a weapon in hand; a
/// spark of `PARTICLE_GLOW01` every 0.1 s at a point at random along the arrow's shaft (its bones
/// `weapon_fx01` and `weapon_fx02`, `0x766200`), of the size `2·(share + 1)` growing by
/// `15·share + 10` a second, living 0.425 s (−600), riding with the arrow (the system's flags
/// `0x1204`), added by the alpha of its life in its element's colour; the fire lights the night
/// by `200 + 200·share`.
#[derive(Clone, Debug, PartialEq)]
pub struct ArrowSparks {
    pub system: System,
    share: f32,
    colour: [f32; 3],
    lights: bool,
}

impl ArrowSparks {
    /// The sparks of an arrow of `element`: of the share 0 whatever its element's, which its shot's
    /// event carries (`0x5BD76E`, #2093); the arrow's own share sizes its line of glows alone.
    pub fn of_arrow(element: crate::arrows::Element) -> Option<Self> {
        Self::new(element, 0.0)
    }

    /// The sparks of a weapon in hand of `element` at `share`: its share in thousandths, truncated,
    /// as its event carries it (`0x5CE308`) and the start takes it back (`0x77A8E0`).
    pub fn of_weapon(element: crate::arrows::Element, share: f32) -> Option<Self> {
        Self::new(element, (share * 1000.0).trunc() * 0.001)
    }

    /// The sparks of `element` at `share`: none of the magic, which only glows.
    fn new(element: crate::arrows::Element, share: f32) -> Option<Self> {
        let (colour, lights) = match element {
            crate::arrows::Element::Fire => ([255.0, 136.0, 68.0], true),
            crate::arrows::Element::Poison => ([68.0, 255.0, 68.0], false),
            crate::arrows::Element::Magic => return None,
        };
        Some(Self {
            system: System::new(ARROW_SPARKS_COUNT),
            share,
            colour: colour.map(|c: f32| c / 255.0),
            lights,
        })
    }

    fn layer(&self) -> Layer {
        Layer {
            life: -600.0,
            size: self.share * 15.0 + 10.0,
            ..Layer::default()
        }
    }

    fn block(&self) -> Block {
        Block {
            size: spread((self.share + 1.0) * 2.0, 0.0),
            interval: 0.1,
            ..Block::default()
        }
    }

    /// A spark's colour at its life: its element's, at the alpha of its life (the draw's flag 2).
    pub fn colour(&self, life: f32) -> [f32; 4] {
        let [r, g, b] = self.colour;
        [r, g, b, life.clamp(0.0, 255.0) / 255.0]
    }

    /// Its light's radius: the fire's `200 + 200·share` (`0x77A9C7`), none of the poison.
    pub fn light(&self) -> f32 {
        if self.lights {
            self.share * 200.0 + 200.0
        } else {
            0.0
        }
    }

    /// A run of `dt` seconds, the arrow's shaft gone from `from` to `to` in its frame: its sparks
    /// age, and new ones rise along the way the shaft swept.
    pub fn update(
        &mut self,
        dt: f32,
        (from, to): ([[f32; 3]; 2], [[f32; 3]; 2]),
        dice: &mut impl Dice,
    ) {
        let layers = [self.layer()];
        self.system.update(dt, &layers);
        self.system.emit_along_segment(
            &[self.block()],
            &layers,
            ARROW_SPARKS_COUNT,
            (from, to),
            dt,
            dice,
        );
    }
}

/// The burning bone's texture, its grid and its light (`0x775420`, `0x77583D`).
pub const BURNING_BONE_TEXTURE: &str = "PARTICLE_FIRE01";
pub const BURNING_BONE_GRID: u8 = 4;
pub const BURNING_BONE_LIGHT: f32 = 150.0;
/// Its particles at most (`0x5A0C50`), and how long it makes flames past its start.
const BURNING_BONE_COUNT: usize = 300;
const BURNING_BONE_BURNS: f32 = 0.5;

/// A burning bone, `cParticleSystem_burningBone` (`FX_BURNING_BONE`, 788, its start `0x775750`
/// and its run `0x7754B0`), which keeps to its creature's bone (the FX event `0x14`, the bone 20,
/// `Bip01 Head`): from `−0.1·delay` until 0.5 s on its clock a flame of `PARTICLE_FIRE01` every
/// 0.02 s within 6 of the bone on each axis, of the velocity (5, 5, 15) ± 3, a buoyancy of
/// −100 ± 20 that falls by 100 a second, so that it rises ever faster, of a size by the event's
/// `+0x34` (10 for 0, a fifth of it up to 60, else 12) growing by 4 a second, living 0.36 s
/// (−700), in a grey of `+0x34 + 35` (white for 0 and above 220), added whole, turned, its 4 × 4
/// frames played once over its life; it lights the night by 150 and ends with its last flame.
#[derive(Clone, Debug, PartialEq)]
pub struct BurningBone {
    pub system: System,
    clock: f32,
    size: f32,
    pub grey: f32,
    burning: bool,
}

impl BurningBone {
    /// A burning bone of the event's `+0x34` `size` and `+0x3C` `delay`.
    pub fn new(size: f32, delay: u32) -> Self {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a grey of 0 to 255 or more, truncated as `__ftol` does"
        )]
        let grey = (size + 35.0).max(0.0) as u32;
        let grey = if grey > 255 || grey == 35 { 255 } else { grey };
        let flame = if size == 0.0 {
            10.0
        } else if size <= 60.0 {
            size * 0.2
        } else {
            12.0
        };
        let delay = f32::from(u16::try_from(delay).unwrap_or(u16::MAX));
        Self {
            system: System::new(BURNING_BONE_COUNT),
            clock: delay * -0.1,
            size: flame,
            grey: f32::from(u8::try_from(grey).unwrap_or(u8::MAX)) / 255.0,
            burning: true,
        }
    }

    fn layer() -> Layer {
        Layer {
            buoyancy: -100.0,
            life: -700.0,
            size: 4.0,
            ..Layer::default()
        }
    }

    fn block(&self) -> Block {
        Block {
            buoyancy: spread(-100.0, 20.0),
            size: spread(self.size, 0.0),
            velocity: spread([5.0, 5.0, 15.0], [3.0; 3]),
            offset: spread([0.0; 3], [6.0; 3]),
            interval: 0.02,
            ..Block::default()
        }
    }

    /// A run of `dt` seconds, its bone gone from `from` to `to` in its frame (`0x7754B0`): its
    /// flames age and, until its clock passes 0.5 s, new ones leave along the way.
    pub fn update(&mut self, dt: f32, (from, to): ([f32; 3], [f32; 3]), dice: &mut impl Dice) {
        self.clock += dt;
        let layers = [Self::layer()];
        self.system.update(dt, &layers);
        if self.burning {
            let block = self.block();
            self.system
                .emit_along(&[block], &layers, BURNING_BONE_COUNT, (from, to), dt, dice);
        }
        if self.clock > BURNING_BONE_BURNS {
            self.burning = false;
        }
    }

    /// Whether it ended: its flames done and gone.
    pub fn done(&self) -> bool {
        !self.burning && self.system.particles.is_empty()
    }
}

/// The texture of an item's lines of glows (`0x40DA80`, the display's `+0xD4`, loaded at
/// `0x401353`).
pub const GLOW_LINE_TEXTURE: &str = "PARTICLE_GLOW01";

/// A line of glows that an item draws of its own (`0x5BEE90`, through `0x40DA80`): its glows a
/// unit of its length, the half side of each, and its colour (`0xAARRGGBB`), added by its alpha
/// (the source's alpha, the render flag 8 off).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlowLine {
    pub density: f32,
    pub half: f32,
    pub argb: u32,
}

impl GlowLine {
    /// Its colour, red, green, blue and alpha.
    pub fn colour(&self) -> [f32; 4] {
        argb(self.argb)
    }

    /// Its glows' points from `a` to `b`: `⌊length · density⌋` of them, at least one, from `a`
    /// on at even steps, `b` left out.
    pub fn points(&self, a: [f32; 3], b: [f32; 3]) -> Vec<[f32; 3]> {
        let step = std::array::from_fn::<f32, 3, _>(|axis| b[axis] - a[axis]);
        let length = step.iter().map(|d| d * d).sum::<f32>().sqrt();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a count of glows, truncated as `__ftol` does"
        )]
        let count = ((length * self.density) as u32).max(1);
        let count_f = f64::from(count);
        (0..count)
            .map(|k| {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "a share of the line of 0 to 1"
                )]
                let share = (f64::from(k) / count_f) as f32;
                std::array::from_fn(|axis| a[axis] + step[axis] * share)
            })
            .collect()
    }
}

/// The segments of an item's line of glows over the first `found` of its bones 15, 16 and 17
/// (`weapon_fx01` to `weapon_fx03`, `0x5BEE90`): a glow at the first for one, from the first to
/// the second for two, and on from the second to the third for three; none for none.
pub fn glow_segments(found: usize) -> &'static [(usize, usize)] {
    match found {
        0 => &[],
        1 => &[(0, 0)],
        2 => &[(0, 1)],
        _ => &[(0, 1), (1, 2)],
    }
}

/// A glow that an item draws of its own at one of its bones (`0x5BEE90`, through `0x40E2B0`): its
/// half side, its colour (`0xAARRGGBB`), added whole (the source `ONE`, the render flag 8), and
/// its texture (the display's `+0xD8` or `+0xE0`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glow {
    pub half: f32,
    pub argb: u32,
    pub texture: &'static str,
}

impl Glow {
    /// Its colour, red, green, blue and alpha.
    pub fn colour(&self) -> [f32; 4] {
        argb(self.argb)
    }
}

/// The bones at which an item's model glows blue, as many of them from the first as it has
/// (`0x5BE5E0`, the names of `0x401010`): the Seraphim's wings 2 and 4, her lance and her long
/// swords.
pub const BLUE_GLOW_BONES: [&str; 10] = [
    "stdfx_bone01",
    "stdfx_bone02",
    "stdfx_bone03",
    "stdfx_bone04",
    "stdfx_bone05",
    "stdfx_bone06",
    "stdfx_bone07",
    "stdfx_bone08",
    "stdfx_bone09",
    "stdfx_bone10",
];

/// The glow at each of those bones (`0x5BF433`): of a half side 15, blue, of `PARTICLE_GLOW03`.
pub const BLUE_GLOW: Glow = Glow {
    half: 15.0,
    argb: 0xFF10_80FF,
    texture: "PARTICLE_GLOW03",
};

/// The bones at which the wings of the second renderer's effect 9 glitter (`0x5BF540`), and
/// their glitter's texture (the display's `+0xE0`).
pub const GLITTER_BONES: [&str; 4] = ["sera03_fx0", "sera03_fx1", "sera03_fx2", "sera03_fx3"];
pub const GLITTER_TEXTURE: &str = "PARTICLE_SERA03";

/// A glitter's glow at each of those bones and each frame (`0x5BF576`): white, of
/// `PARTICLE_SERA03`, of a half side of 3 to 4 at random.
pub fn wings_glitter(dice: &mut impl Dice) -> Glow {
    Glow {
        half: share(dice) + 3.0,
        argb: 0xFFFF_FFFF,
        texture: GLITTER_TEXTURE,
    }
}

/// The type of the Seraphim's staff (`SeraBFG`), the lightsaber's, and the second renderer's
/// effect of the glittering wings (`Item::model_effect`, `SeraWings05`).
pub const SERAPHIM_STAFF: u32 = 0xEE1;
pub const LIGHTSABER: u32 = 0xC00;
pub const GLITTERING_WINGS: u8 = 9;

/// The Seraphim's staff's line along its bones 15 and 16 (`0x5BF4E6`).
const SERAPHIM_STAFF_LINE: [GlowLine; 1] = [GlowLine {
    density: 0.3,
    half: 15.0,
    argb: 0x3888_88FF,
}];

/// The lightsaber's two lines along its bones 15 and 16 (`0x5BF77B`, `0x5BF7CA`): a thin core
/// and a wide faint halo.
const LIGHTSABER_LINES: [GlowLine; 2] = [
    GlowLine {
        density: 0.8,
        half: 1.5,
        argb: 0xFF88_88FF,
    },
    GlowLine {
        density: 0.1,
        half: 10.0,
        argb: 0x4088_88FF,
    },
];

/// What an item's model draws of its own by its type, besides its element's line
/// (`crate::arrows::element_line`) and its bones' blue glows ([`BLUE_GLOW_BONES`]): its lines
/// along its bones 15 and 16, and whether it glitters at its [`GLITTER_BONES`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OwnGlows {
    pub lines: &'static [GlowLine],
    pub glitters: bool,
}

/// What an item of the type `item` draws of its own (`0x5BEE90`), its weapon record's parent
/// `parent` (`Weapon.pak +0x24`, 0 for none) and its entry's effect of the second renderer
/// `effect` (`Item::model_effect`'s kind): the Seraphim's staff's line, the two lines of a
/// lightsaber, its own type or of that parent (`0x4283C0`), and the glittering wings' glitter.
pub fn own_glows(item: u32, parent: u32, effect: Option<u8>) -> OwnGlows {
    let lines: &'static [GlowLine] = if item == SERAPHIM_STAFF {
        &SERAPHIM_STAFF_LINE
    } else if item == LIGHTSABER || parent == LIGHTSABER {
        &LIGHTSABER_LINES
    } else {
        &[]
    };
    OwnGlows {
        lines,
        glitters: effect == Some(GLITTERING_WINGS),
    }
}

/// The ring's texture, its quads' share of its particles' sizes, its light, and its particles at
/// most (`0x7AB500`, `0x7AB680`, `0x7AB6F0`).
pub const REANIMATE_TEXTURE: &str = "PARTICLE_LINE01";
pub const REANIMATE_SIZE: f32 = 0.125;
pub const REANIMATE_LIGHT: f32 = 200.0;
const REANIMATE_COUNT: usize = 500;
/// How fast its point turns about its place, in radians a second, and how far from it.
const REANIMATE_TURN: f32 = 10.0;
const REANIMATE_RADIUS: f32 = 25.0;

/// The ring about a creature that a caster raises (924, `FX_REANIMATE`,
/// `cParticleSystem_reanimate`, its constructor `0x7AB500`, its start `0x7AB6F0`, its run
/// `0x7AB590` and its draw `0x7AB680`; the FX event `0x84`): a point that turns once about its
/// place on a circle of 25, at ten radians a second (some 0.63 s), leaves particles of
/// `PARTICLE_LINE01` that stand where they are born, one every 0.004 s at the detail 2, of a size
/// of 40 ± 10 falling by 10 a second, living 2.55 s (−100), a pale blue that rises over the first
/// 10 entries of their life, holds over 200 and fades over the last 46 ([`Reanimate::ramp`]);
/// drawn standing on their feet at an eighth of their size, the ramp's colour added by its alpha
/// (the flags `0x89`); it lights the night by 200 and ends with its last particle.
#[derive(Clone, Debug, PartialEq)]
pub struct Reanimate {
    pub system: System,
    angle: f32,
}

impl Default for Reanimate {
    fn default() -> Self {
        Self::new()
    }
}

impl Reanimate {
    /// A ring as it starts, its point at the angle 0.
    pub fn new() -> Self {
        Self {
            system: System::new(REANIMATE_COUNT),
            angle: 0.0,
        }
    }

    fn layer() -> Layer {
        Layer {
            life: -100.0,
            size: -10.0,
            ..Layer::default()
        }
    }

    /// Its block at its point's angle `angle`: its offset on the circle.
    fn block(angle: f32) -> Block {
        let (sin, cos) = angle.sin_cos();
        Block {
            size: spread(40.0, 10.0),
            offset: spread(
                [REANIMATE_RADIUS * cos, REANIMATE_RADIUS * sin, 0.0],
                [0.0; 3],
            ),
            interval: 0.004,
            ..Block::default()
        }
    }

    /// Its ramp (`+0x20AC`): (100, 100, 256) of 256, from nothing to whole over 10 entries,
    /// whole over 200, to nothing over 46.
    pub fn ramp() -> Ramp {
        let blue = |alpha: f32| [100.0 / 256.0, 100.0 / 256.0, 1.0, alpha];
        Ramp::new(
            blue(0.0),
            &[(blue(1.0), 10), (blue(1.0), 200), (blue(0.0), 46)],
        )
    }

    /// A run of `dt` seconds (`0x7AB590`): its particles age, and while its point has not turned
    /// once it leaves new ones from it.
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) {
        let layers = [Self::layer()];
        self.system.update(dt, &layers);
        if self.angle < std::f32::consts::TAU {
            let block = Self::block(self.angle);
            self.system.emit(&[block], &layers, REANIMATE_COUNT, dice);
        }
        self.angle += dt * REANIMATE_TURN;
    }

    /// Whether it ended: its turn over and its particles gone.
    pub fn done(&self) -> bool {
        self.angle >= std::f32::consts::TAU && self.system.particles.is_empty()
    }
}

/// The blast's flames' textures, the mine's and the bomb trap's, of a grid of 4; its scorch's two,
/// one by a coin; the trap's ring's; the scorch's half side; the ring's width and segments; the
/// quake of a mine's blast (`0x779960`, `0x779B60`, `0x7BAD70`).
pub const EXPLOSION_FLAMES: [&str; 2] = ["PARTICLE_FIRE01", "PARTICLE_FIRE04"];
pub const EXPLOSION_GRID: u8 = 4;
pub const EXPLOSION_SCORCHES: [&str; 2] = ["FX_STREAKS02", "FX_STREAKS01"];
pub const EXPLOSION_RING: &str = "Fx_ExplosionCircle01";
pub const EXPLOSION_SCORCH_HALF: f32 = 50.0;
pub const EXPLOSION_RING_WIDTH: f32 = 20.0;
pub const EXPLOSION_RING_SEGMENTS: u16 = 15;
pub const MINE_QUAKE: u32 = 21;
const EXPLOSION_COUNT: usize = 100;
/// How long it makes flames and strikes, how long its scorch lies whole, and how fast it fades
/// and its ring grows (`0x779A50`, the floats at `0x890820`, `0x89076C` and `0x8907D8`).
const EXPLOSION_BURNS: f32 = 0.8;
const EXPLOSION_SCORCHED: f32 = 10.0;
const EXPLOSION_FADES: f32 = 0.2;
const EXPLOSION_RING_GROWS: f32 = 50.0;
const EXPLOSION_RING_FROM: f32 = 10.0;
const EXPLOSION_RING_TO: f32 = 50.0;

/// A blast, `cParticleSystem_explosion` (794, `FX_EXPLOSION`, its constructor `0x779960`, its
/// start `0x779F60`, its run `0x779A50` and its draw `0x779B60`), of the scale 1 that both its
/// makers give it (the variant 100): a Dwarf's mine's (`+0x3C` 0) and the Dark Elf's bomb trap's
/// (1). For 0.8 s a flame every 0.02 s at the detail 2 within 20 of a point 10 over its place, of
/// a size of 10 ± 5 growing by 20 a second, living 0.85 s (−300), turned at random, its 4 × 4
/// frames played once over its life, white at the alpha of its life, added (the flags 7): of
/// `PARTICLE_FIRE01` the mine's, of `FIRE04` the trap's; a scorch of `FX_STREAKS02` or `01` under
/// it on the ground, of a half side of 50 at a turn at random, whole for 10 s and fading over 5,
/// laid over by its alpha; and the trap's alone, a ring of `Fx_ExplosionCircle01` from 10 out to
/// 50 at 50 a second, 20 wide, added whole. It strikes while it burns (its flags `0x400`) and it
/// ends with its scorch.
#[derive(Clone, Debug, PartialEq)]
pub struct Explosion {
    pub system: System,
    trap: bool,
    clock: f32,
    /// Its scorch's texture of [`EXPLOSION_SCORCHES`], its turn, and its alpha.
    pub scorch: usize,
    pub scorch_turn: f32,
    pub fade: f32,
    ring: f32,
}

impl Explosion {
    /// A mine's blast, or the bomb trap's where `trap`: its scorch's coin rolled as it is made,
    /// then its turn as it starts.
    pub fn new(trap: bool, dice: &mut impl Dice) -> Self {
        let scorch = usize::try_from(dice.below(2)).unwrap_or(0);
        let scorch_turn = share(dice) * std::f32::consts::TAU;
        Self {
            system: System::new(EXPLOSION_COUNT),
            trap,
            clock: 0.0,
            scorch,
            scorch_turn,
            fade: 1.0,
            ring: EXPLOSION_RING_FROM,
        }
    }

    fn layer() -> Layer {
        Layer {
            life: -300.0,
            size: 20.0,
            ..Layer::default()
        }
    }

    fn block() -> Block {
        Block {
            size: spread(10.0, 5.0),
            angle: spread(std::f32::consts::PI, std::f32::consts::PI),
            offset: spread([0.0, 0.0, 10.0], [20.0; 3]),
            interval: 0.02,
            ..Block::default()
        }
    }

    /// Its flames' texture of [`EXPLOSION_FLAMES`].
    pub fn flames(&self) -> usize {
        usize::from(self.trap)
    }

    /// Whether it strikes still: while it burns.
    pub fn strikes(&self) -> bool {
        self.clock <= EXPLOSION_BURNS
    }

    /// Its ring's inner radius while it shows, the trap's alone.
    pub fn ring(&self) -> Option<f32> {
        (self.trap && self.ring < EXPLOSION_RING_TO).then_some(self.ring)
    }

    /// A run of `dt` seconds (`0x779A50`): its flames age, and while it burns new ones rise; past
    /// 10 s its scorch fades; its ring grows.
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) {
        let layers = [Self::layer()];
        self.system.update(dt, &layers);
        if self.strikes() {
            self.system
                .emit(&[Self::block()], &layers, EXPLOSION_COUNT, dice);
        } else if self.clock > EXPLOSION_SCORCHED {
            self.fade = (self.fade - EXPLOSION_FADES * dt).max(0.0);
        }
        self.clock += dt;
        self.ring += EXPLOSION_RING_GROWS * dt;
    }

    /// Whether it ended: its scorch gone.
    pub fn done(&self) -> bool {
        self.fade <= 0.0
    }
}

/// A Dwarf's mine's model and its textures, its own and its flash's, its smoke's, its model's
/// scale, and its particles at most (`0x7BAC30`, `0x7BAF30`).
pub const MINE_MODEL: &str = "DWARF_MINE.GRN";
pub const MINE_TEXTURE: &str = "DWARF_MINE";
pub const MINE_FLASH_TEXTURE: &str = "DWARF_MINE2";
pub const MINE_SMOKE_TEXTURE: &str = "PARTICLE_SMOKE04";
pub const MINE_SCALE: f32 = 0.4;
const MINE_COUNT: usize = 100;
const MINE_PUFFS: usize = 20;
/// Its flash's clock's turn (`0x7BAD70`, the float at `0x8909A8`): from 1.5 it goes back to −0.2,
/// and below 0 the flash shows.
const MINE_BLINK_TOP: f32 = 1.5;
const MINE_BLINK_FROM: f32 = -0.2;

/// A Dwarf's mine lying on the ground (1153, `FX_DWARF_MINE`, `cParticleSystem_dwarf_mine`, its
/// constructor `0x7BAC30`, its start `0x7BB0B0`, its run `0x7BAD70` and its draw `0x7BAF30`): its
/// model [`MINE_MODEL`] at 0.4 of its size, turned about the vertical at random, in its own
/// texture, lit as the world's models are; its button's flash, the model again in
/// [`MINE_FLASH_TEXTURE`] added whole, for 0.2 s in every 1.7 s from 1.5 s after it is laid;
/// and as it is laid a burst of 20 dark puffs of [`MINE_SMOKE_TEXTURE`] rising ever faster,
/// growing by 8 a second, living 1.275 s (−200), of the ramp [`DwarfMine::ramp`], laid over by
/// their alpha, turned, darkened by the night (the flags `0x4C`).
#[derive(Clone, Debug, PartialEq)]
pub struct DwarfMine {
    pub system: System,
    /// Its turn about the vertical.
    pub turn: f32,
    blink: f32,
    smoked: bool,
}

impl DwarfMine {
    /// A mine as it is laid, its turn rolled.
    pub fn new(dice: &mut impl Dice) -> Self {
        Self {
            system: System::new(MINE_COUNT),
            turn: share(dice) * std::f32::consts::TAU,
            blink: 0.0,
            smoked: false,
        }
    }

    fn layer() -> Layer {
        Layer {
            life: -200.0,
            size: 8.0,
            ..Layer::default()
        }
    }

    fn block() -> Block {
        Block {
            buoyancy: spread(-35.0, 5.0),
            size: spread(6.0, 2.0),
            angle: spread(std::f32::consts::PI, std::f32::consts::PI),
            velocity: spread([0.0, 0.0, 15.0], [20.0, 20.0, 0.0]),
            offset: spread([0.0; 3], [5.0; 3]),
            frames: FRAME_OF_LAYER,
            ..Block::default()
        }
    }

    /// Its puffs' ramp (`+0x20B8`): a grey (50, 50, 50) of 256 at the alpha 0.996, to the alpha
    /// 0.39 over 60 entries, then to (74, 56, 71) at the alpha 0 over 196.
    pub fn ramp() -> Ramp {
        let grey = |alpha: f32| [50.0 / 256.0, 50.0 / 256.0, 50.0 / 256.0, alpha];
        Ramp::new(
            grey(255.0 / 256.0),
            &[
                (grey(100.0 / 256.0), 60),
                ([74.0 / 256.0, 56.0 / 256.0, 71.0 / 256.0, 0.0], 196),
            ],
        )
    }

    /// A run of `dt` seconds (`0x7BAD70`): its flash's clock turns, its puffs age, and at its
    /// first run they burst.
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) {
        self.blink += dt;
        if self.blink > MINE_BLINK_TOP {
            self.blink = MINE_BLINK_FROM;
        }
        self.system.update(dt, &[Self::layer()]);
        if !self.smoked {
            self.smoked = true;
            self.system
                .burst(&Self::block(), MINE_PUFFS, MINE_COUNT, dice);
        }
    }

    /// Whether its button flashes.
    pub fn flashes(&self) -> bool {
        self.blink < 0.0
    }
}

/// The mines' sign's texture, its particles at most, the circle its tongues stand on, and how
/// many it makes a second while every mine is left (`0x7BA580`, `0x7BA610`, the float at
/// `0x890D48`).
pub const MINE_SIGN_TEXTURE: &str = "PARTICLE_FLOOP01";
const MINE_SIGN_COUNT: usize = 500;
pub const MINE_SIGN_RADIUS: f32 = 25.0;
const MINE_SIGN_RATE: f32 = 200.0;
/// Its accumulator's top, at which it starts again (the float at `0x890998`).
const MINE_SIGN_TOP: f32 = 100.0;

/// The Dwarf's sign of his mines (1154, `FX_DWARF_MINE_SIGN`, `cParticleSystem_dwarf_mine_sign`,
/// its constructor `0x7BA580`, its start `0x7BA8D0`, its run `0x7BA610` and its draw `0x7BA830`),
/// riding with him (its flags `0x2202`): tongues of [`MINE_SIGN_TEXTURE`] lying flat on the ground
/// about his feet, each standing on the circle of 25 and pointing out from it at a turn of ±0.1,
/// along an arc from his right that is the share of his mines left, as many a second as 200
/// times that share; of a size of 4 ± 2 growing by 8 a second, living 1.275 s (−200), of the ramp
/// [`MineSign::ramp`], added by their alpha (the flags `0x8D`). It goes with his last mine.
#[derive(Clone, Debug, PartialEq)]
pub struct MineSign {
    pub system: System,
    due: f32,
}

impl Default for MineSign {
    fn default() -> Self {
        Self::new()
    }
}

impl MineSign {
    /// A sign as it starts.
    pub fn new() -> Self {
        Self {
            system: System::new(MINE_SIGN_COUNT),
            due: 0.0,
        }
    }

    fn layer() -> Layer {
        Layer {
            life: -200.0,
            size: 8.0,
            ..Layer::default()
        }
    }

    /// Its ramp (`+0x20B4`): from (17, 7, 228) of 256 at the alpha 1, to (51, 126, 203) at 0.79
    /// over 96 entries, to (60, 141, 141) at 0.55 over 49, to (64, 64, 64) at 0.25 over 111.
    pub fn ramp() -> Ramp {
        let c = |r: f32, g: f32, b: f32, a: f32| [r / 256.0, g / 256.0, b / 256.0, a];
        Ramp::new(
            c(17.0, 7.0, 228.0, 1.0),
            &[
                (c(51.0, 126.0, 203.0, 0.79), 96),
                (c(60.0, 141.0, 141.0, 0.55), 49),
                (c(64.0, 64.0, 64.0, 0.25), 111),
            ],
        )
    }

    /// A run of `dt` seconds (`0x7BA610`) with `share` of the mines left: its tongues age, and
    /// new ones stand along its arc, each at a point at random on it, its turn pointing out.
    pub fn update(&mut self, dt: f32, share_left: f32, dice: &mut impl Dice) {
        let layers = [Self::layer()];
        self.system.update(dt, &layers);
        self.due += share_left * dt * MINE_SIGN_RATE;
        if self.due > MINE_SIGN_TOP {
            self.due = 0.0;
        }
        while self.due >= 1.0 {
            let a = std::f32::consts::TAU * share_left * share(dice);
            let (sin, cos) = a.sin_cos();
            let block = Block {
                size: spread(4.0, 2.0),
                angle: spread(std::f32::consts::FRAC_PI_2 - a, 0.1),
                offset: spread(
                    [MINE_SIGN_RADIUS * cos, MINE_SIGN_RADIUS * sin, 0.0],
                    [0.0; 3],
                ),
                ..Block::default()
            };
            self.system.burst(&block, 1, MINE_SIGN_COUNT, dice);
            self.due -= 1.0;
        }
    }
}

/// The flying skull's model and its scale, its trail's texture and its particles at most
/// (`0x7A5DE0`, `0x7A6040`).
pub const SKULL_MODEL: &str = "SKULL.GRN";
pub const SKULL_SCALE: f32 = 2.0;
pub const SKULL_TRAIL_TEXTURE: &str = "PARTICLE_GLOW03";
const SKULL_COUNT: usize = 200;

/// The death's kiss's flying skull (905, `FX_FLYINGSKULL`, `cParticleSystem_flyingskull`, its
/// constructor `0x7A5DE0`, its start `0x7A6370`, its run `0x7A5F40` and its draw `0x7A6040`),
/// which flies as the homing souls (its flags `0x10000`, its speed 150): its model [`SKULL_MODEL`]
/// at twice its size, turned about the vertical to its heading (`atan2(−y, x) + π/2`), at a point
/// that wobbles about its place as a soul's ([`soul_wobble`]); and from that point a trail of
/// glows of [`SKULL_TRAIL_TEXTURE`], one every 0.05 s at the detail 2, of a size of 18 ± 2 falling
/// by 15 a second, living 0.85 s (−300), of the ramp [`FlyingSkull::ramp`], added whole (the
/// flags `0x19`). Its seconds over, or its object ended, it shows no more and makes no more, and
/// ends with its last glow.
#[derive(Clone, Debug, PartialEq)]
pub struct FlyingSkull {
    pub system: System,
    seconds: f32,
    out: bool,
}

impl FlyingSkull {
    /// A skull for `seconds` (its FX event's `+0x4C`).
    pub fn new(seconds: f32) -> Self {
        Self {
            system: System::new(SKULL_COUNT),
            seconds,
            out: false,
        }
    }

    fn layer() -> Layer {
        Layer {
            life: -300.0,
            size: -15.0,
            ..Layer::default()
        }
    }

    fn block(t: f32) -> Block {
        Block {
            size: spread(18.0, 2.0),
            offset: spread(soul_wobble(t), [0.0; 3]),
            interval: 0.05,
            ..Block::default()
        }
    }

    /// Its trail's ramp (`+0x20B0`): from nothing to an orange (0.996, 0.852, 0.051) over 10
    /// entries, to a rust (0.879, 0.383, 0.160) over 126, to nothing over 120.
    pub fn ramp() -> Ramp {
        Ramp::new(
            [0.0; 4],
            &[
                ([0.996, 0.852, 0.051, 0.996], 10),
                ([0.879, 0.383, 0.160, 0.781], 126),
                ([0.0; 4], 120),
            ],
        )
    }

    /// A run of `dt` seconds, its place gone from `from` to `to` in the models' frame
    /// (`0x7A5F40`): its glows age, and while it flies it leaves new ones along its way.
    pub fn update(&mut self, dt: f32, (from, to): ([f32; 3], [f32; 3]), dice: &mut impl Dice) {
        let layers = [Self::layer()];
        self.system.update(dt, &layers);
        if self.out {
            return;
        }
        let block = Self::block(self.system.clock);
        self.system
            .emit_along(&[block], &layers, SKULL_COUNT, (from, to), dt, dice);
        if self.system.clock > self.seconds {
            self.out = true;
        }
    }

    /// Its object's end (`+0x74` bit 1): it shows and makes no more.
    pub fn end(&mut self) {
        self.out = true;
    }

    /// Whether it shows its model and its wobbling point there: while it flies.
    pub fn flies(&self) -> Option<[f32; 3]> {
        (!self.out).then(|| soul_wobble(self.system.clock))
    }

    /// Whether it ended: out and no glow alive.
    pub fn done(&self) -> bool {
        self.out && self.system.particles.is_empty()
    }
}

/// The skull's turn about the vertical in the models' frame by its heading `(x, y)` there
/// (`0x7A6040`): `atan2(−y, x) + π/2`.
pub fn skull_turn([x, y]: [f32; 2]) -> f32 {
    (-y).atan2(x) + std::f32::consts::FRAC_PI_2
}

/// The laser beams' variants that their callers lay (`0x604030`'s flags byte): the Vampiress's
/// bite's, the hellfire's and the eye lasers', the drains'.
pub const BITE_BEAM: u8 = 0;
pub const LASER_BEAM: u8 = 1;
pub const DRAIN_BEAM: u8 = 2;

/// A laser segment's quad: its four corners in its own frame and their texture's points.
pub type LaserQuad = ([[f32; 3]; 4], [[f32; 2]; 4]);

/// A segment of a laser beam (901, `FX_LASERBEAM`, `cParticleSystem_laserbeam`, its constructor
/// `0x7A3790`, start `0x7A50B0`, run `0x7A3DF0` and draw `0x7A4D50`; #1092, #2119): the `k`-th of
/// the `n` that `createLaserbeam` (`0x604030`) lays along a line, a cell of ground long each,
/// staying where it was laid. It is two crossed quads along its axis, one across and one upright,
/// added whole in its variant's texture and colour times its fade, which holds for its variant's
/// seconds and then falls by its rate a second, when it ends: the bite's (0) and the drains' (2 and
/// the others) 20 wide in `FX_VAMPBLOODLINE`, a quarter of the texture stepping every 0.1 s,
/// swaying across and up and down by up to 7 at the chain's middle, half grey or tinted; the
/// lasers' (1) and the variant 4's 30 wide in `LASERBEAMPART`, straight, orange and pale pink.
#[derive(Clone, Debug, PartialEq)]
pub struct LaserSegment {
    variant: u8,
    k: u16,
    n: u16,
    clock: f32,
    fade: f32,
}

impl LaserSegment {
    /// The `k`-th segment of `n` of the variant `variant` as it starts (`0x7A50B0`).
    pub fn new(variant: u8, (k, n): (u16, u16)) -> Self {
        Self {
            variant,
            k,
            n: n.max(1),
            clock: 0.0,
            fade: 1.0,
        }
    }

    /// Its variant's hold in seconds and the fall of its fade a second (`+0x20F4`, `+0x20F8`).
    fn hold_rate(&self) -> (f32, f32) {
        match self.variant {
            0 => (4.4, 20.0),
            1 | 4 => (0.0, 2.0),
            3 => (2.0, 1.0),
            5 => (1.0, 1.5),
            _ => (0.0, 1.0),
        }
    }

    /// Whether it is straight and 30 wide, the texture whole (the variants 1 and 4), rather than
    /// swaying and 20 wide on a quarter of it.
    fn straight(&self) -> bool {
        matches!(self.variant, 1 | 4)
    }

    /// Its texture by its variant (`+0x2404`, `+0x2408`).
    pub fn texture(variant: u8) -> &'static str {
        if matches!(variant, 1 | 4) {
            "LASERBEAMPART"
        } else {
            "FX_VAMPBLOODLINE"
        }
    }

    /// The sound each segment of the variant plays as it starts: the bite's `VAMP_BLUTBISS`, the
    /// lasers' `SPELL_LASER` (`0x7A50B0`).
    pub fn sound(variant: u8) -> Option<u32> {
        match variant {
            0 => Some(2076),
            1 => Some(2074),
            _ => None,
        }
    }

    /// A run of `dt` seconds (`0x7A3DF0`): past its hold its fade falls by its rate.
    pub fn update(&mut self, dt: f32) {
        self.clock += dt;
        let (hold, rate) = self.hold_rate();
        if self.clock > hold {
            self.fade -= rate * dt;
        }
    }

    /// Whether it ended: its fade below 0.
    pub fn done(&self) -> bool {
        self.fade < 0.0
    }

    /// Its colour now, as the frame's values (`⌊255 f⌋`, halved but for the lasers).
    pub fn colour(&self) -> [f32; 4] {
        let c = (255.0 * self.fade.max(0.0)).floor();
        let [r, g, b] = match self.variant {
            1 => [255.0, 146.0, 39.0].map(|v| (v * self.fade.max(0.0)).floor()),
            4 => [255.0, 235.0, 235.0].map(|v| (v * self.fade.max(0.0)).floor()),
            10 => [
                (c / 2.0).floor(),
                (c * 50.0 / 255.0).floor(),
                (c * 50.0 / 255.0).floor(),
            ],
            11 => [
                (c * 50.0 / 255.0).floor(),
                (c * 50.0 / 255.0).floor(),
                (c / 2.0).floor(),
            ],
            12 => [
                (c / 2.0).floor(),
                (c / 2.0).floor(),
                (c * 50.0 / 255.0).floor(),
            ],
            _ => [(c / 2.0).floor(); 3],
        };
        [r / 255.0, g / 255.0, b / 255.0, 1.0]
    }

    /// The sway of the joint `j` of the chain at its clock, across and up (`0x7A3DF0`): 0 at both
    /// ends of the chain, up to 7 at its middle.
    fn sway(&self, j: u16) -> (f32, f32) {
        let (j, t) = (f32::from(j), self.clock);
        let s = (j * std::f32::consts::PI / f32::from(self.n)).sin();
        let a = 4.0 * (1.456_34 * j + 3.0 * t + 1.783_56).cos()
            + 3.0 * (2.852_61 * j + 5.895_635 * t).cos();
        let b = 4.0 * (0.964_52 * j + 3.0 * t + 0.743_72).cos()
            + 3.0 * (2.578_38 * j + 5.836_23 * t).cos();
        (a * s, b * s)
    }

    /// Its two quads in its own frame, `x` along it from 0 to `length`, `y` across and `z` up,
    /// each its corners from its start's minus side to its end's and back on the plus side, and
    /// their texture's points: the across one and the upright one.
    pub fn quads(&self, length: f32) -> [LaserQuad; 2] {
        let (half, ends) = if self.straight() {
            (15.0, [(0.0, 0.0); 2])
        } else {
            (10.0, [self.sway(self.k), self.sway(self.k + 1)])
        };
        // The band of the texture: whole, or the quarter that steps every 0.1 s.
        let (low, high) = if self.straight() {
            (0.0, 1.0)
        } else {
            let quarter = (10.0 * self.clock).floor().rem_euclid(4.0) * 0.25;
            (quarter, quarter + 0.25)
        };
        let uvs = [[0.0, high], [1.0, high], [1.0, low], [0.0, low]];
        let [(a0, b0), (a1, b1)] = ends;
        let across = [
            [0.0, a0 - half, b0],
            [length, a1 - half, b1],
            [length, a1 + half, b1],
            [0.0, a0 + half, b0],
        ];
        let upright = [
            [0.0, a0, b0 - half],
            [length, a1, b1 - half],
            [length, a1, b1 + half],
            [0.0, a0, b0 + half],
        ];
        [(across, uvs), (upright, uvs)]
    }
}

/// The axes of a laser segment's frame along `direction` in the models' frame (`0x7A50B0`):
/// turned about `(0, −n.z, n.y)` by `acos(n.x)`, so that its `x` lies along the unit direction
/// `n`; its `x`, `y` and `z` there.
pub fn laser_axes(direction: [f32; 3]) -> [[f32; 3]; 3] {
    let length = direction.iter().map(|v| v * v).sum::<f32>().sqrt();
    let identity = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    if length < 1e-7 {
        return identity;
    }
    let n = direction.map(|v| v / length);
    let axis = [0.0, -n[2], n[1]];
    let size = axis.iter().map(|v| v * v).sum::<f32>().sqrt();
    if size < 1e-7 {
        return identity;
    }
    let a = axis.map(|v| v / size);
    let angle = n[0].clamp(-1.0, 1.0).acos();
    let (sin, cos) = angle.sin_cos();
    // Rodrigues's turn of `v` about `a` by the angle.
    let turn = |v: [f32; 3]| {
        let cross = [
            a[1] * v[2] - a[2] * v[1],
            a[2] * v[0] - a[0] * v[2],
            a[0] * v[1] - a[1] * v[0],
        ];
        let dot = a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
        std::array::from_fn(|i| v[i] * cos + cross[i] * sin + a[i] * dot * (1.0 - cos))
    };
    identity.map(turn)
}

/// The points of a vine, the thorns' plant's or a tentacle's tendril's (`0x798820`, `0x7B0DF0`).
pub const VINE_POINTS: u16 = 128;
/// The thorns' plant's textures: its vine's and spurs' dots and its glow on the ground.
pub const PLANT_TEXTURE: &str = "PARTICLE_GLOW01";
pub const PLANT_GLOW_TEXTURE: &str = "PARTICLE_GLOW03";
/// The colours of a hero's plant and of a creature's (`0x799640`: `0xFF40FF40`, `0xFF4040FF`).
pub const PLANT_GREEN: [f32; 4] = [64.0 / 255.0, 1.0, 64.0 / 255.0, 1.0];
pub const PLANT_BLUE: [f32; 4] = [64.0 / 255.0, 64.0 / 255.0, 1.0, 1.0];

/// The forms of a vine's curve along each axis by its rolls `U1` to `U4`: `sin(0.1 k f)·a`, its
/// size `a` and its frequency `f` each `p·U + q`, its size along x by `U1`, its frequency by `U2`,
/// along y by `U3` and `U4`; the curve turned by `2π·U0` about the vertical.
struct Curve {
    x_size: [f32; 2],
    x_freq: [f32; 2],
    y_size: [f32; 2],
    y_freq: [f32; 2],
}

/// A vine's stem (`+0x20C4`), its two sways (`+0x26C4`, `+0x2CC4`).
const STEM: Curve = Curve {
    x_size: [2.0, 4.5],
    x_freq: [1.0, 0.5],
    y_size: [2.0, 6.5],
    y_freq: [1.0, 0.7],
};
const SWAY_A: Curve = Curve {
    x_size: [3.0, 3.5],
    x_freq: [0.8, 0.2],
    y_size: [3.0, 6.0],
    y_freq: [0.4, 0.1],
};
const SWAY_B: Curve = Curve {
    x_size: [3.0, 2.5],
    x_freq: [1.0, 0.5],
    y_size: [2.0, 7.0],
    y_freq: [1.0, 1.0],
};

/// A [`Curve`] of its rolls: its sizes and frequencies along each axis and its turn's sine and
/// cosine.
struct Rolled {
    x: [f32; 2],
    y: [f32; 2],
    turn: (f32, f32),
}

impl Curve {
    /// The curve of five rolls of `dice`, `U0` to `U4` in turn.
    fn roll(&self, dice: &mut impl Dice) -> Rolled {
        let [u0, u1, u2, u3, u4]: [f32; 5] = std::array::from_fn(|_| share(dice));
        let of = |[p, q]: [f32; 2], u: f32| p * u + q;
        Rolled {
            x: [of(self.x_size, u1), of(self.x_freq, u2)],
            y: [of(self.y_size, u3), of(self.y_freq, u4)],
            turn: (std::f32::consts::TAU * u0).sin_cos(),
        }
    }
}

impl Rolled {
    /// Its point `k` across the vertical.
    fn at(&self, k: f32) -> [f32; 2] {
        let ([x_size, x_freq], [y_size, y_freq], (sin, cos)) = (self.x, self.y, self.turn);
        let x = (0.1 * k * x_freq).sin() * x_size;
        let y = (0.1 * k * y_freq).sin() * y_size;
        [x * cos - y * sin, x * sin + y * cos]
    }
}

/// A vine's shape, rolled as it is made (`0x798820`, `0x7B0DF0`): its stem of 128 points in the
/// models' frame from its foot, rising half a unit a point and winding out as it rises, and its
/// two sways, each a loop of 128 offsets across, the first 64 rolled and the rest mirrored, which
/// climb it at 30 and 27 entries a second.
#[derive(Clone, Debug, PartialEq)]
pub struct Vine {
    stem: Vec<[f32; 3]>,
    sways: [Vec<[f32; 2]>; 2],
}

impl Vine {
    /// A vine of the rolls of `dice`: its stem's five, then each sway's five.
    pub fn new(dice: &mut impl Dice) -> Self {
        let stem = STEM.roll(dice);
        let stem = (0..VINE_POINTS)
            .map(|k| {
                let k = f32::from(k);
                let [x, y] = stem.at(k);
                let share = k / f32::from(VINE_POINTS);
                [x * share, y * share, 0.5 * k]
            })
            .collect();
        let sways = [SWAY_A, SWAY_B].map(|curve| {
            let sway = curve.roll(dice);
            let half: Vec<[f32; 2]> = (0..VINE_POINTS / 2)
                .map(|k| sway.at(f32::from(k)))
                .collect();
            half.iter().chain(half.iter().rev()).copied().collect()
        });
        Self { stem, sways }
    }

    /// Its stem's point `k`.
    pub fn stem(&self, k: usize) -> [f32; 3] {
        self.stem.get(k).copied().unwrap_or_default()
    }

    /// Its sways' offsets at the point `k` at the clock `t`, added: the first's entry
    /// `k − ⌊30 t⌋` about its loop, the second's `k − ⌊27 t⌋` (`0x798F10`).
    pub fn sway(&self, k: usize, t: f32) -> [f32; 2] {
        let points = f32::from(VINE_POINTS);
        let entry = |sway: &[[f32; 2]], rate: f32| {
            let k = f32::from(u16::try_from(k).unwrap_or(0));
            let at = (k - (rate * t).floor()).rem_euclid(points);
            let at = usize::try_from(crate::fight::truncated(f64::from(at))).unwrap_or(0);
            sway.get(at).copied().unwrap_or_default()
        };
        let ([ax, ay], [bx, by]) = (entry(&self.sways[0], 30.0), entry(&self.sways[1], 27.0));
        [ax + bx, ay + by]
    }
}

/// How a [`KillerPlant`] grows: it rises, stands for its seconds, sinks, and ends.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Growth {
    Rises,
    Stands,
    Sinks,
    Done,
}

/// The thorns' plant (848, `FX_KILLERPLANTS`, `cParticleSystem_killerplants`, its constructor
/// `0x798820`, start `0x799640`, run `0x798E50` and draw `0x798F10`; #1092): a vine of 128
/// dots of [`PLANT_TEXTURE`] facing the camera, from a half side of 3 at its foot to 1 at its
/// tip, 64 high, swaying as its two sways climb it, seven spurs of dots straight out along its
/// bends, and a glow of [`PLANT_GLOW_TEXTURE`] flat on the ground under it, all added by their
/// alpha in its colour, green a hero's and blue a creature's. Its rise goes from −3 to 0 over a
/// second, which every dot's half side takes, so that it grows from its foot up; it stands for
/// its seconds, and sinks as it rose.
#[derive(Clone, Debug, PartialEq)]
pub struct KillerPlant {
    vine: Vine,
    spurs: [usize; 7],
    clock: f32,
    rise: f32,
    stands: f32,
    growth: Growth,
    pub colour: [f32; 4],
}

impl KillerPlant {
    /// A plant that stands `seconds` after its rise, a hero's or a creature's, of the rolls of
    /// `dice`: its seven spurs' points first, the `m`-th `⌊128 m / 7⌋ + rand() % 7` and at least
    /// 1, then its vine's.
    pub fn new(seconds: f32, hero: bool, dice: &mut impl Dice) -> Self {
        let spurs = std::array::from_fn(|m| {
            let m = u32::try_from(m).unwrap_or(0);
            let at = (128 * m / 7 + dice.below(7)).max(1);
            usize::try_from(at).unwrap_or(1)
        });
        Self {
            vine: Vine::new(dice),
            spurs,
            clock: 0.0,
            rise: -3.0,
            stands: seconds,
            growth: Growth::Rises,
            colour: if hero { PLANT_GREEN } else { PLANT_BLUE },
        }
    }

    /// A run of `dt` seconds (`0x798E50`): its rise grows by 3 a second to 0, it stands its
    /// seconds, and its rise falls by 3 a second until it ends below −3.
    pub fn update(&mut self, dt: f32) {
        self.clock += dt;
        match self.growth {
            Growth::Rises => {
                self.rise += 3.0 * dt;
                if self.rise >= 0.0 {
                    self.rise = 0.0;
                    self.growth = Growth::Stands;
                }
            }
            Growth::Stands => {
                self.stands -= dt;
                if self.stands <= 0.0 {
                    self.growth = Growth::Sinks;
                }
            }
            Growth::Sinks => {
                self.rise -= 3.0 * dt;
                if self.rise < -3.0 {
                    self.rise = -3.0;
                    self.growth = Growth::Done;
                }
            }
            Growth::Done => {}
        }
    }

    /// Whether it ended.
    pub fn done(&self) -> bool {
        self.growth == Growth::Done
    }

    /// The half side of its glow on the ground: `5·(rise + 3)`, 15 at its full rise.
    pub fn glow(&self) -> f32 {
        5.0 * (self.rise + 3.0)
    }

    /// Its vine's point `k` at its clock, from its foot in the models' frame: the stem's, and the
    /// sways' times `k / 128` across it.
    fn point(&self, k: usize) -> [f32; 3] {
        let [x, y, z] = self.vine.stem(k);
        let [sx, sy] = self.vine.sway(k, self.clock);
        let share = f32::from(u16::try_from(k).unwrap_or(0)) / f32::from(VINE_POINTS);
        [x + share * sx, y + share * sy, z]
    }

    /// Its dots, each a centre from its foot in the models' frame and a half side (`0x798F10`):
    /// the vine's where its half side, its width `1 + (128 − k)/64` and its rise, is above 0, and
    /// at each of its seven spurs' points so drawn while its rise is above −1, a spur's dots 0.75
    /// apart straight on along the vine from the point before, of the half side `1.5 + rise`
    /// falling by 0.075 a dot while above 0.5, 14 at its full rise.
    pub fn dots(&self) -> Vec<([f32; 3], f32)> {
        let mut dots = Vec::new();
        let mut spurs = self.spurs.iter().copied().peekable();
        for k in 0..usize::from(VINE_POINTS) {
            let width = 1.0 + f32::from(VINE_POINTS - u16::try_from(k).unwrap_or(0)) / 64.0;
            let half = width + self.rise;
            if half <= 0.0 {
                continue;
            }
            let at = self.point(k);
            dots.push((at, half));
            if k == 0 || spurs.peek() != Some(&k) {
                continue;
            }
            spurs.next();
            if self.rise <= -1.0 {
                continue;
            }
            let before = self.point(k - 1);
            let along = [0, 1, 2].map(|i| at[i] - before[i]);
            let length = along.iter().map(|v| v * v).sum::<f32>().sqrt();
            let along = if length < 1e-7 {
                [0.0; 3]
            } else {
                along.map(|v| v / length)
            };
            let mut n = 1.0_f32;
            loop {
                let half = 1.5 + self.rise - 0.075 * (n - 1.0);
                if half <= 0.5 {
                    break;
                }
                dots.push(([0, 1, 2].map(|i| at[i] + 0.75 * n * along[i]), half));
                n += 1.0;
            }
        }
        dots
    }
}

/// The thrown ball's model and its glows' texture (867, `0x79E630`), and its particles.
pub const BALL_MODEL: &str = "BOTTLE01.GRN";
pub const BALL_TEXTURE: &str = "PARTICLE_GLOW03";
const BALL_COUNT: usize = 100;

/// The colour of a thrown ball's glows by its variant, the art or the spell that throws it
/// (`0x79EA50`, the event's `+0x3C`): the Dark Elf's traps 1049 and 1050 a pale cyan, 1051 a grey,
/// 1052 a green and 1053 a light grey, the goblin mages' spell 53 white; none for another.
pub fn ball_colour(variant: u16) -> Option<[f32; 4]> {
    let [r, g, b] = match variant {
        1049 | 1050 => [0x80, 0xF0, 0xFF],
        1051 => [0xA0, 0xA0, 0xA0],
        1052 => [0x20, 0xFF, 0x20],
        1053 => [0xC0, 0xC0, 0xC0],
        53 => [0xFF, 0xFF, 0xFF],
        _ => return None,
    }
    .map(|c: u8| f32::from(c) / 255.0);
    Some([r, g, b, 1.0])
}

/// A thrown ball (867, `FX_BALL`, `cParticleSystem_ball`, its constructor `0x79E630`, start
/// `0x79EA50`, run `0x79E730` and draw `0x79E900`; #1092): its model [`BALL_MODEL`] tumbling
/// about the models' x and y at 2 to 4 radians a second each, rolled as it is thrown, and a
/// trail of glows of [`BALL_TEXTURE`] behind it, one every 0.005 s at the detail 2, of the size
/// 7 ± 2 falling by 30 a second and the buoyancy 0 ± 5, within 3 of its place along each axis,
/// their life falling by 350 a second, added by their alpha in its colour times their life (the
/// billboards' flags 3). Landed, it makes no more and ends with its last glow, its model drawn
/// till then.
#[derive(Clone, Debug, PartialEq)]
pub struct TrapBall {
    pub system: System,
    pub colour: [f32; 4],
    pub turns: [f32; 2],
    rates: [f32; 2],
    out: bool,
}

impl TrapBall {
    /// A ball of the colour `colour`, its two rates rolled by `dice`, `2·(1 + U)` each.
    pub fn new(colour: [f32; 4], dice: &mut impl Dice) -> Self {
        let rates = std::array::from_fn(|_| 2.0 * (1.0 + share(dice)));
        Self {
            system: System::new(BALL_COUNT),
            colour,
            turns: [0.0; 2],
            rates,
            out: false,
        }
    }

    fn layer() -> Layer {
        Layer {
            life: -350.0,
            size: -30.0,
            ..Layer::default()
        }
    }

    fn block() -> Block {
        Block {
            buoyancy: spread(0.0, 5.0),
            size: spread(7.0, 2.0),
            offset: spread([0.0; 3], [3.0; 3]),
            interval: 0.005,
            ..Block::default()
        }
    }

    /// A run of `dt` seconds, its place gone from `from` to `to` in the models' frame
    /// (`0x79E730`): its glows age, it leaves new ones along its way while it flies, and it
    /// tumbles.
    pub fn update(&mut self, dt: f32, (from, to): ([f32; 3], [f32; 3]), dice: &mut impl Dice) {
        let layers = [Self::layer()];
        self.system.update(dt, &layers);
        if !self.out {
            self.system
                .emit_along(&[Self::block()], &layers, BALL_COUNT, (from, to), dt, dice);
        }
        for (turn, rate) in self.turns.iter_mut().zip(self.rates) {
            *turn += dt * rate;
        }
    }

    /// Its landing (its object's `+0x74` bit 2): it makes no more glows.
    pub fn end(&mut self) {
        self.out = true;
    }

    /// Whether it ended: landed and no glow alive.
    pub fn done(&self) -> bool {
        self.out && self.system.particles.is_empty()
    }

    /// A glow's colour: its colour, its alpha times its life's share.
    pub fn glow_colour(&self, life: f32) -> [f32; 4] {
        let [r, g, b, a] = self.colour;
        [r, g, b, a * (life / 255.0).clamp(0.0, 1.0)]
    }
}

/// The tentacles' textures: their tendrils' dots and their runes, `fx_tentacle_rune1` to 15.
pub const TENTACLE_TEXTURE: &str = "particle_glow01";
pub const TENTACLE_RUNES: u8 = 15;
/// The runes an object of the tentacles rolls, one a segment of its rings, each of the 15 at
/// random (`0x7B0DF0`'s `+0x7CEC`: `rand() % 15 + 1`).
const RUNE_ROLLS: usize = 50;

/// The name of the tentacles' rune `rune`, 1 to 15.
pub fn tentacle_rune(rune: u8) -> String {
    format!("fx_tentacle_rune{rune}")
}

/// A ring of runes flat on the ground (`0x7638E0`): its inner and outer radius, its segments,
/// each of the next of its object's runes from the first, and its turn about the vertical.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RuneRing {
    pub inner: f32,
    pub outer: f32,
    pub segments: u16,
    pub turn: f32,
}

impl RuneRing {
    /// Its segments, each its index among its object's runes and its corners in the models'
    /// frame from its centre, at the angles `θ` from `(i − 1)·2π/n` to `i·2π/n` turned by its
    /// turn, `(sin θ, cos θ)` times its radii: the inner corner, the outer, the next outer and
    /// the next inner; the texture's `u` runs along it, its `v` from the outer edge in.
    pub fn segments(&self) -> Vec<(usize, [[f32; 2]; 4])> {
        let n = f32::from(self.segments.max(1));
        let edge = |i: u16, radius: f32| {
            let angle = std::f32::consts::TAU * f32::from(i) / n - self.turn;
            [radius * angle.sin(), radius * angle.cos()]
        };
        (0..self.segments)
            .map(|i| {
                (
                    usize::from(i),
                    [
                        edge(i, self.inner),
                        edge(i, self.outer),
                        edge(i + 1, self.outer),
                        edge(i + 1, self.inner),
                    ],
                )
            })
            .collect()
    }
}

/// An object's runes rolled by `dice`, 1 to 15 each.
fn runes(dice: &mut impl Dice) -> Vec<u8> {
    (0..RUNE_ROLLS)
        .map(|_| u8::try_from(dice.below(u32::from(TENTACLE_RUNES))).unwrap_or(0) + 1)
        .collect()
}

/// The four rings of the tentacles' aura about the Daemon (1013 of the variant 0, `0x7B14F0`):
/// their radii, their segments and their turns in radians a second.
const AURA_RINGS: [(f32, f32, u16, f32); 4] = [
    (25.0, 32.0, 17, -0.873_88),
    (30.0, 39.0, 22, 0.783_45),
    (37.0, 46.0, 27, -0.543_21),
    (44.0, 53.0, 32, 0.376_46),
];
/// The two rings under a tentacle (1013 of the variant 1), at their full size.
const TENTACLE_RINGS: [(f32, f32, u16, f32); 2] =
    [(25.0, 32.0, 17, 0.584_63), (30.0, 39.0, 17, -0.752_35)];
/// A tentacle's tendrils.
const TENDRILS: usize = 5;

/// The rings of `rings` at the clock `t`, their radii times `size`.
fn rune_rings(rings: &[(f32, f32, u16, f32)], t: f32, size: f32) -> Vec<RuneRing> {
    rings
        .iter()
        .map(|&(inner, outer, segments, turn)| RuneRing {
            inner: inner * size,
            outer: outer * size,
            segments,
            turn: turn * t,
        })
        .collect()
}

/// The tentacles' aura about the Daemon (1013, `FX_DAYOFTHETENTACLE`,
/// `cParticleSystem_dayofthetentacle` of the variant 0, its constructor `0x7B0DF0`, start
/// `0x7B1990`, run `0x7B1480` and draw `0x7B14F0`; #1092): four rings of black runes laid over
/// the ground about his feet by their alpha, turning against each other, for his effect's
/// seconds and its sink's second, whole until it ends.
#[derive(Clone, Debug, PartialEq)]
pub struct TentacleAura {
    pub runes: Vec<u8>,
    clock: f32,
    left: f32,
}

impl TentacleAura {
    /// An aura of `seconds`, its runes rolled by `dice`.
    pub fn new(seconds: f32, dice: &mut impl Dice) -> Self {
        Self {
            runes: runes(dice),
            clock: 0.0,
            left: seconds + crate::spells::TENTACLE_SINKS,
        }
    }

    /// A run of `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        self.clock += dt;
        self.left -= dt;
    }

    /// Whether it ended.
    pub fn done(&self) -> bool {
        self.left <= 0.0
    }

    /// Its rings now.
    pub fn rings(&self) -> Vec<RuneRing> {
        rune_rings(&AURA_RINGS, self.clock, 1.0)
    }
}

/// A tentacle at a corpse (1013 of the variant 1, `0x7B0DF0`, `0x7B1990`, `0x7B1480`,
/// `0x7B14F0`; #1092): five tendrils of black dots of [`TENTACLE_TEXTURE`] facing the camera,
/// each a vine scaled from its foot, `(k/128)·(stem + sways)`, so that its dots crowd at its foot
/// and it rises 63 high, its foot `0.2·o` and its tip `1.2·o` from the corpse, `o` its offset of
/// up to 40 along each axis, of the half side `1 + (128 − k)/51.2` from 3.5 to 1, over two rings
/// of black runes turning against each other, all laid over the scene by their alpha. It stands
/// whole for its seconds, then sinks for a second, its dots' half sides falling by 3 a second and
/// its rings shrinking to nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct Tentacle {
    pub runes: Vec<u8>,
    vines: Vec<(Vine, [f32; 2])>,
    clock: f32,
    seconds: f32,
    sink: f32,
}

impl Tentacle {
    /// A tentacle that stands `seconds` before it sinks, of the rolls of `dice`: its runes, then
    /// each tendril's vine and its foot's offset, along y first.
    pub fn new(seconds: f32, dice: &mut impl Dice) -> Self {
        let runes = runes(dice);
        let vines = (0..TENDRILS)
            .map(|_| {
                let vine = Vine::new(dice);
                let y = 80.0 * share(dice) - 40.0;
                let x = 80.0 * share(dice) - 40.0;
                (vine, [x, y])
            })
            .collect();
        Self {
            runes,
            vines,
            clock: 0.0,
            seconds,
            sink: 0.0,
        }
    }

    /// A run of `dt` seconds (`0x7B1480`): past its seconds its sink falls by 3 a second.
    pub fn update(&mut self, dt: f32) {
        self.clock += dt;
        if self.clock > self.seconds {
            self.sink -= 3.0 * dt;
        }
    }

    /// Whether it ended: sunk below −3.
    pub fn done(&self) -> bool {
        self.sink < -3.0
    }

    /// Its rings now, of their full size times `(sink + 3)/3`.
    pub fn rings(&self) -> Vec<RuneRing> {
        let size = ((self.sink + 3.0) / 3.0).max(0.0);
        rune_rings(&TENTACLE_RINGS, self.clock, size)
    }

    /// Its tendrils' dots, each a centre from the corpse in the models' frame and a half side,
    /// where that is above 0.
    pub fn dots(&self) -> Vec<([f32; 3], f32)> {
        let points = f32::from(VINE_POINTS);
        let mut dots = Vec::new();
        for (vine, [ox, oy]) in &self.vines {
            for k in 0..usize::from(VINE_POINTS) {
                let index = u16::try_from(k).unwrap_or(0);
                let half = 1.0 + f32::from(VINE_POINTS - index) / 51.2 + self.sink;
                if half <= 0.0 {
                    continue;
                }
                let share = f32::from(index) / points;
                let [x, y, z] = vine.stem(k);
                let [sx, sy] = vine.sway(k, self.clock);
                let foot = share + 0.2;
                dots.push((
                    [
                        share * (x + sx) + foot * ox,
                        share * (y + sy) + foot * oy,
                        share * z,
                    ],
                    half,
                ));
            }
        }
        dots
    }
}

/// The whirlwind's texture and particles (831, `0x789E10`), and the fire bolt's (922,
/// `0x7AA980`).
pub const WINDSTRIKE_TEXTURE: &str = "PARTICLE_CLOUD01";
pub const SHADDAR_TEXTURE: &str = "PARTICLE_SMOKE03";
const WINDSTRIKE_COUNT: usize = 200;
const SHADDAR_COUNT: usize = 200;

/// Which of the two bolts' trails: the Mage's whirlwind's dust (831, `FX_WINDSTRIKE`, of the spell
/// 29) or the fire bolt's green smoke (922, `FX_SHADDAR_MAGIC`, of the creatures' spell 52).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrailKind {
    Windstrike,
    Shaddar,
}

/// A bolt's trail of particles (#1092): the whirlwind's (831, `cParticleSystem_windstrike`, its
/// start `0x78A170`, run `0x789EA0` and draw `0x789F50`), puffs of [`WINDSTRIKE_TEXTURE`] in the
/// colour of the ground under its caster, one every 0.01 s at the detail 2, shot up at 350 ± 50
/// and falling back at the buoyancy 700, of the size 10 ± 5 growing by 30 a second, within 15 of
/// its way, living 0.64 s, laid over the scene by their life's alpha and tinted by the light, left
/// in the world along its way, a column of dust about 90 high behind it, gone at once at a cell
/// that stops it; or the fire bolt's (922, `cParticleSystem_shaddar_magic`, `0x7AAC20`,
/// `0x7AAA60`, `0x7AAB90`), puffs of [`SHADDAR_TEXTURE`]'s first frame riding with it, one every
/// 0.05 s, of the size 12 ± 6 growing by 13 a second, flying off at up to 18 along each axis and
/// turning, living 0.57 s, of the ramp [`BoltTrail::ramp`] added by their alpha, a cloud of green
/// smoke about it that fades where it stops.
#[derive(Clone, Debug, PartialEq)]
pub struct BoltTrail {
    pub system: System,
    pub kind: TrailKind,
    out: bool,
}

impl BoltTrail {
    /// A trail of `kind` as its bolt starts.
    pub fn new(kind: TrailKind) -> Self {
        let count = match kind {
            TrailKind::Windstrike => WINDSTRIKE_COUNT,
            TrailKind::Shaddar => SHADDAR_COUNT,
        };
        Self {
            system: System::new(count),
            kind,
            out: false,
        }
    }

    fn layer(&self) -> Layer {
        match self.kind {
            TrailKind::Windstrike => Layer {
                life: -400.0,
                size: 30.0,
                ..Layer::default()
            },
            TrailKind::Shaddar => Layer {
                life: -450.0,
                size: 13.0,
                ..Layer::default()
            },
        }
    }

    fn block(&self) -> Block {
        match self.kind {
            TrailKind::Windstrike => Block {
                buoyancy: spread(700.0, 0.0),
                size: spread(10.0, 5.0),
                angle: spread(std::f32::consts::PI, std::f32::consts::PI),
                velocity: spread([0.0, 0.0, 350.0], [0.0, 0.0, 50.0]),
                offset: spread([0.0; 3], [15.0, 15.0, 0.0]),
                interval: 0.01,
                ..Block::default()
            },
            TrailKind::Shaddar => Block {
                size: spread(12.0, 6.0),
                angle: spread(std::f32::consts::PI, std::f32::consts::PI),
                spin: spread(0.0, 10.0),
                velocity: spread([0.0; 3], [18.0; 3]),
                interval: 0.05,
                ..Block::default()
            },
        }
    }

    /// The fire bolt's puffs' ramp (`+0x20B0`), at the alpha 0.781: from (0.332, 0.980, 0.758) to
    /// (0.555, 0.984, 0.645) over 76 entries, to (0.129, 0.996, 0.641) over 66, to nothing over
    /// 114.
    pub fn ramp() -> Ramp {
        Ramp::new(
            [0.332, 0.980, 0.758, 0.781],
            &[
                ([0.555, 0.984, 0.645, 0.781], 76),
                ([0.129, 0.996, 0.641, 0.781], 66),
                ([0.129, 0.996, 0.641, 0.0], 114),
            ],
        )
    }

    /// A run of `dt` seconds, its bolt gone from `from` to `to` in the models' frame
    /// (`0x789EA0`, `0x7AAA60`): its puffs age, and while its bolt flies it makes new ones, the
    /// whirlwind's along its way, the fire bolt's about its origin, riding with it.
    pub fn update(&mut self, dt: f32, (from, to): ([f32; 3], [f32; 3]), dice: &mut impl Dice) {
        let layers = [self.layer()];
        self.system.update(dt, &layers);
        if self.out {
            return;
        }
        let blocks = [self.block()];
        match self.kind {
            TrailKind::Windstrike => {
                self.system
                    .emit_along(&blocks, &layers, WINDSTRIKE_COUNT, (from, to), dt, dice);
            }
            TrailKind::Shaddar => self.system.emit(&blocks, &layers, SHADDAR_COUNT, dice),
        }
    }

    /// Its bolt's end: it makes no more; the whirlwind's at a cell that stops it, `at_once`, goes
    /// at once with its puffs (`+0x74` bit 2).
    pub fn end(&mut self, at_once: bool) {
        self.out = true;
        if at_once && self.kind == TrailKind::Windstrike {
            self.system.particles.clear();
        }
    }

    /// Whether it ended: out and no puff alive.
    pub fn done(&self) -> bool {
        self.out && self.system.particles.is_empty()
    }
}

/// The ice splinters' model and textures (839, `0x78CA30`): the icicle (the first of the three
/// it loads, the one drawn), the hero's icicles' texture and the others', the rings' and the
/// sparks'.
pub const SPLINTER_MODEL: &str = "ICICLE01.GRN";
pub const SPLINTER_TEXTURE: &str = "ICICLE";
pub const SPLINTER_OTHERS_TEXTURE: &str = "ICICLE2";
pub const SPLINTER_RING_TEXTURE: &str = "FX_CIRCLE02";
pub const SPLINTER_SPARK_TEXTURE: &str = "PARTICLE_SPARK07";
const SPLINTER_SHARDS: usize = 5;
const SPLINTER_COUNT: usize = 200;
/// How many of the places it passed each icicle draws a ring at (of the ten it keeps).
pub const SPLINTER_RINGS: usize = 4;

/// The ice splinters (839, `cParticleSystem_icebullets`, `0x78D640`, `0x78CD10`, `0x78CF50`):
/// five icicles about their place, each within 20 of it along each axis, laid along their flight
/// and turning about it at a radian a second, each trailing rings at the places they passed, one
/// every 0.1 s; at their end they burst into 40 sparks each, which fall and bounce off the
/// ground.
#[derive(Clone, Debug, PartialEq)]
pub struct IceSplinters {
    pub system: System,
    /// The icicles' offsets from their place.
    pub offsets: [[f32; 3]; SPLINTER_SHARDS],
    /// How long they flew, their turn about their flight in radians.
    pub clock: f32,
    /// The time to the next ring's place, from 1 down by 10 a second (`+0x265C`).
    pub timer: f32,
    /// The places they passed, the newest first (`+0x25E4`).
    pub history: [Option<[f32; 3]>; SPLINTER_RINGS],
    pub hero: bool,
    burst: bool,
}

impl IceSplinters {
    /// The splinters leaving `start` in the models' frame, a hero's or another's.
    pub fn new(start: [f32; 3], hero: bool, dice: &mut impl Dice) -> Self {
        let mut history = [None; SPLINTER_RINGS];
        history[0] = Some(start);
        Self {
            system: System::new(SPLINTER_COUNT),
            offsets: std::array::from_fn(|_| spread_3(spread([0.0; 3], [20.0; 3]), dice)),
            clock: 0.0,
            timer: 1.0,
            history,
            hero,
            burst: false,
        }
    }

    /// Whether they still fly: drawn as icicles and rings, not burst.
    pub fn flying(&self) -> bool {
        !self.burst
    }

    /// The sparks' ramp (`+0x20A8`): each entry `i` a hero's blue white `(⌊i/2⌋, ⌊i/2⌋, i)`,
    /// another's grey `⌊i/4⌋`, in 255ths, its alpha 0 (drawn whole).
    pub fn ramp(&self) -> Ramp {
        let hero = self.hero;
        Ramp(std::array::from_fn(|i| {
            let i = f32::from(u8::try_from(i).unwrap_or(u8::MAX));
            if hero {
                let half = (i / 2.0).floor();
                [half / 255.0, half / 255.0, i / 255.0, 0.0]
            } else {
                let grey = (i / 4.0).floor() / 255.0;
                [grey, grey, grey, 0.0]
            }
        }))
    }

    /// A run of `dt` seconds at `place` in the models' frame, `ground` the ground's height under
    /// it: they turn, and while they fly take a ring's place every 0.1 s; their sparks age,
    /// their upward velocity turned back at the ground (`0x7645C0` of the mode 0).
    pub fn update(&mut self, dt: f32, place: [f32; 3], ground: f32) {
        self.clock += dt;
        if !self.burst {
            self.timer -= 10.0 * dt;
            if self.timer <= 0.0 {
                self.history.rotate_right(1);
                self.history[0] = Some(place);
                self.timer = 1.0;
            }
        }
        let layers = [Layer {
            life: -150.0,
            ..Layer::default()
        }];
        self.system.update_over(dt, &layers, Some(ground));
    }

    /// The rings to draw: each icicle's at each place it passed, its centre, its half side
    /// `2·((2 − timer) + j)` for the `j`-th place back, and its colour's share, the last fading
    /// by the timer; none once they burst.
    pub fn rings(&self) -> Vec<([f32; 3], f32, f32)> {
        if self.burst {
            return Vec::new();
        }
        let mut rings = Vec::new();
        for offset in &self.offsets {
            for (j, place) in self.history.iter().enumerate() {
                let Some(place) = place else {
                    continue;
                };
                // A place not yet passed is skipped, as the original skips one of x and y 0.
                let j = f32::from(u8::try_from(j).unwrap_or(0));
                let share = if j >= 3.0 { self.timer } else { 1.0 };
                rings.push((
                    std::array::from_fn(|k| place[k] + offset[k]),
                    2.0 * ((2.0 - self.timer) + j),
                    share,
                ));
            }
        }
        rings
    }

    /// Their end at `place`, flying along the unit `direction`, both in the models' frame, at
    /// their time's end or at a cell that stops them: the icicles go, 40 sparks bursting from
    /// each, flying on at 70 along their way within 10 along each axis.
    pub fn burst(&mut self, place: [f32; 3], direction: [f32; 3], dice: &mut impl Dice) {
        if self.burst {
            return;
        }
        self.burst = true;
        let per = SPLINTER_COUNT / SPLINTER_SHARDS;
        for offset in self.offsets {
            let block = Block {
                buoyancy: spread(140.0, 20.0),
                size: spread(2.0, 0.0),
                velocity: spread(direction.map(|d| 70.0 * d), [10.0; 3]),
                offset: spread(std::array::from_fn(|k| place[k] + offset[k]), [2.0; 3]),
                ..Block::default()
            };
            self.system.burst(&block, per, SPLINTER_COUNT, dice);
        }
    }

    /// Whether they ended: burst and no spark alive.
    pub fn done(&self) -> bool {
        self.burst && self.system.particles.is_empty()
    }
}

/// The Wood Elf's bubble's model and textures (847, `0x797B30`): the bush, the hero's texture
/// and the others', its burst's glows; and its burst's sonar ring's (906, `0x798460`).
pub const BUBBLE_MODEL: &str = "ELVESTRAUCH.GRN";
pub const BUBBLE_TEXTURE: &str = "ELVESTRAUCH";
pub const BUBBLE_OTHERS_TEXTURE: &str = "ELVESTRAUCH2";
pub const BUBBLE_GLOW_TEXTURE: &str = "PARTICLE_GLOW03";
pub const SONAR_TEXTURE: &str = "FX_SONAREFFECT";
const BUBBLE_GLOWS: usize = 75;

/// The bubble's stages (`+0x20B1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BubbleStage {
    Grows,
    Flies,
    Shrinks,
    Burst,
}

/// The Wood Elf's bubble (847, `cParticleSystem_bubbletrap`, `0x798380`, `0x797D80`,
/// `0x798030`): its bush grows to its size in a third of a second, flies, and shrinks away over
/// its last half second, or at once where a cell stops it; an arrow that bursts it leaves 75
/// green glows drifting about where it was.
#[derive(Clone, Debug, PartialEq)]
pub struct BubbleTrap {
    pub system: System,
    pub clock: f32,
    /// Its bush's share of its size, 0 to 1 (`+0x20AC`).
    pub scale: f32,
    seconds: f32,
    stage: BubbleStage,
}

impl BubbleTrap {
    /// A bubble of `seconds` seconds.
    pub fn new(seconds: f32) -> Self {
        Self {
            system: System::new(BUBBLE_GLOWS),
            clock: 0.0,
            scale: 0.0,
            seconds,
            stage: BubbleStage::Grows,
        }
    }

    /// Whether its bush shows, not burst and not shrunk away.
    pub fn drawn(&self) -> bool {
        self.stage != BubbleStage::Burst && self.scale > 0.0
    }

    /// The glows' ramp (`+0x20B4`): each entry `i` green, `(⌊i/2⌋, i, ⌊i/2⌋)` in 255ths, its alpha
    /// 0 (drawn whole).
    pub fn ramp() -> Ramp {
        Ramp(std::array::from_fn(|i| {
            let i = f32::from(u8::try_from(i).unwrap_or(u8::MAX));
            let half = (i / 2.0).floor() / 255.0;
            [half, i / 255.0, half, 0.0]
        }))
    }

    /// A run of `dt` seconds: its bush grows by 3 a second to its size, shrinks by 2 a second
    /// from half a second before its seconds' end, and its glows age.
    pub fn update(&mut self, dt: f32) {
        self.clock += dt;
        match self.stage {
            BubbleStage::Grows => {
                self.scale = (self.scale + 3.0 * dt).min(1.0);
                if self.scale >= 1.0 {
                    self.stage = BubbleStage::Flies;
                }
            }
            BubbleStage::Flies if self.clock > self.seconds - 0.5 => {
                self.stage = BubbleStage::Shrinks;
            }
            BubbleStage::Shrinks => self.scale = (self.scale - 2.0 * dt).max(0.0),
            _ => {}
        }
        let layers = [Layer {
            life: -150.0,
            ..Layer::default()
        }];
        self.system.update(dt, &layers);
    }

    /// Its stop at a cell it may not pass (`+0x74` bit 2): it shrinks away where it is, without
    /// a burst.
    pub fn stop(&mut self) {
        if self.stage != BubbleStage::Burst {
            self.stage = BubbleStage::Shrinks;
        }
    }

    /// Its burst at `at` in the models' frame (`+0x74` bit 8): its bush goes, 75 glows within 10
    /// of it, drifting by up to 10 a second along each axis.
    pub fn burst(&mut self, at: [f32; 3], dice: &mut impl Dice) {
        if self.stage == BubbleStage::Burst {
            return;
        }
        self.stage = BubbleStage::Burst;
        let block = Block {
            buoyancy: spread(0.0, 10.0),
            size: spread(10.0, 3.0),
            velocity: spread([0.0; 3], [10.0; 3]),
            offset: spread(at, [10.0; 3]),
            ..Block::default()
        };
        self.system.burst(&block, BUBBLE_GLOWS, BUBBLE_GLOWS, dice);
    }

    /// Whether it ended: shrunk away, or burst and no glow alive.
    pub fn done(&self) -> bool {
        match self.stage {
            BubbleStage::Shrinks => self.scale <= 0.0,
            BubbleStage::Burst => self.system.particles.is_empty(),
            _ => false,
        }
    }
}

/// The bubble's burst's sonar ring (906, `cParticleSystem_bubbletrap_explo`, `0x7985D0`,
/// `0x7984F0`, `0x798530`): a flat green ring on the ground from the radius 10, 30 wide, growing
/// by 250 a second and fading for half a second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SonarRing {
    pub clock: f32,
}

impl SonarRing {
    /// A run of `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        self.clock += dt;
    }

    /// Its inner and outer radii.
    pub fn radii(&self) -> (f32, f32) {
        let inner = 10.0 + 250.0 * self.clock;
        (inner, inner + 30.0)
    }

    /// Its colour, `(⌊128 f⌋, ⌊255 f⌋, ⌊128 f⌋)` for `f = 2·(0.5 − t)`, drawn whole.
    pub fn colour(&self) -> [f32; 4] {
        let f = (2.0 * (0.5 - self.clock)).clamp(0.0, 1.0);
        let byte = |most: f32| (most * f).floor() / 255.0;
        [byte(128.0), byte(255.0), byte(128.0), 1.0]
    }

    /// Whether it ended: past half a second.
    pub fn done(&self) -> bool {
        self.clock > 0.5
    }
}

/// The Dark Elf's soulcatcher's twirls' and glow's textures (869, `0x76A6D0`), its particles, its
/// height over its bone, and its glow's colour `0xFFFFEF80`.
pub const SOULCATCHER_TEXTURE: &str = "PARTICLE_TWIRL01";
pub const SOULCATCHER_GLOW_TEXTURE: &str = "PARTICLE_SPARK07";
const SOULCATCHER_COUNT: usize = 100;
pub const SOULCATCHER_HIGH: f32 = 40.0;
pub const SOULCATCHER_GLOW: [f32; 4] = [1.0, 239.0 / 255.0, 128.0 / 255.0, 1.0];

/// The Dark Elf's soulcatcher (869, `cParticleSystem_soulcatcher`, `0x76AA40`, `0x76A860`,
/// `0x76A960`): twirls spinning 40 over his neck, four in five of a layer added in gold, one in
/// five of a longer one laid over in black, riding with him; each soul it absorbs swells them by
/// 20 and lights a pale yellow glow of a half side of twice that, both falling by 13 a second.
#[derive(Clone, Debug, PartialEq)]
pub struct Soulcatcher {
    pub system: System,
    /// The swelling of its last soul (`+0x29B4`).
    pub pulse: f32,
}

impl Default for Soulcatcher {
    fn default() -> Self {
        Self {
            system: System::new(SOULCATCHER_COUNT),
            pulse: 0.0,
        }
    }
}

impl Soulcatcher {
    /// Its two layers (`+0x28AC`): the gold of the life −500 a second (0.51 s), the black of −250
    /// (1.02 s).
    fn layers() -> [Layer; 2] {
        [-500.0, -250.0].map(|life| Layer {
            life,
            ..Layer::default()
        })
    }

    /// Its two ramps (`+0x20A4`, `+0x24A4`): the gold (0.977, 0.883, 0.1875) and black, each
    /// from nothing to its alpha, 0.25 and 0.996, over 128 entries and back to nothing over 128.
    pub fn ramps() -> [Ramp; 2] {
        [([0.977, 0.883, 0.1875], 0.25), ([0.0; 3], 0.996)].map(|([r, g, b], alpha)| {
            Ramp::new(
                [r, g, b, 0.0],
                &[([r, g, b, alpha], 128), ([r, g, b, 0.0], 128)],
            )
        })
    }

    /// A run of `dt` seconds, its bone at `at` in the models' frame from the place it rides with
    /// (`0x76A860`): its twirls age, its blocks' size is its pulse and 20, its pulse falls by 13
    /// a second, and it makes twirls at 40 over its bone, one every 0.02 s.
    pub fn update(&mut self, dt: f32, at: [f32; 3], dice: &mut impl Dice) {
        let layers = Self::layers();
        self.system.update(dt, &layers);
        let block = Block {
            size: spread(self.pulse + 20.0, 8.0),
            angle: spread(std::f32::consts::PI, std::f32::consts::PI),
            spin: spread(6.0, 2.0),
            offset: spread([at[0], at[1], at[2] + SOULCATCHER_HIGH], [1.0; 3]),
            interval: 0.02,
            ..Block::default()
        };
        self.pulse = (self.pulse - 13.0 * dt).max(0.0);
        self.system
            .emit(&[block, block], &layers, SOULCATCHER_COUNT, dice);
    }

    /// A soul absorbed (`+0x74` bit 8): its pulse 20.
    pub fn absorb(&mut self) {
        self.pulse = 20.0;
    }

    /// Its glow's half side while its pulse lasts (`0x762E10` of the style 4): twice its pulse.
    pub fn glow(&self) -> Option<f32> {
        (self.pulse > 0.0).then_some(2.0 * self.pulse)
    }
}

/// The souls' texture, colour and particles (780, `cParticleSystem_puzzleSolved`, `0x76C930`): its
/// corners' `0x00A0FFB0`, drawn whole.
pub const SOUL_SPARK_TEXTURE: &str = "PARTICLE_SPARK06";
pub const SOUL_COLOUR: [f32; 4] = [160.0 / 255.0, 1.0, 176.0 / 255.0, 1.0];
pub const SOUL_SPARKS: usize = 500;
/// How long a soul's sparks live at most.
const SOUL_SECONDS: f32 = 10.0;

/// A soul homing on a soulcatcher (780, `0x76CD50`, `0x76C9D0`, `0x76CB10` with the flag
/// `0x4000`): the dying creature's figure in pale green sparks, one at each of its vertices it
/// takes, rising and drawn to its place by a pull of 50 that grows by 100 a second, their size 5
/// shrinking by 2 a second, that rate rising by 0.5 a second to 0; where it arrives or a cell
/// stops it they stop and fade out in 1.02 s; it ends past 10 s.
#[derive(Clone, Debug, PartialEq)]
pub struct SoulSparks {
    pub system: System,
    clock: f32,
    pull: f32,
    size: f32,
    life: f32,
}

impl SoulSparks {
    /// A soul of sparks at `points` in the models' frame, the first [`SOUL_SPARKS`] of them
    /// (`0x767FB0`).
    pub fn new(points: impl IntoIterator<Item = [f32; 3]>) -> Self {
        let mut system = System::new(SOUL_SPARKS);
        system.particles = points
            .into_iter()
            .take(SOUL_SPARKS)
            .map(|place| Particle {
                place,
                velocity: [0.0; 3],
                buoyancy: -50.0,
                size: 5.0,
                life: 255.0,
                angle: 0.0,
                spin: 0.0,
                layer: 0,
                frame: 0,
            })
            .collect();
        Self {
            system,
            clock: 0.0,
            pull: 50.0,
            size: -2.0,
            life: 0.0,
        }
    }

    /// A run of `dt` seconds, its place at `centre` in the models' frame: its pull grows by 100 a
    /// second and its size's rate by 0.5 to 0, and its sparks age, drawn toward its place.
    pub fn update(&mut self, dt: f32, centre: [f32; 3]) {
        self.clock += dt;
        self.pull += 100.0 * dt;
        self.size = (self.size + 0.5 * dt).min(0.0);
        let layers = [Layer {
            life: self.life,
            size: self.size,
            pull: self.pull,
            ..Layer::default()
        }];
        self.system.update_toward(dt, &layers, centre);
    }

    /// Its arrival, or its stop at a cell (`+0x74` bit 2): every spark stops, and they fade at
    /// 250 a second.
    pub fn arrive(&mut self) {
        for spark in &mut self.system.particles {
            spark.velocity = [0.0; 3];
        }
        self.life = -250.0;
    }

    /// Whether it ended: no spark alive, or past its 10 s.
    pub fn done(&self) -> bool {
        self.system.particles.is_empty() || self.clock > SOUL_SECONDS
    }
}

/// The homing fires' texture and particles (910 and 1164, `0x7A6A40`, `0x7BF690`).
pub const HOMING_FIRE_TEXTURE: &str = "PARTICLE_FIRE03";
const HOMING_FIRE_COUNT: usize = 300;

/// The two homing fires, of one code (`0x7A6AD0`, `0x7BF720`): the big demons' (910,
/// `FX_DAEMONFIRE`, of the spell 50) and Anducar's guards' (1164, `FX_ANDUCAR_GARDE`, of the
/// spell 70).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HomingKind {
    Demon,
    Guard,
}

/// A homing fire's flames (910 and 1164, `cParticleSystem_daemonfire` and
/// `cParticleSystem_anducar_garde`, their starts `0x7A6F50` and `0x7BFA90`, their run and draw
/// of one code; #1092): flames of [`HOMING_FIRE_TEXTURE`] at the fire, one every 0.006 s at the
/// detail 2, rising (the buoyancy −35 ± 34) and growing as they burn for 0.85 s, the demons' of
/// the size 12 ± 1.5 within 15 of it growing by 15 a second, the guards' of 6 ± 0.5 within 5
/// growing by 10, of the ramp [`HomingFlames::ramp`], turned at random, added by their alpha,
/// left in the world behind it as it flies; at its burst all its dead flames at once in a ball,
/// and none more; past its flight it makes no more and ends with its last.
#[derive(Clone, Debug, PartialEq)]
pub struct HomingFlames {
    pub system: System,
    kind: HomingKind,
    out: bool,
    burst: bool,
}

impl HomingFlames {
    /// The flames of a fire of `kind` as it starts.
    pub fn new(kind: HomingKind) -> Self {
        Self {
            system: System::new(HOMING_FIRE_COUNT),
            kind,
            out: false,
            burst: false,
        }
    }

    /// Its flames' layer: theirs as it burns, the burst's after it (its layers cleared and set
    /// anew at the burst).
    fn layer(&self) -> Layer {
        if self.burst {
            return Layer {
                life: -200.0,
                size: -15.0,
                ..Layer::default()
            };
        }
        Layer {
            life: -300.0,
            size: match self.kind {
                HomingKind::Demon => 15.0,
                HomingKind::Guard => 10.0,
            },
            ..Layer::default()
        }
    }

    fn block(&self) -> Block {
        let (size, offset) = match self.kind {
            HomingKind::Demon => (spread(12.0, 1.5), 15.0),
            HomingKind::Guard => (spread(6.0, 0.5), 5.0),
        };
        Block {
            buoyancy: spread(-35.0, 34.0),
            size,
            angle: spread(std::f32::consts::PI, std::f32::consts::PI),
            velocity: spread([0.0; 3], [0.5; 3]),
            offset: spread([0.0; 3], [offset; 3]),
            interval: 0.006,
            ..Block::default()
        }
    }

    /// Its flames' ramp (`+0x20AC`): from nothing to a pale yellow at the alpha 0.352 over 5
    /// entries, to an orange red at 0.996 over 40, to a darker red at 0.898 over 29, to black at
    /// 0.547 over 85, to nothing over 97.
    pub fn ramp() -> Ramp {
        Ramp::new(
            [0.0; 4],
            &[
                ([0.996, 0.953, 0.516, 0.352], 5),
                ([0.996, 0.313, 0.156, 0.996], 40),
                ([0.910, 0.262, 0.078, 0.898], 29),
                ([0.0, 0.0, 0.0, 0.547], 85),
                ([0.0; 4], 97),
            ],
        )
    }

    /// A run of `dt` seconds, the fire gone from `from` to `to` in the models' frame: its flames
    /// age, and until it stops it leaves new ones along its way.
    pub fn update(&mut self, dt: f32, (from, to): ([f32; 3], [f32; 3]), dice: &mut impl Dice) {
        let layers = [self.layer()];
        self.system.update(dt, &layers);
        if !self.out {
            self.system.emit_along(
                &[self.block()],
                &layers,
                HOMING_FIRE_COUNT,
                (from, to),
                dt,
                dice,
            );
        }
    }

    /// Its burst at `at` in the models' frame, its contact or a cell that stops it: all its dead
    /// flames at once, of the size 12 ± 1.5 within 15 of it, flying out at up to 25 along each
    /// axis, their life falling by 200 a second and their size by 15; then none more.
    pub fn burst(&mut self, at: [f32; 3], dice: &mut impl Dice) {
        self.out = true;
        let block = Block {
            size: spread(12.0, 1.5),
            angle: spread(std::f32::consts::PI, std::f32::consts::PI),
            velocity: spread([0.0; 3], [25.0; 3]),
            offset: spread(at, [15.0; 3]),
            ..Block::default()
        };
        self.burst = true;
        let room = HOMING_FIRE_COUNT.saturating_sub(self.system.particles.len());
        self.system.burst(&block, room, HOMING_FIRE_COUNT, dice);
    }

    /// Its flight's end: it makes no more.
    pub fn end(&mut self) {
        self.out = true;
    }

    /// Whether it ended: out and no flame alive.
    pub fn done(&self) -> bool {
        self.out && self.system.particles.is_empty()
    }
}

/// The demon fire's model, its lava's frames (`A_LAVA00` to `A_LAVA49`) and its rays' texture
/// (1015, `0x7B2B00`), and its particles.
pub const DEMON_FIRE_MODEL: &str = "SPHERE.GRN";
/// The sphere's radius in the models' frame, at its size 1.
pub const SPHERE_RADIUS: f32 = 20.0;
pub const LAVA_FRAMES: u8 = 50;
pub const DEMON_FIRE_RAYS: &str = "PARTICLE_LINE01";
const DEMON_FIRE_COUNT: usize = 500;

/// The name of the lava's frame `frame`, 0 to 49.
pub fn lava_frame_name(frame: u8) -> String {
    format!("A_LAVA{frame:02}")
}

/// The lava's frame at the clock `t` running at `rate` frames a second: `⌊fmod(rate·t, 49.5)⌋`, so
/// that its last frame shows half as long (`0x7B2DF0`, `0x7ADBC0`).
pub fn lava_frame(t: f32, rate: f32) -> u8 {
    let at = (rate * t).rem_euclid(49.5).floor();
    u8::try_from(crate::fight::truncated(f64::from(at))).unwrap_or(0)
}

/// The demon fire (1015, `FX_DEMONFIRE`, `cParticleSystem_demonfire`, its constructor
/// `0x7B2B00`, start `0x7B3110`, run `0x7B2CA0` and draw `0x7B2DF0`; #1092): a ball of lava
/// hanging `45 + 3 sin(1.17 t)` over its point, two spheres of [`DEMON_FIRE_MODEL`] in the lava's
/// frames at 70 a second, the inner lit and turning at a radian a second, the outer added over it
/// and turning at 1.2456, of the sizes `(0.3 + 0.05 sin t)·g` and `(0.35 + 0.05 sin t)·g`; and a
/// starburst of thin rays of [`DEMON_FIRE_RAYS`] about it, one every 0.004 s at the detail 2, of
/// the size 30 ± 7 falling by 25 a second, turned at random, within 1 of the ball's centre along
/// each axis, their life falling by 200 a second, of the ramp [`DemonFire::ramp`], added by their
/// alpha. Its growth `g` rises from 0 to 1 over 2 s, holds until it strikes no more, and falls to
/// 0 over 2 s more, when it ends; it makes rays until it strikes no more.
#[derive(Clone, Debug, PartialEq)]
pub struct DemonFire {
    pub system: System,
    strikes: f32,
    lives: f32,
}

impl DemonFire {
    /// A fire that strikes until `strikes` seconds from its start and lives `lives`
    /// (`crate::spells::hellfire_times`).
    pub fn new((strikes, lives): (f32, f32)) -> Self {
        Self {
            system: System::new(DEMON_FIRE_COUNT),
            strikes,
            lives,
        }
    }

    fn layer() -> Layer {
        Layer {
            life: -200.0,
            size: -25.0,
            ..Layer::default()
        }
    }

    fn block(centre: [f32; 3]) -> Block {
        Block {
            size: spread(30.0, 7.0),
            angle: spread(std::f32::consts::PI, std::f32::consts::PI),
            offset: spread(centre, [1.0; 3]),
            interval: 0.004,
            ..Block::default()
        }
    }

    /// Its rays' ramp (`+0x20B0`): from nothing in an orange (0.922, 0.363, 0.035) to its full
    /// alpha over 76 entries, to a yellow (0.996, 0.617, 0.25) over 66, to nothing in a brown
    /// (0.566, 0.371, 0.184) over 114.
    pub fn ramp() -> Ramp {
        Ramp::new(
            [0.922, 0.363, 0.035, 0.0],
            &[
                ([0.922, 0.363, 0.035, 0.996], 76),
                ([0.996, 0.617, 0.25, 0.996], 66),
                ([0.566, 0.371, 0.184, 0.0], 114),
            ],
        )
    }

    /// A run of `dt` seconds (`0x7B2CA0`): its rays age, and while it strikes it makes new ones
    /// about its ball's centre.
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) {
        let layers = [Self::layer()];
        self.system.update(dt, &layers);
        if self.system.clock < self.strikes {
            let block = Self::block(self.centre());
            self.system.emit(&[block], &layers, DEMON_FIRE_COUNT, dice);
        }
    }

    /// Its ball's centre over its point, in the models' frame (`+0x250C`).
    pub fn centre(&self) -> [f32; 3] {
        [0.0, 0.0, 45.0 + 3.0 * (1.17 * self.system.clock).sin()]
    }

    /// Its growth `g` (`+0x20A8`).
    pub fn growth(&self) -> f32 {
        let t = self.system.clock;
        if t <= self.strikes {
            (t / 2.0).min(1.0)
        } else {
            (1.0 - (t - self.strikes) / 2.0).max(0.0)
        }
    }

    /// Its two spheres' sizes and turns about the vertical, the inner's and the outer's.
    pub fn spheres(&self) -> [(f32, f32); 2] {
        let (t, g) = (self.system.clock, self.growth());
        let pulse = 0.05 * t.sin();
        [((0.3 + pulse) * g, t), ((0.35 + pulse) * g, 1.2456 * t)]
    }

    /// Its lava's frame now.
    pub fn frame(&self) -> u8 {
        lava_frame(self.system.clock, 70.0)
    }

    /// Whether it ended.
    pub fn done(&self) -> bool {
        self.system.clock >= self.lives
    }
}

/// The hell disk's model, at its size (1005, `0x7AD8C0`, `0x7ADBC0`), and its particles.
pub const HELL_DISK_MODEL: &str = "HELL_DISK.GRN";
pub const HELL_DISK_SCALE: f32 = 0.08;
pub const HELL_DISK_GLOW: &str = "PARTICLE_GLOW03";
const HELL_DISK_COUNT: usize = 300;
/// The colour of its glow (`0x00B07151`).
pub const HELL_DISK_GLOW_COLOUR: [f32; 4] = [176.0 / 255.0, 113.0 / 255.0, 81.0 / 255.0, 1.0];

/// The hell disk (1005, `FX_HELLDISK`, `cParticleSystem_helldisk`, its constructor `0x7AD8C0`,
/// start `0x7AE040`, run `0x7ADB10` and draw `0x7ADBC0`; #1092): its model
/// [`HELL_DISK_MODEL`] at 0.08 of its size, its rings whirling in its clip's loop, in the lava's
/// frames at 100 a second and again added over itself half a frame on; a glow of
/// [`HELL_DISK_GLOW`] about it, of the half side `30 + 10·(1 + sin t)`, added whole in
/// [`HELL_DISK_GLOW_COLOUR`]; and a trail of glows behind it, one every 0.015 s at the detail 2,
/// of the size 10 falling by 4 a second, their life falling by 200 a second, of the ramp
/// [`HellDisk::ramp`], added by their alpha. Stopped past its seconds, it makes no more and ends
/// with its last glow, its model and its glow shown till then.
#[derive(Clone, Debug, PartialEq)]
pub struct HellDisk {
    pub system: System,
    out: bool,
}

impl Default for HellDisk {
    fn default() -> Self {
        Self::new()
    }
}

impl HellDisk {
    /// A hell disk as it starts (`0x7AE040`).
    pub fn new() -> Self {
        Self {
            system: System::new(HELL_DISK_COUNT),
            out: false,
        }
    }

    fn layer() -> Layer {
        Layer {
            life: -200.0,
            size: -4.0,
            ..Layer::default()
        }
    }

    fn block() -> Block {
        Block {
            size: spread(10.0, 0.0),
            interval: 0.015,
            ..Block::default()
        }
    }

    /// Its trail's ramp (`+0x20B0`): (0.996, 0.391, 0.234) throughout, its alpha 0 for 20 entries,
    /// up to 0.781 over 30 and down to 0 over 206.
    pub fn ramp() -> Ramp {
        let [r, g, b] = [0.996, 0.391, 0.234];
        Ramp::new(
            [r, g, b, 0.0],
            &[
                ([r, g, b, 0.0], 20),
                ([r, g, b, 0.781], 30),
                ([r, g, b, 0.0], 206),
            ],
        )
    }

    /// A run of `dt` seconds, its place gone from `from` to `to` in the models' frame
    /// (`0x7ADB10`): its glows age, and until it stops it leaves new ones along its way.
    pub fn update(&mut self, dt: f32, (from, to): ([f32; 3], [f32; 3]), dice: &mut impl Dice) {
        let layers = [Self::layer()];
        self.system.update(dt, &layers);
        if !self.out {
            self.system.emit_along(
                &[Self::block()],
                &layers,
                HELL_DISK_COUNT,
                (from, to),
                dt,
                dice,
            );
        }
    }

    /// Its stop past its seconds (`+0x20AC` 1): it makes no more glows.
    pub fn end(&mut self) {
        self.out = true;
    }

    /// Whether it ended: stopped and no glow alive.
    pub fn done(&self) -> bool {
        self.out && self.system.particles.is_empty()
    }

    /// The half side of its glow now.
    pub fn glow(&self) -> f32 {
        30.0 + 10.0 * (1.0 + self.system.clock.sin())
    }

    /// Its lava's frames now, its model's and its added copy's half a frame on (`0x7ADBC0`: the
    /// second `⌊fmod(100 t + 50, 49.5)⌋`).
    pub fn frames(&self) -> [u8; 2] {
        let t = self.system.clock;
        [lava_frame(t, 100.0), lava_frame(t + 0.5, 100.0)]
    }
}

/// The pentagram's textures, its glow's colour and half side, its sparks' colour and half side,
/// and its ring's radius (782, `0x7727E0`, `0x773020`).
pub const PENTAGRAM_GLOW: &str = "PARTICLE_GLOW03";
pub const PENTAGRAM_SPARK: &str = "PARTICLE_SPARK02";
pub const PENTAGRAM_PUFF: &str = "PARTICLE_CLOUD01";
pub const PENTAGRAM_GLOW_COLOUR: [f32; 4] = [96.0 / 255.0, 192.0 / 255.0, 207.0 / 255.0, 1.0];
pub const PENTAGRAM_GLOW_HALF: f32 = 70.0;
pub const PENTAGRAM_SPARK_COLOUR: [f32; 4] = [128.0 / 255.0, 240.0 / 255.0, 1.0, 96.0 / 255.0];
pub const PENTAGRAM_SPARK_HALF: f32 = 4.0;
const PENTAGRAM_RADIUS: f32 = 40.0;
const PENTAGRAM_COUNT: usize = 300;
/// The star's points by its turn, at the ring's radius: the pentagram `{5/2}` and back to the
/// first (`+0x2544`).
const STAR: [f32; 6] = [
    0.0,
    4.0 * std::f32::consts::PI / 5.0,
    8.0 * std::f32::consts::PI / 5.0,
    2.0 * std::f32::consts::PI / 5.0,
    6.0 * std::f32::consts::PI / 5.0,
    0.0,
];

/// How a [`PentagramKiller`] goes on: it draws itself over 2 s, it lies armed, it bursts (the
/// burst's clock), it fades out unburst, and it ends.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Pentagram {
    Arming,
    Armed,
    Burst(f32),
    Expired,
    Done,
}

/// The point at the angle `angle` on a circle of `radius` in the models' frame, `(sin, cos)`.
fn on_circle(angle: f32, radius: f32) -> [f32; 3] {
    [radius * angle.sin(), radius * angle.cos(), 0.0]
}

/// The pentagram of the trap 1050 (782, `FX_PENTAGRAMMKILLER`,
/// `cParticleSystem_pentagrammKiller`, its constructor `0x7727E0`, start `0x773790`, run
/// `0x772C40` and draw `0x773020`; #1092): a glow of [`PENTAGRAM_GLOW`] flat on the ground, 140
/// across, in [`PENTAGRAM_GLOW_COLOUR`]; over its 2 s of arming a ring of radius 40 of twinkling
/// sparks of [`PENTAGRAM_SPARK`] draws itself about it and a five-pointed star of them fills in,
/// turning at a radian a second; armed, two comets of sparks chase about the ring against the
/// star; burst, the comets go, the ring and the star thin out over a second and a column of fire
/// puffs of [`PENTAGRAM_PUFF`] shoots up; expired, the ring and the star fade out over 2 s. Its
/// sparks lie anew at random each frame.
#[derive(Clone, Debug, PartialEq)]
pub struct PentagramKiller {
    pub system: System,
    seconds: f32,
    turn: f32,
    fade: f32,
    stage: Pentagram,
}

impl PentagramKiller {
    /// A pentagram that stands armed until `seconds` from its start (`0x773790`).
    pub fn new(seconds: f32) -> Self {
        Self {
            system: System::new(PENTAGRAM_COUNT),
            seconds,
            turn: 0.0,
            fade: 0.0,
            stage: Pentagram::Arming,
        }
    }

    fn layer() -> Layer {
        Layer {
            life: -650.0,
            ..Layer::default()
        }
    }

    fn block() -> Block {
        Block {
            size: spread(30.0, 0.0),
            angle: spread(std::f32::consts::PI, std::f32::consts::PI),
            velocity: spread([0.0, 0.0, 350.0], [15.0, 15.0, 150.0]),
            offset: spread([0.0; 3], [10.0, 10.0, 0.0]),
            interval: 0.003,
            ..Block::default()
        }
    }

    /// Its puffs' ramp (`+0x20BC`): white, to (1, 0.5, 0.2) over 60 entries, to nothing in
    /// (1, 0.2, 0) over 196.
    pub fn ramp() -> Ramp {
        Ramp::new(
            [1.0; 4],
            &[([1.0, 0.5, 0.2, 1.0], 60), ([1.0, 0.2, 0.0, 0.0], 196)],
        )
    }

    /// A run of `dt` seconds (`0x772C40`), its object `gone`: it arms, and gone it bursts, or
    /// fades out past its seconds.
    pub fn update(&mut self, dt: f32, gone: bool, dice: &mut impl Dice) {
        self.system.clock += dt;
        let clock = self.system.clock;
        self.turn = (self.turn + dt).rem_euclid(std::f32::consts::TAU);
        let expired = clock >= self.seconds;
        self.stage = match self.stage {
            Pentagram::Arming if gone || expired => Pentagram::Expired,
            Pentagram::Arming => {
                self.fade = (self.fade + 0.5 * dt).min(1.0);
                if self.fade >= 1.0 {
                    Pentagram::Armed
                } else {
                    Pentagram::Arming
                }
            }
            Pentagram::Armed if expired => Pentagram::Expired,
            Pentagram::Armed if gone => Pentagram::Burst(0.0),
            Pentagram::Burst(b) => {
                let b = b + dt;
                let layers = [Self::layer()];
                self.system.clock -= dt;
                self.system.update(dt, &layers);
                if b <= 1.0 {
                    self.system
                        .emit(&[Self::block()], &layers, PENTAGRAM_COUNT, dice);
                }
                if b > 1.0 && self.system.particles.is_empty() {
                    Pentagram::Done
                } else {
                    Pentagram::Burst(b)
                }
            }
            Pentagram::Expired => {
                self.fade -= 0.5 * dt;
                if self.fade <= 0.0 {
                    Pentagram::Done
                } else {
                    Pentagram::Expired
                }
            }
            other => other,
        };
    }

    /// Whether it ended.
    pub fn done(&self) -> bool {
        self.stage == Pentagram::Done
    }

    /// Its ring's and star's radius, which shrinks as it bursts (`+0x20B4`).
    fn radius(&self) -> f32 {
        match self.stage {
            Pentagram::Burst(b) => ((1.0 - b) * PENTAGRAM_RADIUS).max(0.0),
            _ => PENTAGRAM_RADIUS,
        }
    }

    /// Its sparks now, each a point from its place in the models' frame, rolled anew by `dice`:
    /// `⌊4·f·r⌋` on its ring within the share `f` of its turn that its fade draws, its comets'
    /// 80 while armed, and `⌊2·f·r⌋` along each of its star's five sides, `r` its radius.
    pub fn sparks(&self, dice: &mut impl Dice) -> Vec<[f32; 3]> {
        if self.stage == Pentagram::Done {
            return Vec::new();
        }
        let (f, r) = (self.fade, self.radius());
        let whole = |n: f32| usize::try_from(crate::fight::truncated(f64::from(n))).unwrap_or(0);
        let mut sparks = Vec::new();
        for _ in 0..whole(4.0 * f * r) {
            let angle = share(dice) * f * std::f32::consts::TAU;
            sparks.push(on_circle(angle, PENTAGRAM_RADIUS));
        }
        if self.stage == Pentagram::Armed {
            let head = 1.0 - self.turn;
            for start in [head, head + std::f32::consts::PI] {
                for i in 0..whole(r) {
                    let i = f32::from(u16::try_from(i).unwrap_or(0));
                    let angle = (start + share(dice) * i / 30.0).rem_euclid(std::f32::consts::TAU);
                    sparks.push(on_circle(angle, PENTAGRAM_RADIUS));
                }
            }
        }
        let points = STAR.map(|angle| on_circle(self.turn + angle, PENTAGRAM_RADIUS));
        for side in points.windows(2) {
            let [from, to] = [side[0], side[1]];
            for _ in 0..whole(2.0 * f * r) {
                let along = share(dice);
                sparks.push([0, 1, 2].map(|k| from[k] + along * (to[k] - from[k])));
            }
        }
        sparks
    }
}

/// The confusion's textures, its circles' half sides and its helix's colour (871, `0x778E80`,
/// `0x779020`).
pub const CONFUSION_CIRCLE: &str = "FX_MAGICCIRCLE01";
pub const CONFUSION_SPINNER: &str = "FX_MAGICCIRCLE02";
pub const CONFUSION_HELIX: &str = "FX_HELIX01";
pub const CONFUSION_CIRCLE_HALF: f32 = 70.0;
pub const CONFUSION_SPINNER_HALF: f32 = 17.0;
pub const CONFUSION_HELIX_COLOUR: [f32; 3] = [13.0 / 255.0, 253.0 / 255.0, 228.0 / 255.0];
/// The confusion's helix's segments, its radius and half width, and its rise and turn a segment
/// (`+0x20AC`).
const CONFUSION_SEGMENTS: u8 = 50;
const CONFUSION_RADIUS: f32 = 30.0;
const CONFUSION_HALF: f32 = 2.0;
const CONFUSION_RISE: f32 = 0.5;
const CONFUSION_TURN: f32 = -0.3;

/// The confusion of the trap 1051 (871, `FX_CONFUSION`, `cParticleSystem_confusion`, its
/// constructor `0x778E80`, start `0x7792C0`, run `0x778F40` and draw `0x779020`; #1092): a rune
/// circle of [`CONFUSION_CIRCLE`] flat on the ground, 140 across, turning at a radian a second; a
/// small one of [`CONFUSION_SPINNER`] spinning at 5 against it, flying up from it ever faster to 100
/// high and starting again; and a screw-thread ribbon of [`CONFUSION_HELIX`] of 50 segments at
/// the radius 30, sliding up a screw line from a turn at random and fading in and out over each
/// 6.67 s; all added. It ends at once with its seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfusionCircle {
    clock: f32,
    phase: f32,
    start: f32,
    rise: f32,
    alpha: f32,
}

impl ConfusionCircle {
    /// A confusion as it starts (`0x7792C0`), its helix's first turn rolled by `dice`.
    pub fn new(dice: &mut impl Dice) -> Self {
        Self {
            clock: 0.0,
            phase: 0.0,
            start: share(dice) * std::f32::consts::TAU,
            rise: 1.0,
            alpha: 0.0,
        }
    }

    /// A run of `dt` seconds (`0x778F40`): its helix's phase grows by 0.15 a second, at 1 back to
    /// 0 on a turn rolled anew, its alpha `⌊255·sin(π·phase)⌋`; its small circle's height grows
    /// by itself times the frame's time, back to 1 past 100.
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) {
        self.clock += dt;
        self.phase += 0.15 * dt;
        if self.phase >= 1.0 {
            self.phase = 0.0;
            self.start = share(dice) * std::f32::consts::TAU;
        } else {
            self.alpha = (255.0 * (std::f32::consts::PI * self.phase).sin()).floor() / 255.0;
        }
        self.rise += self.rise * dt;
        if self.rise > 100.0 {
            self.rise = 1.0;
        }
    }

    /// Its circles' turns now, the big one's and the small one's, and the small one's height.
    pub fn circles(&self) -> (f32, f32, f32) {
        (self.clock, -5.0 * self.clock, self.rise)
    }

    /// Its helix's alpha now.
    pub fn alpha(&self) -> f32 {
        self.alpha
    }

    /// Its helix's two ribbons from its place in the models' frame (`0x762C30`): the point `j`
    /// at `θ₀ − 0.3·(k + j)` and `0.5·(k + j)` high at the radius 30, `k = 100·phase`, a band 4
    /// across flat and one 4 high upright.
    pub fn helix(&self) -> [Ribbon; 2] {
        let k = 100.0 * self.phase;
        helix_ribbons(
            (CONFUSION_RISE * k, self.start + CONFUSION_TURN * k),
            (CONFUSION_RISE, CONFUSION_TURN),
            (CONFUSION_RADIUS, CONFUSION_HALF),
            CONFUSION_SEGMENTS,
        )
    }
}

/// The ground's poison's textures, its circle's half side, colour and turn, its stain's half side
/// (792, `0x778760`, `0x778980`).
pub const POISON_STAIN: &str = "FX_GROUNDPOISON";
pub const POISON_SMOKE: &str = "PARTICLE_SMOKE03";
pub const POISON_CIRCLE: &str = "FX_MAGICCIRCLE03";
pub const POISON_CIRCLE_HALF: f32 = 65.0;
pub const POISON_CIRCLE_COLOUR: [f32; 4] = [160.0 / 255.0, 160.0 / 255.0, 160.0 / 255.0, 1.0];
pub const POISON_STAIN_HALF: f32 = 70.0;
const POISON_COUNT: usize = 200;

/// The ground's poison of the trap 1052 (792, `FX_GROUND_POISON`, `cParticleSystem_groundPoison`,
/// its constructor `0x778760`, start `0x778D10`, run `0x7788F0` and draw `0x778980`; #1092): a
/// green stain of [`POISON_STAIN`] laid flat on the ground by its alpha, 140 across; under a
/// player's trap a circle of [`POISON_CIRCLE`] turning at 0.2 radians a second, added in
/// [`POISON_CIRCLE_COLOUR`]; and puffs of green smoke of [`POISON_SMOKE`] rising from a square of
/// 30 about it, one every 0.04 s at the detail 2, of the size 18 falling by 2 a second, the
/// buoyancy −100 falling by 5 a second, a frame of its grid of 2 at random, their life falling by
/// 200 a second, of the ramp [`GroundPoison::ramp`], added. It ends at once with its seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct GroundPoison {
    pub system: System,
}

impl Default for GroundPoison {
    fn default() -> Self {
        Self::new()
    }
}

impl GroundPoison {
    /// A poison as it starts (`0x778D10`).
    pub fn new() -> Self {
        Self {
            system: System::new(POISON_COUNT),
        }
    }

    fn layer() -> Layer {
        Layer {
            buoyancy: -5.0,
            life: -200.0,
            size: -2.0,
            ..Layer::default()
        }
    }

    fn block() -> Block {
        Block {
            buoyancy: spread(-100.0, 0.0),
            size: spread(18.0, 0.0),
            offset: spread([0.0; 3], [15.0, 15.0, 0.0]),
            interval: 0.04,
            frames: 4,
            ..Block::default()
        }
    }

    /// Its smoke's ramp (`+0x20A8`): (0.5, 1, 0.5), to (0.2, 1, 0.2) over 128 entries, to black
    /// over 128, opaque.
    pub fn ramp() -> Ramp {
        Ramp::new(
            [0.5, 1.0, 0.5, 1.0],
            &[([0.2, 1.0, 0.2, 1.0], 128), ([0.0, 0.0, 0.0, 1.0], 128)],
        )
    }

    /// A run of `dt` seconds (`0x7788F0`): its puffs rise and age, and it makes new ones.
    pub fn update(&mut self, dt: f32, dice: &mut impl Dice) {
        let layers = [Self::layer()];
        self.system.update(dt, &layers);
        self.system
            .emit(&[Self::block()], &layers, POISON_COUNT, dice);
    }

    /// Its circle's turn now.
    pub fn turn(&self) -> f32 {
        0.2 * self.system.clock
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spawning::Rng;

    #[test]
    fn a_strike_spills_its_classs_sparks_and_they_fall_and_go() {
        let mut dice = Rng::of(3, 4, 5);
        let mut burst = hit_sparks(Sparks::Sera, &mut dice);
        assert_eq!(burst.particles.len(), 151);
        assert_eq!(burst.texture, "PARTICLE_SPARK06");
        assert_eq!(
            hit_sparks(Sparks::Vamp, &mut dice).texture,
            "PARTICLE_GLOW07"
        );
        // Born blue (127, 127, 255), dying black.
        let born = burst.ramp.at(255.0);
        assert!((born[0] - 127.0 / 255.0).abs() < 1e-6 && (born[2] - 1.0).abs() < 1e-6);
        assert_eq!(burst.ramp.at(0.0), [0.0, 0.0, 0.0, 1.0]);
        let mut system = System::new(burst.particles.len());
        system.particles = std::mem::take(&mut burst.particles);
        let layers = [burst.layer];
        for _ in 0..20 {
            system.update(0.025, &layers);
        }
        // Half a second on, they have risen at most some 60 and come back down.
        assert!(system.particles.iter().all(|spark| spark.place[2] < 60.0));
        for _ in 0..40 {
            system.update(0.025, &layers);
        }
        assert!(system.particles.is_empty());
    }

    #[test]
    fn the_arts_spark_by_their_classes() {
        assert_eq!(sparks_of(1059), Some((Sparks::Glad, SparksAt::Neck)));
        assert_eq!(sparks_of(1072), Some((Sparks::Sera, SparksAt::LeftFoot)));
        assert_eq!(sparks_of(1060), Some((Sparks::Glad, SparksAt::RightFoot)));
        assert_eq!(sparks_of(1041), Some((Sparks::Glad, SparksAt::RightFoot)));
        assert_eq!(sparks_of(1002), Some((Sparks::Elve, SparksAt::Neck)));
        assert_eq!(sparks_of(1001), Some((Sparks::Sera, SparksAt::Neck)));
        assert_eq!(sparks_of(1017), Some((Sparks::Sera, SparksAt::Root)));
        assert_eq!(sparks_of(1074), Some((Sparks::Vamp, SparksAt::Neck)));
        assert_eq!(sparks_of(1068), Some((Sparks::Dark, SparksAt::Neck)));
        // The jumps of the Gladiator and the Vampiress, the throw and the spells, none.
        assert_eq!(sparks_of(1003), None);
        assert_eq!(sparks_of(1032), None);
        assert_eq!(sparks_of(1013), None);
        assert_eq!(sparks_of(5), None);
    }

    #[test]
    fn a_torch_burns_a_tongue_of_flames_and_goes_out_with_its_last() {
        let mut dice = Rng::of(7, 8, 7);
        let mut torch = Torchsmoke::new();
        for _ in 0..40 {
            torch.update(0.025, ([0.0; 3], [0.0; 3]), &mut dice);
        }
        // One flame every 0.03 s, each living 0.32 s: some ten alive, risen at most about 19.
        let alive = torch.system.particles.len();
        assert!((9..=12).contains(&alive), "{alive}");
        assert!(
            torch
                .system
                .particles
                .iter()
                .all(|flame| flame.place[2] <= 20.0)
        );
        torch.put_out();
        for _ in 0..20 {
            torch.update(0.025, ([0.0; 3], [0.0; 3]), &mut dice);
        }
        assert!(torch.done());
    }

    #[test]
    fn the_breath_leaves_flames_and_smoke_along_its_way_and_its_wall_burns_its_seconds() {
        let mut dice = Rng::of(10, 0x6, 2);
        let breath = walloffire();
        let mut system = System::new(breath.count);
        // A second's flight at 150 a second along x: a particle every 0.01 s, one in five smoke,
        // laid along the way it flew.
        for step in 0..40 {
            let from = [
                150.0 * 0.025 * f32::from(u8::try_from(step).unwrap_or(0)),
                0.0,
                80.0,
            ];
            let to = [from[0] + 150.0 * 0.025, 0.0, 80.0];
            system.update(0.025, &breath.layers);
            system.emit_along(
                &breath.blocks,
                &breath.layers,
                breath.count,
                (from, to),
                0.025,
                &mut dice,
            );
        }
        let alive = system.particles.len();
        assert!((95..=101).contains(&alive), "{alive}");
        let smoke = system.particles.iter().filter(|p| p.layer == 1).count();
        assert!((10..=30).contains(&smoke), "{smoke}");
        let reach = system
            .particles
            .iter()
            .map(|p| p.place[0])
            .fold(f32::MIN, f32::max);
        assert!((140.0..=160.0).contains(&reach), "{reach}");
        // Born yellow-white at an alpha of 90, the smoke a transparent grey.
        assert_eq!(breath.ramps[0].at(255.0)[3], 90.0 / 255.0);
        assert_eq!(breath.ramps[1].at(255.0)[3], 0.0);
        // A wall's fire: a flame every 0.08 s for its 10 s, each living 2.55 s, then none.
        let wall = walloffire_line();
        let mut fire = System::new(wall.count);
        for _ in 0..400 {
            fire.update(0.025, &wall.layers);
            if fire.clock < crate::arts::WALL_BURNS {
                fire.emit(&wall.blocks, &wall.layers, wall.count, &mut dice);
            }
        }
        let burning = fire.particles.len();
        assert!((24..=33).contains(&burning), "{burning}");
        assert!(fire.particles.iter().all(|p| p.place[2] == 0.0));
        for _ in 0..110 {
            fire.update(0.025, &wall.layers);
        }
        assert!(fire.particles.is_empty());
        // Its grey up over its first entries, held, and gone at its death.
        assert!((wall.ramps[0].at(200.0)[0] - 199.0 / 255.0).abs() < 1e-6);
        assert_eq!(wall.ramps[0].at(0.0)[0], 0.0);
    }

    #[test]
    fn the_ring_of_poison_spreads_fades_and_its_twirls_shrink_over_3_s() {
        // The ring: 40 wide from 150 t, green, held to 0.45 s, gone at 0.75 s.
        let (inner, outer, colour) = poison_ring(0.2).unwrap_or_default();
        assert!((inner - 30.0).abs() < 1e-4 && (outer - 70.0).abs() < 1e-4);
        assert_eq!(colour, [34.0 / 255.0, 128.0 / 255.0, 34.0 / 255.0, 1.0]);
        let (_, _, fading) = poison_ring(0.55).unwrap_or_default();
        assert_eq!(fading[1], 85.0 / 255.0);
        assert!(poison_ring(0.75).is_none());
        // The twirls: whole at the start, turning the other way at their two rates, gone at 3 s.
        assert_eq!(poison_twirls(0.0), Some([(100.0, 0.0), (90.0, 0.0)]));
        let [(half, angle), (_, other)] = poison_twirls(1.5).unwrap_or_default();
        assert!((half - 75.0).abs() < 1e-4 && (angle + 2.55).abs() < 1e-4);
        assert!((other + 1.95).abs() < 1e-4);
        assert!(poison_twirls(POISON_RING_LIVES).is_none());
        // The sparks: 150 at once, shot up and falling back, gone within 1.5 s.
        let mut dice = Rng::of(11, 0x8, 4);
        let mut burst = ring_sparks(&mut dice);
        assert_eq!(burst.particles.len(), 150);
        assert!(
            burst
                .particles
                .iter()
                .all(|p| p.place[2] == 0.0 && (110.0..=190.0).contains(&p.velocity[2]))
        );
        let mut system = System::new(150);
        system.particles = std::mem::take(&mut burst.particles);
        for _ in 0..61 {
            system.update(0.025, &[burst.layer]);
        }
        assert!(system.particles.is_empty());
    }

    #[test]
    fn the_blast_spreads_its_ring_at_its_reach_in_its_blue() {
        let (inner, outer, colour) = blast_ring(0.5, 160.0).unwrap_or_default();
        assert!((inner - 80.0).abs() < 1e-4 && (outer - 120.0).abs() < 1e-4);
        // Fading from 0.45 s: a share of 5/6.
        assert_eq!(colour, [83.0 / 255.0, 83.0 / 255.0, 106.0 / 255.0, 1.0]);
        assert!(blast_ring(0.75, 160.0).is_none());
        let mut dice = Rng::of(12, 0x9, 5);
        let burst = blast_sparks(&mut dice);
        assert_eq!(burst.particles.len(), 150);
        assert_eq!(burst.ramp.at(156.0), [0.820, 0.820, 0.996, 1.0]);
        assert_eq!(ring_sparks(&mut dice).ramp.at(156.0)[0], 254.0 / 255.0);
    }

    #[test]
    fn the_holy_light_rises_in_sparks_pulses_and_fades_over_its_seconds() {
        assert!((holy_pulse(0.0) - 0.8).abs() < 1e-6);
        assert!((holy_pulse(std::f32::consts::FRAC_PI_8) - 0.9).abs() < 1e-6);
        let mut dice = Rng::of(12, 0x3, 24);
        let mut light = HolyLight::new(4.0);
        for _ in 0..80 {
            light.update(0.025, &mut dice);
        }
        // A spark every 0.03 s living 0.85 s: some 28 alive, risen fast, at half its fade.
        let alive = light.system.particles.len();
        assert!((26..=30).contains(&alive), "{alive}");
        assert!(light.system.particles.iter().any(|p| p.place[2] > 300.0));
        assert!((light.fade - 0.5).abs() < 1e-3, "{}", light.fade);
        assert!(!light.done());
        for _ in 0..80 {
            light.update(0.025, &mut dice);
        }
        assert!(light.done());
    }

    #[test]
    fn the_blades_of_light_climb_in_two_turning_spirals_over_a_ring() {
        let mut blades = Blades::default();
        // Three seconds: 99 high and turned by 15 radians less two whole turns.
        for _ in 0..120 {
            blades.update(0.025);
        }
        assert!((blades.height - 99.0).abs() < 0.1, "{}", blades.height);
        let turned = 15.0 - 2.0 * std::f32::consts::TAU;
        assert!((blades.angle - turned).abs() < 1e-3, "{}", blades.angle);
        // It rises no higher than 100 and a step more.
        for _ in 0..40 {
            blades.update(0.025);
        }
        assert!(blades.height < 101.0);
        let dots = blades.dots(50.0);
        assert_eq!(dots.len(), 500);
        // The ring on the ground at the radius, the spirals rising to under the height.
        assert!(dots[..100].iter().all(|d| d[2] == 0.0));
        assert!(dots.iter().all(|d| (d[0].hypot(d[1]) - 50.0).abs() < 1e-3));
        let top = dots[100..].iter().map(|d| d[2]).fold(0.0, f32::max);
        assert!(top < blades.height && top > 0.99 * blades.height);
        // The sparks at the spirals' tops, on either side.
        let [a, b] = blades.sparks(50.0);
        assert!((a[0] + b[0]).abs() < 1e-3 && (a[1] + b[1]).abs() < 1e-3);
        assert_eq!(a[2], blades.height);
    }

    #[test]
    fn a_chain_lightnings_bolt_bulges_between_its_ends_and_its_impact_draws_its_sparks_back() {
        let mut dice = Rng::of(20, 0x31F, 8);
        let mut bolt = ChainBolt::new(100.0, &mut dice);
        let lengths: Vec<f32> = bolt.segments.iter().map(|s| s.length).collect();
        assert_eq!(lengths, [30.0, 30.0, 30.0, 10.0]);
        // Its start lies on its line; each segment's far end is the next's near end.
        assert!(bolt.segments[0].near.iter().all(|o| o.abs() < 1e-5));
        for pair in bolt.segments.windows(2) {
            let s =
                (std::f32::consts::PI * (100.0 - 30.0 * f32::from(pair[1].index)) / 100.0).sin();
            let t = (std::f32::consts::PI * (100.0 - 30.0 * f32::from(pair[0].index) - 30.0)
                / 100.0)
                .sin();
            for k in 0..2 {
                assert!((pair[1].near[k] * t - pair[0].far[k] * s).abs() < 1e-4);
            }
        }
        assert!(
            bolt.segments
                .iter()
                .all(|s| s.far.iter().all(|o| o.abs() <= 8.0))
        );
        assert_eq!(ChainBolt::new(1_000.0, &mut dice).segments.len(), 19);
        // Whole for 0.4 s, gone 0.05 s after.
        for _ in 0..15 {
            bolt.update(0.025);
        }
        assert_eq!((bolt.fade, bolt.grey()), (1.0, 127.5 / 255.0));
        for _ in 0..4 {
            bolt.update(0.025);
        }
        assert!(bolt.done());
        let mut impact = ChainImpact::new(&mut dice);
        assert_eq!(impact.sparks.particles.len(), 15);
        // Its sparks fly off and the pull draws them back: nearer than without it.
        let mut free = impact.sparks.clone();
        for _ in 0..12 {
            impact.update(0.025);
            free.update(0.025, &[ChainImpact::layer()]);
        }
        let reach = |sparks: &System| {
            sparks
                .particles
                .iter()
                .map(|p| p.place.iter().map(|a| a * a).sum::<f32>().sqrt())
                .sum::<f32>()
        };
        assert!(reach(&impact.sparks) < reach(&free));
        for _ in 0..16 {
            impact.update(0.025);
        }
        assert!(impact.done());
        assert_eq!(
            ChainImpact::ramp().at(255.0),
            [127.0 / 255.0, 127.0 / 255.0, 1.0, 0.0]
        );
    }

    #[test]
    fn the_lightnings_bolts_climb_slanted_and_its_sparks_fall() {
        let mut dice = Rng::of(13, 0x31E, 1);
        let bolts = LightningBolt::three(&mut dice);
        assert!(bolts.iter().all(|b| (-1.0..=1.0).contains(&b.slant)));
        let bolt = LightningBolt { slant: 0.5 };
        // The foot of the first segment 20 wide about its place, its top 20 to the side, 250 up.
        let [bl, br, tr, tl] = bolt.segment(0, false);
        assert_eq!((bl.0, br.0), ([-10.0, 0.0, 0.0], [10.0, 0.0, 0.0]));
        assert_eq!((tl.0, tr.0), ([10.0, 0.0, 250.0], [30.0, 0.0, 250.0]));
        // The strip along `u`, from the slant at the top to one more at the foot.
        assert_eq!(
            (tl.1, tr.1, br.1, bl.1),
            ([0.5, 0.125], [0.5, 0.0], [1.5, 0.0], [1.5, 0.125])
        );
        // Turned over and shifted by half.
        let [fbl, _, ftr, _] = bolt.segment(0, true);
        assert_eq!((fbl.1, ftr.1), ([2.0, 0.0], [1.0, 0.125]));
        // The third segment follows the second.
        let [second_bl, ..] = bolt.segment(2, false);
        assert_eq!(second_bl.0, [30.0, 0.0, 500.0]);
        let burst = lightning_sparks(LIGHTNING_COLOURS[0], &mut dice);
        assert_eq!(burst.particles.len(), 150);
        assert_eq!(burst.ramp.at(128.0), LIGHTNING_COLOURS[0]);
    }

    #[test]
    fn the_light_is_a_ball_of_slices() {
        let slices: Vec<(f32, f32)> = maglite_slices().collect();
        assert_eq!(slices.len(), 21);
        assert_eq!(slices[0], (-25.0, 2_400.0_f32.sqrt()));
        assert_eq!(slices[10], (0.0, 55.0));
        assert_eq!(slices[20].0, 25.0);
    }

    #[test]
    fn a_lost_soul_trails_green_dots_from_its_wobble_and_beats_its_glow() {
        assert_eq!((beat(0.0), beat(0.15), beat(0.3)), (0.0, 0.5, 1.0));
        assert!((beat(0.65) - 0.5).abs() < 1e-5 && (beat(1.0)).abs() < 1e-5);
        assert_eq!(
            soul_glow(0.3),
            (28.0, [192.0 / 255.0, 1.0, 192.0 / 255.0, 1.0])
        );
        assert!(soul_wobble(0.4).iter().all(|o| o.abs() <= 5.0));
        let mut dice = Rng::of(21, 0x352, 9);
        let mut soul = LostSoul::new(1.0);
        let mut at = [0.0f32; 3];
        for _ in 0..30 {
            let from = at;
            at[0] += 150.0 * 0.025;
            soul.update(0.025, (from, at), &mut dice);
        }
        // A dot every 0.01 s, living 0.64 s: its trail lies behind it along its way.
        let dots = soul.system.particles.len();
        assert!((60..=76).contains(&dots), "{dots}");
        assert!(
            soul.system
                .particles
                .iter()
                .all(|p| p.place[0] <= at[0] + 5.0)
        );
        assert!(
            soul.system
                .particles
                .iter()
                .any(|p| p.place[0] < at[0] - 50.0)
        );
        assert_eq!(LostSoul::new(1.0).ramp().at(255.0)[1], 255.0 / 256.0);
        // A dryad ghost's dots are blue, its seconds endless.
        let ghost = LostSoul::dryad_ghost();
        assert_eq!(ghost.ramp().at(255.0)[2], 255.0 / 256.0);
        assert_eq!(ghost.ramp().at(255.0)[1], 60.0 / 256.0);
        for _ in 0..70 {
            soul.update(0.025, (at, at), &mut dice);
        }
        assert!(soul.done());
    }

    #[test]
    fn a_magic_prison_stands_a_column_of_lilac_glows() {
        let mut dice = Rng::of(23, 0x356, 11);
        let prison = magic_prison();
        let mut glows = System::new(prison.count);
        for _ in 0..120 {
            glows.update(0.025, &prison.layers);
            glows.emit(&prison.blocks, &prison.layers, prison.count, &mut dice);
        }
        // One every 0.05 s living 1.275 s: about 25 at once, in a column up to 100 high.
        let alive = glows.particles.len();
        assert!((20..=27).contains(&alive), "{alive}");
        assert!(glows.particles.iter().all(|p| p.place[0].abs() <= 2.0
            && p.place[1].abs() <= 2.0
            && p.place[2] >= -30.0
            && p.place[2] <= 130.0));
        assert!(glows.particles.iter().any(|p| p.place[2] > 50.0));
        assert_eq!(
            prison.ramps[0].at(255.0)[..3],
            [63.0 / 255.0, 63.0 / 255.0, 127.0 / 255.0]
        );
        assert_eq!(prison.ramps[0].at(0.0)[..3], [0.0; 3]);
    }

    #[test]
    fn a_fears_column_rises_as_it_fades_in_and_its_ring_spreads_over_and_over() {
        let mut fear = HellScream::new(25.0);
        assert_eq!(fear.ring().0, 10.0);
        for _ in 0..40 {
            fear.update(0.025);
        }
        // Whole after a second, its column 124 high, its ring at 80 and dimming.
        assert!((fear.fade - 1.0).abs() < 1e-3);
        let top = fear.column().last().map_or(0.0, |(height, _)| height);
        assert!((top - 124.0).abs() < 0.2, "{top}");
        assert!(fear.column().all(|(_, half)| (2.6..=18.0).contains(&half)));
        let (inner, outer, colour) = fear.ring();
        assert!((inner - 80.0).abs() < 0.1 && (outer - inner - 20.0).abs() < 1e-4);
        assert!(colour[0] < 1.0 && colour[0] > 0.7);
        for _ in 0..12 {
            fear.update(0.025);
        }
        // Past 100 it starts again at 10.
        assert!(fear.ring().0 < 20.0);
        for _ in 0..958 {
            fear.update(0.025);
        }
        assert!(!fear.done() && fear.fade < 1.0);
        for _ in 0..200 {
            fear.update(0.025);
        }
        assert!(fear.done());
    }

    #[test]
    fn a_summonings_six_objects_draw_a_burning_pentagram_in_its_circle() {
        let mut dice = Rng::of(24, 0x396, 12);
        let mut circle = DemonSummon::new(0);
        let mut side = DemonSummon::new(1);
        assert_eq!((circle.light(), side.light()), (SUMMON_LIGHT, 0.0));
        for _ in 0..80 {
            circle.update(0.025, &mut dice);
            side.update(0.025, &mut dice);
        }
        // The circle's glows lie on its radius; the side's on the line from its first point,
        // half way along it after 2 s of the 4 s of its turn.
        assert!(circle.system.particles.iter().all(|p| {
            let r = (p.place[0] * p.place[0] + p.place[1] * p.place[1]).sqrt();
            (r - SUMMON_RADIUS).abs() < 1.0
        }));
        let to = summon_point(1);
        assert!((side.emitter[0] - to[0]).abs() < 10.0 && (side.emitter[1] - to[1]).abs() < 10.0);
        assert!(side.system.particles.iter().all(|p| {
            // On the segment from (0, 285) to its second point: across it, under 1.
            let (a, b) = (summon_point(0), to);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let cross = ((p.place[0] - a[0]) * dy - (p.place[1] - a[1]) * dx).abs();
            cross / (dx * dx + dy * dy).sqrt() < 1.0
        }));
        // It emits for 8 s, its last glows dying 1.7 s after.
        for _ in 0..400 {
            circle.update(0.025, &mut dice);
        }
        assert!(circle.done());
        assert_eq!(circle.ramp().at(0.0)[..3], [0.0; 3]);
        assert_eq!(
            DemonSummon::new(10).ramp().at(251.0)[..3],
            [150.0 / 256.0, 100.0 / 256.0, 100.0 / 256.0]
        );
    }

    #[test]
    fn a_choir_fades_in_then_rises_black_wisps_over_its_beating_discs() {
        let mut dice = Rng::of(22, 0x3F3, 10);
        let mut choir = Choir::new(60.0, 3.0);
        assert_eq!(choir.base, 10.0);
        for _ in 0..40 {
            choir.update(0.025, &mut dice);
        }
        // A second of fading in, with no wisp yet.
        assert!(choir.fade < 1.0 + 1e-3 && choir.system.particles.is_empty());
        for _ in 0..41 {
            choir.update(0.025, &mut dice);
        }
        assert!(!choir.system.particles.is_empty());
        assert!(
            choir
                .system
                .particles
                .iter()
                .all(|p| p.place[0].abs() <= 10.0 && p.place[1].abs() <= 10.0 && p.place[2] >= 0.0)
        );
        let discs: Vec<(f32, f32)> = choir.discs().collect();
        assert_eq!(discs.len(), 5);
        assert_eq!(discs[4].0, 12.0);
        let t = choir.clock;
        assert!((discs[0].1 - 10.0 * (t.sin() + 10.0)).abs() < 1e-3);
        assert_eq!(Choir::wisp(255.0), [0.0, 0.0, 0.0, 1.0]);
        for _ in 0..200 {
            choir.update(0.025, &mut dice);
        }
        assert!(choir.done());
    }

    #[test]
    fn the_fire_skin_burns_flames_about_its_bearer() {
        let mut dice = Rng::of(14, 0x335, 2);
        let skin = fire_skin(true);
        assert_eq!(skin.texture, "PARTICLE_FIRE02");
        assert_eq!(fire_skin(false).texture, "PARTICLE_FIRE09");
        let mut flames = System::new(skin.count);
        for _ in 0..40 {
            flames.update(0.025, &skin.layers);
            flames.emit(&skin.blocks, &skin.layers, skin.count, &mut dice);
        }
        // A flame every 0.01 s, living up to 0.73 s but shrinking out at 50 a second: under 73.
        let alive = flames.particles.len();
        assert!((25..=73).contains(&alive), "{alive}");
        assert!(
            flames
                .particles
                .iter()
                .all(|p| p.place[0].abs() <= 15.0 && p.place[2] == 0.0)
        );
        // Born transparent, at its whitest 32 entries on, black at its death.
        assert_eq!(skin.ramps[0].at(255.0)[3], 0.0);
        assert_eq!(skin.ramps[0].at(224.0), [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(skin.ramps[0].at(0.0), [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn a_fire_spirals_flame_burns_wide_and_orange_to_its_end() {
        let mut dice = Rng::of(16, 0x316, 4);
        let circle = fire_circle();
        assert_eq!(circle.texture, "PARTICLE_FIRE02");
        assert_eq!(circle.light, 300.0);
        let mut flames = System::new(circle.count);
        for _ in 0..40 {
            flames.update(0.025, &circle.layers);
            flames.emit(&circle.blocks, &circle.layers, circle.count, &mut dice);
        }
        // A flame every 0.01 s, of 20 to 60 shrinking at 70 a second: the smallest out by 0.29 s.
        let alive = flames.particles.len();
        assert!((25..=73).contains(&alive), "{alive}");
        assert!(flames.particles.iter().all(|p| p.place[0].abs() <= 7.0
            && p.place[1].abs() <= 7.0
            && p.place[2] == 0.0
            && p.size <= 60.0));
        // Born transparent, its orange whole 32 entries on and to its death.
        let orange = [1.0, 0.7, 0.5, 1.0];
        assert_eq!(circle.ramps[0].at(255.0)[3], 0.0);
        assert_eq!(circle.ramps[0].at(224.0), orange);
        assert_eq!(circle.ramps[0].at(0.0), orange);
    }

    #[test]
    fn a_meteor_falls_from_aside_bursts_and_leaves_its_crater_to_fade() {
        let mut dice = Rng::of(18, 0x337, 6);
        let mut meteor = Meteor::new(0.2, &mut dice);
        assert!(
            meteor
                .turns
                .iter()
                .all(|turn| (0.0..=std::f32::consts::TAU).contains(turn))
        );
        assert!(!meteor.update(0.1, &mut dice));
        assert_eq!(meteor.crater_alpha(), None);
        let mut steps = 0;
        while !meteor.update(0.025, &mut dice) {
            steps += 1;
            assert!(steps < 40, "it lands 0.625 s after its wait");
        }
        // It comes down from the side onto its place, its trail behind it.
        assert!((-1.0..=5.0).contains(&meteor.head[0]), "{:?}", meteor.head);
        assert!(meteor.system.particles.len() > 300);
        assert!(meteor.system.particles.iter().any(|p| p.place[2] > 250.0));
        assert_eq!(meteor.phase, MeteorPhase::Burning);
        assert_eq!(meteor.crater_alpha(), Some(1.0));
        // Its fire and smoke burn out in under 2 s; its crater lies 15 s and fades over 3.
        for _ in 0..80 {
            meteor.update(0.025, &mut dice);
        }
        assert!(matches!(meteor.phase, MeteorPhase::Lying(_)));
        for _ in 0..640 {
            meteor.update(0.025, &mut dice);
        }
        let alpha = meteor.crater_alpha().unwrap_or(0.0);
        assert!(
            matches!(meteor.phase, MeteorPhase::Fading(_)) && alpha < 1.0,
            "{alpha}"
        );
        for _ in 0..120 {
            meteor.update(0.025, &mut dice);
        }
        assert!(meteor.done());
        // The fire's ramp warms from grey; the smoke's greys.
        let [fire, smoke] = Meteor::ramps();
        assert_eq!(
            fire.at(255.0),
            [86.0 / 256.0, 86.0 / 256.0, 86.0 / 256.0, 255.0 / 256.0]
        );
        assert_eq!(smoke.at(0.0), [0.0; 4]);
    }

    #[test]
    fn the_purgatory_burns_wider_under_its_column_a_creatures_white() {
        let mut dice = Rng::of(17, 0x334, 5);
        let fire = hellfire(true);
        assert_eq!(fire.light, 200.0);
        assert_eq!(fire.layers, fire_circle().layers);
        let mut flames = System::new(fire.count);
        for _ in 0..40 {
            flames.update(0.025, &fire.layers);
            flames.emit(&fire.blocks, &fire.layers, fire.count, &mut dice);
        }
        assert!(
            flames
                .particles
                .iter()
                .all(|p| p.place[0].abs() <= 25.0 && p.place[1].abs() <= 25.0 && p.place[2] == 0.0)
        );
        assert!(flames.particles.iter().any(|p| p.place[0].abs() > 7.0));
        let creatures = hellfire(false);
        assert_eq!(creatures.texture, "PARTICLE_FIRE09");
        assert_eq!(creatures.ramps[0].at(0.0), [1.0; 4]);
        // Its column fades from its foot to nothing at its top, in its colour.
        let column = hellfire_column(false);
        assert_eq!(column.foot, [64.0 / 255.0, 64.0 / 255.0, 1.0]);
        assert_eq!(
            (column.height, column.alphas, column.tops),
            (200.0, [160.0, 128.0], [0.0; 2])
        );
    }

    #[test]
    fn the_gust_circles_rises_and_goes_in_the_grounds_colour() {
        assert_eq!(
            argb(GROUND_COLOURS[3]),
            [48.0 / 255.0, 64.0 / 255.0, 16.0 / 255.0, 1.0]
        );
        let mut dice = Rng::of(15, 0x33A, 3);
        let mut gust = Twister::new(5.0, &mut dice);
        assert_eq!(gust.puffs.len(), 250);
        assert!(gust.puffs.iter().all(|p| (30.0..=35.0).contains(&p.radius)
            && (0.0..=1.0).contains(&p.share)
            && (0.3..=4.0).contains(&p.spin)));
        // Its column at 100 by 3.4 s, standing to its 5 s, then 2 s of climbing.
        for _ in 0..140 {
            gust.update(0.025);
        }
        assert_eq!(gust.height, 100.0);
        for _ in 0..70 {
            gust.update(0.025);
        }
        assert!(gust.height > 100.0 && !gust.done());
        for _ in 0..80 {
            gust.update(0.025);
        }
        assert!(gust.done());
        // A puff at the top widens by 0.6 and grows to a half side of 21.
        let puff = Puff {
            angle: 0.0,
            radius: 30.0,
            share: 1.0,
            spin: 1.0,
            turn: 0.0,
        };
        assert_eq!(puff.at(100.0), ([48.0, 0.0, 100.0], 21.0));
    }

    #[test]
    fn the_battle_fog_puffs_for_its_seconds_and_lives_out() {
        let fog = battle_fog(0);
        assert_eq!(fog.ramps[0].at(255.0)[3], 0.0);
        assert_eq!(battle_fog(1).ramps[0].at(255.0)[0], 193.0 / 256.0);
        let mut dice = Rng::of(16, 0x364, 4);
        let mut puffs = System::new(fog.count);
        for _ in 0..400 {
            puffs.update(0.025, &fog.layers);
            puffs.emit(&fog.blocks, &fog.layers, fog.count, &mut dice);
        }
        // A puff every 0.3 s living 5.1 s: 17 alive, each a frame of the four.
        let alive = puffs.particles.len();
        assert!((16..=18).contains(&alive), "{alive}");
        assert!(
            puffs
                .particles
                .iter()
                .all(|p| p.frame < 4 && p.place[1].abs() <= 80.0)
        );
        for _ in 0..210 {
            puffs.update(0.025, &fog.layers);
        }
        assert!(puffs.particles.is_empty());
    }

    #[test]
    fn a_ring_of_frosts_object_strikes_bursts_and_stands_its_iceberg() {
        let mut dice = Rng::of(19, 0x348, 7);
        let mut ice = Iceberg::new(25.0, &mut dice);
        assert!(ice.model < 4 && (0.0..=std::f32::consts::TAU).contains(&ice.turn));
        let mut frames = std::collections::BTreeSet::new();
        for _ in 0..59 {
            ice.update(0.025, &mut dice);
            frames.insert(ice.frame);
        }
        // It strikes for 1.5 s, its frame of eight flickering; no sparks yet.
        assert!(ice.striking() && ice.sparks.particles.is_empty());
        assert!(frames.len() > 4 && frames.iter().all(|&frame| frame < 8));
        ice.update(0.025, &mut dice);
        ice.update(0.025, &mut dice);
        assert!(!ice.striking());
        assert_eq!(ice.sparks.particles.len(), 50);
        // Its sparks fall onto the ground and die within 1.7 s; it stands to its 25 s.
        for _ in 0..80 {
            ice.update(0.025, &mut dice);
        }
        assert!(ice.sparks.particles.is_empty() && !ice.done());
        for _ in 0..900 {
            ice.update(0.025, &mut dice);
        }
        assert!(ice.done());
        assert_eq!(Iceberg::ramp().at(255.0), [1.0, 1.0, 1.0, 0.0]);
        let mut odd = Iceberg::new(1.0, &mut dice);
        odd.frame = 5;
        assert_eq!(odd.strike(), (2, [1.0, 0.5]));
    }

    #[test]
    fn a_banishing_rune_pulses_warm_and_ends_with_its_seconds() {
        let (grey, bar, rays) = rune_colours(0.0);
        assert_eq!(grey, [128.0 / 255.0, 128.0 / 255.0, 128.0 / 255.0, 1.0]);
        assert_eq!(bar, [96.0 / 255.0, 48.0 / 255.0, 0.0, 1.0]);
        assert_eq!(rays[0], [16.0 / 255.0, 8.0 / 255.0, 0.0, 1.0]);
        // At its brightest the grey holds at 255.
        let (bright, ..) = rune_colours(std::f32::consts::FRAC_PI_2);
        assert_eq!(bright[0], 1.0);
        let mut dice = Rng::of(17, 0x325, 5);
        let mut rune = Rune::new(2.0);
        for _ in 0..40 {
            rune.update(0.025, &mut dice);
        }
        // A spark every 0.05 s living 0.51 s: some ten, rising.
        let alive = rune.sparks.particles.len();
        assert!((9..=11).contains(&alive), "{alive}");
        assert!(rune.sparks.particles.iter().all(|p| p.place[2] >= 0.0));
        assert!(!rune.done());
        for _ in 0..41 {
            rune.update(0.025, &mut dice);
        }
        assert!(rune.done());
    }

    #[test]
    fn a_flip_book_plays_once_over_a_particles_life() {
        assert_eq!(age_frame(255.0, 4), 0);
        assert_eq!(age_frame(127.5, 4), 7);
        assert_eq!(age_frame(0.0, 4), 15);
        assert_eq!(age_frame(-5.0, 2), 3);
    }

    #[test]
    fn the_bfgs_glows_pulse_against_one_another() {
        assert_eq!(
            bfg_greys(0.0),
            [127.0 / 255.0, 128.0 / 255.0, 254.0 / 255.0]
        );
        // A quarter of its period on: the first at its brightest, the second at its darkest.
        let quarter = bfg_greys(std::f32::consts::FRAC_PI_2 / 4.21);
        assert!((quarter[0] - 254.0 / 255.0).abs() < 1e-6, "{quarter:?}");
        assert!((quarter[1] - 1.0 / 255.0).abs() < 1e-6, "{quarter:?}");
    }

    #[test]
    fn a_fireball_leaves_its_flames_along_its_way_and_burns_out_after_its_hit() {
        let mut dice = Rng::of(7, 7, 8);
        let mut fireball = Firestrike::new();
        // A second at 160 a second along x, in frames of 0.05 s.
        let mut at = [0.0; 3];
        for _ in 0..20 {
            let to = [at[0] + 8.0, 0.0, 0.0];
            fireball.update(0.05, (at, to), &mut dice);
            at = to;
        }
        // One flame every 0.07 s, each living 0.73 s: some ten alive, along the way behind it.
        let alive = fireball.system.particles.len();
        assert!((9..=11).contains(&alive), "{alive}");
        assert!(
            fireball
                .system
                .particles
                .iter()
                .all(|flame| (40.0..=164.0).contains(&flame.place[0])),
            "{:?}",
            fireball.system.particles
        );
        // A hit: one flame more about its place, then none, and it ends with its last.
        fireball.hit();
        fireball.update(0.05, (at, at), &mut dice);
        assert_eq!(fireball.state, Flames::Out);
        let burst = fireball.system.particles.last().expect("the burst's flame");
        assert!((burst.place[0] - at[0]).abs() <= 16.0, "{burst:?}");
        for _ in 0..20 {
            fireball.update(0.05, (at, at), &mut dice);
        }
        assert!(fireball.done());
    }

    #[test]
    fn the_emperors_rays_crackle_for_their_time_and_flare_at_a_hit() {
        let mut dice = Rng::of(11, 5, 9);
        let mut emperor = Emperor::new();
        for _ in 0..50 {
            emperor.update(0.01, &mut dice);
        }
        // One ray every 0.005 s, each living 0.21 s: some 42 alive.
        let alive = emperor.system.particles.len();
        assert!((38..=44).contains(&alive), "{alive}");
        assert!((emperor.glow_half(0.5) - 12.0).abs() < 1e-6);
        // A hit: 2 s more, 1.75 times as large.
        emperor.flare();
        for _ in 0..150 {
            emperor.update(0.01, &mut dice);
        }
        assert!(emperor.emits() && (emperor.glow_half(0.0) - 17.5).abs() < 1e-6);
        for _ in 0..80 {
            emperor.update(0.01, &mut dice);
        }
        assert!(emperor.done());
        // Its ramp: black at death, (220, 220, 255) at birth.
        let ramp = Emperor::ramp();
        assert_eq!(ramp.at(0.0), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(
            ramp.at(255.0),
            [219.0 / 255.0, 219.0 / 255.0, 254.0 / 255.0, 1.0]
        );
    }

    #[test]
    fn a_burst_clouds_in_its_sets_colour_for_a_second() {
        assert_eq!(
            [0, 1, 2, 3, 4, 9].map(burst_colour),
            [
                0xFFAA_0000,
                0xFF00_AA00,
                0xFF11_1111,
                0xFF73_5623,
                0x80AB_BECD,
                0xFF00_0000
            ]
        );
        let cloud = burst_cloud(0xFF00_AA00);
        // Green at its birth, gone at its death.
        assert_eq!(cloud.ramp.at(255.0), [0.0, 170.0 / 255.0, 0.0, 1.0]);
        assert!(cloud.ramp.at(0.0)[3] < 0.01);
        let stream = cloud.stream.expect("a stream");
        let mut system = System::new(stream.count);
        let mut dice = Rng::of(1, 2, 3);
        let layers = std::slice::from_ref(&cloud.layer);
        let mut most = 0;
        for _ in 0..60 {
            system.update(1.0 / 60.0, layers);
            if system.clock < stream.until {
                system.emit(
                    std::slice::from_ref(&stream.block),
                    layers,
                    stream.count,
                    &mut dice,
                );
            }
            most = most.max(system.particles.len());
        }
        // A particle each 0.02 s for a second, none dying before 1.275 s.
        assert!((48..=50).contains(&most), "{most}");
        for _ in 0..90 {
            system.update(1.0 / 60.0, layers);
        }
        assert!(system.particles.is_empty());
    }

    #[test]
    fn the_last_burnings_sparks_bounce_and_their_flames_and_scorch_fade() {
        // The goblins' red sparks, the ghoul's and the skeletons' flames, none for another.
        assert_eq!(
            [142, 73, 81, 76].map(burn_variant),
            [Some(0), Some(7), Some(8), None]
        );
        let mut dice = Rng::of(1, 2, 3);
        let sparks = generic_sparks(&[[0.0, 0.0, 10.0]], BURN_SPARKS, &mut dice);
        assert_eq!(sparks.particles.len(), GENERIC_PARTICLES);
        assert_eq!(sparks.ramp.at(255.0)[1], 64.0 / 255.0);
        // A spark that falls onto the ground bounces back up without loss.
        let mut system = System::new(1);
        system.particles.push(Particle {
            place: [0.0, 0.0, 0.5],
            velocity: [0.0, 0.0, -10.0],
            size: 5.0,
            life: 255.0,
            buoyancy: 0.0,
            angle: 0.0,
            spin: 0.0,
            layer: 0,
            frame: 0,
        });
        system.update_over(0.1, &[sparks.layer], Some(0.0));
        assert!(system.particles[0].velocity[2] > 0.0);
        assert!(
            death_flames(7).is_some() && death_flames(8).is_some() && death_flames(0).is_none()
        );
        // The scorch whole to 10 s, half at 11 s, gone at 12 s; its texture by the coin.
        assert_eq!(scorch_alpha(9.0), Some(1.0));
        assert_eq!(scorch_alpha(11.0), Some(0.5));
        assert_eq!(scorch_alpha(12.0), None);
        assert_eq!(scorch_texture(true), "fx_streaks02");
    }

    #[test]
    fn a_scripts_ring_of_sparks_rises_about_its_place_and_is_gone_in_two_and_a_half_seconds() {
        let mut dice = Rng::of(4, 5, 6);
        // The variant 1 in red: 150 on a ring of 25, rising ever faster, in the colour.
        let ring = generic_ring(RISING_RING, 0xFFFF_0000, &mut dice).unwrap();
        assert_eq!(ring.particles.len(), 150);
        assert_eq!(ring.ramp.at(128.0), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(ring.light, 150.0);
        for particle in &ring.particles {
            let [x, y, z] = particle.place;
            assert!(((x * x + y * y).sqrt() - 25.0).abs() < 1e-3 && z == 0.0);
            assert!((-90.0..=-10.0).contains(&particle.buoyancy));
        }
        // The variant 4: 500 tighter and smaller; another variant none.
        let spawn = generic_ring(SPAWN_RING, 0xFFFF_FFFF, &mut dice).unwrap();
        assert_eq!(spawn.particles.len(), GENERIC_PARTICLES);
        assert_eq!(spawn.particles[0].size, 7.0);
        assert!(generic_ring(10, 0xFFFF_FFFF, &mut dice).is_none());
        // They live 255 / 104 s.
        let mut system = System::new(150);
        system.particles = ring.particles;
        for _ in 0..24 {
            system.update_over(0.1, &[ring.layer], ring.ground);
        }
        assert!(!system.particles.is_empty());
        system.update_over(0.1, &[ring.layer], ring.ground);
        assert!(system.particles.is_empty());
    }

    #[test]
    fn a_scripts_smoke_puffs_rise_ever_faster_tinted_and_are_gone_in_1_275_seconds() {
        let mut dice = Rng::of(7, 8, 9);
        // The variant 10 in a grey: 25 about the place, of the colour's bytes over 256, tinted.
        let smoke = generic_smoke(0xFF80_8080, &mut dice);
        assert_eq!(smoke.particles.len(), 25);
        assert_eq!(smoke.ramp.at(255.0), [0.5, 0.5, 0.5, 0.996]);
        assert_eq!(smoke.ramp.at(0.0)[3], 0.0);
        assert!(smoke.tinted && smoke.light == 0.0 && smoke.blend == Blend::Over);
        for particle in &smoke.particles {
            let [x, y, z] = particle.place;
            assert!(x.abs() <= 15.0 && y.abs() <= 15.0 && z.abs() <= 4.0);
            assert!((-70.0..=-50.0).contains(&particle.buoyancy));
            assert!((7.0..=9.0).contains(&particle.size));
            assert_eq!(particle.velocity[2], 5.0);
        }
        // They rise, grow by 10 a second and live 255 / 200 s.
        let mut system = System::new(25);
        system.particles.clone_from(&smoke.particles);
        for _ in 0..12 {
            system.update(0.1, &[smoke.layer]);
        }
        let first = (smoke.particles[0], system.particles[0]);
        assert!(first.1.place[2] > first.0.place[2] + 30.0);
        assert!((first.1.size - first.0.size - 12.0).abs() < 1e-3);
        system.update(0.1, &[smoke.layer]);
        assert!(system.particles.is_empty());
    }

    #[test]
    fn a_gold_arrow_bobs_and_turns_under_a_helix_that_fades_in_and_out() {
        let mut dice = Rng::of(1, 2, 3);
        let mut beacon = Beacon::new(&mut dice);
        assert_eq!(beacon.arrow(), (0.0, 45.0));
        assert_eq!(beacon.light(), 100.0);
        // Half a cycle on (1.6 s): the helix whole, the arrow turned by 3.2 radians.
        for _ in 0..16 {
            beacon.update(0.1, &mut dice);
        }
        assert!(beacon.alpha() > 0.99);
        let (spin, height) = beacon.arrow();
        assert!((spin - 3.2).abs() < 1e-3 && (35.0..=55.0).contains(&height));
        // The ribbons: 50 segments each, a fifth of a radian and a quarter of a unit apart.
        let [flat, upright] = beacon.helix();
        assert_eq!((flat.len(), upright.len()), (50, 50));
        let [inner, outer] = flat[0];
        let radius = |[x, y, _]: [f32; 3]| x.hypot(y);
        assert!((radius(inner) - 15.0).abs() < 1e-3 && (radius(outer) - 19.0).abs() < 1e-3);
        assert!((upright[0][0][2] - upright[0][1][2] - 4.0).abs() < 1e-3);
        assert!((flat[1][0][2] - flat[0][0][2] - 0.25).abs() < 1e-4);
        // The cycle ends at 3.33 s, the helix starting again at its foot, 50 over the place.
        for _ in 0..18 {
            beacon.update(0.1, &mut dice);
        }
        assert!((beacon.helix()[0][0][0][2] - 50.0).abs() < 1.0);
        assert!((25.0..=175.0).contains(&beacon.light()));
    }

    #[test]
    fn the_glitter_wanders_within_ten_its_sparks_in_full_hues() {
        for step in 0..200 {
            let t = step as f32 * 0.137;
            let sparks = glitter(t, GLITTER_SPARKS[2]);
            assert_eq!(sparks.len(), 8);
            for spark in sparks {
                assert!(spark.offset.iter().all(|v| v.abs() <= 10.0));
                assert!((1.0..=3.0).contains(&spark.half));
                let peak = spark.colour.iter().copied().fold(0.0, f32::max);
                assert!((peak - 250.0 / 255.0).abs() < 1e-5);
                assert!(spark.colour.contains(&0.0));
            }
        }
        // The wheel runs on round its turn: blue at 0, yellow at a half, red at two thirds.
        assert_eq!(hue_colour(0.0, 1.0), [0.0, 0.0, 1.0]);
        assert_eq!(hue_colour(0.5, 1.0), [1.0, 1.0, 0.0]);
        assert_eq!(hue_colour(4.0 / 6.0, 1.0), [1.0, 0.0, 0.0]);
        let near_end = hue_colour(0.999, 1.0);
        assert!(near_end[2] == 1.0 && near_end[0] < 0.01);
        // A spark's hue turns once in two seconds of the game, its time running at half of them.
        let (now, later) = (glitter(3.0, 3), glitter(4.0, 3));
        assert!((now[0].colour[0] - later[0].colour[0]).abs() < 1e-4);
    }

    #[test]
    fn a_mouse_events_sparks_rise_as_high_as_25_at_half_the_value() {
        for step in 0..200 {
            let t = step as f32 * 0.137;
            let (event, glitter) = (event_sparks(t, 8), glitter(t, 8));
            for (spark, glint) in event.iter().zip(&glitter) {
                assert!((0.0..=25.0).contains(&spark.offset[2]));
                // The same wander across and along, and the same grey square.
                assert_eq!(spark.offset[..2], glint.offset[..2]);
                assert!((spark.half - glint.half).abs() < f32::EPSILON);
                let peak = spark.colour.iter().copied().fold(0.0, f32::max);
                assert!((peak - 125.0 / 255.0).abs() < 1e-5);
            }
        }
    }

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
    fn a_risen_undeads_ring_rises_and_its_even_half_falls_back() {
        let burst = rise_ring();
        assert_eq!(burst.particles.len(), 75);
        let radius = |p: &Particle| p.place[0].hypot(p.place[1]);
        assert!(
            burst
                .particles
                .iter()
                .all(|p| (radius(p) - 17.5).abs() < 1e-3)
        );
        let mut system = System::new(75);
        system.particles = burst.particles.clone();
        for _ in 0..42 {
            system.update(1.0 / 60.0, &[burst.layer]);
        }
        // At 0.7 s the even ones stand at their highest, about 24.5, the odd ones at 49.
        let even = system.particles[0].place[2];
        let odd = system.particles[1].place[2];
        assert!((even - 24.5).abs() < 1.0, "{even}");
        assert!((odd - 49.0).abs() < 1.0, "{odd}");
        // All gone at 1.275 s.
        for _ in 0..36 {
            system.update(1.0 / 60.0, &[burst.layer]);
        }
        assert!(system.particles.is_empty());
    }

    #[test]
    fn a_vanishing_corpses_dust_leaves_its_points_and_fades() {
        let mut dice = Rng::of(0, 0, 9);
        let points = [[0.0, 0.0, 10.0], [5.0, 0.0, 40.0]];
        let burst = dust(&points, &mut dice);
        assert_eq!(burst.particles.len(), 150);
        assert!(
            burst
                .particles
                .iter()
                .all(|p| points.contains(&p.place) && p.frame < 4)
        );
        assert!((burst.ramp.at(255.0)[3] - 0.5).abs() < 1e-3);
        assert!(dust(&[], &mut dice).particles.is_empty());
        let mut system = System::new(150);
        system.particles = burst.particles;
        for _ in 0..154 {
            system.update(1.0 / 60.0, &[burst.layer]);
        }
        assert!(system.particles.is_empty());
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

    #[test]
    fn a_magic_fire_burns_three_layers_each_of_its_frame_by_their_rates() {
        let fire = fire(fire_variant(904).unwrap()).unwrap();
        assert_eq!((fire.layers.len(), fire.grid, fire.light), (3, 2, 230.0));
        assert!(fire.blends.iter().all(|&blend| blend == Blend::Whole));
        let mut system = System::new(fire.count);
        let mut dice = Rng::of(0, 0, 11);
        let mut born = [0_usize; 3];
        for _ in 0..600 {
            let before = system.particles.len();
            system.update(1.0 / 60.0, &fire.layers);
            let survived = system.particles.len();
            system.emit(&fire.blocks, &fire.layers, fire.count, &mut dice);
            for particle in &system.particles[survived..] {
                born[usize::from(particle.layer)] += 1;
            }
            assert!(survived <= before);
        }
        // Each particle takes its layer's frame.
        assert!(system.particles.iter().all(|p| p.frame == p.layer));
        // About 38 a second over 10 s, by the rates' shares 0.38, 0.16 and 0.46.
        let all: usize = born.iter().sum();
        assert!((360..=400).contains(&all), "{all}");
        let share = |k: usize| born[k] as f32 / all as f32;
        assert!((share(0) - 0.385).abs() < 0.06, "{born:?}");
        assert!((share(1) - 0.156).abs() < 0.05, "{born:?}");
        assert!((share(2) - 0.459).abs() < 0.06, "{born:?}");
        // The flames rise; the glows hang about the system's place, not their block's 20 up.
        assert!(
            system
                .particles
                .iter()
                .filter(|p| p.layer == 0)
                .all(|p| p.velocity[2] > 0.0)
        );
        let glows: Vec<_> = system.particles.iter().filter(|p| p.layer == 2).collect();
        assert!(!glows.is_empty() && glows.iter().all(|p| p.place[2].abs() < 2.0));
    }

    #[test]
    fn a_geyser_bursts_a_second_and_one_of_them_sounds() {
        let mut dice = Rng::of(0, 0, 3);
        let mut fire = fire(7).unwrap();
        let mut latch = false;
        let mut geyser = Geyser { clock: 1.5 };
        assert_eq!(
            geyser.update(0.4, &mut fire, &mut latch, &mut dice),
            Jet::Rests
        );
        assert_eq!(fire.blocks[0].interval, 0.1);
        // Below 1, the burst; its sound once, and none for another geyser while it bursts.
        assert_eq!(
            geyser.update(0.2, &mut fire, &mut latch, &mut dice),
            Jet::Bursts { sounds: true }
        );
        assert_eq!(fire.blocks[0].velocity.value[2], 200.0);
        let mut other = Geyser { clock: 0.5 };
        assert_eq!(
            other.update(0.1, &mut fire, &mut latch, &mut dice),
            Jet::Bursts { sounds: false }
        );
        // Past 0, at rest again 30 to 100 s on, the latch open.
        assert_eq!(
            geyser.update(1.0, &mut fire, &mut latch, &mut dice),
            Jet::Rests
        );
        assert!((30.0..=100.0).contains(&geyser.clock) && !latch);
    }

    #[test]
    fn a_creature_bleeds_by_its_class_its_set_and_its_type() {
        assert!(bleeds(30, 1, 0) && bleeds(36, 2, 1) && bleeds(44, 3, 2));
        // No undead, no set of bones nor of none, no dragon.
        assert!(!bleeds(30, 5, 0) && !bleeds(30, 1, 3) && !bleeds(30, 1, 4));
        assert!(!bleeds(52, 1, 0) && !bleeds(1, 1, 0));
    }

    #[test]
    fn a_blood_fountain_spurts_five_seconds_then_dries_up() {
        let mut dice = Rng::of(9, 0, 8);
        let mut fountain = BloodFountain::new(0);
        assert_eq!(
            fountain.ramp.at(255.0),
            [255.0 / 256.0, 12.0 / 256.0, 0.0, 1.0]
        );
        let place = ([0.0, 0.0, 50.0], [0.0, 0.0, 50.0]);
        for _ in 0..50 {
            fountain.update(0.1, place, ([0.0, 0.0, 1.0], 0.0), &mut dice);
        }
        assert_eq!(fountain.strength, 1.0);
        // A drop every 0.005 s living 1.42 s, unless the ground takes it first.
        let count = fountain.system.particles.len();
        assert!((200..=284).contains(&count), "{count}");
        assert!(fountain.system.particles.iter().all(|p| p.place[2] > 0.0));
        fountain.update(0.1, place, ([0.0, 0.0, 1.0], 0.0), &mut dice);
        assert!((fountain.strength - 0.95).abs() < 1e-4);
        for _ in 0..19 {
            fountain.update(0.1, place, ([0.0, 0.0, 1.0], 0.0), &mut dice);
        }
        assert!(fountain.done());
    }

    #[test]
    fn a_blood_fountain_bleeds_the_colour_of_its_set() {
        assert_eq!(
            BloodFountain::blood(1).at(255.0)[..2],
            [12.0 / 256.0, 255.0 / 256.0]
        );
        assert_eq!(BloodFountain::blood(2).at(255.0)[..2], [12.0 / 256.0; 2]);
        assert_eq!(BloodFountain::blood(0).at(0.0), [0.0; 4]);
        let mut dice = Rng::of(1, 2, 3);
        let sounds: std::collections::HashSet<u32> =
            (0..50).map(|_| blood_sound(&mut dice)).collect();
        assert_eq!(sounds.len(), 2);
    }

    #[test]
    fn a_dragons_breath_burns_from_its_head_after_its_wait_and_lays_fires() {
        let mut dice = Rng::of(5, 2, 852);
        let mut fire = DragonFire::new(dragon_fire_variant(88), &mut dice);
        assert_eq!(fire.variant, 2);
        let head = ([0.0, 0.0, 200.0], [0.0, 0.0, 200.0]);
        let mut laid = 0;
        for _ in 0..14 {
            laid += u32::from(fire.update(0.1, head, [0.0, -1.0, 0.0], &mut dice));
        }
        // Nothing for 1.5 s.
        assert!(!fire.burns() && fire.system.particles.is_empty() && laid == 0);
        for _ in 0..11 {
            laid += u32::from(fire.update(0.1, head, [0.0, -1.0, 0.0], &mut dice));
        }
        // Then a jet along the head's −y, at most 0.2 s long, and a fire every 0.3 s.
        assert!(fire.burns() && !fire.system.particles.is_empty());
        assert!(fire.system.particles.iter().all(|p| p.place[1] < 10.0));
        assert!(fire.system.particles.iter().all(|p| p.place[1] > -260.0));
        assert!((3..=4).contains(&laid), "{laid}");
        for _ in 0..16 {
            fire.update(0.1, head, [0.0, -1.0, 0.0], &mut dice);
        }
        assert!(fire.done());
    }

    #[test]
    fn a_fire_on_the_ground_burns_two_seconds_and_dies_down() {
        let mut dice = Rng::of(5, 3, 923);
        let mut fire = GroundFire::new(0);
        for _ in 0..20 {
            fire.update(0.1, &mut dice);
        }
        let count = fire.system.particles.len();
        assert!((45..=51).contains(&count), "{count}");
        assert!(
            fire.system
                .particles
                .iter()
                .all(|p| p.place[0].abs() <= 50.0 && p.place[2] == 0.0)
        );
        assert!(!fire.done());
        for _ in 0..26 {
            fire.update(0.1, &mut dice);
        }
        assert!(fire.done());
        assert_eq!(GroundFire::ramp().at(255.0), [0.0; 4]);
        assert_eq!(GroundFire::ramp().at(128.0)[0], 200.0 / 256.0);
    }

    #[test]
    fn deep_lava_bursts_throw_a_rock_in_an_arc_that_lays_a_small_fire() {
        assert!(deep_lava(5, [-10, -5, -2, -30]));
        assert!(!deep_lava(5, [-10, -5, -1, -30]) && !deep_lava(1, [-10; 4]));
        assert!(!deep_lava(8, [-10; 4]));
        let mut dice = Rng::of(4, 8, 877);
        let mut burst = LavaBurst::new(&mut dice);
        let mut highest = 0.0_f32;
        let mut seconds = 0.0;
        while !burst.update(0.05, &mut dice) {
            highest = highest.max(burst.place()[2]);
            seconds += 0.05;
            assert!(seconds < 3.0);
        }
        // 1.7 to 2.5 s up to 80 to 180 high, 150 to 250 away.
        assert!((1.6..=2.6).contains(&seconds), "{seconds}");
        assert!((79.0..=181.0).contains(&highest), "{highest}");
        // Past the way's end by the last run's step, under it a little: the fire goes on the ground.
        let [x, y, z] = burst.place();
        assert!(
            (149.0..=260.0).contains(&x.hypot(y)) && z < 1.0,
            "{x} {y} {z}"
        );
        assert!(!burst.flies() && !burst.done());
        assert_eq!(
            LavaBurst::ramp().at(255.0),
            [127.0 / 255.0, 49.0 / 255.0, 9.0 / 255.0, 1.0]
        );
        let mut flames = LavaFlames::new(&mut dice);
        assert_eq!(flames.system.particles.len(), 5);
        for _ in 0..40 {
            flames.update(0.05);
        }
        assert!(flames.done());
    }

    #[test]
    fn an_arrows_fire_sparks_along_its_shaft_and_lights_by_its_share() {
        let mut dice = Rng::of(7, 9, 796);
        let mut sparks = ArrowSparks::of_weapon(crate::arrows::Element::Fire, 0.5).unwrap();
        assert!(ArrowSparks::of_weapon(crate::arrows::Element::Magic, 0.5).is_none());
        assert_eq!(sparks.light(), 300.0);
        // An arrow's are of the share 0 whatever its element's: of the size 2, its fire's light 200.
        let arrow = ArrowSparks::of_arrow(crate::arrows::Element::Fire).unwrap();
        assert_eq!(arrow.light(), 200.0);
        assert_eq!(arrow.block().size, spread(2.0, 0.0));
        assert_eq!(arrow.layer().size, 10.0);
        // A weapon's in thousandths, truncated.
        let weapon = ArrowSparks::of_weapon(crate::arrows::Element::Poison, 0.4129).unwrap();
        assert!((weapon.share - 0.412).abs() < 1e-6);
        let shaft = (
            [[0.0, 0.0, 50.0], [60.0, 0.0, 50.0]],
            [[0.0, 0.0, 50.0], [60.0, 0.0, 50.0]],
        );
        for _ in 0..10 {
            sparks.update(0.05, shaft, &mut dice);
        }
        // A spark every 0.1 s, living 0.425 s, on the shaft.
        let count = sparks.system.particles.len();
        assert!((4..=5).contains(&count), "{count}");
        assert!(
            sparks
                .system
                .particles
                .iter()
                .all(|p| (0.0..=60.0).contains(&p.place[0]) && p.place[2] == 50.0)
        );
        assert_eq!(
            sparks.colour(255.0),
            [1.0, 136.0 / 255.0, 68.0 / 255.0, 1.0]
        );
    }

    #[test]
    fn a_burning_bone_burns_white_half_a_second_and_its_flames_rise() {
        let mut dice = Rng::of(7, 8, 788);
        let mut bone = BurningBone::new(0.0, 0);
        assert_eq!(bone.grey, 1.0);
        assert_eq!(BurningBone::new(20.0, 0).grey, 55.0 / 255.0);
        let at = ([0.0, 0.0, 100.0], [0.0, 0.0, 100.0]);
        for _ in 0..10 {
            bone.update(0.05, at, &mut dice);
        }
        assert!(!bone.system.particles.is_empty());
        assert!(bone.system.particles.iter().all(|p| p.velocity[2] > 0.0));
        for _ in 0..12 {
            bone.update(0.05, at, &mut dice);
        }
        assert!(bone.done());
    }

    #[test]
    fn a_line_of_glows_has_a_glow_a_unit_over_its_density_from_its_start() {
        let line = GlowLine {
            density: 0.5,
            half: 2.0,
            argb: 0x40FF_8844,
        };
        let points = line.points([0.0; 3], [10.0, 0.0, 0.0]);
        assert_eq!(points.len(), 5);
        assert_eq!(points[0], [0.0; 3]);
        assert_eq!(points[4], [8.0, 0.0, 0.0]);
        // A line of no length still has its one glow.
        assert_eq!(line.points([1.0; 3], [1.0; 3]), vec![[1.0; 3]]);
        assert_eq!(
            line.colour(),
            [1.0, 136.0 / 255.0, 68.0 / 255.0, 64.0 / 255.0]
        );
    }

    #[test]
    fn a_line_of_glows_runs_over_the_bones_found() {
        assert!(glow_segments(0).is_empty());
        assert_eq!(glow_segments(1), &[(0, 0)]);
        assert_eq!(glow_segments(2), &[(0, 1)]);
        assert_eq!(glow_segments(3), &[(0, 1), (1, 2)]);
    }

    #[test]
    fn the_staff_the_lightsabers_and_the_glittering_wings_draw_their_own() {
        assert_eq!(own_glows(SERAPHIM_STAFF, 0, None).lines.len(), 1);
        assert_eq!(own_glows(LIGHTSABER, 0, None).lines.len(), 2);
        // A type derived from the lightsaber, not one of its model alone.
        assert_eq!(own_glows(2296, LIGHTSABER, None).lines.len(), 2);
        assert_eq!(own_glows(3073, 0, None), OwnGlows::default());
        assert!(own_glows(3111, 0, Some(GLITTERING_WINGS)).glitters);
        assert!(!own_glows(4018, 0, Some(3)).glitters);
    }

    #[test]
    fn a_glitter_is_white_of_a_half_side_of_3_to_4() {
        let mut low = crate::spawning::Script::new(vec![0]);
        let mut high = crate::spawning::Script::new(vec![32_767]);
        assert_eq!(wings_glitter(&mut low).half, 3.0);
        assert_eq!(wings_glitter(&mut high).half, 4.0);
        assert_eq!(wings_glitter(&mut low).colour(), [1.0; 4]);
        assert_eq!(BLUE_GLOW.colour()[2], 1.0);
    }

    #[test]
    fn a_raised_creatures_ring_turns_once_and_its_lines_stand_and_fade() {
        let mut dice = Rng::of(3, 5, 924);
        let mut ring = Reanimate::new();
        for _ in 0..13 {
            ring.update(0.05, &mut dice);
        }
        // Some 0.63 s of a line every 0.004 s, on the circle of 25, none moving.
        let count = ring.system.particles.len();
        assert!((140..=170).contains(&count), "{count}");
        assert!(ring.system.particles.iter().all(|p| {
            let r = p.place[0].hypot(p.place[1]);
            (r - 25.0).abs() < 0.01 && p.place[2] == 0.0
        }));
        assert!(!ring.done());
        for _ in 0..60 {
            ring.update(0.05, &mut dice);
        }
        assert!(ring.done());
        let ramp = Reanimate::ramp();
        assert_eq!(ramp.at(255.0)[3], 0.0);
        assert_eq!(ramp.at(200.0), [100.0 / 256.0, 100.0 / 256.0, 1.0, 1.0]);
    }

    #[test]
    fn a_blast_burns_for_0_8_s_and_its_scorch_lies_15_s() {
        let mut dice = Rng::of(1, 2, 794);
        let mut mine = Explosion::new(false, &mut dice);
        assert_eq!(mine.flames(), 0);
        assert_eq!(mine.ring(), None);
        for _ in 0..16 {
            mine.update(0.05, &mut dice);
        }
        // A flame every 0.02 s over 0.8 s, none dead yet.
        let count = mine.system.particles.len();
        assert!((38..=42).contains(&count), "{count}");
        assert!(!mine.strikes());
        let mut steps = 0;
        while !mine.done() {
            mine.update(0.05, &mut dice);
            steps += 1;
        }
        // Whole until 10 s, then 5 s of fading: 14.2 s past its first 0.8.
        assert!((280..=290).contains(&steps), "{steps}");
        let trap = Explosion::new(true, &mut dice);
        assert_eq!((trap.flames(), trap.ring()), (1, Some(10.0)));
    }

    #[test]
    fn a_mine_puffs_once_and_flashes_two_tenths_in_every_1_7_s() {
        let mut dice = Rng::of(5, 6, 1153);
        let mut mine = DwarfMine::new(&mut dice);
        mine.update(0.05, &mut dice);
        assert_eq!(mine.system.particles.len(), 20);
        let mut flashing = 0;
        for _ in 0..340 {
            mine.update(0.05, &mut dice);
            flashing += usize::from(mine.flashes());
        }
        // 17 s: ten flashes of four or five frames each.
        assert!((36..=50).contains(&flashing), "{flashing}");
        assert!(mine.system.particles.is_empty());
    }

    #[test]
    fn the_mines_sign_stands_its_tongues_on_the_arc_of_the_mines_left() {
        let mut dice = Rng::of(8, 9, 1154);
        let mut sign = MineSign::new();
        for _ in 0..20 {
            sign.update(0.05, 0.25, &mut dice);
        }
        // 200 a second times a quarter over a second.
        let count = sign.system.particles.len();
        assert!((45..=55).contains(&count), "{count}");
        assert!(sign.system.particles.iter().all(|p| {
            let a = p.place[1].atan2(p.place[0]);
            p.place[0].hypot(p.place[1]) > 24.9
                && (-0.01..=std::f32::consts::FRAC_PI_2 + 0.01).contains(&a)
        }));
    }

    #[test]
    fn a_flying_skull_trails_a_glow_every_twentieth_and_ends_after_its_seconds() {
        let mut dice = Rng::of(4, 4, 905);
        let mut skull = FlyingSkull::new(1.0);
        for k in 0..10 {
            let x = f32::from(u8::try_from(k).unwrap_or(0)) * 7.5;
            skull.update(0.05, ([x, 0.0, 50.0], [x + 7.5, 0.0, 50.0]), &mut dice);
        }
        assert_eq!(skull.system.particles.len(), 10);
        assert!(skull.flies().is_some());
        for _ in 0..40 {
            skull.update(0.05, ([0.0; 3], [0.0; 3]), &mut dice);
        }
        assert!(skull.flies().is_none() && skull.done());
        assert_eq!(FlyingSkull::ramp().at(255.0), [0.0; 4]);
        assert!((skull_turn([1.0, 0.0]) - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    }

    #[test]
    fn a_laser_segment_holds_and_fades_by_its_variant_and_lies_along_its_axis() {
        // The hellfire's: 0.5 s, orange, 30 wide, straight.
        let mut laser = LaserSegment::new(LASER_BEAM, (0, 3));
        assert_eq!(laser.colour(), [1.0, 146.0 / 255.0, 39.0 / 255.0, 1.0]);
        let [(across, uvs), _] = laser.quads(53.0);
        assert_eq!(
            (across[0], across[2]),
            ([0.0, -15.0, 0.0], [53.0, 15.0, 0.0])
        );
        assert_eq!(uvs[0], [0.0, 1.0]);
        for _ in 0..15 {
            laser.update(1.0 / 30.0);
        }
        assert!(!laser.done());
        for _ in 0..2 {
            laser.update(1.0 / 30.0);
        }
        assert!(laser.done());
        // The drain's: half grey, 20 wide, pinned at the chain's start, a quarter of the texture.
        let drain = LaserSegment::new(DRAIN_BEAM, (0, 4));
        assert_eq!(drain.colour()[0], 127.0 / 255.0);
        let [(across, uvs), (upright, _)] = drain.quads(53.0);
        assert_eq!(
            (across[0], upright[0]),
            ([0.0, -10.0, 0.0], [0.0, 0.0, -10.0])
        );
        assert_eq!((uvs[0], uvs[2]), ([0.0, 0.25], [1.0, 0.0]));
        // The bite's holds 4.4 s.
        let mut bite = LaserSegment::new(BITE_BEAM, (1, 2));
        for _ in 0..130 {
            bite.update(1.0 / 30.0);
        }
        assert!(!bite.done());
        // A segment along y turns its x onto y, its z kept up.
        let [x, _, z] = laser_axes([0.0, 2.0, 0.0]);
        assert!((x[1] - 1.0).abs() < 1e-6 && (z[2] - 1.0).abs() < 1e-6);
        assert_eq!(LaserSegment::texture(LASER_BEAM), "LASERBEAMPART");
        assert_eq!(LaserSegment::sound(BITE_BEAM), Some(2076));
    }

    #[test]
    fn a_killer_plant_grows_from_its_foot_stands_and_sinks_with_its_seven_spurs() {
        let mut dice = Rng::of(5, 5, 848);
        let mut plant = KillerPlant::new(2.0, true, &mut dice);
        assert_eq!(plant.colour, PLANT_GREEN);
        assert!(plant.dots().is_empty());
        assert_eq!(plant.glow(), 0.0);
        // A third of its rise: the dots of a width above 2, its lower half.
        for _ in 0..10 {
            plant.update(1.0 / 30.0);
        }
        let low = plant.dots();
        assert_eq!(low.len(), 64);
        assert!(low.iter().all(|&([_, _, z], _)| z < 32.0));
        // Risen: the 128 dots of its vine to 63.5 high and its seven spurs' 14 each.
        for _ in 0..25 {
            plant.update(1.0 / 30.0);
        }
        let dots = plant.dots();
        assert_eq!(dots.len(), 128 + 7 * 14);
        assert_eq!(plant.glow(), 15.0);
        assert!((dots[0].1 - 3.0).abs() < 1e-6);
        assert!(dots.iter().any(|&([_, _, z], _)| z == 63.5));
        // Its two seconds, then its sink of a second.
        for _ in 0..60 {
            plant.update(1.0 / 30.0);
        }
        assert!(!plant.done());
        for _ in 0..32 {
            plant.update(1.0 / 30.0);
        }
        assert!(plant.done() && plant.dots().is_empty());
        assert_eq!(KillerPlant::new(3.0, false, &mut dice).colour, PLANT_BLUE);
    }

    #[test]
    fn a_tentacle_stands_its_seconds_over_its_rune_rings_then_sinks_in_a_second() {
        let mut dice = Rng::of(6, 6, 1013);
        let mut tentacle = Tentacle::new(1.0, &mut dice);
        assert_eq!(tentacle.runes.len(), 50);
        assert!(tentacle.runes.iter().all(|&rune| (1..=15).contains(&rune)));
        let dots = tentacle.dots();
        assert_eq!(dots.len(), 5 * 128);
        assert!((dots[0].1 - 3.5).abs() < 1e-6);
        // Its tips 63.5 · 127/128 high, its feet within 8 of the corpse.
        assert!(
            dots.iter()
                .all(|&([x, y, z], _)| z < 63.1 && (z > 0.0 || x.abs().max(y.abs()) <= 8.0))
        );
        let rings = tentacle.rings();
        assert_eq!(rings.len(), 2);
        assert_eq!(
            (rings[0].inner, rings[1].outer, rings[1].segments),
            (25.0, 39.0, 17)
        );
        let segment = rings[0].segments()[0];
        assert_eq!(segment.0, 0);
        assert!((segment.1[0][1] - 25.0).abs() < 1e-4 && (segment.1[1][1] - 32.0).abs() < 1e-4);
        for _ in 0..45 {
            tentacle.update(1.0 / 30.0);
        }
        assert!(!tentacle.done() && tentacle.rings()[0].inner < 25.0);
        for _ in 0..30 {
            tentacle.update(1.0 / 30.0);
        }
        assert!(tentacle.done() && tentacle.dots().is_empty());
        let mut aura = TentacleAura::new(2.0, &mut dice);
        assert_eq!(
            aura.rings().iter().map(|ring| ring.segments).sum::<u16>(),
            98
        );
        for _ in 0..85 {
            aura.update(1.0 / 30.0);
        }
        assert!(!aura.done());
        for _ in 0..6 {
            aura.update(1.0 / 30.0);
        }
        assert!(aura.done());
        assert_eq!(tentacle_rune(15), "fx_tentacle_rune15");
    }

    #[test]
    fn a_thrown_ball_trails_its_glows_tumbling_and_ends_with_its_last() {
        let mut dice = Rng::of(7, 7, 867);
        let colour = ball_colour(1052).unwrap_or_default();
        assert_eq!(colour, [32.0 / 255.0, 1.0, 32.0 / 255.0, 1.0]);
        assert_eq!(ball_colour(1048), None);
        let mut ball = TrapBall::new(colour, &mut dice);
        for k in 0..10u8 {
            let x = f32::from(k) * 5.0;
            ball.update(0.01, ([x, 0.0, 40.0], [x + 5.0, 0.0, 40.0]), &mut dice);
        }
        // Two a hundredth of a second, the first born 0.1 s ago and still alive (7 ± 2 by 30).
        assert!((19..=20).contains(&ball.system.particles.len()));
        assert!(ball.turns.iter().all(|turn| (0.2..=0.4).contains(turn)));
        assert!(ball.glow_colour(127.5)[3] - 0.5 < 1e-6);
        ball.end();
        for _ in 0..40 {
            ball.update(0.01, ([50.0, 0.0, 40.0], [50.0, 0.0, 40.0]), &mut dice);
        }
        assert!(ball.done());
    }

    #[test]
    fn a_demon_fire_grows_bursts_its_rays_while_it_strikes_and_shrinks() {
        let mut dice = Rng::of(8, 8, 1015);
        let mut fire = DemonFire::new((3.0, 5.0));
        for _ in 0..30 {
            fire.update(1.0 / 30.0, &mut dice);
        }
        // A second on: half grown, its rays of the last second alive, 250 a second.
        assert!((fire.growth() - 0.5).abs() < 1e-3);
        assert!((240..=251).contains(&fire.system.particles.len()));
        let [(inner, _), (outer, _)] = fire.spheres();
        assert!(inner < outer && inner > 0.1);
        for _ in 0..90 {
            fire.update(1.0 / 30.0, &mut dice);
        }
        // Past its strikes: shrinking, no more rays made.
        assert!((fire.growth() - 0.5).abs() < 1e-2);
        assert!(fire.system.particles.len() < 240 && !fire.done());
        for _ in 0..31 {
            fire.update(1.0 / 30.0, &mut dice);
        }
        assert!(fire.done());
        assert_eq!((lava_frame(0.5, 70.0), lava_frame(0.71, 70.0)), (35, 0));
        assert_eq!(lava_frame_name(7), "A_LAVA07");
    }

    #[test]
    fn a_hell_disk_trails_its_glows_and_ends_with_its_last() {
        let mut dice = Rng::of(9, 9, 1005);
        let mut disk = HellDisk::new();
        for k in 0..20u8 {
            let x = f32::from(k) * 4.0;
            disk.update(0.015, ([x, 0.0, 60.0], [x + 4.0, 0.0, 60.0]), &mut dice);
        }
        // One every 0.015 s.
        assert!((19..=20).contains(&disk.system.particles.len()));
        assert!((disk.glow() - (40.0 + 10.0 * 0.3_f32.sin())).abs() < 1e-3);
        // At 0.3 s: the frame 30, the copy's 80 less 49.5.
        let [own, copy] = disk.frames();
        assert!((29..=30).contains(&own) && copy == 30);
        disk.end();
        for _ in 0..90 {
            disk.update(0.015, ([80.0, 0.0, 60.0], [80.0, 0.0, 60.0]), &mut dice);
        }
        assert!(disk.done());
        assert_eq!(HellDisk::ramp().at(255.0)[3], 0.0);
    }

    #[test]
    fn a_pentagram_draws_itself_bursts_into_fire_and_ends() {
        let mut dice = Rng::of(10, 10, 782);
        let mut pentagram = PentagramKiller::new(30.0);
        for _ in 0..30 {
            pentagram.update(1.0 / 30.0, false, &mut dice);
        }
        // Half drawn: 80 of the ring's and 40 of each side's.
        assert_eq!(pentagram.sparks(&mut dice).len(), 80 + 5 * 40);
        for _ in 0..31 {
            pentagram.update(1.0 / 30.0, false, &mut dice);
        }
        // Armed: the ring's 160, the comets' 80 and the star's 400.
        assert_eq!(pentagram.sparks(&mut dice).len(), 160 + 80 + 400);
        assert!(
            pentagram
                .sparks(&mut dice)
                .iter()
                .all(|&[x, y, _]| (x * x + y * y).sqrt() <= 40.001)
        );
        pentagram.update(1.0 / 30.0, true, &mut dice);
        for _ in 0..15 {
            pentagram.update(1.0 / 30.0, true, &mut dice);
        }
        assert!(!pentagram.system.particles.is_empty());
        assert!(
            pentagram
                .system
                .particles
                .iter()
                .all(|p| p.velocity[2] >= 200.0)
        );
        for _ in 0..60 {
            pentagram.update(1.0 / 30.0, true, &mut dice);
        }
        assert!(pentagram.done() && pentagram.sparks(&mut dice).is_empty());
        // Unburst, it fades out over 2 s past its seconds.
        let mut expired = PentagramKiller::new(3.0);
        for _ in 0..90 {
            expired.update(1.0 / 30.0, false, &mut dice);
        }
        for _ in 0..58 {
            expired.update(1.0 / 30.0, true, &mut dice);
        }
        assert!(!expired.done());
        for _ in 0..5 {
            expired.update(1.0 / 30.0, true, &mut dice);
        }
        assert!(expired.done());
    }

    #[test]
    fn a_confusions_helix_climbs_fading_in_and_out_and_its_spinner_rises() {
        let mut dice = Rng::of(11, 11, 871);
        let mut confusion = ConfusionCircle::new(&mut dice);
        for _ in 0..100 {
            confusion.update(1.0 / 30.0, &mut dice);
        }
        // A phase of 0.5: fully shown, its helix from 50 to 74.5 high.
        assert!(confusion.alpha() > 0.99);
        let [flat, upright] = confusion.helix();
        assert_eq!((flat.len(), upright.len()), (50, 50));
        assert!((flat[0][0][2] - 25.0).abs() < 0.1 && (flat[49][0][2] - 49.5).abs() < 0.1);
        assert!((upright[0][0][2] - upright[0][1][2] - 4.0).abs() < 1e-4);
        let (big, small, rise) = confusion.circles();
        assert!((small + 5.0 * big).abs() < 1e-4 && rise > 1.0);
        let mut poison = GroundPoison::new();
        for _ in 0..30 {
            poison.update(1.0 / 30.0, &mut dice);
        }
        // 25 a second, rising.
        assert!((24..=26).contains(&poison.system.particles.len()));
        assert!(
            poison
                .system
                .particles
                .iter()
                .all(|p| p.frame < 4 && p.place[2] >= 0.0)
        );
        assert_eq!(GroundPoison::ramp().at(255.0), [0.5, 1.0, 0.5, 1.0]);
    }

    #[test]
    fn a_homing_fire_trails_its_flames_and_bursts_into_a_ball() {
        let mut dice = Rng::of(13, 13, 910);
        let mut fire = HomingFlames::new(HomingKind::Demon);
        for k in 0..30u8 {
            let x = f32::from(k) * 4.0;
            fire.update(
                1.0 / 60.0,
                ([x, 0.0, 60.0], [x + 4.0, 0.0, 60.0]),
                &mut dice,
            );
        }
        // Half a second at one every 0.006 s, each living 0.85 s.
        assert!((80..=84).contains(&fire.system.particles.len()));
        assert!(fire.system.particles.iter().all(|p| p.place[2] >= 40.0));
        fire.burst([120.0, 0.0, 60.0], &mut dice);
        assert_eq!(fire.system.particles.len(), 300);
        for _ in 0..80 {
            fire.update(
                1.0 / 60.0,
                ([120.0, 0.0, 60.0], [120.0, 0.0, 60.0]),
                &mut dice,
            );
        }
        assert!(fire.done());
        let mut guard = HomingFlames::new(HomingKind::Guard);
        guard.update(0.1, ([0.0; 3], [0.0; 3]), &mut dice);
        assert!(guard.system.particles.iter().all(|p| p.size < 8.0));
        assert_eq!(HomingFlames::ramp().at(251.0)[3], 0.352);
    }

    #[test]
    fn a_whirlwind_leaves_its_dust_and_a_fire_bolt_carries_its_smoke() {
        let mut dice = Rng::of(14, 14, 831);
        let mut dust = BoltTrail::new(TrailKind::Windstrike);
        for k in 0..30u8 {
            let x = f32::from(k) * 3.3;
            dust.update(1.0 / 60.0, ([x, 0.0, 0.0], [x + 3.3, 0.0, 0.0]), &mut dice);
        }
        // Half a second at a hundred a second, each living 0.64 s and rising to at most
        // 400² / 1400.
        assert!((48..=51).contains(&dust.system.particles.len()));
        assert!(
            dust.system
                .particles
                .iter()
                .all(|p| p.place[2] >= 0.0 && p.place[2] < 115.0)
        );
        dust.end(true);
        assert!(dust.done());
        let mut smoke = BoltTrail::new(TrailKind::Shaddar);
        for _ in 0..30 {
            smoke.update(1.0 / 60.0, ([0.0; 3], [5.0, 0.0, 0.0]), &mut dice);
        }
        // Twenty a second about its origin.
        assert!((9..=11).contains(&smoke.system.particles.len()));
        assert!(
            smoke
                .system
                .particles
                .iter()
                .all(|p| p.place.iter().all(|v| v.abs() < 20.0))
        );
        smoke.end(false);
        assert!(!smoke.done());
        for _ in 0..40 {
            smoke.update(1.0 / 60.0, ([0.0; 3], [0.0; 3]), &mut dice);
        }
        assert!(smoke.done());
        assert_eq!(BoltTrail::ramp().at(255.0)[3], 0.781);
    }

    #[test]
    fn the_splinters_trail_rings_and_burst_into_sparks() {
        let mut dice = Rng::of(14, 14, 839);
        let mut ice = IceSplinters::new([0.0, 0.0, 60.0], true, &mut dice);
        assert!(ice.offsets.iter().flatten().all(|v| v.abs() <= 20.0));
        // A ring's place every 0.1 s: four places after 0.3 s and more.
        for k in 0..30u8 {
            ice.update(0.0125, [f32::from(k) * 2.0, 0.0, 60.0], 0.0);
        }
        assert_eq!(ice.history.iter().flatten().count(), 4);
        let rings = ice.rings();
        assert_eq!(rings.len(), 20);
        assert!(
            rings
                .iter()
                .all(|&(_, half, _)| (2.0..=10.0).contains(&half))
        );
        ice.burst([60.0, 0.0, 60.0], [1.0, 0.0, 0.0], &mut dice);
        assert!(ice.rings().is_empty() && !ice.flying());
        assert_eq!(ice.system.particles.len(), 200);
        // They fall and bounce off the ground and live 1.7 s.
        for _ in 0..100 {
            ice.update(0.0125, [60.0, 0.0, 60.0], 0.0);
        }
        assert!(ice.system.particles.iter().all(|p| p.place[2] > -20.0));
        for _ in 0..80 {
            ice.update(0.0125, [60.0, 0.0, 60.0], 0.0);
        }
        assert!(ice.done());
        assert_eq!(
            ice.ramp().at(255.0),
            [127.0 / 255.0, 127.0 / 255.0, 1.0, 0.0]
        );
        let grey = IceSplinters::new([0.0; 3], false, &mut dice)
            .ramp()
            .at(255.0);
        assert_eq!(grey, [63.0 / 255.0, 63.0 / 255.0, 63.0 / 255.0, 0.0]);
    }

    #[test]
    fn a_bubble_grows_flies_and_shrinks_or_bursts() {
        let mut bubble = BubbleTrap::new(7.35);
        bubble.update(0.2);
        assert!((bubble.scale - 0.6).abs() < 1e-5 && bubble.drawn());
        bubble.update(0.2);
        assert_eq!(bubble.scale, 1.0);
        // Half a second before its end it shrinks, gone at its end.
        for _ in 0..66 {
            bubble.update(0.1);
        }
        assert!(bubble.scale < 1.0 && bubble.scale > 0.0);
        for _ in 0..6 {
            bubble.update(0.1);
        }
        assert!(bubble.done() && !bubble.drawn());
        // A cell stops it: it shrinks where it is.
        let mut stopped = BubbleTrap::new(7.35);
        stopped.update(0.5);
        stopped.stop();
        stopped.update(0.3);
        assert!((stopped.scale - 0.4).abs() < 1e-5 && !stopped.done());
        // An arrow bursts it into 75 green glows living 1.7 s.
        let mut dice = Rng::of(14, 14, 847);
        let mut popped = BubbleTrap::new(7.35);
        popped.update(1.0);
        popped.burst([0.0, 0.0, 60.0], &mut dice);
        assert!(!popped.drawn() && popped.system.particles.len() == 75);
        for _ in 0..18 {
            popped.update(0.1);
        }
        assert!(popped.done());
        assert_eq!(
            BubbleTrap::ramp().at(255.0),
            [127.0 / 255.0, 1.0, 127.0 / 255.0, 0.0]
        );
        let mut ring = SonarRing::default();
        assert_eq!(ring.radii(), (10.0, 40.0));
        assert_eq!(ring.colour(), [128.0 / 255.0, 1.0, 128.0 / 255.0, 1.0]);
        ring.update(0.25);
        assert_eq!(ring.radii(), (72.5, 102.5));
        assert_eq!(
            ring.colour(),
            [64.0 / 255.0, 127.0 / 255.0, 64.0 / 255.0, 1.0]
        );
        ring.update(0.3);
        assert!(ring.done());
    }

    #[test]
    fn a_soulcatcher_twirls_over_its_bone_and_swells_at_a_soul() {
        let mut dice = Rng::of(14, 14, 869);
        let mut catcher = Soulcatcher::default();
        for _ in 0..60 {
            catcher.update(1.0 / 60.0, [0.0, 0.0, 100.0], &mut dice);
        }
        // Fifty a second, four in five gold living 0.51 s, one in five black living 1.02 s.
        let n = catcher.system.particles.len();
        assert!((25..=40).contains(&n), "{n}");
        assert!(
            catcher
                .system
                .particles
                .iter()
                .all(|p| (p.place[2] - 140.0).abs() <= 1.0 && (12.0..=28.0).contains(&p.size))
        );
        assert_eq!(catcher.glow(), None);
        catcher.absorb();
        assert_eq!(catcher.glow(), Some(40.0));
        catcher.update(0.5, [0.0, 0.0, 100.0], &mut dice);
        assert_eq!(catcher.pulse, 13.5);
        let [gold, black] = Soulcatcher::ramps();
        assert_eq!(gold.at(128.0)[3], 0.25);
        assert_eq!(black.at(128.0)[3], 0.996);
    }

    #[test]
    fn a_souls_sparks_stream_to_its_place_and_fade_where_it_arrives() {
        let points = (0..600u16).map(|k| [f32::from(k % 10), f32::from(k / 10), 0.0]);
        let mut soul = SoulSparks::new(points);
        assert_eq!(soul.system.particles.len(), SOUL_SPARKS);
        let centre = [200.0, 0.0, 90.0];
        let far = |soul: &SoulSparks| {
            soul.system
                .particles
                .iter()
                .map(|p| {
                    let d: [f32; 3] = std::array::from_fn(|k| p.place[k] - centre[k]);
                    d.iter().map(|v| v * v).sum::<f32>().sqrt()
                })
                .fold(0.0, f32::max)
        };
        let before = far(&soul);
        for _ in 0..60 {
            soul.update(1.0 / 60.0, centre);
        }
        // Drawn toward its place, the size shrunk by about 1.75.
        assert!(far(&soul) < before);
        let size = soul.system.particles[0].size;
        assert!((size - (5.0 - 2.0 + 0.25)).abs() < 0.05, "{size}");
        soul.arrive();
        assert!(soul.system.particles.iter().all(|p| p.velocity == [0.0; 3]));
        for _ in 0..62 {
            soul.update(1.0 / 60.0, centre);
        }
        assert!(soul.done());
    }

    #[test]
    fn a_layers_pull_turns_its_particles_toward_the_place() {
        let mut system = System::new(1);
        system.particles.push(Particle {
            place: [10.0, 0.0, 0.0],
            velocity: [0.0; 3],
            buoyancy: 0.0,
            size: 1.0,
            life: 255.0,
            angle: 0.0,
            spin: 0.0,
            layer: 0,
            frame: 0,
        });
        let layers = [Layer {
            pull: 50.0,
            ..Layer::default()
        }];
        system.update_toward(0.1, &layers, [0.0; 3]);
        assert_eq!(system.particles[0].velocity, [-5.0, 0.0, 0.0]);
    }
}
