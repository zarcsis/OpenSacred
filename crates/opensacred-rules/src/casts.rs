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
    (distance >= APPROACH_FROM && roll % 101 < 5).then_some(
        if distance > LASERS_BEYOND || choice % 5 >= 4 {
            EYE_LASERS
        } else {
            DEMON_FIRE
        },
    )
}

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
        _ => return None,
    })
}

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
/// nearer than 200, at most 15 tries of a point with a straight way, and how far it blinks
/// (**estimate**: the tries' geometry is not read).
pub const BLINKER_FAR: f32 = 350.0;
pub const BLINKER_NEAR: f32 = 200.0;
pub const BLINK_TRIES: u32 = 15;
pub const BLINK_REACH: f32 = 400.0;

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
    use super::*;
    use crate::fight::{Rank, Rules};

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
}
