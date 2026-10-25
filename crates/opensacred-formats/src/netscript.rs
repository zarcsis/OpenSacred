//! NetScript bytecode, `bin/<tree>/{FunkCode,StartCode,QuestCode}.bin` (docs/02 §7; the reader
//! of the original in docs/re/netscript.md): records `{u16 type; u16 length}`, the length counting
//! the 4 header bytes, back to back to the end of the file. The body of a record is a stream of
//! operands, each a tag byte and the payload the tag selects; type 40 holds a struct instead and
//! type 41 a u16. Confirmed on every record of the 20 trees by
//! `tools/analysis/scripts/netscript_dump.py`.

use crate::error::{Result, field};
use crate::reader::Reader;
use opensacred_data::netscript::{Shape, VARIABLE_TAG, shape};

mod vectoren;

pub use vectoren::{Function, Quest, Region, RegionEntry, Vectoren};

/// Record types: the size of the walker's jump table (`FUN_00475680`).
pub const RECORD_TYPES: u16 = 141;
/// A dialog table entry: an 80-byte struct instead of operands.
pub const DIALOG: u16 = 40;
/// The block selector: a u16 from 1 to 5 instead of operands.
pub const BLOCK: u16 = 41;

const DIALOG_SIZE: usize = 80;
const DIALOG_NAME: usize = 64;
/// In place of the i32 × 3 of tags 0x04, 0x0C, 0x0D and 0x3C: a name follows.
const NAME_SENTINEL: u32 = 0xFFFF_FFFE;
/// In place of the i32 of tags 0x0B, 0x38 and 0x5F: a name follows.
const INT_NAME_SENTINEL: u32 = 0xFFED_2979;
/// A variable reference: [`VARIABLE_TAG`], this u32 and the variable's name (`FUN_00453970`).
const VARIABLE_MAGIC: u32 = 0xFEED_BEEF;

pub use opensacred_data::netscript::{KEYWORDS, name};

/// A script file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    pub records: Vec<Record>,
    /// Bytes of the file.
    pub size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// Offset of the record header in the file.
    pub offset: usize,
    /// Type, below [`RECORD_TYPES`].
    pub kind: u16,
    pub body: Body,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    Operands(Vec<Operand>),
    /// Type 40.
    Dialog(Dialog),
    /// Type 41.
    Block(u16),
}

/// The body of type 40: `{i32 -1; char name[64]; i32 a; i32 b; i32 0}`, the name padded with
/// zeros. The walker copies it into a table of 80-byte entries; the meaning of `a` and `b` is
/// not known (`a` 2,502 values, `b` 22).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dialog {
    pub name: String,
    pub values: [i32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operand {
    pub tag: u8,
    pub value: Value,
}

/// The payload of an operand, by the class of its tag. Strings are Latin-1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// No payload: 66 tags of the default handler, and 0x71.
    Marker,
    U8(u8),
    U16(u16),
    U16x2([u16; 2]),
    U16x4([u16; 4]),
    I32(i32),
    I32x2([i32; 2]),
    I32x3([i32; 3]),
    I32x4([i32; 4]),
    Str(String),
    /// Two strings back to back (tags 0x3A, 0x92).
    Strs(String, String),
    /// A number and a string (tags 0x6F, 0x7A); tags 0x48, 0x49, 0x4A, 0x5D, 0x5E, 0x6D and 0x6E
    /// add an expression whose value replaces the number when the number is negative and the
    /// byte after the string is not 0.
    NumStr {
        number: i32,
        text: String,
        expression: Option<String>,
    },
    /// A name in place of the numbers of tags 0x04, 0x0C, 0x0D, 0x3C (a position) and 0x0B, 0x38,
    /// 0x5F (a variable).
    Name(String),
    /// A variable reference in place of the number of tag 0x1F, or the operand of tag 0x9F.
    Variable(String),
    /// A byte, and a string when it is 1 (tags 0x37, 0x4B).
    Flag {
        flag: u8,
        text: Option<String>,
    },
    /// The payload runs past the end of the record; the original reads on into the bytes that
    /// follow the record in its copy buffer. Only 11 records of type 50 end so, with tag 0x2A.
    Truncated,
}

