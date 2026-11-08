//! The archers' arrows (docs/re/fight.md, "The archers"): a shot's arrow (`cArrow3D`, type 3070)
//! flies straight from its shooter's left hand toward its target's point at 600 a second for
//! 1.5 s (`0x5BB010`), ends on a cell that may not be walked on, but the ground's nibble 2, or at
//! the ground (`0x5BAA60`), and in each frame hits its target, or the first creature, nearer the
//! part of its way it flew than the sum of the two types' sizes (`0x5FFC70`, `0x6226B0`), when
//! that creature may be hit (`0x548F60`); after a hit it sticks in a creature of a size above 20
//! for 5 s, but not in the local player's hero (`0x5BAB40`).
//! `opensacred-game` (`arrows`) makes the arrows, draws them, finds the creatures near them and
//! sends the hits; this module is the rules alone, over points of the ground ([`Spot`]) and
//! heights.

use crate::minds::Spot;

/// How fast an arrow flies, in world units a second (`+0x200`), and how long, in seconds
/// (`+0x234`): at most 900 from the hand.
pub const SPEED: f32 = 600.0;
pub const LIFE: f32 = 1.5;
/// The size of a type whose `items.pak +0x14` is 0, as the arrows' and the heroes' are
/// (`0x428CE0`).
pub const SIZE: f32 = 50.0;
/// The effect of the multi-shot (`0x15`): a shot of a hero with it looses as many arrows as its
/// whole value (`+0x25D`, 1 to 20).
pub const MULTI_SHOT: u32 = 0x15;
/// The most arrows of a shot (`+0x25D`).
const MOST_ARROWS: u32 = 20;
/// How far each arrow of a shot after the first turns from the first, to either side in turn,
/// in radians (`⌈i/2⌉ · 0.05`), and a special arrow's of the Wood Elf (`⌈i/2⌉ · 0.15`).
const FAN: f32 = 0.05;
const SPECIAL_FAN: f32 = 0.15;

/// The type of a plain shot's arrow (`ARROW_LONG.GRN`).
pub const ARROW: u32 = 3070;
/// The type of the `Rundum`'s arrows with a bow (`TYPE_WEAPON_ARROW_MULTI`).
pub const MULTI_ARROW: u32 = 3069;
/// The type of the `Hardhit`'s ray with a bow (`TYPE_WEAPON_ARROW_HARD`, #2275).
pub const HARD_ARROW: u32 = 3068;
/// The type of the `Attacke`'s arrows with a bow (`TYPE_WEAPON_ARROW_ZEAL`, #2275).
pub const ZEAL_ARROW: u32 = 3079;
/// How far the `Attacke`'s arrows with a bow turn from the first, to either side in turn, in
/// radians (`⌈i/2⌉ · 0.01`, `0x4DCB80`).
const ZEAL_FAN: f32 = 0.01;

/// The `Hardhit`'s ray with a bow (`0x4DCB80` at its case `0x3E`, #2275): from the shooter's
/// left hand along the ground toward its target, a step of 40 at a time while the point's cell may
/// be walked on or its ground's nibble is 2, which the arrows fly over (`0x636A60`, which takes
/// `0x636C10` or that nibble), at most 25, each step laying the object 887 (`FX_HARDHIT_BOW`); its
/// arrow stands at the hand for 1 s (`0x5BD2D0`: its speed 0, `+0x234` 1.0) and tests the whole
/// ray for its hits at each frame (`0x5BB010`, `+0x244` its length).
pub const RAY_STEP: f32 = 40.0;
pub const RAY_STEPS: u32 = 25;
pub const RAY_LIFE: f32 = 1.0;

/// The `Hardhit`'s ray's points from `from` toward `toward` on the ground ([`RAY_STEP`]): the
/// first [`RAY_STEPS`] at most, each while `walkable` lets its cell be walked or flown over; the ray's length
/// is their count times the step (`0x4DCB80`). With no way to its target every point is `from`.
pub fn ray(from: Spot, toward: Spot, walkable: impl Fn(Spot) -> bool) -> Vec<Spot> {
    let way = toward - from;
    let length = way.length();
    let step = if length >= 1e-7 {
        way * (RAY_STEP / length)
    } else {
        Spot::new(0.0, 0.0)
    };
    let mut points = Vec::new();
    let mut at = from;
    while points.len() < usize::try_from(RAY_STEPS).unwrap_or(usize::MAX) && walkable(at) {
        points.push(at);
        at = at + step;
    }
    points
}

