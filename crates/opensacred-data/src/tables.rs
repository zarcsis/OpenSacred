//! The converted tables (docs/02 §6, §5.5, §4.1): one RON file each under the tables directory.
//!
//! Every byte of an original record lands in a field: those whose meaning is not known yet keep
//! the original layout in spans named by their offset (`words_84` holds the u16 words from
//! `+0x84`), so that the semantics can be added later without converting again from a lossy
//! table.

use serde::{Deserialize, Serialize};

pub const CREATURES: &str = "creatures.ron";
pub const WEAPONS: &str = "weapons.ron";
pub const ITEMS: &str = "items.ron";
pub const MOTIONS: &str = "motions.ron";
pub const TRIGGERS: &str = "triggers.ron";
pub const BALANCE: &str = "balance.ron";
pub const WORLD_INDEX: &str = "world_index.ron";
pub const WORLD_MAP: &str = "world_map.ron";
pub const ITEM_SETS: &str = "item_sets.ron";
pub const WEAPON_MODIFIERS: &str = "weapon_modifiers.ron";
pub const WEAPON_GROUPS: &str = "weapon_groups.ron";
pub const ARMOUR_SWITCHES: &str = "armour_switches.ron";
pub const STAIRS: &str = "stairs.ron";
pub const STATIC_REMAP: &str = "static_remap.ron";
pub const MERCENARIES: &str = "mercenaries.ron";
pub const MERCENARIES_ADDON: &str = "mercenaries_addon.ron";
pub const START_POINTS: &str = "start_points.ron";
pub const ARTS: &str = "arts.ron";
pub const VOICES: &str = "voices.ron";
pub const IDLE_NOISES: &str = "idle_noises.ron";
pub const PORTRAITS: &str = "portraits.ron";
/// The 256 bytes of `Sacred.exe` from which the weather's choice rolls its times and its
/// intensities, by a rolling index (`0x95FA84`, `0x625290`; docs/re/weather.md), as a list of
/// numbers.
pub const WEATHER_ROLLS: &str = "weather_rolls.ron";
/// The figures of the choice of the hero that `Sacred.exe` holds (`0x9E3248`; #1130), as a list
/// of [`Figure`]s.
pub const FIGURES: &str = "figures.ron";
/// The combat arts' runes that the drops draw from, of the table that `Sacred.exe` holds
/// (`0x9565A0`, `0x559BF0`; #1183), as a list of their 123 types.
pub const RUNES: &str = "runes.ron";
/// The interface's images that `Sacred.exe` holds (`0x9ECEB0`; #1087), which the interface's
/// controls name by their indices, as a list of [`UiImage`]s in the order of the indices.
pub const UI_IMAGES: &str = "ui_images.ron";
/// The types' names that `Sacred.exe` holds (`0x8EC328`; #1171), by which the client's code
/// sorts some types (the footsteps' sounds), as a list of [`TypeName`]s in the table's order.
pub const TYPE_NAMES: &str = "type_names.ron";

/// A creature type of `Creature.pak`, in table order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Creature {
    /// Also the item slot of the creature's model.
    pub id: u32,
    pub class: u8,
    /// 0x01 flies, 0x02 big, 0x10 no shadow, 0x20 ghost, 0x40, 0x80.
    pub flags: u8,
    /// Experience `a + level × b`.
    pub experience: [u16; 2],
    pub base: [u8; 6],
    pub skills: [u8; 2],
    pub skills_extra: [u8; 16],
    pub speed: [u16; 2],
    pub bonus: [[u8; 2]; 6],
    pub bonus_value: [u8; 6],
    pub bonus_class: [u8; 6],
    pub damping: [[u8; 5]; 4],
}

/// A weapon or armour type of `Weapon.pak`, in table order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Weapon {
    /// The item type.
    pub id: u32,
    /// The item type it inherits from, or 0.
    pub parent: u32,
    /// An internal name.
    pub name: String,
    pub floats_00: [f32; 5],
    pub bytes_14: [u8; 4],
    pub float_18: f32,
    pub bytes_1c: [u8; 8],
    pub words_84: Vec<u16>,
}

/// An object type: a used slot of `Items.pak`, or of the overlay `Items03.pak`, whose one slot
/// (3999) is empty in `Items.pak`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Item {
    pub slot: u32,
    pub kind: u8,
    pub class: u8,
    pub subclass: u8,
    /// A `.GRN` file or an editor label.
    pub name: String,
    pub texture: u32,
    /// The `mixed.pak` sprite, 0 for none.
    pub sprite: u32,
    pub byte_03: u8,
    pub words_08: [u32; 2],
    pub words_14: [u32; 8],
    pub bytes_34: [u8; 3],
    pub byte_57: u8,
    pub words_58: [u32; 10],
}

