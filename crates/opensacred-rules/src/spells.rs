//! The spells' cast and their objects (docs/re/arts.md, "The spells' cast", "The objects'
//! systems", "The objects' actions", "The Mage's spells", "The Seraphim's spells"; #672, #872 to
//! #874): the clip that a spell plays, the share of it at which the spell acts, the fireball's
//! flight, the effects that an object's action 6 gives, the healing's health, and the objects
//! that strike about themselves once a second. A spell's damages are the bolts' of its
//! record ([`crate::arts::bolt_damages`]) by the caster's spells' multipliers
//! ([`crate::fight::Combat::spells`]), and its regeneration starts as it acts.

use opensacred_data::tables::Spell;

use crate::fight::{Effect, to_f32};

/// The Mage's spells: the stone skin, `STEINHAUT`; the fireball, `FEUERBALL`, his first; the
/// healing, `HEILKRAFT`; and the regeneration, `REIKI`.
pub const STONE_SKIN: u16 = 2;
pub const FIREBALL: u16 = 5;
pub const HEALING: u16 = 7;
pub const REIKI: u16 = 32;

/// The Seraphim's striking spells: the lightning, `BLITZ_SERA`; the holy light,
/// `HIMMELSLEUCHTEN`; the blades of light, `LICHTKLINGEN`; and the light, `LICHT`.
pub const LIGHTNING: u16 = 13;
pub const HOLY_LIGHT: u16 = 15;
pub const BLADES_OF_LIGHT: u16 = 17;
pub const LIGHT: u16 = 20;

/// The Seraphim's strengthening spells: the crusader's strength, `GLAUBENSSTAERKE`, and the
/// holy shield, `LICHTSCHILD`.
pub const CRUSADERS_STRENGTH: u16 = 21;
pub const HOLY_SHIELD: u16 = 22;

/// The Seraphim's holding spells: the irritation, `IRRITATION`, and the conversion,
/// `BEKEHRUNG`.
pub const IRRITATION: u16 = 18;
pub const CONVERSION: u16 = 19;

/// The Mage's attacking spells: the fire spiral, `FEUERSPIRALE`; the purgatory, `FEGEFEUER`; the
/// hail of meteors, `METEORITENHAGEL`; the whirlwind, `WIRBELWIND`; the chain lightning,
/// `BLITZSCHLAG`; and the ice splinters, `EISSPLITTER`.
pub const SPIRAL: u16 = 11;
pub const PURGATORY: u16 = 23;
/// The Mage's other spells: the teleport, `PHASEN`; the water form, `WASSERFORM`; the flesh to
/// stone, `FLEISCHSTEIN`; and the ring of frost, `FROSTRING`.
pub const TELEPORT: u16 = 1;
pub const WATER_FORM: u16 = 4;
pub const STONE: u16 = 25;
pub const FROST_RING: u16 = 35;
/// The Mage's banishing circle (`BANNKREIS`, `0x522DE3`).
pub const BANISHING: u16 = 16;

/// The Wood Elf's summoning of her unicorn (`SUMMONBEAR`, `0x523413`), at most at the level 250
/// (`0x564D60`).
pub const SUMMON: u16 = 3;

/// The creature type that a summoning makes for a caster of type `kind` (`0x426F60`): the
/// unicorn 72 for the Wood Elf (5), the wolf 588 for the Vampiress (6 and 7), else 545.
pub fn summoned(kind: u32) -> u32 {
    match kind {
        5 => 72,
        6 | 7 => 588,
        _ => 545,
    }
}

/// The Wood Elf's changeling (`MOUSE`, `0x525C8C`): the creatures it changes stand within a
/// squared 32,000 of her (`0x600730`), the first 20; each turns into one of 14 animals at random
/// (`0x795E20`): a rabbit, three pigs, three deer, a sheep and six chickens.
pub const CHANGELING: u16 = 36;
pub const CHANGES_WITHIN: f32 = 178.885;
pub const CHANGES_MOST: usize = 20;
pub const ANIMALS: [u32; 14] = [
    516, 520, 521, 522, 531, 532, 533, 544, 560, 561, 562, 563, 565, 566,
];
/// A boss's level's factor in the changeling's test (`0x795E20`).
const CHANGES_BOSS: f32 = 1.3;

/// Whether the changeling at the level `level` of the Wood Elf of level `hero` changes a
/// creature of `Creature.pak` class `class`, of a big type or not, of the level `creature`, a
/// boss or not (`0x525C8C`, `0x795E20`): of no big type nor the class 10, its level, a boss's
/// times 1.3, below `0.4·l + 5·L`.
pub fn changes(class: u8, big: bool, creature: u32, boss: bool, hero: u32, level: u32) -> bool {
    let creature = if boss {
        to_f32(creature) * CHANGES_BOSS
    } else {
        to_f32(creature)
    };
    class != 10 && !big && creature < 0.4 * to_f32(hero) + 5.0 * to_f32(level)
}

/// How long the changeling of `spell` at the level `level` keeps a creature changed, a boss or
/// not (`0x795E20`): `v₀` seconds, a third for a boss.
pub fn changed_seconds(spell: &Spell, level: u32, boss: bool) -> f32 {
    let seconds = to_f32(value(spell, 0, level));
    if boss { seconds / 3.0 } else { seconds }
}

/// The Wood Elf's spells: the speed, the bubble, the thorns, the plant cage, the lost souls and
/// the plants' healing.
pub const SPEED: u16 = 37;
pub const BUBBLE: u16 = 38;
pub const THORNS: u16 = 39;
pub const CAGE: u16 = 40;
pub const SOULS: u16 = 41;
pub const PLANT_HEAL: u16 = 42;

/// The bubble (`0x798380`, `0x5AC138`): its speed, its time (its model's clip's, not read:
/// **estimate**), how often and how near it strikes, and its burst's reach and factor (906,
/// `0x5AEE99`).
pub const BUBBLE_COURSE: (f32, f32, f32) = (100.0, 4.0, 25.0);
pub const BUBBLE_BEAT: f32 = 0.5;
pub const BURST: (f32, f32) = (300.0, 2.0);
/// The thorns about their centre along each axis, and their reach (`0x5260CB`, `0x5ADBCF`).
pub const THORNS_SCATTER: u32 = 75;
pub const THORNS_REACH: f32 = 50.0;
/// The souls about the Wood Elf, their first angle, their speed and reach (`0x5268E5`,
/// `0x79D310`, `0x5AEAC4`).
pub const SOULS_FROM: f32 = 50.0;
pub const SOULS_ANGLE: f32 = 2.6416;
pub const SOULS_SPEED: f32 = 150.0;
pub const SOULS_REACH: f32 = 20.0;

/// The effect that holds a creature, its time's factor 0 (`0x583485`, `0x537160`).
pub const HELD: u32 = 15;
/// The teleport's step along its line, in world units, and when it puts the caster at its end,
/// in seconds over the cast's rate (`0x522742`, the system `0x771410`).
pub const TELEPORT_STEP: f32 = 10.0;
pub const TELEPORT_TIME: f32 = 3.35;
/// The water form's overlay: the 2 s of its state 3 after its value 0 (`0x44A520`).
pub const WATER_AFTER: f32 = 2.0;
/// The ring of frost's eight objects about the caster: their distance, the first's angle off π
/// and the steps between them, their reach with the creature's size, and their life
/// (`0x52555A`, `0x5ACA87`).
pub const FROST_FROM: f32 = 170.0;
pub const FROST_ANGLE: f32 = std::f32::consts::PI - 0.5;
pub const FROST_STEP: f32 = std::f32::consts::FRAC_PI_4;
pub const FROST_REACH: f32 = 60.0;
pub const FROST_LIFE: f32 = 25.0;
/// How long the ring of frost's zone slows a creature that stands in it, given again while it
/// stays (`0x59F960`, `0x5941A0`).
pub const FROST_SLOWS_FOR: f32 = 1.0;