/// Why an operand stops the record.
enum Stop {
    /// The payload runs past the end of the record.
    Past,
    /// Tag 0x9F without the variable reference.
    NoVariable,
}

use Stop::Past;

/// Cursor over a record body.
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], Stop> {
        let end = self.pos.checked_add(len).ok_or(Past)?;
        let bytes = self.bytes.get(self.pos..end).ok_or(Past)?;
        self.pos = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Stop> {
        let bytes = self.take(N)?;
        bytes.try_into().map_err(|_| Past)
    }

    fn u8(&mut self) -> Result<u8, Stop> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, Stop> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, Stop> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn i32(&mut self) -> Result<i32, Stop> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    fn peek_u32(&self) -> Option<u32> {
        let bytes = self.bytes.get(self.pos..self.pos.checked_add(4)?)?;
        Some(u32::from_le_bytes(bytes.try_into().ok()?))
    }

    /// A NUL-terminated string.
    fn string(&mut self) -> Result<String, Stop> {
        let rest = self.bytes.get(self.pos..).ok_or(Past)?;
        let len = rest.iter().position(|&b| b == 0).ok_or(Past)?;
        self.pos += len + 1;
        Ok(latin1(&rest[..len]))
    }

    /// A variable reference `9F EF BE ED FE name\0` at the cursor.
    fn variable(&mut self) -> Result<Option<String>, Stop> {
        let magic = self
            .bytes
            .get(self.pos + 1..self.pos + 5)
            .map(|b| b == VARIABLE_MAGIC.to_le_bytes());
        if self.bytes.get(self.pos) != Some(&VARIABLE_TAG) || magic != Some(true) {
            return Ok(None);
        }
        self.pos += 5;
        self.string().map(Some)
    }
}

/// Latin-1 bytes as a string: each byte is the code point.
fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

/// One operand after its tag.
fn value(tag: u8, body: &mut Cursor<'_>) -> Result<Value, Stop> {
    let i32s = |body: &mut Cursor<'_>| -> Result<[i32; 3], Stop> {
        Ok([body.i32()?, body.i32()?, body.i32()?])
    };
    Ok(match shape(tag) {
        Shape::End | Shape::Marker => Value::Marker,
        Shape::U8 => Value::U8(body.u8()?),
        Shape::U16 => Value::U16(body.u16()?),
        Shape::U16x2 => Value::U16x2([body.u16()?, body.u16()?]),
        Shape::U16x4 => Value::U16x4([body.u16()?, body.u16()?, body.u16()?, body.u16()?]),
        Shape::I32 => Value::I32(body.i32()?),
        Shape::I32x2 => Value::I32x2([body.i32()?, body.i32()?]),
        Shape::I32x3 => Value::I32x3(i32s(body)?),
        Shape::I32x4 => Value::I32x4([body.i32()?, body.i32()?, body.i32()?, body.i32()?]),
        Shape::Str => Value::Str(body.string()?),
        Shape::Strs => Value::Strs(body.string()?, body.string()?),
        Shape::NumStr | Shape::NumStrExpression => {
            let number = body.i32()?;
            let text = body.string()?;
            let expression = if shape(tag) == Shape::NumStrExpression
                && number < 0
                && body.bytes.get(body.pos).is_some_and(|&b| b != 0)
            {
                Some(body.string()?)
            } else {
                None
            };
            Value::NumStr {
                number,
                text,
                expression,
            }
        }
        Shape::VectorOrName => {
            if body.peek_u32() == Some(NAME_SENTINEL) {
                body.u32()?;
                Value::Name(body.string()?)
            } else {
                Value::I32x3(i32s(body)?)
            }
        }
        Shape::IntOrName => {
            let number = body.u32()?;
            if number == INT_NAME_SENTINEL {
                Value::Name(body.string()?)
            } else {
                Value::I32(i32::from_le_bytes(number.to_le_bytes()))
            }
        }
        Shape::Flag => {
            let flag = body.u8()?;
            let text = if flag == 1 {
                Some(body.string()?)
            } else {
                None
            };
            Value::Flag { flag, text }
        }
        Shape::IntOrVariable => match body.variable()? {
            Some(name) => Value::Variable(name),
            None => Value::I32(body.i32()?),
        },
        Shape::Variable => {
            // The handler reads the reference from the tag itself.
            body.pos -= 1;
            match body.variable()? {
                Some(name) => Value::Variable(name),
                None => return Err(Stop::NoVariable),
            }
        }
    })
}

