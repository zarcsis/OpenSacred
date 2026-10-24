//! The fight's rules as far as the game follows them (docs/re/fight.md): the creatures' levels,
//! their health, their attack, defence, damage and resistances, a strike's hit, the items'
//! values, the regeneration, and the hero's experience and levels. `opensacred-game` (`fight`)
//! keeps the components, the resources and the systems that call them; this module is the rules
//! alone.

use std::collections::HashMap;

use opensacred_data::tables::{Balance, Creature, Weapon, WeaponModifier};

use crate::arts;
use crate::spawning::{Dice, Rng, SpawnValues};

/// The hero's level, which the creatures' levels follow: the hero gains none yet.
pub const HERO_LEVEL: u32 = 1;
/// The balance's row of a difficulty's values that the game plays: bronze, the row's sixth
/// value (docs/re/fight.md).
const BRONZE: usize = 5;

/// The kinds of damage: physical, fire, magic and poison.
const KINDS: usize = 4;

/// The rows of the table of `A` and `D` at `0x956C80`, all alike: the per cent of the attributes
/// `a0` and `a2` that the attack takes, and of an item's `A`, and the same of the defence
/// (docs/re/fight.md, "An NPC's attack, defence and damage").
const ATTACK: [f32; 3] = [50.0, 50.0, 100.0];
const DEFENCE: [f32; 3] = [20.0, 80.0, 100.0];

/// The hero's experience, which its kills earn (`0x57D860`), its level by it (`0x57F8D0`), and
/// the attribute and skill points its levels gave (`0x57E160`), which it cannot spend yet, and
/// the skills it may still learn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Experience {
    pub points: u64,
    pub level: u32,
    pub attribute_points: u32,
    pub skill_points: u32,
    to_learn: u32,
}

impl Default for Experience {
    fn default() -> Self {
        Self {
            points: 0,
            level: HERO_LEVEL,
            attribute_points: 0,
            skill_points: 0,
            to_learn: 0,
        }
    }
}

/// The skill slots a hero holds at the start (**hypothesis**, docs/re/fight.md).
const HELD_SKILLS: u32 = 2;
/// The highest level (`0x57F8D0`).
const TOP_LEVEL: u32 = 216;

impl Experience {
    /// Adds `earned` (`0x57E160`), the experience held below the top level's end, and gives the
    /// levels it reaches: for each, an attribute point, and for each level `p` reached, half,
    /// rounded up, of the skill slots held and open (two held, one more open from each of the
    /// balance's `skilllearn` levels up to `p`); returns whether the level grew.
    pub fn add(&mut self, earned: u64, rules: &Rules) -> bool {
        self.points = (self.points + earned).min(level_end(TOP_LEVEL) - 1);
        let reached = level_of(self.points, self.level);
        if reached <= self.level {
            return false;
        }
        let from = self.level;
        for p in from + 1..=reached {
            let opened = rules
                .skill_levels
                .iter()
                .filter(|&&t| from < t && t <= p)
                .count();
            let n = HELD_SKILLS + self.to_learn + u32::try_from(opened).unwrap_or(0);
            self.skill_points += n.div_ceil(2);
        }
        self.attribute_points += reached - from;
        let opened = rules
            .skill_levels
            .iter()
            .filter(|&&t| from < t && t <= reached)
            .count();
        self.to_learn += u32::try_from(opened).unwrap_or(0);
        self.level = reached;
        true
    }
}

/// The experience at which level `level` ends (`0x57F6E0`):
/// `⌊(L⁴ + 40L³ + 150L² + 200L − 90)/100⌋·100`.
pub fn level_end(level: u32) -> u64 {
    let l = u64::from(level);
    (l.pow(4) + 40 * l.pow(3) + 150 * l.pow(2) + 200 * l).saturating_sub(90) / 100 * 100
}

/// The level of `points` of experience (`0x57F8D0`): the one whose span holds them, at most 216,
/// the level `current` while they are below the first level's end.
pub fn level_of(points: u64, current: u32) -> u32 {
    if points < level_end(1) {
        return current;
    }
    (1..TOP_LEVEL)
        .take_while(|&level| level_end(level) <= points)
        .last()
        .map_or(current, |level| level + 1)
}

/// The experience that the level's setter gives a creature at its level `level` (`0x564D60`):
/// the end of the level before, 0 below level 2.
pub fn level_start(level: u32) -> u64 {
    if level < 2 { 0 } else { level_end(level - 1) }
}

/// Adds a kill's `earned` experience to the `points` of a creature that is no hero, at its level
/// `level` (`addExperience`, `0x57E160`, #690), held below the top level's end; the level of its
/// new points where that is above `level`, which the level's setter then gives it, held
/// ([`Rank::held`]). The attribute and skill points that the original counts at each such level
/// an NPC never spends.
pub fn creature_earns(points: &mut u64, level: u32, earned: u64) -> Option<u32> {
    *points = (*points + earned).min(level_end(TOP_LEVEL) - 1);
    let reached = level_of(*points, level);
    (reached > level).then_some(reached)
}

/// A creature's rank (docs/re/fight.md, "Bosses and champions"): a boss (`+0x1F4` bit
/// `0x80000`), a champion (`+0x2B8` bit `0x40`, with a champion's levelling, `+0xB0` bit
/// `0x400`), a champion's levelling alone, or neither.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Rank {
    #[default]
    Common,
    Boss,
    Champion,
    /// A champion's levelling without the champion's mark (#621): a fill's 343 or 212, made at
    /// the marker level 1 and levelled at its first fight as a champion is, with the champions'
    /// drops' bonus and none of the factors of the levels (#615); the factors of its health, its
    /// attack and defence and the drop's chance, which the mark gates, are a common creature's.
    Levelled,
}

impl Rank {
    /// The rank of a fill's creature of type `creature` (`FillSector`, `0x47E6B7`): the types
    /// 203, 342 and 349 champions; 343 and 212 a champion's levelling alone (`0x47E6F3`, after a
    /// roll of 5 for 343 that changes nothing); 37, 48, 55 and 84 a boss or a champion by a coin,
    /// 127 and 49 so in one case of two; else, `hostile` to the hero, a champion when a roll of 0
    /// to 1,000 is below `HarteSau`.
    pub fn of_fill(creature: u32, hostile: bool, rng: &mut Rng, rules: &Rules) -> Self {
        let coin = |rng: &mut Rng| {
            if rng.below(2) == 1 {
                Self::Boss
            } else {
                Self::Champion
            }
        };
        match creature {
            203 | 342 | 349 => Self::Champion,
            343 | 212 => Self::Levelled,
            37 | 48 | 55 | 84 => coin(rng),
            127 | 49 if rng.below(2) == 0 => coin(rng),
            _ if hostile && to_f32(rng.below(1001)) < rules.ranks.champions => Self::Champion,
            _ => Self::Common,
        }
    }

    /// The rank of a `CreateNPC`'s creature of the level tag `0x1F`, `tag` (`0x4843C9`): a boss
    /// for [`BOSS_TAG`], else common (the bosses of the tag `0x72`, which only the dynamic quests'
    /// records carry, are not followed).
    pub fn of_tag(tag: u32) -> Self {
        if tag == BOSS_TAG {
            Self::Boss
        } else {
            Self::Common
        }
    }

    /// Its level, made at the hero's level `hero` (`0x550E50`): the hero's plus `LevelUpBoss` or
    /// `LevelUpChamp` (a champion's levelling alone too, which the strike levels as it does a
    /// champion, `0x56B682`), at least `OffLevel` plus the sector's lowest, and unless its type is
    /// big at most `LevelKap` plus the sector's highest plus the same; a common creature's
    /// `level`.
    pub fn level(
        self,
        level: u32,
        hero: u32,
        values: &SpawnValues,
        big: bool,
        rules: &Rules,
    ) -> u32 {
        let up = match self {
            Self::Common => return level,
            Self::Boss => rules.ranks.level_up[0],
            Self::Champion | Self::Levelled => rules.ranks.level_up[1],
        };
        let mut target = hero + up;
        let top = rules.ranks.cap + values.highest + up;
        if !big && target > top {
            target = top;
        }
        target.max(rules.ranks.off + values.lowest).max(1)
    }

    /// The level that the level's setter gives a creature of this rank and of type `creature`,
    /// which is no horse, asked for `level` without the difficulty's raise (`0x564D60` with `a` 1
    /// and `b` 0, as `addExperience` calls it, #690), in a sector of `values`: a common creature's
    /// held as [`held_level`] holds it; a boss's or a champion's, marked, at `LevelUpBoss` or
    /// `LevelUpChamp` more, and not at all for a big type.
    pub fn held(
        self,
        level: u32,
        values: Option<&SpawnValues>,
        creature: u32,
        big: bool,
        rules: &Rules,
    ) -> u32 {
        let up = match self {
            Self::Common | Self::Levelled => {
                return held_level(level, values, creature, false, rules);
            }
            Self::Boss => rules.ranks.level_up[0],
            Self::Champion => rules.ranks.level_up[1],
        };
        if big {
            return level.max(1);
        }
        level.min(most_level(values, creature, rules) + up).max(1)
    }

    /// The factor of its most health (`0x5658F0`): `HPFakBoss` or `HPFakChamp`, three times for
    /// the types 701, 137 and 53, 1.8 times for a creature of class 10 on bronze.
    pub fn health(self, creature: u32, class: u8, rules: &Rules) -> f32 {
        let factor = match self {
            Self::Common | Self::Levelled => return 1.0,
            Self::Boss => rules.ranks.health[0],
            Self::Champion => rules.ranks.health[1],
        };
        match (creature, class) {
            (701 | 137 | 53, _) => 3.0 * factor,
            (_, 10) => 1.8 * factor,
            _ => factor,
        }
    }

    /// Its attack's and its defence's factors (`0x56B9AC`): `AWFak…` and `VWFak…`.
    pub fn fight(self, rules: &Rules) -> [f32; 2] {
        match self {
            Self::Common | Self::Levelled => [1.0, 1.0],
            Self::Boss => rules.ranks.fight[0],
            Self::Champion => rules.ranks.fight[1],
        }
    }

    /// Its kill's factor of the experience (`0x57D860`): twice for a boss, 1.5 times for a
    /// champion; a champion's levelling alone a common creature's (**hypothesis**: which bit
    /// gives the factor is not read, #621).
    pub fn experience(self) -> f32 {
        match self {
            Self::Common | Self::Levelled => 1.0,
            Self::Boss => 2.0,
            Self::Champion => 1.5,
        }
    }

    /// Its kill's experience of `earned`, what its level and worth earn ([`experience_of`]):
    /// times its factor, whole (**hypothesis**: the order of the factors makes no difference but
    /// at the least of 1).
    pub fn earned(self, earned: u32) -> u32 {
        whole(to_f32(earned) * self.experience())
    }
}

/// The balance's values of the bosses and the champions: the chance in 1,001 of a champion
/// (`HarteSau`), the levels above the hero's (`LevelUpBoss`, `LevelUpChamp`), bronze's
/// `OffLevel` and `LevelKap` by its column, and the factors of the health (`HPFak…`) and of the
/// attack and the defence (`AWFak…`, `VWFak…`), the boss's first.
#[derive(Clone, Debug, PartialEq)]
struct Ranks {
    champions: f32,
    level_up: [u32; 2],
    off: u32,
    cap: u32,
    health: [f32; 2],
    fight: [[f32; 2]; 2],
}

impl Default for Ranks {
    /// The original's values.
    fn default() -> Self {
        Self {
            champions: 20.0,
            level_up: [2, 2],
            off: 0,
            cap: 0,
            health: [2.5, 2.0],
            fight: [[1.5, 2.0], [1.3, 1.5]],
        }
    }
}

impl Ranks {
    fn of(b: &Balance) -> Self {
        let default = Self::default();
        let f = |name: &str, or: f32| b.f32(name).unwrap_or(or);
        let u = |name: &str, or: u32| {
            b.i32(name)
                .and_then(|v| u32::try_from(v).ok())
                .unwrap_or(or)
        };
        let row = |name: &str, or: u32| {
            b.row_i32(name, BRONZE)
                .and_then(|v| u32::try_from(v).ok())
                .unwrap_or(or)
        };
        Self {
            champions: f("HarteSau", default.champions),
            level_up: [u("LevelUpBoss", 2), u("LevelUpChamp", 2)],
            off: row("OffLevel", default.off),
            cap: row("LevelKap", default.cap),
            health: [f("HPFakBoss", 2.5), f("HPFakChamp", 2.0)],
            fight: [
                [f("AWFakBoss", 1.5), f("VWFakBoss", 2.0)],
                [f("AWFakChamp", 1.3), f("VWFakChamp", 1.5)],
            ],
        }
    }
}

/// How many values a row of the bands of the levels holds (`0x56A890`).
const BANDS: usize = 5;

/// The balance's rows of the factor of a creature's strike at the hero by its level
/// (`0x56A890`): the factors (`WaffenAll`, five floats) and the levels that start the bands
/// (`AllOverLv`, five integers).
#[derive(Clone, Debug, PartialEq)]
struct Bands {
    factors: [f32; BANDS],
    levels: [i32; BANDS],
}

impl Default for Bands {
    /// The original's values.
    fn default() -> Self {
        Self {
            factors: [0.6, 0.65, 0.75, 0.85, 0.95],
            levels: [0, 4, 10, 20, 35],
        }
    }
}

impl Bands {
    /// The rows of `b`, each value the original's where the balance lacks it.
    fn of(b: &Balance) -> Self {
        let default = Self::default();
        Self {
            factors: std::array::from_fn(|i| {
                b.row_f32("WaffenAll", i).unwrap_or(default.factors[i])
            }),
            levels: std::array::from_fn(|i| b.row_i32("AllOverLv", i).unwrap_or(default.levels[i])),
        }
    }

    /// The factor at the level `level` (`0x56A890`): the band `i` is the first of the first three
    /// whose levels hold it, `levels[i] ≤ L ≤ levels[i + 1]`, else the fourth whatever the level
    /// (the loop ends on its last band without its test); the factor
    /// `factors[i] + (factors[i + 1] − factors[i])/(L − levels[i])` on the x87 unit at the
    /// world's 53 bits, the step divided by the level's distance from the band's start rather
    /// than interpolated (a quirk): 0.65 at level 1, 0.6125 at 4, 0.75 at 5, 0.667 at 10, 0.85
    /// at 11, 0.76 at 20, 0.95 at 21 and toward 0.85 above. A factor of 0 or below is 1, and so
    /// is the one of a level at its band's start, which the original divides by zero (the level 0
    /// alone, on file).
    fn factor(&self, level: u32) -> f64 {
        let l = i32::from(u16::try_from(level).unwrap_or(u16::MAX));
        let i = (0..BANDS - 2)
            .find(|&i| self.levels[i] <= l && l <= self.levels[i + 1])
            .unwrap_or(BANDS - 2);
        let over = l.wrapping_sub(self.levels[i]);
        if over == 0 {
            return 1.0;
        }
        let (from, to) = (f64::from(self.factors[i]), f64::from(self.factors[i + 1]));
        let factor = (to - from) / f64::from(over) + from;
        if factor <= 0.0 { 1.0 } else { factor }
    }
}

/// The experience that a hero of level `hero` earns for a creature of level `level` worth
/// `[a, b]` (`0x57D860`): `a + E·b`, `E` the level on bronze's scale, the game's, times bronze's
/// `ProzExp`, times the factor of the levels (`0x551530`): with `r = (E + 9)/(H + 9)` against
/// the threshold `t` of the test of the levels (`takes_on`), 0.04 up to `t`, then
/// `(r − t)/(UnterExpVoll − t)`, 1 from `UnterExpVoll` to `ObereEXPVoll`, `ObereEXPVoll/r` above,
/// at least 0.04, and 1 for none; above bronze's `LevelKap` the hero's level `H` takes it down by
/// `(LevelKap/H)²`, from level 100 the decay of a high hero (`0x57E050`), and above the bronze
/// levels the fade of a difficulty outgrown (`StartLevMulti`); at least 1, whole. No bonuses of
/// the killer, bosses or champions yet (docs/decisions/2026-10-20-levels.md).
pub fn experience_of([a, b]: [u16; 2], level: u32, hero: u32, rules: &Rules) -> u32 {
    let [share, anteil, voll, upper] = rules.experience;
    let (e, h) = (to_f32(level), to_f32(hero));
    let mut x = (f32::from(a) + e * f32::from(b)) * share;
    let cap = rules.level_cap;
    if cap > 0.0 && h > cap {
        x *= cap * cap / (h * h);
    }
    x *= weighing(level, hero, (anteil, voll, upper));
    if x <= 0.0 {
        x = 1.0;
    }
    x *= decay(hero) * fade(h, rules.start_levels);
    whole(x.max(1.0))
}

/// The factor of the levels (`0x551530`, docs/re/fight.md, "The hero's levels"), which the kill's
/// experience and the drops take, of a creature of level `level` on bronze's scale against a hero
/// of level `hero`: with `r = (E + 9)/(H + 9)` against the threshold `t` (`(H + 6)/(H + 9)` up to
/// level 10, `(H − 6)/(H + 9)` from 92, else `anteil`), 0.04 up to `t`, then
/// `(r − t)/(voll − t)`, 1 from `voll` to `upper`, `upper/r` above; at least 0.04.
pub fn weighing(level: u32, hero: u32, (anteil, voll, upper): (f32, f32, f32)) -> f32 {
    let (e, h) = (to_f32(level), to_f32(hero));
    let r = (e + 9.0) / (h + 9.0);
    let t = match hero {
        0..=10 => (h + 6.0) / (h + 9.0),
        92.. => (h - 6.0) / (h + 9.0),
        _ => anteil,
    };
    let factor = if r <= t {
        0.04
    } else if r > upper {
        upper / r
    } else if r >= voll {
        1.0
    } else {
        (r - t) / (voll - t)
    };
    factor.max(0.04)
}

/// The decay of the experience of a hero of level `hero` (`0x57E050`): 1 up to level 99,
/// `0.985^(H − 99)` to 124, `0.98^(H − 99)` to 149, `0.975^(H − 109)` from 150.
fn decay(hero: u32) -> f32 {
    let power = |base: f32, from: u32| base.powf(to_f32(hero.saturating_sub(from)));
    match hero {
        0..=99 => 1.0,
        100..=124 => power(0.985, 99),
        125..=149 => power(0.98, 99),
        _ => power(0.975, 109),
    }
}

/// The fade of a difficulty outgrown, on bronze (`0x57D860`): 1 up to `S1 + 10`, then
/// `(S2 + 1 − H)/(S2 − S1 − 9)`, at least 0, of the row `StartLevMulti`'s `S1` and `S2`.
fn fade(h: f32, [s1, s2]: [f32; 2]) -> f32 {
    if h <= s1 + 10.0 {
        return 1.0;
    }
    ((s2 + 1.0 - h) / (s2 - s1 - 9.0)).max(0.0)
}

/// A creature's regeneration (`0x562730`): its value (`+0x138`) and what it has gathered toward
/// its next health (`+0x13C`, a whole number).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Regenerates {
    pub value: f32,
    kept: u32,
}

impl Regenerates {
    pub fn new(value: f32) -> Self {
        Self { value, kept: 0 }
    }

    /// Gathers `value · rate · seconds`, the seconds of the world's tick, and gives the health it
    /// makes (`0x562730`): the sum with what was kept taken whole at each tick, each 4,096 of it
    /// one health, the remainder kept the low ten bits of what was gathered when it gives any
    /// (the original's mask `0x3FF`, not `0xFFF`), so that a tick's step above 1,024 loses its
    /// overshoot's 1,024s (#616). A creature at its most health, `full`, or at 0 gathers
    /// nothing and keeps nothing.
    pub fn gather(&mut self, rate: f32, seconds: f32, full: bool) -> u32 {
        if full {
            self.kept = 0;
            return 0;
        }
        let gathered = whole_f64(
            f64::from(self.value) * f64::from(rate) * f64::from(seconds) + f64::from(self.kept),
        );
        if gathered < 4096 {
            self.kept = gathered;
            return 0;
        }
        self.kept = gathered & 0x3FF;
        gathered >> 12
    }
}

/// The regeneration value of a creature of `base` at level `level` (`0x5796A0`): `2·a3`, its
/// attribute 3 raised by its level, times 1 plus its skill 10's per cent (`0x566070`), taken
/// whole (`+0x138` is a whole number, which the skill's case stores with `_ftol`).
pub fn regeneration(base: &Base, level: u32, skills: &[(u8, u32)], rules: &Rules) -> f32 {
    regeneration_with(base, level, skills, &[], rules)
}

/// [`regeneration`] with the creature's effects `effects` (#651): before the skill, the effect 12
/// takes the value times 1 plus its own, truncated (`0x5796A0`).
pub fn regeneration_with(
    base: &Base,
    level: u32,
    skills: &[(u8, u32)],
    effects: &[Effect],
    rules: &Rules,
) -> f32 {
    let b3 = u32::from(base.attributes[3]);
    let mut value = to_f32(2 * (b3 + b3 * (level.max(1) - 1) / 10));
    for effect in effects.iter().filter(|e| e.id == effects::REGENERATION) {
        value = quickened(value, f64::from(effect.value) + 1.0);
    }
    let skill: f32 = skills
        .iter()
        .filter(|&&(skill, _)| skill == 10)
        .map(|&(_, at)| curve(rules.curves.regeneration, at) / 100.0)
        .sum();
    to_f32(whole(value * (1.0 + skill)))
}

/// How near its nearest hostile puts the hero in the fight, in world units: the client's
/// `+0x200` bit `0x8000`, which the hero's look sets and clears (`0x547070`), in which the hero
/// stands in its fight stance and regenerates at `RegInFight` (1,000 with a bow, which the hero
/// does not hold yet).
pub const IN_FIGHT_WITHIN: f32 = 350.0;
/// How near a hostile makes the hero regenerate at `RegEnemyNear` (`0x562730`).
const ENEMY_NEAR: f32 = 1000.0;