/// The time's factor that the ring of frost's zone at the level `level` gives a creature of the
/// level `creature`, a boss or not, of the class 10 or not (`0x59F960`, the effect 15):
/// `clamp(M/(15·L), 0.3, 0.8)`, `M` doubled for a boss and again for the class 10 (**hypothesis**:
/// the doubling is the level's, before the clamp).
pub fn frost_slow(level: u32, creature: u32, boss: bool, class10: bool) -> f32 {
    let mut m = to_f32(creature);
    if boss {
        m *= 2.0;
    }
    if class10 {
        m *= 2.0;
    }
    (m / (15.0 * to_f32(level.max(1)))).clamp(0.3, 0.8)
}
pub const METEORS: u16 = 26;
pub const WHIRLWIND: u16 = 29;
pub const CHAIN: u16 = 33;
pub const SPLINTERS: u16 = 34;

/// The fire spiral's five flames (`0x523EDA`, `0x5A6B1E`): their angle's speed in radians a
/// second, how fast their radius's growth `v` rises to its most, and their reach with the
/// creature's size.
pub const SPIRAL_FLAMES: u8 = 5;
pub const SPIRAL_TURNS: f32 = 3.054;
pub const SPIRAL_GROWS: f32 = 150.0;
pub const SPIRAL_MOST: f32 = 175.0;
pub const SPIRAL_REACH: f32 = 50.0;
/// The purgatory's speed and how far it looks for a creature (`0x5A780B`).
pub const PURGATORY_SPEED: f32 = 40.0;
/// The meteors (`0x524A83`, `0x7866C0`): how far from the point each lands along each axis, its
/// most wait in seconds, its fall's time, and its reach with the creature's size.
pub const METEOR_SCATTER: i32 = 120;
pub const METEOR_WAIT: u32 = 800;
pub const METEOR_FALLS: f32 = 0.625;
pub const METEOR_REACH: f32 = 65.0;
/// The whirlwinds' speed and time, and the splinters' (`0x78A170`, `0x78D640`), with their
/// reaches.
pub const WHIRLWIND_COURSE: (f32, f32, f32) = (200.0, 2.0, 50.0);
pub const SPLINTERS_COURSE: (f32, f32, f32) = (150.0, 3.0, 40.0);
/// The zones (`0xAD2530`): how far from a zone's cell along each axis a creature stands in it
/// (`0x59F960`, `0x5941A0`); and how far beyond its place a creature that the banishing circle
/// takes runs away from its centre (`0x539F60`), with its runes' distance about the caster.
pub const ZONE_REACH: i32 = 5;
pub const BANISHED_RUNS: f32 = 700.0;
pub const RUNES_FROM: f32 = 200.0;

/// Whether the banishing circle at the level `level` takes at its cast a creature of the level
/// `creature`, a boss or not (`0x522DE3`, `0x59F960`): no boss, its level at most `5·L + 3`; one
/// that steps into it later it takes whatever its level (`0x5941A0`).
pub fn banishes(level: u32, creature: u32, boss: bool) -> bool {
    !boss && creature <= 5 * level + 3
}

/// Where a creature at `here` that the banishing circle about `centre` takes runs (`0x539F60`):
/// 700 beyond its place away from the centre, along the line (or along x from the centre itself).
pub fn banished_to([x, y]: [f32; 2], [cx, cy]: [f32; 2]) -> [f32; 2] {
    let (dx, dy) = (x - cx, y - cy);
    let d = (dx * dx + dy * dy).sqrt();
    let (ux, uy) = if d > f32::EPSILON {
        (dx / d, dy / d)
    } else {
        (1.0, 0.0)
    };
    [x + ux * BANISHED_RUNS, y + uy * BANISHED_RUNS]
}

/// A boss's level's factor in the whirlwind's test of a creature it knocks back (`0x5AD135`).
const BOSS_LEVEL: f32 = 1.3;

/// How far the whirlwind of `spell` at the level `level` knocks a creature back, and the level
/// below which it does, its caster of the level `caster` (`0x5AD135`): `⌊v₁·f⌋`, and
/// `5·L + 0.4·l`.
pub fn whirlwind_knocks(spell: &Spell, level: u32, caster: u32) -> (f32, f32) {
    let left = to_f32(value(spell, 1, level));
    (left, 5.0 * to_f32(level) + 0.4 * to_f32(caster))
}

/// Whether a whirlwind whose test's level is `below` knocks back a creature of the level
/// `level`, a boss or not, of a big type or not (`0x5AD135`): its level, a boss's times 1.3,
/// below it, and no big type; the event of the kind 3 comes before the strike's.
pub fn whirlwind_knocks_back(below: f32, level: u32, boss: bool, big: bool) -> bool {
    let level = if boss {
        to_f32(level) * BOSS_LEVEL
    } else {
        to_f32(level)
    };
    !big && level < below
}
/// How long the chain lightning waits before it leaps on, and how far it looks (`0x77E2A0`,
/// `0x5B46CA`).
pub const CHAIN_LEAPS: f32 = 0.2;
pub const CHAIN_LOOKS: f32 = 316.2;

/// The fire spiral's flame's radius about its centre, `v` its growth and `angle` its angle
/// (`0x5A6B1E`): `(v/175)·(80 + 40·cos a)`.
pub fn spiral_radius(v: f32, angle: f32) -> f32 {
    v / SPIRAL_MOST * (80.0 + 40.0 * angle.cos())
}

/// How many objects of `spell` at the level `level` (`0x4D26E0`, the factor 1): the meteors' and
/// the splinters' ratio of their values 0 and 1, the whirlwind's value 0, each from 1 to 20.
pub fn count(spell: &Spell, level: u32) -> u32 {
    let n = match spell.id {
        METEORS | SPLINTERS => ratio(spell, 0, 1, level),
        // The thorns' and the souls' value 0 over their value 2, held at 10 and 8.
        THORNS | SOULS => ratio(spell, 0, 2, level),
        // A cage for five of the value 0's seconds (`0x52634B`).
        CAGE => value(spell, 0, level).min(20) / 5,
        _ => value(spell, 0, level),
    };
    n.clamp(1, 20)
}

/// The angles off the line to the aim of the `n` whirlwinds (`0x524EA2`): `−0.05·n + 0.1·i` for
/// `i` from 1 to `n`; and of the `n` splinters at the distance `d` to the aim, held from 50 to
/// 300 (`0x5258B5`): `(15/d)·(i − n/2)` for `i` from 0.
pub fn fan(spell: u16, n: u32, distance: f32) -> Vec<f32> {
    match spell {
        WHIRLWIND => (1..=n)
            .map(|i| -0.05 * to_f32(n) + 0.1 * to_f32(i))
            .collect(),
        _ => {
            let step = 15.0 / distance.clamp(50.0, 300.0);
            (0..n)
                .map(|i| step * (to_f32(i) - to_f32(n) / 2.0))
                .collect()
        }
    }
}

/// The point that the teleport takes its caster to (`0x5228F0` to `0x522A98`): along the line
/// from `from` to `to` in steps of [`TELEPORT_STEP`], at most `points` of them, while each point
/// may be passed (`passes`), the last on which it may stand (`stands`); a caster stopping before
/// it passes `to`. `from` itself where none is.
pub fn teleport(
    from: [f32; 2],
    to: [f32; 2],
    points: u32,
    passes: impl Fn([f32; 2]) -> bool,
    stands: impl Fn([f32; 2]) -> bool,
) -> [f32; 2] {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let length = dx.hypot(dy);
    if length < 1e-7 {
        return from;
    }
    let step = [
        (TELEPORT_STEP * dx / length).trunc(),
        (TELEPORT_STEP * dy / length).trunc(),
    ];
    let mut last = from;
    for i in 0..points {
        let k = to_f32(i);
        let point = [from[0] + step[0] * k, from[1] + step[1] * k];
        let along = (point[0] - from[0]).hypot(point[1] - from[1]);
        if along > length || !passes(point) {
            break;
        }
        if stands(point) {
            last = point;
        }
    }
    last
}

