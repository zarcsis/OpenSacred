//! The scripts' traps (#2916; docs/re/effects.md, "The scripts' traps"): the two FX objects that a
//! sector's `Enter` makes, each of which makes the rest itself (`0x5A3840`, `0x5AA7E0`).
//!
//! The field (`FX_TRAP02_PARENT`, 891) lays a pill (`FX_TRAP02_PILL`, 892) on every point of a
//! grid two cells apart that may be walked on, and up to twelve ghosts (`FX_TRAP02_GHOST`, 893)
//! among them, which walk the grid's lines ([`Field`]); the local hero coming near a pill takes it
//! for experience, and near a ghost takes a strike, at most once a second for the whole field.
//!
//! The fences (`FX_TRAP03_PARENT`, 894) send a slow fence of lightning every 3 s and a fast one
//! back every 5.5 s ([`Fences`]), eight segments (`FX_TRAP03_LASER`, 895) each ([`fence`]), which
//! fly through walls, fade, and strike every living creature they pass, each at most once a
//! second ([`Laser`]).
//!
//! The field's places are the world's on the ground, in its units: `x` along the cells' columns
//! and `y` along their rows (the game's X and Z). The fences' are in the models' frame about their
//! maker's point (`x` along the screen's horizontal on the ground, `y` toward the viewer, `z` up).

use crate::spawning::Dice;

/// The FX types of the field and of the fences, the two that the scripts make.
pub const FIELD: u32 = 891;
pub const FENCES: u32 = 894;

/// The field's grid: its step, twice a cell's side (`0x891994`), and its columns and rows
/// (`0x5A4113`…`0x5A4556`), from the field's point toward the world's +x and +y alone.
pub const STEP: f32 = 2.0 * 53.665_63;
pub const COLUMNS: u8 = 21;
pub const ROWS: u8 = 19;
/// The most ghosts a field makes, and the bit that both of a ghost's indices hold (`i & 4`).
pub const GHOSTS: usize = 12;
const GHOST_BIT: u8 = 4;
/// The squared distance within which the hero takes a pill or a ghost's strike (`0x5AB010`):
/// about 63 world units.
pub const TOUCH: f32 = 4000.0;
/// A pill's experience (the event `0x10D`'s `+0x20`).
pub const PILL_EXPERIENCE: u64 = 10;
/// A ghost's strike, from nobody (the event 5's attacker 0): 200 of each of the four kinds, at
/// most once a second for the whole field (its `+0x11C`).
pub const GHOST_DAMAGES: [f32; 4] = [200.0; 4];
pub const GHOST_PERIOD: f32 = 1.0;
/// A ghost's steps a second along the grid (`+0xA8` growing by twice the time), the last index
/// it keeps to on both axes, the tries at a heading, and the share of a free heading that it
/// keeps (`rand() % 11` below 7).
const GHOST_STEPS: f32 = 2.0;
const LAST_INDEX: i32 = 20;
const TRIES: u32 = 50;
const KEEPS: (u32, u32) = (7, 11);

/// The looks of the pills and the ghosts (`0x7A21A0`, `0x7A24A0`): a glow each, 20 over its
/// point, of its texture, its half side beating once a second by 3 from 10 or from 15
/// (`0x768790`); the pill's yellow, added whole, the ghost's blue, added by its alpha.
pub const PILL_TEXTURE: &str = "PARTICLE_BUBBLE01";
pub const GHOST_TEXTURE: &str = "PARTICLE_FACE01";
pub const PILL_COLOUR: [f32; 4] = [1.0, 235.0 / 255.0, 9.0 / 255.0, 1.0];
pub const GHOST_COLOUR: [f32; 4] = [15.0 / 255.0, 38.0 / 255.0, 1.0, 1.0];
pub const GLOW_LIFT: f32 = 20.0;
const PILL_HALF: f32 = 10.0;
const GHOST_HALF: f32 = 15.0;
const BEAT: f32 = 3.0;

/// A piece of a field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Piece {
    /// A pill at its place on the ground, until the hero takes it.
    Pill { at: [f32; 2], taken: bool },
    /// A ghost walking the grid.
    Ghost(Ghost),
}

