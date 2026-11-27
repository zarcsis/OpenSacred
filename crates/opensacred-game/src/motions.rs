//! A creature's clips by the original's motion slots (docs/re/motions.md): a model's file keeps,
//! with each clip, the slots of its `ModelDesc` (docs/02 §4.1) that name it (`extras.slots`,
//! §4.5), and the client plays for an action the slot that a table gives by the class of what
//! the creature holds, or the nearest slot the model has (`0x5467A0`); and the motions' own
//! factors of their clips' speed ([`Factors`]).

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail};
use bevy::gltf::Gltf;
use bevy::prelude::{
    AnimationClip, App, AssetEvent, AssetId, Assets, First, MessageReader, Plugin, Res, ResMut,
    Resource,
};
use serde_json::Value;

use crate::files;

/// The motions' own factors of their clips' speed (`MotionDesc +0x9C`, `floats_62[0]` of
/// `tables/motions.ron`), which every clip's speed takes (`0x413610`,
/// `opensacred_rules::minds::motion_factor`; docs/re/movement.md, "The motions' own factors"):
/// those that apply, by the motion's name, and by the clips of the models loaded that bear it.
#[derive(Resource, Default)]
pub struct Factors {
    /// By the motion's name, upper-cased without its `.GRN`; none before the tables are read.
    by_name: Option<HashMap<String, f32>>,
    by_clip: HashMap<AssetId<AnimationClip>, f32>,
}

impl Factors {
    /// The factors of the motions of `motions` that apply.
    fn of_motions(motions: &[opensacred_data::tables::Motion]) -> HashMap<String, f32> {
        motions
            .iter()
            .filter_map(|motion| {
                let factor = opensacred_rules::minds::motion_factor(motion.floats_62[0]);
                let name = motion.name.to_ascii_uppercase();
                let name = name.strip_suffix(".GRN").unwrap_or(&name).to_owned();
                (factor != 1.0).then_some((name, factor))
            })
            .collect()
    }

    /// The factor of the clip `clip`'s speed, its motion's own where it applies, else 1.
    pub fn of(&self, clip: impl Into<AssetId<AnimationClip>>) -> f32 {
        self.by_clip.get(&clip.into()).copied().unwrap_or(1.0)
    }
}

/// Takes the clips of each model as it loads into [`Factors`] by their motions' names, the
/// tables read at the first.
fn index(
    mut events: MessageReader<AssetEvent<Gltf>>,
    gltfs: Res<Assets<Gltf>>,
    data: Option<Res<crate::game::Data>>,
    mut factors: ResMut<Factors>,
) {
    let loaded: Vec<_> = events
        .read()
        .filter_map(|event| match event {
            AssetEvent::Added { id }
            | AssetEvent::Modified { id }
            | AssetEvent::LoadedWithDependencies { id } => Some(*id),
            _ => None,
        })
        .collect();
    if loaded.is_empty() {
        return;
    }
    let Factors { by_name, by_clip } = &mut *factors;
    let by_name = by_name.get_or_insert_with(|| {
        let motions = data.as_ref().and_then(|data| data.0.tables.motions().ok());
        motions.map_or_else(HashMap::new, Factors::of_motions)
    });
    for gltf in loaded.into_iter().filter_map(|id| gltfs.get(id)) {
        for (name, clip) in &gltf.named_animations {
            if let Some(&factor) = by_name.get(name.as_ref()) {
                by_clip.insert(clip.id(), factor);
            }
        }
    }
}

/// The motions' own factors of their clips' speed.
pub struct MotionsPlugin;

impl Plugin for MotionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Factors>().add_systems(First, index);
    }
}

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

/// What a hero stands in by its own cases where its class's idle is not it (`0x5467A0` from
/// `0x546971`; docs/re/motions.md, "The heroes' own cases").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnIdle {
    /// Its fight stance, whatever lies near.
    Stance,
    /// The clip of this slot.
    Slot(u16),
}

/// The idle of the hero of the type `kind` holding `class` by its own cases (#3891, #3892): the
/// Gladiator (2) holding the class 2 stands in his stance whatever lies near; the Vampiress by
/// day (6) with the class 1 or 2 out of a fight in the slot 1, `VMPD_IDLE_BH`, not her class's.
pub fn own_idle(kind: u32, class: u8) -> Option<OwnIdle> {
    match (kind, class) {
        (2, 2) => Some(OwnIdle::Stance),
        (6, 1 | 2) => Some(OwnIdle::Slot(1)),
        _ => None,
    }
}

/// The fight stance's slot of the hero of the type `kind` holding `class` by its own cases
/// (#3892): the Vampiress by day (6) bare-handed the slot 15, `VMPD_FIDLE_1H`, not 14.
pub fn own_stance(kind: u32, class: u8) -> Option<u16> {
    (kind == 6 && class == 0).then_some(15)
}