/// The operands of a record body at `offset` in the file.
fn operands(file: &Reader<'_>, offset: usize, bytes: &[u8]) -> Result<Vec<Operand>> {
    let mut body = Cursor { bytes, pos: 0 };
    let mut out = Vec::new();
    while let Some(&tag) = bytes.get(body.pos) {
        let at = offset + 4 + body.pos;
        body.pos += 1;
        if shape(tag) == Shape::End {
            return Err(file.error_at(
                at,
                field(
                    "operand tag",
                    tag.into(),
                    "not one that ends the record".into(),
                ),
            ));
        }
        match value(tag, &mut body) {
            Ok(Value::Strs(_, second)) if second.is_empty() => {
                // The 0x3A handler measures the second string from its second byte.
                return Err(
                    file.error_at(at, field("second string length", 0, "at least 1".into()))
                );
            }
            Ok(value) => out.push(Operand { tag, value }),
            Err(Stop::NoVariable) => {
                return Err(file.error_at(
                    at,
                    field(
                        "variable reference",
                        tag.into(),
                        "tag 0x9f followed by 0xfeedbeef".into(),
                    ),
                ));
            }
            Err(Past) => {
                out.push(Operand {
                    tag,
                    value: Value::Truncated,
                });
                break;
            }
        }
    }
    Ok(out)
}

impl Script {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let file = Reader::new(data);
        let mut r = file.clone();
        let mut records = Vec::new();
        while !r.rest().is_empty() {
            let offset = r.offset();
            let kind = r.u16()?;
            let length = r.u16()?;
            if length < 4 {
                return Err(file.error_at(
                    offset + 2,
                    field("record length", length.into(), "at least 4".into()),
                ));
            }
            if kind >= RECORD_TYPES {
                return Err(file.error_at(
                    offset,
                    field("record type", kind.into(), "below 141".into()),
                ));
            }
            let mut body = r.take(usize::from(length) - 4, "record body")?;
            let bytes = body.rest();
            let body = match kind {
                DIALOG => Body::Dialog(dialog(&mut body)?),
                BLOCK => {
                    let block = body.u16()?;
                    if !(1..=5).contains(&block) || !body.rest().is_empty() {
                        return Err(file.error_at(
                            offset + 4,
                            field("block", block.into(), "a u16 from 1 to 5".into()),
                        ));
                    }
                    Body::Block(block)
                }
                _ => Body::Operands(operands(&file, offset, bytes)?),
            };
            records.push(Record { offset, kind, body });
        }
        Ok(Self {
            records,
            size: data.len(),
        })
    }
}

fn dialog(body: &mut Reader<'_>) -> Result<Dialog> {
    let start = body.offset();
    let size = body.rest().len();
    if size != DIALOG_SIZE {
        return Err(body.error(field(
            "dialog entry size",
            u64::try_from(size).unwrap_or(u64::MAX),
            "80".into(),
        )));
    }
    let head = body.u32()?;
    let name = body.bytes(DIALOG_NAME, "dialog name")?;
    let len = name.iter().position(|&b| b == 0).unwrap_or(DIALOG_NAME);
    let values = [body.u32()?, body.u32()?].map(|v| i32::from_le_bytes(v.to_le_bytes()));
    let tail = body.u32()?;
    if head != u32::MAX || tail != 0 || name[len..].iter().any(|&b| b != 0) {
        return Err(body.error_at(
            start,
            field(
                "dialog entry",
                head.into(),
                "-1, a name padded with zeros, two i32 and 0".into(),
            ),
        ));
    }
    Ok(Dialog {
        name: latin1(&name[..len]),
        values,
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod game_tests;