/// A ghost of a field (`0x5A8C45`…`0x5A914E`): its grid's indices, its heading (`+0xA4`, 0 to 3:
/// the world's +x, +y, −x, −y), its step toward the next point (`+0xA8`, 0 to 1), and its place
/// in the world as the last frame left it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ghost {
    pub i: i32,
    pub j: i32,
    pub heading: u8,
    pub step: f32,
    pub at: [f32; 2],
}

/// The world's step of a ghost's `heading` along x and y.
pub fn heading(heading: u8) -> (i32, i32) {
    match heading & 3 {
        0 => (1, 0),
        1 => (0, 1),
        2 => (-1, 0),
        _ => (0, -1),
    }
}

/// A grid's index or a heading's step as a float.
fn float(n: i32) -> f32 {
    f32::from(i16::try_from(n).unwrap_or(0))
}

/// The world's place of the grid's point `(i, j)` of a field at `origin`, truncated as the
/// original's (`0x5A4113`): a cell's corner, or a hair before it.
pub fn point(origin: [f32; 2], i: f32, j: f32) -> [f32; 2] {
    [
        (origin[0] + STEP * i).floor(),
        (origin[1] + STEP * j).floor(),
    ]
}

impl Ghost {
    /// A ghost made at the grid's point `(i, j)` of a field at `origin`, its heading a roll
    /// (`0x5A4565`: `rand() % 4`).
    pub fn new(origin: [f32; 2], (i, j): (u8, u8), dice: &mut impl Dice) -> Self {
        Self {
            i: i32::from(i),
            j: i32::from(j),
            heading: u8::try_from(dice.below(4)).unwrap_or(0),
            step: 0.0,
            at: point(origin, f32::from(i), f32::from(j)),
        }
    }

    /// The ghost `dt` seconds on: a step every half second, then the next point: on along its
    /// heading where the point a step on from its place lies on a cell that may be walked on
    /// (`walkable`), its indices stay within the grid, and a roll keeps it, else a new heading at
    /// random, up to 50 tries; its place along the line between the two points.
    pub fn update(
        &mut self,
        dt: f32,
        origin: [f32; 2],
        walkable: impl Fn([f32; 2]) -> bool,
        dice: &mut impl Dice,
    ) {
        self.step += GHOST_STEPS * dt;
        if self.step >= 1.0 {
            let (dx, dy) = heading(self.heading);
            self.i += dx;
            self.j += dy;
            for _ in 0..TRIES {
                let (dx, dy) = heading(self.heading);
                let (i, j) = (self.i + dx, self.j + dy);
                let next = [self.at[0] + STEP * float(dx), self.at[1] + STEP * float(dy)];
                if (0..=LAST_INDEX).contains(&i)
                    && (0..=LAST_INDEX).contains(&j)
                    && walkable(next)
                    && dice.below(KEEPS.1) < KEEPS.0
                {
                    break;
                }
                self.heading = u8::try_from(dice.below(4)).unwrap_or(0);
            }
            self.step = 0.0;
        }
        let (dx, dy) = heading(self.heading);
        self.at = point(
            origin,
            float(self.i) + self.step * float(dx),
            float(self.j) + self.step * float(dy),
        );
    }
}

/// What the hero touched in a frame ([`Field::touch`]): the pill it took, if any, and whether a
/// ghost struck it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Touched {
    pub pill: Option<usize>,
    pub struck: bool,
}

/// A field of pills and ghosts (`cParticleSystem_trap02_parent` and its object): its point, its
/// pieces in the order made, the cooldown of its ghosts' strike (`+0x11C`), and its clock, by
/// which every pill and ghost beats in step.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub origin: [f32; 2],
    pub pieces: Vec<Piece>,
    pub cooldown: f32,
    pub clock: f32,
}

