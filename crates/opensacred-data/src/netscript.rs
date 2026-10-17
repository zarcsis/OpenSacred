//! The converted scripts (docs/02 §7): the NetScript bytecode of a tree in an open encoding.
//!
//! `netscript.ron` holds the [`NetScript`] index of the 20 trees; each tree is a [`Tree`] file,
//! postcard-encoded in a zlib stream ([`Tree::encode`]). The records keep the original record
//! types and operand tags: their meaning is recovered type by type (docs/re/netscript.md), and the
//! interpreter matches on them. `FunkCode.bin` is split into its named function blocks by
//! `Vectoren.bin`.

use serde::{Deserialize, Serialize};

use crate::packed::{DecodeError, decode, encode};

/// Version of the tree encoding; a file of another version does not load.
pub const VERSION: u32 = 1;
/// The index file under the script directory.
pub const INDEX: &str = "netscript.ron";

/// The index, `netscript.ron`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetScript {
    pub trees: Vec<TreeEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeEntry {
    /// The directory in the installation, such as `bin/Addon/TYPE_NPC_ELVE`.
    pub source: String,
    /// The tree file, relative to `netscript.ron`.
    pub file: String,
    pub records: usize,
    pub functions: usize,
}

/// The scripts of a tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tree {
    /// `StartCode.bin`: the records run when the world starts.
    pub start: Vec<Record>,
    /// `QuestCode.bin`: the starting quest flags; empty in the Underworld trees.
    pub quest: Vec<Record>,
    /// `FunkCode.bin`, by its function blocks.
    pub functions: Vec<Function>,
    pub quests: Vec<Quest>,
    pub regions: Vec<Region>,
}

/// A function block of `FunkCode.bin`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    /// The id of the quest it belongs to, or −1.
    pub quest: i32,
    /// 0, or the region of a dynamic quest in the Sacred campaign trees (hypothesis).
    pub region: i32,
    pub records: Vec<Record>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// The original record type, below 141.
    pub kind: u16,
    pub body: Body,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Body {
    Operands(Vec<Operand>),
    /// Type 40: a dialog table entry.
    Dialog {
        name: String,
        values: [i32; 2],
    },
    /// Type 41: a block selector, 1 to 5.
    Block(u16),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operand {
    /// The original operand tag.
    pub tag: u8,
    pub value: Value,
}

/// The payload of an operand (`opensacred_formats::netscript::Value`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
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
    Strs(String, String),
    NumStr {
        number: i32,
        text: String,
        expression: Option<String>,
    },
    Name(String),
    Variable(String),
    Flag {
        flag: u8,
        text: Option<String>,
    },
    /// The original reads the payload from past the end of the record.
    Truncated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quest {
    pub id: u32,
    pub name: String,
    pub values: [i32; 8],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    pub id: u32,
    pub name: String,
    pub values: [i32; 5],
    pub lists: [Vec<u32>; 3],
    pub entries: Vec<RegionEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionEntry {
    pub list: Vec<u32>,
    pub values: [i32; 4],
    pub tail: [u8; 8],
}

impl Tree {
    /// The tree file: the version and the tree, postcard-encoded, in a zlib stream.
    pub fn encode(&self) -> Result<Vec<u8>, postcard::Error> {
        encode(VERSION, self)
    }

    /// The tree of a file that [`Tree::encode`] wrote.
    pub fn decode(file: &[u8]) -> Result<Self, DecodeError> {
        decode(VERSION, file)
    }

    /// Records in the tree.
    pub fn records(&self) -> usize {
        self.start.len()
            + self.quest.len()
            + self
                .functions
                .iter()
                .map(|f| f.records.len())
                .sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trees_round_trip() {
        let record = |kind, body| Record { kind, body };
        let tree = Tree {
            start: vec![record(
                1,
                Body::Operands(vec![
                    Operand {
                        tag: 0x01,
                        value: Value::Str("res:1".into()),
                    },
                    Operand {
                        tag: 0x48,
                        value: Value::NumStr {
                            number: -1,
                            text: "Q".into(),
                            expression: Some("A+VAR(B)".into()),
                        },
                    },
                ]),
            )],
            quest: vec![record(41, Body::Block(3))],
            functions: vec![Function {
                name: "dq_reward".into(),
                quest: -1,
                region: 0,
                records: vec![record(
                    40,
                    Body::Dialog {
                        name: "dlg".into(),
                        values: [3, -1],
                    },
                )],
            }],
            quests: vec![Quest {
                id: 99,
                name: "MP".into(),
                values: [0, 0, 6, 8, 0, 9, 0, 0],
            }],
            regions: vec![],
        };
        assert_eq!(Tree::decode(&tree.encode().unwrap()).unwrap(), tree);
        assert_eq!(tree.records(), 3);
    }
}