impl Item {
    /// The float at `+0x57` (docs/02 §6.1), which the packed record splits between `byte_57` and
    /// the low three bytes of `words_58[0]`: the angle on the screen, in degrees, that an object of
    /// the type faces as it is made (`0x5FB530`'s event `0x40`, `0x44BCD0`); −30 for the chests,
    /// barrels and book stands of the start scripts, 90 for the heroes, 0 for most types.
    pub fn angle(&self) -> f32 {
        f32::from_bits(u32::from(self.byte_57) | self.words_58[0] << 8)
    }

    /// The flags, the record's first four bytes as a `u32` (docs/02 §6.1): `kind` in the low
    /// byte, then `class`, `subclass` and `byte_03`.
    pub fn flags(&self) -> u32 {
        u32::from_le_bytes([self.kind, self.class, self.subclass, self.byte_03])
    }

    /// The category at `+0x2E` (docs/02 §6.1), byte 2 of `words_14[6]`: what an item is and where
    /// it goes (5 weapons, 6 armour, 8 rings, 9 potions, 20 amulets, …).
    pub fn category(&self) -> u8 {
        self.words_14[6].to_le_bytes()[2]
    }

    /// The sound profile at `+0x24` (docs/02 §6.1), `words_14[4]`: a slot of `sndProfiles.pak`,
    /// whose groups a weapon's swing, hit and parry take (500 a sword, 501 a bow, 503 a knife), 0
    /// for none and for every creature (docs/re/sound.md, "The fight's sounds").
    pub fn sound(&self) -> u32 {
        self.words_14[4]
    }
}

/// The creatures' voices (`0x6852B0`, docs/re/sound.md, "The fight's sounds"), `voices.ron`: the
/// types that speak, by the case of each type in `Sacred.exe`'s byte table at `0x68BBD0`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Voices {
    pub voices: Vec<Voice>,
}

/// A creature type's voice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Voice {
    /// The creature's type, 1 to 714.
    pub creature: u16,
    /// Its profile, a slot of `sndProfiles.pak`, or the heroes' two, of which each line takes one
    /// at random.
    pub profiles: Vec<u16>,
    /// A line of the groups 1 `ATTACK`, 2 `NORMALHIT`, 3 `CRITICALHIT` and 7 `DEATH` plays when
    /// `rand() % n` is 0: 1 always, 0 never.
    pub rolls: [u8; 4],
    /// The seconds for which a sound id that played waits before it plays again.
    pub wait: f32,
    /// The ids of its lines at a cast and when it falls unconscious, one of them at random.
    pub casts: Vec<u16>,
    pub faints: Vec<u16>,
}

impl Voice {
    /// The roll of a group, 0 for a group of none (`APPROACH`, `FOLLOW`, `FLEE`, …).
    pub fn roll(&self, group: u8) -> u8 {
        match group {
            1 => self.rolls[0],
            2 => self.rolls[1],
            3 => self.rolls[2],
            7 => self.rolls[3],
            _ => 0,
        }
    }
}

/// The creatures' portraits in the dialogues' window, `portraits.ron` (#1032; docs/re/netscript.md,
/// "Dialogues"): each type's part of a texture of the interface by `Sacred.exe`'s function
/// `0x4345C0` and its images' table `0x9ECEB0`, and the default's of a type without one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Portraits {
    pub default: Portrait,
    pub creatures: Vec<CreaturePortrait>,
}

/// A creature type's portrait.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct CreaturePortrait {
    pub creature: u16,
    pub portrait: Portrait,
}

/// A portrait: the texture of `texture.pak` it is a part of, by its name lower-cased
/// (`gui_portraitx34`), and its part, its left, top, width and height in pixels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Portrait {
    pub texture: String,
    pub rect: [u16; 4],
}

/// A type's name: the type's number (an `items.pak` slot) and its name (`TYPE_NPC_GOBLIN_WARRIOR`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct TypeName {
    pub kind: u32,
    pub name: String,
}

