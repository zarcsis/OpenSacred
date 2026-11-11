//! The creatures' casts (docs/re/ai.md, "The creatures' spells"; #1852): which types cast which
//! spells by their marks (`0x43C540`, the fight's specials `0x4D4E50`), the band they keep from
//! their target, their deadline between two casts and the rolls of their decisions. A creature
//! does not choose from its arts' list, which `Creature.pak` leaves empty: its spells are its
//! type's. The mark 1: the mummies, the magic skeletons and the orc shaman; the mark 4's dragons
//! and big demons; the mark 8's magicians; the casters of marks of their own.

use crate::spawning::Dice;
use crate::spells::{FIREBALL, WHIRLWIND};

/// `BLITZ_SERA`, the lightning of the mummy 03 (13).
pub const LIGHTNING: u16 = 13;
/// `METEORITENHAGEL`, the orc shaman's (26).
pub const METEORS: u16 = 26;
/// The flesh to stone (`FLEISCHSTEIN`, 25), which the mark 1's casters hold the hero with.
pub const STONE: u16 = 25;

/// What a caster of the mark 1 casts at its target (`0x4D51ED`'s path B): its spell and the
/// fireball's variant (`+0x13C`): the mummies 44, 64 and 65 `WIRBELWIND` (29), the mummy 03 (66)
/// the lightning (13), the magic skeletons 48 and 80 `FEUERBALL` (5) of the variants 1 and 2, the
/// orc shaman (67) `METEORITENHAGEL` (26); none for another type.
pub fn mark_1(creature: u32) -> Option<(u16, u8)> {
    Some(match creature {
        44 | 64 | 65 => (WHIRLWIND, 0),
        66 => (LIGHTNING, 0),
        48 => (FIREBALL, 1),
        80 => (FIREBALL, 2),
        67 => (METEORS, 0),
        _ => return None,
    })
}

/// Whether a creature of type `creature` is a dragon (`0x4D5B95`): of the mark 4 and of
/// `Creature.pak`'s class 10 (`0x426B70`), the types 52, 74, 88 and 100 to 103.
pub fn dragon(creature: u32) -> bool {
    matches!(creature, 52 | 74 | 88 | 100..=103)
}

/// How much farther than its reach a dragon casts its meteors from.
pub const DRAGON_BEYOND: f32 = 500.0;

/// Whether a dragon casts `METEORITENHAGEL` (26) at a decision (`0x4D5B95`): farther than its
/// reach `reach` and [`DRAGON_BEYOND`] from its target, by a roll `rand() % 2` of 0 (**hypothesis**
/// for the value: one decision of two), with no deadline and no test of the way.
pub fn dragon_casts(distance: f32, reach: f32, roll: u32) -> bool {
    distance > reach + DRAGON_BEYOND && roll.is_multiple_of(2)
}

/// Whether a creature of type `creature` is a big demon (`TYPE_NPC_BIG_DEMON`, 53 and 94), of the
/// mark 4.
pub fn big_demon(creature: u32) -> bool {
    matches!(creature, 53 | 94)
}

/// The big demons' own spells (docs/re/arts.md, "The creatures' own spells"): the eye lasers
/// (49), which land the demon's own strike on its target from afar where a straight line of
/// cells reaches it, and the demon's fire (50), a homing fire whose burst lands its strike on what
/// it touches; their records hold no damage.
pub const EYE_LASERS: u16 = 49;
pub const DEMON_FIRE: u16 = 50;

/// What a big demon casts at a decision of its melee (`0x4D5B95`), standing: in one decision of
/// three (a roll `rand() % 3` of 0; **hypothesis** for the value) with a straight way to its target
/// (`way`), the eye lasers in two cases of three (`choice`, a `rand() % 3` below 2) or the fire, with
/// no deadline; without the way, none: it runs about its target ([`big_demon_runs`]).
pub fn big_demon_casts(roll: u32, way: bool, choice: u32) -> Option<u16> {
    (roll.is_multiple_of(3) && way).then_some(if choice % 3 < 2 {
        EYE_LASERS
    } else {
        DEMON_FIRE
    })
}

/// Whether a big demon without a straight way runs about its target at a decision (`0x4D5B95`):
/// in the decision of three that would have cast.
pub fn big_demon_runs(roll: u32, way: bool) -> bool {
    roll.is_multiple_of(3) && !way
}

/// How many decisions a big demon stands facing its target after its run about it (`+0x129`).
pub const DEMON_STANDS: u8 = 50;

/// The distances of a big demon's casts as it comes on (`0x4E1F73`): from [`APPROACH_FROM`], the
/// eye lasers beyond [`LASERS_BEYOND`].
pub const APPROACH_FROM: f32 = 371.0;
pub const LASERS_BEYOND: f32 = 500.0;

/// What a big demon casts as it comes on to its target on its level (`0x4E0300`'s sub-state 1 at
/// `0x4E1F73`), at each tick: from [`APPROACH_FROM`] away, by a roll `rand() % 101` below 5, the
/// eye lasers beyond [`LASERS_BEYOND`], else the fire in four cases of five (`choice`, a
/// `rand() % 5` below 4) and the lasers in one.
pub fn big_demon_approaches(distance: f32, roll: u32, choice: u32) -> Option<u16> {
    approaches(distance, roll).then_some(if distance > LASERS_BEYOND || choice % 5 >= 4 {
        EYE_LASERS
    } else {
        DEMON_FIRE
    })
}

/// Whether a creature of the mark 4 that comes on to its target on its level makes its own at a
/// tick (`0x4E1F5C`): from [`APPROACH_FROM`] away, by a roll `rand() % 101` below 5.
pub fn approaches(distance: f32, roll: u32) -> bool {
    distance >= APPROACH_FROM && roll % 101 < 5
}

/// What a creature of type `creature` makes as it comes on (`0x4E1FC4`, the melee's state 9 at
/// `0x4E3464`): the medium spider (75, `med_spider.grn`), of the mark 4, shoots the railgun's bolt
/// at its target ([`SPIDER_SHOT`], #2067), a dragon breathes its fire ([`DRAGON_FIRE`], #1964),
/// and the giant spider (55) and the killer crab queen (348) spit ([`SPIT`], #2099); the fire
/// ghoul (73) takes the state too, its own not followed.
pub fn coming_on(creature: u32) -> Option<u16> {
    if creature == 75 {
        Some(SPIDER_SHOT)
    } else if matches!(creature, GIANT_SPIDER | CRAB_QUEEN) {
        Some(SPIT)
    } else if dragon(creature) {
        Some(DRAGON_FIRE)
    } else {
        None
    }
}

/// A dragon's breath of fire as it comes on, a pseudo-spell of its FX type (`FX_DRAGONFIRE`, 852,
/// `0x4E3B40`), at the dragon's head (the FX event `0x4C`'s bone 20), of the variant of its type
/// (`particles::dragon_fire_variant`); `FIREGHUL_FLAMEBREATH` (11122) at it at a rate's factor of
/// 0.5 as it is made (`0x4E3DB9`), and again from its start (`0x77AF2D`).
pub const DRAGON_FIRE: u16 = 852;
pub const FLAME_BREATH: u32 = 11122;

/// The spider's shot, a pseudo-spell of the railgun's FX type (`FX_RAILGUN`, 801), the clip's slot
/// it plays (`0x593320(0x4F, …)`, the dragons' fire's too), and its sounds as it shoots (`0x4E369E`): in four cases of five
/// (`rand() % 5` below 4) one of `GIANTSPIDER_FOOTSTEPS01`, `02`, `03` and `04`, at a rate's factor
/// of 0.5.
pub const SPIDER_SHOT: u16 = 801;
pub const SPIDER_SHOT_CLIP: u16 = 0x4F;
pub const SPIDER_SHOT_SOUNDS: [u32; 4] = [1248, 1249, 1242, 1243];

/// The sound of the spider's shot of the roll `roll` (`rand() % 5`), none for 4.
pub fn spider_shot_sound(roll: u32) -> Option<u32> {
    SPIDER_SHOT_SOUNDS
        .get(usize::try_from(roll % 5).ok()?)
        .copied()
}

/// The spit of the giant spider and the killer crab queen, a pseudo-spell of its FX type
/// (`FX_SPIDERPUKE`, 880, #2099; docs/re/effects.md, "The creatures' spit"): they take the melee's
/// state 9 as they come on, play [`SPIDER_SHOT_CLIP`] and spit once as the clip's share left falls
/// below 0.6 (`0x4E4CF0`'s sub-state 5, the double `0x890F48`), at [`SPIT_AT`] of it.
pub const SPIT: u16 = 880;
pub const GIANT_SPIDER: u32 = 55;
pub const CRAB_QUEEN: u32 = 348;
pub const SPIT_AT: f32 = 0.4;

/// Whether a creature of type `creature` is the eiterblume (186) or its spinneling (333).
pub fn flowers(creature: u32) -> bool {
    matches!(creature, 186 | 333)
}

/// The flowers' clip of their spit (`0x4E34D6`), and the share of it at which they spit: the
/// share left below 0.475 (the double `0x890F40`).
pub const FLOWERS_CLIP: u16 = 0x91;
const FLOWERS_SPIT_AT: f32 = 0.525;

/// The share of the clip at which a creature of type `creature` spits ([`SPIT_AT`], the
/// flowers' later).
pub fn spit_at(creature: u32) -> f32 {
    if flowers(creature) {
        FLOWERS_SPIT_AT
    } else {
        SPIT_AT
    }
}

/// What a flower does at a decision of its melee (the fight's specials `0x4D9674`), its target
/// `distance` away and its reach `reach`: nearer than one and a half reaches, truncated, it fights
/// in melee ([`Flower::Melee`]); else nearer than 400 it spits by a roll `rand() % 7` (`roll`) of 1,
/// and stands otherwise; nearer than 800 it leaps at its target by the same roll (the art 1043,
/// the Dark Elf's panther's leap), and stands otherwise; farther it stands. It never walks to its
/// target from beyond its melee.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flower {
    Melee,
    Spits,
    Leaps,
    Stands,
}

