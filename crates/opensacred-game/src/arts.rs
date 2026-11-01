//! The hero's combat arts (docs/re/arts.md, #506, #507): its template's at a new game
//! (`opensacred_rules::arts::Arts::new_game`), with the times of the arts' records of
//! `tables/arts.ron` shortened by its skills; their regeneration with the game's time at the
//! speeds of its attributes and items; and what the console's `arts` shows and does, as the
//! original's driver does: the list, an art taught as runes teach it, every art ready, and an
//! art's use, which starts the regeneration of every art of its kind until the hero makes them
//! itself (#509).

use std::path::Path;
use std::sync::Arc;

use bevy::prelude::*;
use opensacred_data::heroes::HeroArt;
use opensacred_data::tables::{ARTS, Arts as Tables, Balance};
use opensacred_rules::arts::{Arts, Context, Shortening, Speeds, Table};

use crate::fight::{Base, Experience, Holding};
use crate::files;
use crate::hero::Body;

/// The hero's arts, and what teaching one more needs.
#[derive(Resource, Clone, Debug, Default)]
pub struct HeroArts {
    pub arts: Arts,
    /// The hero's creature type, its class's number (`Class::creature`).
    pub kind: u32,
    /// The arts' two tables.
    pub tables: Arc<Tables>,
    /// The hero's skills and their levels with their items' bonuses, in their slots' order
    /// (`fight::hero_skills`).
    pub skills: Vec<(u8, u32)>,
    /// The curves of the skills that shorten the arts' times.
    pub shortening: Shortening,
    /// While a combo runs or its mark holds (`combos::ComboRun`), the times of the arts used,
    /// which go to the combos rather than to the arts' own regeneration (`0x562470`).
    pub combo_uses: Option<Vec<f32>>,
    /// The speeds its arts regenerate at, of the latest tick ([`regenerate`]), over which an
    /// art's tooltip divides its time (`0x5893B0`, #1694).
    pub speeds: Speeds,
}

impl HeroArts {
    /// The arts of a new game of the hero of creature type `kind` and skills `skills`, of its
    /// template's `template`; the tables of `assets`, or none where `tables/arts.ron` is lacking
    /// (assets of a converter before #505), the arts then keeping their template's times; the
    /// skills' curves of `balance`, the original's without it.
    pub fn new_game(
        assets: &Path,
        kind: u32,
        template: &[HeroArt],
        skills: Vec<(u8, u32)>,
        balance: Option<&Balance>,
    ) -> Self {
        let path = assets.join("tables").join(ARTS);
        let tables = match files::read_to_string(&path) {
            Ok(text) => ron::from_str::<Tables>(&text).unwrap_or_else(|e| {
                warn!("arts: {}: {e}", path.display());
                Tables::default()
            }),
            Err(e) => {
                warn!("arts: {}: {e}", path.display());
                Tables::default()
            }
        };
        let mut hero = Self {
            arts: Arts::default(),
            kind,
            tables: Arc::new(tables),
            skills,
            shortening: balance.map(Shortening::of).unwrap_or_default(),
            combo_uses: None,
            speeds: Speeds::default(),
        };
        hero.arts = Arts::new_game(template, &hero.context());
        hero
    }

    /// What the stats' recomputation makes the hero's arts' times of.
    fn context(&self) -> Context<'_> {
        Context {
            tables: &self.tables,
            kind: self.kind,
            skills: &self.skills,
            shortening: &self.shortening,
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
        let mut arts = std::mem::take(&mut self.arts);
        let rose = arts.teach(table, art, levels, &self.context());
        self.arts = arts;
        if rose {
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

    /// A use of `art`, as its executor starts the regeneration at its first strike or its cast
    /// (`0x562470`): its time added to what is still to run of every art of its kind, without a
    /// group. The hero needs not hold it, as the original's use of an art it lacks. While a
    /// combo runs or its mark holds, the time goes to the combos instead ([`Self::combo_uses`]).
    pub fn start(&mut self, art: u16) {
        let kind = self.arts.get(art).map_or_else(
            || {
                if art < 1000 || matches!(art, 1019 | 1085..=1087) {
                    Table::Spells
                } else {
                    Table::WeaponArts
                }
            },
            opensacred_rules::arts::Entry::kind,
        );
        let time = self.arts.time_of(art, &self.tables);
        if let Some(uses) = &mut self.combo_uses {
            uses.push(time);
            return;
        }
        self.arts.start(time, kind, None, 0);
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

/// The arts' regeneration (`0x562730`), the hero's alone, on the world's tick as the creatures'
/// updates run it (`frame::WorldTick`, #627): what is still to run of each art goes down by the
/// tick's seconds times its factor and its kind's speed, the speeds by the hero's level,
/// attributes, items and effects (`fight::arts_speeds_with`, #763).
#[expect(
    clippy::type_complexity,
    reason = "a Bevy system's queries carry their filters"
)]
fn regenerate(
    tick: Res<crate::frame::WorldTick>,
    experience: Res<Experience>,
    arts: Option<ResMut<HeroArts>>,
    heroes: Query<(&Base, Option<&Holding>, Option<&crate::effects::Effects>), With<Body>>,
) {
    let (Some(seconds), Some(mut arts), Some((base, holding, effects))) =
        (tick.seconds, arts, heroes.iter().next())
    else {
        return;
    };
    let holds = holding.map(|holding| holding.0.clone()).unwrap_or_default();
    let effects = effects
        .map(crate::effects::Effects::list)
        .unwrap_or_default();
    let speeds =
        opensacred_rules::fight::arts_speeds_with(base, experience.level, &holds, &effects);
    arts.arts.regenerate(seconds, speeds);
    arts.speeds = speeds;
    // The skills with the bonuses of the items it holds and wears (#1717), as they change.
    let skills = crate::fight::hero_skills(base, experience.level, &holds.items);
    if arts.skills != skills {
        arts.skills = skills;
    }
}

/// The hero's arts' regeneration.
pub struct ArtsPlugin;

impl Plugin for ArtsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            regenerate
                .in_set(crate::frame::Unpaused)
                .after(crate::frame::Step::Tick)
                .run_if(crate::frame::ticked),
        );
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
        let mut arts = HeroArts::new_game(Path::new("no assets"), 1, &template, Vec::new(), None);
        assert_eq!(
            arts.lines(),
            ["art 1071 level 1 time 15.0 s, factor 1, to run 0.0 s"]
        );
        // A use starts its own kind's regeneration, of its own time.
        arts.start(1071);
        assert_eq!(
            arts.lines(),
            ["art 1071 level 1 time 15.0 s, factor 1, to run 15.0 s"]
        );
        arts.ready();
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