/// The type of the arrows that a shot of the combat art `art` makes (`0x4DCB80`, the byte
/// `+0x18E` the art less 1000): the Wood Elf's special arrows 1004, 1006, 1007 and 1008 the
/// types 3078, 3074, 3075 and 3077 (`ARROW_EXPLOSION.GRN`, `ARROW_SPIDER.GRN`,
/// `ARROW_LONG.GRN`, `ARROW_BLADE.GRN`), the `Rundum` with a bow (1061) the type 3069
/// (`TYPE_WEAPON_ARROW_MULTI`, #2238), the `Hardhit` and the `Attacke` with a bow (1062, 1063)
/// the types 3068 and 3079 ([`HARD_ARROW`], [`ZEAL_ARROW`], #2275), any other shot [`ARROW`].
pub fn projectile(art: Option<u16>) -> u32 {
    match art {
        Some(1061) => MULTI_ARROW,
        Some(1062) => HARD_ARROW,
        Some(1063) => ZEAL_ARROW,
        Some(1004) => 3078,
        Some(1006) => 3074,
        Some(1007) => 3075,
        Some(1008) => 3077,
        _ => ARROW,
    }
}

/// The arrows of a shot of `art`'s `count` arrows (`+0x261`, 1 for a plain shot) of a hero with
/// the multi-shot's whole value `whole` (`+0x25D`): their product, 1 to 20.
pub fn volley_of(count: u32, whole: Option<i32>) -> u32 {
    (count.max(1) * volley(whole)).clamp(1, MOST_ARROWS)
}

/// The arrows of a shot of a hero with the multi-shot's whole value `whole` (`+0x25D`): that many,
/// 1 to 20, 1 without it.
pub fn volley(whole: Option<i32>) -> u32 {
    whole
        .and_then(|whole| u32::try_from(whole).ok())
        .map_or(1, |whole| whole.clamp(1, MOST_ARROWS))
}

/// How far the `i`-th arrow of a shot turns in the plane from the first's line (`0x4DD90E`):
/// `⌈i/2⌉ · 0.05` radians, to one side for an odd `i` and to the other for an even.
pub fn fanned(i: u32) -> f32 {
    fanned_by(i, FAN)
}

/// How far the `i`-th arrow of a shot of `count` arrows of the combat art `art` turns from the
/// first's line (`0x4DCB80`): by `⌈i/2⌉ · 0.15` radians for the Wood Elf's 1004, 1006 and 1007,
/// by `⌈i/2⌉ · 2π/count` for her 1008, round the circle, by `⌈i/2⌉ · 0.01` for the `Attacke`
/// with a bow (1063), as [`fanned`] for any other.
pub fn fanned_for(i: u32, count: u32, art: Option<u16>) -> f32 {
    match art {
        Some(1004 | 1006 | 1007) => fanned_by(i, SPECIAL_FAN),
        Some(1063) => fanned_by(i, ZEAL_FAN),
        Some(1008) => {
            let count = f32::from(u16::try_from(count.max(1)).unwrap_or(u16::MAX));
            fanned_by(i, std::f32::consts::TAU / count)
        }
        _ => fanned(i),
    }
}

fn fanned_by(i: u32, fan: f32) -> f32 {
    let step = f32::from(u16::try_from(i.div_ceil(2)).unwrap_or(u16::MAX)) * fan;
    if i % 2 == 1 { step } else { -step }
}

/// How fast the Wood Elf's blades 1008 fly (type 3077, `0x5BD2D0`: two thirds of [`SPEED`]),
/// and how fast they turn toward their target, in radians a second (`0x5BB010`: by at most
/// 5 times the frame's time).
pub const HOMING_SPEED: f32 = 400.0;
pub const HOMING_TURN: f32 = 5.0;
/// How often a blade strikes, in seconds (`+0x240`), and how far from the creature it struck it
/// takes its next target (`0x600570`: a squared distance below 100,000).
pub const BLADE_STRIKES_EVERY: f32 = 0.5;
pub const BLADE_NEXT_WITHIN: f32 = 316.227_77;

/// Whether the Wood Elf's club arrow 1007 at the level in play `level` knocks back the creature
/// of the level `target` that it hits, shot by a hero of the level `hero` (`0x5BB010`): below
/// `0.4·hero + 5·level`, and of a type that a kick may move (`arts::knockback_chance` above 0,
/// with no roll; not here).
pub fn club_knocks(target: u32, hero: u32, level: u32) -> bool {
    let bound = f64::from(hero).mul_add(0.4, f64::from(5 * level));
    f64::from(target) < bound
}

/// The effect by which a hero turns arrows aside (`0x19`, `0x561090`).
pub const DEFLECTION: u32 = 0x19;

/// The chance that a hero with the effect [`DEFLECTION`] keyed by `art`, of the level `level`
/// (its entry's first word at `+0x08`), turns an arrow of a shooter of the level `shooter` aside
/// (`0x561090`): `c / (L + 9 + c)`, `c = (5·level + 9)·k`, `k` 2 for the effect of the spell 22
/// and 1 for the others, at most 1; a roll of `rand() / 32767` not above it turns the arrow.
pub fn deflection_chance(level: u16, art: u32, shooter: u32) -> f32 {
    let k = if art == 0x16 { 2.0 } else { 1.0 };
    let c = (f32::from(level) * 5.0 + 9.0) * k;
    let shooter = f32::from(u16::try_from(shooter).unwrap_or(u16::MAX));
    (c / (shooter + 9.0 + c)).min(1.0)
}