impl Field {
    /// The field of the point `origin` (`0x5A40E2`): for each column and, within it, each row, a
    /// pill at the grid's point where its cell may be walked on (`walkable`), and there a ghost
    /// too where both indices hold the bit 4, up to twelve.
    pub fn new(
        origin: [f32; 2],
        walkable: impl Fn([f32; 2]) -> bool,
        dice: &mut impl Dice,
    ) -> Self {
        let mut pieces = Vec::new();
        let mut ghosts = 0;
        for i in 0..COLUMNS {
            for j in 0..ROWS {
                let at = point(origin, f32::from(i), f32::from(j));
                if !walkable(at) {
                    continue;
                }
                pieces.push(Piece::Pill { at, taken: false });
                if ghosts < GHOSTS && i & GHOST_BIT != 0 && j & GHOST_BIT != 0 {
                    pieces.push(Piece::Ghost(Ghost::new(origin, (i, j), dice)));
                    ghosts += 1;
                }
            }
        }
        Self {
            origin,
            pieces,
            cooldown: 0.0,
            clock: 0.0,
        }
    }

    /// The field `dt` seconds on: its clock, its strike's cooldown down to 0 (`0x5A9A10`), and
    /// each ghost's walk.
    pub fn update(&mut self, dt: f32, walkable: impl Fn([f32; 2]) -> bool, dice: &mut impl Dice) {
        self.clock += dt;
        self.cooldown = (self.cooldown - dt).max(0.0);
        let origin = self.origin;
        for piece in &mut self.pieces {
            if let Piece::Ghost(ghost) = piece {
                ghost.update(dt, origin, &walkable, dice);
            }
        }
    }

    /// What the hero at `hero` touches (`0x5AB010`), its pieces taken in the order made: the
    /// first pill within reach is taken and ends the frame's test; a ghost within reach strikes
    /// while the cooldown is 0, which it sets to a second.
    pub fn touch(&mut self, hero: [f32; 2]) -> Touched {
        let near = |at: [f32; 2]| {
            let (dx, dy) = (at[0] - hero[0], at[1] - hero[1]);
            dx * dx + dy * dy < TOUCH
        };
        let mut touched = Touched::default();
        for (index, piece) in self.pieces.iter_mut().enumerate() {
            match piece {
                Piece::Pill { at, taken } if !*taken && near(*at) => {
                    *taken = true;
                    touched.pill = Some(index);
                    return touched;
                }
                Piece::Ghost(ghost) if self.cooldown <= 0.0 && near(ghost.at) => {
                    self.cooldown = GHOST_PERIOD;
                    touched.struck = true;
                }
                _ => {}
            }
        }
        touched
    }

    /// The half sides of a pill's glow and of a ghost's at the field's clock.
    pub fn halves(&self) -> [f32; 2] {
        let beat = BEAT * crate::particles::beat(self.clock);
        [PILL_HALF + beat, GHOST_HALF + beat]
    }
}

/// A fence's line from its far end to its maker's point, along `(−1, 1.1)` normalised, of 230,
/// 40 over the maker (`0x5AA863`…`0x5AAC2A`), cut into segments of 30, the last of 20, each
/// joint swaying across by `4 sin(πL/230)` at `L` from the maker's end.
pub const FENCE_LINE: [f32; 2] = [-0.672_673_6, 0.739_940_9];
pub const FENCE_LENGTH: f32 = 230.0;
pub const FENCE_LIFT: f32 = 40.0;
const SEGMENT: f32 = 30.0;
const SWAY: f32 = 4.0;
/// The fast fence's offset from the slow one's place (`0x890EA0`, `0x8919EC`), and how often each
/// comes.
pub const FAST_OFFSET: [f32; 2] = [800.0, 880.0];
const SLOW_EVERY: f32 = 3.0;
const FAST_EVERY: f32 = 5.5;