impl Rules {
    /// The rate of a creature's regeneration (`0x562730`): an NPC's `RegInFight`; the hero's
    /// `RegInFight` in the fight, the nearest hostile of its look (`hostile`, its distance, if one
    /// is found) within [`IN_FIGHT_WITHIN`], `RegEnemyNear` within 1,000, else `RegNoEnemy`.
    pub fn regeneration_rate(&self, hero: bool, hostile: Option<f32>) -> f32 {
        let [fighting, near, alone] = self.regeneration;
        match hostile {
            _ if !hero => fighting,
            Some(distance) if distance < IN_FIGHT_WITHIN => fighting,
            Some(distance) if distance <= ENEMY_NEAR => near,
            _ => alone,
        }
    }
}

/// The balance's values that a creature's fight takes: the points of the level's factor of the
/// health of a creature not a hero (`HP_s1`…`HP_w5`) and the difficulty's `ProzHP`; the offset
/// of the level's factor of the attributes (`BalStatOff`), the divisors of each kind's damage
/// and resistance multipliers (`BalChar*D`, `BalChar*R`) and of the resistances
/// (`BalanceRes*`), and the factors of all damages and resistances (`BalanceDmg`, `BalanceRes`);
/// the experience's factor of bronze (`ProzExp`) and the levels' (`UnterExpAnteil`,
/// `UnterExpVoll`, `ObereEXPVoll`); bronze's factors of the strikes between the hero and the
/// creatures (`ProzAW`, `ProzDmg`, `ProzRes`); the hero's rates of regeneration fighting, with a
/// hostile near and without (`RegInFight`, `RegEnemyNear`, `RegNoEnemy`); the distances and the
/// share of the range's factor of an attack (`FernAWMin`, `FernAWMax`, `FernAWProz`); the
/// bands of the factor of a creature's strike at the hero by its level (`WaffenAll`,
/// `AllOverLv`); and the factor of each arrow of a multi-shot, to the power of their number
/// (`Multipfeil`, `0x56D173`).
#[derive(Clone, Debug, PartialEq)]
pub struct Rules {
    points: Vec<(f32, f32)>,
    factor: f32,
    stat_offset: f32,
    damage_divisors: [f32; KINDS],
    resistance_divisors: [f32; KINDS],
    resistances: [f32; KINDS],
    balance: [f32; 2],
    experience: [f32; 4],
    difficulty: [f32; 3],
    regeneration: [f32; 3],
    curves: Curves,
    level_cap: f32,
    start_levels: [f32; 2],
    skill_levels: [u32; 6],
    ranks: Ranks,
    range: [f32; 3],
    bands: Bands,
    multi_shot: f32,
}

impl Default for Rules {
    /// Without the balance, no factor of the health, and the original's values of the rest.
    fn default() -> Self {
        Self {
            points: Vec::new(),
            factor: 1.0,
            stat_offset: 20.0,
            damage_divisors: [220.0, 440.0, 220.0, 440.0],
            resistance_divisors: [280.0, 280.0, 320.0, 260.0],
            resistances: [22.0, 17.0, 17.0, 25.0],
            balance: [1.0, 1.0],
            experience: [1.0, 0.85, 1.0, 1.1],
            difficulty: [0.65, 0.9, 1.0],
            regeneration: [36.0, 48.0, 128.0],
            curves: Curves::default(),
            level_cap: 50.0,
            start_levels: [60.0, 100.0],
            skill_levels: [3, 6, 12, 20, 30, 50],
            ranks: Ranks::default(),
            range: [250.0, 1000.0, 0.25],
            bands: Bands::default(),
            multi_shot: 0.83,
        }
    }
}

impl Rules {
    /// The rules of the balance `balance`, the original's values where it lacks them (the
    /// health's factors 1).
    pub fn of(balance: Option<&Balance>) -> Self {
        let default = Self::default();
        let Some(b) = balance else {
            return default;
        };
        let points = (1..=5)
            .map(|i| Some((b.f32(&format!("HP_s{i}"))?, b.f32(&format!("HP_w{i}"))?)))
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default();
        let named = |names: [&str; KINDS], or: [f32; KINDS]| {
            let mut values = or;
            for (value, name) in values.iter_mut().zip(names) {
                *value = b.f32(name).unwrap_or(*value);
            }
            values
        };
        Self {
            points,
            factor: b.row_f32("ProzHP", BRONZE).unwrap_or(1.0),
            stat_offset: b.f32("BalStatOff").unwrap_or(default.stat_offset),
            damage_divisors: named(
                ["BalCharPD", "BalCharFD", "BalCharMD", "BalCharGD"],
                default.damage_divisors,
            ),
            resistance_divisors: named(
                ["BalCharPR", "BalCharFR", "BalCharMR", "BalCharGR"],
                default.resistance_divisors,
            ),
            resistances: named(
                [
                    "BalanceResPh",
                    "BalanceResFe",
                    "BalanceResMa",
                    "BalanceResGi",
                ],
                default.resistances,
            ),
            balance: [
                b.f32("BalanceDmg").unwrap_or(1.0),
                b.f32("BalanceRes").unwrap_or(1.0),
            ],
            experience: [
                b.row_f32("ProzExp", BRONZE).unwrap_or(1.0),
                b.f32("UnterExpAnteil").unwrap_or(0.85),
                b.f32("UnterExpVoll").unwrap_or(1.0),
                b.f32("ObereEXPVoll").unwrap_or(1.1),
            ],
            difficulty: [
                b.row_f32("ProzAW", BRONZE).unwrap_or(0.65),
                b.row_f32("ProzDmg", BRONZE).unwrap_or(0.9),
                b.row_f32("ProzRes", BRONZE).unwrap_or(1.0),
            ],
            regeneration: {
                let mut rates = default.regeneration;
                for (rate, name) in
                    rates
                        .iter_mut()
                        .zip(["RegInFight", "RegEnemyNear", "RegNoEnemy"])
                {
                    if let Some(value) = b.i32(name).and_then(|v| i16::try_from(v).ok()) {
                        *rate = f32::from(value);
                    }
                }
                rates
            },
            curves: Curves::of(b),
            // The cap and the start levels by the difficulty itself, bronze's first.
            level_cap: b
                .row_i32("LevelKap", 0)
                .and_then(|v| i16::try_from(v).ok())
                .map_or(default.level_cap, f32::from),
            start_levels: [1, 2].map(|i| {
                b.row_i32("StartLevMulti", i)
                    .and_then(|v| i16::try_from(v).ok())
                    .map_or(default.start_levels[i - 1], f32::from)
            }),
            skill_levels: {
                let mut levels = default.skill_levels;
                for (i, level) in levels.iter_mut().enumerate() {
                    if let Some(value) = b
                        .i32(&format!("skilllearn{}", i + 1))
                        .and_then(|v| u32::try_from(v).ok())
                    {
                        *level = value;
                    }
                }
                levels
            },
            ranks: Ranks::of(b),
            range: {
                let [least, most, share] = default.range;
                let distance = |name, or| {
                    b.i32(name)
                        .and_then(|v| i16::try_from(v).ok())
                        .map_or(or, f32::from)
                };
                [
                    distance("FernAWMin", least),
                    distance("FernAWMax", most),
                    b.f32("FernAWProz").unwrap_or(share),
                ]
            },
            bands: Bands::of(b),
            multi_shot: b.f32("Multipfeil").unwrap_or(default.multi_shot),
        }
    }

    /// The factor of each arrow's damage of a shot of `arrows` arrows, the multi-shot's
    /// (`0x56D173`): `Multipfeil` to the power of the number, 1 for a single arrow.
    pub fn multi_shot(&self, arrows: u32) -> f32 {
        if arrows <= 1 {
            1.0
        } else {
            self.multi_shot
                .powi(i32::try_from(arrows).unwrap_or(i32::MAX))
        }
    }

    /// The factor of a creature's level `level` (`0x5658F0`): the first pair of points whose
    /// levels hold it, else the last pair, which the original takes without its test, so that
    /// the factor goes on along the last segment's line above the last point (#418); the pair's
    /// first weight plus the level times its slope, the level taken whole rather than from the
    /// first point (the original's own). 1 without two points.
    fn level_factor(&self, level: f32) -> f32 {
        let pairs = self.points.windows(2);
        let last = pairs.clone().last();
        pairs
            .take(self.points.len().saturating_sub(2))
            .find(|pair| pair[0].0 <= level && level <= pair[1].0)
            .or(last)
            .map_or(1.0, |pair| {
                let ((s0, w0), (s1, w1)) = (pair[0], pair[1]);
                w0 + level * (w1 - w0) / (s1 - s0)
            })
    }
}

/// Whether a creature of level `level` takes on a hero of level `hero` (`0x542B20`, docs/re/fight.md,
/// "Who fights"): its level on bronze's scale, `E` (the game plays bronze), against the hero's,
/// `r = (E + 9)/(H + 9)` above the threshold `t` (`(H + 6)/(H + 9)` up to level 10, `(H − 6)/(H + 9)`
/// from 92, else `anteil`), by more than 0.04 of `voll − t`: when the hero would earn more than the
/// least experience from it.
pub fn takes_on(level: u32, hero: u32, (anteil, voll): (f32, f32)) -> bool {
    let (e, h) = (to_f32(level), to_f32(hero));
    let r = (e + 9.0) / (h + 9.0);
    let t = match hero {
        0..=10 => (h + 6.0) / (h + 9.0),
        92.. => (h - 6.0) / (h + 9.0),
        _ => anteil,
    };
    r > t && (r - t) / (voll - t) > 0.04
}

/// A level, or another whole number, as a float.
fn to_f32(level: u32) -> f32 {
    f32::from(u16::try_from(level).unwrap_or(u16::MAX))
}

/// The whole part of a float, 0 below 0.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a damage, whole and positive, far below the integers' range"
)]
fn whole(value: f32) -> u32 {
    value.max(0.0).floor() as u32
}

/// The base attributes that the stats' maker `0x586C00` gives a type without a `Creature.pak`
/// record (docs/re/spawning.md, "A type without a record").
pub const RECORDLESS_BASE: [u8; 6] = [25, 15, 20, 20, 10, 5];
/// The walk and run speeds that `0x4F73C0` gives a type that its list of types leaves out, which
/// `0x4F75F0` takes where the type's record gives none.
pub const DEFAULT_SPEEDS: [u16; 2] = [100, 200];

/// The record that a type of `id` without one in `Creature.pak` is made as, the waters' crab 163
/// (`0x43AD80` gives it none; docs/re/spawning.md, "A type without a record"): of class 0, the
/// stats' maker's base attributes ([`RECORDLESS_BASE`]) with no skill, bonus or damping, the flags
/// 0 of the table's first record, which the shadow's test reads, and the default speeds
/// ([`DEFAULT_SPEEDS`], the crab's); no experience, its kill's not read.
pub fn recordless(id: u32) -> Creature {
    Creature {
        id,
        class: 0,
        flags: 0,
        experience: [0, 0],
        base: RECORDLESS_BASE,
        skills: [0; 2],
        skills_extra: [0; 16],
        speed: DEFAULT_SPEEDS,
        bonus: [[0; 2]; 6],
        bonus_value: [0; 6],
        bonus_class: [0; 6],
        damping: [[0; 5]; KINDS],
    }
}

/// A creature type's values of `Creature.pak` that its fight takes: its six base attributes
/// (`+0x0C`), its six bonuses (`+0x2A`: a kind and a target; their values `+0x36` and the
/// opponents' classes `+0x3C`), the per cent of each kind's damage that it takes off on each
/// difficulty (`+0x42`), and its experience (`+0x08`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Base {
    /// Its type, the record's id.
    pub id: u32,
    pub attributes: [u8; 6],
    pub bonus: [[u8; 2]; 6],
    pub bonus_value: [u8; 6],
    pub bonus_class: [u8; 6],
    pub damping: [[u8; 5]; KINDS],
    pub experience: [u16; 2],
    /// Its eight skill slots (`0x586C00`): `+0x14`, `+0x15` and, for a type not of class 1, the
    /// first six of `+0x16`; 0 for none.
    pub skills: [u8; 8],
    /// Whether its creature is in the Vampiress's night form (`+0x14` bit `0x2000`), which takes
    /// [`NIGHT_SLOWS`] off its three speeds.
    pub night: bool,
}

/// What the Vampiress's night form takes off the attack speed, `+0x3E` and the pace, after the
/// skills and before their limits (`0x5796A0` at `0x57CA23`, the creature's `+0x14` bit
/// `0x2000`; seen: 154, 120 and 130 by day, 139, 105 and 115 by night).
pub const NIGHT_SLOWS: f32 = 15.0;

impl Base {
    pub fn of(creature: &Creature) -> Self {
        Self {
            id: creature.id,
            attributes: creature.base,
            bonus: creature.bonus,
            bonus_value: creature.bonus_value,
            bonus_class: creature.bonus_class,
            damping: creature.damping,
            experience: creature.experience,
            skills: {
                let mut skills = [0; 8];
                skills[..2].copy_from_slice(&creature.skills);
                if creature.class != 1 {
                    skills[2..].copy_from_slice(&creature.skills_extra[..6]);
                }
                skills
            },
            night: false,
        }
    }
}

/// What a creature holds, as its values take it: a weapon (`0x546F80`), a shield (`0x54A660`),
/// a ranged weapon (the kinds 7, 12, 13 and 14, `0x54A2A0`), and the values of the items it holds
/// and wears that have them: an NPC's in its hands, a hero's of its template too.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Holds {
    pub weapon: bool,
    pub shield: bool,
    pub ranged: bool,
    pub items: Vec<ItemValues>,
}

/// An item's values (its record, docs/re/items.md): each kind's least and most damage, its
/// resistances, its `A` and `D`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ItemValues {
    pub damage: [[f32; 2]; KINDS],
    pub resistance: [f32; KINDS],
    pub attack: f32,
    pub defence: f32,
    /// Its bonus slots, where its record gives them (a fixed item's, a hero template's).
    pub bonuses: [Bonus; BONUSES],
}

/// How many bonus slots an item's record has.
pub const BONUSES: usize = 8;

/// A bonus slot of an item's record (docs/re/items.md): its condition word (`+0x3A`), its effect
/// (`+0x4A`, the effect in the low 16 bits, the attribute that scales it in byte 2) and its value
/// (`+0x6A`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bonus {
    pub condition: u16,
    pub effect: u32,
    pub value: i16,
}

impl Bonus {
    /// Whether it counts in the values of a creature of type `holder` (`0x5796A0`): its
    /// condition's low byte 0 or 13, of no opponent's class, and its bits 8 to 11 0 or 7, or
    /// the holder's type (6 the Vampiress's both, 6 and 7; `0x568B30`).
    fn holds(self, holder: u32) -> bool {
        let [class, kind] = self.condition.to_le_bytes();
        let kind = u32::from(kind & 0xF);
        (class == 0 || class == 13)
            && (kind == 0 || kind == 7 || kind == holder || (kind == 6 && holder == 7))
    }

    /// Its value, times one of the attributes `a` over 100 by the effect's byte 2 (`0x568C40`:
    /// 1 `a0`, 2 `a2`, 3 `a1`, 4 `a3`, 5 `a4`, 6 `a5`), the product whole before the hundredth.
    fn value(self, a: [u32; 6]) -> f32 {
        let scale = match (self.effect >> 16) & 0xFF {
            1 => a[0],
            2 => a[2],
            3 => a[1],
            4 => a[3],
            5 => a[4],
            6 => a[5],
            _ => return f32::from(self.value),
        };
        let product = i64::from(self.value) * i64::from(scale);
        #[expect(
            clippy::cast_precision_loss,
            reason = "the client's product of a value and an attribute, a few thousand at most"
        )]
        let product = product as f32;
        product * 0.01
    }
}

impl ItemValues {
    /// The values of an item's record, given as its `u16` words from its `+4`, as `Weapon.pak
    /// +0x84` holds a fixed item's and a hero template its items' (docs/re/items.md): each kind's
    /// least damage at `+0x1C`, its most at `+0x24`, `A` at `+0x2C`, `D` at `+0x2E` and the
    /// resistances at `+0x32`, and its bonus slots ([`Bonus`]); a word past the end is 0.
    pub fn of_record(words: &[u16]) -> Self {
        let word = |offset: usize| words.get((offset - 4) / 2).copied().unwrap_or(0);
        let at = |offset: usize| f32::from(word(offset));
        Self {
            damage: std::array::from_fn(|kind| [at(0x1C + 2 * kind), at(0x24 + 2 * kind)]),
            resistance: std::array::from_fn(|kind| at(0x32 + 2 * kind)),
            attack: at(0x2C),
            defence: at(0x2E),
            bonuses: std::array::from_fn(|slot| Bonus {
                condition: word(0x3A + 2 * slot),
                effect: u32::from(word(0x4A + 4 * slot)) | (u32::from(word(0x4C + 4 * slot)) << 16),
                value: i16::from_le_bytes(word(0x6A + 2 * slot).to_le_bytes()),
            }),
        }
    }
}

/// The words of a template of the items' values (`wpmod.bin`, docs/re/items.md).
type TemplateWords = [u32; 53];

/// The templates of the items' values by the item types they serve, the first in the file's
/// order for a type (`0x42CA70`), and the types' parents (`Weapon.pak +0x24`).
#[derive(Clone, Debug, Default)]
pub struct Templates {
    by_type: HashMap<u32, TemplateWords>,
    parents: HashMap<u32, u32>,
}

/// The constant 0.01 of the item maker and of the skills (`0x890CB0`), a single-precision float a
/// little below it.
const HUNDREDTH: f64 = 0.009_999_999_776_482_582;

/// A value stored as a single-precision float.
fn single(value: f64) -> f64 {
    f64::from(stored(value))
}

/// A double stored as a float, as the x87 unit stores a result: the nearest float.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the original stores the value as a float"
)]
fn stored(value: f64) -> f32 {
    value as f32
}

/// A per cent `value` added to a multiplier `times` as the world's x87 unit adds it, at 53 bits
/// (#589), and stored as a float (`0x566070`, `0x5796A0`): `value` times the float 0.01
/// (`0x890CB0`) plus `times` (#593).
fn raised(times: f32, value: f32) -> f32 {
    stored(f64::from(value) * HUNDREDTH + f64::from(times))
}

/// The whole part of a double, 0 below 0.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "an item's value, whole and positive, far below the integers' range"
)]
fn whole_f64(value: f64) -> u32 {
    value.max(0.0).floor() as u32
}

/// A word of a template read as signed.
fn signed(word: u32) -> i64 {
    i64::from(i32::from_ne_bytes(word.to_ne_bytes()))
}

/// The game's roll of a whole number between `least` and `most` (`0x443B20`): a negative least
/// counts as 0, a negative most as 1, the two swapped when reversed.
fn roll_between(least: i64, most: i64, rng: &mut impl Dice) -> f64 {
    let (a, b) = (least.max(0), if most < 0 { 1 } else { most });
    let (low, high) = (a.min(b), a.max(b));
    let span = u32::try_from(high - low + 1).unwrap_or(1);
    let rolled = low + i64::from(rng.below(span));
    f64::from(u32::try_from(rolled).unwrap_or(0))
}

impl Templates {
    /// The templates of `modifiers` and the parents of `weapons`.
    pub fn new(modifiers: &[WeaponModifier], weapons: &[Weapon]) -> Self {
        let mut by_type = HashMap::new();
        for modifier in modifiers {
            let Ok(words) = TemplateWords::try_from(modifier.words.as_slice()) else {
                continue;
            };
            let count = usize::try_from(words[5]).unwrap_or(0).min(5);
            for &item in &words[..count] {
                by_type.entry(item).or_insert(words);
            }
        }
        let parents = weapons
            .iter()
            .filter(|w| w.parent != 0)
            .map(|w| (w.id, w.parent))
            .collect();
        Self { by_type, parents }
    }

    /// The words 38 to 40 of the template of an item of type `item`, read as signed
    /// (docs/re/items.md, "The template"): a byte of the item's flags, 29 for "make nothing" and
    /// −1 for a type that the drops never make; the item's least level; and the least quality a
    /// drop takes the type at. The template is the maker's, by the chain of parents, which on
    /// file finds the same as the drops' own lookup `0x42CA70` (one parent) for every type they
    /// choose from (#414).
    pub fn limits(&self, item: u32) -> Option<[i64; 3]> {
        self.of(item)
            .map(|words| [38, 39, 40].map(|i| signed(words[i])))
    }

    /// The words 38 to 40 of the template that the drops' lookup `0x42CA70` finds for an item of
    /// type `item` (#596): its parent's, once, for a type within 2001–2998 or 6501–7998, none
    /// where that parent lies within 2000–2999 or 6500–7999 itself, else its own.
    pub fn looked_up(&self, item: u32) -> Option<[i64; 3]> {
        let item = if matches!(item, 2001..=2998 | 6501..=7998) {
            match self.parents.get(&item) {
                Some(2000..=2999 | 6500..=7999) => return None,
                Some(&parent) => parent,
                None => item,
            }
        } else {
            item
        };
        self.by_type
            .get(&item)
            .map(|words| [38, 39, 40].map(|i| signed(words[i])))
    }

    /// The template of an item of type `item`, its parent's while the type lies within 2000–2999
    /// or 6500–7999, at most six times (`0x5D5080`).
    fn of(&self, item: u32) -> Option<&TemplateWords> {
        let mut item = item;
        for _ in 0..6 {
            if !matches!(item, 2000..=2999 | 6500..=7999) {
                break;
            }
            match self.parents.get(&item) {
                Some(&parent) => item = parent,
                None => break,
            }
        }
        self.by_type.get(&item)
    }