/// What becomes of an arrow that the effect [`DEFLECTION`] keyed by `art` turns aside
/// (`0x561090`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deflected {
    /// Sent back at its shooter, the hero its shooter now, its hits of the effect's value: the
    /// Dark Elf's arrowproof 1045 (`0x415`).
    Back,
    /// Sent off in a direction at random in the plane, the hero its shooter now: the spell 28.
    Away,
    /// Destroyed: the spells 22, 24 and 30.
    Gone,
    /// Neither hits nor is changed, flying on: the others.
    Passes,
}

/// What becomes of an arrow that the effect [`DEFLECTION`] keyed by `art` turns aside.
pub fn deflected(art: u32) -> Deflected {
    match art {
        0x415 => Deflected::Back,
        0x1C => Deflected::Away,
        0x16 | 0x18 | 0x1E => Deflected::Gone,
        _ => Deflected::Passes,
    }
}

/// The `Creature.pak` class of the ghosts, which the arrows pass through (`0x426BC0`).
pub const GHOST: u8 = 11;
/// How long an arrow that passes through what it hits waits before it may hit again, in seconds
/// (`+0x240`, `0x5BB010`).
pub const PASSES_EVERY: f32 = 0.5;

/// Whether an arrow of the type `projectile` passes through the creature it hits rather than
/// ending there (`0x5BB010`): the plain arrow 3070 and the bow arts' 3069 and 3079 pass through a
/// ghost ([`GHOST`], `ghost`) and through anything where their shooter's flag of set 63 holds
/// (`pierces`, the stats' `+0x148` bit `0x10000000`, `0x5BBD87`), hitting again at most every
/// [`PASSES_EVERY`]; the others end at their hit.
pub fn passes(projectile: u32, ghost: bool, pierces: bool) -> bool {
    matches!(projectile, ARROW | MULTI_ARROW | ZEAL_ARROW) && (ghost || pierces)
}

/// How long an arrow stays stuck in a creature after its hit, in seconds, and the size above
/// which a creature takes it (`0x5BAB40`).
pub const STICKS_FOR: f32 = 5.0;
const STICKS_ABOVE: f32 = 20.0;

/// A type's size for the arrows' hits: its `items.pak +0x14`, 50 for 0 (`0x428CE0`).
pub fn size(word: u32) -> f32 {
    match u16::try_from(word) {
        Ok(0) | Err(_) => SIZE,
        Ok(size) => f32::from(size),
    }
}

/// A point of the world: its point of the ground and its height, in world units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub ground: Spot,
    pub height: f32,
}

impl Point {
    pub const fn new(ground: Spot, height: f32) -> Self {
        Self { ground, height }
    }

    /// The point `seconds` on along `velocity`, a velocity a second.
    fn on(self, velocity: Self, seconds: f32) -> Self {
        Self::new(
            self.ground + velocity.ground * seconds,
            self.height + velocity.height * seconds,
        )
    }
}

/// An arrow in flight (`cArrow3D`, `0x5BB010`): where it is, its velocity a second, fixed at its
/// start in three dimensions without gravity, how long it still flies, and a ray's length, along
/// which a ray tests its hits standing ([`Arrow::ray`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arrow {
    at: Point,
    velocity: Point,
    left: f32,
    reach: Option<f32>,
}

impl Arrow {
    /// An arrow from `from` toward `toward`, straight at 600 a second for 1.5 s (`0x4EB700`); it
    /// leads no moving target. One aimed at its own point stands where it is.
    pub fn new(from: Point, toward: Point) -> Self {
        Self {
            at: from,
            velocity: along(
                toward.ground - from.ground,
                toward.height - from.height,
                SPEED,
            ),
            left: LIFE,
            reach: None,
        }
    }

    /// The `Hardhit`'s ray with a bow from `from` toward `toward`, of the length `length` (the
    /// type 3068, `0x5BD2D0`): it stands where it is for [`RAY_LIFE`], pointing along its way,
    /// and tests its hits along the whole of it at each frame (`+0x244`, `0x5BB010`).
    pub fn ray(from: Point, toward: Point, length: f32) -> Self {
        Self {
            at: from,
            velocity: along(
                toward.ground - from.ground,
                toward.height - from.height,
                1.0,
            ),
            left: RAY_LIFE,
            reach: Some(length),
        }
    }

    /// A blade of the Wood Elf's 1008 from `from` toward `toward`, at [`HOMING_SPEED`] for
    /// `seconds` (`0x5BD2D0`), which [`Arrow::home`] turns toward its target.
    pub fn homing(from: Point, toward: Point, seconds: f32) -> Self {
        Self {
            at: from,
            velocity: along(
                toward.ground - from.ground,
                toward.height - from.height,
                HOMING_SPEED,
            ),
            left: seconds,
            reach: None,
        }
    }

