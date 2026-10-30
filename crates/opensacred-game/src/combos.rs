//! The hero's combos (#1530; docs/re/arts.md, "A hero's combos"): the four that a new game gives
//! it empty (`0x55E1B0`, `opensacred_rules::combos`), their regeneration on the world's tick at
//! the speed of the hero's regenerations, their images in the bar's slots (the combos' table at
//! `0x955348`) and their description's lines (`0x581550`); the console's `combos` lists them and
//! makes one as the combat master does. The combos page shows them (`combos_page`); their run in
//! play is #1531's, the master's window #1540's.

use bevy::prelude::*;
use opensacred_rules::combos::{self, COMBOS, Combo, STEPS, Step};

use crate::arts::HeroArts;
use crate::fight::{Base, Experience, Holding};
use crate::game::AppState;
use crate::hero::Body;
use crate::lines::Texts;

/// The image of the first combo's grey picture; each combo's are three, grey, pale and vivid, in
/// `gui_combo01` (the table's `+0x24` to `+0x2C`).
const FIRST_IMAGE: u32 = 801;

/// The hero's combos.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct HeroCombos(pub [Combo; COMBOS]);

impl Default for HeroCombos {
    fn default() -> Self {
        Self(combos::new_hero())
    }
}

impl HeroCombos {
    /// The combo of the id `id`, none for another art.
    pub fn get(&self, id: u16) -> Option<&Combo> {
        combos::index_of(id).and_then(|index| self.0.get(index))
    }

    /// Lines for the console: each combo's steps, its time, its speed and what of it has
    /// regenerated.
    pub fn lines(&self) -> Vec<String> {
        self.0
            .iter()
            .enumerate()
            .map(|(index, combo)| {
                let steps: Vec<String> = combo
                    .steps
                    .iter()
                    .flatten()
                    .map(|(art, level)| format!("{art}:{level}"))
                    .collect();
                let steps = if steps.is_empty() {
                    "no steps".to_owned()
                } else {
                    steps.join(" ")
                };
                format!(
                    "combo {} ({:#x}): {steps}, time {:.1} s, speed {:.2}, regenerated {:.1} s, \
                     shown {:.1} s",
                    index + 1,
                    combo.id,
                    combo.time,
                    combo.speed,
                    combo.regenerated,
                    combo.shown_time()
                )
            })
            .collect()
    }
}

/// Whether the art `art` is one of the hero's combos (the cursor's and a slot's kind 4).
pub(crate) fn is_combo(art: u16) -> bool {
    combos::index_of(art).is_some()
}

/// The three images of the combo `id`, grey, pale and vivid.
pub(crate) fn sprites(id: u16) -> Option<[u32; 3]> {
    let first = FIRST_IMAGE + 3 * u32::try_from(combos::index_of(id)?).ok()?;
    Some([first, first + 1, first + 2])
}

/// The steps that `words` name, each `ART:LEVEL`, at most four.
pub(crate) fn parse_steps(words: &[String]) -> Result<[Step; STEPS], String> {
    if words.len() > STEPS {
        return Err(format!("a combo has {STEPS} steps at most"));
    }
    let mut steps = [None; STEPS];
    for (step, word) in steps.iter_mut().zip(words) {
        let parsed = word
            .split_once(':')
            .and_then(|(art, level)| Some((art.parse().ok()?, level.parse().ok()?)))
            .filter(|&(_, level): &(u16, u8)| level > 0)
            .ok_or_else(|| format!("{word}: a step is ART:LEVEL, its level 1 or more"))?;
        *step = Some(parsed);
    }
    Ok(steps)
}

/// The name of the art `art`, of `arts`' tables and `texts`.
pub(crate) fn art_name<'a>(art: u16, arts: &HeroArts, texts: &'a Texts) -> Option<&'a str> {
    let tables = &arts.tables;
    let name = tables
        .spells
        .iter()
        .find(|spell| spell.id == art)
        .map(|spell| spell.name)
        .or_else(|| {
            tables
                .weapon_arts
                .iter()
                .find(|record| record.id == art)
                .map(|record| record.name)
        })?;
    texts.id(name)
}

/// The description's lines of `combo` (`0x581550`'s mode 2): each step's art's name and its level.
pub(crate) fn step_lines(combo: &Combo, arts: &HeroArts, texts: &Texts) -> Vec<String> {
    combo
        .steps
        .iter()
        .flatten()
        .map(|&(art, level)| {
            let name = art_name(art, arts, texts).map_or_else(|| art.to_string(), str::to_owned);
            format!("{name}: {level}")
        })
        .collect()
}

/// The combos' regeneration on the world's tick, as the arts': each at the speed of the hero's
/// regenerations (`Combo::speed_of`), its attributes `MagicRegen` and `PhysicalRegen` grown by its
/// level and raised by its items' (`fight::regenerations`).
fn regenerate(
    tick: Res<crate::frame::WorldTick>,
    experience: Res<Experience>,
    combos: Option<ResMut<HeroCombos>>,
    heroes: Query<(&Base, Option<&Holding>), With<Body>>,
) {
    let (Some(seconds), Some(mut combos), Some((base, holding))) =
        (tick.seconds, combos, heroes.iter().next())
    else {
        return;
    };
    let holds = holding.map(|holding| holding.0.clone()).unwrap_or_default();
    let (magic, physical) = opensacred_rules::fight::regenerations(base, experience.level, &holds);
    for combo in &mut combos.0 {
        let speed = combo.speed_of(magic, physical);
        if combo.speed != speed {
            combo.speed = speed;
        }
        if !combo.ready() {
            combo.regenerate(seconds);
        }
    }
}

/// A new game's combos, empty.
fn reset(mut commands: Commands) {
    commands.insert_resource(HeroCombos::default());
}

/// The hero's combos.
pub struct CombosPlugin;

impl Plugin for CombosPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HeroCombos>()
            .add_systems(OnEnter(AppState::InGame), reset)
            .add_systems(
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
    fn each_combo_has_its_three_images() {
        assert_eq!(sprites(0x200), Some([801, 802, 803]));
        assert_eq!(sprites(0x203), Some([810, 811, 812]));
        assert_eq!(sprites(1071), None);
        assert!(is_combo(0x201) && !is_combo(0x204) && !is_combo(13));
    }

    #[test]
    fn the_console_names_a_combos_steps() {
        let words = |text: &str| text.split(' ').map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            parse_steps(&words("1070:3 1071:2 20:1")),
            Ok([Some((1070, 3)), Some((1071, 2)), Some((20, 1)), None])
        );
        assert_eq!(parse_steps(&[]), Ok([None; STEPS]));
        assert!(parse_steps(&words("1070:0")).is_err());
        assert!(parse_steps(&words("1070")).is_err());
        assert!(parse_steps(&words("1:1 2:1 3:1 4:1 5:1")).is_err());
    }
}
