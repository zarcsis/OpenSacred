//! A creature's combat arts (docs/re/arts.md): the list its stats hold, an art's level in play,
//! each entry's regeneration time by its record, the runes that teach the arts, and the hero's
//! arts at a new game. `opensacred-game` keeps the hero's list; the regeneration that runs it
//! down (#507), the skills that shorten the times (`0x566070`, #507) and the arts' strikes (#508)
//! are not here.

use opensacred_data::heroes::HeroArt;
use opensacred_data::tables::Arts as Tables;

/// The bit of an entry's flags that a rune sets: on every art it adds (`0x580750`) and on every
/// art it raises for good (`0x581F80`).
pub const TAUGHT: u16 = 1;
/// The Vampiress's arts by day and by night, in pairs (`0x580750`, `0x552370`).
const VAMPIRESS: [(u16, u16); 8] = [
    (1022, 1023),
    (1024, 1025),
    (1026, 1027),
    (1028, 1029),
    (1073, 1038),
    (1074, 1039),
    (1075, 1040),
    (1076, 1041),
];
/// The Daemon's arts in pairs (`0x891154`, read by `0x552370`).
const DAEMON: [(u16, u16); 5] = [
    (1094, 1091),
    (1097, 1092),
    (1099, 1101),
    (1098, 1102),
    (1100, 1103),
];
/// The arts that `0x581F80` raises with an art, where the creature holds them.
const RAISED_WITH: [(u16, u16); 19] = [
    (1022, 1023),
    (1024, 1025),
    (1026, 1027),
    (1028, 1029),
    (1000, 1038),
    (1001, 1039),
    (1002, 1040),
    (1009, 1041),
    (1073, 1038),
    (1074, 1039),
    (1075, 1040),
    (1076, 1041),
    (1025, 1024),
    (1027, 1026),
    (1029, 1028),
    (1038, 1073),
    (1039, 1074),
    (1040, 1075),
    (1041, 1076),
];
/// The arts that `0x581D40` finds for the generic `Rundum`, `Hardhit`, `Attacke` and `Kick`
/// (1000, 1001, 1002, 1009): every class's own.
const OWN: [(u16, &[u16]); 4] = [
    (1000, &[1078, 1090, 1057, 1069, 1065, 1061, 1073, 1038]),
    (1001, &[1079, 1091, 1058, 1062, 1066, 1070, 1074, 1039]),
    (1002, &[1080, 1092, 1059, 1063, 1067, 1071, 1075, 1040]),
    (1009, &[1093, 1060, 1064, 1068, 1072, 1076, 1041]),
];

/// The table an art is of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Table {
    /// The first, the spells.
    Spells,
    /// The second, the weapon arts.
    WeaponArts,
}

impl Table {
    /// The stats' number of the table: 1 or 2.
    pub fn number(self) -> u32 {
        match self {
            Self::Spells => 1,
            Self::WeaponArts => 2,
        }
    }

    /// The table of the stats' number `number`.
    pub fn of_number(number: u32) -> Option<Self> {
        match number {
            1 => Some(Self::Spells),
            2 => Some(Self::WeaponArts),
            _ => None,
        }
    }
}

/// An entry of a creature's list of arts (the stats' `+0x102`, 22 bytes each).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub table: Table,
    pub art: u16,
    pub level: u8,
    /// A bonus to the level, of the items and the sets (`0x571710`).
    pub bonus: u8,
    /// [`TAUGHT`], and the shortcut's slot in the high byte (`0x580C80`, `0x580D00`).
    pub flags: u16,
    /// The regeneration time, in seconds.
    pub time: f32,
    /// The regeneration's factor, 1.0 (`0x571080`).
    pub factor: f32,
    /// The regeneration still to run, in seconds.
    pub remaining: f32,
}

