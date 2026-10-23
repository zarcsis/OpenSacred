//! The hero's combat arts (docs/re/arts.md, #506): its template's at a new game
//! (`opensacred_rules::arts::Arts::new_game`), with the times of the arts' records of
//! `tables/arts.ron`, and what the console's `arts` shows and does, as the original's driver does:
//! the list, an art taught as runes teach it, every art ready. Nothing makes them yet (#509), and
//! their regeneration does not run (#507).

use std::path::Path;
use std::sync::Arc;

use bevy::prelude::*;
use opensacred_data::heroes::HeroArt;
use opensacred_data::tables::{ARTS, Arts as Tables};
use opensacred_rules::arts::{Arts, Table};

/// The hero's arts, and what teaching one more needs.
#[derive(Resource, Clone, Debug, Default)]
pub struct HeroArts {
    pub arts: Arts,
    /// The hero's creature type, its class's number (`Class::creature`).
    pub kind: u32,
    /// The arts' two tables.
    pub tables: Arc<Tables>,
}

impl HeroArts {
    /// The arts of a new game of the hero of creature type `kind`, of its template's `template`;
    /// the tables of `assets`, or none where `tables/arts.ron` is lacking (assets of a converter
    /// before #505), the arts then keeping their template's times.
    pub fn new_game(assets: &Path, kind: u32, template: &[HeroArt]) -> Self {
        let path = assets.join("tables").join(ARTS);
        let tables = match std::fs::read_to_string(&path) {
            Ok(text) => ron::from_str::<Tables>(&text).unwrap_or_else(|e| {
                warn!("arts: {}: {e}", path.display());
                Tables::default()
            }),
            Err(e) => {
                warn!("arts: {}: {e}", path.display());
                Tables::default()
            }
        };
        Self {
            arts: Arts::new_game(template, &tables),
            kind,
            tables: Arc::new(tables),
        }
    }

    /// Teaches `art`, below 1000 a spell, by `levels`, as that many runes would, whatever the
    /// hero's class.
    pub fn learn(&mut self, art: u16, levels: u8) -> Result<(), String> {
        let table = if art < 1000 {
            Table::Spells
        } else {
            Table::WeaponArts
        };
        if self.arts.teach(table, art, levels, self.kind, &self.tables) {
            Ok(())
        } else {
            Err(format!("{art} cannot rise by {levels}: 255 at most"))
        }
    }

    /// Every art ready at once.
    pub fn ready(&mut self) {
        for entry in &mut self.arts.entries {
            entry.remaining = 0.0;
        }
    }

    /// A line for each art, as the original's driver prints them: its table, id, level and
    /// bonus, whether a rune taught it, its shortcut's slot, its time, its factor and what is
    /// still to run.
    pub fn lines(&self) -> Vec<String> {
        if self.arts.entries.is_empty() {
            return vec!["no combat arts".into()];
        }
        self.arts
            .entries
            .iter()
            .map(|e| {
                let table = match e.table {
                    Table::Spells => "spell",
                    Table::WeaponArts => "art",
                };
                let bonus = if e.bonus > 0 {
                    format!("+{}", e.bonus)
                } else {
                    String::new()
                };
                let taught = if e.taught() { " learnt" } else { "" };
                let slot = match e.slot() {
                    0 => String::new(),
                    slot => format!(" slot {slot}"),
                };
                format!(
                    "{table} {} level {}{bonus}{taught}{slot} time {:.1} s, factor {}, to run {:.1} s",
                    e.art, e.level, e.time, e.factor, e.remaining
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arts_print_as_the_original_drivers() {
        let template = [HeroArt {
            table: 2,
            id: 1071,
            level: 1,
            bonus: 0,
            flags: 0,
            time: 15.0,
            factor: 1.0,
            remaining: 0.0,
        }];
        let mut arts = HeroArts::new_game(Path::new("no assets"), 1, &template);
        assert_eq!(
            arts.lines(),
            ["art 1071 level 1 time 15.0 s, factor 1, to run 0.0 s"]
        );
        // Without the tables an art taught keeps no time; with them it takes its record's.
        arts.learn(18, 2).unwrap();
        assert_eq!(
            arts.lines()[1],
            "spell 18 level 2 learnt time 0.0 s, factor 1, to run 0.0 s"
        );
        assert!(arts.learn(18, 255).is_err());
        arts.arts.entries.clear();
        assert_eq!(arts.lines(), ["no combat arts"]);
    }
}