/// The Mage's skins and shields: the fire skin, `FLAMMENHAUT`; the quickening, `KATARAKT`; the
/// gust, `WINDBOE`; the shield wall, `SCHILDWALL`; and the mana's spring, `AUE`.
pub const FIRE_SKIN: u16 = 24;
pub const QUICKENING: u16 = 27;
pub const GUST: u16 = 28;
pub const SHIELD_WALL: u16 = 30;
pub const SPRING: u16 = 31;

/// The Dwarf's spells: the greed, the burrowing, the dwarven armour and the dwarven steel.
pub const GREED: u16 = 55;
pub const MINES: u16 = 56;
pub const BURROWING: u16 = 57;
pub const ARMOUR: u16 = 58;
pub const STEEL: u16 = 59;

/// The Daemon's spells: the call of death, the force of hell, the tentacles and the fear.
pub const DISK: u16 = 60;
pub const DOOM: u16 = 62;
pub const FORCE: u16 = 63;
pub const HELLFIRE: u16 = 64;
pub const TENTACLES: u16 = 65;
pub const CHOIR: u16 = 66;
pub const FEAR: u16 = 67;

/// The hell disk (`0x7AE040`, `0x5A7DE0`, `0x5AF93F`): its speed, how far it looks for a
/// creature to home on (a squared 100,000), and how near it strikes one, with the creature's
/// size.
pub const DISK_SPEED: f32 = 250.0;
pub const DISK_LOOKS: f32 = 316.2;
pub const DISK_REACH: f32 = 50.0;
/// The hellfire's choice of a target (`0x5AFF86`): within 800 of the hero and 1,300 of it along
/// each axis, of a weight of 1 and 3 more with a ranged weapon; else within 300 and the
/// creature's size of it.
pub const HELLFIRE_HERO: f32 = 800.0;
pub const HELLFIRE_BOX: f32 = 1300.0;
pub const HELLFIRE_NEAR: f32 = 300.0;
/// The choir's reach about the Daemon (a squared 32,000) and the most creatures it takes
/// (`0x527DA9`).
pub const CHOIR_REACH: f32 = 178.9;
pub const CHOIR_MOST: usize = 20;

/// The call of death's effect, whose value multiplies the Daemon's strikes' damage while above
/// 1 (`0x56D1EC`), rising by his kills (`0x5339F3`) and falling with the time (`0x56373E`); its
/// word, which the adder keeps, is here its `level`.
pub const DOOMED: u32 = 46;
/// The tentacles' effect, under which each of his kills raises a tentacle at its corpse
/// (`0x533818`).
pub const TENTACLED: u32 = 41;
/// The fear's five objects about the Daemon: their distance from him, the first's angle, the
/// steps between them, their reach with the creature's size, their life, and how long the
/// effects they give hold (`0x528024`, `0x7B0AF0`, `0x5AFD26`).
pub const FEAR_FROM: f32 = 160.0;
pub const FEAR_ANGLE: f32 = 0.733;
pub const FEAR_STEP: f32 = std::f32::consts::TAU / 5.0;
pub const FEAR_REACH: f32 = 150.0;
pub const FEAR_LIFE: f32 = 25.0;
pub const FEAR_HOLDS: f32 = 1.0;
/// A tentacle's reach with the creature's size (`0x5AFE9C`).
pub const TENTACLE_REACH: f32 = 120.0;

/// The greed's effect (`crate::fight::effects::SHIELDS`): `D`'s multiplier times 100 over its
/// value, and its whole value the level at which the record's value 1 is the drops' `s`
/// (`0x53464E`; docs/re/items.md).
pub const GREEDY: u32 = 42;
/// The burrowing's effect, under which the Dwarf's pace is 0 (`0x56377E`).
pub const BURROWED: u32 = 58;
/// The steel's effect, which divides the physical resistance of what he strikes by its value
/// over 100 (`0x56C723`).
pub const PIERCES: u32 = 56;

/// The effect by which spells are turned aside (`0x1A`; docs/re/fight.md).
pub const TURNS_SPELLS: u32 = 0x1A;

/// How long the shield wall's object runs before it gives its effects, in seconds (at the
/// world's time).
pub const WALL_TIME: f32 = 1.0;

/// The effects by which they hold their target (the action 7, `0x5B5BB3`): 13, under which a
/// creature leaves the hero alone (`+0x1F4` bit `0x20000`; `crate::arts::DOMINATED`), and 14,
/// which turns its sides over (`+0x1F4` bit `0x40000`, `crate::minds::Mind::turned`).
pub const IRRITATED: u32 = 13;
pub const CONVERTED: u32 = 14;

/// The spells that the game makes.
pub const MADE: [u16; 48] = [
    STONE_SKIN,
    FIREBALL,
    HEALING,
    REIKI,
    SPIRAL,
    PURGATORY,
    TELEPORT,
    WATER_FORM,
    STONE,
    FROST_RING,
    BANISHING,
    SUMMON,
    CHANGELING,
    SPEED,
    BUBBLE,
    THORNS,
    CAGE,
    SOULS,
    PLANT_HEAL,
    METEORS,
    WHIRLWIND,
    CHAIN,
    SPLINTERS,
    LIGHTNING,
    HOLY_LIGHT,
    BLADES_OF_LIGHT,
    LIGHT,
    CRUSADERS_STRENGTH,
    HOLY_SHIELD,
    IRRITATION,
    CONVERSION,
    FIRE_SKIN,
    QUICKENING,
    GUST,
    SHIELD_WALL,
    SPRING,
    GREED,
    MINES,
    BURROWING,
    ARMOUR,
    STEEL,
    DISK,
    DOOM,
    FORCE,
    HELLFIRE,
    TENTACLES,
    CHOIR,
    FEAR,
];

/// The Dwarf's mines (`0x7BA8D0`, `0x7BB0B0`, `0x5B0719`, `0x5B094A`): how near another mine keeps
/// the sign from laying one, how near a hostile creature sets one off (its size and the
/// creature's), how far from it he stands for that, how long it lies before it goes off by
/// itself, and how far its explosion (the object 794, `0x5ADFD3`) strikes, with the creature's
/// size.
pub const MINES_APART: f32 = 50.0;
pub const MINE_REACH: f32 = 90.0;
pub const MINE_SAFE: f32 = 100.0;
pub const MINE_LIFE: f32 = 30.0;
pub const BLAST_REACH: f32 = 80.0;

/// The spells that a cast refuses while an effect keyed by the spell holds on its target, the
/// caster for the launch types 0 and 5 or without a creature for its target (`0x51D840` at
/// `0x51DA76`, `0x582B00`): their effect `0x22` among them.
const LOCKED: [u16; 15] = [2, 17, 18, 19, 20, 21, 22, 24, 25, 27, 28, 30, 31, 32, 37];

/// The keys whose effects refuse a cast of `spell` while one holds on its target: the spell
/// itself, for the spells that last; none for another.
pub fn locked_by(spell: u16) -> &'static [u16] {
    match LOCKED.iter().position(|&locked| locked == spell) {
        Some(at) => &LOCKED[at..=at],
        None => &[],
    }
}

/// How long the crusader's strength's object runs before it gives its effects, in seconds
/// (its update `0x7853E0`, then the action 6; **hypothesis**: at the world's time, its clock not
/// read as the stone skin's is).
pub const STRENGTH_TIME: f32 = 1.6;

/// The effect that the objects of 17 and 20 give their bearer for their time, keyed by the
/// spell (`0x784C90`, `0x7850D0`); what it does is not read.
pub const BORNE: u32 = 35;

