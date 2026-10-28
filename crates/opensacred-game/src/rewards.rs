//! The scripts' rewards (#1162; docs/re/netscript.md, "The rewards"): the records `AddExp` (17),
//! `AddGold` (18) and `Gewinn` (125) that the scripts ran, carried out on the hero before `npcs`
//! takes the records. `AddExp` adds its points to the hero's experience (`0x48D480`), `AddGold`
//! its amount to the hero's gold, less than 0 too, with a line of the amount and the game's word
//! for gold over the hero, light yellow, or red for a loss (`0x48DA40`); `Gewinn` gives a quest's
//! gold and experience by its flags and an item at most, laid beside the hero (`0x4AC940`,
//! `opensacred_rules::rewards`, #1386). `scripts` keeps them out of the functions of finished
//! quests and reads their variables. A side quest won may give the hero a rune of its class
//! (`0x45FC60`, #1386), and every experience that the hero gains shows a line of the points and
//! the game's word for experience over it, under the lines it shows (`0x57E160`).
//!
//! Not followed yet: `Gewinn`'s sets' pieces by the quests' count (`0x559BF0`), the level of its
//! item's values (taken as the hero's), and a reward to another creature than the hero.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use opensacred_data::netscript::{Body as RecordBody, Record, Value};
use opensacred_rules::rewards::{Prize, class_rune};

use crate::fight::{Experience, Rules};
use crate::game::{AppState, Difficulty};
use crate::hero::Body;
use crate::lines::{Says, Texts};
use crate::npcs::Npcs;
use crate::pickup::Pack;
use crate::scripts::WorldRecords;
use crate::terrain::Level;

/// The records' kinds.
const ADD_EXP: u16 = 17;
const ADD_GOLD: u16 = 18;
const GEWINN: u16 = 125;
/// The operands' tags: a creature's name, a number, and `Gewinn`'s other numbers' tag.
const NAME: u8 = 0x01;
const NUMBER: u8 = 0x0B;
const NUMBER_TOO: u8 = 0x9F;
/// The game's word for gold (the text 1004), and the line's colours for a gain and a loss
/// (`0xFFFFFF80`, `0xFFFF4040`).
const GOLD_WORD: &str = "1004";
const GAIN: Color = Color::srgb(1.0, 1.0, 0.5);
const LOSS: Color = Color::srgb(1.0, 0.25, 0.25);
/// The most gold the hero holds (`0x57FD80`).
const MOST_GOLD: i64 = 0x7FFF_FFFE;
/// The game's word for experience (the text 1071) and its line's colour (`0xFFFFFF20`,
/// `0x57E160`).
const EXPERIENCE_WORD: &str = "1071";
const EXPERIENCE_GAIN: Color = Color::srgb(1.0, 1.0, 32.0 / 255.0);
/// How far before the hero a reward's item is made, in world units (`0x45FD07`).
const LAID_BEFORE: f32 = 40.0;
/// The most of the count of a class's runes given (`0x424690`).
const MOST_GIVEN: u32 = 0xFFFE;

/// A side quest that an `ExitQuest` won (`scripts::Effects::won`), which may give the hero a
/// rune of its class (`0x46D362`, `0x45FC60`).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuestWon(pub u32);

/// The runes of its class that the quests gave the hero (the player's `+0x5710`, `0x424690`),
/// which weigh against the next ([`opensacred_rules::drops::Rules::quest_rune_range`]); kept for
/// the session (**hypothesis**: the original keeps it with the player).
#[derive(Resource, Default)]
struct RunesGiven(u32);

impl RunesGiven {
    fn one_more(&mut self) {
        self.0 = (self.0 + 1).min(MOST_GIVEN);
    }
}

/// What laying a reward's item takes: the drops' tables and rolls, the assets, the hero's arts,
/// and the runes of its class given.
#[derive(SystemParam)]
struct Laying<'w> {
    tables: Option<Res<'w, crate::drops::Tables>>,
    dice: ResMut<'w, crate::drops::Dice>,
    assets: Res<'w, AssetServer>,
    arts: Option<Res<'w, crate::arts::HeroArts>>,
    given: ResMut<'w, RunesGiven>,
}

impl Laying<'_> {
    /// Lays `item` before the hero at `at`, turned `turned`, on `level`, made at the hero's
    /// level `made_at` of the item maker's key `quality`.
    fn lay(
        &mut self,
        commands: &mut Commands,
        (at, turned, level): (Vec3, Quat, Level),
        (item, made_at, quality): (u32, u32, i64),
    ) {
        let Some(tables) = self.tables.as_deref() else {
            return;
        };
        let before = at + turned * Vec3::X * LAID_BEFORE;
        let laid = crate::drops::lay_reward(
            commands,
            (&self.assets, tables, &mut self.dice),
            (before, level),
            (item, made_at, quality),
        );
        if let Some(laid) = laid {
            info!("rewards: laid {laid}");
        }
    }
}