/// The flowers' leap, the Dark Elf's panther's leap (1043, `GUI_MOVE_PANTHER`) as a pseudo-spell of
/// their casts (#2231): ordered at the hero (`0x4D9674`), made by the Dark Elf's executor
/// (`0x506770`), its clip their slot 144 (`0x50A02D`, not the hero's 182), its leap as its clip's
/// share left falls below 0.7 (`arts::PANTHER_LEAPS`), along an arc three times as high as a
/// hero's (`0x507DC9`, the float `0x890768`) in half its clip's time, `SPELL_TURBOBOOST` at
/// the flower at the rate's factor 0.5; no strike.
pub const FLOWER_LEAP: u16 = 1043;
pub const FLOWER_LEAP_CLIP: u16 = 144;
pub const FLOWER_LEAP_HIGHER: f32 = 3.0;
pub const FLOWER_LEAP_SOUND: u32 = 2081;

/// The flowers' distances of the spit and of the leap (`0x4D9674`).
pub const FLOWER_SPITS_WITHIN: f32 = 400.0;
pub const FLOWER_LEAPS_WITHIN: f32 = 800.0;

/// Whether a flower `distance` from its target, of the reach `reach`, fights in melee, which
/// draws no roll.
pub fn flower_melee(distance: f32, reach: f32) -> bool {
    distance < (1.5 * reach).floor()
}

/// What a flower does beyond its melee by the roll `roll` (`rand() % 7`, [`Flower`]).
pub fn flower(distance: f32, roll: u32) -> Flower {
    let wins = roll % 7 == 1;
    if distance < FLOWER_SPITS_WITHIN && wins {
        Flower::Spits
    } else if (FLOWER_SPITS_WITHIN..FLOWER_LEAPS_WITHIN).contains(&distance) && wins {
        Flower::Leaps
    } else {
        Flower::Stands
    }
}

/// The killer crab queen's cages as it spits (`0x4E58EF`): by an odd `rand()` with a target, ten
/// objects 849 at the target's place, the plant cage (40) of the level `⌊L/3⌋`, at least 1, which
/// hold the target 3 s as a creature's cage does and strike nothing; `HELF_PFLANZENKONTROLLE` at
/// the first and the sixth.
pub const CRAB_CAGES: u32 = 10;
pub const CRAB_CAGES_HOLD: f32 = 3.0;

/// Whether the crab queen drops its cages on its target by the roll `roll` (`rand()`).
pub fn crab_cages(creature: u32, roll: u32) -> bool {
    creature == CRAB_QUEEN && roll & 1 == 1
}

/// The giant spider's young (`TYPE_NPC_MED_SPIDER`, 75), which it makes in place of its spit.
pub const YOUNG_SPIDER: u32 = 75;

/// Whether a spitter of type `creature` makes its young in place of its spit (`0x4E5A2A`): the
/// giant spider, where the spit's first roll `rand() % 3`, `roll`, is 2, in a game of one player.
pub fn bears_young(creature: u32, roll: u32) -> bool {
    creature == GIANT_SPIDER && roll % 3 == 2
}

/// How many young the giant spider makes (`0x4E5A2A`): a count from −1 rises by one at each, and
/// it stops once the count is above a roll `rand() % 3` (`roll`): two always, a third in two cases
/// of three, a fourth in one of three of those.
pub fn young(mut roll: impl FnMut() -> u32) -> u32 {
    let mut made = 0;
    loop {
        made += 1;
        if made > roll() % 3 + 1 {
            return made;
        }
    }
}

/// A young spider's level, of its mother's `level`: half of it, truncated, and one (`0x549500`).
pub fn young_level(level: u32) -> u32 {
    level / 2 + 1
}

/// How far ahead of the spitter, along its facing, its spit lands, in the models' units, of a
/// unit roll `unit` (`0x4E5B3B`): 300 + 100·U.
pub fn spit_reach(unit: f32) -> f32 {
    300.0 + 100.0 * unit
}

/// The spit's sounds as it is spat (`SOUND_FX_SPIDER_PUKE01` to `04`, its start `0x77B700`) and as
/// it lands (`SOUND_FX_SPIDER_PUKEIMPACT01` to `04`, its run `0x77B180`), each one of four by a
/// roll `rand() % 5` and none for 4, at the rate's factor `0.4 + 0.2·U`.
pub const SPIT_SOUNDS: [u32; 4] = [11227, 11228, 11229, 11230];
pub const SPIT_LANDS: [u32; 4] = [11231, 11232, 11233, 11234];

/// The sound of `sounds` of the roll `roll` (`rand() % 5`), none for 4.
pub fn spit_sound(sounds: [u32; 4], roll: u32) -> Option<u32> {
    sounds.get(usize::try_from(roll % 5).ok()?).copied()
}

/// The marks that a spit lays where it lands (779, `0x5B33AB`, `0x4271E0`): the green blood's of the
/// giant spider's gib set for the spiders' spit, the red blood's for the crab queen's, one of six
/// by a roll `rand() % 6`; each of a half side of 50, held 30 s.
pub const SPIT_MARKS: [[&str; 6]; 2] = [
    [
        "FX_BLOOD01_G",
        "FX_BLOOD02_G",
        "FX_BLOOD03_G",
        "FX_BLOOD04_G",
        "FX_BLOOD05_G",
        "FX_BLOOD06_G",
    ],
    [
        "FX_BLOOD01",
        "FX_BLOOD02",
        "FX_BLOOD03",
        "FX_BLOOD04",
        "FX_BLOOD05",
        "FX_BLOOD06",
    ],
];
pub const SPIT_MARK_HALF: f32 = 50.0;
pub const SPIT_MARK_HOLDS: f32 = 30.0;

/// The homing fires (docs/re/arts.md, "The creatures' own spells"; the object update's flag
/// `0x10000`): held at the caster's hand for their charge, then flying at [`HOMING_SPEED`] a second
/// toward their target for [`HOMING_FLIGHT`] seconds, turning by at most [`HOMING_TURN`] radians a
/// second; the demon's fire charges [`DEMON_FIRE_CHARGE`] seconds and is of the size
/// [`DEMON_FIRE_SIZE`].
pub const HOMING_SPEED: f32 = 250.0;
pub const HOMING_FLIGHT: f32 = 5.0;
pub const HOMING_TURN: f32 = 3.0;
pub const DEMON_FIRE_CHARGE: f32 = 2.5;
pub const DEMON_FIRE_SIZE: f32 = 70.0;

/// The clip's slot that a creature of type `creature` casts the spell `spell` with, on foot
/// (`0x522030`): by the spell, a few types their own, the dragons (`dragon`, `Creature.pak`'s
/// class 10) always 81, Anducar (182) and his demon (183) always 154.
pub fn clip(spell: u16, creature: u32, dragon: bool) -> u16 {
    if spell == SPIT && flowers(creature) {
        return FLOWERS_CLIP;
    }
    if spell == FLOWER_LEAP {
        return FLOWER_LEAP_CLIP;
    }
    if matches!(spell, SPIDER_SHOT | DRAGON_FIRE | SPIT) {
        return SPIDER_SHOT_CLIP;
    }
    if dragon {
        return 81;
    }
    if matches!(creature, 182 | 183) {
        return 154;
    }
    let ice = matches!(creature, 140 | 146);
    match spell {
        1 | 2 | 4 | 16 | 18 | 27 | 35 => 163,
        3 | 8 | 14 | 42 => 160,
        5 | 29 | 33 | 37 | 41 | 58 | 59 | 61 | 64 | 65 => 155,
        6 => 164,
        7 | 12 | 36 => 161,
        11 | 55 => 156,
        13 | 15 | 22 if ice => 147,
        13 | 28 | 40 | 56 | 66 => 158,
        17 if creature == 141 => 154,
        26 if creature == 183 => 154,
        17 | 20 | 21 | 22 | 25 | 26 | 30..=32 | 34 => 157,
        19 | 39 => 159,
        24 | 38 => 162,
        49 => 144,
        50 => 145,
        53 => 84,
        _ => 154,
    }
}

/// The band a caster of the mark 1 keeps from its target (`0x4E0300`'s sub-state 1): it walks to
/// [`BAND_POINT`] from its target where it stands nearer than [`BAND_NEAR`] after it was ordered
/// a cast, or farther than [`BAND_FAR`], while its decisions on its casting path stay below
/// [`BAND_MOVES`] (`+0x12A`).
pub const BAND_NEAR: f32 = 240.0;
pub const BAND_POINT: f32 = 270.0;
pub const BAND_FAR: f32 = 300.0;
pub const BAND_MOVES: u8 = 25;

/// Whether a caster at `distance` from its target walks back into its band, `ordered` a cast
/// before, `moves` its decisions on its casting path in this fight, this one counted.
pub fn leaves_band(distance: f32, ordered: bool, moves: u8) -> bool {
    moves < BAND_MOVES && ((distance < BAND_NEAR && ordered) || distance > BAND_FAR)
}

/// How far about its target a caster runs to where it neither casts nor keeps its band.
pub const STROLL: f32 = 280.0;

/// How many world's ticks a caster of the mark 1 stands facing its target after a decision
/// (`+0x129`), before it decides again.
pub const STANDS: u8 = 10;

/// The seconds of the real clock until a caster may cast again (`0x4D4670(1)`, the deadline
/// `+0x24D`): `⌊(6 + roll + hostiles) · (1 − level / 300) · b⌋`, `roll` a `rand() % 3`,
/// `hostiles` the hero's hostiles of the marks 8 and `0x200` fighting hand to hand, `b` 0.8 for a
/// boss or a champion (`elite`), else 1.
pub fn deadline(level: u32, elite: bool, hostiles: u32, roll: u32) -> u32 {
    let base = f64::from(6 + roll % 3 + hostiles);
    let level = 1.0 - f64::from(level) / 300.0;
    let elite = if elite { 0.8 } else { 1.0 };
    whole((base * level * elite).floor())
}

/// Whether a caster of the mark 1 casts at a decision (`0x4D51ED`): a roll `rand() % 3` of 2, a
/// straight way to its target, and its deadline passed.
pub fn casts(roll: u32, way: bool, deadline_passed: bool) -> bool {
    roll % 3 == 2 && way && deadline_passed
}

/// Whether a caster of the mark 1 takes its casting path at a decision rather than fighting hand
/// to hand (`0x4D51ED`): where the hero has more than one hostile and it is not the nearest, or
/// else in two cases of three (`roll` a `rand() % 3` not 0).
pub fn casting(crowded: bool, nearest: bool, roll: u32) -> bool {
    (crowded && !nearest) || !roll.is_multiple_of(3)
}