    /// A homing arrow's turn in a frame of `dt` seconds before its flight (`0x5BB010`, `+0x1E4`
    /// bit `0x10`): its direction turned toward `toward` by at most [`HOMING_TURN`] times `dt`
    /// radians, its speed kept; then, where its next point's cell may not be walked on
    /// (`walkable`), its velocity turned back, and else, where that point would lie at or below
    /// the ground (`ground`), its rise.
    pub fn home(
        &mut self,
        dt: f32,
        toward: Point,
        walkable: impl FnOnce(Point) -> bool,
        ground: impl FnOnce(Point) -> Option<f32>,
    ) {
        let speed = self.speed();
        if speed < 1e-7 {
            return;
        }
        let [vx, vy, vz] = [
            self.velocity.ground.x / speed,
            self.velocity.ground.y / speed,
            self.velocity.height / speed,
        ];
        let to = along(
            toward.ground - self.at.ground,
            toward.height - self.at.height,
            1.0,
        );
        let [tx, ty, tz] = [to.ground.x, to.ground.y, to.height];
        let dot = (vx * tx + vy * ty + vz * tz).clamp(-1.0, 1.0);
        let angle = dot.acos();
        if angle > 0.001 {
            // The part of the way toward it square to its flight, the plane it turns in.
            let [ux, uy, uz] = [tx - dot * vx, ty - dot * vy, tz - dot * vz];
            let length = (ux * ux + uy * uy + uz * uz).sqrt();
            if length >= 1e-7 {
                let (sin, cos) = angle.min(HOMING_TURN * dt).sin_cos();
                let turned = |v: f32, u: f32| (v * cos + u / length * sin) * speed;
                self.velocity =
                    Point::new(Spot::new(turned(vx, ux), turned(vy, uy)), turned(vz, uz));
            }
        }
        let next = self.at.on(self.velocity, dt);
        if !walkable(next) {
            self.velocity = Point::new(self.velocity.ground * -1.0, -self.velocity.height);
        } else if ground(next).is_some_and(|ground| next.height <= ground) {
            self.velocity.height = -self.velocity.height;
        }
    }

    fn speed(&self) -> f32 {
        (self.velocity.ground.length().powi(2) + self.velocity.height.powi(2)).sqrt()
    }

    /// Where it is.
    pub fn at(&self) -> Point {
        self.at
    }

    /// Its velocity a second, the way it points.
    pub fn velocity(&self) -> Point {
        self.velocity
    }

    /// The arrow turned in the plane by `angle` radians, its speed kept.
    pub fn turned_by(mut self, angle: f32) -> Self {
        let (sin, cos) = angle.sin_cos();
        let g = self.velocity.ground;
        self.velocity = Point::new(
            Spot::new(g.x * cos - g.y * sin, g.x * sin + g.y * cos),
            self.velocity.height,
        );
        self
    }

    /// The arrow turned where it is along `direction` at its speed, its time going on
    /// (`0x561090`).
    pub fn turn(&mut self, direction: Point) {
        self.velocity = along(direction.ground, direction.height, SPEED);
    }

    /// Its flight in a frame of `dt` seconds (`0x5BB010`): none once its time is out, when it is
    /// destroyed; else its time runs down and its next point lies `dt` seconds on, where it goes
    /// when `open` lets it fly there ([`open`], `0x5BAA60`), and else its time ends where it is.
    /// The part of its way it flew, from its last point to its next, on which its hits are
    /// tested ([`picks`]) whether or not it got there.
    /// A ray stands, its way the whole ray.
    pub fn fly(&mut self, dt: f32, open: impl FnOnce(Point) -> bool) -> Option<(Spot, Spot)> {
        if self.left <= 0.0 {
            return None;
        }
        self.left -= dt;
        if let Some(reach) = self.reach {
            return Some((
                self.at.ground,
                self.at.ground + self.velocity.ground * reach,
            ));
        }
        let next = self.at.on(self.velocity, dt);
        let from = self.at.ground;
        if open(next) {
            self.at = next;
        } else {
            self.left = 0.0;
        }
        Some((from, next.ground))
    }
}

/// The velocity of `speed` along the ground's `ground` and the rise `up`, none for no way.
fn along(ground: Spot, up: f32, speed: f32) -> Point {
    let length = (ground.length().powi(2) + up * up).sqrt();
    if length >= 1e-7 {
        Point::new(ground * (speed / length), up * (speed / length))
    } else {
        Point::default()
    }
}

/// Whether an arrow may fly to a point (`0x5BAA60`): its cell may be walked on (`walkable`,
/// `0x636C10`), or is of the ground's nibble 2 (`flown_over`, the ponds, the fences and the
/// walls), which it flies over; and it lies above the ground's height there (`ground`,
/// `0x636520`), which is none outside the world. The statics do not stop it.
pub fn open(walkable: bool, flown_over: bool, height: f32, ground: Option<f32>) -> bool {
    (walkable || flown_over) && ground.is_some_and(|ground| height > ground)
}