/// A record's numbers of its tags `0x0B` and `0x9F`, in their order in the record, the first
/// three. The scripts' calls hold each tag's numbers together, the `0x0B`s before the `0x9F`s;
/// of the orders that the trees' 5,666 `Gewinn`s take (4,606 `0B 0B 0B`, 1,000 `0B 9F 9F`, 20
/// `9F 9F 9F`, 40 `0B 9F 0B`, counted 2026-10-27 on the listings), only the last differs, a
/// variable's value between two numbers, which is put back there.
fn numbers(record: &Record) -> Vec<i32> {
    let RecordBody::Operands(ops) = &record.body else {
        return Vec::new();
    };
    let of = |tag: u8| -> Vec<i32> {
        ops.iter()
            .filter(|op| op.tag == tag)
            .filter_map(|op| match op.value {
                Value::I32(n) => Some(n),
                Value::U16(n) => Some(i32::from(n)),
                Value::U8(n) => Some(i32::from(n)),
                _ => None,
            })
            .collect()
    };
    let (numbers, too) = (of(NUMBER), of(NUMBER_TOO));
    match (numbers.as_slice(), too.as_slice()) {
        (&[first, last], &[between]) => vec![first, between, last],
        _ => numbers.into_iter().chain(too).take(3).collect(),
    }
}

/// Whether a record names the hero: by `HERO`, case aside, or by no name, as the scripts' own
/// rewards do (**hypothesis**: the function's creature, the hero, for a record without a name).
fn for_hero(record: &Record) -> bool {
    let RecordBody::Operands(ops) = &record.body else {
        return true;
    };
    ops.iter()
        .filter(|op| op.tag == NAME)
        .all(|op| match &op.value {
            Value::Str(name) | Value::Name(name) => name.eq_ignore_ascii_case("hero"),
            _ => true,
        })
}

/// Gives the hero the rewards that the scripts ran this frame, before `npcs` takes the records.
fn reward(
    mut commands: Commands,
    records: Option<Res<WorldRecords>>,
    (mut pack, mut experience, rules, difficulty): (
        ResMut<Pack>,
        ResMut<Experience>,
        Res<Rules>,
        Option<Res<Difficulty>>,
    ),
    npcs: Option<Res<Npcs>>,
    texts: Option<Res<Texts>>,
    mut laying: Laying,
    heroes: Query<(Entity, &Transform, &crate::fight::Base, &Body)>,
) {
    let Some(records) = records else {
        return;
    };
    let Some((hero, at, base, body)) = heroes.iter().next() else {
        return;
    };
    let gold = |commands: &mut Commands, pack: &mut Pack, amount: i64| {
        pack.gold =
            u32::try_from((i64::from(pack.gold) + amount).clamp(0, MOST_GOLD)).unwrap_or(pack.gold);
        let word = texts
            .as_deref()
            .and_then(|texts| texts.named(GOLD_WORD))
            .unwrap_or_default();
        let colour = if amount < 1 { LOSS } else { GAIN };
        Says::add(
            commands,
            hero,
            Says::own(format!("{amount} {word}").trim_end().to_owned(), colour),
        );
    };
    for record in &records.0 {
        if ![ADD_EXP, ADD_GOLD, GEWINN].contains(&record.kind) {
            continue;
        }
        if !for_hero(record) {
            debug!("rewards: a reward to another creature than the hero is not followed");
            continue;
        }
        let numbers = numbers(record);
        let level = experience.0.level;
        match record.kind {
            ADD_EXP => {
                let points = numbers.last().copied().unwrap_or(0);
                if let Ok(points) = u64::try_from(points)
                    && points > 0
                {
                    info!("rewards: the hero gains {points} experience");
                    experience.0.add(points, &rules.0);
                }
            }
            ADD_GOLD => {
                let amount = i64::from(numbers.last().copied().unwrap_or(0));
                if amount != 0 {
                    info!("rewards: the hero's gold changes by {amount}");
                    gold(&mut commands, &mut pack, amount);
                }
            }
            _ => {
                let prize = Prize::of(&numbers);
                if let Some(amount) = prize.gold(level) {
                    info!("rewards: a quest's reward of {amount} gold ({prize:?})");
                    gold(&mut commands, &mut pack, amount);
                }
                let (x, z) = (at.translation.x, at.translation.z);
                let middle = npcs.as_deref().and_then(|npcs| {
                    let (cx, cy) = crate::paths::cell_of(x, z);
                    npcs.middle((u32::try_from(cx).ok()?, u32::try_from(cy).ok()?))
                });
                let difficulty = difficulty.as_deref().map(|d| d.0).unwrap_or_default();
                if let Some(points) =
                    prize.experience(i64::from(middle.unwrap_or(0)), level, difficulty)
                {
                    info!("rewards: a quest's reward of {points} experience ({prize:?})");
                    experience.0.add(points, &rules.0);
                }
                // Its item, one at most, made beside the hero (#1386).
                let Some(tables) = laying.tables.as_deref() else {
                    continue;
                };
                let mut drops_hero = crate::drops::hero(base, level);
                crate::drops::carrying(&mut drops_hero, &pack, laying.arts.as_deref(), tables);
                let quality = prize.quality(difficulty, tables.rules(), &mut laying.dice.0);
                let Some(item) =
                    prize.item(tables.catalogue(), &drops_hero, quality, &mut laying.dice.0)
                else {
                    continue;
                };
                info!(
                    "rewards: a quest's reward of the item {} ({prize:?})",
                    item.item
                );
                if item.class_rune {
                    laying.given.one_more();
                }
                laying.lay(
                    &mut commands,
                    (at.translation, at.rotation, body.level()),
                    (item.item, level, quality),
                );
            }
        }
    }
}