/// An image of the interface: the texture of `texture.pak` it is a part of, by its name
/// lower-cased (`gui_main_03`), and its part, its left, top, width and height in pixels; the
/// entry 0, `INVALID`, of no size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct UiImage {
    pub texture: String,
    pub rect: [u16; 4],
    /// Whether the original draws its part turned about across and along, as the table's
    /// entries whose right stands left of their left, or whose bottom above their top, do; left
    /// out of the file where it is neither (#1236).
    #[serde(default, skip_serializing_if = "straight")]
    pub turned: [bool; 2],
    /// Its two anchor points relative to its part's top left, `x0`, `y0`, `x1`, `y1`: the
    /// original's, each not 0 made relative to the part's left or top, both at the part's centre
    /// for the buttons and icons (`0x761580`); left out of the file where all are 0 (#1267).
    #[serde(default, skip_serializing_if = "unanchored")]
    pub anchors: [i16; 4],
}

/// Whether an image is turned neither way.
fn straight(turned: &[bool; 2]) -> bool {
    *turned == [false, false]
}

/// Whether an image has no anchors.
fn unanchored(anchors: &[i16; 4]) -> bool {
    *anchors == [0; 4]
}

impl Portraits {
    /// The portrait of the creature type `creature`, its own or the default.
    pub fn of(&self, creature: u32) -> &Portrait {
        self.creatures
            .iter()
            .find(|c| u32::from(c.creature) == creature)
            .map_or(&self.default, |c| &c.portrait)
    }
}

/// The creatures' idle noises, `idle_noises.ron` (#714; docs/re/sound.md, "The idle noises"):
/// what each type makes in the sound manager's update as a creature hostile to the hero and as
/// one that is not, the two lists that the update follows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct IdleNoises {
    pub hostile: Vec<IdleNoise>,
    pub peaceful: Vec<IdleNoise>,
}

/// A creature type's idle noise.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct IdleNoise {
    /// The creature's type, 1 to 714.
    pub creature: u16,
    /// It plays when `rand() % n` is 0, after the update's own roll: 1 always.
    pub roll: u8,
    pub sound: IdleSound,
    /// The seconds for which the sound id that played waits before it plays again.
    pub wait: f32,
    /// Added to the voices' factor of the rate, 0.45 + (`rand() % 10`) / 90, which plays the
    /// sample at its rate times the factor + 0.5 (`0x693510`, #1140); `volume` in the files
    /// converted before.
    #[serde(alias = "volume")]
    pub rate: f32,
}

/// A figure of the choice of the hero, of the table that `Sacred.exe` holds at `0x9E3248`, in the
/// screen's order (docs/re/equipment.md, "The choice of the hero's figures"; #1130).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Figure {
    /// The hero's class, its creature type: 1 the Seraphim to 9 the Daemon.
    pub class: u32,
    /// The item types it shows, 0 for none, which go into the slots 13, 12, 1, none, 0, 4, 2 and
    /// 17.
    pub items: [u32; 8],
    /// Where it stands in its camera's world, Z up, and its turn about Z in degrees.
    pub position: [f32; 3],
    pub turn: f32,
    /// Three floats that the client does not read.
    pub floats_34: [f32; 3],
    /// Its window in the screen's pixels of 1024 × 768: left, top, width and height.
    pub window: [f32; 4],
}

/// What an idle noise plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub enum IdleSound {
    /// A variant of the group 0 of a profile, a slot of `sndProfiles.pak`.
    Profile(u16),
    /// One of `count` sound ids from `first`, at random.
    Ids { first: u16, count: u16 },
}

/// The parameters of a motion of `MOTIONS.PAK`, in the global motion order: those of
/// `models.pak`, then those of `models03.pak`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Motion {
    /// The motion's `.GRN` file.
    pub name: String,
    pub value_06: u16,
    pub floats_46: [f32; 4],
    pub floats_62: [f32; 9],
    pub value_be: u16,
}

/// `bin/Balance.bin`: the client's balance variables (docs/02 §6.3), the 355 that the keys of the
/// balance text name by name, the image of all of them as stored, and the tables after the image
/// ("The tail of `Balance.bin`"), the drops' by their meaning and the others as words.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Balance {
    /// The named variables, in the image's order.
    pub variables: Vec<BalanceVariable>,
    /// The image's 1,224 words as stored, the named and the unnamed: each an f32 or an i32.
    pub image: Vec<u32>,
    /// The tail's first table: 33 lists of records of three words (−1 or effect numbers) by a
    /// key, 0 to 32; not followed.
    #[serde(default)]
    pub tail_1: Vec<BalanceList>,
    /// The tail's second table: the drops' weights of the quality keys 0 to 17, in their order.
    #[serde(default)]
    pub quality_weights: Vec<QualityWeight>,
    /// The tail's third table: the item maker's quality keys 0 to 15 and the values of the
    /// quality pass `0x5D2E40` they take.
    #[serde(default)]
    pub quality_passes: Vec<QualityPass>,
    /// The tail's fourth table: nine rows of seven words by a key, 1 to 9; not followed.
    #[serde(default)]
    pub tail_4: Vec<BalanceRow>,
    /// The tail's fifth table: the parameters of a chance by an effect's value.
    #[serde(default)]
    pub effect_chances: Vec<EffectChance>,
    /// The 570 words after the tail's tables, of another layout, not read.
    pub rest: Vec<u32>,
}