/// The distances from which a caster of the mark 1 or 8 holds the hero (`0x4D51ED`'s path A).
pub const HOLD_FROM: f32 = 400.0;
pub const HOLD_TO: f32 = 1_500.0;

/// Whether a caster of the mark 1 casts the stone (25) at the hero (`0x4D51ED`'s path A): from
/// [`HOLD_FROM`] to [`HOLD_TO`] away, a roll `rand() % (n + 1)` of 2 with `n` 3 and the hero's
/// hostiles of the marks 8 and `0x200` fighting hand to hand (`0x4D4670(2)`), its level above 5
/// and at least the hero's, the hero not held already and a straight way to it.
pub fn holds(
    distance: f32,
    (roll, hostiles): (u32, u32),
    (level, hero_level): (u32, u32),
    (held, way): (bool, bool),
) -> bool {
    (HOLD_FROM..=HOLD_TO).contains(&distance)
        && roll % (4 + hostiles) == 2
        && level > 5
        && level >= hero_level
        && !held
        && way
}

/// The marks that the creature's maker gives the types (`0x43C540`, kept at `+0x25F`), which the
/// fight's specials switch on (`0x4D4E50`): each run of types with its mark, by the types; any
/// other type has none.
const MARKS: &[(std::ops::RangeInclusive<u32>, u16)] = &[
    (34..=34, 0x1000),
    (35..=35, 0x2),
    (41..=43, 0x800),
    (44..=44, 0x1),
    (45..=45, 0x2),
    (46..=46, 0x10),
    (47..=47, 0x2),
    (48..=48, 0x1),
    (52..=53, 0x4),
    (55..=55, 0x4),
    (56..=56, 0x9),
    (57..=62, 0x2),
    (63..=63, 0x10),
    (64..=67, 0x1),
    (68..=68, 0x2),
    (70..=70, 0x2),
    (71..=71, 0x40),
    (72..=72, 0x800),
    (73..=75, 0x4),
    (76..=76, 0x400),
    (80..=80, 0x1),
    (81..=81, 0x1000),
    (88..=88, 0x4),
    (94..=94, 0x4),
    (96..=98, 0x2),
    (100..=103, 0x4),
    (105..=105, 0x8),
    (118..=118, 0x400),
    (125..=126, 0x10),
    (127..=127, 0x8),
    (130..=130, 0x10),
    (133..=133, 0x1FF),
    (140..=140, 0x208),
    (141..=141, 0x209),
    (143..=144, 0x8),
    (145..=145, 0x20B),
    (146..=146, 0x20A),
    (182..=183, 0x8),
    (184..=184, 0x212),
    (185..=185, 0x20C),
    (186..=186, 0x211),
    (187..=187, 0x218),
    (190..=190, 0x20F),
    (191..=191, 0x215),
    (192..=192, 0x20D),
    (195..=195, 0x216),
    (202..=202, 0x21A),
    (203..=203, 0x219),
    (204..=204, 0x21C),
    (205..=208, 0x218),
    (212..=212, 0x8),
    (216..=216, 0x8),
    (218..=219, 0x20E),
    (247..=247, 0x217),
    (248..=248, 0x215),
    (249..=249, 0x212),
    (250..=252, 0x20C),
    (253..=255, 0x218),
    (256..=256, 0xA),
    (258..=258, 0x8),
    (262..=270, 0x8),
    (271..=271, 0x20),
    (304..=306, 0x10),
    (311..=312, 0x8),
    (319..=321, 0x1FF),
    (326..=326, 0x1FF),
    (329..=329, 0x20D),
    (330..=332, 0x218),
    (333..=333, 0x211),
    (335..=335, 0x213),
    (336..=336, 0x214),
    (342..=343, 0x20D),
    (344..=344, 0x21A),
    (348..=348, 0x4),
    (350..=350, 0x21C),
    (351..=352, 0x219),
    (353..=353, 0x216),
    (354..=354, 0x217),
    (363..=363, 0x20D),
    (512..=512, 0x400),
    (567..=568, 0x400),
    (569..=569, 0x10),
    (570..=571, 0x400),
    (573..=583, 0x400),
    (693..=693, 0x8),
    (697..=697, 0x8),
    (700..=700, 0x8),
    (701..=701, 0x1FE),
];

/// The mark of a creature of type `creature` (`0x43C540`), 0 for none.
pub fn mark(creature: u32) -> u16 {
    MARKS
        .iter()
        .find(|(types, _)| types.contains(&creature))
        .map_or(0, |&(_, mark)| mark)
}

/// Whether a creature of type `creature` is of the mark 8 (`0x43C540`), a magician: the wood elf
/// 02 (105), the goblin mages (127, 143) and the god (144), Anducar (182) and his demon (183), the
/// terror sprinter mage (212), the dryad druid (216), the black magicians (258, 262 to 270), the
/// priestess (311) and the priest (312), the dark elf M 01 (693), Sharifa (697) and Sharifo (700).
pub fn mark_8(creature: u32) -> bool {
    matches!(
        creature,
        105 | 127 | 143 | 144 | 182 | 183 | 212 | 216 | 258 | 262
            ..=270 | 311 | 312 | 693 | 697 | 700
    )
}

/// `PLANTCAGE` (40), which the mark 8's magicians hold the hero with.
pub const CAGE: u16 = 40;

/// Whether a magician of the mark 8 casts the plant cage at the hero (`0x4D5F33`): as the mark 1's
/// stone ([`holds`]), its level above 7; Anducar (182) and his demon (183) never.
pub fn cages(
    creature: u32,
    distance: f32,
    (roll, hostiles): (u32, u32),
    (level, hero_level): (u32, u32),
    (held, way): (bool, bool),
) -> bool {
    !matches!(creature, 182 | 183)
        && (HOLD_FROM..=HOLD_TO).contains(&distance)
        && roll % (4 + hostiles) == 2
        && level > 7
        && level >= hero_level
        && !held
        && way
}

/// The band a magician of the mark 8 keeps (`0x4D5F33`): it leaves its decision for its band's
/// walk where it stands nearer than [`MAGE_NEAR`] after it was ordered a cast, or farther than
/// [`MAGE_FAR`], while its decisions stay below [`MAGE_MOVES`] (`+0x12A`).
pub const MAGE_NEAR: f32 = 350.0;
pub const MAGE_FAR: f32 = 600.0;
pub const MAGE_MOVES: u8 = 20;

/// Whether a magician at `distance` from its target leaves its decision for its band's walk,
/// `ordered` a cast before, `moves` its decisions in this fight, this one counted.
pub fn mage_leaves_band(distance: f32, ordered: bool, moves: u8) -> bool {
    moves < MAGE_MOVES && ((distance < MAGE_NEAR && ordered) || distance > MAGE_FAR)
}

/// Whether the band's walk (`0x4E0300`'s sub-state 1 in the special mode) walks a caster of type
/// `creature` at `distance` to [`BAND_POINT`] from its target: nearer than [`BAND_NEAR`], or
/// farther than [`BAND_FAR`] (800 for Anducar's demon, 183); between them it decides again.
pub fn band_walks(distance: f32, creature: u32) -> bool {
    let far = if creature == 183 { 800.0 } else { BAND_FAR };
    distance < BAND_NEAR || distance > far
}

/// Whether a magician casts at a decision (`0x4D5F33`): a roll `rand() % 3` of 1, with a straight
/// way to its target and its deadline passed, or whatever they are where its target fights it.
pub fn mage_casts(roll: u32, way: bool, passed: bool, fought: bool) -> bool {
    roll % 3 == 1 && ((way && passed) || fought)
}

/// Whether a magician that does not cast stays facing its target (`0x4D5F33`): a roll
/// `rand() % (k + 1)` not 1, `k` 9 while its deadline runs and 1 once it passed, farther than 399
/// with a straight way; else, standing, it runs about its target.
pub fn mage_stays(roll: u32, distance: f32, way: bool) -> bool {
    roll != 1 && distance > 399.0 && way
}

/// The roll's bound of [`mage_stays`]: `k + 1`.
pub fn mage_stays_bound(passed: bool) -> u32 {
    if passed { 2 } else { 10 }
}

/// How far about its target a magician runs (300; Anducar's demon 700), and the decisions it
/// stands facing it after (`+0x129`).
pub fn mage_run(creature: u32) -> f32 {
    if creature == 183 { 700.0 } else { 300.0 }
}
pub const MAGE_RUN_STANDS: u8 = 3;

/// Whether a magician's cast keeps no deadline in effect (`0x4D4670(1)` halved or quartered as an
/// absolute time): the god (144), Anducar (182) and his demon (183).
pub fn mage_has_no_deadline(creature: u32) -> bool {
    matches!(creature, 144 | 182 | 183)
}

/// The raising (43) of the black magicians 263 and 264, Anducar's demon's rings of flames (69)
/// and the terror sprinter mage's aura (72), the creatures' own (docs/re/arts.md, "The creatures'
/// own spells").
pub const RAISING: u16 = 43;
pub const RINGS: u16 = 69;
pub const SPRINTERS_AURA: u16 = 72;

/// The flags word's bit of a creature that a raising stood up again (`+0x1F4` bit `0x100000`):
/// its next death gives no experience, and it may rise again.
pub const RAISED: u32 = 0x10_0000;

/// Where the goblin mage 127 throws its ball at a target at `target` (the chooser `0x4D31C0`):
/// the target's point moved by `jitter`, two whole numbers from −100 to 100 (`rand() % 201 −
/// 100`), where `stands` lets a creature stand on it; else the first of five points about the
/// target that it lets stand on, 10 aside along `x`, one side and the other, and from 10 short to
/// 10 beyond along `y` by fives; else, untested, the sixth, 10 aside and 15 beyond.
pub fn goblin_point(
    target: (f32, f32),
    jitter: (i16, i16),
    stands: impl Fn((f32, f32)) -> bool,
) -> (f32, f32) {
    let jittered = (
        target.0 + f32::from(jitter.0),
        target.1 + f32::from(jitter.1),
    );
    if stands(jittered) {
        return jittered;
    }
    let about = |k: u8| {
        let side = if k.is_multiple_of(2) { -10.0 } else { 10.0 };
        (target.0 + side, target.1 + f32::from(k) * 5.0 - 10.0)
    };
    (0..5)
        .map(about)
        .find(|&point| stands(point))
        .unwrap_or_else(|| about(5))
}

