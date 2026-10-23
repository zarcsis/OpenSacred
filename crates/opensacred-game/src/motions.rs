//! A creature's clips by the original's motion slots (docs/re/motions.md): a model's file keeps,
//! with each clip, the slots of its `ModelDesc` that name it (`extras.slots`, docs/02 §4.1), and
//! the client plays for an action the slot that a table gives by the class of what the creature
//! holds, or the nearest slot the model has (`0x5467A0`).

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::files;

/// What a creature does, of the client's actions (`0x5467A0`'s 0, 1 and 2): the clip it plays,
/// and the gait it moves at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Stand,
    Walk,
    Run,
}

/// The class of what a creature holds (`0x546F80`), by which it picks its clips: `Weapon.pak
/// +0x1E` of the weapon in its right hand, else of the one in its left, 0 for bare hands; with
/// a weapon in each hand, the left not a shield, class 8, two blades (0 if neither has a class);
/// class 14 counts as 12. `left` is the left hand's item, whether a shield, and its class.
pub fn class(left: Option<(bool, u8)>, right: Option<u8>) -> u8 {
    let class = match (left, right) {
        (Some((false, left)), Some(right)) => {
            if left == 0 && right == 0 {
                0
            } else {
                8
            }
        }
        (_, Some(right)) => right,
        (Some((_, left)), None) => left,
        (None, None) => 0,
    };
    if class == 14 { 12 } else { class }
}

/// The slot of `action` for `class` (`0x956E40`, a row of 20 slots an action): the 13 classes
/// from 0, bare hands, to 12 in order from the action's first slot, standing 1, walking 27,
/// running 40; class 13 has slots of its own, class 14 takes 12's, and the rest of the row is 0.
fn table(action: Action, class: u8) -> u16 {
    let first = match action {
        Action::Stand => 1,
        Action::Walk => 27,
        Action::Run => 40,
    };
    match class {
        0..=12 => first + u16::from(class),
        13 => match action {
            Action::Stand => 235,
            Action::Walk => 237,
            Action::Run => 238,
        },
        14 => first + 12,
        _ => 0,
    }
}

/// The slot a creature plays for `action` holding `class`, of a model that has the slots `has`,
/// and the gait it moves at, as `0x5467A0` picks them: the table's slot (`table`) where the
/// model has it; else, standing, slot 2 where it has that, else 1; walking, the first it has of
/// the walks 27, 28 and 29, then of the runs 40, 41 and 42, at a run, else the standing slot;
/// running, the runs first, then the walks, at a walk. Its other cases are not followed: a
/// mounted creature's 179, the small spider's walk, the fight stance (#155).
pub fn slot(action: Action, class: u8, has: impl Fn(u16) -> bool) -> (u16, Action) {
    let slot = table(action, class);
    if slot != 0 && has(slot) {
        return (slot, action);
    }
    let walks = [27, 28, 29].map(|s| (s, Action::Walk));
    let runs = [40, 41, 42].map(|s| (s, Action::Run));
    let tried = match action {
        Action::Stand => None,
        Action::Walk => walks.into_iter().chain(runs).find(|&(s, _)| has(s)),
        Action::Run => runs.into_iter().chain(walks).find(|&(s, _)| has(s)),
    };
    tried.unwrap_or((if has(2) { 2 } else { 1 }, action))
}

/// A model's clips by their slots.
#[derive(Clone, Debug, Default)]
pub struct Slots(HashMap<u16, String>);