    /// The values of an item of type `item` made at `level` on bronze (`0x5D5080`,
    /// docs/re/items.md), none without a template or for its word 38 of 29: the level's factor
    /// `f = 25 + 2.5·level`; the damage's budget `p = max(S·f/100 + 0.5, 1)` by the scale `S`,
    /// its kinds rolled by their chances, normalised to `p` and split by the spread `w` into a
    /// least and a most; the resistances likewise, unsplit; `A` and `D` by their chances times
    /// `f`. No magic bonus yet (docs/decisions/2026-10-20-weapons.md).
    pub fn make<D: Dice>(&self, item: u32, level: u32, rng: &mut D) -> Option<ItemValues> {
        let words = self.of(item)?;
        if words[38] == 29 {
            return None;
        }
        let w = |i: usize| signed(words[i]);
        let level = f64::from(level.min(270));
        let f = single(25.0 + 250.0 * level * HUNDREDTH);
        let p = single((f64::from(words[6]) * f * HUNDREDTH + 0.5).max(1.0));
        let chance = |i: usize, rng: &mut D| i64::from(rng.below(100)) < w(i);
        // A triple's least and most, the most above the least.
        let rolled = |i: usize, rng: &mut D| {
            let (least, most) = (w(i + 1), w(i + 2));
            roll_between(least, most.max(least + 1), rng)
        };
        let mut values = ItemValues::default();
        // The damage: fire, magic, poison (unless both others), then physical.
        if [8, 11, 14, 17].iter().any(|&i| w(i) > 0) {
            let mut a = [0_u32; KINDS];
            for (kind, i) in [(1, 11), (2, 14), (3, 17)] {
                if kind == 3 && a[1] > 0 && a[2] > 0 {
                    continue;
                }
                if chance(i, rng) {
                    a[kind] = whole_f64((rolled(i, rng) * p * HUNDREDTH).max(1.0));
                }
            }
            if chance(8, rng) {
                a[0] = if a[1..].iter().all(|&v| v == 0) {
                    whole_f64(p)
                } else {
                    whole_f64(rolled(8, rng) * p * HUNDREDTH)
                };
            }
            let n = normalised(a, p);
            let q = (f64::from(words[7]) * HUNDREDTH).min(100.0);
            for (ends, &n) in values.damage.iter_mut().zip(&n) {
                if n > 0 {
                    let n = f64::from(n);
                    *ends = [
                        to_f32(whole_f64((n - n * q).max(1.0))),
                        to_f32(whole_f64((q + 1.0) * n)),
                    ];
                }
            }
        }
        // The resistances: 1, 2, 3 (unless both others), then 0.
        if [20, 23, 26, 29].iter().any(|&i| w(i) > 0) {
            let mut a = [0_u32; KINDS];
            for (kind, i) in [(1, 23), (2, 26), (3, 29)] {
                if kind == 3 && a[1] > 0 && a[2] > 0 {
                    continue;
                }
                if chance(i, rng) {
                    a[kind] = whole_f64((rolled(i, rng) * p * HUNDREDTH).max(1.0));
                }
            }
            if chance(20, rng) {
                a[0] = if a[1..].iter().all(|&v| v == 0) {
                    whole_f64(p)
                } else {
                    whole_f64(rolled(20, rng) * p * HUNDREDTH)
                };
            }
            values.resistance = normalised(a, p).map(to_f32);
        }
        for (value, i) in [(&mut values.attack, 32), (&mut values.defence, 35)] {
            if chance(i, rng) {
                let (least, most) = (w(i + 1), w(i + 2));
                *value = to_f32(whole_f64(roll_between(least, most, rng) * f * HUNDREDTH));
            }
        }
        Some(values)
    }
}

/// The raw values `a` normalised to the budget `p` (`0x5D0280`): each `⌊a·p/T⌋` of their total
/// `T`, the physical `⌊p⌋` when the total is 0, the remainder `⌊p − Σ + 0.5⌋` to the first that
/// has any.
fn normalised(mut a: [u32; KINDS], p: f64) -> [u32; KINDS] {
    if a.iter().all(|&v| v == 0) {
        a[0] = whole_f64(p);
    }
    let total = f64::from(a.iter().sum::<u32>());
    if total <= 0.0 {
        return a;
    }
    let mut n = a.map(|v| whole_f64(f64::from(v) * p / total));
    let rest = whole_f64(p - f64::from(n.iter().sum::<u32>()) + 0.5);
    if let Some(first) = n.iter_mut().find(|v| **v > 0) {
        *first += rest;
    }
    n
}

/// The skills a creature of `base` counts at level `level`, and their levels: an NPC the first
/// of its slots, none below level 3, one up to 6, two up to 20, three up to 35, else all eight,
/// each at a third of its level (`0x564D60`); a hero its two slots at level 1 (`0x586C00`;
/// **hypothesis**: the template's own levels are not read).
pub fn skills(base: &Base, level: u32, hero: bool) -> Vec<(u8, u32)> {
    let (count, at) = if hero {
        (2, 1)
    } else {
        let count = match level {
            0..3 => 0,
            3..=6 => 1,
            7..=20 => 2,
            21..=35 => 3,
            _ => 8,
        };
        (count, level / 3)
    };
    base.skills
        .iter()
        .take(count)
        .filter(|&&skill| skill != 0)
        .map(|&skill| (skill, at))
        .collect()
}

/// A skill's per cent at level `level` (`0x566070`), of the balance's triple `[off, S, w]`:
/// `off` at level 1, `w` at level `S + 1`, `off + 2(w − off)(1 − 1/(1 + (level − 1)/S))`; none
/// below level 1. The world's x87 unit computes it at 53 bits and stores it as a float (#589,
/// #593): the float nearest the doubles' result, which single precision can miss by a bit.
pub(crate) fn curve(triple: [f32; 3], level: u32) -> f32 {
    stored(per_cent(triple, level))
}

/// A skill's per cent at level `level` as the world's x87 unit computes it at 53 bits, before
/// it is stored ([`curve`]): the most health's raise keeps it on the unit (`0x5658F0`, #598).
fn per_cent([off, steps, most]: [f32; 3], level: u32) -> f64 {
    if level < 1 || steps <= 0.0 {
        return 0.0;
    }
    let [off, steps, most] = [off, steps, most].map(f64::from);
    let x = (most - off) * (1.0 - 1.0 / (f64::from(level - 1) / steps + 1.0));
    x + x + off
}

/// The balance's triples of the skills' curves (`0x566070`): the damages' (2, `WK…`), the
/// attack's and the defence's (8, `W…AW`, `W…VW`), the regeneration's and the most health's (10,
/// `HR…AW`, `HP…VW`), the
/// defence's again (9, `P…`, with a shield `P…S`), each resistance's and the pace's (11, `R…Ph`,
/// `R…Fe`, `R…Ma`, `R…Gi`, `R…SP`), and the fist fight's attack and attack speed (24, `FK…AW`,
/// `FK…SP`).
#[derive(Clone, Debug, PartialEq)]
struct Curves {
    damage: [f32; 3],
    attack: [f32; 3],
    defence: [f32; 3],
    regeneration: [f32; 3],
    health: [f32; 3],
    parry: [f32; 3],
    parry_shield: [f32; 3],
    resistances: [[f32; 3]; KINDS],
    pace: [f32; 3],
    fist: [f32; 3],
    fist_speed: [f32; 3],
}

impl Default for Curves {
    /// The original's values.
    fn default() -> Self {
        Self {
            damage: [12.0, 50.0, 200.0],
            attack: [7.0, 50.0, 125.0],
            defence: [13.0, 50.0, 225.0],
            regeneration: [6.0, 50.0, 150.0],
            health: [4.0, 50.0, 130.0],
            parry: [15.0, 50.0, 250.0],
            parry_shield: [25.0, 50.0, 375.0],
            resistances: [
                [12.0, 50.0, 250.0],
                [12.0, 50.0, 200.0],
                [12.0, 50.0, 200.0],
                [12.0, 50.0, 200.0],
            ],
            pace: [1.0, 50.0, 10.0],
            fist: [12.0, 50.0, 200.0],
            fist_speed: [6.0, 50.0, 100.0],
        }
    }
}

impl Curves {
    /// The triples of `balance`, the original's where it lacks them.
    fn of(balance: &Balance) -> Self {
        let default = Self::default();
        let triple = |names: [&str; 3], or: [f32; 3]| {
            let mut values = or;
            for (value, name) in values.iter_mut().zip(names) {
                *value = balance.f32(name).unwrap_or(*value);
            }
            values
        };
        let resistance = |kind: usize, suffix: &str| {
            triple(
                [
                    &format!("Roff{suffix}"),
                    &format!("R__s{suffix}"),
                    &format!("R__w{suffix}"),
                ],
                default.resistances[kind],
            )
        };
        Self {
            damage: triple(["WKoff", "WK_s", "WK_w"], default.damage),
            attack: triple(["WoffAW", "W__sAW", "W__wAW"], default.attack),
            defence: triple(["WoffVW", "W__sVW", "W__wVW"], default.defence),
            regeneration: triple(["HRoffAW", "HR__sAW", "HR__wAW"], default.regeneration),
            health: triple(["HPoffVW", "HP__sVW", "HP__wVW"], default.health),
            parry: triple(["Poff", "P__s", "P__w"], default.parry),
            parry_shield: triple(["PoffS", "P__sS", "P__wS"], default.parry_shield),
            resistances: [
                resistance(0, "Ph"),
                resistance(1, "Fe"),
                resistance(2, "Ma"),
                resistance(3, "Gi"),
            ],
            pace: triple(["RoffSP", "R_s_SP", "R_w_SP"], default.pace),
            fist: triple(["FKoffAW", "FK_s_AW", "FK_w_AW"], default.fist),
            fist_speed: triple(["FKoffSP", "FK_s_SP", "FK_w_SP"], default.fist_speed),
        }
    }
}

/// A creature's attack `A`, defence `D`, each kind's least and most damage and resistance, with
/// the balance's factors, the per cent of each kind's damage that it takes off, and its attack
/// speed in per cent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Combat {
    pub attack: f32,
    pub defence: f32,
    pub damage: [[f32; 2]; KINDS],
    pub resistance: [f32; KINDS],
    pub damping: [u8; KINDS],
    pub speed: f32,
    /// Its pace, the stats' word `+0x40` ([`pace`]), whose hundredth scales its walk and run
    /// speeds and the rate of their clips ([`Self::pace_factor`]).
    pub pace: f32,
}

impl Combat {
    /// The speed of its attack's clip (`0x4E4CF0`): its attack speed over 100, times the
    /// factor of chained strikes (`+0x210`, 1 for a strike that is not chained).
    pub fn clip_speed(&self) -> f32 {
        self.speed / 100.0
    }

    /// The factor of its walk and run speeds and of their clips' rate (`0x585070` read by
    /// `0x4F75F0`): its pace over 100 times its float `+0x2C9`, 1.0 (docs/re/movement.md, "The
    /// speed"; #599).
    pub fn pace_factor(&self) -> f32 {
        self.pace / 100.0
    }
}

/// The attack speed of a creature of type `id` (`0x5796A0`, `0x584FE0`): 140 for every type but
/// the 5th's (the Wood Elf's), 100; no items, weapon skills or effects yet (#294).
pub fn attack_speed(id: u32) -> f32 {
    if id == 5 { 100.0 } else { 140.0 }
}

/// The pace of a creature of type `id` before its items and skills, the word `+0x40` that the
/// recomputation of its stats starts from (`0x5796A0`, at `0x579A61`): 130 for every type but the
/// 5th's (the Wood Elf's), 120. A horse's comes from a table by its saddle (`0x4462B0`), which
/// is not read, and the hostile creatures' on the difficulties 3 and 4 are 160 and 190.
pub fn pace(id: u32) -> f32 {
    if id == 5 { 120.0 } else { 130.0 }
}

/// An effect on a creature (docs/re/arts.md, "The effects"): an entry of its stats' list from
/// `+0x11A` to `+0x11E`, 32 bytes, that `0x582D50` adds: the effect's id (`+0x00`), a whole value
/// (`+0x04`), a value as a float (`+0x14`), the art that keys it (`+0x1C`), and the level its
/// adder's eighth argument puts first in the entry's list at `+0x08` (the arrowproof's, which
/// the deflection reads, `0x561090`; 0 for none).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effect {
    pub id: u32,
    pub whole: i32,
    pub value: f32,
    pub art: u32,
    pub level: u16,
}

impl Effect {
    /// An effect of `id` and `value`, of no whole value and no art.
    pub fn new(id: u32, value: f32) -> Self {
        Self {
            id,
            whole: 0,
            value,
            art: 0,
            level: 0,
        }
    }
}

/// Turns a part of one kind's least and most damage into another's by the effect 53 `turns`
/// (`0x5796A0` at `0x57CA45`): each part `part · end + 0.5` truncated, `part` `1 − value^whole`
/// stored as a float, taken from the one kind and added to the other.
fn turn_damage(damage: &mut [[f32; 2]; KINDS], turns: &Effect) {
    let from = usize::from(turns.level & 0xF);
    let to = usize::from((turns.level >> 8) & 0xF);
    if from >= KINDS || to >= KINDS {
        return;
    }
    let value = f64::from(turns.value.clamp(0.0, 1.0));
    let part = f64::from(stored(1.0 - value.powi(turns.whole)));
    let moved = damage[from].map(|end| to_f32(whole_f64(part * f64::from(end) + 0.5)));
    for (end, moved) in moved.into_iter().enumerate() {
        damage[from][end] -= moved;
        damage[to][end] += moved;
    }
}

/// The effects' ids that the recomputation follows (`0x5796A0`, its loop over the list after the
/// items and the record's bonuses, before the skills).
pub mod effects {
    /// `A`'s multiplier (`+0xE6`) and `D`'s (`+0xEA`), plus the value.
    pub const ATTACK: u32 = 1;
    pub const DEFENCE: u32 = 2;
    /// The four kinds' damage multipliers (`+0x96` to `+0xA2`), plus the value, from the
    /// physical; or the kind of the whole value, 0 to 3.
    pub const DAMAGE: u32 = 3;
    pub const DAMAGE_OF: u32 = 33;
    /// The four kinds' resistances (`+0x66` to `+0x72`), plus the value, from the physical.
    pub const RESISTANCE: u32 = 7;
    /// Both regeneration speeds of the arts, the spells' (`+0xEE`) and the weapon arts' (`+0xF2`).
    pub const ARTS_SPEED: u32 = 11;
    /// The regeneration value (`+0x138`), times 1 plus the value, truncated.
    pub const REGENERATION: u32 = 12;
    /// The attack speed (`+0x3C`) and the pace (`+0x40`) times the value, truncated, and `+0x3E`
    /// too for 28.
    pub const SPEEDS: u32 = 28;
    pub const QUICKENS: u32 = 52;
    /// The whole value taken from the attack speed, the pace and `+0x3E`.
    pub const SLOWS: u32 = 30;
    /// `D`'s multiplier times 100 over the value, a value above `f32::EPSILON`.
    pub const SHIELDS: u32 = 42;
    /// The fire damage's most and least (`+0x7A`, `+0x8A`), plus the whole value.
    pub const FIRE: u32 = 59;
    /// A part of one kind's damage turned into another's, the first of them alone (`0x5796A0` at
    /// `0x57CA45`, after the loop): the kinds the low nibble of its level and that of its second
    /// byte (`0x582D50`'s eighth argument), the part `1 − value^whole`, its value held from 0 to 1
    /// as it is added (`0x582D50`); the Daemon's forms' auras (docs/re/arts.md, "The Daemon's").
    pub const TURNS: u32 = 53;
}

/// The least multiplier of `D` that the effect 2 leaves (the float `0x3C23D70A`).
const LEAST_DEFENCE_TIMES: f32 = 0.01;

/// A whole number of the stats (the speeds `+0x3C`, `+0x40`, the regeneration value `+0x138`)
/// times `by`, as the x87 unit multiplies them, truncated as `_ftol` does (`0x5796A0`, the effects
/// 12, 28 and 52).
#[expect(
    clippy::cast_possible_truncation,
    reason = "a speed or a regeneration value, far below the integers' range"
)]
fn quickened(whole: f32, by: f64) -> f32 {
    (f64::from(whole) * by).trunc() as f32
}

/// The attack, defence, damage and resistances of an NPC of `base` and level `level`, `armed`
/// when it holds a weapon (the recomputation `0x5796A0` and the attack record `0x568D70`;
/// docs/re/fight.md, "An NPC's attack, defence and damage"):
/// - the attributes `a_i = b_i + ⌊b_i(L − 1)/10⌋` and the level's factor
///   `F = (156 − BalStatOff)·L/156 + BalStatOff + 9`;
/// - `A = (a0 + a2)/2`, `D = 0.2·a0 + 0.8·a2`;
/// - unarmed, the bonuses of the kinds 1 to 7 (`x` per cent of an attribute or of the level) to a
///   kind's most (`x`) and least (`x/2 + 1`); armed or not, the bonuses to a kind's resistance;
///   a physical most of 0 takes `0.15(2a0 + a2) + L/2` to `0.175(a0 + a2) + L/2`;
/// - each kind's damage times `1 +` its part of the base attributes times `F` over `BalChar*D`;
/// - the resistances of the attributes over `BalanceRes*`, times `1 + F·b1/10` over `BalChar*R`.
///
/// - the skills `skills` (`0x566070`): 2 raises every damage multiplier, 8 the attack's and the
///   defence's, 9 the defence's (more with a shield), 11 each resistance's, by its curve's per
///   cent, and 24, the fist fight, unarmed, the attack's by its curve's per cent and the attack
///   speed by its other curve, taken whole (#588); an attack or a defence whose multiplier is not
///   1 is truncated.
/// - `A` and `D`, their multipliers and their products as the world's x87 unit computes them, at
///   53 bits (#589, #593): each part of `A` and `D` times the float 0.01, the items' first, and
///   each per cent of a multiplier (the items' bonuses, then the skills in their slots' order)
///   times it, added and stored as a float in turn; the product of `A` or `D` and its multiplier
///   truncated as it is, not stored as a float first.
///
/// No items, combat arts or bonuses of other kinds yet; an armed NPC's damage is the default's,
/// its weapon's unknown (#294; docs/decisions/2026-10-20-hits.md). Without effects
/// ([`combat_with`]).
pub fn combat(
    base: &Base,
    level: u32,
    holds: Holds,
    skills: &[(u8, u32)],
    rules: &Rules,
) -> Combat {
    combat_with(base, level, holds, skills, &[], rules)
}