/// How near the hero a magician of the mark 8 stands for its cast at the hero's follower to be at
/// the hero instead (`0x4D695B`, the float `0x890EB0`).
const AT_THE_HERO_WITHIN: f32 = 750.0;

/// Whether a magician of the mark 8 that casts at a follower of the local hero casts at the hero
/// instead (`0x4D6939`), the hero at `distance` from it: nearer than 750 (`0x5370B0`).
pub fn at_the_hero(distance: f32) -> bool {
    distance < AT_THE_HERO_WITHIN
}

/// The summon (`SUMMONBEAR`, 3).
pub const SUMMON: u16 = 3;

/// The creature that a summon by a creature of type `creature` makes (`0x426F60`), `coin` a
/// `rand()` that chooses between two: the Wood Elf's unicorn 72, the Vampiress's wolf 588; for a
/// creature, by its type, 543 or 567 for the black magicians 258 and 262 to 271, 68 or 70 for the
/// priest 312, 76 or 118 for the goblin mage 127, 587 for the wood elf 02 (105) and 325, 142 for
/// 143 and 145 for the god (144), 545 for any other.
pub fn summoned(creature: u32, coin: u32) -> u32 {
    let odd = !coin.is_multiple_of(2);
    match creature {
        5 => 72,
        6 | 7 => 588,
        105 | 325 => 587,
        127 => {
            if odd {
                76
            } else {
                118
            }
        }
        143 => 142,
        144 => 145,
        258 | 262..=271 => {
            if odd {
                543
            } else {
                567
            }
        }
        312 => {
            if odd {
                68
            } else {
                70
            }
        }
        _ => 545,
    }
}

/// Where a creature's summon makes its creature (the chooser `0x4D31C0`'s spell 3): about its
/// target at `target` at the caster's distance `distance` from it, the whole units of the rounded
/// root, at the first of the angles from `start + 1` degrees on, a degree at a time and round
/// once, `start` a `rand() % 361`, whose point `stands` lets a creature stand on; none where none
/// does, and it casts none. Its π is the original's 3.1415.
pub fn summon_point(
    target: (f32, f32),
    distance: f32,
    start: u16,
    stands: impl Fn((f32, f32)) -> bool,
) -> Option<(f32, f32)> {
    let mut angle = start + 1;
    for _ in 0..360 {
        let radians = f32::from(angle) * SUMMON_PI / 180.0;
        let point = (
            target.0 + distance * radians.cos(),
            target.1 + distance * radians.sin(),
        );
        if stands(point) {
            return Some(point);
        }
        angle = (angle + 1) % 360;
    }
    None
}

/// The π of the summon's angles (the chooser `0x4D31C0`'s), short of the true one.
#[expect(
    clippy::approx_constant,
    reason = "the original's own approximation, which turns its angles so"
)]
const SUMMON_PI: f32 = 3.1415;

/// Whether a dead creature may be raised (`0x523220`): hostile to the hero, not of the black
/// magicians 263 and 264, not of the mark 8, not of `Creature.pak`'s class 10 (the dragons); a
/// priest (312) raises none.
pub fn raisable(caster: u32, (creature, class): (u32, u8), hostile: bool) -> bool {
    caster != 312 && hostile && !matches!(creature, 263 | 264) && !mark_8(creature) && class != 10
}

/// Anducar's demon's rings of flames (`0x528B72`): their radii and counts about the target's
/// place, one at the centre, 8 at 160 and 12 at 320.
pub const RINGS_OF_FLAMES: [(f32, u16); 3] = [(0.0, 1), (160.0, 8), (320.0, 12)];
/// How fast the flames' wave goes out from the target's place, how long a flame rises, burns and
/// fades, and the reach of its strikes, once a second (`0x7C04B0`, `0x5B1947`).
pub const FLAMES_WAVE: f32 = 350.0;
pub const FLAMES_LIFE: f32 = 1.75;
pub const FLAMES_SIZE: f32 = 90.0;

/// The terror sprinter mage's aura (`0x7C1390`): its life, its reach, the sprinters it gives its
/// copy (211, 349), the effect it gives the mage for its life (35, keyed by 72) and the copy's
/// physical damage's effect (3 of 4.0).
pub const SPRINTERS_AURA_LIFE: f32 = 8.0;
pub const SPRINTERS_AURA_REACH: f32 = 270.0;
pub fn sprinter(creature: u32) -> bool {
    matches!(creature, 211 | 349)
}

/// The casters of marks of their own (`0x4D4E50`; docs/re/ai.md, "The creatures' spells").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnMark {
    /// The black magician 10 (271, `0x20`): the ice splinters, and a blink away from near.
    Blinker,
    /// The ice elf (140, `0x208`): the lightning, the holy light, the holy shield on itself.
    IceElf,
    /// The ice druid (141, `0x209`): the plant cage, the blades of light about itself.
    IceDruid,
    /// The god's child (145, `0x20B`): the chain lightning or the lightning.
    GodsChild,
    /// The demon prince (185, 250 to 252, `0x20C`): its fan of flames (71).
    Prince,
    /// Anducar's guard (184, 249, `0x212`): its homing fire (70).
    AnducarsGuard,
    /// The prince's guards (187, 205 to 208, 253 to 255, 330 to 332, `0x218`): the plant cage.
    PrincesGuard,
    /// The eiterblume and its spinneling (186, 333, `0x211`): the spit, and the panther's leap
    /// ([`flower`]).
    Flower,
    /// The hornet (190, `0x20F`): it flanks the hero while another hornet of its mark is in the
    /// hero's list (`0x4D89F2`, #2562).
    Hornet,
    /// The hornet queen (336, `0x214`): it circles the hero while another hornet lives in the
    /// hero's list (`0x4DAB44`, #2562).
    Queen,
    /// The insect larva and the small slime worm (191, 248, `0x215`): it dithers and hides from
    /// the hero (`0x4D9789`, #2562).
    Larva,
    /// The ice elf champion (146, `0x20A`): near the hero, the Dark Elf's `Attacke` or `Hardhit`
    /// (`0x4D8133`, #2564).
    IceChampion,
    /// The hell golems (204, 350, `0x21C`): the Gladiator's `Attacke` by its roll (`0x4DAA8A`,
    /// #2564).
    HellGolem,
    /// The queens of the mark `0x217` (247, 354): the Dark Elf's `Attacke` by its roll
    /// (`0x4DA90D`, #2564).
    ArtQueen,
    /// The Fetzers (202, 344, `0x21A`): a phase chosen once, `0x217` or `0x21B`, then the Dark
    /// Elf's `Attacke` by its roll (`0x4D9A62`, `0x4DA90D`, `0x4DA9EC`, #2568).
    Fetzer,
    /// The brain-footers (203, 351, 352, `0x219`): their stare at the hero, which lengthens and
    /// freezes the regeneration of its arts (`0x4D9C2F`, #2567).
    BrainFooter,
    /// The minimummies (192, 329, 342, 343) and Tuktuk (363) of `0x20D`: a boss's blink, daggers
    /// and runs about the hero, a plain one's run round it and the Gladiator's `Attacke`
    /// (`0x4D8C4F`, #2565).
    Minimummy,
}

/// The caster's mark of its own of a creature of type `creature`, if any.
pub fn own_mark(creature: u32) -> Option<OwnMark> {
    Some(match creature {
        271 => OwnMark::Blinker,
        140 => OwnMark::IceElf,
        141 => OwnMark::IceDruid,
        145 => OwnMark::GodsChild,
        185 | 250..=252 => OwnMark::Prince,
        184 | 249 => OwnMark::AnducarsGuard,
        187 | 205..=208 | 253..=255 | 330..=332 => OwnMark::PrincesGuard,
        186 | 333 => OwnMark::Flower,
        HORNET => OwnMark::Hornet,
        QUEEN => OwnMark::Queen,
        191 | 248 => OwnMark::Larva,
        146 => OwnMark::IceChampion,
        204 | 350 => OwnMark::HellGolem,
        247 | 354 => OwnMark::ArtQueen,
        202 | 344 => OwnMark::Fetzer,
        203 | 351 | 352 => OwnMark::BrainFooter,
        192 | 329 | 342 | 343 | TUKTUK => OwnMark::Minimummy,
        _ => return None,
    })
}

/// The kind of hero whose action of the arts a creature of type `creature` takes as it makes a
/// weapon art at a special's order (`0x5500D0`, the action of `0x95583C`): the Dark Elf's (4) for
/// 146, 190, 202, 203, 247, 344, 351, 352 and 354; the Gladiator's (2) for 192, 204, 329, 342,
/// 343, 350 and 363; the Wood Elf's (5) for 218 and 219; none for any other (#2564).
pub fn executor(creature: u32) -> Option<u32> {
    match creature {
        146 | 190 | 202 | 203 | 247 | 344 | 351 | 352 | 354 => Some(4),
        192 | 204 | 329 | 342 | 343 | 350 | 363 => Some(2),
        218 | 219 => Some(5),
        _ => None,
    }
}

/// The weapon arts that the specials order (#2564): the Gladiator's `Attacke`, the Dark Elf's
/// `Hardhit` and `Attacke`.
pub const GLADIATORS_ATTACKE: u16 = 1059;
pub const DARK_ELFS_HARDHIT: u16 = 1066;
pub const DARK_ELFS_ATTACKE: u16 = 1067;
/// The decisions a creature stands after it is ordered an art (`+0x129` 10).
pub const ART_STANDS: u8 = 10;
/// How far beyond its reach the ice elf champion makes its arts (`0x4D8133`).
pub const ICE_CHAMPION_REACH: f32 = 30.0;
/// The specials' rolls' `b` (#2548): the hell golems' 15, the queens' of `0x217` 50.
pub const GOLEMS_B: u32 = 15;
pub const QUEENS_B: u32 = 50;
/// The Fetzers' marks (#2568): the one they start of, which chooses a phase, and the phases',
/// `0x217` by the roll of [`QUEENS_B`] (30 for the type 344) and `0x21B` by
/// [`FETZERS_FAST_B`].
pub const FETZER_CHOOSES: u16 = 0x21A;
pub const FETZER_PHASE: u16 = 0x217;
pub const FETZER_FAST: u16 = 0x21B;
pub const FETZER_344_B: u32 = 30;
pub const FETZERS_FAST_B: u32 = 15;
/// The decisions a Fetzer stands as it chooses (`+0x129` 1).
pub const FETZER_STANDS: u8 = 1;