/// The slot a creature plays for `action` holding `class`, of a model that has the slots `has`,
/// and the gait it moves at, as `0x5467A0` picks them: the table's slot (`table`) where the
/// model has it; else, standing, slot 2 where it has that, else 1; walking, the first it has of
/// the walks 27, 28 and 29, then of the runs 40, 41 and 42, at a run, else the standing slot;
/// running, the runs first, then the walks, at a walk. Its other cases are followed elsewhere:
/// the fight stance by [`Slots::stance`] (#219), the heroes' own standing cases by [`own_idle`]
/// and [`own_stance`] (#3891, #3892), the attacks by [`Slots::attacks`], a rider's 169, else
/// 166, and a flyer's 179 by `npcs` (#2288, #2250); the small spider's walk, always the slot 27,
/// is not followed.
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

/// The name of `MOTIONS.PAK`'s record 0, whose fractions a slot that a model lacks takes, its
/// entry of the model's slot table 0 (`0x4135D0`; docs/02-data-formats.md §4.1).
pub const LACKING: &str = "INVALID_MOTION";

/// An NPC's attack's clips for the class of what it holds ([`Slots::attacks`]): its class's own,
/// at their own hit fractions, or the clip of the stand-in that its model has in their place,
/// which `MOTIONS.PAK`'s record 0 times (`stand_in`; `0x4135D0`, docs/re/motions.md, "Where the
/// model lacks the slot").
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attacks {
    pub clips: Vec<String>,
    pub stand_in: bool,
}

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

    /// The clip of the pick-up, slot 172 (`MOTION_PICKUP`), which a hero plays as it takes an
    /// item off the ground (`0x53A170`, docs/re/items.md, "The take"; #1219).
    pub fn pickup(&self) -> Option<&str> {
        self.slot(172)
    }

    /// The clip in `slot`, where the model has one.
    pub fn slot(&self, slot: u16) -> Option<&str> {
        self.0.get(&slot).map(String::as_str)
    }

    /// The rider's strike from the saddle with a weapon of `class` (`0x5467A0`'s action 4 on
    /// horseback, #2302): a crossbow's (the classes 12 and 14) 234; a bow's (7 and 13) 195, else
    /// 196; another's 194, else 193; none where the model has none.
    pub fn riding_attack(&self, class: u8) -> Option<&str> {
        let slots: &[u16] = match class {
            12 | 14 => &[234],
            7 | 13 => &[195, 196],
            _ => &[194, 193],
        };
        slots.iter().find_map(|&slot| self.slot(slot))
    }

    /// The attack's own clips for `class`, the slots 79 + 5 · class to 4 more where the model has
    /// them (`0x5467A0`, the action 4; class 13's its own 241 alone, #2820, class 14 taking 12's;
    /// docs/re/motions.md).
    fn own_attacks(&self, class: u8) -> Vec<String> {
        let of = |first: u16, count: u16| {
            (first..first + count)
                .filter_map(|slot| self.slot(slot).map(str::to_owned))
                .collect::<Vec<_>>()
        };
        match class {
            13 => of(Self::attack_slot(13), 1),
            _ => of(Self::attack_slot(class), 5),
        }
    }

    /// An NPC's attack's clips for `class` ([`Attacks`]): its class's own, whose slot the
    /// original names whether its model has it or not (`0x5467A0`'s action 4 for an NPC); where
    /// the model has none of them, the clip of the first of the slots 89, 84 and 79 that it has,
    /// which the models' clip getter gives in the slot's place (`0x4118B0`,
    /// `opensacred_rules::minds::stand_ins`; docs/re/motions.md, "Where the model lacks the
    /// slot", #3314), none where it has none of them either: a grave's skeleton warrior holding
    /// a dagger swings its slot 84's clip.
    pub fn attacks(&self, class: u8) -> Attacks {
        let clips = self.own_attacks(class);
        if !clips.is_empty() {
            return Attacks {
                clips,
                stand_in: false,
            };
        }
        let clips = opensacred_rules::minds::stand_ins(Self::attack_slot(class))
            .iter()
            .find_map(|&slot| self.slot(slot))
            .map(|name| vec![name.to_owned()])
            .unwrap_or_default();
        Attacks {
            clips,
            stand_in: true,
        }
    }

    /// The hero's attack's clips for `class`, the first first (`0x5467A0`'s heroes' case of the
    /// action 4, docs/re/motions.md, "The heroes' own cases"; #3893): the slots that its model
    /// fills one after another from its class's first, at most five (one of the class 13); where
    /// it fills not the first, the stand-in that the models' clip getter gives in its place, as
    /// an NPC's ([`Slots::attacks`]), timed as its is by the motions' record 0 ([`LACKING`],
    /// #3921), none where it has none either. No class 0's.
    pub fn hero_attacks(&self, class: u8) -> Attacks {
        let first = Self::attack_slot(class);
        let most = if class == 13 { 1 } else { 5 };
        let own: Vec<String> = (first..first + most)
            .map_while(|slot| self.slot(slot).map(str::to_owned))
            .collect();
        if own.is_empty() {
            self.attacks(class)
        } else {
            Attacks {
                clips: own,
                stand_in: false,
            }
        }
    }

    /// The attack's first slot for `class` (`0x5467A0`'s action 4, the table `0x956E40`): 79 + 5 ·
    /// class, 241 for class 13, class 14 taking 12's.
    pub fn attack_slot(class: u8) -> u16 {
        match class {
            13 => 241,
            14 => 79 + 5 * 12,
            _ => 79 + 5 * u16::from(class.min(12)),
        }
    }

    /// The defending slot of a model of these slots for `class` (`0x5467A0`'s action 5): 53 +
    /// class, 239 for class 13, class 14 taking 12's; none where the model lacks it, where the
    /// original's falls back on a standing slot and so does not parry (`0x4E4F26`–`0x4E4F4E`,
    /// #2487).
    pub fn defend_slot(&self, class: u8) -> Option<u16> {
        let slot = match class {
            13 => 239,
            14 => 53 + 12,
            _ => 53 + u16::from(class.min(12)),
        };
        self.slot(slot).map(|_| slot)
    }

    /// The fight stance's clip for `class`, only where the model has its slot: 14 to 26 for the
    /// classes 0 to 12 (class 14 taking 12's), 236 for class 13 (`0x956E40`'s row of the action
    /// 3); a standing creature takes it instead of its idle only then (`0x5467A0`,
    /// docs/re/motions.md, "The fight stance").
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

    #[test]
    fn the_gladiator_and_the_vampiress_by_day_stand_by_their_own_cases() {
        // The Gladiator in his stance with the class 2 alone.
        assert_eq!(own_idle(2, 2), Some(OwnIdle::Stance));
        for class in [0, 1, 3, 10] {
            assert_eq!(own_idle(2, class), None, "{class}");
        }
        // The Vampiress by day in the slot 1 with the classes 1 and 2, her stance 15 bare-handed;
        // by night, the type 7, neither.
        for class in [1, 2] {
            assert_eq!(own_idle(6, class), Some(OwnIdle::Slot(1)));
            assert_eq!(own_stance(6, class), None);
        }
        assert_eq!(own_idle(6, 0), None);
        assert_eq!(own_stance(6, 0), Some(15));
        assert_eq!(own_idle(7, 1), None);
        assert_eq!(own_stance(7, 0), None);
        // The others none.
        assert_eq!(own_idle(1, 2), None);
        assert_eq!(own_stance(2, 0), None);
    }

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
    fn a_creature_defends_by_its_weapons_class_where_its_model_has_the_clip() {
        // A thief's two defending clips, both hands' and one hand's (#2487).
        let thief = Slots::of([(53, "THF_DEFEND_BH"), (54, "THF_DEFEND_1H")]);
        assert_eq!(thief.defend_slot(0), Some(53));
        assert_eq!(thief.defend_slot(1), Some(54));
        // A dagger's (5) it lacks: it does not parry.
        assert_eq!(thief.defend_slot(5), None);
        // 13 its own slot, 14 as 12.
        let all = Slots::of([(239, "X_DEFEND_13"), (65, "X_DEFEND_12")]);
        assert_eq!(all.defend_slot(13), Some(239));
        assert_eq!(all.defend_slot(14), Some(65));
        assert_eq!(all.defend_slot(12), Some(65));
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
    fn a_rider_strikes_by_its_weapons_class() {
        // A crossbow's 234; a bow's 195, else 196; another's 194, else 193; none without them.
        let rider = Slots::of([
            (193, "R_ATTACK_B"),
            (194, "R_ATTACK_A"),
            (196, "R_SHOOT_B"),
            (234, "R_CROSSBOW"),
        ]);
        assert_eq!(rider.riding_attack(12), Some("R_CROSSBOW"));
        assert_eq!(rider.riding_attack(14), Some("R_CROSSBOW"));
        assert_eq!(rider.riding_attack(7), Some("R_SHOOT_B"));
        assert_eq!(rider.riding_attack(13), Some("R_SHOOT_B"));
        assert_eq!(rider.riding_attack(1), Some("R_ATTACK_A"));
        let plain = Slots::of([(193, "P_ATTACK_B")]);
        assert_eq!(plain.riding_attack(0), Some("P_ATTACK_B"));
        assert_eq!(plain.riding_attack(12), None);
        assert_eq!(Slots::default().riding_attack(0), None);
    }

    /// A grave's skeleton warrior holds a dagger of class 5, and its model `SKEL_WARRIOR` fills
    /// the slots 79, 84, 85, 99 and 114 of the attacks (#2708, #3314).
    #[test]
    fn a_class_without_strikes_of_its_own_strikes_with_the_first_stand_in() {
        let own = |clips: &[&str]| Attacks {
            clips: clips.iter().map(|&clip| clip.to_owned()).collect(),
            stand_in: false,
        };
        let stand_in = |clips: &[&str]| Attacks {
            clips: clips.iter().map(|&clip| clip.to_owned()).collect(),
            stand_in: true,
        };
        let skeleton = Slots::of([
            (79, "BH_A"),
            (84, "1H_A"),
            (85, "1H_B"),
            (99, "STAB_A"),
            (114, "BOW_A"),
        ]);
        assert_eq!(skeleton.attacks(0), own(&["BH_A"]));
        assert_eq!(skeleton.attacks(1), own(&["1H_A", "1H_B"]));
        // The dagger's 104 lacking: the slot 84's, as the getter gives it, not 89, not 79.
        assert_eq!(skeleton.attacks(5), stand_in(&["1H_A"]));
        // 89 before 84 and 79; 79 last.
        let both = Slots::of([(79, "BH_A"), (84, "1H_A"), (89, "2H_A")]);
        assert_eq!(both.attacks(5), stand_in(&["2H_A"]));
        assert_eq!(Slots::of([(79, "BH_A")]).attacks(5), stand_in(&["BH_A"]));
        assert_eq!(Slots::default().attacks(5), stand_in(&[]));
        // Class 13's attack is its own 241 alone, not class 12's, with no stand-in; 14 takes 12's.
        let gunner = Slots::of([(79, "BH_A"), (139, "XBOW_A"), (140, "XBOW_B"), (241, "GUN")]);
        assert_eq!(gunner.attacks(13), own(&["GUN"]));
        assert_eq!(gunner.attacks(12), own(&["XBOW_A", "XBOW_B"]));
        assert_eq!(gunner.attacks(14), own(&["XBOW_A", "XBOW_B"]));
        assert_eq!(Slots::of([(79, "BH_A")]).attacks(13), stand_in(&[]));
    }

    #[test]
    fn the_heros_attacks_run_from_its_class_first_else_the_getters_stand_in() {
        let hero = Slots::of([
            (79, "BH_A"),
            (80, "BH_B"),
            (84, "1H_A"),
            (86, "1H_C"),
            (89, "2H_A"),
        ]);
        // The slots one after another from the class's first, to the first gap.
        let clips = |class| hero.hero_attacks(class).clips;
        assert_eq!(clips(0), ["BH_A", "BH_B"]);
        assert_eq!(clips(1), ["1H_A"]);
        assert_eq!(clips(2), ["2H_A"]);
        assert!(!hero.hero_attacks(2).stand_in);
        // Its class's first not filled, the stand-in, the first of 89, 84 and 79, not class 0's.
        assert_eq!(clips(5), ["2H_A"]);
        // A stand-in, timed by the motions' record 0 (#3921).
        assert!(hero.hero_attacks(5).stand_in);
        let bare = Slots::of([(79, "BH_A"), (80, "BH_B")]);
        assert_eq!(bare.hero_attacks(5).clips, ["BH_A"]);
        assert!(Slots::default().hero_attacks(5).clips.is_empty());
    }

    #[test]
    fn the_motions_own_factors_that_apply_are_kept_by_name() {
        let motion = |name: &str, factor| opensacred_data::tables::Motion {
            name: name.to_owned(),
            value_06: 0,
            floats_46: [0.0; 4],
            floats_62: [factor, 0.6, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            value_be: 0,
        };
        let factors = Factors::of_motions(&[
            motion("MAGE_ATTACK_BOGEN_A.GRN", 1.4),
            motion("sera_horse_attack_armbrust.grn", 0.8),
            motion("INVALID_MOTION", 0.6),
            motion("MAGE_ATTACK_ARMBRUST_A.GRN", 0.6),
            motion("ELFF_ATTACK_BOGEN_A.GRN", 0.0),
            motion("X_ONE.GRN", 1.0),
        ]);
        assert_eq!(
            factors,
            HashMap::from([
                ("MAGE_ATTACK_BOGEN_A".to_owned(), 1.4),
                ("SERA_HORSE_ATTACK_ARMBRUST".to_owned(), 0.8),
            ])
        );
        // A clip of no motion with a factor plays at its rate.
        assert_eq!(
            Factors::default().of(AssetId::<AnimationClip>::default()),
            1.0
        );
    }

    #[test]
    fn a_model_without_the_slot_has_no_clip() {
        assert_eq!(Slots::default().clip(Action::Stand, 0), None);
        assert!(Slots::parse(b"{}").unwrap().0.is_empty());
        assert!(Slots::parse(b"not json").is_err());
    }
}