/// A side quest won gives the hero a rune of its class by the chance of its arts and runes over
/// its level (`0x46D362`, `0x45FC60`; `opensacred_rules::drops::Rules::quest_rune_range`), laid
/// before it.
fn quest_runes(
    mut commands: Commands,
    mut won: MessageReader<QuestWon>,
    experience: Res<Experience>,
    pack: Res<Pack>,
    mut laying: Laying,
    heroes: Query<(&Transform, &crate::fight::Base, &Body)>,
) {
    for &QuestWon(quest) in won.read() {
        let (Some(tables), Some((at, base, body))) =
            (laying.tables.as_deref(), heroes.iter().next())
        else {
            continue;
        };
        let mut hero = crate::drops::hero(base, experience.0.level);
        crate::drops::carrying(&mut hero, &pack, laying.arts.as_deref(), tables);
        let range = tables.rules().quest_rune_range(&hero, &mut laying.given.0);
        if !tables.rules().rune_given(range, &mut laying.dice.0) {
            debug!("rewards: quest {quest} won, no rune (of 0 to {range})");
            continue;
        }
        let Some(rune) = class_rune(hero.creature, tables.catalogue(), &mut laying.dice.0) else {
            continue;
        };
        info!("rewards: quest {quest} won, the rune {rune} (of 0 to {range})");
        laying.given.one_more();
        laying.lay(
            &mut commands,
            (at.translation, at.rotation, body.level()),
            (rune, experience.0.level, 0),
        );
    }
}

/// Shows a line of the experience the hero gained over it, under the lines it shows
/// (`0x57E160`: the engine's event 6 of the points and the game's word for experience, at each
/// gain of a player's): the gains of a frame together.
fn experience_line(
    mut commands: Commands,
    experience: Res<Experience>,
    mut last: Local<Option<u64>>,
    texts: Option<Res<Texts>>,
    heroes: Query<Entity, With<Body>>,
) {
    let points = experience.0.points;
    let Some(before) = last.replace(points) else {
        return;
    };
    let (Some(hero), Some(gained)) = (heroes.iter().next(), points.checked_sub(before)) else {
        return;
    };
    if gained == 0 {
        return;
    }
    let word = texts
        .as_deref()
        .and_then(|texts| texts.named(EXPERIENCE_WORD))
        .unwrap_or_default();
    Says::add(
        &mut commands,
        hero,
        Says::own(
            format!("{gained} {word}").trim_end().to_owned(),
            EXPERIENCE_GAIN,
        ),
    );
}

/// The scripts' rewards.
pub struct RewardsPlugin;

impl Plugin for RewardsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RunesGiven>()
            .add_message::<QuestWon>()
            .add_systems(
                Update,
                (reward, quest_runes, experience_line)
                    .chain()
                    .before(crate::npcs::script_records)
                    .run_if(in_state(AppState::InGame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use opensacred_data::netscript::Operand;

    use super::*;

    fn record(kind: u16, ops: Vec<(u8, Value)>) -> Record {
        Record {
            kind,
            body: RecordBody::Operands(
                ops.into_iter()
                    .map(|(tag, value)| Operand { tag, value })
                    .collect(),
            ),
        }
    }

    #[test]
    fn a_reward_reads_its_numbers_and_its_creature() {
        let gewinn = record(
            GEWINN,
            vec![
                (NUMBER, Value::I32(2)),
                (NUMBER, Value::I32(10)),
                (NUMBER, Value::I32(3)),
            ],
        );
        assert_eq!(numbers(&gewinn), [2, 10, 3]);
        assert!(for_hero(&gewinn));
        let named = |name: &str| {
            record(
                ADD_GOLD,
                vec![(NAME, Value::Str(name.into())), (NUMBER, Value::I32(-100))],
            )
        };
        // A variable's value between two numbers, as quest 10259's reward holds it.
        let between = record(
            GEWINN,
            vec![
                (NUMBER, Value::I32(1)),
                (NUMBER, Value::I32(3)),
                (NUMBER_TOO, Value::I32(7)),
            ],
        );
        assert_eq!(numbers(&between), [1, 7, 3]);
        assert!(for_hero(&named("HERO")));
        assert!(!for_hero(&named("someone")));
        assert_eq!(numbers(&named("hero")), [-100]);
    }
}