/// The minimummies' terms (`0x4D8C4F`, #2565): Tuktuk, a boss whatever its rank; the boss's
/// blink nearer than `⌊1.5·R⌋` to `3·R` from the hero, `R` the two creatures' reach, its run
/// about the hero at `3·R` and the plain one's round it at `⌊1.5·R'⌋`, `R'` its and the second
/// striker's of its group's; its deadline 2 s on; the hurled blades it throws; the plain one's
/// roll's `b` 40 (`0x4D2D90(40)`); and the decisions each run stands, 15.
pub const TUKTUK: u32 = 363;
pub const MUMMY_NEAR: f32 = 1.5;
pub const MUMMY_FAR: f32 = 3.0;
pub const MUMMY_DEADLINE: f64 = 2.0;
pub const HURLED_BLADES: u16 = 1013;
pub const MUMMIES_B: u32 = 40;
pub const MUMMY_STANDS: u8 = 15;

/// The minimummies' daggers (`0x4D93C1`, the runner `0x4FE870`, #2575): the dagger 1712 that one
/// takes into its empty right hand (slot 13), and the more it throws after the first, `R(2, 4)`
/// (`+0x2B3`), each taken anew as the one before strikes; how long after it holds a dagger it
/// throws it, by the strikes seen 0.67 to 0.77 s apart at 4 to 6 cells, each some 0.1 s after
/// its throw (**estimate**: the model has no clip of the throw, its slot 147).
pub const DAGGER: u32 = 1712;
pub const DAGGERS_MORE: (u32, u32) = (2, 4);
pub const DAGGER_THROWN_AFTER: f32 = 0.6;

/// The daggers a minimummy throws after the first of its volley, `R(2, 4)` ([`DAGGERS_MORE`]).
pub fn daggers_more(rng: &mut impl Dice) -> u8 {
    let (least, most) = DAGGERS_MORE;
    let more = least + rng.below(most + 1 - least);
    u8::try_from(more).unwrap_or(u8::MAX)
}

/// The seconds of a minimummy's blink's two battle fogs: the spell 46's value 0 at a third of its
/// level, `min(19 + ⌊L/3⌋, 60)`, times 0.1 (`0x4D8F49`; **hypothesis** for the factor's word).
pub fn blink_fog_seconds(level: u32) -> f32 {
    let value = (19 + level / 3).min(60);
    f32::from(u16::try_from(value).unwrap_or(60)) * 0.1
}

/// The brain-footers' stare (`0x4D9C2F`, #2567): its roll's `b`, the decisions it stands after
/// it or its failure, how far ahead of the real clock a deadline it waits for may stand, and the
/// seconds of its drain and its freeze by its strength `w`, `10·w`.
pub const STARE_B: u32 = 30;
pub const STARE_STANDS: u8 = 15;
pub const STARE_AHEAD: f64 = 100.0;
pub const STARE_SECONDS: f32 = 10.0;
/// The bone it stares from (24) and the bones of its target it takes one of (`rand() % 50`),
/// by `0x44CA30`; the brain-footer 203's sound at itself (`SPELL_LASER`, 2074).
pub const STARE_BONE: usize = 24;
pub const STARE_BONES: u32 = 50;
pub const STARE_SOUND: u32 = 2074;
/// The effect that freezes the regeneration of its bearer's arts and combos while it holds
/// (`0x582D50` of 57 sets `+0x200` bit `0x1000000`, which `0x562730` takes, #2567).
pub const FREEZES_REGENERATION: u32 = 57;

/// What a brain-footer's stare drains, by its type (`0x4D9C2F`): the beam's variant (203 red 10,
/// 351 blue 11, 352 yellow 12), the effect 57's whole value (2, 1, 3), and the kinds of arts
/// whose regeneration it lengthens: the spells, the weapon arts and the combos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stare {
    pub variant: u8,
    pub whole: i32,
    pub spells: bool,
    pub weapon_arts: bool,
    pub combos: bool,
}

/// The stare of a brain-footer of type `creature`, none for any other type.
pub fn stare(creature: u32) -> Option<Stare> {
    Some(match creature {
        203 => Stare {
            variant: 10,
            whole: 2,
            spells: true,
            weapon_arts: true,
            combos: true,
        },
        351 => Stare {
            variant: 11,
            whole: 1,
            spells: true,
            weapon_arts: false,
            combos: true,
        },
        352 => Stare {
            variant: 12,
            whole: 3,
            spells: false,
            weapon_arts: true,
            combos: true,
        },
        _ => return None,
    })
}

/// The strength `w` of a brain-footer's stare by the levels (`0x4D9C2F`, the weighing of
/// `0x551530` inlined): `r = (E + 9)/(H + 9)` of its level on bronze's scale `E` against the
/// hero's `H`, against the threshold `t` (`(H + 6)/(H + 9)` up to level 10, `(H − 6)/(H + 9)`
/// from 92, else `anteil`); none where `r` is at most `t` or its share of the way to `voll` at
/// most 0.04; `0.1 + 0.2·(r − t)/(voll − t)` below `voll`, `0.3 + 0.4·(r − voll)/(upper − voll)`
/// up to `upper`, 1 above.
pub fn stare_strength(
    level_on_bronze: u32,
    hero: u32,
    (anteil, voll, upper): (f32, f32, f32),
) -> Option<f32> {
    let e = f32::from(u16::try_from(level_on_bronze).unwrap_or(u16::MAX));
    let h = f32::from(u16::try_from(hero).unwrap_or(u16::MAX));
    let r = (e + 9.0) / (h + 9.0);
    let t = match hero {
        0..=10 => (h + 6.0) / (h + 9.0),
        92.. => (h - 6.0) / (h + 9.0),
        _ => anteil,
    };
    let share = (r - t) / (voll - t);
    if r <= t || share <= 0.04 {
        return None;
    }
    Some(if r < voll {
        0.1 + 0.2 * share
    } else if r <= upper {
        0.3 + 0.4 * (r - voll) / (upper - voll)
    } else {
        1.0
    })
}

/// The difficulty's `p` of the specials' rolls, bronze's first.
const SPECIAL_P: [u32; 5] = [20, 60, 80, 100, 130];

/// Whether a special's roll of `b` passes on `difficulty` (#2548): `roll`, `R(1, 100)`, below
/// `⌊b·p/100⌋`, `p` 20, 60, 80, 100 or 130 from bronze to niobium.
pub fn special_passes(b: u32, difficulty: crate::drops::Difficulty, roll: u32) -> bool {
    let p = SPECIAL_P.get(difficulty.index()).copied().unwrap_or(30);
    roll < b * p / 100
}

/// The marks whose bosses and champions keep back from the hero while nearly whole
/// (`0x4D4EF6`–`0x4D51AC`, #2563): none, `0x200`, `0x400`, `0x800`, `0x1000`, `0x212` and
/// `0x217`.
pub const KEEPS_BACK: [u16; 7] = [0, 0x200, 0x400, 0x800, 0x1000, 0x212, 0x217];
/// Its terms: whole while it misses less than a twelfth of its most health; it comes on from
/// beyond 300, or from within 240 once ordered, for 25 moves; else it walks to 280 from the hero
/// at a random angle and stands 25 decisions; each keep-back stands 2.
pub const KEEP_SHARE: u32 = 12;
pub const KEEP_NEAR: f32 = 240.0;
pub const KEEP_FAR: f32 = 300.0;
pub const KEEP_MOVES: u8 = 25;
pub const KEEP_BACK: f32 = 280.0;
pub const KEEP_STANDS: u8 = 25;
pub const KEEP_PAUSE: u8 = 2;

/// The hornet (190), the hornet hunter (335) and the hornet queen (336), the hornets of the
/// Underworld, of the marks `0x20F`, `0x213` and `0x214`.
pub const HORNET: u32 = 190;
pub const HUNTER: u32 = 335;
pub const QUEEN: u32 = 336;
pub const HORNETS: [u32; 3] = [HORNET, HUNTER, QUEEN];

/// The hornet's flank (`0x4D89F2`): `50·((id & 3) + 8)` from the hero, 400 to 550 by its id,
/// along the line from the hero to it turned 45° one way for an odd id and the other for an even
/// one.
pub const FLANK_STEP: f32 = 50.0;
pub const FLANK_STEPS: u32 = 8;
pub const FLANK_TURN: f32 = 45.0;

/// The queen's circle (`0x4DAB44`): 480 from the hero, at its own angle from the hero turned by
/// 0.4 rad, one way for an odd id and the other for an even one; its walk kept while it is 100 or
/// more from its point, and the decisions it then stands, 10.
pub const QUEEN_CIRCLE: f32 = 480.0;
pub const QUEEN_TURN: f32 = 0.4;
pub const QUEEN_KEEPS: f32 = 100.0;
pub const QUEEN_STANDS: u8 = 10;

/// The larva's dithering and its hiding (`0x4D9789`): the decisions it stands, 10; it hides 800
/// from itself, the first of 18 points 20° apart from an angle at random that it may stand on,
/// out of the hero's straight line, with a path to it.
pub const LARVA_STANDS: u8 = 10;
pub const HIDES_AT: f32 = 800.0;
pub const HIDE_TRIES: u32 = 18;
pub const HIDE_STEP: f32 = 20.0;

/// The holy light (15), the blades of light (17), the holy shield (22), the chain lightning (33),
/// the ice splinters (34), the guard's fire (70) and the prince's fan of flames (71).
pub const HOLY_LIGHT: u16 = 15;
pub const BLADES: u16 = 17;
pub const HOLY_SHIELD: u16 = 22;
pub const CHAIN: u16 = 33;
pub const SPLINTERS: u16 = 34;
pub const GUARDS_FIRE: u16 = 70;
pub const FAN_OF_FLAMES: u16 = 71;

/// The black magician 10's distances (`0x4D71A5`): its band's walk farther than 350, its blink
/// nearer than 200, and at most 15 tries of a point with a straight way (`0x4D722B`).
pub const BLINKER_FAR: f32 = 350.0;
pub const BLINKER_NEAR: f32 = 200.0;
pub const BLINK_TRIES: u32 = 15;
/// The colour of the sight it leaves where it stood (856, its event's `+0x38`).
pub const BLINK_COLOUR: u32 = 0xFF80_FF80;

