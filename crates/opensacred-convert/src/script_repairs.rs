//! Repairs of the scripts' data (docs/02 §7.4) that `netscript` makes as it writes the trees
//! (#723, docs/decisions/2026-10-25-chest-cell.md). A repair names its trees and its record by
//! the names in the record and the one after it; a tree it names whose start script lacks such a
//! record is left as it is, with a warning.

use opensacred_data::netscript::{Body, Record, Value};

/// `TriggerPatch`, which puts cells under a trigger (docs/re/movement.md, "The chests' cells").
const TRIGGER_PATCH: u16 = 50;
/// `CreateObj`.
const CREATE_OBJ: u16 = 8;

/// A start script's `TriggerPatch` whose last cell (`0x2A`) runs past its record's end, so that the
/// client reads it from whatever follows the record (docs/re/netscript.md, "Truncated operands"),
/// given the cell (`0x04`) of the `CreateObj` after it: the cell of the object that it closes, as
/// the patch before every other object of its kind names.
#[derive(Debug, Clone, Copy)]
pub struct Cell {
    /// What the cell is, for the summary and the warnings.
    pub what: &'static str,
    /// The trees whose start scripts hold the record.
    pub trees: &'static [&'static str],
    /// The trigger that the patch names (`0x01`).
    pub trigger: &'static str,
    /// The name (`0x29`) of the object after it.
    pub object: &'static str,
}

/// The repairs that `netscript` makes.
pub const CELLS: [Cell; 1] = [CHEST];

/// A chest of the Underworld's and the multiplayer's start scripts, the one of their 22 chests of
/// its object (1339) whose patch names no cell; each of the other 21 has one patch, on its own
/// cell. Its cell is closed to every creature, as the other chests' are.
const CHEST: Cell = Cell {
    what: "a chest's cell",
    trees: &[
        "bin/Addon/NetScript",
        "bin/Addon/NetScriptCamp",
        "bin/Addon/TYPE_NPC_DAEMONIN",
        "bin/Addon/TYPE_NPC_DARKELVE",
        "bin/Addon/TYPE_NPC_ELVE",
        "bin/Addon/TYPE_NPC_GLADIATOR",
        "bin/Addon/TYPE_NPC_MAGICIAN",
        "bin/Addon/TYPE_NPC_SERAPHIM",
        "bin/Addon/TYPE_NPC_VAMPIRELADY",
        "bin/Addon/TYPE_NPC_ZWERG",
        "bin/NetScript",
    ],
    trigger: "chestTrigger",
    object: "chest_Pilzwald_14",
};

/// What [`apply`] did to a tree's start script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// No repair names the tree.
    None,
    /// Repaired: the number of cells given.
    Repaired(usize),
    /// A tree that a repair names whose start script lacks its record: left as it is.
    Mismatch(String),
}

/// Applies the repairs of `cells` that name the tree `source` to its start script `start`.
pub fn apply(cells: &[Cell], source: &str, start: &mut [Record]) -> Outcome {
    let mut repaired = 0;
    let mut named = false;
    for cell in cells.iter().filter(|c| c.trees.contains(&source)) {
        named = true;
        let found = (1..start.len()).find_map(|i| {
            let at = object_cell(&start[i], cell.object)?;
            truncated_patch(&start[i - 1], cell.trigger).then_some((i - 1, at))
        });
        let Some((i, at)) = found else {
            return Outcome::Mismatch(format!(
                "{source}'s start script has no patch of {} with a truncated cell before {}, \
                 which its repair gives {}; left as it is",
                cell.trigger, cell.object, cell.what
            ));
        };
        if let Body::Operands(ops) = &mut start[i].body
            && let Some(last) = ops.last_mut()
        {
            last.value = Value::I32x3(at);
            repaired += 1;
        }
    }
    if named {
        Outcome::Repaired(repaired)
    } else {
        Outcome::None
    }
}

/// Whether `record` is a `TriggerPatch` of `trigger` whose last operand is a truncated cell.
fn truncated_patch(record: &Record, trigger: &str) -> bool {
    let Body::Operands(ops) = &record.body else {
        return false;
    };
    record.kind == TRIGGER_PATCH
        && ops
            .first()
            .is_some_and(|o| o.tag == 0x01 && matches!(&o.value, Value::Str(s) if s == trigger))
        && ops
            .last()
            .is_some_and(|o| o.tag == 0x2A && o.value == Value::Truncated)
}

/// The cell of `record` if it is a `CreateObj` of the object `name`.
fn object_cell(record: &Record, name: &str) -> Option<[i32; 3]> {
    let Body::Operands(ops) = &record.body else {
        return None;
    };
    if record.kind != CREATE_OBJ
        || !ops
            .iter()
            .any(|o| o.tag == 0x29 && matches!(&o.value, Value::Str(s) if s == name))
    {
        return None;
    }
    ops.iter().find_map(|o| match (o.tag, &o.value) {
        (0x04, Value::I32x3(at)) => Some(*at),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use opensacred_data::netscript::Operand;

    use super::*;

    fn record(kind: u16, ops: Vec<(u8, Value)>) -> Record {
        Record {
            kind,
            body: Body::Operands(
                ops.into_iter()
                    .map(|(tag, value)| Operand { tag, value })
                    .collect(),
            ),
        }
    }

    fn patch(last: Value) -> Record {
        record(
            TRIGGER_PATCH,
            vec![(0x01, Value::Str("chestTrigger".into())), (0x2A, last)],
        )
    }

    fn chest(name: &str) -> Record {
        record(
            CREATE_OBJ,
            vec![
                (0x02, Value::I32(1339)),
                (0x04, Value::I32x3([1641, 911, 0])),
                (0x29, Value::Str(name.into())),
            ],
        )
    }

    #[test]
    fn the_truncated_cell_takes_the_chests_cell() {
        let mut start = vec![patch(Value::Truncated), chest("chest_Pilzwald_14")];
        assert_eq!(
            apply(&CELLS, "bin/NetScript", &mut start),
            Outcome::Repaired(1)
        );
        assert_eq!(start[0], patch(Value::I32x3([1641, 911, 0])));
        assert_eq!(start[1], chest("chest_Pilzwald_14"));
    }

    #[test]
    fn only_the_named_trees_and_records_are_repaired() {
        let mut start = vec![patch(Value::Truncated), chest("chest_Pilzwald_14")];
        assert_eq!(
            apply(&CELLS, "bin/TYPE_NPC_ELVE", &mut start),
            Outcome::None
        );
        assert_eq!(start[0], patch(Value::Truncated));
        for mut start in [
            vec![patch(Value::Truncated), chest("chest_Pilzwald_13")],
            vec![patch(Value::I32x3([1, 2, 0])), chest("chest_Pilzwald_14")],
            vec![chest("chest_Pilzwald_14"), patch(Value::Truncated)],
        ] {
            let before = start.clone();
            assert!(matches!(
                apply(&CELLS, "bin/NetScript", &mut start),
                Outcome::Mismatch(_)
            ));
            assert_eq!(start, before);
        }
    }
}
