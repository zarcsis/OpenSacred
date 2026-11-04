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