/// [`combat`] with the creature's effects `effects` (#651), which the recomputation adds after
/// the items and before the skills, each in its turn by its id ([`effects`]): to `A`'s and `D`'s
/// multipliers, `D`'s held at 0.01 at least, and `D`'s times 100 over a value; to the kinds'
/// damage multipliers and to their resistances; the attack speed and the pace times a value or
/// less a whole one, truncated; the fire damage's ends. The effect 40, by the art 1015's level,
/// is left out.
pub fn combat_with(
    base: &Base,
    level: u32,
    holds: Holds,
    skills: &[(u8, u32)],
    effects: &[Effect],
    rules: &Rules,
) -> Combat {
    let armed = holds.weapon;
    let level = level.max(1);
    let b = base.attributes.map(f32::from);
    let grown = base
        .attributes
        .map(|b| u32::from(b) + u32::from(b) * (level - 1) / 10);
    let a = grown.map(to_f32);
    let l = to_f32(level);
    let off = rules.stat_offset;
    let f = (156.0 - off) * l / 156.0 + off + 9.0;
    let mut damage = [[0.0_f32; 2]; KINDS];
    let mut resistance = [0.0_f32; KINDS];
    for i in 0..6 {
        let [kind, target] = base.bonus[i];
        // A bonus of a class holds only against an opponent of that class; none has one.
        if base.bonus_class[i] != 0 {
            continue;
        }
        let source = match kind {
            1 => a[0],
            2 => a[1],
            3 => a[3],
            4 => a[4],
            5 => a[2],
            6 => a[5],
            7 => l,
            _ => continue,
        };
        let x = f32::from(base.bonus_value[i]) * source / 100.0;
        match usize::from(target) {
            t @ 1..=4 if !armed => {
                damage[t - 1][1] += x;
                damage[t - 1][0] += x / 2.0 + 1.0;
            }
            t @ 5..=8 => resistance[t - 5] += x,
            _ => {}
        }
    }
    // The items' own damage, resistances, `A` and `D`, and their bonus slots (`0x5796A0`, #490):
    // 801 to 804 a kind's damage (with the condition's bit 15, its resistance), 805 to 808 a
    // hundredth to its multiplier, 809 and 810 to those of `A` and `D`, 811 the attack speed, 815
    // to 820 the attributes, by the attributes before them; the other effects are not followed.
    // `A` and `D` and their multipliers as the recomputation stores them (`+0x5A`, `+0x5E`, `+0xE6`,
    // `+0xEA`): each item's `A` and `D` times the row's 100 per cent, then its bonuses.
    let (mut attack, mut defence) = (0.0_f32, 0.0_f32);
    let (mut attack_times, mut defence_times) = (1.0_f32, 1.0_f32);
    let mut item_multipliers = [0.0_f32; KINDS];
    let mut speed_bonus = 0.0;
    let mut pace = pace(base.id);
    let mut attribute_bonuses = [0.0_f32; 6];
    for item in &holds.items {
        for (ends, item) in damage.iter_mut().zip(item.damage) {
            ends[0] += item[0];
            ends[1] += item[1];
        }
        for (value, item) in resistance.iter_mut().zip(item.resistance) {
            *value += item;
        }
        attack = raised(attack, item.attack * ATTACK[2]);
        defence = raised(defence, item.defence * DEFENCE[2]);
        for bonus in item.bonuses.iter().filter(|bonus| bonus.holds(base.id)) {
            let value = bonus.value(grown);
            let kind = |first: u32| usize::try_from((bonus.effect & 0xFFFF) - first).ok();
            match bonus.effect & 0xFFFF {
                801..=804 if bonus.condition & 0x8000 != 0 => {
                    if let Some(kind) = kind(801) {
                        resistance[kind] += value;
                    }
                }
                801..=804 => {
                    if let Some(kind) = kind(801) {
                        damage[kind][0] += value;
                        damage[kind][1] += value;
                    }
                }
                805..=808 if bonus.condition & 0xE000 == 0 => {
                    if let Some(kind) = kind(805) {
                        item_multipliers[kind] += value / 100.0;
                    }
                }
                809 => attack_times = raised(attack_times, value),
                810 => defence_times = raised(defence_times, value),
                811 => speed_bonus += value,
                // The pace, added and truncated (`0x57A8C6`).
                812 => pace = (pace + value).trunc(),
                effect if let Some(attribute) = attribute_of(effect) => {
                    attribute_bonuses[attribute] += value.trunc();
                }
                _ => {}
            }
        }
    }
    // The items' attributes, held at 0 and above, and the attributes' resistances by them.
    let a: [f32; 6] = std::array::from_fn(|i| (a[i] + attribute_bonuses[i]).max(0.0));
    for (value, of) in resistance.iter_mut().zip([
        (a[3] + a[1]) / rules.resistances[0],
        a[2] / rules.resistances[1],
        a[4] / rules.resistances[2],
        (a[5] + a[1]) / rules.resistances[3],
    ]) {
        *value += of;
    }
    // Unarmed, or with a weapon whose values are not made, the default.
    let made = holds.items.iter().any(|item| item.damage[0][1] > 0.0);
    if damage[0][1] == 0.0 && (!armed || !made) {
        damage[0][1] += 0.15 * (2.0 * a[0] + a[2]) + l / 2.0;
        damage[0][0] += 0.175 * (a[0] + a[2]) + l / 2.0;
    }
    // The effects (`0x5796A0`, #651), after the items, before the skills. The attack speed is a
    // whole number of the stats by then (`+0x3C`).
    let mut speed = attack_speed(base.id) + speed_bonus;
    let mut speeds_changed = false;
    for effect in effects {
        let kind = |first: u32| usize::try_from(effect.id.checked_sub(first)?).ok();
        match effect.id {
            effects::ATTACK => attack_times += effect.value,
            effects::DEFENCE => {
                defence_times += effect.value;
                if defence_times < LEAST_DEFENCE_TIMES {
                    defence_times = LEAST_DEFENCE_TIMES;
                }
            }
            3..=6 => {
                if let Some(kind) = kind(effects::DAMAGE) {
                    item_multipliers[kind] += effect.value;
                }
            }
            effects::DAMAGE_OF => {
                if let Some(times) = usize::try_from(effect.whole)
                    .ok()
                    .and_then(|kind| item_multipliers.get_mut(kind))
                {
                    *times += effect.value;
                }
            }
            7..=10 => {
                if let Some(kind) = kind(effects::RESISTANCE) {
                    resistance[kind] += effect.value;
                }
            }
            effects::SPEEDS | effects::QUICKENS => {
                speed = quickened(speed.trunc(), f64::from(effect.value));
                pace = quickened(pace, f64::from(effect.value));
                speeds_changed = true;
            }
            effects::SLOWS => {
                let whole = effect.whole as f32;
                speed = speed.trunc() - whole;
                pace -= whole;
                speeds_changed = true;
            }
            effects::SHIELDS if effect.value > f32::EPSILON => {
                defence_times = stored(100.0 / f64::from(effect.value) * f64::from(defence_times));
            }
            effects::FIRE => {
                let whole = effect.whole as f32;
                damage[1][0] += whole;
                damage[1][1] += whole;
            }
            _ => {}
        }
    }
    if !speeds_changed {
        speed = attack_speed(base.id) + speed_bonus;
    }
    if let Some(turns) = effects.iter().find(|effect| effect.id == effects::TURNS) {
        turn_damage(&mut damage, turns);
    }
    // The skills' parts of the multipliers.
    let curves = &rules.curves;
    let mut damage_skill = 0.0;
    let mut resistance_skill = [0.0; KINDS];
    let mut fist_speed = 0.0;
    for &(skill, at) in skills {
        match skill {
            2 => damage_skill += curve(curves.damage, at) / 100.0,
            8 => {
                attack_times = raised(attack_times, curve(curves.attack, at));
                defence_times = raised(defence_times, curve(curves.defence, at));
            }
            9 => {
                let parry = if holds.shield {
                    curves.parry_shield
                } else {
                    curves.parry
                };
                defence_times = raised(defence_times, curve(parry, at));
            }
            11 => {
                for (value, triple) in resistance_skill.iter_mut().zip(curves.resistances) {
                    *value += curve(triple, at) / 100.0;
                }
                // And the pace by a fifth curve, truncated (`0x5681E1`).
                pace = (pace + curve(curves.pace, at)).trunc();
            }
            // Holding nothing (`0x546F80` of class 0).
            24 if !armed => {
                attack_times = raised(attack_times, curve(curves.fist, at));
                fist_speed += curve(curves.fist_speed, at);
            }
            _ => {}
        }
    }
    // The physical part by `b2` rather than `b0` with a ranged weapon (#492).
    let physical = if holds.ranged { b[2] } else { b[0] };
    let damage_parts = [physical, b[0] + b[2], b[4], b[1] + b[5]];
    let [dmg, res] = rules.balance;
    for (kind, ends) in damage.iter_mut().enumerate() {
        let factor = 1.0
            + damage_skill
            + item_multipliers[kind]
            + f * damage_parts[kind] / 10.0 / rules.damage_divisors[kind];
        *ends = ends.map(|end| end.max(0.0) * factor.max(0.0) * dmg);
    }
    for (kind, value) in resistance.iter_mut().enumerate() {
        let factor =
            1.0 + resistance_skill[kind] + f * b[1] / 10.0 / rules.resistance_divisors[kind];
        *value = value.max(0.0) * factor.max(0.0) * res;
    }
    // The attributes' parts after the items' (`0x5796A0`), each held at 0 and above.
    let part =
        |[t0, t1]: [f32; 2]| f64::from(a[0]) * f64::from(t0) + f64::from(a[2]) * f64::from(t1);
    let attack = raised(attack, stored(part([ATTACK[0], ATTACK[1]]))).max(0.0);
    let defence = raised(defence, stored(part([DEFENCE[0], DEFENCE[1]]))).max(0.0);
    // The attack record (`0x568D70`) truncates the product of the two floats, exact at 53 bits.
    let multiplied = |value: f32, times: f32| {
        let times = times.max(0.0);
        if times == 1.0 {
            value
        } else {
            to_f32(whole_f64(f64::from(value) * f64::from(times)))
        }
    };
    // The night form's speeds, after the skills (`0x57CA23`).
    let slows = if base.night { NIGHT_SLOWS } else { 0.0 };
    Combat {
        attack: multiplied(attack, attack_times),
        defence: multiplied(defence, defence_times),
        damage,
        resistance,
        damping: base.damping.map(|by_difficulty| by_difficulty[0]),
        speed: ((speed + fist_speed).trunc() - slows).clamp(100.0, 220.0),
        pace: pace - slows,
    }
}

/// The attribute that an item's bonus of `effect` (the low 16 bits of its effect) raises
/// (`0x5796A0`): 815 to 820 the attributes `a0`, `a2`, `a1`, `a3`, `a4`, `a5` in turn.
fn attribute_of(effect: u32) -> Option<usize> {
    let at = usize::try_from((effect & 0xFFFF).checked_sub(815)?).ok()?;
    [0, 2, 1, 3, 4, 5].get(at).copied()
}

/// The speeds that the arts of a creature of `base` at level `level` regenerate at, holding
/// `holds` (`0x5796A0`, [`arts::Speeds::of`]): its items' bonuses 813 and 814 (`0x32D`,
/// `0x32E`) and its attributes `MagicRegen` (4) and `PhysicalRegen` (3), grown by its level and
/// raised by its items' (as [`combat`]'s, held at 0 and above); no effect `0xB` yet.
pub fn arts_speeds(base: &Base, level: u32, holds: &Holds) -> arts::Speeds {
    arts_speeds_with(base, level, holds, &[])
}

/// [`arts_speeds`] with the creature's effects `effects` (#651): each effect 11 adds its value to
/// both speeds before the attributes' factors.
pub fn arts_speeds_with(
    base: &Base,
    level: u32,
    holds: &Holds,
    effects: &[Effect],
) -> arts::Speeds {
    let level = level.max(1);
    let grown = base
        .attributes
        .map(|b| u32::from(b) + u32::from(b) * (level - 1) / 10);
    let mut bonuses = [0.0_f32; 2];
    let mut attribute_bonuses = [0.0_f32; 6];
    for item in &holds.items {
        for bonus in item.bonuses.iter().filter(|bonus| bonus.holds(base.id)) {
            let value = bonus.value(grown);
            match bonus.effect & 0xFFFF {
                813 => bonuses[0] += value * 0.01,
                814 => bonuses[1] += value * 0.01,
                effect if let Some(attribute) = attribute_of(effect) => {
                    attribute_bonuses[attribute] += value.trunc();
                }
                _ => {}
            }
        }
    }
    let a: [f32; 6] = std::array::from_fn(|i| (to_f32(grown[i]) + attribute_bonuses[i]).max(0.0));
    let effect = effects
        .iter()
        .filter(|e| e.id == effects::ARTS_SPEED)
        .map(|e| e.value)
        .sum();
    arts::Speeds::of(bonuses, effect, a[4], a[3], base.id)
}

/// What a strike's resolution (`0x56B130`) takes besides its two sides' values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blow {
    /// The strike's factor (the event's `+0x1C`, [`scaled`]): its share of a series, an art's.
    pub share: f32,
    /// The exponent of its damage against the resistances ([`exponent`]).
    pub k: f32,
    /// The factor of each kind's rolled damage: bronze's `ProzDmg` for a creature striking the
    /// hero, else 1 ([`weighed`], #620).
    pub rolled: f32,
    /// The factor of the sum by the attacker's level ([`weapons_factor`]).
    pub weapons: f64,
}

impl Default for Blow {
    /// A plain strike: every factor 1.
    fn default() -> Self {
        Self {
            share: 1.0,
            k: 1.0,
            rolled: 1.0,
            weapons: 1.0,
        }
    }
}

/// The values of a strike between `attacker` and `defender` on bronze (`0x56B130`): a creature
/// striking the hero has its attack times `ProzAW` and its rolled damage times `ProzDmg` (each
/// kind's, as the attack record rolls it, [`roll_damage`], #620), and the exponent of the damage
/// `k = −log₂(1 − p)`, `p` half of 1 plus a hundredth of each level it has above the hero's; the
/// hero striking a creature meets its defence times `ProzAW` and its resistances times
/// `ProzRes`; between creatures (#630), as they are, with the exponent of the attacker's levels
/// above its defender's, as the resolution takes it for any attacker that is no player's (an id
/// above 16). `hero` tells which side is the hero's: the
/// attacker's, the defender's or neither. The blow's share and factor of the levels are 1, which
/// the strike sets.
pub fn weighed(
    mut attacker: Combat,
    mut defender: Combat,
    hero: Option<Side>,
    levels: (u32, u32),
    rules: &Rules,
) -> (Combat, Combat, Blow) {
    let [aw, dmg, res] = rules.difficulty;
    let mut blow = Blow::default();
    match hero {
        Some(Side::Defender) => {
            attacker.attack *= aw;
            blow.rolled = dmg;
            blow.k = exponent(levels.0.saturating_sub(levels.1));
        }
        Some(Side::Attacker) => {
            defender.defence *= aw;
            for value in &mut defender.resistance {
                *value *= res;
            }
        }
        None => blow.k = exponent(levels.0.saturating_sub(levels.1)),
    }
    (attacker, defender, blow)
}

/// The exponent `k` of a strike's damage against the resistances (`0x56B130`) of an attacker
/// `above` levels above its defender: `−log₂(1 − p)`, `p` half of 1 plus a hundredth of each
/// level, which the weight `0x43EEE0` holds at 0.99 only from 1 up: 1 for none above, about 7.64
/// for 99 above, where `p` stays 0.995, and 6.64 for 98 and for 100 and more.
pub fn exponent(above: u32) -> f32 {
    let p = 0.5 * (1.0 + to_f32(above) * 0.01);
    let p = if p >= 1.0 { 0.99 } else { p.max(0.0) };
    -(1.0 - p).log2()
}

/// The ratio `x/y` of the weight `0x43EEE0`, 10,000 for a divisor `y` within ±0.001, the bounds
/// included: the double 0.001, so that the float 0.001, a little above it, divides.
fn ratio(x: f32, y: f32) -> f32 {
    if f64::from(y).abs() <= 0.001 {
        10_000.0
    } else {
        x / y
    }
}

/// A strike's hit chance, an attack `attack` against a defence `defence` (`0x43EEE0` with the
/// exponent 1): `1 − 1/(1 + A/D)`, that is `A/(A + D)`; against a defence within ±0.001, 0.9999
/// whatever the attack, so that the roll of 1,000 misses.
pub fn hit_chance(attack: f32, defence: f32) -> f32 {
    1.0 - 1.0 / (1.0 + ratio(attack, defence))
}

/// The part of a kind's damage `damage` that its resistance `resistance` lets through with the
/// exponent `k` (`0x43EEE0`): `d·(1 − 1/(1 + d/r)^k)`, `d²/(d + r)` for the exponent 1; against a
/// resistance within ±0.001, `d·(1 − 10,001^−k)`, 0.9999 of it for `k` 1; none of none.
pub fn dealt(damage: f32, resistance: f32, k: f32) -> f32 {
    if damage <= 0.0 {
        return 0.0;
    }
    damage * (1.0 - 1.0 / (1.0 + ratio(damage, resistance)).powf(k))
}

/// The factor of the attack of a strike with a ranged weapon (the kinds 7, 12, 13 and 14,
/// `0x427ED0`) at `distance` between the attacker and its target (`0x56B449`): 1 up to
/// `FernAWMin` (250), falling linearly toward `1 − FernAWProz` (0.75) at `FernAWMax` (1,000), and
/// `FernAWProz` (0.25) from there, a jump.
pub fn range_factor(distance: f32, rules: &Rules) -> f32 {
    let [least, most, share] = rules.range;
    if distance <= least {
        1.0
    } else if distance >= most {
        share
    } else {
        1.0 - (distance - least) * share / (most - least)
    }
}

/// The factor of the sum of a strike's damage by its attacker's level, `level`
/// (`0x56A890(attacker, defender)`, which the strike's resolution `0x56B130` calls at
/// `0x56D05C`; docs/re/fight.md, "A strike's resolution"): for a creature striking the hero
/// (`hero` the defender's side; in the original an attacker of an id above 16 and a defender of
/// 16 or below) that is neither a boss (`+0x1F4` bit `0x80000`) nor a champion (`+0xB0` bit
/// `0x400`, a champion's levelling), `WaffenAll`'s factor of its band of `AllOverLv`'s levels
/// ([`Bands::factor`]); else 1. Its twin `0x56A800` takes `MagieAll` for two spells
/// (`0x5ABD80`, `0x5B2600`), which the creatures do not cast yet.
pub fn weapons_factor(hero: Option<Side>, level: u32, rank: Rank, rules: &Rules) -> f64 {
    match (hero, rank) {
        (Some(Side::Defender), Rank::Common) => rules.bands.factor(level),
        _ => 1.0,
    }
}

/// A side of a strike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Attacker,
    Defender,
}

/// Each kind's damage of a strike (`0x568D70`, as the attack record rolls it; #620): while the
/// physical most is below 100 the ends times 100 on the x87 unit, truncated toward 0, a whole
/// number `n` between them (a least below 0 taken as 0, a most below 0 as 1) and the damage `n`
/// times the float 0.01 stored as a float; below 1,000 likewise in tenths, `n` times the float
/// 0.1; below 10,000 in units; from 10,000 the least plus a roll of 0 to 100 hundredths of the
/// span, `(most − least)·r·0.01 + least`. 15 physical when every kind's is below 1; then each
/// times `factor` (`ProzDmg` for a creature striking the hero, `0x56B130`, after the record) at
/// 53 bits, stored as a float.
fn roll_damage(damage: &[[f32; 2]; KINDS], factor: f32, rng: &mut Rng) -> [f32; KINDS] {
    let step = match damage[0][1] {
        most if most < 100.0 => Some((100.0, HUNDREDTH)),
        most if most < 1000.0 => Some((10.0, f64::from(0.1_f32))),
        most if most < 10_000.0 => Some((1.0, 1.0)),
        _ => None,
    };
    let mut rolled = damage.map(|[least, most]| {
        let (least, most) = (f64::from(least), f64::from(most));
        let Some((step, unit)) = step else {
            let r = f64::from(rng.below(101));
            return stored((most - least) * r * HUNDREDTH + least);
        };
        let low = truncated(least * step).max(0);
        let high = truncated(most * step);
        let high = if high < 0 { 1 } else { high };
        let (low, high) = (low.min(high), low.max(high));
        let n = if low == high {
            low
        } else {
            let span = u32::try_from(high - low + 1).unwrap_or(u32::MAX);
            low + i64::from(rng.below(span))
        };
        stored(f64::from(i32::try_from(n).unwrap_or(i32::MAX)) * unit)
    });
    if rolled.iter().all(|&d| d < 1.0) {
        rolled[0] = 15.0;
    }
    if factor != 1.0 {
        rolled = rolled.map(|d| stored(f64::from(d) * f64::from(factor)));
    }
    rolled
}

/// A double truncated toward 0, as `__ftol` takes it, held within the integers of 32 bits.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the original truncates the end of a damage to an integer of 32 bits"
)]
fn truncated(value: f64) -> i64 {
    value
        .trunc()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i64
}

/// The damage that a strike of `attacker` deals `defender`, the strike's factor `share`, or
/// none when it misses (`0x56B130`): the chance `A/(A + D)` (`0x43EEE0`) against a roll of 0 to
/// 1,000 over 1,000; each kind's damage `d` rolled, then `d·(1 − 1/(1 + d/r)^k)` against the
/// kind's resistance `r`, `d²/(d + r)` for the exponent `k` of 1, less the defender's damping
/// of bronze; the sum, a float, times the factor of the attacker's level and the strike's factor
/// ([`taken`]), at least 1, whole. `blow` holds those factors, `k` and the rolled damage's.
pub fn strike(attacker: &Combat, defender: &Combat, blow: Blow, rng: &mut Rng) -> Option<u32> {
    let chance = hit_chance(attacker.attack, defender.defence);
    if chance < to_f32(rng.below(1001)) / 1000.0 {
        return None;
    }
    Some(damage(attacker, defender, blow, rng))
}

/// A strike of a combat art of the second table at its level in play `level` (`0x56B130`: a
/// hit's kind other than 0 by a creature in its action of the arts, or its action 5), the blow's
/// share its factor (`arts::factor`): its chance by the art's attack ([`art_attack`]) against a
/// roll of 0 to 1,000 over 1,000, its damage as a plain strike's ([`strike`]).
pub fn art_strike(
    attacker: &Combat,
    defender: &Combat,
    blow: Blow,
    level: u32,
    rng: &mut Rng,
) -> Option<u32> {
    let chance = hit_chance(art_attack(attacker.attack, level), defender.defence);
    if chance < to_f32(rng.below(1001)) / 1000.0 {
        return None;
    }
    Some(damage(attacker, defender, blow, rng))
}

/// The attack that a strike of a combat art of the second table takes into its hit's chance
/// (`0x56B130`): `A · (1.3 + 0.125·L)`, `L` the art's level in play (the entry's level and bonus,
/// `0x580A10`), at 53 bits and stored as a float.
pub fn art_attack(attack: f32, level: u32) -> f32 {
    stored((f64::from(level) * 0.125 + 1.3) * f64::from(attack))
}

/// A strike's damage once it lands: each kind's rolled and let through by its resistance, less
/// the damping, summed, and what the sum takes off the health ([`taken`]).
fn damage(attacker: &Combat, defender: &Combat, blow: Blow, rng: &mut Rng) -> u32 {
    let total: f32 = roll_damage(&attacker.damage, blow.rolled, rng)
        .iter()
        .zip(defender.resistance)
        .zip(defender.damping)
        .map(|((&d, r), damping)| {
            dealt(d, r, blow.k) * f32::from(100_u8.saturating_sub(damping)) / 100.0
        })
        .sum();
    taken(total, blow.share, blow.weapons)
}

/// What the sum `sum` of a strike's kinds of damage takes off its defender's health (`0x56B130`):
/// times the factor of the attacker's level `weapons` ([`weapons_factor`], at `0x56D05C`), the
/// product on the x87 unit stored as a float, times the strike's factor `factor` ([`scaled`]),
/// at least 1, whole (the skills' specials and the effects that follow are not read).
fn taken(sum: f32, factor: f32, weapons: f64) -> u32 {
    let sum = stored(weapons * f64::from(sum));
    whole(scaled(sum, factor).max(1.0))
}

/// A strike's damage `total` times its factor `factor` (the event's `+0x1C`, `0x56B130`) where the
/// factor lies between 0 and 500 and is not 1, else as it is: a factor of 0 scales nothing.
pub fn scaled(total: f32, factor: f32) -> f32 {
    if factor > 0.0 && factor < 500.0 && factor != 1.0 {
        total * factor
    } else {
        total
    }
}

/// The highest level that a fill's run gives its creatures before the level's setter holds them
/// (`0x47D34B`).
const FILLS_MOST: u32 = 300;