/// How often an object that strikes about itself strikes, in seconds: the cooldown its case of
/// the hits sets (`+0x11C`, counted down by the update).
pub const BEAT: f32 = 1.0;

/// An object that strikes about itself once a [`BEAT`] while it lives: the creatures hostile to
/// its caster, alive, nearer it than its reach and their size, and of the `Creature.pak` classes
/// `classes` where it names some.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub reach: f32,
    pub classes: &'static [u8],
    /// Whether it follows its bearer, else stands where it was made.
    pub follows: bool,
}

/// The object of `spell` that strikes about itself (`0x5ABD80`): the holy light's 804 at its
/// point within 150 (`0x5AC972`), the blades of light's 814 about its bearer within 110
/// (`0x5ACCB1`), the light's 817 about its bearer within 200, of the classes 5, 9 and 11 alone,
/// the undead, the demons and the ghosts by their models' names (`0x5ADA5F`).
pub fn area(spell: u16) -> Option<Area> {
    match spell {
        HOLY_LIGHT => Some(Area {
            reach: 150.0,
            classes: &[],
            follows: false,
        }),
        BLADES_OF_LIGHT => Some(Area {
            reach: 110.0,
            classes: &[],
            follows: true,
        }),
        LIGHT => Some(Area {
            reach: 200.0,
            classes: &[5, 9, 11],
            follows: true,
        }),
        // The fire skin's 821 about its bearer within its size, 120 (`0x5AD3E1`), and the
        // gust's 826 within 125 (`0x5ACEBA`).
        FIRE_SKIN => Some(Area {
            reach: 120.0,
            classes: &[],
            follows: true,
        }),
        GUST => Some(Area {
            reach: 125.0,
            classes: &[],
            follows: true,
        }),
        _ => None,
    }
}

/// The effects that turn arrows and spells aside, which the objects of the fire skin, the gust
/// and the shield wall give their bearer as they start, keyed by the spell, of the level `level`,
/// for the value 0 at its level ([`crate::arrows::DEFLECTION`] for the three, and
/// [`TURNS_SPELLS`] for the shield wall; `0x78BE10`, `0x788D90`, `0x78B040`); none for another
/// spell. Their whole value, the system's `+0xAC`, is left out (**hypothesis**: the object's id).
pub fn turning(spell: &Spell, level: u32) -> Vec<(Effect, f32)> {
    let ids: &[u32] = match spell.id {
        FIRE_SKIN | GUST => &[crate::arrows::DEFLECTION],
        SHIELD_WALL => &[crate::arrows::DEFLECTION, TURNS_SPELLS],
        _ => &[],
    };
    let seconds = to_f32(value(spell, 0, level));
    ids.iter()
        .map(|&id| {
            let effect = Effect {
                id,
                whole: 0,
                value: 0.0,
                art: u32::from(spell.id),
                level: u16::try_from(level).unwrap_or(u16::MAX),
            };
            (effect, seconds)
        })
        .collect()
}

/// Whether the object of `spell` gives its bearer the effect [`BORNE`].
pub fn borne(spell: u16) -> bool {
    matches!(spell, BLADES_OF_LIGHT | LIGHT)
}

/// How long the lightning's sight lasts, in seconds (its particles, **estimate**).
pub const LIGHTNING_SHOWS: f32 = 0.25;

/// The effect by which the executor marks a spell that lasts on its bearer (`0x22`,
/// `0x539E70`), of no value, keyed by the spell, for its value 0 at its level; what reads it is
/// not read.
pub const LASTS: u32 = 0x22;

/// How long the stone skin's shield runs before it gives its effects, in seconds of its clock,
/// which runs at the frame's time times the cast's rate (its system's update, the action 6).
pub const SHIELD_TIME: f32 = 1.6;

/// Whether the game makes the spell `spell`.
pub fn made(spell: u16) -> bool {
    MADE.contains(&spell)
}

/// The slot of the clip that a hero on foot plays for the spell `spell` (`0x522030`): 155, 161,
/// 157 or 163 by its list, else 154.
pub fn slot(spell: u16) -> u16 {
    match spell {
        5 | 29 | 33 | 37 | 41 | 58 | 59 | 61 | 64 | 65 => 155,
        7 | 12 | 36 => 161,
        20 | 21 | 25 | 30 | 31 | 32 | 34 => 157,
        1 | 2 | 4 | 16 | 18 | 27 | 35 => 163,
        _ => 154,
    }
}

/// The share of its clip still to play below which the spell `spell` acts (`0x521D60`, its
/// doubles by the spell): 0.66 for the fireball, 0.56 for the stone skin (2), 0.5 for the
/// healing (7), 0.35 for the regeneration (32); none for 9 and 10, whose case acts otherwise
/// (not read), nor for the spells it does not know (44 to 48, 51, 54 and 68).
pub fn share(spell: u16) -> Option<f32> {
    Some(match spell {
        1 => 1.0,
        3 | 42 | 43 | 57 => 0.9,
        14 | 35 | 37 | 39 | 40 => 0.8,
        50 => 0.78,
        34 => 0.73,
        11 | 24 | 38 => 0.7,
        5 | 29 | 52 | 53 => 0.66,
        15 | 33 => 0.65,
        49 => 0.64,
        6 | 23 => 0.6,
        2 | 4 | 16 | 18 | 19 | 27 => 0.56,
        7 | 8 | 12 | 13 | 26 | 28 | 41 | 55 | 56 | 58 | 61 | 63..=67 | 72 => 0.5,
        69 => 0.44,
        17 | 20 | 21 | 22 | 25 | 30 | 31 | 32 | 36 => 0.35,
        59 | 60 | 71 => 0.2,
        62 => 0.1,
        70 => 0.95,
        _ => return None,
    })
}

/// How much of its clip still to play at which a cast that has acted goes back to standing, in
/// seconds (`0x521D60`'s sub-state 4).
pub const STANDS_WITHIN: f32 = 0.3;

/// Whether the spell `spell` turns its caster to its target's place as it starts (`0x51E110`):
/// the fireball alone of those read.
pub fn turns(spell: u16) -> bool {
    spell == FIREBALL
}

/// The fireball's speed in units a second at its level `level` for a hero's cast (its particle
/// system's start, `0x76BE80`): 150 + 10·L, at most 500.
pub fn fireball_speed(level: u32) -> f32 {
    (150.0 + 10.0 * to_f32(level)).min(500.0)
}

/// The fireball's most time in seconds at its level `level` (`0x76BE80`): 2 + 0.5·L.
pub fn fireball_life(level: u32) -> f32 {
    2.0 + 0.5 * to_f32(level)
}

/// Whether the executor gives the bearer of the spell `spell` the effect [`LASTS`] (`0x522480`;
/// docs/re/arts.md, "The spells' executor").
pub fn lasts(spell: u16) -> bool {
    matches!(
        spell,
        2 | 17 | 21 | 22 | 24 | 27 | 28 | 30 | 31 | 32 | 37 | 41
    )
}

/// The record's value `a` at the level `level` over its value `b`, truncated and held at the cap
/// of `a`, or the value `a` alone where `b` is 0 (`0x4D26E0`): how many objects some spells make.
pub fn ratio(spell: &Spell, a: usize, b: usize, level: u32) -> u32 {
    let raw = |j: usize| {
        let [base, step] = spell.values[j].map(u32::from);
        base + step * level
    };
    let whole = match raw(b) {
        0 => raw(a),
        b => raw(a) / b,
    };
    match spell.caps[a] {
        0 => whole,
        cap => whole.min(u32::from(cap)),
    }
}

/// The record's value `j` at the level `level`, `base + step·L`, held at its cap where it has
/// one (`0x4D26B0`).
pub fn value(spell: &Spell, j: usize, level: u32) -> u32 {
    let ([base, step], cap) = (spell.values[j], spell.caps[j]);
    let value = u32::from(base) + u32::from(step) * level;
    if cap > 0 {
        value.min(u32::from(cap))
    } else {
        value
    }
}