/// The distance in the plane of `point` from the part of a way from `from` to `to` (`0x6226B0`).
fn from_way(from: Spot, to: Spot, point: Spot) -> f32 {
    let way = to - from;
    let length = way.x * way.x + way.y * way.y;
    let along = if length > 0.0 {
        let to_point = point - from;
        ((to_point.x * way.x + to_point.y * way.y) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (point - (from + way * along)).length()
}

/// The creature that an arrow of size `size`, flying from `from` to `to` in a frame, comes on
/// (`0x5FFC70`, `0x6226B0`): of the creatures `near`, each with its point of the ground and its
/// size, those whose point lies nearer that part of its way, in the plane, than the sum of the two
/// sizes; of them its `target` when it is one, else the first; the height is not tested. The
/// arrow hits it only when it may be hit, alive and hostile to the shooter (`0x548F60`); any
/// other lets it fly on.
pub fn picks<T: Copy + PartialEq>(
    (from, to): (Spot, Spot),
    size: f32,
    near: impl IntoIterator<Item = (T, Spot, f32)>,
    target: T,
) -> Option<T> {
    let mut first = None;
    for (creature, at, its) in near {
        if from_way(from, to, at) >= size + its {
            continue;
        }
        if creature == target {
            return Some(creature);
        }
        first = first.or(Some(creature));
    }
    first
}

/// Whether a stopped arrow sticks into the creature it hit (`0x5BAB40`): one of a size above 20,
/// but not the local player's hero (`hero`).
pub fn sticks(size: f32, hero: bool) -> bool {
    !hero && size > STICKS_ABOVE
}

/// The element that an arrow shows of its shooter's weapon (`+0x248`, `0x5BD2D0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    Fire,
    Poison,
    Magic,
}

/// The share of its element above which an arrow shows it (`cItemBase`'s `+0x1DC`, `0x5BE130`).
pub const ELEMENT_SHOWS_ABOVE: f32 = 0.1;

/// The element that an arrow of a weapon of each kind's most damage `most` (physical, fire, magic,
/// poison; its record's `+0x24` to `+0x2A`, whole) shows, and its share (`0x5BD2D0`): the largest
/// of the three larger than both others, its most over twice the physical, held to 0 … 1; none
/// where it does not pass [`ELEMENT_SHOWS_ABOVE`], or where no kind is larger than both others.
pub fn element(most: [f32; 4]) -> Option<(Element, f32)> {
    let [physical, fire, magic, poison] = most.map(f32::trunc);
    let (kind, value) = if fire > poison && fire > magic {
        (Element::Fire, fire)
    } else if poison > fire && poison > magic {
        (Element::Poison, poison)
    } else if magic > poison && magic > fire {
        (Element::Magic, magic)
    } else {
        return None;
    };
    let share = (value / (physical * 2.0)).clamp(0.0, 1.0);
    (share > ELEMENT_SHOWS_ABOVE).then_some((kind, share))
}

/// The element that a weapon of each kind's most damage `most` (physical, fire, magic, poison; its
/// record's `+0x24` to `+0x2A`, whole) shows in hand, and its share (`cWeapon3D::toggleVisuals`,
/// `0x5CDB60`): the largest of the three, larger than both others and than a third of the
/// physical; the fire's and the poison's share `(v − min/2) / (max/3 − min/2)`, the magic's
/// `(v − min/3) / (max/4 − min/3)`, `min` and `max` the balance's `SpawnWeaponMin` and
/// `SpawnWeaponMax`, held to 0 … 1; none where it does not pass [`ELEMENT_SHOWS_ABOVE`].
pub fn weapon_element(most: [f32; 4], [min, max]: [f32; 2]) -> Option<(Element, f32)> {
    const THIRD: f32 = 0.333_333_34;
    let [physical, fire, magic, poison] = most.map(f32::trunc);
    let third = physical * THIRD;
    let (kind, share) = if fire > poison && fire > magic && fire > third {
        let low = min * 0.5;
        (Element::Fire, (fire - low) / (max * THIRD - low))
    } else if poison > fire && poison > magic && poison > third {
        let low = min * 0.5;
        (Element::Poison, (poison - low) / (max * THIRD - low))
    } else if magic > poison && magic > fire && magic > third {
        let low = min * THIRD;
        (Element::Magic, (magic - low) / (max * 0.25 - low))
    } else {
        return None;
    };
    let share = share.clamp(0.0, 1.0);
    (share > ELEMENT_SHOWS_ABOVE).then_some((kind, share))
}

/// The line of glows that an item of an element draws along its bones `weapon_fx01` to
/// `weapon_fx03` (`0x5BEE90`, through `0x40DA80`; `crate::particles::glow_segments`), its half
/// side by the share `share`.
pub fn element_line(element: Element, share: f32) -> crate::particles::GlowLine {
    let (density, half, argb) = match element {
        Element::Fire => (0.5, share * 4.0 + 2.0, 0x40FF_8844),
        Element::Poison => (0.5, share * 4.0 + 2.0, 0x4044_FF44),
        Element::Magic => (0.7, share * 10.0, 0x5070_33FF),
    };
    crate::particles::GlowLine {
        density,
        half,
        argb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, y: f32) -> Spot {
        Spot::new(x, y)
    }

    #[test]
    fn a_size_of_0_is_50() {
        assert_eq!(size(0), 50.0);
        assert_eq!(size(45), 45.0);
        assert_eq!(size(25), 25.0);
    }

    #[test]
    fn an_arrow_flies_straight_at_600_a_second_for_1_5_s() {
        // From a hand 90 over the ground toward a neck 130 over it, 600 away along x.
        let from = Point::new(at(0.0, 0.0), 90.0);
        let toward = Point::new(at(600.0, 0.0), 130.0);
        let mut arrow = Arrow::new(from, toward);
        let speed = arrow.velocity();
        let length = (speed.ground.length().powi(2) + speed.height.powi(2)).sqrt();
        assert!((length - SPEED).abs() < 1e-3, "{length}");
        // A second takes it on by 600 along its way, rising as it goes.
        let mut flown = 0.0;
        let mut frames = 0;
        while let Some((last, next)) = arrow.fly(0.125, |_| true) {
            flown += (next - last).length();
            frames += 1;
            if frames == 8 {
                let p = arrow.at();
                assert!(
                    (p.ground.x - 598.7).abs() < 0.5 && p.height > 125.0,
                    "{p:?}"
                );
            }
        }
        // It flies 1.5 s, 12 frames of 0.125 s, about 900 along the ground, and then is gone.
        assert_eq!(frames, 12);
        assert!((flown - 898.0).abs() < 2.0, "{flown}");
        assert_eq!(arrow.fly(0.1, |_| true), None);
    }

    #[test]
    fn an_arrow_ends_where_it_may_not_fly_on() {
        let mut arrow = Arrow::new(
            Point::new(at(0.0, 0.0), 90.0),
            Point::new(at(600.0, 0.0), 0.0),
        );
        // The frame it meets a wall it stays where it was, its way tested for hits still, and
        // it is gone the next.
        let way = arrow.fly(0.1, |_| false);
        assert_eq!(arrow.at().ground, at(0.0, 0.0));
        assert!(way.is_some_and(|(last, next)| last == at(0.0, 0.0) && next.x > 50.0));
        assert_eq!(arrow.fly(0.1, |_| true), None);
        // A wall's cell, a pond's, a fence's of nibble 2 it flies over; under the ground or off
        // the world it ends.
        assert!(open(true, false, 50.0, Some(10.0)));
        assert!(open(false, true, 50.0, Some(10.0)));
        assert!(!open(false, false, 50.0, Some(10.0)));
        assert!(!open(true, false, 10.0, Some(10.0)));
        assert!(!open(true, false, 50.0, None));
    }

    #[test]
    fn an_arrow_hits_its_target_or_the_first_creature_near_its_way() {
        let way = (at(0.0, 0.0), at(100.0, 0.0));
        // The hero 90 off the way: within the arrow's 50 and its own 50.
        let hero = ("hero", at(50.0, 90.0), 50.0);
        // A wolf of 45 that 96 off misses; a goblin of 25, 41 from the way's end, is near.
        let wolf = ("wolf", at(50.0, -96.0), 45.0);
        let goblin = ("goblin", at(140.0, 10.0), 25.0);
        assert_eq!(picks(way, 50.0, [wolf, hero, goblin], "hero"), Some("hero"));
        // Without its target near, the first that is, the farther ones not.
        assert_eq!(picks(way, 50.0, [wolf, goblin], "hero"), Some("goblin"));
        assert_eq!(picks(way, 50.0, [wolf], "hero"), None);
        let far = ("hero", at(50.0, 101.0), 50.0);
        assert_eq!(picks(way, 50.0, [far], "hero"), None);
        // A way of no length measures from its point.
        let still = (at(0.0, 0.0), at(0.0, 0.0));
        assert_eq!(
            picks(still, 50.0, [("hero", at(0.0, 99.0), 50.0)], "hero"),
            Some("hero")
        );
    }

    #[test]
    fn an_arrow_sticks_in_a_creature_but_not_in_the_hero() {
        assert!(sticks(45.0, false));
        assert!(!sticks(20.0, false));
        assert!(!sticks(50.0, true));
    }

    #[test]
    fn a_multi_shot_fans_its_arrows_to_either_side() {
        assert_eq!(volley(None), 1);
        assert_eq!(volley(Some(2)), 2);
        assert_eq!(volley(Some(0)), 1);
        assert_eq!(volley(Some(30)), 20);
        assert_eq!(fanned(1), 0.05);
        assert_eq!(fanned(2), -0.05);
        assert!((fanned(3) - 0.1).abs() < 1e-6 && (fanned(4) + 0.1).abs() < 1e-6);
        assert!((fanned_for(1, 1, Some(1004)) - 0.15).abs() < 1e-6);
        assert!((fanned_for(4, 5, Some(1007)) + 0.3).abs() < 1e-6);
        assert_eq!(fanned_for(1, 2, None), 0.05);
        // The blades round the circle: four a quarter of it apart, the last opposite the first.
        assert!((fanned_for(1, 4, Some(1008)) - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!((fanned_for(3, 4, Some(1008)) - std::f32::consts::PI).abs() < 1e-6);
        let arrow = Arrow::new(Point::default(), Point::new(Spot::new(600.0, 0.0), 0.0));
        let turned = arrow.turned_by(std::f32::consts::FRAC_PI_2);
        assert!(turned.velocity().ground.x.abs() < 1e-3);
        assert!((turned.velocity().ground.y - SPEED).abs() < 1e-3);
    }

    #[test]
    fn a_deflection_turns_an_arrow_by_its_level_against_the_shooters() {
        // The arrowproof of level 1 against a shooter of level 1: 14 / (1 + 9 + 14).
        assert!((deflection_chance(1, 0x415, 1) - 14.0 / 24.0).abs() < 1e-6);
        // The spell 22's doubles its `c`; at most 1.
        assert!((deflection_chance(1, 0x16, 1) - 28.0 / 38.0).abs() < 1e-6);
        assert!(deflection_chance(1, 0x415, 50) < 0.25);
        assert_eq!(deflected(0x415), Deflected::Back);
        assert_eq!(deflected(0x1C), Deflected::Away);
        assert_eq!(deflected(0x16), Deflected::Gone);
        assert_eq!(deflected(0x18), Deflected::Gone);
        assert_eq!(deflected(0x1E), Deflected::Gone);
        assert_eq!(deflected(7), Deflected::Passes);
        // Sent back along a new direction at its speed, where it is.
        let mut arrow = Arrow::new(Point::default(), Point::new(Spot::new(600.0, 0.0), 0.0));
        arrow.turn(Point::new(Spot::new(0.0, -3.0), 0.0));
        assert_eq!(arrow.at(), Point::default());
        assert!((arrow.velocity().ground.y + SPEED).abs() < 1e-3);
    }

    #[test]
    fn the_special_arrows_fly_their_own_projectiles() {
        assert_eq!(projectile(None), ARROW);
        assert_eq!(projectile(Some(1004)), 3078);
        assert_eq!(projectile(Some(1006)), 3074);
        assert_eq!(projectile(Some(1007)), 3075);
        assert_eq!(projectile(Some(1008)), 3077);
        // The `Rundum` with a bow's own (#2238); another art's shot a plain arrow.
        assert_eq!(projectile(Some(1061)), MULTI_ARROW);
        // The `Hardhit`'s and the `Attacke`'s with a bow (#2275).
        assert_eq!(projectile(Some(1062)), HARD_ARROW);
        assert_eq!(projectile(Some(1063)), ZEAL_ARROW);
        assert!((fanned_for(3, 5, Some(1063)) - 0.02).abs() < 1e-6);
        assert_eq!(projectile(Some(1064)), ARROW);
        // Its count times the multi-shot's, at most 20.
        assert_eq!(volley_of(3, None), 3);
        assert_eq!(volley_of(5, Some(3)), 15);
        assert_eq!(volley_of(5, Some(6)), 20);
        assert_eq!(volley_of(1, Some(2)), 2);
        assert_eq!(
            [1, 3, 4, 9].map(crate::arts::bow_rundum_arrows),
            [2, 4, 5, 5]
        );
        assert!(crate::arts::bow_rundum(1069) && !crate::arts::bow_rundum(1062));
        assert!(crate::arts::bow_hardhit(1070) && !crate::arts::bow_hardhit(1071));
        assert!(crate::arts::bow_attacke(1080) && !crate::arts::bow_attacke(1079));
    }

    #[test]
    fn the_plain_and_the_bow_arts_arrows_pass_through_the_ghosts() {
        assert!(passes(ARROW, true, false) && passes(MULTI_ARROW, false, true));
        assert!(passes(ZEAL_ARROW, true, true));
        // Not a living creature for a shooter without the flag, nor the other arrows.
        assert!(!passes(ARROW, false, false));
        assert!(!passes(HARD_ARROW, true, true) && !passes(3077, true, false));
    }

    #[test]
    fn the_hardhits_ray_stands_and_tests_its_whole_length() {
        // Its points 40 apart while their cells may be walked on, at most 25.
        let from = Spot::new(0.0, 0.0);
        let points = ray(from, Spot::new(300.0, 0.0), |at| at.x < 200.0);
        assert_eq!(points.len(), 5);
        assert!((points[4].x - 160.0).abs() < 1e-3);
        assert_eq!(ray(from, Spot::new(0.0, 9.0), |_| true).len(), 25);
        // It stands a second, its way the ray's whole length along its line.
        let mut arrow = Arrow::ray(
            Point::new(from, 80.0),
            Point::new(Spot::new(0.0, 100.0), 80.0),
            200.0,
        );
        let (a, b) = arrow.fly(0.5, |_| true).expect("a way");
        assert_eq!((a, arrow.at().ground), (from, from));
        assert!((b.y - 200.0).abs() < 1e-3 && b.x.abs() < 1e-3);
        assert!(arrow.fly(0.5, |_| true).is_some());
        assert!(arrow.fly(0.1, |_| true).is_none());
    }

    #[test]
    fn a_blade_turns_toward_its_target_at_its_rate() {
        let open = |_: Point| true;
        let low = |_: Point| Some(-100.0);
        // Flying along +x at 400, its target square to its way along +y.
        let mut blade = Arrow::homing(
            Point::default(),
            Point::new(Spot::new(100.0, 0.0), 0.0),
            6.0,
        );
        assert!((blade.velocity().ground.x - HOMING_SPEED).abs() < 1e-3);
        let target = Point::new(Spot::new(0.0, 1000.0), 0.0);
        blade.home(0.1, target, open, low);
        // A tenth of a second turns it by half a radian, its speed kept.
        let v = blade.velocity();
        assert!((v.ground.y.atan2(v.ground.x) - 0.5).abs() < 1e-4);
        assert!((blade.speed() - HOMING_SPEED).abs() < 1e-2);
        // Near its line it turns onto it.
        let mut near = Arrow::homing(
            Point::default(),
            Point::new(Spot::new(100.0, 0.0), 0.0),
            6.0,
        );
        near.home(0.1, Point::new(Spot::new(1000.0, 100.0), 0.0), open, low);
        let v = near.velocity();
        assert!((v.ground.y.atan2(v.ground.x) - 0.1_f32.atan()).abs() < 1e-4);
        // A cell that may not be walked on sends it back; the ground turns its rise.
        let mut back = Arrow::homing(
            Point::default(),
            Point::new(Spot::new(100.0, 0.0), 0.0),
            6.0,
        );
        back.home(0.1, Point::new(Spot::new(1000.0, 0.0), 0.0), |_| false, low);
        assert!((back.velocity().ground.x + HOMING_SPEED).abs() < 1e-3);
        let mut up = Arrow::homing(
            Point::new(Spot::default(), 10.0),
            Point::new(Spot::new(100.0, 0.0), 0.0),
            6.0,
        );
        let falling = up.velocity().height;
        up.home(0.1, Point::new(Spot::new(1000.0, 0.0), -90.0), open, |_| {
            Some(8.0)
        });
        assert!(falling < 0.0 && up.velocity().height > 0.0);
    }

    #[test]
    fn the_club_arrow_knocks_back_a_creature_below_its_bound() {
        // A hero of level 1 at the level 1: below 5.4.
        assert!(club_knocks(5, 1, 1));
        assert!(!club_knocks(6, 1, 1));
        // Of level 20 at the level 3: below 23.
        assert!(club_knocks(22, 20, 3));
        assert!(!club_knocks(23, 20, 3));
    }

    #[test]
    fn an_arrow_shows_the_largest_element_of_its_weapon_above_a_fifth_of_its_physical() {
        assert_eq!(element([20.0, 10.0, 2.0, 0.0]), Some((Element::Fire, 0.25)));
        assert_eq!(
            element([20.0, 0.0, 2.0, 30.0]),
            Some((Element::Poison, 0.75))
        );
        assert_eq!(element([5.0, 0.0, 40.0, 0.0]), Some((Element::Magic, 1.0)));
        // A fifth of the physical or less, a tie, or none at all.
        assert_eq!(element([20.0, 4.0, 0.0, 0.0]), None);
        assert_eq!(element([20.0, 10.0, 10.0, 0.0]), None);
        assert_eq!(element([20.0, 0.0, 0.0, 0.0]), None);
        assert_eq!(element_line(Element::Fire, 0.5).density, 0.5);
        // In hand: above a third of the physical, by the balance's 25 and 275.
        let spawn = [25.0, 275.0];
        let (kind, share) = weapon_element([60.0, 40.0, 0.0, 0.0], spawn).unwrap();
        assert_eq!(kind, Element::Fire);
        assert!((share - 27.5 / (275.0 * 0.333_333_34 - 12.5)).abs() < 1e-6);
        assert_eq!(weapon_element([150.0, 40.0, 0.0, 0.0], spawn), None);
        assert_eq!(weapon_element([60.0, 14.0, 0.0, 0.0], spawn), None);
        assert_eq!(element_line(Element::Magic, 0.5).half, 5.0);
    }
}
