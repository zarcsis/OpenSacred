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

/// The Mage's skins and shields: the fire skin, `FLAMMENHAUT`; the quickening, `KATARAKT`; the
/// gust, `WINDBOE`; the shield wall, `SCHILDWALL`; and the mana's spring, `AUE`.
pub const FIRE_SKIN: u16 = 24;
pub const QUICKENING: u16 = 27;
pub const GUST: u16 = 28;
pub const SHIELD_WALL: u16 = 30;
pub const SPRING: u16 = 31;

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
pub const MADE: [u16; 17] = [
    STONE_SKIN,
    FIREBALL,
    HEALING,
    REIKI,
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
];

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
        assert!(made(FIREBALL) && made(REIKI) && !made(1));
    }
}
