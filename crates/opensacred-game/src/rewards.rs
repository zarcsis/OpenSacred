//! The scripts' rewards (#1162; docs/re/netscript.md, "The rewards"): the records `AddExp` (17),
//! `AddGold` (18) and `Gewinn` (125) that the scripts ran, carried out on the hero before `npcs`
//! takes the records. `AddExp` adds its points to the hero's experience (`0x48D480`), `AddGold`
//! its amount to the hero's gold, less than 0 too, with a line of the amount and the game's word
//! for gold over the hero, light yellow, or red for a loss (`0x48DA40`); `Gewinn` gives a quest's
//! gold and experience by its flags (`0x4AC940`, `opensacred_rules::rewards`). `scripts` keeps
//! them out of the functions of finished quests and reads their variables.
//!
//! Not followed yet: `Gewinn`'s items and its sets' pieces, and a reward to another creature
//! than the hero.

use bevy::prelude::*;
use opensacred_data::netscript::{Body as RecordBody, Record, Value};
use opensacred_rules::rewards::Prize;

use crate::fight::{Experience, Rules};
use crate::game::{AppState, Difficulty};
use crate::hero::Body;
use crate::lines::{Says, Texts};
use crate::npcs::Npcs;
use crate::pickup::Pack;
use crate::scripts::WorldRecords;

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
    heroes: Query<(Entity, &Transform), With<Body>>,
) {
    let Some(records) = records else {
        return;
    };
    let Some((hero, at)) = heroes.iter().next() else {
        return;
    };
    let mut gold = |pack: &mut Pack, amount: i64| {
        pack.gold =
            u32::try_from((i64::from(pack.gold) + amount).clamp(0, MOST_GOLD)).unwrap_or(pack.gold);
        let word = texts
            .as_deref()
            .and_then(|texts| texts.named(GOLD_WORD))
            .unwrap_or_default();
        let colour = if amount < 1 { LOSS } else { GAIN };
        commands.entity(hero).insert(Says::own(
            format!("{amount} {word}").trim_end().to_owned(),
            colour,
        ));
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
                    gold(&mut pack, amount);
                }
            }
            _ => {
                let prize = Prize::of(&numbers);
                if let Some(amount) = prize.gold(level) {
                    info!("rewards: a quest's reward of {amount} gold ({prize:?})");
                    gold(&mut pack, amount);
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
            }
        }
    }
}

/// The scripts' rewards.
pub struct RewardsPlugin;

impl Plugin for RewardsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            reward
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
