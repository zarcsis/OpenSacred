use super::*;

/// A record of `kind` with `body`.
fn record(kind: u16, body: &[u8]) -> Vec<u8> {
    let mut data = kind.to_le_bytes().to_vec();
    data.extend_from_slice(&u16::try_from(body.len() + 4).unwrap().to_le_bytes());
    data.extend_from_slice(body);
    data
}

fn operands_of(body: &[u8]) -> Vec<Operand> {
    let script = Script::parse(&record(1, body)).unwrap();
    match &script.records[0].body {
        Body::Operands(operands) => operands.clone(),
        other => panic!("{other:?}"),
    }
}

fn op(tag: u8, value: Value) -> Operand {
    Operand { tag, value }
}

fn s(text: &str) -> String {
    text.to_owned()
}

#[test]
fn operands_take_the_shape_their_tag_selects() {
    let mut body = b"\x01res:1\0".to_vec();
    body.extend_from_slice(&[0x02, 0x63, 0x01, 0, 0]);
    body.extend_from_slice(&[0x04, 0xFE, 0xFF, 0xFF, 0xFF]);
    body.extend_from_slice(b"pos_A\0\x0e");
    body.extend_from_slice(&[0x1F, 0x9F, 0xEF, 0xBE, 0xED, 0xFE]);
    body.extend_from_slice(b"dq_num\0");
    body.extend_from_slice(&[0x1F, 0xE8, 0x03, 0, 0]);
    body.extend_from_slice(&[0x0C, 1, 0, 0, 0, 2, 0, 0, 0, 0xFD, 0xFF, 0xFF, 0xFF]);
    body.extend_from_slice(&[0x15, 1, 0, 2, 0, 3, 0, 4, 0, 0x28, 7]);
    body.extend_from_slice(b"\x92a\0b\0\x37\x01res:2\0\x37\x00");
    assert_eq!(
        operands_of(&body),
        [
            op(0x01, Value::Str(s("res:1"))),
            op(0x02, Value::I32(0x163)),
            op(0x04, Value::Name(s("pos_A"))),
            op(0x0E, Value::Marker),
            op(0x1F, Value::Variable(s("dq_num"))),
            op(0x1F, Value::I32(1000)),
            op(0x0C, Value::I32x3([1, 2, -3])),
            op(0x15, Value::U16x4([1, 2, 3, 4])),
            op(0x28, Value::U8(7)),
            op(0x92, Value::Strs(s("a"), s("b"))),
            op(
                0x37,
                Value::Flag {
                    flag: 1,
                    text: Some(s("res:2"))
                }
            ),
            op(
                0x37,
                Value::Flag {
                    flag: 0,
                    text: None
                }
            ),
        ]
    );
}

#[test]
fn names_replace_numbers_after_their_sentinels() {
    let mut body = vec![0x0B, 0x79, 0x29, 0xED, 0xFF];
    body.extend_from_slice(b"DQ_NUM\0");
    body.extend_from_slice(&[0x0B, 0xFF, 0xFF, 0xFF, 0xFF]);
    body.extend_from_slice(&[0x9F, 0xEF, 0xBE, 0xED, 0xFE]);
    body.extend_from_slice(b"dq_level\0");
    assert_eq!(
        operands_of(&body),
        [
            op(0x0B, Value::Name(s("DQ_NUM"))),
            op(0x0B, Value::I32(-1)),
            op(0x9F, Value::Variable(s("dq_level"))),
        ]
    );
}

/// An expression follows the string of tag 0x48 when the number is negative; tag 0x7A has no
/// expression, so the next byte is a tag.
#[test]
fn only_negative_numbers_of_seven_tags_take_an_expression() {
    let numstr = |tag: u8, number: i32, rest: &[u8]| {
        let mut body = vec![tag];
        body.extend_from_slice(&number.to_le_bytes());
        body.extend_from_slice(b"QUEST\0");
        body.extend_from_slice(rest);
        operands_of(&body)
    };
    let value = |number, expression: Option<&str>| Value::NumStr {
        number,
        text: s("QUEST"),
        expression: expression.map(s),
    };
    assert_eq!(
        numstr(0x48, -1, b"A+VAR(B)\0"),
        [op(0x48, value(-1, Some("A+VAR(B)")))]
    );
    assert_eq!(
        numstr(0x48, 5, b"\x01x\0"),
        [op(0x48, value(5, None)), op(0x01, Value::Str(s("x")))]
    );
    assert_eq!(
        numstr(0x7A, -1, b"\x01x\0"),
        [op(0x7A, value(-1, None)), op(0x01, Value::Str(s("x")))]
    );
}

#[test]
fn a_payload_past_the_end_is_truncated() {
    assert_eq!(
        operands_of(b"\x01chestTrigger\0\x2a"),
        [
            op(0x01, Value::Str(s("chestTrigger"))),
            op(0x2A, Value::Truncated)
        ]
    );
    assert_eq!(operands_of(b"\x01no end"), [op(0x01, Value::Truncated)]);
}

#[test]
fn dialog_entries_and_blocks_are_not_operands() {
    let mut entry = vec![0xFF; 4];
    let mut name = b"dlg_A".to_vec();
    name.resize(64, 0);
    entry.extend_from_slice(&name);
    for value in [3i32, -1, 0] {
        entry.extend_from_slice(&value.to_le_bytes());
    }
    let mut data = record(DIALOG, &entry);
    data.extend(record(BLOCK, &[2, 0]));
    data.extend(record(42, &[]));
    let script = Script::parse(&data).unwrap();
    let bodies: Vec<_> = script.records.iter().map(|r| (r.offset, &r.body)).collect();
    assert_eq!(
        bodies,
        [
            (
                0,
                &Body::Dialog(Dialog {
                    name: s("dlg_A"),
                    values: [3, -1]
                })
            ),
            (84, &Body::Block(2)),
            (90, &Body::Operands(vec![])),
        ]
    );
}

#[test]
fn malformed_records_are_errors() {
    let error = |data: &[u8]| Script::parse(data).unwrap_err().to_string();
    assert!(error(&record(141, &[])).contains("`record type` is 141"));
    assert!(error(&[1, 0, 3, 0]).contains("`record length` is 3"));
    assert!(error(&[1, 0, 9, 0, 0x01]).contains("record body"));
    assert!(error(&record(1, &[0x0E, 0x17])).contains("offset 0x5: field `operand tag` is 23"));
    assert!(error(&record(1, &[0x9F, 0, 0, 0, 0])).contains("`variable reference`"));
    assert!(error(&record(BLOCK, &[6, 0])).contains("`block` is 6"));
    assert!(error(&record(1, b"\x3aHERO\0\0")).contains("`second string length`"));
    assert!(error(&record(DIALOG, &[0xFF; 80])).contains("`dialog entry`"));
}

#[test]
fn record_types_are_named_by_the_first_keyword() {
    assert_eq!(name(1), Some("CreateNPC"));
    assert_eq!(name(55), Some("DelNPC"));
    assert_eq!(name(115), Some("MakeGroup"));
    assert_eq!(name(40), None);
    let types: std::collections::BTreeSet<u16> = KEYWORDS.iter().map(|&(_, k)| k).collect();
    assert_eq!(types.len(), 135);
}