/// The effect [`LASTS`] of the spell `spell` at the level `level`, with its seconds.
pub fn lasting(spell: &Spell, level: u32) -> (Effect, f32) {
    let effect = Effect {
        id: LASTS,
        whole: 0,
        value: 0.0,
        art: u32::from(spell.id),
        level: 0,
    };
    (effect, to_f32(value(spell, 0, level)))
}

/// What an object's action 6 gives its target (`0x5B2600`; docs/re/arts.md, "The objects'
/// actions"): each of the record's pairs from the attack's on whose `base + step·L` is not 0, an
/// effect for the value 0 at the level in seconds, keyed by the spell: the attack's and the
/// defence's multipliers (the effects 1 and 2) of a hundredth of it, the four resistances (7 to
/// 10) of it, the speeds (28) of 1 plus a hundredth, the arts' regeneration speeds (11) and the
/// regeneration (12) of a hundredth.
pub fn effects(spell: &Spell, level: u32) -> Vec<(Effect, f32)> {
    let seconds = to_f32(value(spell, 0, level));
    (4..13)
        .filter_map(|pair| {
            let [base, step] = spell.effects[pair].map(u32::from);
            let of = to_f32(base + step * level);
            if of == 0.0 {
                return None;
            }
            let (id, value) = match pair {
                4 => (1, of / 100.0),
                5 => (2, of / 100.0),
                6..=9 => (u32::try_from(pair).ok()? + 1, of),
                10 => (28, 1.0 + of / 100.0),
                11 => (11, of / 100.0),
                _ => (12, of / 100.0),
            };
            let effect = Effect {
                id,
                whole: 0,
                value,
                art: u32::from(spell.id),
                level: 0,
            };
            Some((effect, seconds))
        })
        .collect()
}

/// The holy shield's effect on its bearer (its system's start `0x785DE0`, at `0x785EB8`): the
/// arrows turned aside ([`crate::arrows::DEFLECTION`]) of the level `level`, keyed by the spell,
/// for its value 0 at its level, 43 + 2·L at most 120; its whole value, the system's `+0xAC`, is
/// left out (**hypothesis**: the object's id).
pub fn holy_shield(spell: &Spell, level: u32) -> (Effect, f32) {
    let effect = Effect {
        id: crate::arrows::DEFLECTION,
        whole: 0,
        value: 0.0,
        art: u32::from(spell.id),
        level: u16::try_from(level).unwrap_or(u16::MAX),
    };
    (effect, to_f32(value(spell, 0, level)))
}

/// What the holding spell `spell` gives its target of the level `target`, cast at the level `level`
/// by a caster of the level `caster` (the action 7, `0x5B5BB3` and `0x5B5EE2`): the effect
/// [`IRRITATED`] for the irritation, [`CONVERTED`] for the conversion, of the whole value the
/// caster's level, keyed by the spell, for `trunc(v₀·f)·min(1, 5L/M)` seconds, `v₀` the value 0
/// at its level, `M` the target's level and `f` the irritation's factor `1/(t − c)` where the
/// caster's level `c` is below the target's `t` (`0x524196`; **hypothesis**: `t` the target's
/// level), else 1; none for another spell.
pub fn holding(spell: &Spell, level: u32, caster: u32, target: u32) -> Option<(Effect, f32)> {
    let id = match spell.id {
        IRRITATION => IRRITATED,
        CONVERSION => CONVERTED,
        _ => return None,
    };
    let factor = if spell.id == IRRITATION && caster < target {
        1.0 / f64::from(target - caster)
    } else {
        1.0
    };
    let whole = (f64::from(value(spell, 0, level)) * factor).trunc();
    let share = (5.0 * f64::from(level) / f64::from(target.max(1))).min(1.0);
    let effect = Effect {
        id,
        whole: i32::try_from(caster).unwrap_or(i32::MAX),
        value: 0.0,
        art: u32::from(spell.id),
        level: 0,
    };
    Some((effect, narrow(whole * share)))
}

/// A number of seconds as the float the effects keep.
#[expect(
    clippy::cast_possible_truncation,
    reason = "an effect's seconds, a few hundred at most"
)]
fn narrow(seconds: f64) -> f32 {
    seconds as f32
}

/// The Dwarf's own effects of `spell` at the level `level`, keyed by the spell, for its value 0
/// at its level (`0x7BB8D0`, `0x7B9C80`, `0x7BA240`): the greed's [`GREEDY`] of the whole value
/// `L` and the value of its defence's pair, 100 + 10·L; the burrowing's [`BURROWED`]; the steel's
/// [`PIERCES`] of its physical resistance's pair, 100 + 15·L, the effect 1 of its attack's pair
/// over 100 and the effect 2 of 0; none for another spell.
pub fn dwarfs(spell: &Spell, level: u32) -> Vec<(Effect, f32)> {
    let pair = |at: usize| {
        let [base, step] = spell.effects[at].map(u32::from);
        to_f32(base + step * level)
    };
    let effect = |id: u32, whole: u32, value: f32| Effect {
        id,
        whole: i32::try_from(whole).unwrap_or(i32::MAX),
        value,
        art: u32::from(spell.id),
        level: 0,
    };
    let effects = match spell.id {
        GREED => vec![effect(GREEDY, level, pair(5))],
        BURROWING => vec![effect(BURROWED, 0, 0.0)],
        STEEL => vec![
            effect(PIERCES, 0, pair(6)),
            effect(1, 0, pair(4) / 100.0),
            effect(2, 0, 0.0),
        ],
        _ => Vec::new(),
    };
    let seconds = to_f32(value(spell, 0, level));
    effects.into_iter().map(|e| (e, seconds)).collect()
}

/// The Daemon's own effects of `spell` at the level `level`, keyed by the spell, for its value 0
/// at its level: the call of death's [`DOOMED`] of the whole value `v₂`, the value 1 and the word
/// `v₁` (`0x527CFC`); the force of hell's effect 4 of `v₁/100`, 59 of its fire's pair and 1 of its
/// attack's pair over 100, from each of its two objects (`0x7C40E0`); the tentacles'
/// [`TENTACLED`] of the whole value `L` (`0x52829F`); none for another spell.
pub fn daemons(spell: &Spell, level: u32) -> Vec<(Effect, f32)> {
    let pair = |at: usize| {
        let [base, step] = spell.effects[at].map(u32::from);
        base + step * level
    };
    let effect = |id: u32, whole: u32, value: f32, word: u32| Effect {
        id,
        whole: i32::try_from(whole).unwrap_or(i32::MAX),
        value,
        art: u32::from(spell.id),
        level: u16::try_from(word).unwrap_or(u16::MAX),
    };
    let effects = match spell.id {
        DOOM => vec![effect(
            DOOMED,
            value(spell, 2, level),
            1.0,
            value(spell, 1, level),
        )],
        FORCE => {
            let one = [
                effect(4, level, to_f32(value(spell, 1, level)) / 100.0, 0),
                effect(59, pair(1), 0.0, 0),
                effect(1, 0, to_f32(pair(4)) / 100.0, 0),
            ];
            one.iter().chain(&one).copied().collect()
        }
        TENTACLES => vec![effect(TENTACLED, level, 0.0, 0)],
        _ => Vec::new(),
    };
    let seconds = to_f32(value(spell, 0, level));
    effects.into_iter().map(|e| (e, seconds)).collect()
}

/// The call of death's value after a kill of its holder, its word `word` (`0x5339F3`): a third
/// of the way from `value` to `1 + word/1000`.
pub fn doom_kills(value: f32, word: u16) -> f32 {
    value + (f32::from(word) - 1000.0 * (value - 1.0)) / 3000.0
}