/// A try of the black magician's blink (`0x4D722B`): the offset from it of `rand() % 1001 − 500`
/// world units along each axis, of the rolls `x_roll` and `y_roll`, a square of ±500.
pub fn blink_try(x_roll: u32, y_roll: u32) -> (f32, f32) {
    let offset = |roll: u32| f32::from(u16::try_from(roll % 1001).unwrap_or(0)) - 500.0;
    (offset(x_roll), offset(y_roll))
}

/// The dryad druid (216), whose decision of the mark 8 blinks about its target and sets thorns
/// (`0x4D6396`…`0x4D6824`, #1883).
pub const DRYAD_DRUID: u32 = 216;

/// Whether the dryad druid blinks at a decision (`0x4D5F33` at `0x4D6350`): a roll `rand() % 6` of
/// 0, with a straight way to its target (`0x4FCC80`), where its target fights it (`0x4EB890`).
pub fn druid_blinks(roll: u32, way: bool, fought: bool) -> bool {
    roll == 0 && way && fought
}

/// Where the dryad druid blinks: `angle_roll` (`rand() % 361`) degrees about its target, three
/// times the reach of the two (`0x4DBC60`) from it, as an offset from its target's point.
pub fn druid_blink(angle_roll: u32, reach: f32) -> (f32, f32) {
    let angle = f32::from(u16::try_from(angle_roll % 361).unwrap_or(0)).to_radians();
    let far = 3.0 * reach;
    (far * angle.cos(), far * angle.sin())
}

/// The thorns that the dryad druid sets as it blinks (`0x4D2E10`): a group at its target's point
/// at the spell's level `L/5 + 1` of its own level `L`, and five about it, 100 from it at 0, 72,
/// 144, 216 and 288 degrees, at `L/7 + 1`; each an offset of its centre and its spell's level.
pub fn druid_thorns(level: u32) -> Vec<((f32, f32), u32)> {
    let mut groups = vec![((0.0, 0.0), level / 5 + 1)];
    groups.extend((0_u16..5).map(|k| {
        let angle = f32::from(k * 72).to_radians();
        (
            (DRUID_RING * angle.cos(), DRUID_RING * angle.sin()),
            level / 7 + 1,
        )
    }));
    groups
}

/// How far the dryad druid's ring of thorns stands from its target (the double at `0x890EB8`).
pub const DRUID_RING: f32 = 100.0;
/// Its deadline after a blink: the real clock's whole seconds and 2 (`0x4D6760`).
pub const DRUID_DEADLINE: f64 = 2.0;
/// The thorns' scatter of a creature's `0x4D2E10`: each `rand() % 101` beyond its centre along
/// both axes, where it may stand, else at the centre.
pub const DRUID_SCATTER: (f32, u32) = (0.0, 101);

/// What a caster of the spell `spell` casts it on: itself (the holy shield, the blades of light),
/// else its target.
pub fn on_itself(spell: u16) -> bool {
    matches!(spell, HOLY_SHIELD | BLADES)
}

/// Whether the launch test refuses `spell` on a target that holds an effect keyed by it
/// (`0x51D840`, `0x582B00`): the spells 2, 17 to 22, 24, 25, 27, 28, 30 to 32, 37, 40, 41, 47,
/// 55 to 59, 62, 63, 65 and 72.
pub fn refused_while_keyed(spell: u16) -> bool {
    matches!(
        spell,
        2 | 17..=22 | 24 | 25 | 27 | 28 | 30..=32 | 37 | 40 | 41 | 47 | 55..=59 | 62 | 63 | 65 | 72
    )
}

/// What the ice elf casts within its reach and 30 (`0x4D4E50`, mark `0x208`), by a roll of 1 in 3
/// (`roll`, a `R(0, 2)` of 0; **hypothesis** for the value) with its deadline passed: of nine
/// (`choice`), the lightning at the hero in six, the holy light in two, the holy shield on itself
/// in one.
pub fn ice_elf_casts(
    distance: f32,
    reach: f32,
    (roll, choice): (u32, u32),
    passed: bool,
) -> Option<u16> {
    (distance <= reach + 30.0 && roll % 3 == 0 && passed).then_some(match choice % 9 {
        0..=5 => crate::spells::LIGHTNING,
        6 | 7 => HOLY_LIGHT,
        _ => HOLY_SHIELD,
    })
}

/// What the ice druid casts within five times its reach (mark `0x209`): the blades of light about
/// itself in two cases of three (`choice`), else its weapon arts 1018 or 1019 (not spells, none
/// here).
pub fn ice_druid_casts(distance: f32, reach: f32, choice: u32) -> Option<u16> {
    (distance <= 5.0 * reach && choice % 3 < 2).then_some(BLADES)
}

/// What the god's child casts within its reach and 30 (mark `0x20B`), by a roll of 1 in 3: the
/// chain lightning at the hero in three cases of four (`choice`), else the lightning.
pub fn gods_child_casts(distance: f32, reach: f32, (roll, choice): (u32, u32)) -> Option<u16> {
    (distance <= reach + 30.0 && roll % 3 == 0).then_some(if choice % 4 < 3 {
        CHAIN
    } else {
        crate::spells::LIGHTNING
    })
}

/// Whether the demon prince casts its fan of flames (mark `0x20C`): beyond its reach and 30, in one
/// decision of two (`roll`).
pub fn prince_casts(distance: f32, reach: f32, roll: u32) -> bool {
    distance > reach + 30.0 && roll.is_multiple_of(2)
}

/// The heal and the shields a magician casts on a friend (`0x4D31C0`): `HEILKRAFT` (7), the fire
/// skin (24) and the stone skin (2).
pub const HEAL: u16 = 7;
pub const FIRE_SKIN: u16 = 24;
pub const STONE_SKIN: u16 = 2;

/// Whether `spell` is cast on a friend of the caster's rather than on its target: the heal and the
/// shields.
pub fn on_a_friend(spell: u16) -> bool {
    matches!(spell, HEAL | FIRE_SKIN | STONE_SKIN)
}

/// How far a magician's friends are that it aids (`0x4D31C0`): the hero's hostiles within 700.
pub const FRIENDS_WITHIN: f32 = 700.0;

/// The chance of 101 by which a magician of type `creature` aids a friend instead of its type's
/// spell (`0x4D31C0`): 30, 60 for the priestess (311) and the priest (312); none for the types of a
/// choice of their own (183, 127, 143, 144, 182, 212, 697).
pub fn aid_chance(creature: u32) -> Option<u32> {
    match creature {
        183 | 127 | 143 | 144 | 182 | 212 | 697 => None,
        311 | 312 => Some(60),
        _ => Some(30),
    }
}

/// What a magician casts on a friend (`0x4D31C0`), with a roll `rand() % 101` below its chance
/// ([`aid_chance`]): the heal on a friend under three quarters of its health (`wounded`), else on
/// one that fights (`fighting`) the fire skin in two cases of three (`shield`, a `rand() % 3` below
/// 2) or the stone skin; none without such a friend (**hypothesis** for the friends' rule, whose
/// float operands are lost).
pub fn aid(roll: u32, chance: u32, (wounded, fighting): (bool, bool), shield: u32) -> Option<u16> {
    if roll % 101 >= chance {
        return None;
    }
    if wounded {
        Some(HEAL)
    } else if fighting {
        Some(if shield % 3 < 2 {
            FIRE_SKIN
        } else {
            STONE_SKIN
        })
    } else {
        None
    }
}

/// Sharifa (697), whose heal goes to the hero under a third of his health (`0x4D31C0`).
pub const SHARIFA: u32 = 697;

/// Whether the local hero at `health` of `most` is low enough for Sharifa's heal (`0x4D31C0`):
/// his health under a third of his most, whole.
pub fn hero_low(health: u32, most: u32) -> bool {
    health < most / 3
}

/// The friend of the least share of health for the heal of a magician's type's choice
/// (`0x4D3580`…`0x4D36CD`): of `candidates`, each its health and its most, the first of the
/// least share, the caster's own first in the list; none of an empty list.
pub fn least_share<T: Copy>(candidates: &[(T, u32, u32)]) -> Option<T> {
    let share = |health: u32, most: u32| f64::from(health) / f64::from(most.max(1));
    candidates
        .iter()
        .fold(None, |least: Option<(T, f64)>, &(who, health, most)| {
            let this = share(health, most);
            match least {
                Some((_, low)) if this >= low => least,
                _ => Some((who, this)),
            }
        })
        .map(|(who, _)| who)
}

/// Whether a friend at `health` of `most` is wounded enough for a magician's heal: under three
/// quarters of its most.
pub fn wounded(health: u32, most: u32) -> bool {
    u64::from(health) * 4 < u64::from(most) * 3
}

