//! A record's call back as the record it was (`opensacred_data::netscript::Record`), for a host
//! whose readers take the records as the converter keeps them: its type by its call's name
//! (`opensacred_data::luau::kind_of`), its operands by their tags' shapes
//! (`opensacred_data::netscript::shape`), a tag that repeats one operand for each of its values,
//! and its markers in their order (`marks`, #993). A variable's reference and a number's
//! expression come as the values the runtime read for them.

use std::collections::BTreeMap;

use opensacred_data::luau::{DIALOG, MARKS, kind_of};
use opensacred_data::netscript::{self, Body, Record, Shape, Value, shape};

use crate::Operand;

/// The record that the call `name` with `operands` (its one table) stands for; none for a call
/// of no record type, or of operands of no table.
pub fn record(name: &str, operands: &[Operand]) -> Option<Record> {
    let kind = kind_of(name)?;
    let tags = match operands {
        [] => &BTreeMap::new(),
        [Operand::Tags(tags)] => tags,
        [Operand::List(list)] if list.is_empty() => &BTreeMap::new(),
        _ => return None,
    };
    if kind == DIALOG {
        let name = match tags.get("name")? {
            Operand::Text(name) => name.clone(),
            _ => return None,
        };
        let values = match tags.get("values")? {
            Operand::List(values) => match values.as_slice() {
                [Operand::Number(a), Operand::Number(b)] => [int(*a)?, int(*b)?],
                _ => return None,
            },
            _ => return None,
        };
        return Some(Record {
            kind,
            body: Body::Dialog { name, values },
        });
    }
    let mut out = Vec::new();
    let mut markers = Vec::new();
    for (key, operand) in tags {
        let Some(tag) = key
            .strip_prefix('_')
            .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        else {
            continue;
        };
        for value in values(shape(tag), operand)? {
            if value == Value::Marker {
                markers.push(tag);
            } else {
                out.push(netscript::Operand { tag, value });
            }
        }
    }
    // The markers in their order where the call lists it, else by their tags.
    let order: Option<Vec<u8>> = match tags.get(MARKS) {
        Some(Operand::List(marks)) => marks
            .iter()
            .map(|mark| match mark {
                Operand::Number(tag) => u8::try_from(int(*tag)?).ok(),
                _ => None,
            })
            .collect(),
        _ => None,
    };
    out.extend(
        order
            .unwrap_or(markers)
            .into_iter()
            .map(|tag| netscript::Operand {
                tag,
                value: Value::Marker,
            }),
    );
    Some(Record {
        kind,
        body: Body::Operands(out),
    })
}

/// The values of a tag of the shape `shape` that a call holds as `operand`: one, or one for each
/// of a tag that repeats, a list of them.
fn values(shape: Shape, operand: &Operand) -> Option<Vec<Value>> {
    if let Some(one) = value(shape, operand) {
        return Some(vec![one]);
    }
    match operand {
        Operand::List(list) => list.iter().map(|each| value(shape, each)).collect(),
        _ => None,
    }
}