/// The looks of a fence's segment (`0x7A2CE0`): crossed quads, flat and upright, 50 to each side
/// of its line, of `FX_LIGHTNING01` grey by its fade, added whole; the texture's `u` 300 a unit
/// along the flat one and 50 along the upright one, each segment 30 on from the last; a glow of
/// `PARTICLE_GLOW06` at each end of the fence.
pub const LASER_TEXTURE: &str = "FX_LIGHTNING01";
pub const LASER_GLOW_TEXTURE: &str = "PARTICLE_GLOW06";
pub const LASER_HALF_WIDTH: f32 = 50.0;
const FLAT_U: f32 = 300.0;
const UPRIGHT_U: f32 = 50.0;
/// A segment shorter than this is the fence's last, which shows a glow at its far end
/// (`0x896B10`).
const LAST_SHORTER: f32 = 29.0;
/// A segment's strike (`0x5ABD80`, the case `0x37F`): 200 physical and 100 magic, from nobody,
/// to every living creature nearer its start than its size, 20, and the creature's, at most once
/// a second for each segment.
pub const LASER_DAMAGES: [f32; 4] = [200.0, 0.0, 100.0, 0.0];
pub const LASER_SIZE: f32 = 20.0;
const LASER_PERIOD: f32 = 1.0;
/// A segment's speeds and holds, slow and fast (`0x7A3570`), and its fade's fall a second once
/// it holds no more.
const SPEEDS: [f32; 2] = [100.0, 450.0];
const HOLDS: [f32; 2] = [12.0, 2.6];
const FADING: f32 = 2.0;
/// Its glow's half side, rolled anew each frame from 17 to 23 (`0x7A2C50`).
const GLOW_HALF: f32 = 17.0;
const GLOW_GROWTH: f32 = 6.0;
/// Its world light, by its fade: 100 for a fence's first segment, 150 for the others.
const FIRST_LIGHT: f32 = 100.0;
const LIGHT: f32 = 150.0;

/// The two timers of a maker of fences (`+0xA8`, `+0xAC`), both 0 as it is made, so that both
/// fences come at its first frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fences {
    slow: f32,
    fast: f32,
}

impl Fences {
    /// The maker `dt` seconds on: whether the slow fence and the fast one come in this frame, each
    /// as its timer falls below 0, which then starts again.
    pub fn update(&mut self, dt: f32) -> [bool; 2] {
        let due = |timer: &mut f32, every: f32| {
            *timer -= dt;
            let due = *timer < 0.0;
            if due {
                *timer = every;
            }
            due
        };
        [
            due(&mut self.slow, SLOW_EVERY),
            due(&mut self.fast, FAST_EVERY),
        ]
    }
}

/// A segment of a fence as it is made: where it starts in the models' frame about its maker's
/// point, its length, its index from the fence's far end, its two joints' indices into the
/// manager's table of random vectors (`+0x20FC`, `+0x2100`) and their sways (`+0x2104`,
/// `+0x2108`), and whether it is of the fast fence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub start: [f32; 3],
    pub length: f32,
    pub index: u8,
    pub joints: [u8; 2],
    pub sways: [f32; 2],
    pub fast: bool,
}

/// A fence's eight segments, slow or `fast` (`0x5AA82C`): from the far end of its line toward its
/// maker's point, each joint shared by the two segments it joins.
pub fn fence(fast: bool, dice: &mut impl Dice) -> Vec<Segment> {
    let offset = if fast { FAST_OFFSET } else { [0.0, 0.0] };
    let [nx, ny] = FENCE_LINE;
    let mut start = [
        offset[0] + FENCE_LENGTH * nx,
        offset[1] + FENCE_LENGTH * ny,
        FENCE_LIFT,
    ];
    let mut roll = || u8::try_from(dice.below(256)).unwrap_or(0);
    let mut joints = [roll(), roll()];
    let mut left = FENCE_LENGTH;
    let mut segments = Vec::new();
    let sway = |at: f32| SWAY * (at * std::f32::consts::PI / FENCE_LENGTH).sin();
    while left > 0.0 {
        segments.push(Segment {
            start,
            length: left.min(SEGMENT).floor(),
            index: u8::try_from(segments.len()).unwrap_or(u8::MAX),
            joints,
            sways: [sway(left), sway(left - SEGMENT)],
            fast,
        });
        start[0] -= SEGMENT * nx;
        start[1] -= SEGMENT * ny;
        joints = [joints[1], roll()];
        left -= SEGMENT;
    }
    segments
}

impl Segment {
    /// Its line along the ground in the models' frame, from its start toward its maker's end.
    pub fn along(&self) -> [f32; 2] {
        [-FENCE_LINE[0], -FENCE_LINE[1]]
    }

    /// The texture's `u` at its two ends on the flat quad and on the upright one.
    pub fn u(&self) -> [[f32; 2]; 2] {
        let from = SEGMENT * f32::from(self.index);
        [FLAT_U, UPRIGHT_U].map(|per| [from / per, (from + self.length) / per])
    }

    /// Whether it shows a glow at its start (the fence's far end) and at its own end (the
    /// maker's end of the fence).
    pub fn glows(&self) -> [bool; 2] {
        [self.index == 0, self.length < LAST_SHORTER]
    }