impl Entry {
    /// A template's entry, as the hero's loader reads it (`0x585C60`); none of a table but the
    /// two.
    pub fn of_template(art: &HeroArt) -> Option<Self> {
        Some(Self {
            table: Table::of_number(art.table)?,
            art: art.id,
            level: art.level,
            bonus: art.bonus,
            flags: art.flags,
            time: art.time,
            factor: art.factor,
            remaining: art.remaining,
        })
    }

    /// Whether a rune taught it ([`TAUGHT`]).
    pub fn taught(&self) -> bool {
        self.flags & TAUGHT != 0
    }

    /// The shortcut's slot, 0 for none.
    pub fn slot(&self) -> u8 {
        self.flags.to_be_bytes()[0]
    }

    /// The level in play: the level and the bonus.
    pub fn in_play(&self) -> u32 {
        u32::from(self.level) + u32::from(self.bonus)
    }
}

/// A creature as its arts take it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Creature {
    /// Its type (`+0x10`): a hero's is its class, 1 the Seraphim to 9 the Daemon (6 and 7 the
    /// Vampiress by day and by night).
    pub kind: u32,
    /// Its level (the stats' `+0x56`).
    pub level: u32,
    /// Whether it is a player's.
    pub player: bool,
}

/// The bit of a creature's class in the tables' masks (`0x4D1B20`, `0x4D24C0`); 0 for a creature
/// that is no hero.
pub fn class_mask(kind: u32) -> u32 {
    match kind {
        1 => 0x1,
        2 => 0x2,
        3 => 0x4,
        4 => 0x8,
        5 => 0x10,
        6 | 7 => 0x20,
        8 => 0x0040_0000,
        9 => 0x0080_0000,
        _ => 0,
    }
}

/// The class's own `Rundum` and `Hardhit` for the generic 1061 and 1062 (`0x580A10`): the Mage's
/// and the Wood Elf's stay 1061 and 1062, the Wood Elf's own.
pub fn own(art: u16, kind: u32) -> u16 {
    match (art, kind) {
        (1061, 1) => 1069,
        (1061, 2) => 1057,
        (1061, 4) => 1065,
        (1061, 6 | 7) => 1073,
        (1061, 8) => 1078,
        (1061, 9) => 1090,
        (1062, 1) => 1070,
        (1062, 2) => 1058,
        (1062, 4) => 1066,
        (1062, 6 | 7) => 1074,
        (1062, 8) => 1079,
        (1062, 9) => 1091,
        _ => art,
    }
}

/// An art's counterpart in the creature's other form, as a rune takes it (`0x552370` with its
/// flag set): the Daemon's pairs both ways, the Vampiress's night art for her day art by day (6)
/// and her day art for her night art by night (7); the art itself for any other.
pub fn counterpart(art: u16, kind: u32) -> u16 {
    match kind {
        9 => DAEMON
            .iter()
            .find_map(|&(a, b)| {
                if a == art {
                    Some(b)
                } else if b == art {
                    Some(a)
                } else {
                    None
                }
            })
            .unwrap_or(art),
        6 => VAMPIRESS
            .iter()
            .find(|&&(_, night)| night == art)
            .map_or(art, |&(day, _)| day),
        7 => VAMPIRESS
            .iter()
            .find(|&&(day, _)| day == art)
            .map_or(art, |&(_, night)| night),
        _ => art,
    }
}

/// An art's regeneration time at the level `level`, from its record: a spell's `base + step·L`
/// in whole seconds (`0x4D25C0`), a weapon art's in floats (`0x4D1C00`); the first table is
/// searched first (`0x571080`), and none holds an art of neither.
pub fn time(tables: &Tables, art: u16, level: u32) -> Option<f64> {
    if let Some(spell) = tables.spells.iter().find(|s| s.id == art) {
        let [base, step] = spell.regeneration.map(u32::from);
        return Some(f64::from(base + step * level));
    }
    let record = tables.weapon_arts.iter().find(|a| a.id == art)?;
    let [base, step] = record.regeneration.map(f64::from);
    Some(f64::from(level).mul_add(step, base))
}