/// The spell a magician of type `creature` casts (the chooser `0x4D31C0`): by its type, a script's
/// spell (`+0x2B6`, `scripted`) first for the others, `hero_low` whether the hero's health is under
/// a third of its most (Sharifa's heal of him), `followers` its followers; none, and it stops.
/// The spells 4, 30, 32 and 33 become 13 and 35 becomes 11; a type of no spell takes 5 in one case
/// of four, else 3 in one of five, else 34 or 43; the summon (3) while it has followers becomes 5
/// in four cases of five, else 43. The friends' heals and shields of the others' chance (30, 60
/// for the priestess and the priest) are not here. Which value of each roll is which is a
/// **hypothesis**: the first of the pair, or the larger share, the roll's lower values.
pub fn mage_spell(
    creature: u32,
    scripted: u16,
    (hero_low, followers): (bool, u32),
    rng: &mut impl Dice,
) -> Option<u16> {
    let mut half = |a: u16, b: u16| if rng.below(2) == 0 { a } else { b };
    let spell = match creature {
        183 => half(52, 69),
        127 | 143 | 144 => 53,
        182 => half(5, 52),
        212 => 72,
        697 if hero_low => 7,
        697 => match rng.below(21) {
            0..=4 => 5,
            5..=9 => 34,
            10..=13 => 40,
            14..=16 => 13,
            17..=19 => 39,
            _ => return None,
        },
        _ if scripted != 0 => scripted,
        267 | 270 => 26,
        105 => half(38, 34),
        216 | 265 => 39,
        258 => 5,
        262 => 34,
        263 => {
            if rng.below(5) < 4 {
                43
            } else {
                3
            }
        }
        264 => {
            if rng.below(5) < 4 {
                3
            } else {
                43
            }
        }
        266 => {
            if rng.below(3) < 2 {
                5
            } else {
                40
            }
        }
        268 => 23,
        269 => 13,
        311 => half(26, 34),
        312 => half(23, 34),
        693 => half(39, 5),
        700 => 52,
        _ => 0,
    };
    let spell = match spell {
        4 | 30 | 32 | 33 => 13,
        35 => 11,
        1..=72 => spell,
        _ if rng.below(4) == 0 => 5,
        _ if rng.below(5) == 0 => 3,
        _ => {
            if rng.below(2) == 0 {
                34
            } else {
                43
            }
        }
    };
    Some(if spell == 3 && (1..100).contains(&followers) {
        if rng.below(5) < 4 { 5 } else { 43 }
    } else {
        spell
    })
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a count of seconds, not negative, far below the integers' range"
)]
fn whole(value: f64) -> u32 {
    value.max(0.0) as u32
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_marks_are_the_types_that_minds_takes_for_marked() {
        for creature in 0..1_000 {
            assert_eq!(
                super::mark(creature) != 0,
                crate::minds::marked(creature),
                "{creature}"
            );
        }
        assert_eq!(super::mark(44), 1);
        assert_eq!(super::mark(56), 9);
        assert_eq!(super::mark(271), 0x20);
        assert_eq!(super::mark(363), 0x20D);
        assert_eq!(super::mark(701), 0x1FE);
        assert_eq!(super::mark(1), 0);
    }

    use super::*;
    use crate::fight::{Rank, Rules};

    #[test]
    fn a_volley_throws_two_to_four_more_daggers() {
        let mut rng = crate::spawning::Rng::of(3, 5, 7);
        let mut seen = [0u32; 5];
        for _ in 0..300 {
            seen[usize::from(daggers_more(&mut rng))] += 1;
        }
        assert_eq!(seen[0] + seen[1], 0, "{seen:?}");
        assert!(seen[2..].iter().all(|&n| n > 60), "{seen:?}");
    }

    #[test]
    fn the_flowers_spit_or_leap_beyond_their_melee_and_the_crab_cages() {
        assert_eq!(own_mark(186), Some(OwnMark::Flower));
        assert_eq!(clip(SPIT, 333, false), FLOWERS_CLIP);
        assert_eq!(clip(FLOWER_LEAP, 186, false), FLOWER_LEAP_CLIP);
        assert!((spit_at(186) - 0.525).abs() < f32::EPSILON);
        assert!((spit_at(GIANT_SPIDER) - SPIT_AT).abs() < f32::EPSILON);
        // A reach of 75: melee within 112.
        assert!(flower_melee(111.0, 75.0) && !flower_melee(112.0, 75.0));
        assert_eq!(flower(300.0, 1), Flower::Spits);
        assert_eq!(flower(300.0, 2), Flower::Stands);
        assert_eq!(flower(400.0, 8), Flower::Leaps);
        assert_eq!(flower(800.0, 1), Flower::Stands);
        assert!(crab_cages(CRAB_QUEEN, 3) && !crab_cages(CRAB_QUEEN, 2));
        assert!(!crab_cages(GIANT_SPIDER, 3));
    }

    #[test]
    fn the_giant_spider_spits_or_bears_two_to_four_young() {
        assert_eq!(coming_on(GIANT_SPIDER), Some(SPIT));
        assert_eq!(coming_on(CRAB_QUEEN), Some(SPIT));
        assert_eq!(clip(SPIT, GIANT_SPIDER, false), SPIDER_SHOT_CLIP);
        // The giant spider alone bears young, on a first roll of 2.
        assert!(bears_young(GIANT_SPIDER, 2) && !bears_young(GIANT_SPIDER, 1));
        assert!(!bears_young(CRAB_QUEEN, 2));
        // Two young where the second roll is 0, three where the third is 0 or 1, else four.
        let young_of = |rolls: &[u32]| {
            let mut rolls = rolls.iter().copied();
            young(|| rolls.next().unwrap())
        };
        assert_eq!(young_of(&[2, 0]), 2);
        assert_eq!(young_of(&[0, 1, 1]), 3);
        assert_eq!(young_of(&[1, 2, 2, 0]), 4);
        assert_eq!(young_level(21), 11);
        assert_eq!(spit_sound(SPIT_LANDS, 3), Some(11234));
        assert_eq!(spit_sound(SPIT_SOUNDS, 4), None);
        assert!((spit_reach(1.0) - 400.0).abs() < f32::EPSILON);
    }

    #[test]
    fn the_goblin_mages_ball_lands_where_a_creature_may_stand() {
        let target = (1000.0, 2000.0);
        assert_eq!(goblin_point(target, (-100, 40), |_| true), (900.0, 2040.0));
        // The jittered point not stood on: the first of the five about the target.
        let not_jittered = |point: (f32, f32)| point != (900.0, 2040.0);
        assert_eq!(
            goblin_point(target, (-100, 40), not_jittered),
            (990.0, 1990.0)
        );
        let east = |point: (f32, f32)| point.0 > 1000.0 && point.1 > 2000.0;
        assert_eq!(goblin_point(target, (-100, 40), east), (1010.0, 2005.0));
        // None stood on: the sixth, untested.
        assert_eq!(goblin_point(target, (0, 0), |_| false), (1010.0, 2015.0));
    }

    #[test]
    fn a_cast_at_the_heros_follower_is_at_the_hero_within_750() {
        assert!(at_the_hero(749.0));
        assert!(!at_the_hero(750.0));
    }

    #[test]
    fn a_summon_makes_its_casters_creature_about_the_target_at_its_distance() {
        // The black magician 264 of level 30 made a 567 (seen in the original).
        assert_eq!(summoned(264, 0), 567);
        assert_eq!(summoned(264, 1), 543);
        assert_eq!(summoned(312, 1), 68);
        assert_eq!(summoned(311, 0), 545);
        assert_eq!(summoned(5, 0), 72);
        // The first point that stands, a degree on from the start.
        let all = |_| true;
        let (x, y) = summon_point((1000.0, 1000.0), 200.0, 359, all).unwrap_or_default();
        assert!((x - 1200.0).abs() < 0.1 && (y - 1000.0).abs() < 0.1);
        // Nothing stands on the side y above 1000: it turns on to the first that does.
        let below = |(_, y): (f32, f32)| y <= 1000.0;
        let (_, y) = summon_point((1000.0, 1000.0), 200.0, 0, below).unwrap_or_default();
        assert!(y <= 1000.0);
        assert_eq!(summon_point((0.0, 0.0), 100.0, 0, |_| false), None);
    }

    #[test]
    fn a_dragon_casts_its_meteors_from_beyond_its_reach_and_500_in_one_decision_of_two() {
        assert!(dragon(52) && dragon(101) && !dragon(53) && !dragon(94));
        assert!(dragon_casts(800.0, 250.0, 0));
        assert!(!dragon_casts(800.0, 250.0, 1));
        assert!(!dragon_casts(750.0, 250.0, 0));
    }

    #[test]
    fn a_big_demon_casts_its_lasers_or_its_fire_standing_and_coming_on() {
        assert!(big_demon(53) && big_demon(94) && !big_demon(52));
        // Standing: one decision of three with a way, the lasers in two cases of three.
        assert_eq!(big_demon_casts(0, true, 1), Some(EYE_LASERS));
        assert_eq!(big_demon_casts(3, true, 2), Some(DEMON_FIRE));
        assert_eq!(big_demon_casts(1, true, 0), None);
        assert_eq!(big_demon_casts(0, false, 0), None);
        assert!(big_demon_runs(0, false) && !big_demon_runs(0, true) && !big_demon_runs(2, false));
        // Coming on: from 371, by a roll below 5 of 101; beyond 500 the lasers, nearer the fire
        // in four cases of five.
        assert_eq!(big_demon_approaches(370.0, 0, 0), None);
        assert_eq!(big_demon_approaches(400.0, 5, 0), None);
        assert_eq!(big_demon_approaches(400.0, 4, 3), Some(DEMON_FIRE));
        assert_eq!(big_demon_approaches(400.0, 4, 4), Some(EYE_LASERS));
        assert_eq!(big_demon_approaches(600.0, 0, 0), Some(EYE_LASERS));
    }

    #[test]
    fn the_medium_spider_shoots_and_the_dragons_breathe_fire_as_they_come_on() {
        assert_eq!(coming_on(75), Some(SPIDER_SHOT));
        assert_eq!(coming_on(88), Some(DRAGON_FIRE));
        assert_eq!((coming_on(73), coming_on(76)), (None, None));
        // The dragons breathe by the clip of the state 9, not their casts' 81.
        assert_eq!(clip(DRAGON_FIRE, 88, true), SPIDER_SHOT_CLIP);
        assert_eq!(clip(METEORS, 88, true), 81);
        // As the big demons: from 371 away, by a roll below 5 of 101.
        assert!(!approaches(370.0, 0) && !approaches(400.0, 5) && approaches(400.0, 104));
        assert_eq!(clip(SPIDER_SHOT, 75, false), SPIDER_SHOT_CLIP);
        // One of its four sounds in four cases of five.
        assert_eq!(spider_shot_sound(0), Some(1248));
        assert_eq!(spider_shot_sound(3), Some(1243));
        assert_eq!(spider_shot_sound(4), None);
    }

    #[test]
    fn the_magicians_choose_their_spells_by_their_types() {
        use crate::spawning::Script;
        assert!(mark_8(127) && mark_8(266) && mark_8(312) && !mark_8(271) && !mark_8(48));
        let mut none = Script::new(Vec::new());
        assert_eq!(mage_spell(127, 0, (false, 0), &mut none), Some(53));
        assert_eq!(mage_spell(268, 0, (false, 0), &mut none), Some(23));
        // A script's spell first, 33 made 13.
        assert_eq!(mage_spell(268, 33, (false, 0), &mut none), Some(13));
        // The black magician 266: the fireball in two cases of three, else the cage.
        assert_eq!(
            mage_spell(266, 0, (false, 0), &mut Script::new(vec![1])),
            Some(5)
        );
        assert_eq!(
            mage_spell(266, 0, (false, 0), &mut Script::new(vec![2])),
            Some(40)
        );
        // The 264's summon becomes the fireball while it has a follower.
        assert_eq!(
            mage_spell(264, 0, (false, 1), &mut Script::new(vec![0, 0])),
            Some(5)
        );
        // Sharifa heals the hero under a third of his health, else rolls, once for nothing.
        assert_eq!(mage_spell(697, 0, (true, 0), &mut none), Some(7));
        assert_eq!(
            mage_spell(697, 0, (false, 0), &mut Script::new(vec![20])),
            None
        );
    }

    #[test]
    fn the_casters_of_marks_of_their_own_cast_their_spells() {
        assert_eq!(own_mark(271), Some(OwnMark::Blinker));
        assert_eq!(own_mark(251), Some(OwnMark::Prince));
        assert_eq!(own_mark(266), None);
        assert_eq!(
            ice_elf_casts(100.0, 90.0, (0, 5), true),
            Some(crate::spells::LIGHTNING)
        );
        assert_eq!(ice_elf_casts(100.0, 90.0, (0, 7), true), Some(HOLY_LIGHT));
        assert_eq!(ice_elf_casts(100.0, 90.0, (0, 8), true), Some(HOLY_SHIELD));
        assert_eq!(ice_elf_casts(100.0, 90.0, (0, 0), false), None);
        assert_eq!(ice_elf_casts(130.0, 90.0, (0, 0), true), None);
        assert_eq!(ice_druid_casts(400.0, 90.0, 1), Some(BLADES));
        assert_eq!(ice_druid_casts(400.0, 90.0, 2), None);
        assert_eq!(gods_child_casts(100.0, 90.0, (0, 2)), Some(CHAIN));
        assert_eq!(
            gods_child_casts(100.0, 90.0, (0, 3)),
            Some(crate::spells::LIGHTNING)
        );
        assert!(prince_casts(200.0, 90.0, 0) && !prince_casts(100.0, 90.0, 0));
        assert!(refused_while_keyed(17) && refused_while_keyed(40) && !refused_while_keyed(13));
        assert!(on_itself(BLADES) && !on_itself(CHAIN));
    }

    #[test]
    fn a_raising_stands_up_a_hostile_monster_of_no_magician_nor_dragon() {
        assert!(raisable(263, (36, 5), true));
        assert!(!raisable(263, (36, 5), false));
        assert!(!raisable(263, (264, 2), true) && !raisable(263, (266, 2), true));
        assert!(!raisable(263, (52, 10), true) && !raisable(312, (36, 5), true));
        assert!(sprinter(211) && sprinter(349) && !sprinter(212));
    }

    #[test]
    fn a_magician_aids_a_wounded_or_fighting_friend_by_its_chance() {
        assert_eq!(aid_chance(127), None);
        assert_eq!(aid_chance(312), Some(60));
        assert_eq!(aid_chance(266), Some(30));
        assert_eq!(aid(29, 30, (true, true), 0), Some(HEAL));
        assert_eq!(aid(30, 30, (true, true), 0), None);
        assert_eq!(aid(0, 30, (false, true), 1), Some(FIRE_SKIN));
        assert_eq!(aid(0, 30, (false, true), 2), Some(STONE_SKIN));
        assert_eq!(aid(0, 30, (false, false), 0), None);
        assert!(wounded(74, 100) && !wounded(75, 100));
        assert!(on_a_friend(HEAL) && !on_a_friend(5));
    }

    #[test]
    fn a_magician_keeps_its_band_casts_and_stays() {
        assert!(mage_leaves_band(650.0, false, 1) && !mage_leaves_band(650.0, false, 20));
        assert!(mage_leaves_band(300.0, true, 1) && !mage_leaves_band(300.0, false, 1));
        assert!(band_walks(650.0, 266) && !band_walks(650.0, 183) && !band_walks(270.0, 266));
        assert!(mage_casts(1, true, true, false) && mage_casts(4, false, false, true));
        assert!(!mage_casts(1, true, false, false) && !mage_casts(0, true, true, true));
        assert!(mage_stays(0, 500.0, true) && !mage_stays(1, 500.0, true));
        assert!(!mage_stays(0, 399.0, true) && !mage_stays(0, 500.0, false));
        assert!(cages(266, 500.0, (2, 0), (8, 3), (false, true)));
        assert!(!cages(266, 500.0, (2, 0), (7, 3), (false, true)));
        assert!(!cages(182, 500.0, (2, 0), (30, 3), (false, true)));
    }

    #[test]
    fn the_mark_1s_casters_cast_their_types_spells() {
        assert_eq!(mark_1(48), Some((FIREBALL, 1)));
        assert_eq!(mark_1(80), Some((FIREBALL, 2)));
        assert_eq!(mark_1(64), Some((WHIRLWIND, 0)));
        assert_eq!(mark_1(66), Some((LIGHTNING, 0)));
        assert_eq!(mark_1(67), Some((METEORS, 0)));
        assert_eq!(mark_1(33), None);
    }

    #[test]
    fn a_creature_casts_with_its_spells_clip() {
        assert_eq!(clip(FIREBALL, 48, false), 155);
        assert_eq!(clip(WHIRLWIND, 64, false), 155);
        assert_eq!(clip(LIGHTNING, 66, false), 158);
        assert_eq!(clip(LIGHTNING, 140, false), 147);
        assert_eq!(clip(STONE, 44, false), 157);
        assert_eq!(clip(METEORS, 67, false), 157);
        assert_eq!(clip(METEORS, 52, true), 81);
        assert_eq!(clip(5, 182, false), 154);
        assert_eq!(clip(53, 127, false), 84);
    }

    #[test]
    fn a_caster_waits_five_to_seven_seconds_shorter_as_it_rises() {
        // 6 to 8 at the level 1, five to seven seconds; a boss four to five.
        assert_eq!(deadline(1, false, 0, 0), 5);
        assert_eq!(deadline(1, false, 0, 2), 7);
        assert_eq!(deadline(1, true, 0, 2), 6);
        assert_eq!(deadline(60, false, 0, 0), 4);
        // Each hostile of the magicians in the melee a second more.
        assert_eq!(deadline(1, false, 2, 0), 7);
    }

    #[test]
    fn a_caster_keeps_its_band_and_casts_by_its_rolls() {
        assert!(leaves_band(200.0, true, 0) && !leaves_band(200.0, false, 0));
        assert!(leaves_band(400.0, false, 0) && !leaves_band(400.0, false, BAND_MOVES));
        assert!(!leaves_band(270.0, true, 0));
        assert!(casts(2, true, true) && !casts(1, true, true) && !casts(2, false, true));
        assert!(!casts(2, true, false));
        // Alone or the nearest, it fights hand to hand in one decision of three.
        assert!(!casting(false, true, 0) && casting(false, true, 1));
        assert!(casting(true, false, 0));
        // The stone from 400 to 1,500, its level above 5 and at least the hero's.
        assert!(holds(800.0, (2, 0), (6, 6), (false, true)));
        assert!(!holds(300.0, (2, 0), (6, 6), (false, true)));
        assert!(!holds(800.0, (2, 0), (5, 1), (false, true)));
        assert!(!holds(800.0, (2, 0), (6, 7), (false, true)));
        assert!(!holds(800.0, (2, 0), (6, 6), (true, true)));
    }

    #[test]
    fn a_creatures_spell_is_of_its_fifth_level_and_its_damage_of_magie_all() {
        let rules = Rules::default();
        assert_eq!(rules.creature_spell_level(9, Rank::Common), 2);
        assert_eq!(rules.creature_spell_level(9, Rank::Boss), 2);
        // The balance's 1.4 a float a little below it: 5 · 1.4 falls short of 7 (a quirk).
        assert_eq!(rules.creature_spell_level(20, Rank::Boss), 6);
        assert_eq!(rules.creature_spell_level(25, Rank::Boss), 8);
        assert_eq!(rules.creature_spell_level(20, Rank::Champion), 6);
        let damage = |level| (rules.creature_spell_damage(level) * 1000.0).round();
        assert_eq!(
            [1, 2, 3, 4, 5, 6, 7].map(damage),
            [597.0, 642.0, 707.0, 690.0, 790.0, 771.0, 767.0]
        );
    }

    #[test]
    fn the_black_magicians_blink_tries_a_square_of_500_about_it() {
        assert_eq!(blink_try(0, 1000), (-500.0, 500.0));
        assert_eq!(blink_try(500, 1001), (0.0, -500.0));
        assert_eq!(blink_try(1500, 250), (-1.0, -250.0));
    }

    #[test]
    fn the_dryad_druid_blinks_three_reaches_off_and_sets_six_groups_of_thorns() {
        // One roll of six, a straight way, its target fighting it.
        assert!(druid_blinks(0, true, true));
        assert!(!druid_blinks(1, true, true) && !druid_blinks(0, false, true));
        assert!(!druid_blinks(0, true, false));
        // At 0 and 90 degrees, the reach 100: 300 off.
        let (x, y) = druid_blink(0, 100.0);
        assert!((x - 300.0).abs() < 1e-3 && y.abs() < 1e-3);
        let (x, y) = druid_blink(90, 100.0);
        assert!(x.abs() < 1e-3 && (y - 300.0).abs() < 1e-3);
        // A druid of level 21: the centre's at 5, the ring's five at 4, 100 off at 72 degrees.
        let groups = druid_thorns(21);
        assert_eq!(groups.len(), 6);
        assert_eq!(groups[0], ((0.0, 0.0), 5));
        assert!(groups[1..].iter().all(|&(_, level)| level == 4));
        let ((x, y), _) = groups[2];
        assert!((x.hypot(y) - DRUID_RING).abs() < 1e-3);
        assert!((y.atan2(x).to_degrees() - 72.0).abs() < 1e-3);
    }

    #[test]
    fn sharifas_heal_takes_the_least_share_once_the_hero_is_under_a_third() {
        assert!(hero_low(99, 300) && !hero_low(100, 300));
        // Sharifa at 90 of 100, the hero at 40 of 150, a follower at 50 of 60.
        assert_eq!(
            least_share(&[(0, 90, 100), (1, 40, 150), (2, 50, 60)]),
            Some(1)
        );
        // None below the caster's own: herself.
        assert_eq!(least_share(&[(0, 10, 100), (1, 40, 150)]), Some(0));
        assert_eq!(least_share::<u8>(&[]), None);
    }
}