/// The level of a fill's run, which all the creatures that it makes share (`FillSector`,
/// `0x47C610`, from `0x47D187`; docs/re/spawning.md, "`FillSector`"; #582): the hero's, `hero`,
/// raised to the lowest of its sector's levels, `values`, when below it (to their highest where
/// the lowest is not below it), but not held at the highest; moved once by its band, `r % 2` below
/// 6, then two rolls averaged, `(r % 4 + r % 4)/2 − 1` below 30, `(r % 6 + r % 6)/2 − 2` below 90
/// and `(r % 10 + r % 10)/2 − 3` below 250, none from 250; then raised by the time played,
/// `minutes`, through the difficulty's scaling of the levels (`0x552230`, as `CreateNPC`'s), at
/// most 300. Each creature takes it through [`filled_level`].
pub fn fill_level(values: &SpawnValues, hero: u32, minutes: u32, dice: &mut impl Dice) -> u32 {
    let level = if hero >= values.lowest {
        hero
    } else {
        values.lowest.min(values.highest)
    };
    let moved = match level {
        0..6 => level + dice.below(2),
        6..30 => (level + averaged(4, dice)).saturating_sub(1),
        30..90 => (level + averaged(6, dice)).saturating_sub(2),
        90..250 => (level + averaged(10, dice)).saturating_sub(3),
        _ => level,
    };
    played_level(moved, minutes).min(FILLS_MOST)
}

/// Two rolls below `n` averaged, whole (`(rand() % n + rand() % n)/2`).
fn averaged(n: u32, dice: &mut impl Dice) -> u32 {
    let first = dice.below(n);
    (first + dice.below(n)) / 2
}

/// The level that a fill's creature of type `creature`, a horse or not, takes from its run's,
/// `level` ([`fill_level`]; `0x47E7C0`): one or two more on a dungeon's sector, `dungeon`, by a
/// roll of its own (`rand() % 2`, #570), then as the level's setter holds it as the creature
/// maker sets it ([`held_level`]), at most the highest of its sector's levels, `values`. A boss's
/// or a champion's is its rank's instead ([`Rank::level`]), which rolls nothing here.
pub fn filled_level(
    level: u32,
    dungeon: bool,
    values: &SpawnValues,
    (creature, horse): (u32, bool),
    rules: &Rules,
    dice: &mut impl Dice,
) -> u32 {
    let level = if dungeon {
        level + 1 + dice.below(2)
    } else {
        level
    };
    held_level(level, Some(values), creature, horse, rules)
}

/// What raises the level of a creature that `CreateNPC` makes once its sector's levels hold it
/// (`0x482510`, at `0x484714`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Raise {
    /// A horse (`Creature.pak` class 4, `0x4266F0`): by an eighth of the level of the hero's
    /// skill 22 (`0x582450`; **hypothesis**: riding).
    Horse { riding: u32 },
    /// Any other creature: by the time played, in whole minutes, through the difficulty's
    /// scaling of the levels (`0x552230`).
    Played { minutes: u32 },
}

/// The level that `CreateNPC` gives a creature of type `creature` of a record without a level of
/// its own (`0x482510`, from `0x48459F`; docs/re/spawning.md, "`CreateNPC`'s level"): the hero's,
/// `hero`, held within the levels of the creature's sector, `values`, where it has both: their
/// lowest below them, their highest above, else the hero's or one more by a roll (`rand() % 2`);
/// then raised by `raise`, and held by the level's setter ([`held_level`], at `0x48475B`): a
/// horse's at most 250, any other's at most its sector's highest.
pub fn scripted_level(
    values: Option<&SpawnValues>,
    hero: u32,
    raise: Raise,
    creature: u32,
    rules: &Rules,
    dice: &mut impl Dice,
) -> u32 {
    let level = match values.filter(|values| values.lowest > 0 && values.highest > 0) {
        Some(values) if hero < values.lowest => values.lowest,
        Some(values) if hero > values.highest => values.highest,
        Some(_) => hero + dice.below(2),
        None => hero,
    };
    match raise {
        Raise::Horse { riding } => held_level(level + riding / 8, values, creature, true, rules),
        Raise::Played { minutes } => {
            held_level(played_level(level, minutes), values, creature, false, rules)
        }
    }
}

/// The level tag `0x1F` of a `CreateNPC` record that makes its creature a boss of the marker
/// level 1 (`0x4843C9`).
pub const BOSS_TAG: u32 = 1000;

/// The type whose level the level's setter holds at 300 wherever it stands (`0x565473`).
const AT_MOST_300: u32 = 701;

/// The highest level the level's setter leaves a horse (`0x56550C`).
const HORSES_MOST: u32 = 250;

/// The level that the level's setter (`0x564D60`, docs/re/spawning.md, "The level's setter")
/// leaves a creature of type `creature`, a horse or not, set to `level` without its scaling, as
/// `CreateNPC` and the creature maker set it (`0x48475B`, `0x549500`), and with it on bronze,
/// where the scaling keeps it: a horse's at most 250 (`0x4266F0`); any other's at most the
/// highest level of its sector, `values`, plus bronze's `LevelKap` (0), or that addition alone
/// where the sector has no highest, 300 for the type 701; at least 1, where the original leaves 0
/// in a sector without levels (docs/decisions/2026-10-23-level-zero.md). A boss's or a champion's
/// hold, raised by `LevelUpBoss` or `LevelUpChamp` and none for a big type, is [`Rank::level`]'s:
/// the setter holds a creature before it is ranked.
pub fn held_level(
    level: u32,
    values: Option<&SpawnValues>,
    creature: u32,
    horse: bool,
    rules: &Rules,
) -> u32 {
    if horse {
        return level.min(HORSES_MOST);
    }
    level.min(most_level(values, creature, rules)).max(1)
}

/// The highest level that the level's setter leaves a creature of type `creature` that is no
/// horse, of a sector of `values`, before its rank's raise ([`held_level`]).
fn most_level(values: Option<&SpawnValues>, creature: u32, rules: &Rules) -> u32 {
    if creature == AT_MOST_300 {
        300
    } else {
        values.map_or(0, |values| values.highest) + rules.ranks.cap
    }
}

/// The level that `CreateNPC` gives a creature of type `creature` of a record with the level tag
/// `0x1F`, `tag` (`0x482510`, from `0x4843C2`; docs/re/spawning.md, "`CreateNPC`'s level"), or
/// none for 0, a record without one, whose creature takes [`scripted_level`]'s. For
/// [`BOSS_TAG`], a boss's marker level 1 ([`Rank::of_tag`]), which [`Rank::level`] then levels
/// as a fill's boss. For any other, that level as the level's setter takes it, without the
/// hero's, the sector's lowest or the time played: through the difficulty's scaling of the
/// levels, which keeps it on bronze, then held ([`held_level`]).
pub fn tagged_level(
    tag: u32,
    values: Option<&SpawnValues>,
    creature: u32,
    horse: bool,
    rules: &Rules,
) -> Option<u32> {
    match tag {
        0 => None,
        BOSS_TAG => Some(1),
        level => Some(held_level(level, values, creature, horse, rules)),
    }
}

/// A level `level` scaled by the difficulty and raised by the time played (`0x552230`), bronze's:
/// `⌊L·(1 + x/100) + 0.5⌋`, at least 1, with `x` the tenth of the curve of the minutes `m`
/// (`0x565D70`) `100·(1 − 1/(1 + (m − 1)/120))` from a minute on, truncated, 0 to 9: one more
/// per cent of the level from 15 minutes on, five from 121 minutes. A hero wearing six
/// or more of one count of `+0x4F8` (its byte `0x1C`; **hypothesis**: the pieces of a set) adds
/// 1 to 2.34 to `x` by the difficulty; not followed. The difficulty's own per cent and offset
/// (`OffLevelProz`, `BaseOffLevel`) are 0 on bronze.
fn played_level(level: u32, minutes: u32) -> u32 {
    let curve = if minutes >= 1 {
        100.0 * (1.0 - 1.0 / (1.0 + f64::from(minutes - 1) / 120.0))
    } else {
        0.0
    };
    let x = (curve * f64::from(0.1_f32)).trunc();
    let scaled = f64::from(level) * (1.0 + x * f64::from(0.01_f32)) + 0.5;
    whole_f64(scaled.max(1.0))
}

/// The most health of a creature of `Creature.pak`'s base bytes `base` and level `level`
/// (`0x5658F0`): with `a` and `b` the bytes 0 and 3, `S = a + b + ⌊(L − 1)a/10⌋ + ⌊(L − 1)b/10⌋`
/// and `g = 1.5 + G · max(0, 60 − a − b)`, `⌊S^g · (a0 + a3)/S · 10/31⌋`, 100 for 0, times the
/// level's factor of a creature not a hero and the difficulty's (`rules`); the attributes `a0`
/// and `a3` are `S`'s two terms, without extra points or items' bonuses, so that the most is
/// `⌊S^g · 10/31⌋` (#408); the 100 for 0 comes before the factors, so a most that they take
/// below 1 is 0 (#417), which no creature of `Creature.pak` reaches, the least of its attributes
/// 0 and 3 together being 7 (type 135); raised by its skill 10 before the factors (`skills`,
/// [`hero_health`], #498); a boss's or a champion's by its rank's factor `rank` ([`Rank::health`])
/// after the skill and before the 100 for 0 and the level's factor, each taken whole (#579). No
/// items yet (docs/decisions/2026-10-20-health.md).
pub fn most_health(
    base: [u8; 6],
    level: u32,
    skills: &[(u8, u32)],
    rank: f32,
    rules: &Rules,
) -> u32 {
    let level_f32 = f32::from(u16::try_from(level).unwrap_or(u16::MAX));
    health_of(
        base,
        level,
        health_raise(skills, rules),
        rank,
        f64::from(rules.level_factor(level_f32) * rules.factor),
    )
}

/// The most health of a hero of `Creature.pak`'s base bytes `base` and level `level`
/// (`0x5658F0`): as a creature's, without the factors of a creature not a hero, raised by its
/// skill 10's per cent of the curve `HP…VW` (`skills`, `0x566070`) and taken whole (#492).
pub fn hero_health(base: [u8; 6], level: u32, skills: &[(u8, u32)], rules: &Rules) -> u32 {
    health_of(base, level, health_raise(skills, rules), 1.0, 1.0)
}

/// The skill 10's per cent of the curve `HP…VW` for a most health (`0x5658F0`): of its first
/// slot, at its level, as the x87 unit computes it at 53 bits and keeps it, not stored as a
/// float ([`per_cent`], #598); none without the skill.
fn health_raise(skills: &[(u8, u32)], rules: &Rules) -> Option<f64> {
    skills
        .iter()
        .find(|&&(skill, _)| skill == 10)
        .map(|&(_, at)| per_cent(rules.curves.health, at))
}

/// The factor of the health's exponent by the base (the float at `0x891298`, a little under 0.004).
const HEALTH_EXPONENT: f32 = 0.003_996_323_3;

/// The most health of `base` at `level`, times `rank` and then `factor` (`0x5658F0`), in double
/// precision as the client's FPU computes it: the exponent's factor a float, the rest doubles.
fn health_of(base: [u8; 6], level: u32, raise: Option<f64>, rank: f32, factor: f64) -> u32 {
    let (a, b) = (f64::from(base[0]), f64::from(base[3]));
    let l = f64::from(level.max(1) - 1);
    let (a0, a3) = (a + (l * a / 10.0).floor(), b + (l * b / 10.0).floor());
    let s = a0 + a3;
    let g = 1.5 + f64::from(HEALTH_EXPONENT) * (60.0 - a - b).max(0.0);
    // A base without the attributes 0 and 3 gives nothing, and so 100.
    let most = if s > 0.0 {
        (s.powf(g) * ((a0 + a3) / s) * (10.0 / 31.0)).trunc()
    } else {
        0.0
    };
    // The skill 10's raise `m·v·0.01 + m`, 0.01 the float a little below it, at 53 bits and taken
    // whole, before the 100 for 0 and the factors (#598).
    let most = match raise {
        Some(per_cent) => (most * per_cent * HUNDREDTH + most).trunc(),
        None => most,
    };
    // A boss's or a champion's factor, taken whole, before the 100 for 0 and the level's factor.
    let most = if rank == 1.0 {
        most
    } else {
        (most * f64::from(rank)).trunc()
    };
    let most = if most <= 0.0 { 100.0 } else { most };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a health, whole and not negative, far below the integers' range"
    )]
    let most = (most * factor).floor() as u32;
    most
}

#[cfg(test)]
mod tests {
    use opensacred_data::tables::{BalanceValue, BalanceVariable};

    use super::*;

    #[test]
    fn a_type_without_a_record_is_made_as_the_originals_crab() {
        let crab = recordless(163);
        assert_eq!((crab.class, crab.flags, crab.speed), (0, 0, [100, 200]));
        let base = Base::of(&crab);
        assert_eq!(base.attributes, [25, 15, 20, 20, 10, 5]);
        assert_eq!(base.skills, [0; 8]);
        // Seen in the original (#603): at level 1, without a skill, `A` 22.5 and `D` 21 times 1,
        // its attack speed 140, its physical damage 8.375 to 11 and its resistances 1.59, 1.18,
        // 0.59 and 0.8 before their multipliers, 1.3395, 1.1600, 1.1600, 1.1400 and 1.1723.
        let values = combat(&base, 1, Holds::default(), &[], &Rules::default());
        assert_eq!(
            (values.attack, values.defence, values.speed),
            (22.5, 21.0, 140.0)
        );
        let near = |a: f32, b: f32| (a - b).abs() < 0.001;
        assert!(near(values.damage[0][0], 8.375 * 1.339_452_2), "{values:?}");
        assert!(near(values.damage[0][1], 11.0 * 1.339_452_2), "{values:?}");
        assert!(
            near(values.resistance[0], 1.590_909_1 * 1.160_027_5),
            "{values:?}"
        );
        assert!(near(values.resistance[3], 0.8 * 1.172_337_3), "{values:?}");
    }

    #[test]
    fn the_most_health_grows_with_the_level_by_the_originals_formula() {
        let rules = Rules::default();
        // b0 20, b3 15: S = 35, g = 1.5 + G · 25, about 1.6; ⌊35^g · 10/31⌋.
        let base = [20, 0, 10, 15, 0, 0];
        let g = 1.5 + f64::from(HEALTH_EXPONENT) * 25.0;
        let expected = (35.0_f64.powf(g) * 10.0 / 31.0).floor();
        assert_eq!(f64::from(most_health(base, 1, &[], 1.0, &rules)), expected);
        assert!(most_health(base, 10, &[], 1.0, &rules) > most_health(base, 1, &[], 1.0, &rules));
        // The wolf's base, as a hero's type, at the levels 1, 10 and 100, as the original's
        // `0x5658F0` gives them (the oracle's vectors, #363, #408).
        let wolf = [8, 10, 18, 13, 0, 1];
        assert_eq!(hero_health(wolf, 1, &[], &Rules::default()), 49);
        assert_eq!(hero_health(wolf, 10, &[], &Rules::default()), 139);
        assert_eq!(hero_health(wolf, 100, &[], &Rules::default()), 2588);
        // A base that gives nothing gives 100.
        assert_eq!(most_health([0; 6], 1, &[], 1.0, &rules), 100);
        // A most of 1 that a level's factor of 0.75 takes below 1 is 0, as the original's (#417).
        let weak = Rules {
            points: vec![(1.0, 0.75), (4.0, 0.8)],
            ..Rules::default()
        };
        assert_eq!(hero_health([1; 6], 1, &[], &Rules::default()), 1);
        // The skill 10 at level 1 raises it by `HPoffVW`, 4 %, taken whole: the Dwarf's 133 to
        // the original's 138 (#492).
        let dwarf = [26, 18, 25, 24, 0, 8];
        let rules = Rules::default();
        assert_eq!(hero_health(dwarf, 1, &[], &rules), 133);
        assert_eq!(hero_health(dwarf, 1, &[(10, 1), (3, 1)], &rules), 138);
        // An NPC's too, before the factors of a creature not a hero (#498): a horse's skill 10 of
        // level 3 at level 9 raises its most by 13.7 %.
        let horse = [30, 0, 20, 30, 0, 0];
        let (with, without) = (
            most_health(horse, 9, &[(10, 3)], 1.0, &rules),
            most_health(horse, 9, &[], 1.0, &rules),
        );
        let raise = 1.0 + f64::from(curve(rules.curves.health, 3)) / 100.0;
        assert!(near(curve(rules.curves.health, 3), 13.692));
        assert!(f64::from(with) > f64::from(without) * (raise - 0.01));
        assert!(f64::from(with) < f64::from(without) * (raise + 0.01));
        assert_eq!(most_health([1; 6], 1, &[], 1.0, &weak), 0);
    }

    #[test]
    fn the_skill_10_raises_the_most_health_at_53_bits_by_the_float_hundredth() {
        // The type 557 (base 10 and 20) at level 82: the most 2,850, its skill 10 of level 27 a
        // per cent of 90.2105…, which raises it to exactly 5,421 in real numbers; the original
        // keeps the per cent on the x87 unit and multiplies by the float 0.01, a little below a
        // hundredth, so 5,420.99994 and 5,420, where a float's sum 1 + v/100 gave 5,421 (#598).
        let rules = Rules::default();
        let base = [10, 20, 15, 20, 0, 10];
        assert_eq!(hero_health(base, 82, &[], &rules), 2850);
        assert_eq!(hero_health(base, 82, &[(10, 27)], &rules), 5420);
        // Of its first slot of the skill 10 alone.
        assert_eq!(hero_health(base, 82, &[(10, 27), (10, 3)], &rules), 5420);
    }

    #[test]
    fn a_bosss_factor_comes_before_the_levels_each_taken_whole() {
        // The boss 577 of 55,39 at level 3 (base 19 and 10): the curve gives 98, the boss's 2.5
        // makes 245 and the level's 0.8 on bronze 196, the original's; the level's factor first
        // would give 78 and then 195 (#579).
        let rules = Rules {
            points: vec![
                (1.0, 0.75),
                (4.0, 0.8),
                (10.0, 0.9),
                (20.0, 1.0),
                (35.0, 1.1),
            ],
            ..Rules::default()
        };
        let boss = [19, 20, 20, 10, 0, 15];
        assert_eq!(most_health(boss, 3, &[], 1.0, &rules), 78);
        assert_eq!(most_health(boss, 3, &[], 2.5, &rules), 196);
        // A factor that takes the most to 0 gives 100 before the level's factor, as 0 does.
        assert_eq!(most_health([0; 6], 3, &[], 2.5, &rules), 80);
    }

    #[test]
    fn an_attack_at_range_weakens_with_the_distance() {
        let rules = Rules::default();
        assert_eq!(range_factor(0.0, &rules), 1.0);
        assert_eq!(range_factor(250.0, &rules), 1.0);
        assert!((range_factor(625.0, &rules) - 0.875).abs() < 1e-6);
        assert!((range_factor(999.0, &rules) - 0.750_333).abs() < 1e-5);
        // From 1,000 a quarter, a jump from three quarters.
        assert_eq!(range_factor(1000.0, &rules), 0.25);
        assert_eq!(range_factor(1500.0, &rules), 0.25);
    }

    #[test]
    fn the_level_factor_takes_the_level_whole_between_its_points() {
        let rules = Rules {
            points: vec![
                (1.0, 0.75),
                (4.0, 0.8),
                (10.0, 0.9),
                (20.0, 1.0),
                (35.0, 1.1),
            ],
            ..Rules::default()
        };
        // Between 1 and 4: 0.75 + L · 0.05/3, the level whole.
        assert!((rules.level_factor(1.0) - (0.75 + 0.05 / 3.0)).abs() < 1e-6);
        assert!((rules.level_factor(20.0) - (0.9 + 20.0 * 0.1 / 10.0)).abs() < 1e-6);
        // Above the last point, the last segment's line goes on: 1.0 + L · 0.1/15, as the
        // original's (the oracle's vectors, #363, #418): 1.4 at level 60, 2.44 at 216.
        assert!((rules.level_factor(40.0) - (1.0 + 40.0 * 0.1 / 15.0)).abs() < 1e-6);
        assert!((rules.level_factor(60.0) - 1.4).abs() < 1e-6);
        assert!((rules.level_factor(216.0) - 2.44).abs() < 1e-5);
        // Without two points, 1.
        assert_eq!(Rules::default().level_factor(60.0), 1.0);
    }

    /// A wolf (512) and a goblin warrior (76) of `Creature.pak`, as far as their fight goes.
    #[test]
    fn the_effects_change_the_stats_after_the_items_and_before_the_skills() {
        let rules = Rules::default();
        let wolf = a_wolf();
        let with = |effects: &[Effect], skills: &[(u8, u32)]| {
            combat_with(&wolf, 10, Holds::default(), skills, effects, &rules)
        };
        let plain = with(&[], &[]);
        let whole = |value: f64| to_f32(whole_f64(value));
        // `A`'s multiplier plus 0.25, then the skill 8's per cent on it.
        let raised_attack = with(&[Effect::new(effects::ATTACK, 0.25)], &[]);
        assert_eq!(raised_attack.attack, whole(f64::from(plain.attack) * 1.25));
        let trained = with(&[Effect::new(effects::ATTACK, 0.25)], &[(8, 1)]);
        let times = raised(1.25, curve(rules.curves.attack, 1));
        assert_eq!(
            trained.attack,
            whole(f64::from(plain.attack) * f64::from(times))
        );
        // `D`'s multiplier held at 0.01, and times 100 over a value.
        let low = with(&[Effect::new(effects::DEFENCE, -5.0)], &[]);
        assert_eq!(
            low.defence,
            whole(f64::from(plain.defence) * f64::from(0.01_f32))
        );
        let shielded = with(&[Effect::new(effects::SHIELDS, 50.0)], &[]);
        assert_eq!(shielded.defence, whole(f64::from(plain.defence) * 2.0));
        assert_eq!(with(&[Effect::new(effects::SHIELDS, 0.0)], &[]), plain);
        // A kind's damage multiplier, by its id or by the whole value of 33; the others' stay.
        let fiercer = with(&[Effect::new(4, 0.5)], &[]);
        let by_whole = with(
            &[Effect {
                whole: 1,
                ..Effect::new(effects::DAMAGE_OF, 0.5)
            }],
            &[],
        );
        assert_eq!(fiercer, by_whole);
        assert_eq!(fiercer.damage[0], plain.damage[0]);
        assert!(fiercer.damage[1][1] >= plain.damage[1][1]);
        // A resistance plus its value before its multiplier.
        let resisting = with(&[Effect::new(effects::RESISTANCE, 10.0)], &[]);
        assert!(resisting.resistance[0] > plain.resistance[0] + 10.0);
        assert_eq!(resisting.resistance[1..], plain.resistance[1..]);
        // The fire damage's ends plus the whole value: a wolf's 0 to some.
        let burning = with(
            &[Effect {
                whole: 5,
                ..Effect::new(effects::FIRE, 0.0)
            }],
            &[],
        );
        assert_eq!(plain.damage[1], [0.0, 0.0]);
        assert!(burning.damage[1][0] > 5.0 && burning.damage[1][0] == burning.damage[1][1]);
        // The attack speed and the pace times a value, truncated, held at 220; less a whole value.
        assert_eq!((plain.speed, plain.pace), (140.0, 130.0));
        let quick = with(&[Effect::new(effects::QUICKENS, 1.5)], &[]);
        assert_eq!((quick.speed, quick.pace), (210.0, 195.0));
        let quicker = with(
            &[
                Effect::new(effects::SPEEDS, 1.5),
                Effect::new(effects::QUICKENS, 1.5),
            ],
            &[],
        );
        assert_eq!((quicker.speed, quicker.pace), (220.0, 292.0));
        let slow = with(
            &[Effect {
                whole: 20,
                ..Effect::new(effects::SLOWS, 0.0)
            }],
            &[],
        );
        assert_eq!((slow.speed, slow.pace), (120.0, 110.0));
        // The ids the recomputation does not follow change nothing.
        assert_eq!(
            with(&[Effect::new(34, 9.0), Effect::new(40, 1.0)], &[]),
            plain
        );
    }