impl Slots {
    /// Slots of the given clips, for the tests.
    #[cfg(test)]
    pub(crate) fn of<'a>(slots: impl IntoIterator<Item = (u16, &'a str)>) -> Self {
        Self(
            slots
                .into_iter()
                .map(|(slot, name)| (slot, name.to_owned()))
                .collect(),
        )
    }

    /// The slots of the clips of the `.glb` at `path`, from its JSON chunk.
    pub fn read(path: &Path) -> Result<Self> {
        let what = || format!("reading {}", path.display());
        let mut file = files::open(path).with_context(what)?;
        let mut header = [0; 20];
        file.read_exact(&mut header).with_context(what)?;
        if &header[..4] != b"glTF" || &header[16..] != b"JSON" {
            bail!("{}: not a binary glTF with a JSON chunk", path.display());
        }
        let length = u32::from_le_bytes([header[12], header[13], header[14], header[15]]);
        let mut json = vec![0; usize::try_from(length)?];
        file.read_exact(&mut json).with_context(what)?;
        Self::parse(&json).with_context(what)
    }

    /// The slots of the clips of a glTF's JSON: each animation's name by the slots in its
    /// `extras.slots`.
    fn parse(json: &[u8]) -> Result<Self> {
        let root: Value = serde_json::from_slice(json).context("parsing the glTF JSON")?;
        let mut slots = HashMap::new();
        for animation in root["animations"].as_array().into_iter().flatten() {
            let Some(name) = animation["name"].as_str() else {
                continue;
            };
            for slot in animation["extras"]["slots"]
                .as_array()
                .into_iter()
                .flatten()
            {
                if let Some(slot) = slot.as_u64().and_then(|s| u16::try_from(s).ok()) {
                    slots.entry(slot).or_insert_with(|| name.to_owned());
                }
            }
        }
        Ok(Self(slots))
    }

    /// The clip a creature plays for `action` holding `class`, and the gait it moves at; `None`
    /// where the model has no clip in the slot it falls back to.
    pub fn clip(&self, action: Action, class: u8) -> Option<(&str, Action)> {
        let (slot, gait) = slot(action, class, |s| self.0.contains_key(&s));
        self.0.get(&slot).map(|name| (name.as_str(), gait))
    }

    /// The clip of the talk, slot 177 (`*_TALK_A`), which a townsperson plays once as it meets
    /// its partner (`0x4FC490`) or reaches a trader.
    pub fn talk(&self) -> Option<&str> {
        self.slot(177)
    }

    /// The clip in `slot`, where the model has one.
    pub fn slot(&self, slot: u16) -> Option<&str> {
        self.0.get(&slot).map(String::as_str)
    }

    /// The fight stance's clip for `class`, only where the model has its slot: 14 to 26 for the
    /// classes 0 to 12 (class 14 taking 12's), 236 for class 13 (`0x956E40`'s third row); a
    /// standing creature takes it instead of its idle only then (`0x5467A0`,
    /// docs/re/motions.md, "The fight stance").
    /// The attack's clips for `class`, the slots 79 + 5 · class to 4 more where the model has them
    /// (`0x5467A0`, the action 4; class 14 taking 12's; docs/re/motions.md).
    pub fn attacks(&self, class: u8) -> Vec<String> {
        let class = if class == 14 { 12 } else { class.min(12) };
        let first = 79 + 5 * u16::from(class);
        (first..first + 5)
            .filter_map(|slot| self.slot(slot).map(str::to_owned))
            .collect()
    }

    pub fn stance(&self, class: u8) -> Option<&str> {
        let slot = match class {
            0..=12 => 14 + u16::from(class),
            13 => 236,
            14 => 26,
            _ => return None,
        };
        self.0.get(&slot).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The slots of `THFM`'s clips, a thief's: its idle in 1, 14 and 21, its walk in 27, its run
    /// in 40, a rider's idle in 169.
    fn thief() -> Slots {
        Slots::parse(
            br#"{"animations": [
                {"name": "THFM_HORSE_IDLE", "extras": {"slots": [169]}},
                {"name": "THFM_IDLE_BH", "extras": {"slots": [1, 14, 21]}},
                {"name": "THFM_RUN_BH", "extras": {"slots": [40]}},
                {"name": "THFM_WALK_BH", "extras": {"slots": [27]}},
                {"name": "THFM_DYING_A"}
            ]}"#,
        )
        .unwrap()
    }

    #[test]
    fn a_creature_holds_its_weapons_class() {
        // Bare hands; a sword (1) and a kite shield (0); a crossbow (12) alone in the left hand;
        // two blades; two weapons without a class; 14 as 12.
        assert_eq!(class(None, None), 0);
        assert_eq!(class(Some((true, 0)), Some(1)), 1);
        assert_eq!(class(Some((false, 12)), None), 12);
        assert_eq!(class(Some((false, 9)), Some(9)), 8);
        assert_eq!(class(Some((false, 0)), Some(0)), 0);
        assert_eq!(class(None, Some(14)), 12);
    }

    #[test]
    fn a_thief_with_a_dagger_stands_in_its_plain_idle() {
        // The dagger's class 5 has slot 6, which the thief lacks, as it lacks 2.
        let thief = thief();
        assert_eq!(
            thief.clip(Action::Stand, 5),
            Some(("THFM_IDLE_BH", Action::Stand))
        );
        assert_eq!(
            thief.clip(Action::Walk, 5),
            Some(("THFM_WALK_BH", Action::Walk))
        );
        assert_eq!(
            thief.clip(Action::Run, 0),
            Some(("THFM_RUN_BH", Action::Run))
        );
    }

    #[test]
    fn a_creature_falls_back_as_the_client_does() {
        let has = |slots: &'static [u16]| move |s| slots.contains(&s);
        // A sword's idle where the model has it, else the one-handed idle 2, else 1.
        assert_eq!(slot(Action::Stand, 1, has(&[1, 2])), (2, Action::Stand));
        assert_eq!(slot(Action::Stand, 7, has(&[1, 2, 8])), (8, Action::Stand));
        assert_eq!(slot(Action::Stand, 7, has(&[1, 2])), (2, Action::Stand));
        assert_eq!(slot(Action::Stand, 7, has(&[1])), (1, Action::Stand));
        // A bat walks with what is in its walk slot, its idle; a model without walks runs.
        assert_eq!(slot(Action::Walk, 0, has(&[1, 27])), (27, Action::Walk));
        assert_eq!(slot(Action::Walk, 3, has(&[1, 28, 40])), (28, Action::Walk));
        assert_eq!(slot(Action::Walk, 0, has(&[1, 41])), (41, Action::Run));
        assert_eq!(slot(Action::Run, 0, has(&[1, 29])), (29, Action::Walk));
        // Neither walks nor runs: the standing slot.
        assert_eq!(slot(Action::Walk, 0, has(&[2])), (2, Action::Walk));
        // Class 13's own slots, and a class past the table.
        assert_eq!(
            slot(Action::Stand, 13, has(&[1, 235])),
            (235, Action::Stand)
        );
        assert_eq!(slot(Action::Stand, 200, has(&[1])), (1, Action::Stand));
    }

    #[test]
    fn the_fight_stance_is_the_models_own_slot_or_none() {
        // The thief's stance for bare hands is its idle (14); for a dagger (5) slot 19 it lacks;
        // the stance takes no fallback.
        let thief = thief();
        assert_eq!(thief.stance(0), Some("THFM_IDLE_BH"));
        assert_eq!(thief.stance(5), None);
        assert_eq!(thief.stance(7), Some("THFM_IDLE_BH"));
        let fighter = Slots::parse(
            br#"{"animations": [
                {"name": "F_IDLE", "extras": {"slots": [1]}},
                {"name": "F_FIDLE_2H", "extras": {"slots": [26, 236]}}
            ]}"#,
        )
        .unwrap();
        // Class 12 and 14 share 26; class 13 has 236.
        assert_eq!(fighter.stance(12), Some("F_FIDLE_2H"));
        assert_eq!(fighter.stance(14), Some("F_FIDLE_2H"));
        assert_eq!(fighter.stance(13), Some("F_FIDLE_2H"));
        assert_eq!(fighter.stance(0), None);
    }

    #[test]
    fn a_model_without_the_slot_has_no_clip() {
        assert_eq!(Slots::default().clip(Action::Stand, 0), None);
        assert!(Slots::parse(b"{}").unwrap().0.is_empty());
        assert!(Slots::parse(b"not json").is_err());
    }
}
