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
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct NetScript {
    pub trees: Vec<TreeEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Function {
    pub name: String,
    /// The id of the quest it belongs to, or −1.
    pub quest: i32,
    /// 0, or the region of a dynamic quest in the Sacred campaign trees (hypothesis).
    pub region: i32,
    pub records: Vec<Record>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Record {
    /// The original record type, below 141.
    pub kind: u16,
    pub body: Body,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Operand {
    /// The original operand tag.
    pub tag: u8,
    pub value: Value,
}

/// The payload of an operand (`opensacred_formats::netscript::Value`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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

/// The keywords of the script compiler and the record type each compiles to, from its five
/// resolvers in `Sacred.exe`: statements (`0x451BE0`, `0x452370`), quest directives
/// (`0x452130`, `0x452200`, written with a colon in the source) and dialog commands
/// (`0x4522C0`). Types 0, 39–42 and 47 have no keyword.
pub const KEYWORDS: &[(&str, u16)] = &[
    ("CreateNPC", 1),
    ("SetObjState", 2),
    ("SetNPCState", 3),
    ("SetBaseTrigger", 4),
    ("DelBaseTrigger", 5),
    ("TalkTo", 6),
    ("EquipNPC", 7),
    ("CreateObj", 8),
    ("StartScript", 9),
    ("QuestDone", 10),
    ("QuestNotDone", 11),
    ("QuestInProgress", 12),
    ("QuestNotInProgress", 13),
    ("DeleteFunktionBlock", 14),
    ("ExitQuest", 15),
    ("MoveOver", 16),
    ("AddExp", 17),
    ("AddGold", 18),
    ("AddGems", 19),
    ("TriggerQuest", 20),
    ("SetUpQuest", 21),
    ("CallFunktion", 22),
    ("DefPos", 23),
    ("GroupIsDead", 24),
    ("CheckForQuestPool", 25),
    ("Text", 26),
    ("MaxOffen", 27),
    ("Delay", 28),
    ("Rettungsmission", 29),
    ("ToDoBloecke", 30),
    ("Dialog", 31),
    ("Belohnungen", 32),
    ("HideTmpToDo", 33),
    ("Schatzsuche", 34),
    ("FillRegion", 35),
    ("AddHandel", 36),
    ("SetRG", 37),
    ("UnsetRG", 38),
    ("ExitDlg", 43),
    ("DelDlg", 44),
    ("StartPosition", 45),
    ("Teleport", 46),
    ("CreateTrigger", 48),
    ("SetTriggerState", 49),
    ("TriggerPatch", 50),
    ("FillSector", 51),
    ("SetHP", 52),
    ("QuestBook", 53),
    ("LoseQuest", 54),
    ("DelNPC", 55),
    ("DelOBJ", 55),
    ("SetTimer", 56),
    ("DelTimer", 57),
    ("IF", 58),
    ("ELSE", 59),
    ("SetButton", 60),
    ("ChDlgText", 61),
    ("NOP", 62),
    ("QuestKompassPos", 63),
    ("QuestKompassObj", 64),
    ("DefNum", 65),
    ("ELSEIF", 66),
    ("SetVar", 67),
    ("SetVarBit", 68),
    ("UnsetVarBit", 69),
    ("SetFocus", 70),
    ("NPC_TalkTo", 71),
    ("NPC_Goto", 72),
    ("SetGroupState", 73),
    ("GroupGoto", 74),
    ("IncVar", 75),
    ("DecVar", 76),
    ("UnTriggerQuest", 77),
    ("FillChest", 78),
    ("Morph", 79),
    ("SetST", 80),
    ("SetGS", 81),
    ("SetWI", 82),
    ("SetRP", 83),
    ("SetRM", 84),
    ("SetCH", 85),
    ("SetIcon", 86),
    ("SetQuestInfo", 87),
    ("SetAnimMode", 88),
    ("SetPlayMode", 89),
    ("Wait", 90),
    ("PlayAnim", 91),
    ("Attack", 92),
    ("ActObj", 93),
    ("MouseEvent", 94),
    ("Popup", 95),
    ("ReplaceSpawn", 96),
    ("SetMapIcon", 97),
    ("SecInfo", 98),
    ("Partikel", 99),
    ("SpawnValues", 100),
    ("Toete_NPC", 101),
    ("Begleite", 102),
    ("SetCV", 103),
    ("PlaySound", 104),
    ("RndVar", 105),
    ("SetRgnDialog", 106),
    ("BalanceSpawnItem", 107),
    ("AddPoolPos", 108),
    ("GetPoolPos", 109),
    ("RenPos", 110),
    ("ToBegin", 111),
    ("AddPoolRgn", 112),
    ("GetPoolRgn", 113),
    ("Waffenpool", 114),
    ("MakeGroup", 115),
    ("RegionChange", 116),
    ("ActivateQuest", 117),
    ("SetClientVars", 118),
    ("GetClientVars", 119),
    ("GameEnd", 120),
    ("AutoSave", 121),
    ("SolveTime", 122),
    ("Strafen", 123),
    ("RegionFrei", 124),
    ("Gewinn", 125),
    ("RejectQuest", 126),
    ("PlayMusic", 127),
    ("PlayJingle", 128),
    ("SetScriptSoundControle", 129),
    ("Teleporter", 130),
    ("GameActive", 131),
    ("InfoPlayer", 132),
    ("GetPoolPosition", 133),
    ("SetCinemaMode", 134),
    ("SetOnKill", 135),
    ("SetOnCollect", 136),
    ("SetDrop", 137),
    ("GiveStat", 138),
    ("GiveSkill", 139),
    ("MachVersteck", 140),
];

/// The first keyword of a record type.
pub fn name(kind: u16) -> Option<&'static str> {
    KEYWORDS
        .iter()
        .find(|&&(_, k)| k == kind)
        .map(|&(name, _)| name)
}

/// The shape of a tag's payload: which of the 40 handlers of the operand reader
/// `FUN_00472BC0` the tag dispatches to (docs/re/netscript.md lists them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// The reader logs an error and stops the record.
    End,
    Marker,
    U8,
    U16,
    U16x2,
    U16x4,
    I32,
    I32x2,
    I32x3,
    I32x4,
    Str,
    Strs,
    NumStr,
    NumStrExpression,
    VectorOrName,
    IntOrName,
    Flag,
    IntOrVariable,
    Variable,
}

/// The shape of the payload of the tag `tag`.
pub fn shape(tag: u8) -> Shape {
    match tag {
        0x00 | 0x17 | 0x18 | 0x21 | 0x22 | 0x76 | 0x94 | 0xA2..=0xFF => Shape::End,
        0x28 | 0x8B => Shape::U8,
        0x03 | 0x0A | 0x6B | 0x6C | 0x93 | 0x9B | 0x9C => Shape::U16,
        0x84 => Shape::U16x2,
        0x15 => Shape::U16x4,
        0x02
        | 0x11
        | 0x1C
        | 0x1D
        | 0x36
        | 0x3B
        | 0x53..=0x57
        | 0x73
        | 0x75
        | 0x7E
        | 0x7F
        | 0x86
        | 0x8C
        | 0x90 => Shape::I32,
        0x33 | 0x34 | 0x79 | 0x87..=0x89 => Shape::I32x2,
        0x19 | 0x20 | 0x2A | 0x35 | 0x4D => Shape::I32x3,
        0x3D => Shape::I32x4,
        0x01
        | 0x05
        | 0x09
        | 0x16
        | 0x1E
        | 0x29
        | 0x3E
        | 0x40
        | 0x41
        | 0x47
        | 0x52
        | 0x60
        | 0x63
        | 0x67..=0x6A
        | 0x77
        | 0x7D
        | 0x81..=0x83
        | 0x8F
        | 0x95
        | 0x9D => Shape::Str,
        0x3A | 0x92 => Shape::Strs,
        0x6F | 0x7A => Shape::NumStr,
        0x48..=0x4A | 0x5D | 0x5E | 0x6D | 0x6E => Shape::NumStrExpression,
        0x04 | 0x0C | 0x0D | 0x3C => Shape::VectorOrName,
        0x0B | 0x38 | 0x5F => Shape::IntOrName,
        0x37 | 0x4B => Shape::Flag,
        0x1F => Shape::IntOrVariable,
        VARIABLE_TAG => Shape::Variable,
        _ => Shape::Marker,
    }
}

/// The tag of a variable's reference (`FUN_00453970`), whose payload names the variable.
pub const VARIABLE_TAG: u8 = 0x9F;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Quest {
    pub id: u32,
    pub name: String,
    pub values: [i32; 8],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub struct Region {
    pub id: u32,
    pub name: String,
    pub values: [i32; 5],
    pub lists: [Vec<u32>; 3],
    pub entries: Vec<RegionEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
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