/// A float of the original's stores to an `f32`.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a regeneration time, a few hundred seconds, rounded to the f32 the original stores"
)]
fn narrow(value: f64) -> f32 {
    value as f32
}

/// Why a rune does not teach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// It names the art 0.
    NoArt,
    /// Neither table holds its art for the reader's class.
    NotForClass,
    /// Its art's level is at 255 already.
    Highest,
}

/// A creature's list of arts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Arts {
    pub entries: Vec<Entry>,
}

impl Arts {
    /// The hero's arts at a new game, of its template's: the entries that no rune taught, with
    /// their times by their records (**hypothesis**: a new game drops the arts its template
    /// marks [`TAUGHT`]; the Seraphim starts with 1071 alone and the Gladiator with 1059 alone,
    /// as the original's do, seen 2026-10-23, #506).
    pub fn new_game(template: &[HeroArt], tables: &Tables) -> Self {
        let mut arts = Self {
            entries: template
                .iter()
                .filter_map(Entry::of_template)
                .filter(|entry| !entry.taught())
                .collect(),
        };
        arts.times(tables);
        arts
    }

    /// The index of the entry of `art` (`0x581D40`): for the generic `Rundum`, `Hardhit`,
    /// `Attacke` and `Kick` (1000, 1001, 1002, 1009) the first entry of any class's own, or of the
    /// art itself.
    pub fn index(&self, art: u16) -> Option<usize> {
        let own = OWN
            .iter()
            .find(|&&(generic, _)| generic == art)
            .map_or(&[][..], |&(_, own)| own);
        self.entries
            .iter()
            .position(|e| e.art == art || own.contains(&e.art))
    }

    /// The entry of `art` itself.
    pub fn get(&self, art: u16) -> Option<&Entry> {
        self.entries.iter().find(|e| e.art == art)
    }

    /// The level that `creature` makes `art` at (`0x580A10`): its entry's level and bonus, the
    /// class's own for the generic 1061 and 1062; a creature that is no player's and lacks the
    /// art makes it at `⌊its level / 5⌋ + 1`, a player's not at all.
    pub fn level_in_play(&self, art: u16, creature: Creature) -> Option<u32> {
        match self.get(own(art, creature.kind)) {
            Some(entry) => Some(entry.in_play()),
            None if !creature.player => Some(creature.level / 5 + 1),
            None => None,
        }
    }

    /// The sum of the levels of the arts without their bonuses, the drops' `a` for a rune
    /// (`0x5598B0`, docs/re/items.md).
    pub fn levels(&self) -> u32 {
        self.entries.iter().map(|e| u32::from(e.level)).sum()
    }

    /// Every entry's time by its record (`0x571080`): the time at its level, and with its bonus
    /// half of what the bonus adds where that is more, `v(L) + (v(L + b) − v(L))/2`; the factor
    /// 1.0. An art that neither table holds keeps its time.
    pub fn times(&mut self, tables: &Tables) {
        for entry in &mut self.entries {
            entry.factor = 1.0;
            let Some(at_level) = time(tables, entry.art, u32::from(entry.level)) else {
                continue;
            };
            entry.time = narrow(at_level);
            if let Some(with_bonus) = time(tables, entry.art, entry.in_play()) {
                let stored = f64::from(entry.time);
                if stored < with_bonus {
                    entry.time = narrow((with_bonus - stored).mul_add(0.5, stored));
                }
            }
        }
    }