/// An entry of the tail's first table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct BalanceList {
    pub key: u32,
    pub records: Vec<[u32; 3]>,
}

/// The weight of a quality key in the drops (docs/re/items.md, "A pass: the quality key").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct QualityWeight {
    pub key: u32,
    pub weight: u32,
}

/// The value of the quality pass that a quality key of the item maker takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct QualityPass {
    pub key: u32,
    pub value: u32,
}

/// An entry of the tail's fourth table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct BalanceRow {
    pub key: u32,
    pub words: [u32; 7],
}

/// The parameters of a chance by the value `v` of an effect (`0x5F23A0`):
/// `(1 − a/(b + v))·100 + 0.5` per cent, at most `most`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct EffectChance {
    pub effect: u32,
    pub a: f32,
    pub b: f32,
    pub most: f32,
}

/// A balance variable that a key of the balance text names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct BalanceVariable {
    /// The key, as the client's parser has it (a key that ends in `:` heads a row of values;
    /// `GoldProLevel[1]` names `GoldProLevel`'s second value).
    pub name: String,
    /// Its offset in the image, in bytes.
    pub offset: u16,
    pub value: BalanceValue,
}

/// A balance variable's value, of its kind.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub enum BalanceValue {
    F32(f32),
    I32(i32),
}

impl Balance {
    /// The value of the float variable `name`.
    pub fn f32(&self, name: &str) -> Option<f32> {
        match self.variables.iter().find(|v| v.name == name)?.value {
            BalanceValue::F32(value) => Some(value),
            BalanceValue::I32(_) => None,
        }
    }

    /// The float `index` of the row that the key `name` heads, a row of values by the difficulty
    /// (docs/re/fight.md): the image's word `index` after the key's variable.
    pub fn row_f32(&self, name: &str, index: usize) -> Option<f32> {
        let variable = self.variables.iter().find(|v| v.name == name)?;
        let word = usize::from(variable.offset) / 4 + index;
        self.image.get(word).copied().map(f32::from_bits)
    }

    /// The integer `index` of the row that the key `name` heads, as [`Balance::row_f32`].
    pub fn row_i32(&self, name: &str, index: usize) -> Option<i32> {
        let variable = self.variables.iter().find(|v| v.name == name)?;
        let word = usize::from(variable.offset) / 4 + index;
        self.image
            .get(word)
            .map(|&word| i32::from_ne_bytes(word.to_ne_bytes()))
    }

    /// The value of the integer variable `name`.
    pub fn i32(&self, name: &str) -> Option<i32> {
        match self.variables.iter().find(|v| v.name == name)?.value {
            BalanceValue::I32(value) => Some(value),
            BalanceValue::F32(_) => None,
        }
    }
}

/// An item set of `bin/sets.bin`; slot 0 is unused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct ItemSet {
    pub slot: u32,
    pub items: [u32; 10],
    pub hash: u32,
    pub bytes: [u8; 4],
}

/// A weapon modifier of `bin/wpmod.bin`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct WeaponModifier {
    /// The 53 words of the fixed part.
    pub words: Vec<u32>,
    pub entries: Vec<[u32; 6]>,
}

/// A mercenary position of `bin/merc.bin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Mercenary {
    pub x: u32,
    pub y: u32,
    /// 0–3.
    pub kind: u32,
}

/// A multiplayer start point of `bin/MultiStart.bin`, in world coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct StartPoint {
    pub x: u32,
    pub y: u32,
}

/// The combat arts of the two tables compiled into `Sacred.exe` (docs/02 §6.5,
/// docs/re/arts.md), each in its table's order. A value by the level `L` is a pair `[a, b]`,
/// `a + L × b`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Arts {
    /// The first table's: the ids 1 to 72 but 54 and 68.
    pub spells: Vec<Spell>,
    /// The second table's: 94 of the ids 1000 to 1103.
    pub weapon_arts: Vec<WeaponArt>,
}

