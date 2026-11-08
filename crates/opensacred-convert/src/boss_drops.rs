//! The bosses' drops by their type (docs/re/items.md, "How many drops, and the bonus", #2269):
//! what a boss of some twenty types adds to its death's drops by the switch over its type in the
//! deaths' handler (`0x533E2E`, its jump tables at `0x536EBC` and `0x536F48`) → `tables/
//! boss_drops.ron` ([`opensacred_data::tables::BossDrop`]). The converter carries them as its
//! constants, numbers and the balance's key names alone (docs/08 §3), which a test checks against
//! the jump tables of the user's own `Sacred.exe`.

use opensacred_data::tables::{BossBonus, BossDrop, BossSpecial};

/// A case of the switch: its types, its bonus's key (a balance key, or the image's offset as a
/// number), its calls more, its treasure and its drop of its own.
struct Case {
    types: &'static [u16],
    bonus: Option<Bonus>,
    more: Option<[u8; 2]>,
    treasure: Option<[u16; 4]>,
    special: Option<BossSpecial>,
}

enum Bonus {
    Key(&'static str),
    Offset(u32),
}

/// The cases in the order of their code.
const CASES: &[Case] = &[
    Case {
        types: &[53],
        bonus: Some(Bonus::Key("ITBonus_BIG_DEMON")),
        more: Some([3, 4]),
        treasure: Some([750, 12, 14, 10]),
        special: None,
    },
    Case {
        types: &[94],
        bonus: Some(Bonus::Key("ITBonus_DRAGON")),
        more: Some([3, 4]),
        treasure: Some([250, 12, 14, 2]),
        special: None,
    },
    Case {
        types: &[74],
        bonus: Some(Bonus::Key("ITBonus_DRAGON_ICE")),
        more: Some([3, 4]),
        treasure: Some([500, 12, 14, 2]),
        special: None,
    },
    Case {
        types: &[88],
        bonus: Some(Bonus::Key("ITBonus_DRAGON_UNDEAD")),
        more: Some([3, 4]),
        treasure: Some([500, 12, 14, 2]),
        special: None,
    },
    Case {
        types: &[100],
        bonus: Some(Bonus::Key("ITBonus_DRAGON_ICE03")),
        more: Some([3, 4]),
        treasure: Some([500, 12, 14, 2]),
        special: None,
    },
    Case {
        types: &[101],
        bonus: Some(Bonus::Key("ITBonus_DRAGON_LOROMIR")),
        more: Some([3, 4]),
        treasure: Some([500, 12, 14, 2]),
        special: None,
    },
    Case {
        types: &[102],
        bonus: Some(Bonus::Key("ITBonus_DRAGON_SIRITHCAM")),
        more: Some([3, 4]),
        treasure: Some([500, 12, 14, 2]),
        special: None,
    },
    Case {
        types: &[103],
        bonus: Some(Bonus::Key("ITBonus_DRAGON_SWAMP02")),
        more: Some([3, 4]),
        treasure: Some([500, 12, 14, 2]),
        special: None,
    },
    Case {
        types: &[55],
        bonus: Some(Bonus::Key("ITBonus_GIANT_SPIDER")),
        more: None,
        treasure: None,
        special: None,
    },
    Case {
        types: &[136],
        bonus: Some(Bonus::Key("ITBonus_MEDUSA")),
        more: None,
        treasure: None,
        special: Some(BossSpecial::Doll),
    },
    Case {
        types: &[137],
        bonus: None,
        more: None,
        treasure: None,
        special: Some(BossSpecial::Doll),
    },
    Case {
        types: &[52, 144, 348],
        bonus: Some(Bonus::Key("ITBonus_DRAGON")),
        more: Some([2, 3]),
        treasure: Some([500, 12, 14, 2]),
        special: None,
    },
    Case {
        types: &[145],
        bonus: None,
        more: None,
        treasure: None,
        special: Some(BossSpecial::Undo),
    },
    Case {
        types: &[182],
        bonus: None,
        more: None,
        treasure: None,
        special: Some(BossSpecial::Anducar),
    },
    Case {
        types: &[183],
        bonus: None,
        more: None,
        treasure: None,
        special: Some(BossSpecial::AnducarDemon),
    },
    Case {
        types: &[185, 250, 251, 252],
        bonus: Some(Bonus::Key("ITBonus_DRAGON")),
        more: Some([2, 3]),
        treasure: Some([1000, 12, 14, 2]),
        special: None,
    },
    Case {
        types: &[336, 338],
        bonus: Some(Bonus::Key("ITBonus_MEDUSA")),
        more: Some([2, 3]),
        treasure: None,
        special: None,
    },
    Case {
        types: &[247, 354],
        bonus: Some(Bonus::Offset(0x1248)),
        more: Some([1, 2]),
        treasure: None,
        special: None,
    },
];

/// The bosses' drops, a record a case.
pub fn boss_drops() -> Vec<BossDrop> {
    CASES
        .iter()
        .map(|case| BossDrop {
            types: case.types.to_vec(),
            bonus: case.bonus.as_ref().map(|bonus| match bonus {
                Bonus::Key(key) => BossBonus::Key((*key).to_owned()),
                Bonus::Offset(offset) => BossBonus::Offset(*offset),
            }),
            more: case.more,
            treasure: case.treasure,
            special: case.special,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_boss_type_takes_one_case() {
        let mut types: Vec<u16> = CASES.iter().flat_map(|case| case.types.to_vec()).collect();
        let count = types.len();
        types.sort_unstable();
        types.dedup();
        assert_eq!((types.len(), count), (25, 25));
    }

    /// The switch's jump tables of the user's `Sacred.exe` group the types as the cases do: the
    /// table of 52 to 137 (its index bytes at `0x536EF0`, its 13 addresses at `0x536EBC`, the
    /// index 12 the default), that of 145 to 354 (`0x536F68`, 8 addresses at `0x536F48`, the
    /// index 7 the default), and 144 compared alone with 52's case.
    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn the_cases_group_the_types_as_the_executables_jump_tables() {
        let game = opensacred_locate::original();
        let exe = std::fs::read(game.join("Sacred.exe")).unwrap();
        let pe = opensacred_formats::pe::Image::parse(&exe).unwrap();
        let byte = |address: u32| pe.at(address, 1, "index").unwrap().u8().unwrap();
        let word = |address: u32| {
            u32::from_le_bytes(
                pe.at(address, 4, "address")
                    .unwrap()
                    .array::<4>("address")
                    .unwrap(),
            )
        };
        // Each type's case's address, by the tables.
        let target = |creature: u16| -> Option<u32> {
            match creature {
                52..=137 => {
                    let index = byte(0x536EF0 + u32::from(creature - 52));
                    (index != 12).then(|| word(0x536EBC + 4 * u32::from(index)))
                }
                144 => Some(0x53408B),
                145..=354 => {
                    let index = byte(0x536F68 + u32::from(creature - 145));
                    (index != 7).then(|| word(0x536F48 + 4 * u32::from(index)))
                }
                _ => None,
            }
        };
        let grouped: Vec<Vec<u16>> = CASES
            .iter()
            .map(|case| {
                let first = target(case.types[0]).unwrap();
                (52..=354)
                    .filter(|&creature| target(creature) == Some(first))
                    .collect()
            })
            .collect();
        for (case, group) in CASES.iter().zip(&grouped) {
            // 136 runs on into 137's code, which its own entry reaches apart.
            assert_eq!(case.types, group.as_slice());
        }
        let cased: usize = (52..=354)
            .filter(|&creature| target(creature).is_some())
            .count();
        assert_eq!(cased, 25);
    }
}