/// The value of one operand of the shape `shape`, if `operand` is one.
fn value(shape: Shape, operand: &Operand) -> Option<Value> {
    let numbers = |list: &[Operand]| -> Option<Vec<i32>> {
        list.iter()
            .map(|n| match n {
                Operand::Number(n) => int(*n),
                _ => None,
            })
            .collect()
    };
    Some(match (shape, operand) {
        (Shape::End | Shape::Marker, Operand::Bool(true)) => Value::Marker,
        (Shape::U8, Operand::Number(n)) => Value::U8(u8::try_from(int(*n)?).ok()?),
        (Shape::U16, Operand::Number(n)) => Value::U16(u16::try_from(int(*n)?).ok()?),
        (
            Shape::I32 | Shape::IntOrName | Shape::IntOrVariable | Shape::Variable,
            Operand::Number(n),
        ) => Value::I32(int(*n)?),
        (Shape::IntOrName | Shape::VectorOrName, Operand::Text(name)) => Value::Name(name.clone()),
        (Shape::Str, Operand::Text(text)) => Value::Str(text.clone()),
        (Shape::Flag, Operand::Number(n)) => Value::Flag {
            flag: u8::try_from(int(*n)?).ok()?,
            text: None,
        },
        (_, Operand::List(list)) => match (shape, list.as_slice()) {
            (Shape::Strs, [Operand::Text(a), Operand::Text(b)]) => {
                Value::Strs(a.clone(), b.clone())
            }
            (
                Shape::NumStr | Shape::NumStrExpression,
                [Operand::Number(n), Operand::Text(text)],
            ) => Value::NumStr {
                number: int(*n)?,
                text: text.clone(),
                expression: None,
            },
            (Shape::Flag, [Operand::Number(n), Operand::Text(text)]) => Value::Flag {
                flag: u8::try_from(int(*n)?).ok()?,
                text: Some(text.clone()),
            },
            (Shape::U16x2, _) => {
                let [a, b] = <[i32; 2]>::try_from(numbers(list)?).ok()?;
                Value::U16x2([u16::try_from(a).ok()?, u16::try_from(b).ok()?])
            }
            (Shape::U16x4, _) => {
                let [a, b, c, d] = <[i32; 4]>::try_from(numbers(list)?).ok()?;
                Value::U16x4([
                    u16::try_from(a).ok()?,
                    u16::try_from(b).ok()?,
                    u16::try_from(c).ok()?,
                    u16::try_from(d).ok()?,
                ])
            }
            (Shape::I32x2, _) => Value::I32x2(numbers(list)?.try_into().ok()?),
            (Shape::I32x3 | Shape::VectorOrName, _) => {
                Value::I32x3(numbers(list)?.try_into().ok()?)
            }
            (Shape::I32x4, _) => Value::I32x4(numbers(list)?.try_into().ok()?),
            _ => return None,
        },
        _ => return None,
    })
}