/// A spell, an art of the first table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Spell {
    pub id: u16,
    /// The `global.res` id of its name: the hash of the record's text index in decimal (docs/02
    /// §8).
    pub name: u32,
    /// That of its long description; eleven spells have the index 0, which names no string.
    pub description: u32,
    /// That of its short description, 600 indices after the long one.
    pub short_description: u32,
    /// The icon's texture name, `GUI_SPELL_IRRITATION.TGA`.
    pub icon: String,
    /// Three sprites of the interface's table for the icon, the last the one drawn.
    pub sprites: [u32; 3],
    /// The launch type: 1 on an enemy, 2 on a friend, 3 on the ground, 4 on the dead, 5 on
    /// itself, 0 none.
    pub launch: u16,
    /// The regeneration time in seconds at the speed 1.
    pub regeneration: [u16; 2],
    /// Thirteen effects: the physical, fire, magic and poison damage, the attack, the defence in
    /// percent, the physical, fire, magic and poison resistances, the speed, the mental and the
    /// physical regeneration.
    pub effects: [[u16; 2]; 13],
    /// Three values; the value 0 is the duration in seconds for most.
    pub values: [[u16; 2]; 3],
    /// Each value's cap, 0 for none.
    pub caps: [u16; 3],
    /// 1 water, 2 air, 3 earth, 4 fire, 5 lost, 0 none.
    pub element: u16,
    /// The classes that learn and use it, a mask: 1 the Seraphim, 2 the Gladiator, 4 the Mage, 8
    /// the Dark Elf, 0x10 the Wood Elf, 0x20 the Vampiress, 0x400000 the Dwarf, 0x800000 the
    /// Daemon.
    pub classes: u32,
    /// Bit 0 not on horseback, bit 1 on horseback only with a bow, bit 2 with a two-handed weapon
    /// only.
    pub flags: u8,
}

/// A weapon art, an art of the second table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct WeaponArt {
    pub id: u16,
    /// The `global.res` ids of its name and its descriptions, as a spell's.
    pub name: u32,
    pub description: u32,
    pub short_description: u32,
    /// The icon's texture name, `GUI_MOVE_ATTACKE.TGA`.
    pub icon: String,
    pub sprites: [u32; 3],
    /// A column of the balancing sheet, which no code is known to read.
    pub value_3c: u32,
    /// The regeneration time in seconds at the speed 1.
    pub regeneration: [f32; 2],
    /// The distance it is made from: 101 to 250 the reach of the two creatures, 0 where the
    /// creature stands, below 0 for the jumps.
    pub distance: i32,
    /// The launch type, as a spell's.
    pub launch: u8,
    /// For most arts the damage's factor of each strike; for the jumps 1003 and 1032 the radius
    /// they strike within as they land, for 1083 another value.
    pub factor: [f32; 2],
    /// A second value by the level, of each art its own: the damage's factor of the strikes of
    /// 1003 and 1083, the value that 1032 gives each creature it lands by, the kicks' own.
    pub value_58: [f32; 2],
    /// A whole value by the level: for the `Rundum` arts the radius in percent of the reach.
    pub value_60: [i32; 2],
    /// Another, of each art its own.
    pub value_68: [i32; 2],
    /// The classes, a mask as a spell's.
    pub classes: u32,
    /// The flags, as a spell's.
    pub flags: u8,
}

/// A used slot of `Triggers.PAK`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Trigger {
    pub slot: u32,
    pub id: u32,
    /// 20 bits (hypothesis: an object of `Static.PAK`).
    pub target: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_items_angle_is_the_float_across_its_packed_fields() {
        let item = |word| Item {
            slot: 5201,
            kind: 34,
            class: 0,
            subclass: 0,
            name: "chest.grn".into(),
            texture: 0,
            sprite: 0,
            byte_03: 0,
            words_08: [0; 2],
            words_14: [0; 8],
            bytes_34: [0; 3],
            byte_57: 0,
            words_58: [word, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        };
        // `chest.grn`'s record: `00 00 F0 C1` at `+0x57`, and a byte of the next field at `+0x5B`.
        assert_eq!(item(0x7FC1_F000).angle(), -30.0);
        // A hero's, 90.0.
        assert_eq!(item(0x0042_B400).angle(), 90.0);
        assert_eq!(item(0).angle(), 0.0);
    }
}