    /// Adds `art` of `table` at level 0, marked [`TAUGHT`], unless the creature holds it
    /// (`0x580750`): with the Vampiress's other art of its pair, and with the art's counterpart
    /// ([`counterpart`]) where it is another, whether held or not; then every time
    /// ([`Self::times`]).
    pub fn add(&mut self, table: Table, art: u16, kind: u32, tables: &Tables) {
        if self.get(art).is_some() {
            return;
        }
        let entry = |art| Entry {
            table,
            art,
            level: 0,
            bonus: 0,
            flags: TAUGHT,
            time: 0.0,
            factor: 0.0,
            remaining: 0.0,
        };
        self.entries.push(entry(art));
        if matches!(kind, 6 | 7)
            && let Some(&(_, night)) = VAMPIRESS.iter().find(|&&(day, _)| day == art)
        {
            self.entries.push(entry(night));
        }
        let other = counterpart(art, kind);
        if other != 0 && other != art {
            self.entries.push(entry(other));
        }
        self.times(tables);
    }

    /// Raises the entry at `index` by `levels` (`0x581F80`): for good its level, marked
    /// [`TAUGHT`], with every time recomputed, else its bonus; refused where that passes 255. A
    /// time that grows adds as much to the regeneration still to run. With `linked`, the arts
    /// raised with it (`RAISED_WITH`) that the creature holds rise too, or else its
    /// [`counterpart`]. Whether it rose.
    pub fn raise(
        &mut self,
        index: usize,
        levels: u8,
        for_good: bool,
        linked: bool,
        kind: u32,
        tables: &Tables,
    ) -> bool {
        let Some(entry) = self.entries.get_mut(index) else {
            return false;
        };
        let value = if for_good {
            &mut entry.level
        } else {
            &mut entry.bonus
        };
        // At most 255.
        let Ok(raised) = u8::try_from(u32::from(*value) + u32::from(levels)) else {
            return false;
        };
        *value = raised;
        if for_good {
            entry.flags |= TAUGHT;
        }
        let (art, before) = (entry.art, entry.time);
        if for_good {
            self.times(tables);
        }
        if let Some(entry) = self.entries.get_mut(index)
            && before < entry.time
        {
            entry.remaining += entry.time - before;
        }
        if linked {
            let with = RAISED_WITH
                .iter()
                .find(|&&(raised, _)| raised == art)
                .and_then(|&(_, with)| self.index(with));
            match with {
                Some(with) => {
                    self.raise(with, levels, for_good, false, kind, tables);
                }
                None => {
                    let other = counterpart(art, kind);
                    if other != art
                        && let Some(other) = self.index(other)
                    {
                        self.raise(other, levels, for_good, false, kind, tables);
                    }
                }
            }
        }
        true
    }

    /// Teaches `art` of `table` by `levels`, as that many runes would (`0x580750`, `0x581D40`,
    /// `0x581F80` for good with the linked arts), whatever the creature's class; whether it rose.
    pub fn teach(
        &mut self,
        table: Table,
        art: u16,
        levels: u8,
        kind: u32,
        tables: &Tables,
    ) -> bool {
        self.add(table, art, kind, tables);
        self.index(art)
            .is_some_and(|index| self.raise(index, levels, true, true, kind, tables))
    }

    /// A rune of `art` read by a creature of type `kind` (`cItemUpgrade`, `0x5CD100`), its
    /// requirements met (`0x54ABE0`, the caller's): the art of the second table where that holds
    /// it for the class, else of the first, else refused; added at level 0 if lacking and raised
    /// by 1 for good. A refused rune is kept, a read one used up (its flag `0x8000000`).
    pub fn read_rune(&mut self, art: u16, kind: u32, tables: &Tables) -> Result<(), Refusal> {
        if art == 0 {
            return Err(Refusal::NoArt);
        }
        let class = class_mask(kind);
        let table = if tables
            .weapon_arts
            .iter()
            .any(|a| a.id == art && a.classes & class != 0)
        {
            Table::WeaponArts
        } else if tables
            .spells
            .iter()
            .any(|s| s.id == art && s.classes & class != 0)
        {
            Table::Spells
        } else {
            return Err(Refusal::NotForClass);
        };
        if self.teach(table, art, 1, kind, tables) {
            Ok(())
        } else {
            Err(Refusal::Highest)
        }
    }
}

#[cfg(test)]
mod tests;
