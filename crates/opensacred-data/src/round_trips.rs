//! The open formats' round trips on random values (#1434). A binary file is postcard, which is
//! not self-describing: a field that serde leaves out of the output or writes by another shape
//! breaks it silently. So each type of a binary file reads back as the value written, and a file
//! of another version answers [`DecodeError::Version`]; each type of a RON file reads back from
//! the converter's layout to a value that writes the same text again (a value with a NaN is not
//! equal to itself), with LF line ends only (#834). The values come from `proptest-derive`'s
//! `Arbitrary`, which the types derive in the tests alone; their vectors hold at most 8 values
//! (`PROPTEST_MAX_DEFAULT_SIZE_RANGE` of `.cargo/config.toml`), as they nest four deep.

use std::fmt::Debug;

use proptest::prelude::*;
use ron::ser::PrettyConfig;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::DecodeError;
use crate::{heroes, luau, netscript, packed, sound, tables, world};

/// The converter's RON layout (`opensacred-convert`'s `utf8::pretty`): `ron`'s pretty one with
/// LF line ends.
fn pretty() -> PrettyConfig {
    PrettyConfig::default().new_line("\n")
}

/// `value`'s binary file `file`, written with `version`, reads back by `decode` as the value,
/// and its file of another version does not.
fn reads_back_from_its_file<T: PartialEq + Debug + Serialize>(
    value: &T,
    (file, version): (&[u8], u32),
    decode: fn(&[u8]) -> Result<T, DecodeError>,
) -> Result<(), TestCaseError> {
    let read = decode(file);
    prop_assert_eq!(read.as_ref(), Ok(value));
    let other = packed::encode(version + 1, value)?;
    prop_assert_eq!(
        decode(&other),
        Err(DecodeError::Version {
            found: version + 1,
            expected: version
        })
    );
    Ok(())
}

/// `value` written in RON as the converter writes it reads back to a value that writes the same
/// text, which has no CR.
fn reads_back_from_ron<T: Serialize + DeserializeOwned>(value: &T) -> Result<(), TestCaseError> {
    let write = |value: &T| {
        ron::ser::to_string_pretty(value, pretty()).map_err(|e| TestCaseError::fail(e.to_string()))
    };
    let text = write(value)?;
    prop_assert!(!text.contains('\r'), "a CR in {:?}", text);
    let read: T = ron::from_str(&text).map_err(|e| TestCaseError::fail(format!("{e}: {text}")))?;
    prop_assert_eq!(write(&read)?, text);
    Ok(())
}

proptest! {
    #[test]
    fn a_hero_reads_back_from_its_file(hero: heroes::Hero) {
        reads_back_from_its_file(&hero, (&hero.encode()?, heroes::VERSION), heroes::Hero::decode)?;
    }

    #[test]
    fn a_tree_reads_back_from_its_file(tree: netscript::Tree) {
        reads_back_from_its_file(&tree, (&tree.encode()?, netscript::VERSION), netscript::Tree::decode)?;
    }

    #[test]
    fn a_sector_reads_back_from_its_file(sector: world::Sector) {
        reads_back_from_its_file(&sector, (&sector.encode()?, world::SECTOR_VERSION), world::Sector::decode)?;
    }

    #[test]
    fn the_sprites_read_back_from_their_file(sprites: world::Sprites) {
        reads_back_from_its_file(&sprites, (&sprites.encode()?, world::SECTOR_VERSION), world::Sprites::decode)?;
    }
}

/// A property of each type of a RON file: the files' own types, and each table's record.
macro_rules! ron_round_trips {
    ($($name:ident: $type:ty,)*) => {
        proptest! {
            $(
                #[test]
                fn $name(value: $type) {
                    reads_back_from_ron(&value)?;
                }
            )*
        }
    };
}

ron_round_trips! {
    the_world_reads_back_from_its_ron: world::World,
    the_heroes_read_back_from_their_ron: heroes::Heroes,
    the_trees_read_back_from_their_ron: netscript::NetScript,
    a_trees_index_reads_back_from_its_ron: luau::Index,
    the_sound_reads_back_from_its_ron: sound::Sound,
    a_creature_reads_back_from_its_ron: tables::Creature,
    a_weapon_reads_back_from_its_ron: tables::Weapon,
    an_item_reads_back_from_its_ron: tables::Item,
    the_voices_read_back_from_their_ron: tables::Voices,
    the_portraits_read_back_from_their_ron: tables::Portraits,
    a_type_name_reads_back_from_its_ron: tables::TypeName,
    a_ui_image_reads_back_from_its_ron: tables::UiImage,
    the_idle_noises_read_back_from_their_ron: tables::IdleNoises,
    a_figure_reads_back_from_its_ron: tables::Figure,
    a_motion_reads_back_from_its_ron: tables::Motion,
    the_balance_reads_back_from_its_ron: tables::Balance,
    an_item_set_reads_back_from_its_ron: tables::ItemSet,
    a_weapon_modifier_reads_back_from_its_ron: tables::WeaponModifier,
    a_mercenary_reads_back_from_its_ron: tables::Mercenary,
    a_start_point_reads_back_from_its_ron: tables::StartPoint,
    the_arts_read_back_from_their_ron: tables::Arts,
    a_trigger_reads_back_from_its_ron: tables::Trigger,
}
