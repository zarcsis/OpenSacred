//! The sounds of the hero's spells and combat arts (#1172, #1174; docs/re/sound.md, "Spells and
//! arts"): a spell's cast as its clip starts (`0x51E110`) and its act as it acts (`0x522480`,
//! `spells::Acts`); an art's own cry as it starts (the event `(5, 1)` of `0x4FD0F0`), after
//! which its battle cry is sure (`voices`); and its executor's sound as it starts (the
//! executors' cases, `0x506770`, `0x50E6C0`, `0x514790`, `0x5192E0`; where in the case is not
//! read). Each plays at the caster or the maker, the healing's cast at its target, at the
//! volume 1 and a rate's factor of 0.5, the sample's own rate.
//!
//! The spells' and arts' objects' own sounds, their flights, impacts and loops, are #1173's.

use bevy::prelude::*;

use crate::frame::Unpaused;
use crate::making::{Making, Playing};
use crate::sfx::{Of, Play, Roll};
use crate::spells::Acts;
use opensacred_rules::arts::{self, Style};

/// The healing, whose cast sounds at its target.
const HEALING: u16 = 7;
/// The Gladiator's creature type, whose hand of god sounds its own (`0x4FD0F0`).
const GLADIATOR: u32 = 2;

/// The sounds of the cast of the spell `spell` (`0x51E110`, the jump table `0x521C34`).
pub fn cast(spell: u16) -> &'static [u32] {
    match spell {
        1 => &[2023],
        2 => &[2013],
        4 => &[2012],
        5 => &[2000],
        HEALING => &[2086],
        11 => &[2018],
        13 => &[2034],
        15 => &[2030],
        17 => &[2035],
        18 => &[2038],
        19 => &[2033],
        20 => &[2037],
        21 => &[2032],
        22 => &[2039],
        23 => &[2005],
        24 => &[2003],
        25 => &[2014],
        26 => &[2007],
        29 => &[2015],
        30 => &[2021],
        31 => &[2051],
        33 => &[2043],
        36 => &[2059, 2063],
        37 | 40 => &[2059],
        41 => &[2065],
        60 => &[2118],
        61 => &[2125],
        66 => &[2121],
        62..=65 | 67 => &[2122],
        _ => &[],
    }
}

/// The sounds of the act of the spell `spell` (`0x522480`, the jump table `0x52947C`).
pub fn act(spell: u16) -> &'static [u32] {
    match spell {
        8 | 13 => &[2041],
        16 => &[2029],
        33 => &[2011],
        34 => &[2026],
        35 => &[2052],
        39 | 40 => &[2061],
        42 => &[2060],
        50 => &[2084],
        55 => &[2106],
        57 => &[2112],
        58 => &[2108],
        59 => &[2109],
        67 => &[2117],
        71 => &[2095, 2064],
        _ => &[],
    }
}

/// The own cry of the art `art` as it starts, one of the sounds at even odds (`0x4FD0F0` at
/// `0x4FD87A`, `0x4FDE34`, `0x4FE630`): the jump into the pack, the neck breakers, the hand of
/// god for a Gladiator and the aura for another maker, the hurled weapon, and the auras.
pub fn own_cry(art: u16, maker: u32) -> &'static [u32] {
    match art {
        1003 => &[2098],
        1009..=1011 => &[2099, 2100],
        1012 if maker == GLADIATOR => &[2102],
        1012 | 1014..=1016 => &[2101],
        1013 => &[2103, 2104],
        _ => &[],
    }
}

/// Whether the art `art` takes its battle cry surely: those of an own cry (`0x4FD0F0`).
pub fn cries_surely(art: u16) -> bool {
    !own_cry(art, 0).is_empty()
}

/// The sounds of the executor of the art `art`: the Gladiator's turbo boost, the Vampiress's
/// vampirism, wolves' call, turbo boost, blood bite, leap and short casts, the Dwarf's might and
/// the Daemon's cast, twice.
pub fn executor(art: u16) -> &'static [u32] {
    match art {
        1043 | 1028 | 1029 => &[2081],
        1022 => &[2079],
        1026 | 1027 => &[2080],
        1030 => &[2076],
        1032 => &[2083],
        1034 | 1035 => &[2078],
        1084 => &[2107],
        1103 => &[2122, 2122],
        _ => &[],
    }
}

/// A sound of an art's or a spell's at `at`.
fn play(sounds: &mut MessageWriter<Play>, id: u32, at: Entity) {
    sounds.write(Play {
        sound: Of::Id(id),
        at: Some(at),
        rate: (0.5, Roll::None),
    });
}

/// The sounds of the arts and the spells that the hero starts: a spell's cast, an art's own cry,
/// one of its sounds at even odds, and its executor's.
fn starts(
    started: Query<(Entity, &Making, &Playing), Added<Playing>>,
    hero: Option<Res<crate::hero::Hero>>,
    mut sfx: ResMut<crate::sfx::Sfx>,
    mut sounds: MessageWriter<Play>,
) {
    let maker = hero.map_or(0, |hero| hero.creature);
    for (entity, making, playing) in &started {
        if matches!(arts::style(making.art), Some(Style::Cast(_))) {
            let at = if making.art == HEALING {
                playing.target()
            } else {
                entity
            };
            for &id in cast(making.art) {
                play(&mut sounds, id, at);
            }
            continue;
        }
        let cries = own_cry(making.art, maker);
        let pick = sfx.dice.below(u32::try_from(cries.len()).unwrap_or(1));
        if let Some(&id) = usize::try_from(pick).ok().and_then(|pick| cries.get(pick)) {
            play(&mut sounds, id, entity);
        }
        for &id in executor(making.art) {
            play(&mut sounds, id, entity);
        }
    }
}

/// The sounds of the spells as they act, at their casters.
fn acts(mut acts: MessageReader<Acts>, mut sounds: MessageWriter<Play>) {
    for act in acts.read() {
        for &id in self::act(act.spell) {
            play(&mut sounds, id, act.caster);
        }
    }
}

/// The sounds of the hero's spells and combat arts.
pub struct ArtSoundsPlugin;

impl Plugin for ArtSoundsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (starts, acts).in_set(Unpaused));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spell_sounds_its_cast_and_its_act() {
        assert_eq!(cast(5), [2000]);
        assert_eq!(cast(36), [2059, 2063]);
        assert_eq!(cast(64), [2122]);
        assert!(cast(3).is_empty());
        assert_eq!(act(13), [2041]);
        assert_eq!(act(71), [2095, 2064]);
        assert!(act(5).is_empty());
    }

    #[test]
    fn an_art_cries_by_its_maker_and_then_surely() {
        assert_eq!(own_cry(1012, GLADIATOR), [2102]);
        assert_eq!(own_cry(1012, 1), [2101]);
        assert_eq!(own_cry(1010, 1), [2099, 2100]);
        assert!(cries_surely(1003));
        assert!(!cries_surely(1071));
        assert_eq!(executor(1103), [2122, 2122]);
    }
}