/// The most that the call of death's value of the word `word` rises to by kills, `1 + w/1000`,
/// where [`doom_kills`] stands still (+90 % at the level 1, the word 900); the spell's tooltip
/// tells it.
pub fn doom_most(word: u16) -> f32 {
    1.0 + f32::from(word) / 1000.0
}

/// The time's factor (the effect 15) that the choir's objects give a creature of the level
/// `target`, the spell at the level `level` (`0x527DA9`, its object's action 9):
/// `clamp(2M/(15·L), 0.5, 1)`, a boss's level times 1.3 left to the caller; the spell's tooltip
/// tells it against the hero's own level.
pub fn choir_slows(target: u32, level: u32) -> f32 {
    let (m, l) = (f64::from(target), f64::from(level.max(1)));
    narrow((2.0 * m / (15.0 * l)).clamp(0.5, 1.0))
}

/// The call of death's value after `seconds`, falling by its whole value over 1000 a second to
/// no less than 1 (`0x56373E`).
pub fn doom_fades(value: f32, whole: i32, seconds: f32) -> f32 {
    let whole = i16::try_from(whole).map_or(f32::MAX, f32::from);
    (value - whole / 1000.0 * seconds).max(1.0)
}

/// What the fear's objects give a creature they strike, keyed by the spell, for
/// [`FEAR_HOLDS`] (`0x5AFD26`): the effects 1 and 2 of `−(1 − 100/p)`, `p` the record's attack's
/// and defence's pairs at the level `level`, 110 + 10·L.
pub fn fear(spell: &Spell, level: u32) -> Vec<(Effect, f32)> {
    [(1, 4), (2, 5)]
        .into_iter()
        .map(|(id, at)| {
            let [base, step] = spell.effects[at].map(u32::from);
            let p = to_f32(base + step * level).max(1.0);
            let effect = Effect {
                id,
                whole: 0,
                value: -(1.0 - 100.0 / p),
                art: u32::from(spell.id),
                level: 0,
            };
            (effect, FEAR_HOLDS)
        })
        .collect()
}

/// The drops' `s` of a killer that holds the greed's effect of the whole value `level`: the record
/// 55's value 1 at that level, 15 + 5·L (`0x53464E`).
pub fn treasure(greed: &Spell, level: u32) -> u32 {
    value(greed, 1, level)
}

