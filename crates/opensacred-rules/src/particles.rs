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
    /// A frame of the texture's grid at random below it, the first for 0 or 1 (`+0x60`).
    pub frames: u8,
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
        self.update_over(dt, layers, None);
    }

    /// [`Self::update`], each particle bouncing at the height `ground` of the system's frame
    /// without loss (`0x7645C0`, the mode 0): its vertical velocity turned about where its place
    /// lies at or below it, after its move and before its layer's rates.
    pub fn update_over(&mut self, dt: f32, layers: &[Layer], ground: Option<f32>) {
        self.clock += dt;
        for particle in &mut self.particles {
            let Some(layer) = layers.get(usize::from(particle.layer)) else {
                particle.life = 0.0;
                continue;
            };
            age(particle, dt, layer, ground);
        }
        self.particles.retain(Particle::alive);
    }

    /// Emits, at the system's place (the origin), as many particles as whole intervals of the
    /// first block have passed since the last emission, each of a block that `pick` chooses and
    /// aged by its share of the time (`0x764760`), up to `count` alive.
    pub fn emit(&mut self, blocks: &[Block], layers: &[Layer], count: usize, dice: &mut impl Dice) {
        self.emit_at(blocks, layers, count, dice, |_| [0.0; 3]);
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
        self.emit_at(blocks, layers, count, dice, |moment| {
            let share = if dt > 0.0 {
                ((moment - start) / dt).clamp(0.0, 1.0)
            } else {
                1.0
            };
            std::array::from_fn(|k| from[k] + (to[k] - from[k]) * share)
        });
    }

    /// [`Self::emit`], each particle born at its offset from `place` of its moment.
    fn emit_at(
        &mut self,
        blocks: &[Block],
        layers: &[Layer],
        count: usize,
        dice: &mut impl Dice,
        place: impl Fn(f32) -> [f32; 3],
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
            let layer = pick(blocks.len(), dice);
            let (Some(block), Some(rates)) = (blocks.get(layer), layers.get(layer)) else {
                continue;
            };
            let mut particle = born(block, layer, dice);
            for (axis, at) in particle.place.iter_mut().zip(place(self.last)) {
                *axis += at;
            }
            age(&mut particle, self.clock - self.last, rates, None);
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
        frame: if block.frames > 1 {
            u8::try_from(dice.below(u32::from(block.frames))).unwrap_or(0)
        } else {
            0
        },
    }
}

/// A particle aged by `dt` seconds under the layer `layer`, bouncing at `ground` where given.
fn age(particle: &mut Particle, dt: f32, layer: &Layer, ground: Option<f32>) {
    for k in 0..3 {
        particle.place[k] += dt * particle.velocity[k];
    }
    let fall = dt * particle.buoyancy;
    particle.velocity[2] -= fall;
    for k in 0..3 {
        particle.velocity[k] += fall * layer.wind[k];
    }
    particle.angle += dt * particle.spin;
    if ground.is_some_and(|ground| particle.place[2] <= ground) {
        particle.velocity[2] = -particle.velocity[2];
    }
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
/// geyser, 882 and 881 the lavas' smokes, 886, 885 and 884 the fires.
pub fn fire_variant(kind: u32) -> Option<u8> {
    match kind {
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
        let mut z = t * HELIX_RISE + HELIX_HEIGHT;
        let mut angle = t * HELIX_TURN + self.turn;
        let mut flat = Vec::with_capacity(usize::from(HELIX_SEGMENTS));
        let mut upright = Vec::with_capacity(usize::from(HELIX_SEGMENTS));
        for _ in 0..HELIX_SEGMENTS {
            let (sin, cos) = angle.sin_cos();
            let at = |radius: f32, height: f32| [cos * radius, sin * radius, height];
            flat.push([
                at(HELIX_RADIUS - HELIX_HALF, z),
                at(HELIX_RADIUS + HELIX_HALF, z),
            ]);
            upright.push([
                at(HELIX_RADIUS, z + HELIX_HALF),
                at(HELIX_RADIUS, z - HELIX_HALF),
            ]);
            z += HELIX_RISE;
            angle += HELIX_TURN;
        }
        [flat, upright]
    }
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
    let inner = 150.0 * t;
    Some((
        inner,
        inner + 40.0,
        [byte(34.0), byte(128.0), byte(34.0), 1.0],
    ))
}

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
        ramp: Ramp::new(white, &[(white, 100), ([0.0, 0.0, 0.0, 1.0], 156)]),
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
}

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

    /// Its dots' ramp (`+0x20B0`): (60, 255, 60) out of 256, from opaque to nothing.
    pub fn ramp() -> Ramp {
        let green = |alpha: f32| [60.0 / 256.0, 255.0 / 256.0, 60.0 / 256.0, alpha];
        Ramp::new(green(255.0 / 256.0), &[(green(0.0), 256)])
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
                    let layer = pick(blocks.len(), dice);
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
        assert_eq!(LostSoul::ramp().at(255.0)[1], 255.0 / 256.0);
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
}