    /// The sway of its joints in a frame (`0x7A2CE0`): the offsets of its start and of its end
    /// across it and up, and the shift of its texture's `u`, from the entries `(joint + frame) &
    /// 255` of `table`.
    pub fn sway(&self, table: &[[f32; 3]; 256], frame: u32) -> ([[f32; 2]; 2], f32) {
        let entry = |joint: u8| {
            let at = (u32::from(joint) + frame) & 0xFF;
            table[usize::try_from(at).unwrap_or(0)]
        };
        let [near, far] = [entry(self.joints[0]), entry(self.joints[1])];
        let [a, b] = self.sways;
        (
            [[near[1] * a, near[2] * a], [far[1] * b, far[2] * b]],
            near[0],
        )
    }
}

/// The systems' manager's table of 256 random vectors (`0x7CB740`, `0x7C7A70`), each component
/// from −1 to 1 (**estimate**: `rand()` over 32,767 stretched to the range).
pub fn sway_table(dice: &mut impl Dice) -> [[f32; 3]; 256] {
    let mut unit = || {
        let roll = u16::try_from(dice.below(32_768)).unwrap_or(0);
        f32::from(roll) / 32_767.0 * 2.0 - 1.0
    };
    std::array::from_fn(|_| [unit(), unit(), unit()])
}

/// A segment flying (`cParticleSystem_trap03_laser`): its place in the models' frame about its
/// maker's point, its clock, its fade, its speed, its glow's half side and its strike's cooldown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Laser {
    pub segment: Segment,
    pub place: [f32; 3],
    pub clock: f32,
    pub fade: f32,
    pub speed: f32,
    pub glow: f32,
    pub cooldown: f32,
}

impl Laser {
    /// The segment as it starts (`0x7A3570`): at its start, its fade whole, at its speed.
    pub fn new(segment: Segment) -> Self {
        Self {
            segment,
            place: segment.start,
            clock: 0.0,
            fade: 1.0,
            speed: SPEEDS[usize::from(segment.fast)],
            glow: GLOW_HALF,
            cooldown: 0.0,
        }
    }

    /// Its flight's heading in the models' frame (`0x5A45FC`): the slow fence's along `(1, 1.1)`
    /// normalised, away from its maker's line, the fast one's back.
    pub fn heading(&self) -> [f32; 2] {
        let [x, y] = [-FENCE_LINE[0], FENCE_LINE[1]];
        if self.segment.fast { [-x, -y] } else { [x, y] }
    }

    /// The segment `dt` seconds on, `roll` from 0 to 1 for its glow: it flies at its speed
    /// (the flight `0x10`, through walls), its glow rolled; past its hold it stops and fades
    /// by twice the time, and ends below 0 (`0x7A2C50`). Whether it lives.
    pub fn update(&mut self, dt: f32, roll: f32) -> bool {
        let [hx, hy] = self.heading();
        self.place[0] += self.speed * dt * hx;
        self.place[1] += self.speed * dt * hy;
        self.clock += dt;
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.glow = GLOW_HALF + GLOW_GROWTH * roll;
        if self.clock > HOLDS[usize::from(self.segment.fast)] {
            self.speed = 0.0;
            self.fade -= FADING * dt;
        }
        self.fade >= 0.0
    }

    /// Whether it strikes in this frame, `targets` whether any creature is within its reach: at
    /// most once a second, the cooldown set only by a strike.
    pub fn strikes(&mut self, targets: bool) -> bool {
        let strikes = targets && self.cooldown <= 0.0;
        if strikes {
            self.cooldown = LASER_PERIOD;
        }
        strikes
    }

    /// Its colour's part by its fade (`⌊255·fade⌋`), of which its quads take half.
    pub fn shade(&self) -> f32 {
        (255.0 * self.fade.max(0.0)).floor() / 255.0
    }

    /// Its world light's radius by its fade.
    pub fn light(&self) -> f32 {
        let whole = if self.segment.index == 0 {
            FIRST_LIGHT
        } else {
            LIGHT
        };
        (whole * self.fade.max(0.0)).floor()
    }
}

#[cfg(test)]
mod tests;