/// The health that the healing's action 3 gives its target (`0x5B2600`): its value 0, 10 + 50·L,
/// times `1 + v/100`, `v` the caster's magic lore's per cent ([`crate::fight::magic_lore`]),
/// taken whole (**hypothesis**: the health added, as the record's label says).
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a health, whole and not negative, far below the integers' range"
)]
pub fn healing(spell: &Spell, level: u32, lore: f32) -> u32 {
    let health = f64::from(value(spell, 0, level)) * (1.0 + f64::from(lore) / 100.0);
    health.max(0.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spells_clip_is_its_lists_slot() {
        assert_eq!(slot(FIREBALL), 155);
        assert_eq!((slot(7), slot(32), slot(2), slot(18)), (161, 157, 163, 163));
        assert_eq!(slot(13), 154);
    }

    fn record(id: u16, effects: &[(usize, [u16; 2])], value: [u16; 2], cap: u16) -> Spell {
        let mut spell = Spell {
            id,
            name: 0,
            description: 0,
            short_description: 0,
            icon: String::new(),
            sprites: [0; 3],
            launch: 2,
            regeneration: [0; 2],
            effects: [[0; 2]; 13],
            values: [[0; 2]; 3],
            caps: [0; 3],
            element: 0,
            classes: 4,
            flags: 0,
        };
        for &(pair, of) in effects {
            spell.effects[pair] = of;
        }
        spell.values[0] = value;
        spell.caps[0] = cap;
        spell
    }

    #[test]
    fn the_stone_skin_and_the_regeneration_give_their_records_effects_for_their_value() {
        // GOG 2.28's 2: the defence 15 + 5·L, the physical resistance 25 + 40·L, the poison's
        // 10 + 15·L, for 44 + L seconds at most 120; 32: the regeneration 135 + 15·L for 70 + 2·L
        // at most 180.
        let skin = record(
            2,
            &[(5, [15, 5]), (6, [25, 40]), (9, [10, 15])],
            [44, 1],
            120,
        );
        let given: Vec<_> = effects(&skin, 1)
            .into_iter()
            .map(|(e, s)| (e.id, e.value, e.art, s))
            .collect();
        assert_eq!(
            given,
            [(2, 0.2, 2, 45.0), (7, 65.0, 2, 45.0), (10, 25.0, 2, 45.0)]
        );
        assert_eq!(effects(&skin, 90)[0].1, 120.0);
        let reiki = record(32, &[(12, [135, 15])], [70, 2], 180);
        let [(regeneration, seconds)] = effects(&reiki, 1)[..] else {
            panic!("one effect");
        };
        assert_eq!(
            (regeneration.id, regeneration.value, seconds),
            (12, 1.5, 72.0)
        );
        assert_eq!(
            lasting(&reiki, 1),
            (
                Effect {
                    id: LASTS,
                    whole: 0,
                    value: 0.0,
                    art: 32,
                    level: 0
                },
                72.0
            )
        );
        assert!(lasts(STONE_SKIN) && lasts(REIKI) && !lasts(HEALING) && !lasts(FIREBALL));
    }

    #[test]
    fn the_seraphims_lights_strike_about_them_once_a_second() {
        assert_eq!(
            [HOLY_LIGHT, BLADES_OF_LIGHT, LIGHT].map(|s| area(s).map(|a| (a.reach, a.follows))),
            [
                Some((150.0, false)),
                Some((110.0, true)),
                Some((200.0, true))
            ]
        );
        assert_eq!(area(LIGHT).unwrap().classes, [5, 9, 11]);
        assert!(area(LIGHTNING).is_none() && borne(LIGHT) && !borne(HOLY_LIGHT));
        // GOG 2.28's 15 lives 24 + L seconds, at most 60; 17 and 20 44 + L, at most 120.
        let light = record(HOLY_LIGHT, &[(2, [6, 9])], [24, 1], 60);
        assert_eq!((value(&light, 0, 1), value(&light, 0, 50)), (25, 60));
        assert_eq!(
            [LIGHTNING, HOLY_LIGHT, BLADES_OF_LIGHT, LIGHT].map(share),
            [Some(0.5), Some(0.65), Some(0.35), Some(0.35)]
        );
        assert_eq!((share(9), share(44), share(70)), (None, None, Some(0.95)));
    }

    #[test]
    fn the_crusaders_strength_raises_the_attack_and_the_shield_turns_arrows() {
        // GOG 2.28's 21: the attack 50 + 25·L for 44 + L at most 120; 22: 43 + 2·L at most 120.
        let strength = record(CRUSADERS_STRENGTH, &[(4, [50, 25])], [44, 1], 120);
        let [(attack, seconds)] = effects(&strength, 1)[..] else {
            panic!("one effect");
        };
        assert_eq!((attack.id, attack.value, seconds), (1, 0.75, 45.0));
        let shield = record(HOLY_SHIELD, &[], [43, 2], 120);
        let (deflection, seconds) = holy_shield(&shield, 3);
        assert_eq!(
            (deflection.id, deflection.art, deflection.level, seconds),
            (0x19, 22, 3, 49.0)
        );
        assert_eq!(holy_shield(&shield, 60).1, 120.0);
        assert!(lasts(CRUSADERS_STRENGTH) && lasts(HOLY_SHIELD));
    }

    #[test]
    fn the_irritation_and_the_conversion_hold_their_target_by_the_levels() {
        // GOG 2.28's 18 and 19: the value 0 14 + L, at most 30 and 25.
        let irritation = record(IRRITATION, &[], [14, 1], 30);
        // Seen: the Seraphim of level 1 on a goblin of level 5, the effect 13 of the whole
        // value 1 for 3 s, trunc(15/4).
        let (effect, seconds) = holding(&irritation, 1, 1, 5).unwrap();
        assert_eq!(
            (effect.id, effect.whole, effect.art, seconds),
            (13, 1, 18, 3.0)
        );
        // A caster of the target's level or above takes the whole value 0, by 5L/M at most 1.
        assert_eq!(holding(&irritation, 1, 10, 10).unwrap().1, 7.5);
        assert_eq!(holding(&irritation, 2, 10, 5).unwrap().1, 16.0);
        let conversion = record(CONVERSION, &[], [14, 1], 25);
        let (effect, seconds) = holding(&conversion, 1, 1, 5).unwrap();
        assert_eq!((effect.id, seconds), (14, 15.0));
        assert!(holding(&record(HOLY_SHIELD, &[], [43, 2], 120), 1, 1, 1).is_none());
    }

    #[test]
    fn the_mages_skins_and_shields_turn_arrows_and_strike_about_him() {
        // GOG 2.28's 24: the defence 15 + 5·L, the fire resistance 25 + 40·L, for 44 + L at most
        // 120; 30: the four resistances; 27: the attack and the speed, for 19 + L at most 60.
        let skin = record(
            FIRE_SKIN,
            &[(1, [2, 3]), (5, [15, 5]), (7, [25, 40])],
            [44, 1],
            120,
        );
        let given: Vec<_> = effects(&skin, 1)
            .iter()
            .map(|(e, _)| (e.id, e.value))
            .collect();
        assert_eq!(given, [(2, 0.2), (8, 65.0)]);
        let turns: Vec<_> = turning(&skin, 2)
            .iter()
            .map(|(e, s)| (e.id, e.level, *s))
            .collect();
        assert_eq!(turns, [(0x19, 2, 46.0)]);
        let wall = record(
            SHIELD_WALL,
            &[(6, [4, 6]), (7, [4, 6]), (8, [25, 40]), (9, [4, 6])],
            [44, 1],
            120,
        );
        let ids: Vec<_> = turning(&wall, 1).iter().map(|(e, _)| e.id).collect();
        assert_eq!(ids, [0x19, 0x1A]);
        assert_eq!(effects(&wall, 1).len(), 4);
        let quickening = record(QUICKENING, &[(4, [10, 7]), (10, [13, 2])], [19, 1], 60);
        let given: Vec<_> = effects(&quickening, 1)
            .iter()
            .map(|(e, s)| (e.id, e.value, *s))
            .collect();
        assert_eq!(given, [(1, 0.17, 20.0), (28, 1.15, 20.0)]);
        assert_eq!(area(FIRE_SKIN).map(|a| a.reach), Some(120.0));
        assert_eq!(area(GUST).map(|a| a.reach), Some(125.0));
        assert!(turning(&record(SPRING, &[], [70, 2], 180), 1).is_empty());
        // The spells that last refuse a cast while their effects hold.
        assert_eq!(
            (locked_by(FIRE_SKIN), locked_by(FIREBALL)),
            (&[24][..], &[][..])
        );
    }

    #[test]
    fn the_dwarfs_greed_burrowing_and_steel_give_their_effects() {
        // GOG 2.28's 55: the defence's pair (100, 10), the value 0 19 + L at most 60, the value 1
        // 15 + 5·L; 59: the attack (15, 10), the physical resistance (100, 15), 29 + L at most 90.
        let mut greed = record(GREED, &[(5, [100, 10])], [19, 1], 60);
        greed.values[1] = [15, 5];
        let [(effect, seconds)] = dwarfs(&greed, 2)[..] else {
            panic!("one effect");
        };
        assert_eq!(
            (effect.id, effect.whole, effect.value, seconds),
            (42, 2, 120.0, 21.0)
        );
        assert_eq!(treasure(&greed, 2), 25);
        let steel = record(STEEL, &[(4, [15, 10]), (6, [100, 15])], [29, 1], 90);
        let given: Vec<_> = dwarfs(&steel, 1)
            .iter()
            .map(|(e, s)| (e.id, e.value, *s))
            .collect();
        assert_eq!(given, [(56, 115.0, 30.0), (1, 0.25, 30.0), (2, 0.0, 30.0)]);
        let burrowing = record(BURROWING, &[(5, [30, 20])], [7, 1], 15);
        assert_eq!(dwarfs(&burrowing, 1)[0].0.id, 58);
        assert_eq!(effects(&burrowing, 1)[0].0.id, 2);
        assert!(dwarfs(&record(ARMOUR, &[], [44, 1], 120), 1).is_empty());
    }

    #[test]
    fn the_dwarfs_sign_carries_mines_by_the_ratio_of_its_values() {
        // GOG 2.28's 56: the value 0 400 + 224·L at most 11, the value 1 101 + 20·L: 5 mines at
        // level 1, 8 at level 10, 11 from level 178.
        let mut mines = record(MINES, &[(0, [8, 8]), (1, [7, 7])], [400, 224], 11);
        mines.values[1] = [101, 20];
        assert_eq!(
            [1, 10, 200].map(|level| ratio(&mines, 0, 1, level)),
            [5, 8, 11]
        );
        mines.values[1] = [0, 0];
        assert_eq!(ratio(&mines, 0, 1, 1), 11);
    }

    #[test]
    fn the_daemons_doom_rises_with_his_kills_and_falls_with_the_time() {
        // GOG 2.28's 62: the value 0 24 + L at most 60, 1 840 + 60·L, 2 56 + 4·L.
        let mut doom = record(DOOM, &[], [24, 1], 60);
        doom.values[1] = [840, 60];
        doom.values[2] = [56, 4];
        let [(effect, seconds)] = daemons(&doom, 1)[..] else {
            panic!("one effect");
        };
        assert_eq!(
            (effect.id, effect.whole, effect.value, effect.level, seconds),
            (46, 60, 1.0, 900, 25.0)
        );
        // Each kill a third of the way to 1.9, then the fall of 0.06 a second.
        let once = doom_kills(1.0, 900);
        assert!((once - 1.3).abs() < 1e-6);
        assert!((doom_kills(once, 900) - 1.5).abs() < 1e-6);
        assert!((doom_fades(1.3, 60, 2.0) - 1.18).abs() < 1e-6);
        assert_eq!(doom_fades(1.03, 60, 1.0), 1.0);
        // The most the kills raise it to: where a kill adds nothing, 1.9 (#1741).
        assert!((doom_most(900) - 1.9).abs() < 1e-6);
        assert!((doom_kills(doom_most(900), 900) - doom_most(900)).abs() < 1e-6);
        // The choir's slowing: 2M/(15·L) held to 0.5 to 1.
        assert!((choir_slows(30, 4) - 1.0).abs() < 1e-6);
        assert!((choir_slows(15, 2) - 1.0).abs() < 1e-6);
        assert!((choir_slows(12, 2) - 0.8).abs() < 1e-6);
        assert!((choir_slows(1, 5) - 0.5).abs() < 1e-6);
        // 63: the fire (4, 1), the attack (20, 20), the value 1 25 + 5·L, twice.
        let mut force = record(FORCE, &[(1, [4, 1]), (4, [20, 20])], [29, 1], 90);
        force.values[1] = [25, 5];
        let given: Vec<_> = daemons(&force, 1)
            .iter()
            .map(|(e, _)| (e.id, e.whole, e.value))
            .collect();
        assert_eq!(
            given,
            [
                (4, 1, 0.3),
                (59, 5, 0.0),
                (1, 0, 0.4),
                (4, 1, 0.3),
                (59, 5, 0.0),
                (1, 0, 0.4)
            ]
        );
        // 67: the attack and the defence (110, 10): −1/6 at level 1, for 1 s.
        let terror = record(FEAR, &[(4, [110, 10]), (5, [110, 10])], [25, 0], 0);
        let given = fear(&terror, 1);
        assert_eq!(given.len(), 2);
        assert!((given[0].0.value + 1.0 / 6.0).abs() < 1e-6 && given[1].1 == 1.0);
    }

    #[test]
    fn the_mages_meteors_splinters_and_whirlwinds_come_by_their_values() {
        // GOG 2.28's 26: 201 + 100·L over 93 + 7·L, at most 16; 34: 325 + 108·L over 103 + 5·L,
        // at most 21; 29: 6 + L at most 20.
        let mut meteors = record(METEORS, &[(0, [15, 9]), (1, [3, 2])], [201, 100], 16);
        meteors.values[1] = [93, 7];
        assert_eq!([1, 10, 20].map(|l| count(&meteors, l)), [3, 7, 9]);
        let mut splinters = record(SPLINTERS, &[(0, [2, 3]), (2, [2, 3])], [325, 108], 21);
        splinters.values[1] = [103, 5];
        assert_eq!([1, 10, 20].map(|l| count(&splinters, l)), [4, 9, 12]);
        let whirlwind = record(WHIRLWIND, &[(3, [6, 4])], [6, 1], 20);
        assert_eq!(count(&whirlwind, 1), 7);
        let winds = fan(WHIRLWIND, 7, 0.0);
        assert!((winds[0] + 0.25).abs() < 1e-6 && (winds[6] - 0.35).abs() < 1e-6);
        // Four splinters at 100: 0.15 apart, from −0.3.
        let shards = fan(SPLINTERS, 4, 100.0);
        assert!((shards[0] + 0.3).abs() < 1e-6 && (shards[3] - 0.15).abs() < 1e-6);
        assert!((fan(SPLINTERS, 2, 20.0)[0] + 0.3).abs() < 1e-6);
        // The spiral's flame at its full growth, 40 to 120.
        assert!((spiral_radius(175.0, 0.0) - 120.0).abs() < 1e-4);
        assert!((spiral_radius(175.0, std::f32::consts::PI) - 40.0).abs() < 1e-4);
    }

    #[test]
    fn the_teleport_goes_along_its_line_to_the_last_point_he_may_stand_on() {
        let open = |_: [f32; 2]| true;
        // Toward a point 300 off, 90 points: it stops before passing it.
        assert_eq!(
            teleport([0.0, 0.0], [300.0, 0.0], 90, open, open),
            [300.0, 0.0]
        );
        // Only 10 points: 90 at most.
        assert_eq!(
            teleport([0.0, 0.0], [300.0, 0.0], 10, open, open),
            [90.0, 0.0]
        );
        // A wall at 150 stops it; a stretch he may not stand on before is passed over.
        let wall = |p: [f32; 2]| p[0] < 150.0;
        let ground = |p: [f32; 2]| !(100.0..130.0).contains(&p[0]);
        assert_eq!(
            teleport([0.0, 0.0], [300.0, 0.0], 90, wall, ground),
            [140.0, 0.0]
        );
        assert_eq!(teleport([5.0, 5.0], [5.0, 5.0], 90, open, open), [5.0, 5.0]);
    }

    #[test]
    fn the_changeling_changes_the_weaker_creatures_for_its_seconds() {
        // GOG 2.28's 36: v₀ 14 + L at most 30.
        let changeling = record(CHANGELING, &[], [14, 1], 30);
        assert_eq!(changed_seconds(&changeling, 1, false), 15.0);
        assert_eq!(changed_seconds(&changeling, 1, true), 5.0);
        assert_eq!(changed_seconds(&changeling, 40, false), 30.0);
        assert!(changes(2, false, 5, false, 1, 1));
        assert!(!changes(2, false, 6, false, 1, 1));
        assert!(!changes(2, false, 5, true, 1, 1));
        assert!(!changes(2, true, 1, false, 1, 1));
        assert!(!changes(10, false, 1, false, 1, 1));
        assert_eq!(ANIMALS.len(), 14);
    }

    #[test]
    fn the_ring_of_frost_slows_a_creature_by_its_level_over_the_rings() {
        assert_eq!(frost_slow(1, 1, false, false), 0.3);
        assert_eq!(frost_slow(1, 9, false, false), 0.6);
        assert_eq!(frost_slow(1, 30, false, false), 0.8);
        assert_eq!(frost_slow(2, 9, true, false), 0.6);
        assert_eq!(frost_slow(3, 9, true, true), 0.8);
        let held = |value| Effect::new(15, value);
        assert_eq!(crate::fight::time_factor(&[]), 1.0);
        assert_eq!(crate::fight::time_factor(&[held(0.5), held(0.3)]), 0.3);
        assert_eq!(crate::fight::time_factor(&[held(0.0)]), 1e-4);
    }

    #[test]
    fn the_banishing_circle_sends_the_weaker_creatures_700_away() {
        assert!(banishes(1, 8, false));
        assert!(!banishes(1, 9, false));
        assert!(!banishes(10, 1, true));
        assert_eq!(banished_to([300.0, 400.0], [0.0, 0.0]), [720.0, 960.0]);
        assert_eq!(banished_to([5.0, 5.0], [5.0, 5.0]), [705.0, 5.0]);
    }

    #[test]
    fn the_whirlwind_knocks_back_a_creature_below_its_level() {
        // GOG 2.28's 29: v₁ 60 + 10·L at most 200.
        let mut whirlwind = record(WHIRLWIND, &[], [6, 1], 20);
        whirlwind.values[1] = [60, 10];
        whirlwind.caps[1] = 200;
        assert_eq!(whirlwind_knocks(&whirlwind, 1, 1), (70.0, 5.4));
        assert_eq!(whirlwind_knocks(&whirlwind, 20, 10).0, 200.0);
        assert!(whirlwind_knocks_back(5.4, 5, false, false));
        assert!(!whirlwind_knocks_back(5.4, 6, false, false));
        assert!(!whirlwind_knocks_back(5.4, 5, true, false));
        assert!(!whirlwind_knocks_back(5.4, 1, false, true));
    }

    #[test]
    fn the_wood_elfs_thorns_souls_and_cages_come_by_their_values() {
        // GOG 2.28's 39: 10 + 130·L at most 10 over 49 + 21·L; 41 the same at most 8; 40: 14 + L
        // at most 30.
        let mut thorns = record(THORNS, &[(2, [2, 1]), (3, [4, 3])], [10, 130], 10);
        thorns.values[2] = [49, 21];
        assert_eq!([1, 5, 10].map(|l| count(&thorns, l)), [2, 4, 5]);
        let mut souls = record(SOULS, &[(0, [9, 5]), (2, [5, 2])], [10, 130], 8);
        souls.values[2] = [49, 21];
        assert_eq!([1, 10, 100].map(|l| count(&souls, l)), [2, 5, 6]);
        let cage = record(CAGE, &[], [14, 1], 30);
        assert_eq!([1, 5, 6, 20].map(|l| count(&cage, l)), [3, 3, 4, 4]);
    }

    #[test]
    fn the_healing_heals_its_value_raised_by_the_magic_lore() {
        // 10 + 50·L, times 1.12 by the magic lore at level 1.
        let healing_record = record(7, &[], [10, 50], 0);
        assert_eq!(healing(&healing_record, 1, 12.0), 67);
        assert_eq!(healing(&healing_record, 2, 0.0), 110);
    }

    #[test]
    fn the_fireball_acts_at_two_thirds_of_its_clip_left_and_flies_faster_by_its_level() {
        assert_eq!(share(FIREBALL), Some(0.66));
        assert_eq!(share(10), None);
        // At level 1, 160 a second for 2.5 s, some 400; held at 500 from level 35.
        assert_eq!((fireball_speed(1), fireball_life(1)), (160.0, 2.5));
        assert_eq!((fireball_speed(35), fireball_speed(60)), (500.0, 500.0));
        assert!(made(FIREBALL) && made(REIKI) && !made(6));
    }
}