    #[test]
    fn the_effects_raise_the_regeneration_and_the_arts_speeds() {
        let rules = Rules::default();
        let base = Base {
            attributes: [0, 0, 0, 22, 0, 0],
            ..Base::default()
        };
        // 44 times 1.5, then the skill 10's per cent on it.
        let effect = [Effect::new(effects::REGENERATION, 0.5)];
        assert_eq!(regeneration_with(&base, 1, &[], &effect, &rules), 66.0);
        let skill = curve(rules.curves.regeneration, 1) / 100.0;
        assert_eq!(
            regeneration_with(&base, 1, &[(10, 1)], &effect, &rules),
            to_f32(whole(66.0 * (1.0 + skill)))
        );
        // Both arts' speeds plus 0.2 before the attributes' factors.
        let plain = arts_speeds(&base, 1, &Holds::default());
        let faster = arts_speeds_with(
            &base,
            1,
            &Holds::default(),
            &[Effect::new(effects::ARTS_SPEED, 0.2)],
        );
        assert!(
            (faster.spells - plain.spells * 1.2).abs() < 1e-5,
            "{faster:?}"
        );
        assert!((faster.weapon_arts - plain.weapon_arts * 1.2).abs() < 1e-5);
    }

    #[test]
    fn the_night_form_takes_fifteen_off_the_speeds() {
        let rules = Rules::of(None);
        let day = Base {
            skills: [0; 8],
            ..a_wolf()
        };
        let night = Base { night: true, ..day };
        let [by_day, by_night] =
            [day, night].map(|base| combat(&base, 1, Holds::default(), &[], &rules));
        assert_eq!(by_night.speed, by_day.speed - NIGHT_SLOWS);
        assert_eq!(by_night.pace, by_day.pace - NIGHT_SLOWS);
        // Before the limits: 100 at least.
        let quick = combat_with(
            &night,
            1,
            Holds::default(),
            &[],
            &[Effect {
                whole: 40,
                ..Effect::new(effects::SLOWS, 0.0)
            }],
            &rules,
        );
        assert_eq!(quick.speed, 100.0);
    }

    fn a_wolf() -> Base {
        Base {
            id: 512,
            attributes: [8, 10, 18, 13, 0, 1],
            bonus: [[1, 1], [2, 5], [0, 0], [0, 0], [0, 0], [0, 0]],
            bonus_value: [50, 30, 0, 0, 0, 0],
            skills: [2, 11, 0, 0, 0, 0, 0, 0],
            ..Base::default()
        }
    }