/// A whole number of the scripts, as Luau holds it, if it is one an `i32` holds.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a number of the scripts is whole and within an i32, which the test before it checks"
)]
fn int(n: f64) -> Option<i32> {
    (n.fract() == 0.0 && n >= f64::from(i32::MIN) && n <= f64::from(i32::MAX)).then_some(n as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(n: i32) -> Operand {
        Operand::Number(f64::from(n))
    }

    fn list(items: Vec<Operand>) -> Operand {
        Operand::List(items)
    }

    fn tags(pairs: Vec<(&str, Operand)>) -> Vec<Operand> {
        vec![Operand::Tags(
            pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
        )]
    }

    fn op(tag: u8, value: Value) -> netscript::Operand {
        netscript::Operand { tag, value }
    }

    #[test]
    fn a_call_is_its_record_again() {
        // The start's captain: its type and items repeating `02`, its markers in their order.
        let captain = record(
            "create_npc",
            &tags(vec![
                ("_02", list(vec![number(297), number(1729), number(1200)])),
                ("_03", number(45)),
                ("_04", Operand::Text("hauptmann3".into())),
                ("_0E", Operand::Bool(true)),
                ("_13", Operand::Bool(true)),
                ("_46", Operand::Bool(true)),
                (
                    "marks",
                    list(vec![number(0x0E), number(0x46), number(0x13)]),
                ),
            ]),
        )
        .unwrap();
        assert_eq!(captain.kind, 1);
        assert_eq!(
            captain.body,
            Body::Operands(vec![
                op(0x02, Value::I32(297)),
                op(0x02, Value::I32(1729)),
                op(0x02, Value::I32(1200)),
                op(0x03, Value::U16(45)),
                op(0x04, Value::Name("hauptmann3".into())),
                op(0x0E, Value::Marker),
                op(0x46, Value::Marker),
                op(0x13, Value::Marker),
            ])
        );
        // `0B` repeating its numbers, `19` its triples, `35` one triple.
        let fill = record(
            "fill_sector",
            &tags(vec![
                (
                    "_19",
                    list(vec![
                        list(vec![number(516), number(60), number(0)]),
                        list(vec![number(530), number(10), number(0)]),
                    ]),
                ),
                ("_35", list(vec![number(6), number(6), number(4096)])),
            ]),
        )
        .unwrap();
        assert_eq!(
            fill.body,
            Body::Operands(vec![
                op(0x19, Value::I32x3([516, 60, 0])),
                op(0x19, Value::I32x3([530, 10, 0])),
                op(0x35, Value::I32x3([6, 6, 4096])),
            ])
        );
        let values = record(
            "spawn_values",
            &tags(vec![(
                "_0B",
                list(vec![number(50), number(15), number(30)]),
            )]),
        )
        .unwrap();
        assert_eq!(
            values.body,
            Body::Operands(vec![
                op(0x0B, Value::I32(50)),
                op(0x0B, Value::I32(15)),
                op(0x0B, Value::I32(30)),
            ])
        );
        // A place by its name or by its cell.
        let at = record(
            "create_obj",
            &tags(vec![(
                "_04",
                list(vec![number(684), number(3418), number(0)]),
            )]),
        )
        .unwrap();
        assert_eq!(
            at.body,
            Body::Operands(vec![op(0x04, Value::I32x3([684, 3418, 0]))])
        );
        // A dialogues' entry.
        let entry = record(
            "dialog_entry",
            &tags(vec![
                ("name", Operand::Text("DQ23_ESCORT_NPC".into())),
                ("values", list(vec![number(11439), number(10)])),
            ]),
        )
        .unwrap();
        assert_eq!(
            entry,
            Record {
                kind: 40,
                body: Body::Dialog {
                    name: "DQ23_ESCORT_NPC".into(),
                    values: [11439, 10]
                }
            }
        );
        assert_eq!(record("no_such_record", &tags(vec![])), None);
        assert_eq!(int(2.5), None);
    }

    /// A record's operands in one order: the valued by their tags, the markers in theirs.
    fn sorted(record: &Record) -> Record {
        let Body::Operands(operands) = &record.body else {
            return record.clone();
        };
        let (mut valued, markers): (Vec<_>, Vec<_>) = operands
            .iter()
            .cloned()
            .partition(|o| o.value != Value::Marker);
        valued.sort_by_key(|o| o.tag);
        valued.extend(markers);
        Record {
            kind: record.kind,
            body: Body::Operands(valued),
        }
    }

    #[test]
    #[ignore = "needs the game files in OriginalGame/"]
    fn every_sectors_record_comes_back_from_its_call() {
        use crate::{Handled, Host, Runtime};
        // The converter's masters of the Seraphim's tree (`opensacred-convert netscript`): its
        // records and its Luau, whose sectors' functions hold no condition.
        let masters =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets-src/netscript");
        let tree = netscript::Tree::decode(
            &std::fs::read(masters.join("type_npc_seraphim.nsc")).expect("the tree is converted"),
        )
        .expect("the tree decodes");
        let sources = masters.join("type_npc_seraphim");
        let index: opensacred_data::luau::Index = ron::from_str(
            &std::fs::read_to_string(sources.join(opensacred_data::luau::INDEX))
                .expect("the index is converted"),
        )
        .expect("the index reads");
        let runtime = Runtime::new("TYPE_NPC_SERAPHIM", index, move |file| {
            let source = std::fs::read_to_string(sources.join(file)).ok()?;
            mlua::chunk::Compiler::new().compile(source).ok()
        })
        .expect("the runtime starts");
        /// A host that keeps every record's call back as its record.
        struct Back(Vec<Record>);
        impl Host for Back {
            fn record(&mut self, name: &str, operands: &[Operand], _: &[bool]) -> Handled {
                self.0
                    .push(record(name, operands).unwrap_or_else(|| panic!("{name}: {operands:?}")));
                Handled::Done
            }
            fn var(&mut self, _: &str) -> i32 {
                0
            }
        }
        let mut functions = 0;
        let mut records = 0;
        for function in tree
            .functions
            .iter()
            .filter(|f| f.name.starts_with("Sector") && !f.name.contains("Region"))
        {
            let mut back = Back(Vec::new());
            if runtime.call(&function.name, &mut back).is_err() {
                continue;
            }
            let want: Vec<Record> = function.records.iter().map(sorted).collect();
            let got: Vec<Record> = back.0.iter().map(sorted).collect();
            assert_eq!(got, want, "{}", function.name);
            functions += 1;
            records += got.len();
        }
        assert!(
            // 11,414 functions of 50,358 records (2026-10-25).
            functions > 11_000 && records > 50_000,
            "{functions} {records}"
        );
    }
}