    fn a_goblin() -> Base {
        Base {
            id: 76,
            attributes: [15, 20, 22, 12, 0, 4],
            bonus: [[1, 1], [2, 5], [2, 6], [0, 0], [0, 0], [0, 0]],
            bonus_value: [120, 20, 20, 0, 0, 0],
            ..Base::default()
        }
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn an_npcs_attack_defence_damage_and_resistance_follow_the_originals_examples() {
        let rules = Rules::default();
        // docs/re/fight.md, "An NPC's attack, defence and damage": level 1, unarmed.
        let wolf = combat(&a_wolf(), 1, Holds::default(), &[], &rules);
        assert!(near(wolf.attack, 13.0) && near(wolf.defence, 16.0));
        assert!(near(wolf.damage[0][0], 3.33) && near(wolf.damage[0][1], 4.43));
        assert_eq!(wolf.damage[1..], [[0.0; 2]; 3]);
        assert!(near(wolf.resistance[0], 4.48));
        let goblin = combat(&a_goblin(), 1, Holds::default(), &[], &rules);
        assert!(near(goblin.attack, 18.5) && near(goblin.defence, 20.6));
        assert!(near(goblin.damage[0][0], 12.04) && near(goblin.damage[0][1], 21.67));
        assert!(near(goblin.resistance[0], 6.62));
        // Armed, the damage bonus does not hold: the default's, `0.15(2a0 + a2) + L/2` to
        // `0.175(a0 + a2) + L/2`, times the same multiplier.
        let armed = combat(
            &a_goblin(),
            1,
            Holds {
                weapon: true,
                ..Holds::default()
            },
            &[],
            &rules,
        );
        let factor = goblin.damage[0][1] / 18.0;
        assert!(near(armed.damage[0][1], (0.15 * 52.0 + 0.5) * factor));
        assert!(near(armed.damage[0][0], (0.175 * 37.0 + 0.5) * factor));
        assert!(near(armed.resistance[0], goblin.resistance[0]));
        // The level raises the attributes by a tenth each: a level-10 wolf's are 15, 19, 34, 24.
        let older = combat(&a_wolf(), 10, Holds::default(), &[], &rules);
        assert!(near(older.attack, 24.5) && near(older.defence, 30.2));
    }

    #[test]
    fn a_strike_hits_by_the_chance_of_attack_against_defence() {
        let rules = Rules::default();
        let (wolf, goblin) = (
            combat(&a_wolf(), 1, Holds::default(), &[], &rules),
            combat(&a_goblin(), 1, Holds::default(), &[], &rules),
        );
        let mut rng = Rng::of(50, 39, 0);
        let hits = (0..10_000)
            .filter(|_| strike(&goblin, &wolf, Blow::default(), &mut rng).is_some())
            .count();
        // A/(A + D) = 18.5/34.5 = 0.536.
        assert!((5_100..5_650).contains(&hits), "{hits}");
        // Against no defence the chance is 0.9999: the roll of 1,000 of the 1,001 misses.
        let open = Combat {
            defence: 0.0,
            ..wolf
        };
        let misses = (0..10_010)
            .filter(|_| strike(&wolf, &open, Blow::default(), &mut rng).is_none())
            .count();
        assert!((1..40).contains(&misses), "{misses}");
    }

    #[test]
    fn the_weight_takes_ten_thousand_for_a_divisor_of_nearly_nothing() {
        // The values of `0x43EEE0` that `opensacred-oracle` recorded (#363, #406).
        let close = |a: f32, b: f64| (f64::from(a) - b).abs() <= 1e-6 * b.abs().max(1.0);
        assert!(close(hit_chance(0.0, 0.0), 0.9999));
        assert!(close(hit_chance(0.0, 0.0005), 0.9999));
        assert!(close(hit_chance(5000.0, 0.0005), 0.9999));
        // The float 0.001 lies above the double bound: it divides.
        assert!(close(hit_chance(0.5, 0.001), 0.998_004));
        assert!(close(hit_chance(0.0, 0.001), 0.0));
        assert!(close(hit_chance(13.0, 16.0), 13.0 / 29.0));
        assert!(close(dealt(15.0, 0.0, 1.0), 14.998_500_149_985));
        assert!(close(dealt(15.0, 0.0005, 1.0), 14.998_500_149_985));
        assert!(close(dealt(15.0, 0.001, 1.0), 14.999_000_066_597));
        assert!(close(dealt(15.0, 1.0, 1.0), 14.0625));
        assert!(close(dealt(4.43, 0.0, 1.0), 4.429_556_872_651));
        // A larger exponent takes the difference away: 15 at 50 levels above.
        assert!(close(dealt(15.0, 0.0, exponent(50)), 15.0));
        assert!(close(dealt(0.0, 0.0, 1.0), 0.0));
    }

    #[test]
    fn a_hits_damage_is_less_by_the_resistance_and_the_damping() {
        let mut rng = Rng::of(50, 39, 0);
        let attacker = Combat {
            attack: 10.0,
            defence: 0.0,
            damage: [[10.0, 10.0], [0.0; 2], [0.0; 2], [0.0; 2]],
            resistance: [0.0; 4],
            damping: [0; 4],
            speed: 140.0,
            pace: 130.0,
        };
        let defender = Combat {
            resistance: [10.0, 0.0, 0.0, 0.0],
            ..attacker
        };
        // 10²/(10 + 10) = 5; with a damping of 40 per cent, 3; half a strike, 2.
        assert_eq!(
            strike(&attacker, &defender, Blow::default(), &mut rng),
            Some(5)
        );
        let damped = Combat {
            damping: [40, 0, 0, 0],
            ..defender
        };
        assert_eq!(
            strike(&attacker, &damped, Blow::default(), &mut rng),
            Some(3)
        );
        assert_eq!(
            strike(
                &attacker,
                &defender,
                Blow {
                    share: 0.5,
                    ..Blow::default()
                },
                &mut rng
            ),
            Some(2)
        );
        // Every kind below 1: 15 physical, of which a resistance of 0 lets 0.9999 through.
        let weak = Combat {
            damage: [[0.5, 0.9], [0.0; 2], [0.0; 2], [0.0; 2]],
            ..attacker
        };
        assert_eq!(
            strike(&weak, &attacker, Blow::default(), &mut rng),
            Some(14)
        );
        assert_eq!(strike(&weak, &defender, Blow::default(), &mut rng), Some(9));
    }

    /// A combat art's strike (#508): the attack `A · (1.3 + 0.125·L)` in its chance, as the
    /// original's Seraphim's 1071 took `28.5` into 40.61 at level 1 and 47.74 at level 3, and its
    /// damage times its factor; any landed strike deals 1 at least.
    #[test]
    fn an_arts_strike_takes_its_attack_and_its_factor() {
        assert_eq!(art_attack(28.5, 1), 40.6125);
        assert_eq!(art_attack(28.5, 3), 47.7375);
        assert_eq!(art_attack(10.0, 0), 13.0);
        // The factor scales between 0 and 500 but at 1; 0 scales nothing.
        assert_eq!(
            [0.0, 0.8, 1.0, 499.0, 500.0].map(|factor| scaled(10.0, factor)),
            [10.0, 8.0, 10.0, 4990.0, 10.0]
        );
        let attacker = Combat {
            attack: 10.0,
            defence: 0.0,
            damage: [[10.0, 10.0], [0.0; 2], [0.0; 2], [0.0; 2]],
            resistance: [0.0; 4],
            damping: [0; 4],
            speed: 140.0,
            pace: 130.0,
        };
        let defender = Combat {
            defence: 20.0,
            resistance: [10.0, 0.0, 0.0, 0.0],
            ..attacker
        };
        // 10²/(10 + 10) = 5, times 0.8 = 4; at level 1 the art's chance is 14.25/34.25.
        let landed = |art: bool, rng: &mut Rng| {
            (0..1000)
                .filter_map(|_| {
                    if art {
                        art_strike(
                            &attacker,
                            &defender,
                            Blow {
                                share: 0.8,
                                ..Blow::default()
                            },
                            1,
                            rng,
                        )
                    } else {
                        strike(&attacker, &defender, Blow::default(), rng)
                    }
                })
                .collect::<Vec<_>>()
        };
        let mut rng = Rng::of(50, 39, 0);
        let plain = landed(false, &mut rng);
        let art = landed(true, &mut rng);
        assert!(plain.iter().all(|&d| d == 5) && art.iter().all(|&d| d == 4));
        // About 333 and 416 strikes of the thousand land.
        assert!((300..370).contains(&plain.len()), "{}", plain.len());
        assert!((380..450).contains(&art.len()), "{}", art.len());
        // A strike that lets less than 1 through deals 1.
        let armoured = Combat {
            resistance: [100_000.0, 0.0, 0.0, 0.0],
            defence: 0.0,
            ..defender
        };
        assert_eq!(
            strike(&attacker, &armoured, Blow::default(), &mut rng),
            Some(1)
        );
    }

    #[test]
    fn a_damage_is_rolled_between_its_ends_in_steps_by_its_size() {
        let mut rng = Rng::of(50, 39, 0);
        let damage = [[3.33, 4.43], [0.0; 2], [0.0; 2], [0.0; 2]];
        let rolls: Vec<f32> = (0..1000)
            .map(|_| roll_damage(&damage, 1.0, &mut rng)[0])
            .collect();
        // The float 3.33 is 3.3299999…, whose hundredths truncate to 332 (#620).
        assert!(rolls.iter().all(|&d| (3.32..=4.43).contains(&d)));
        // Hundredths below 100.
        assert!(rolls.iter().any(|&d| (d * 10.0).fract() > 0.01));
        // Tenths from 100.
        let large = [[150.0, 160.0], [0.0; 2], [0.0; 2], [0.0; 2]];
        assert!((0..100).all(|_| {
            let d = roll_damage(&large, 1.0, &mut rng)[0] * 10.0;
            (d - d.round()).abs() < 1e-3
        }));
    }

    #[test]
    fn a_creatures_rolled_damage_takes_proz_dmg_after_the_record_at_53_bits() {
        let mut rng = Rng::of(50, 39, 0);
        let one = |d: f32| [[d, d], [0.0; 2], [0.0; 2], [0.0; 2]];
        // The goblin warrior's strikes at the Seraphim that the original's trace read (#620): the
        // rolls 2163, 1930 and 1962 hundredths, each times the float 0.01 stored as a float, then
        // times 0.9 at 53 bits and stored.
        for (end, expected) in [
            (21.635, 19.467_f32),
            (19.305, 17.369_999),
            (19.625, 17.657_999),
        ] {
            assert_eq!(roll_damage(&one(end), 0.9, &mut rng)[0], expected, "{end}");
        }
        // The rule of 15 takes the rolled damage before the factor: 1.05 stays itself, about
        // 0.94 after it, where the ends scaled first made every kind below 1 and so 15.
        let kept = roll_damage(&one(1.055), 0.9, &mut rng)[0];
        assert!((0.94..0.95).contains(&kept), "{kept}");
        assert_eq!(roll_damage(&one(0.95), 0.9, &mut rng)[0], 13.5);
        // The step goes by the most before the factor: 105 rolls in tenths.
        let tenths = roll_damage(
            &[[100.0, 109.9], [0.0; 2], [0.0; 2], [0.0; 2]],
            1.0,
            &mut rng,
        );
        assert!(((tenths[0] * 10.0) - (tenths[0] * 10.0).round()).abs() < 1e-3);
        // From 10,000 the least plus whole hundredths of the span.
        let huge = [[10_000.0, 20_000.0], [0.0; 2], [0.0; 2], [0.0; 2]];
        assert!((0..100).all(|_| {
            let d = roll_damage(&huge, 1.0, &mut rng)[0];
            (10_000.0..=20_000.0).contains(&d) && (d % 100.0).abs() < 1e-3
        }));
        // A least below 0 is 0, a most below 0 is 1: a roll of 0 to 1 hundredth.
        let negative = roll_damage(&[[-5.0, -1.0], [0.0; 2], [0.0; 2], [0.0; 2]], 1.0, &mut rng);
        assert_eq!(negative[0], 15.0);
    }

    #[test]
    fn an_npc_counts_its_first_skills_by_its_level_at_a_third_of_it() {
        let wolf = a_wolf();
        assert!(skills(&wolf, 1, false).is_empty());
        assert_eq!(skills(&wolf, 4, false), [(2, 1)]);
        assert_eq!(skills(&wolf, 10, false), [(2, 3), (11, 3)]);
        // A hero its two slots at level 1.
        assert_eq!(skills(&wolf, 10, true), [(2, 1), (11, 1)]);
    }

    #[test]
    fn a_skills_curve_runs_from_its_offset_to_its_value_at_s_plus_one() {
        let triple = [12.0, 50.0, 200.0];
        assert!((curve(triple, 1) - 12.0).abs() < 1e-4);
        assert!((curve(triple, 51) - 200.0).abs() < 1e-3);
        assert!((curve(triple, 3) - (12.0 + 376.0 * (1.0 - 1.0 / 1.04))).abs() < 1e-3);
        assert!(curve(triple, 0).abs() < f32::EPSILON);
    }

    #[test]
    fn the_skills_raise_a_level_ten_wolfs_damage_and_resistance_as_the_originals() {
        let rules = Rules::default();
        let wolf = a_wolf();
        // docs/re/fight.md: the skills 2 and 11 at level 3.
        let older = combat(
            &wolf,
            10,
            Holds::default(),
            &skills(&wolf, 10, false),
            &rules,
        );
        assert!(near(older.damage[0][0], 6.66) && near(older.damage[0][1], 10.51));
        assert!((older.resistance[0] - 11.0).abs() < 0.02);
        assert!(near(older.attack, 24.5) && near(older.defence, 30.2));
        // The skill 8 raises the attack and the defence, truncated: 24.5 · 1.07 and 30.2 · 1.13.
        let trained = combat(&wolf, 10, Holds::default(), &[(8, 1)], &rules);
        assert!(near(trained.attack, 26.0) && near(trained.defence, 34.0));
        // The skill 10 the regeneration: 44 · 1.06, 46.64 taken whole (#616).
        let base = Base {
            attributes: [0, 0, 0, 22, 0, 0],
            ..Base::default()
        };
        assert!((regeneration(&base, 1, &[(10, 1)], &rules) - 46.0).abs() < 1e-3);
    }

    #[test]
    fn a_and_d_take_their_multipliers_as_the_originals_world_at_53_bits() {
        let rules = Rules::default();
        let curves = &rules.curves;
        // The multipliers as the original's driver read them exactly (#589): the skill 8 at
        // level 10 gives 1.43 (1.4299999475) and 1.7767797, where single precision gives
        // 1.4300001 and 1.7767799; at level 8 1.3598245 and 1.6507018; the skills 9, then 8, at
        // level 26 `D`'s 4.2599998, single precision 4.26.
        let times = |triples: &[[f32; 3]], at: u32| {
            triples
                .iter()
                .fold(1.0, |times, &triple| raised(times, curve(triple, at)))
        };
        for (triples, at, expected) in [
            (&[curves.attack][..], 10, 1.43_f32),
            (&[curves.defence][..], 10, 1.776_779_7),
            (&[curves.attack][..], 8, 1.359_824_5),
            (&[curves.defence][..], 8, 1.650_701_8),
            (&[curves.attack][..], 26, 1.856_666_6),
            (&[curves.parry, curves.defence][..], 26, 4.259_999_8),
        ] {
            assert_eq!(times(triples, at).to_bits(), expected.to_bits(), "{at}");
        }
        // And so `A` and `D`, the products truncated as they are (#593): the type 200 at level
        // 31 (its skills 2, 11 and 8 at level 10) `A` 100 times 1.43, 142, not 143, and `D` 177;
        // the type 51 at level 78 (11, 2, 9 and 8 at 26) `A` 239 times 1.8566666, 443, and `D`
        // 200 times 4.2599998, 851, not 852; the goblin 76 at level 25 84 and 114.
        let base = |id: u32, attributes: [u8; 6], skills: [u8; 4]| Base {
            id,
            attributes,
            skills: [skills[0], skills[1], skills[2], skills[3], 0, 0, 0, 0],
            ..Base::default()
        };
        for (base, level, attack, defence) in [
            (
                base(200, [25, 20, 25, 16, 10, 15], [2, 11, 8, 24]),
                31,
                142.0,
                177.0,
            ),
            (
                base(51, [35, 25, 20, 43, 10, 12], [11, 2, 9, 8]),
                78,
                443.0,
                851.0,
            ),
            (
                base(76, [15, 20, 22, 12, 0, 4], [8, 2, 0, 0]),
                25,
                84.0,
                114.0,
            ),
        ] {
            let values = combat(
                &base,
                level,
                Holds::default(),
                &skills(&base, level, false),
                &rules,
            );
            assert_eq!(
                (values.attack, values.defence),
                (attack, defence),
                "{}",
                base.id
            );
        }
    }

    #[test]
    fn the_fist_fight_raises_an_unarmed_creatures_attack_and_attack_speed() {
        let rules = Rules::default();
        // The skeleton warrior (type 32) of the skills 2 and 24, as the original's at the levels
        // 10 and 40 (#588): `A` 47 and 180, the attack speed 153 and 182.
        let skeleton = Base {
            id: 32,
            attributes: [25, 35, 15, 15, 0, 50],
            skills: [2, 24, 11, 4, 0, 0, 0, 0],
            ..Base::default()
        };
        for (level, attack, speed) in [(10, 47.0, 153.0), (40, 180.0, 182.0)] {
            let unarmed = combat(
                &skeleton,
                level,
                Holds::default(),
                &skills(&skeleton, level, false),
                &rules,
            );
            assert!(near(unarmed.attack, attack), "{level}: {}", unarmed.attack);
            assert!(near(unarmed.speed, speed), "{level}: {}", unarmed.speed);
        }
        // Armed, the skill does nothing.
        let armed = Holds {
            weapon: true,
            ..Holds::default()
        };
        let with = combat(&skeleton, 10, armed, &[(24, 3)], &rules);
        assert!(near(with.attack, 37.5) && near(with.speed, 140.0));
    }

    #[test]
    fn the_hero_and_the_creatures_strike_each_other_by_bronzes_factors() {
        let rules = Rules::default();
        let (wolf, goblin) = (
            combat(&a_wolf(), 1, Holds::default(), &[], &rules),
            combat(&a_goblin(), 1, Holds::default(), &[], &rules),
        );
        // A creature striking the hero: its attack times 0.65, its rolled damage times 0.9, its
        // damage's ends as they are (#620).
        let (a, d, blow) = weighed(wolf, goblin, Some(Side::Defender), (1, 1), &rules);
        assert!(near(a.attack, 13.0 * 0.65) && a.damage == wolf.damage);
        assert_eq!(d, goblin);
        assert!((blow.k - 1.0).abs() < 1e-6 && near(blow.rolled, 0.9));
        // Ten levels above the hero's: `p` 0.55, `k = −log₂ 0.45`.
        let (.., blow) = weighed(wolf, goblin, Some(Side::Defender), (11, 1), &rules);
        assert!((blow.k - -(0.45_f32).log2()).abs() < 1e-5);
        // The hero striking a creature: its defence times 0.65.
        let (a, d, blow) = weighed(goblin, wolf, Some(Side::Attacker), (1, 11), &rules);
        assert_eq!(a, goblin);
        assert!(near(d.defence, 16.0 * 0.65) && near(d.resistance[0], 4.48));
        assert_eq!(blow, Blow::default());
        // Between creatures, as they are, the exponent of the attacker's levels above its
        // defender's (#630): `p` 0.54 for 8 above, none for one below.
        let (a, d, blow) = weighed(wolf, goblin, None, (9, 1), &rules);
        assert_eq!((a, d), (wolf, goblin));
        assert!((blow.k - -(0.46_f32).log2()).abs() < 1e-5 && near(blow.rolled, 1.0));
        assert_eq!(
            weighed(wolf, goblin, None, (1, 2), &rules),
            (wolf, goblin, Blow::default())
        );
    }

    #[test]
    fn a_creatures_strike_at_the_hero_is_times_the_factor_of_its_band_of_levels() {
        let rules = Rules::default();
        let at = |level| weapons_factor(Some(Side::Defender), level, Rank::Common, &rules);
        // The step over the level's distance from its band's start, not interpolated.
        let close = |a: f64, b: f64| (a - b).abs() < 1e-6;
        for (level, factor) in [
            (1, 0.65),
            (2, 0.625),
            (4, 0.6125),
            (5, 0.75),
            (6, 0.7),
            (10, 0.65 + 0.1 / 6.0),
            (11, 0.85),
            (20, 0.76),
            (21, 0.95),
            (35, 0.85 + 0.1 / 15.0),
            (40, 0.855),
            (216, 0.85 + 0.1 / 196.0),
        ] {
            assert!(close(at(level), factor), "level {level}: {}", at(level));
        }
        // On the x87 unit at 53 bits: at level 1 the float 0.65 exactly.
        assert_eq!(at(1), f64::from(0.65_f32));
        // The level 0 would divide by zero.
        assert_eq!(at(0), 1.0);
        // Not a boss's nor a champion's, not the hero's, not between creatures.
        for rank in [Rank::Boss, Rank::Champion] {
            assert_eq!(weapons_factor(Some(Side::Defender), 1, rank, &rules), 1.0);
        }
        assert_eq!(
            weapons_factor(Some(Side::Attacker), 1, Rank::Common, &rules),
            1.0
        );
        assert_eq!(weapons_factor(None, 1, Rank::Common, &rules), 1.0);
    }

    #[test]
    fn the_bands_of_the_levels_are_the_balances_rows() {
        let row = |name: &str, offset: u16| BalanceVariable {
            name: name.to_owned(),
            offset,
            value: BalanceValue::F32(0.0),
        };
        let mut image = vec![0_u32; 40];
        for (i, value) in [0.5_f32, 0.7, 0.8, 0.9, 1.0].into_iter().enumerate() {
            image[i] = value.to_bits();
        }
        for (i, value) in [0_i32, 2, 10, 20, 35].into_iter().enumerate() {
            image[20 + i] = u32::from_ne_bytes(value.to_ne_bytes());
        }
        let balance = Balance {
            variables: vec![row("WaffenAll", 0), row("AllOverLv", 80)],
            image,
            ..Balance::default()
        };
        let rules = Rules::of(Some(&balance));
        let at = |level| weapons_factor(Some(Side::Defender), level, Rank::Common, &rules);
        assert!((at(1) - f64::from(0.7_f32)).abs() < 1e-9);
        // The second band, from the level 2: a step of 0.1 over one level.
        assert!((at(3) - 0.8).abs() < 1e-6);
        // A factor of 0 or below is 1.
        let mut negative = rules.clone();
        negative.bands.factors = [-1.0, -0.5, 0.75, 0.85, 0.95];
        assert_eq!(negative.bands.factor(1), 1.0);
    }

    #[test]
    fn a_goblins_strikes_at_the_seraphim_take_what_the_originals_took() {
        // The original's strikes of a goblin warrior (76) of level 1 at the Seraphim of level 1
        // (#615): its rolled physical damages after `ProzDmg` and her physical resistance, as
        // `0x43EEE0` took them, and the health she lost, 11, 10 and 10.
        let rules = Rules::default();
        let factor = weapons_factor(Some(Side::Defender), 1, Rank::Common, &rules);
        let resistance = 2.241_398;
        let lost = [
            (19.467, 17.457_025, 11),
            (17.369_999, 15.384_771, 10),
            (17.657_999, 15.669_065, 10),
        ];
        for (rolled, through, health) in lost {
            let sum = dealt(rolled, resistance, 1.0);
            assert!((sum - through).abs() < 1e-4, "{rolled}: {sum}");
            assert_eq!(taken(sum, 1.0, factor), health, "{rolled}");
            // Without the factor, the 17, 15 and 15 the game took before.
            assert!(taken(sum, 1.0, 1.0) >= 15);
        }
        // Half a strike, and at least 1.
        assert_eq!(taken(15.384_771, 0.5, factor), 5);
        assert_eq!(taken(0.4, 1.0, factor), 1);
    }

    #[test]
    fn the_share_of_the_exponent_is_held_at_099_only_from_1_up() {
        let close = |a: f32, b: f64| (f64::from(a) - b).abs() <= 1e-5 * b.abs().max(1.0);
        assert!(close(exponent(0), 1.0));
        assert!(close(exponent(98), -(0.01_f64).log2()));
        // 99 levels above, the share 0.995 stays (#407).
        assert!(close(exponent(99), -(0.005_f64).log2()));
        assert!(close(exponent(100), -(0.01_f64).log2()));
        assert!(close(exponent(150), -(0.01_f64).log2()));
        // The values of `0x43EEE0` that `opensacred-oracle` recorded 99 levels above (#363).
        assert!(close(
            dealt(1.0, 1000.0, exponent(99)),
            0.007_610_927_548_607
        ));
        assert!(close(dealt(15.0, 30.0, exponent(99)), 14.323_800_614_878));
        assert!(close(dealt(4.43, 4.48, exponent(99)), 4.406_881_828_101));
    }

    #[test]
    fn a_larger_exponent_takes_more_of_the_damage_through_the_resistance() {
        let mut rng = Rng::of(50, 39, 0);
        let attacker = Combat {
            attack: 10.0,
            defence: 0.0,
            damage: [[10.0, 10.0], [0.0; 2], [0.0; 2], [0.0; 2]],
            resistance: [0.0; 4],
            damping: [0; 4],
            speed: 140.0,
            pace: 130.0,
        };
        let defender = Combat {
            resistance: [10.0, 0.0, 0.0, 0.0],
            ..attacker
        };
        // `10·(1 − 1/2²)` = 7.5 for `k` 2.
        assert_eq!(
            strike(
                &attacker,
                &defender,
                Blow {
                    k: 2.0,
                    ..Blow::default()
                },
                &mut rng
            ),
            Some(7)
        );
    }

    #[test]
    fn the_heros_health_has_no_factor_of_a_creature() {
        let rules = Rules {
            factor: 2.0,
            ..Rules::default()
        };
        let base = [22, 19, 25, 22, 22, 17];
        assert_eq!(
            most_health(base, 1, &[], 1.0, &rules),
            2 * hero_health(base, 1, &[], &Rules::default())
        );
        assert_eq!(
            hero_health(base, 1, &[], &Rules::default()),
            health_of(base, 1, None, 1.0, 1.0)
        );
    }

    #[test]
    fn regeneration_gives_one_health_for_each_4096_gathered() {
        // `2·a3`: a base of 22 at level 1, 44; at level 11, 2·(22 + 22) = 88.
        let base = Base {
            attributes: [0, 0, 0, 22, 0, 0],
            ..Base::default()
        };
        assert!((regeneration(&base, 1, &[], &Rules::default()) - 44.0).abs() < 1e-6);
        assert!((regeneration(&base, 11, &[], &Rules::default()) - 88.0).abs() < 1e-6);
        let mut regenerates = Regenerates::new(44.0);
        // 44 · 128 a second: 5,632 after one, a health and 5,632 − 4,096 = 1,536 kept as its
        // low ten bits, 512.
        assert_eq!(regenerates.gather(128.0, 0.5, false), 0);
        assert_eq!(regenerates.gather(128.0, 0.5, false), 1);
        assert_eq!(regenerates.kept, 512);
        // At its most it keeps nothing.
        assert_eq!(regenerates.gather(128.0, 0.5, true), 0);
        assert_eq!(regenerates.kept, 0);
    }

    #[test]
    fn the_ticks_mask_takes_the_overshoot_of_a_fast_regeneration() {
        // The Seraphim of level 30 alone (#616): 170 · 128 = 21,760 a second, on ticks of 6 and 7
        // frames, 0.1 and 0.1167 s. Each tick adds 2,176 or 2,538 and each 4,096 crossed keeps
        // only its overshoot's low ten bits: about 4.6 a second where 5.31 would be gathered, as
        // the original's 4.55 measured.
        let mut regenerates = Regenerates::new(170.0);
        let (mut seconds, mut gained) = (0.0, 0);
        for tick in 0..200 {
            let step = if tick % 2 == 0 { 0.1 } else { 0.116_666_67 };
            seconds += step;
            gained += regenerates.gather(128.0, step, false);
        }
        let rate = f64::from(gained) / f64::from(seconds);
        assert!((4.4..4.8).contains(&rate), "{rate}");
        // The Seraphim of level 1 (44) adds below 1,024 a tick and loses only the fractions.
        let mut regenerates = Regenerates::new(44.0);
        let gained: u32 = (0..200)
            .map(|tick| {
                regenerates.gather(128.0, if tick % 2 == 0 { 0.1 } else { 0.116_666_67 }, false)
            })
            .sum();
        assert_eq!(gained, 29);
    }

    #[test]
    fn the_hero_regenerates_by_its_fight_and_the_hostiles_near() {
        let rules = Rules::default();
        let rate = |hero, hostile| rules.regeneration_rate(hero, hostile);
        // An NPC at `RegInFight` always, the hero in the fight, a hostile within 350 (#474).
        assert!((rate(false, None) - 36.0).abs() < f32::EPSILON);
        assert!((rate(true, Some(349.0)) - 36.0).abs() < f32::EPSILON);
        // Else `RegEnemyNear` with a hostile within 1,000, `RegNoEnemy` without.
        assert!((rate(true, Some(350.0)) - 48.0).abs() < f32::EPSILON);
        assert!((rate(true, Some(1_000.0)) - 48.0).abs() < f32::EPSILON);
        assert!((rate(true, Some(1_001.0)) - 128.0).abs() < f32::EPSILON);
        assert!((rate(true, None) - 128.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_kill_earns_the_creatures_experience_by_the_levels() {
        let rules = Rules::default();
        let worth = [20, 5];
        // Of the hero's level, or one above, the whole `a + E·b`.
        assert_eq!(experience_of(worth, 1, 1, &rules), 25);
        assert_eq!(experience_of(worth, 2, 1, &rules), 30);
        // Above `ObereEXPVoll`, less by `ObereEXPVoll/r`: 35 · 1.1/1.2.
        assert_eq!(experience_of(worth, 3, 1, &rules), 32);
        // Below the hero's: at the threshold 0.04, above it `(r − t)/(1 − t)`.
        assert_eq!(experience_of(worth, 2, 5, &rules), 1);
        assert_eq!(experience_of(worth, 3, 5, &rules), 11);
        // Bronze's `ProzExp` and the balance's values.
        let more = Rules {
            experience: [2.0, 0.85, 1.0, 1.1],
            ..Rules::default()
        };
        assert_eq!(experience_of(worth, 1, 1, &more), 50);
    }

    #[test]
    fn every_creature_strikes_at_140_per_cent_but_the_wood_elf() {
        assert!((attack_speed(76) - 140.0).abs() < f32::EPSILON);
        assert!((attack_speed(5) - 100.0).abs() < f32::EPSILON);
        let goblin = combat(&a_goblin(), 1, Holds::default(), &[], &Rules::default());
        assert!((goblin.clip_speed() - 1.4).abs() < 1e-6);
    }

    #[test]
    fn the_levels_end_at_the_originals_experience() {
        assert_eq!(
            [1, 2, 3, 4, 5, 9, 10].map(level_end),
            [300, 1_200, 3_000, 5_900, 10_200, 49_500, 66_900]
        );
        // The top level's end, one above the original's cap of the experience.
        assert_eq!(level_end(TOP_LEVEL), 2_586_931_600);
        assert_eq!(level_of(299, 1), 1);
        assert_eq!(level_of(300, 1), 2);
        assert_eq!(level_of(1_199, 1), 2);
        assert_eq!(level_of(1_200, 2), 3);
        assert_eq!(level_of(49_500, 3), 10);
        assert_eq!(level_of(u64::MAX, 1), TOP_LEVEL);
    }

    #[test]
    fn a_level_gives_an_attribute_point_and_skill_points_by_the_slots() {
        let rules = Rules::default();
        let mut experience = Experience::default();
        assert!(!experience.add(299, &rules));
        // Level 2: one attribute point, one skill point.
        assert!(experience.add(1, &rules));
        assert_eq!(
            (
                experience.level,
                experience.attribute_points,
                experience.skill_points
            ),
            (2, 1, 1)
        );
        // Level 3: another attribute point, two skill points, a third skill to learn.
        assert!(experience.add(900, &rules));
        assert_eq!(
            (
                experience.level,
                experience.attribute_points,
                experience.skill_points
            ),
            (3, 2, 3)
        );
        assert_eq!(experience.to_learn, 1);
    }

    #[test]
    fn a_high_heros_kills_earn_less_by_the_cap_the_decay_and_the_fade() {
        let rules = Rules::default();
        let worth = [52, 48];
        // Up to level 50 the whole; at 60 by (50/60)².
        assert_eq!(experience_of(worth, 1, 1, &rules), 100);
        let at_60 = experience_of(worth, 60, 60, &rules);
        assert!(
            (to_f32(at_60) - 2_932.0 * 2_500.0 / 3_600.0).abs() <= 1.0,
            "{at_60}"
        );
        // At 80 also by the fade of bronze, (101 − 80)/31.
        let at_80 = experience_of(worth, 80, 80, &rules);
        let expected = 3_892.0 * 2_500.0 / 6_400.0 * 21.0 / 31.0;
        assert!((to_f32(at_80) - expected).abs() <= 1.0, "{at_80}");
        // From 101 on bronze, 1 a kill.
        assert_eq!(experience_of(worth, 101, 101, &rules), 1);
        assert!((decay(110) - 0.985_f32.powi(11)).abs() < 1e-5);
        assert!((decay(99) - 1.0).abs() < f32::EPSILON);
    }

    /// The template 29 of `wpmod.bin`, of the daggers 1712 and others (docs/re/items.md).
    fn dagger_template() -> WeaponModifier {
        let mut words = vec![
            1764, 1728, 1774, 1712, 0, 4, 50, 10, 100, 64, 77, 8, 23, 36, 8, 23, 36, 0, 23, 36, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100, 20, 80, 0, 15, 25, 1, 0,
        ];
        words.resize(53, 0);
        WeaponModifier {
            words,
            entries: Vec::new(),
        }
    }

    #[test]
    fn a_goblins_dagger_of_level_1_does_12_to_15_by_its_template() {
        let templates = Templates::new(&[dagger_template()], &[]);
        let mut rng = Rng::of(51, 39, 0);
        let made: Vec<ItemValues> = (0..400)
            .filter_map(|_| templates.make(1712, 1, &mut rng))
            .collect();
        assert_eq!(made.len(), 400);
        // Physical alone in most: 12 to 15; with an element, its most and the physical's sum
        // to 14 or 15.
        let alone = made
            .iter()
            .filter(|v| v.damage[1..].iter().all(|ends| ends[1] == 0.0))
            .count();
        assert!((300..380).contains(&alone), "{alone}");
        for values in &made {
            let most: f32 = values.damage.iter().map(|ends| ends[1]).sum();
            if values.damage[1..].iter().all(|ends| ends[1] == 0.0) {
                assert_eq!(values.damage[0], [12.0, 15.0]);
            } else {
                assert!((13.0..=16.0).contains(&most), "{values:?}");
            }
            assert!((5.0..=21.0).contains(&values.attack) && values.defence == 0.0);
        }
        // At level 5, 17 to 20.
        let level_5 = (0..50)
            .filter_map(|_| templates.make(1712, 5, &mut rng))
            .find(|v| v.damage[1..].iter().all(|ends| ends[1] == 0.0));
        assert_eq!(level_5.map(|v| v.damage[0]), Some([17.0, 20.0]));
        // No template, no values.
        assert_eq!(templates.make(9999, 1, &mut rng), None);
    }

    /// The words from `+4` of the Seraphim's weapon of her template, the record 7901 (docs/02 §9):
    /// physical damage 21 to 23 (`+0x1C`, `+0x24`), `A` 5 and `D` 2 (`+0x2C`, `+0x2E`), the
    /// effects 811 and 812 of the values 14 and 2 in its first two bonus slots (`+0x4A`, `+0x6A`);
    /// a poison resistance of 12 too.
    fn blade() -> Vec<u16> {
        let mut words = vec![0; 63];
        for (offset, value) in [
            (0x1C, 21),
            (0x24, 23),
            (0x2C, 5),
            (0x2E, 2),
            (0x38, 12),
            (0x4A, 811),
            (0x4E, 812),
            (0x6A, 14),
            (0x6C, 2),
        ] {
            words[(offset - 4) / 2] = value;
        }
        words
    }

    /// The arts' speeds by the attributes `MagicRegen` (4) and `PhysicalRegen` (3), grown by the
    /// level and raised by the items, and the items' bonuses 813 and 814.
    #[test]
    fn the_arts_speeds_follow_the_regeneration_attributes_and_the_items() {
        let base = Base {
            id: 1,
            attributes: [10, 10, 10, 50, 20, 10],
            ..Base::default()
        };
        assert_eq!(
            arts_speeds(&base, 1, &Holds::default()),
            arts::Speeds {
                spells: 1.2,
                weapon_arts: 1.5
            }
        );
        // At level 11 the attributes grow by their tenth each level: 40 and 100.
        assert_eq!(
            arts_speeds(&base, 11, &Holds::default()),
            arts::Speeds {
                spells: 1.4,
                weapon_arts: 2.0
            }
        );
        let mut bonuses = [Bonus::default(); BONUSES];
        for (slot, effect, value) in [(0, 813, 10), (1, 818, 10)] {
            bonuses[slot] = Bonus {
                condition: 0,
                effect,
                value,
            };
        }
        let holds = Holds {
            items: vec![ItemValues {
                bonuses,
                ..ItemValues::default()
            }],
            ..Holds::default()
        };
        // The spells' 1.1 · 1.2, the weapon arts' by `PhysicalRegen` 50 + 10.
        assert_eq!(
            arts_speeds(&base, 1, &holds),
            arts::Speeds {
                spells: 1.32,
                weapon_arts: 1.6
            }
        );
    }

    #[test]
    fn a_records_words_give_the_items_values() {
        let mut bonuses = [Bonus::default(); BONUSES];
        bonuses[0] = Bonus {
            condition: 0,
            effect: 811,
            value: 14,
        };
        bonuses[1] = Bonus {
            condition: 0,
            effect: 812,
            value: 2,
        };
        assert_eq!(
            ItemValues::of_record(&blade()),
            ItemValues {
                damage: [[21.0, 23.0], [0.0; 2], [0.0; 2], [0.0; 2]],
                resistance: [0.0, 0.0, 0.0, 12.0],
                attack: 5.0,
                defence: 2.0,
                bonuses,
            }
        );
        assert_eq!(ItemValues::of_record(&[]), ItemValues::default());
    }

    #[test]
    fn a_creatures_pace_is_130_raised_by_its_items_and_its_skill_11() {
        let rules = Rules::default();
        let held = |items| Holds {
            weapon: true,
            shield: false,
            ranged: false,
            items,
        };
        let nothing = Holds::default;
        // A townsperson 655 at 130: its walk of 60 at `GameSpeed` 1.25 times 1.3, 97.5 units a
        // second (the original's about 102, #599).
        let townsperson = Base {
            id: 655,
            skills: [0; 8],
            ..a_wolf()
        };
        let walker = combat(&townsperson, 1, nothing(), &[], &rules);
        assert_eq!(walker.pace, 130.0);
        assert!((60.0 * 1.25 * walker.pace_factor() - 97.5).abs() < 1e-3);
        // The Seraphim's blade's +2 (and +14 to its attack speed): 132, her run of 180 at 297
        // units a second (the original's 294, and the driver's 154 and 132).
        let seraphim = Base {
            id: 1,
            ..a_goblin()
        };
        let runner = combat(
            &seraphim,
            1,
            held(vec![ItemValues::of_record(&blade())]),
            &[],
            &rules,
        );
        assert_eq!((runner.pace, runner.speed), (132.0, 154.0));
        assert!((180.0 * 1.25 * runner.pace_factor() - 297.0).abs() < 1e-3);
        // The Wood Elf's 120.
        let wood_elf = Base {
            id: 5,
            ..a_goblin()
        };
        assert_eq!(combat(&wood_elf, 1, nothing(), &[], &rules).pace, 120.0);
        // The skill 11 by its curve `RoffSP`, `R_s_SP`, `R_w_SP`: 1 at level 1, 2 at level 4.
        assert_eq!(
            combat(&townsperson, 3, nothing(), &[(11, 1)], &rules).pace,
            131.0
        );
        assert_eq!(
            combat(&townsperson, 12, nothing(), &[(11, 4)], &rules).pace,
            132.0
        );
    }

    #[test]
    fn the_items_bonus_slots_count_in_the_values() {
        let rules = Rules::default();
        let held = |items: Vec<ItemValues>| Holds {
            weapon: true,
            shield: false,
            ranged: false,
            items,
        };
        let plain = ItemValues {
            bonuses: [Bonus::default(); BONUSES],
            ..ItemValues::of_record(&blade())
        };
        let (with, without) = (
            combat(
                &a_goblin(),
                1,
                held(vec![ItemValues::of_record(&blade())]),
                &[],
                &rules,
            ),
            combat(&a_goblin(), 1, held(vec![plain]), &[], &rules),
        );
        // 811 raises the attack speed by its value, as the Seraphim's 154 in the original (#490).
        assert!(near(with.speed, without.speed + 14.0));
        assert_eq!(with.attack, without.attack);
        // 801 scaled by `a0` (byte 2 of 1) adds 11 times `a0` over 100 to the physical ends; 809
        // and 810 a hundredth of theirs to the multipliers of `A` and `D`; 816 raises `a2`,
        // and with it `A` by half and `D` by 0.8.
        let mut slots = [Bonus::default(); BONUSES];
        slots[0] = Bonus {
            condition: 0,
            effect: 0x1_0321,
            value: 11,
        };
        slots[1] = Bonus {
            condition: 0,
            effect: 816,
            value: 5,
        };
        let sword = ItemValues {
            bonuses: slots,
            ..plain
        };
        let goblin = combat(&a_goblin(), 1, held(vec![sword]), &[], &rules);
        let a0 = f32::from(a_goblin().attributes[0]);
        let factor = without.damage[0][0] / 21.0;
        assert!(near(
            goblin.damage[0][0],
            (21.0 + (11.0 * a0).trunc() * 0.01) * factor
        ));
        assert!(near(goblin.attack, without.attack + 2.5));
        assert!(near(goblin.defence, without.defence + 4.0));
        // A bonus against an opponent's class or for another hero's type does not count.
        let mut slots = [Bonus::default(); BONUSES];
        slots[0] = Bonus {
            condition: 0x0205,
            effect: 811,
            value: 14,
        };
        let other = ItemValues {
            bonuses: slots,
            ..plain
        };
        assert_eq!(
            combat(&a_goblin(), 1, held(vec![other]), &[], &rules).speed,
            without.speed
        );
    }

    #[test]
    fn an_armed_creature_takes_its_weapons_damage_for_its_bonuses() {
        let rules = Rules::default();
        let dagger = ItemValues {
            damage: [[12.0, 15.0], [0.0; 2], [0.0; 2], [0.0; 2]],
            attack: 10.0,
            ..ItemValues::default()
        };
        let held = Holds {
            weapon: true,
            shield: false,
            ranged: false,
            items: vec![dagger],
        };
        let goblin = combat(&a_goblin(), 1, held.clone(), &[], &rules);
        // With a ranged weapon the physical multiplier takes `b2` for `b0` (#492): the goblin's
        // `b2` of 22 for its `b0` of 15.
        let archer = combat(
            &a_goblin(),
            1,
            Holds {
                ranged: true,
                ..held.clone()
            },
            &[],
            &rules,
        );
        let part = |b: f32| {
            let f = (156.0 - rules.stat_offset) / 156.0 + rules.stat_offset + 9.0;
            f * b / 10.0 / rules.damage_divisors[0]
        };
        let [b0, _, b2, ..] = a_goblin().attributes.map(f32::from);
        assert!(near(
            archer.damage[0][1] / goblin.damage[0][1],
            (1.0 + part(b2)) / (1.0 + part(b0))
        ));
        // 12 to 15 times the goblin's multiplier, 1.204; its attack 18.5 + 10.
        assert!(
            near(goblin.damage[0][0], 12.0 * 1.2037) && near(goblin.damage[0][1], 15.0 * 1.2037)
        );
        assert!(near(goblin.attack, 28.5));
    }

    #[test]
    fn the_fills_make_bosses_and_champions_by_type_and_by_chance() {
        let rules = Rules::default();
        let mut rng = Rng::of(50, 39, 0);
        assert_eq!(Rank::of_fill(203, false, &mut rng, &rules), Rank::Champion);
        let coins: Vec<Rank> = (0..100)
            .map(|_| Rank::of_fill(48, false, &mut rng, &rules))
            .collect();
        assert!(coins.contains(&Rank::Boss) && coins.contains(&Rank::Champion));
        assert!(!coins.contains(&Rank::Common));
        // About 2 % of the hostile creatures, none of the peaceful.
        let champions = (0..10_000)
            .filter(|_| Rank::of_fill(76, true, &mut rng, &rules) == Rank::Champion)
            .count();
        assert!((120..290).contains(&champions), "{champions}");
        assert!((0..1000).all(|_| Rank::of_fill(76, false, &mut rng, &rules) == Rank::Common));
    }

    #[test]
    fn a_fills_343_and_212_take_a_champions_levelling_alone() {
        // `FillSector` makes them at the marker level 1 with `+0xB0` bit `0x400` and no mark,
        // hostile or not (#621).
        let rules = Rules::default();
        let mut rng = Rng::of(50, 39, 0);
        for (creature, hostile) in [(343, true), (343, false), (212, true), (212, false)] {
            assert_eq!(
                Rank::of_fill(creature, hostile, &mut rng, &rules),
                Rank::Levelled
            );
        }
        // The strike levels it as a champion; the mark's factors stay a common creature's.
        let values = SpawnValues {
            density: 50,
            lowest: 4,
            highest: 7,
        };
        assert_eq!(
            Rank::Levelled.level(1, 5, &values, false, &rules),
            Rank::Champion.level(1, 5, &values, false, &rules)
        );
        assert_eq!(Rank::Levelled.health(343, 2, &rules), 1.0);
        assert_eq!(Rank::Levelled.fight(&rules), [1.0, 1.0]);
        assert_eq!(Rank::Levelled.experience(), 1.0);
        // Its strikes at the hero skip the factor of its level, as a champion's do (#615).
        assert_eq!(
            weapons_factor(Some(Side::Defender), 1, Rank::Levelled, &rules),
            1.0
        );
        assert!(weapons_factor(Some(Side::Defender), 1, Rank::Common, &rules) < 1.0);
    }

    #[test]
    fn a_boss_or_champion_takes_the_heros_level_plus_two_within_its_sectors() {
        let rules = Rules::default();
        let values = SpawnValues {
            density: 50,
            lowest: 4,
            highest: 7,
        };
        assert_eq!(Rank::Boss.level(5, 1, &values, false, &rules), 4);
        assert_eq!(Rank::Champion.level(5, 6, &values, false, &rules), 8);
        // At most the sector's highest plus two, but for a big type.
        assert_eq!(Rank::Boss.level(5, 20, &values, false, &rules), 9);
        assert_eq!(Rank::Boss.level(5, 20, &values, true, &rules), 22);
        assert_eq!(Rank::Common.level(5, 20, &values, false, &rules), 5);
        // Its factors.
        assert!((Rank::Boss.health(76, 8, &rules) - 2.5).abs() < 1e-6);
        assert!((Rank::Champion.health(137, 8, &rules) - 6.0).abs() < 1e-6);
        assert_eq!(Rank::Boss.fight(&rules), [1.5, 2.0]);
        assert!((Rank::Champion.experience() - 1.5).abs() < 1e-6);
        assert_eq!(Rank::Champion.earned(25), 37);
        assert_eq!(Rank::Boss.earned(25), 50);
        assert_eq!(Rank::Common.earned(25), 25);
    }

    #[test]
    fn a_creature_takes_on_a_hero_from_whom_it_is_worth_experience() {
        let balance = (0.85, 1.0);
        // A hero of level 1 is taken on by every creature; of level 5, by those of 3 and up; of
        // level 20, by those of 16 and up.
        assert!((1..50).all(|level| takes_on(level, 1, balance)));
        assert!(!takes_on(2, 5, balance) && takes_on(3, 5, balance));
        assert!(!takes_on(15, 20, balance) && takes_on(16, 20, balance));
    }

    #[test]
    fn a_fills_run_takes_one_level_the_heros_raised_to_its_sectors_lowest_and_jittered() {
        /// Rolls that give the values they hold, in order.
        struct Rolls(Vec<u32>);
        impl Dice for Rolls {
            fn below(&mut self, n: u32) -> u32 {
                let roll = self.0.remove(0);
                assert!(roll < n, "{roll} below {n}");
                roll
            }
        }
        fn fill(values: &SpawnValues, hero: u32, minutes: u32, rolls: &[u32]) -> u32 {
            let mut dice = Rolls(rolls.to_vec());
            let level = fill_level(values, hero, minutes, &mut dice);
            assert!(dice.0.is_empty(), "rolls left: {:?}", dice.0);
            level
        }
        let values = |lowest, highest| SpawnValues {
            density: 50,
            lowest,
            highest,
        };
        // A hero of level 1 in 48,39 (25 to 38): from 25, by `(r % 4 + r % 4)/2 − 1`, 25 in 7
        // cases of 16, as the original's fills come out (#582).
        let west = values(25, 38);
        assert_eq!(fill(&west, 1, 0, &[0, 1]), 24);
        assert_eq!(fill(&west, 1, 0, &[1, 2]), 25);
        assert_eq!(fill(&west, 1, 0, &[3, 2]), 26);
        assert_eq!(fill(&west, 1, 0, &[3, 3]), 27);
        let at_25 = (0..4)
            .flat_map(|a| (0..4).map(move |b| [a, b]))
            .filter(|rolls| fill(&west, 1, 0, rolls) == 25)
            .count();
        assert_eq!(at_25, 7);
        // The start (1 to 4): 1 or 2 by one roll.
        assert_eq!(fill(&values(1, 4), 1, 0, &[0]), 1);
        assert_eq!(fill(&values(1, 4), 1, 0, &[1]), 2);
        // A hero above the highest keeps its level and its band here: 40 by −2 to 3.
        assert_eq!(fill(&west, 40, 0, &[0, 0]), 38);
        assert_eq!(fill(&west, 40, 0, &[5, 5]), 43);
        // A lowest not below the highest gives the highest.
        assert_eq!(fill(&values(10, 8), 1, 0, &[1, 1]), 8);
        // From 90 by −3 to 6, none from 250; the time played as `CreateNPC`'s, at most 300.
        assert_eq!(fill(&values(1, 300), 100, 0, &[9, 9]), 106);
        assert_eq!(fill(&values(1, 300), 290, 0, &[]), 290);
        assert_eq!(fill(&values(1, 300), 290, 121, &[]), 300);
        assert_eq!(fill(&values(1, 100), 50, 121, &[2, 3]), 52);
    }

    #[test]
    fn a_fills_creature_takes_its_runs_level_one_or_two_more_in_a_dungeon_held_at_the_highest() {
        let rules = Rules::default();
        let west = SpawnValues {
            density: 50,
            lowest: 25,
            highest: 38,
        };
        let mut rng = Rng::of(48, 39, 0);
        let wolf = (568, false);
        assert_eq!(filled_level(25, false, &west, wolf, &rules, &mut rng), 25);
        // Held at the highest by the level's setter (#562), but a horse.
        assert_eq!(filled_level(43, false, &west, wolf, &rules, &mut rng), 38);
        assert_eq!(
            filled_level(43, false, &west, (554, true), &rules, &mut rng),
            43
        );
        // In a dungeon one or two more, a roll each (#570), held there too.
        let dungeon: Vec<u32> = (0..100)
            .map(|_| filled_level(25, true, &west, wolf, &rules, &mut rng))
            .collect();
        assert!(dungeon.contains(&26) && dungeon.contains(&27));
        assert!(dungeon.iter().all(|level| (26..=27).contains(level)));
        assert_eq!(filled_level(38, true, &west, wolf, &rules, &mut rng), 38);
    }

    #[test]
    fn a_scripts_creature_takes_the_heros_level_within_its_sectors_at_most_their_highest() {
        /// A roll below 2 that always gives the value it holds.
        struct Roll(u32);
        impl Dice for Roll {
            fn below(&mut self, n: u32) -> u32 {
                assert_eq!(n, 2);
                self.0
            }
        }
        let values = |lowest, highest| SpawnValues {
            density: 50,
            lowest,
            highest,
        };
        // The blacksmith 688, a creature that is no horse.
        fn npc(values: Option<&SpawnValues>, hero: u32, raise: Raise, roll: u32) -> u32 {
            scripted_level(values, hero, raise, 688, &Rules::default(), &mut Roll(roll))
        }
        let start = Raise::Played { minutes: 0 };
        // The blacksmith's plateau of 50,44 (4 to 7) and the trader of 64,41 (29 to 45) for a hero
        // of level 1, read in the original (#530); the start (1 to 4), within its levels.
        let smith = values(4, 7);
        assert_eq!(npc(Some(&smith), 1, start, 1), 4);
        assert_eq!(npc(Some(&values(29, 45)), 1, start, 1), 29);
        assert_eq!(npc(Some(&values(1, 4)), 1, start, 0), 1);
        assert_eq!(npc(Some(&values(1, 4)), 1, start, 1), 2);
        // Above its highest the highest; at it, one more held back at the highest by the
        // level's setter (#562).
        assert_eq!(npc(Some(&smith), 9, start, 1), 7);
        assert_eq!(npc(Some(&smith), 7, start, 1), 7);
        // A sector without its levels holds none and rolls none, and the setter holds its
        // creature at bronze's `LevelKap`, 0, and so at 1; one with a lowest of 0 rolls none.
        assert_eq!(npc(None, 3, start, 1), 1);
        assert_eq!(npc(Some(&values(0, 7)), 1, start, 1), 1);
        assert_eq!(npc(Some(&values(0, 7)), 9, start, 1), 7);
        // A horse by an eighth of the hero's riding, without the time played, and not held at
        // the sector's highest.
        let horse = |riding| Raise::Horse { riding };
        assert_eq!(npc(Some(&smith), 1, horse(7), 1), 4);
        assert_eq!(npc(Some(&smith), 1, horse(17), 1), 6);
        assert_eq!(npc(Some(&smith), 7, horse(17), 1), 10);
        // The time played: no more below 15 minutes, a per cent from 15, five from 121 minutes,
        // up to the sector's highest.
        let played = |minutes| Raise::Played { minutes };
        let high = values(40, 250);
        assert_eq!(npc(Some(&high), 100, played(14), 0), 100);
        assert_eq!(npc(Some(&high), 100, played(15), 0), 101);
        assert_eq!(npc(Some(&high), 50, played(120), 0), 52);
        // 50 × 1.05 + 0.5 by the original's 0.01 of a float lies just below 53.
        assert_eq!(npc(Some(&high), 50, played(121), 0), 52);
        assert_eq!(npc(Some(&high), 60, played(121), 0), 63);
        assert_eq!(npc(Some(&smith), 4, played(100_000), 0), 4);
        assert_eq!(npc(Some(&values(40, 100)), 100, played(121), 0), 100);
    }

    #[test]
    fn the_level_setter_holds_a_creature_at_its_sectors_highest_but_a_horse() {
        let rules = Rules::default();
        let start = SpawnValues {
            density: 50,
            lowest: 1,
            highest: 4,
        };
        assert_eq!(held_level(3, Some(&start), 295, false, &rules), 3);
        assert_eq!(held_level(5, Some(&start), 295, false, &rules), 4);
        // Without a highest, at bronze's `LevelKap` alone, 0, and so at 1.
        assert_eq!(held_level(5, None, 295, false, &rules), 1);
        // The type 701 at 300 wherever it stands; a horse at 250.
        assert_eq!(held_level(280, Some(&start), 701, false, &rules), 280);
        assert_eq!(held_level(400, Some(&start), 701, false, &rules), 300);
        assert_eq!(held_level(30, Some(&start), 554, true, &rules), 30);
        assert_eq!(held_level(300, None, 554, true, &rules), 250);
    }

    #[test]
    fn a_creatures_kills_raise_its_level_past_its_levels_end_as_the_setter_holds_it() {
        // The setter starts a creature at its level's start, so that one of level 13 needs 30,600
        // more for its level 14 (docs/re/fight.md, "A strike's resolution").
        assert_eq!(level_start(1), 0);
        assert_eq!(level_start(2), 300);
        assert_eq!(level_start(13), 113_700);
        assert_eq!(level_end(13) - level_start(13), 30_600);
        let mut points = level_start(13);
        assert_eq!(creature_earns(&mut points, 13, 116), None);
        assert_eq!(points, 113_816);
        assert_eq!(creature_earns(&mut points, 13, 30_484), Some(14));
        // Two levels at once; the top level's end holds the points.
        let mut points = level_start(2);
        assert_eq!(creature_earns(&mut points, 2, 5_600), Some(5));
        let mut points = level_start(216);
        assert_eq!(creature_earns(&mut points, 216, u64::from(u32::MAX)), None);
        assert_eq!(points, level_end(216) - 1);
        // The setter holds the level reached at the sector's highest, a boss's and a champion's
        // `LevelUpBoss` and `LevelUpChamp` above it, a big one's not at all.
        let rules = Rules::default();
        let start = SpawnValues {
            density: 50,
            lowest: 1,
            highest: 4,
        };
        let [boss, champion] = rules.ranks.level_up;
        assert_eq!(Rank::Common.held(5, Some(&start), 295, false, &rules), 4);
        assert_eq!(Rank::Levelled.held(5, Some(&start), 343, false, &rules), 4);
        assert_eq!(Rank::Common.held(400, None, 701, false, &rules), 300);
        assert_eq!(
            Rank::Boss.held(14, Some(&start), 689, false, &rules),
            4 + boss
        );
        assert_eq!(
            Rank::Champion.held(14, Some(&start), 76, false, &rules),
            4 + champion
        );
        assert_eq!(Rank::Boss.held(14, Some(&start), 689, true, &rules), 14);
        assert_eq!(Rank::Boss.held(3, Some(&start), 689, false, &rules), 3);
    }

    #[test]
    fn a_scripts_level_tag_gives_a_boss_or_its_level_within_its_sectors_highest() {
        let rules = Rules::default();
        let values = |lowest, highest| SpawnValues {
            density: 50,
            lowest,
            highest,
        };
        let start = values(1, 4);
        // 1000: a boss of the marker level 1, which then takes the hero's level plus two within
        // its sector's, as a fill's boss.
        assert_eq!(Rank::of_tag(BOSS_TAG), Rank::Boss);
        assert_eq!(
            tagged_level(BOSS_TAG, Some(&start), 577, false, &rules),
            Some(1)
        );
        assert_eq!(Rank::Boss.level(1, 1, &start, false, &rules), 3);
        // Any other: that level, with no hero's level, lowest or time played in it, at most its
        // sector's highest: the goodie 692 of 60,75 (6 to 10) at 6, the guards of the start
        // script's garrison in 52,39 (1 to 4) of the tag 5 at 4.
        assert_eq!(Rank::of_tag(6), Rank::Common);
        assert_eq!(
            tagged_level(6, Some(&values(6, 10)), 692, false, &rules),
            Some(6)
        );
        assert_eq!(
            tagged_level(1, Some(&values(6, 10)), 692, false, &rules),
            Some(1)
        );
        assert_eq!(tagged_level(5, Some(&start), 295, false, &rules), Some(4));
        // A horse at most 250, wherever it stands; the type 701 at most 300.
        assert_eq!(
            tagged_level(128, Some(&start), 554, true, &rules),
            Some(128)
        );
        assert_eq!(
            tagged_level(300, Some(&start), 554, true, &rules),
            Some(250)
        );
        assert_eq!(
            tagged_level(400, Some(&start), 701, false, &rules),
            Some(300)
        );
        // A sector without levels holds it at bronze's `LevelKap`, 0, and so at 1.
        assert_eq!(tagged_level(5, None, 295, false, &rules), Some(1));
        // 0 or none: the rule of the records without one.
        assert_eq!(Rank::of_tag(0), Rank::Common);
        assert_eq!(tagged_level(0, Some(&start), 295, false, &rules), None);
    }

    #[test]
    fn the_effect_53_turns_a_part_of_the_physical_damage_into_fire() {
        // The Daemon's 1098 as the original's driver showed it: 16.84 to 20.84 physical became
        // 14.84 to 17.84 and a fire damage of 2 to 3.
        let mut damage = [[16.84, 20.84], [0.0, 0.0], [0.0, 0.0], [0.0, 0.0]];
        let turns = Effect {
            whole: 1,
            level: 0x100,
            ..Effect::new(effects::TURNS, 0.871)
        };
        turn_damage(&mut damage, &turns);
        assert!((damage[0][0] - 14.84).abs() < 1e-4 && (damage[0][1] - 17.84).abs() < 1e-4);
        assert_eq!(damage[1], [2.0, 3.0]);
        // A kind beyond the four changes nothing.
        let mut same = [[10.0, 20.0]; KINDS];
        turn_damage(
            &mut same,
            &Effect {
                level: 0x500,
                ..turns
            },
        );
        assert_eq!(same, [[10.0, 20.0]; KINDS]);
    }
}
