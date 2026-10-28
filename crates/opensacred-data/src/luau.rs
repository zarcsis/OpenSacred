//! The scripts translated into Luau (#723, #720): the index of a tree's functions, by which the
//! runtime (`opensacred-scripts`) loads a function's file as it is first called, and the names of
//! the files that stage B writes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The index of a tree's functions, `index.ron` in its directory: for each name the functions of
/// that name in the tree's order, a name that several share (the quests' `ToDo:-1.1`, 1,986
/// functions of 142 names in a campaign tree) by each of them (**hypothesis**: the client's
/// lookup by name, `0x460590`, takes the first).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    pub functions: BTreeMap<String, Vec<Entry>>,
}

/// A function in a tree's sources: its file and its key in the file's table, its name or, for a
/// name that the file holds already, the name and `#<n>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub file: String,
    pub key: String,
}

/// The index's file in a tree's directory.
pub const INDEX: &str = "index.ron";

/// The extension of a source's bytecode, which stage B compiles.
pub const BYTECODE: &str = "luauc";

/// The key of a record's operands that lists its markers' tags in their order, for a record of
/// two markers or more (#993): the operands' table keeps no order of its keys.
pub const MARKS: &str = "marks";

/// The record type of the dialogues' table's entries, whose call is [`DIALOG_ENTRY`].
pub const DIALOG: u16 = 40;
/// The call of a dialogues' table's entry, `dialog_entry { name = …, values = { … } }`.
pub const DIALOG_ENTRY: &str = "dialog_entry";

/// The calls of the record types whose keywords are German words, by names of the project's own
/// in English (#1388), each after the original's keyword.
pub const ENGLISH: &[(u16, &str)] = &[
    (14, "delete_function_block"), // DeleteFunktionBlock
    (22, "call_function"),         // CallFunktion
    (27, "max_open"),              // MaxOffen
    (29, "rescue_mission"),        // Rettungsmission
    (30, "to_do_blocks"),          // ToDoBloecke
    (32, "rewards"),               // Belohnungen
    (34, "treasure_hunt"),         // Schatzsuche
    (36, "add_trade"),             // AddHandel
    (63, "quest_compass_pos"),     // QuestKompassPos
    (64, "quest_compass_obj"),     // QuestKompassObj
    (99, "particles"),             // Partikel
    (101, "kill_npc"),             // Toete_NPC
    (102, "escort"),               // Begleite
    (114, "weapon_pool"),          // Waffenpool
    (123, "penalties"),            // Strafen
    (124, "free_region"),          // RegionFrei
    (125, "prize"),                // Gewinn
    (140, "make_hiding_place"),    // MachVersteck
];

/// The English call of a record type of a German keyword ([`ENGLISH`]).
fn english(kind: u16) -> Option<&'static str> {
    ENGLISH
        .iter()
        .find(|&&(k, _)| k == kind)
        .map(|&(_, call)| call)
}

/// The call of a record type in Luau: its first keyword in snake case
/// ([`crate::netscript::name`]) or, for a German one, its English name ([`ENGLISH`]), `type<n>`
/// for the types 39 and 47, which have none, and [`DIALOG_ENTRY`] for the dialogues' entries; none
/// for any other, the flow's among them.
pub fn call_name(kind: u16) -> Option<String> {
    match (kind, crate::netscript::name(kind)) {
        (DIALOG, _) => Some(DIALOG_ENTRY.to_owned()),
        (39 | 47, _) => Some(format!("type{kind}")),
        (_, Some(keyword)) => Some(english(kind).map_or_else(|| snake(keyword), str::to_owned)),
        (_, None) => None,
    }
}

/// The record type of a call in Luau ([`call_name`]); a German keyword in snake case names none.
pub fn kind_of(call: &str) -> Option<u16> {
    match call {
        DIALOG_ENTRY => Some(DIALOG),
        "type39" => Some(39),
        "type47" => Some(47),
        _ => ENGLISH
            .iter()
            .find(|&&(_, english)| english == call)
            .map(|&(kind, _)| kind)
            .or_else(|| {
                crate::netscript::KEYWORDS
                    .iter()
                    .find(|&&(keyword, kind)| english(kind).is_none() && snake(keyword) == call)
                    .map(|&(_, kind)| kind)
            }),
    }
}

/// A keyword in snake case: `CreateNPC` → `create_npc`, `NPC_TalkTo` → `npc_talk_to`.
pub fn snake(keyword: &str) -> String {
    let chars: Vec<char> = keyword.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let before = i.checked_sub(1).map(|j| chars[j]);
            let after = chars.get(i + 1).copied();
            let starts_word = match before {
                Some(b) if b.is_ascii_lowercase() || b.is_ascii_digit() => true,
                Some(b) if b.is_ascii_uppercase() => after.is_some_and(|a| a.is_ascii_lowercase()),
                _ => false,
            };
            if starts_word && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_type_and_its_call_name_each_other() {
        assert_eq!(snake("CreateNPC"), "create_npc");
        assert_eq!(snake("NPC_TalkTo"), "npc_talk_to");
        assert_eq!(snake("SetUpQuest"), "set_up_quest");
        for kind in [1, 3, 8, 20, 21, 23, 45, 48, 50, 51, 72, 100, 114, 115, 137] {
            let call = call_name(kind).unwrap();
            assert_eq!(kind_of(&call), Some(kind), "{call}");
        }
        assert_eq!(call_name(40).as_deref(), Some("dialog_entry"));
        assert_eq!(kind_of("type39"), Some(39));
        // `DelNPC` and `DelOBJ` share their type, called by the first.
        assert_eq!(call_name(55).as_deref(), Some("del_npc"));
        assert_eq!(call_name(58), Some("if".to_owned()));
        assert_eq!(kind_of("no_such_record"), None);
    }

    #[test]
    fn the_german_keywords_are_called_in_english() {
        assert_eq!(call_name(125).as_deref(), Some("prize"));
        assert_eq!(call_name(124).as_deref(), Some("free_region"));
        assert_eq!(call_name(101).as_deref(), Some("kill_npc"));
        for &(kind, call) in ENGLISH {
            let keyword = crate::netscript::name(kind).unwrap();
            assert_ne!(snake(keyword), call, "{keyword}");
            assert_eq!(kind_of(&snake(keyword)), None, "{keyword}");
            assert_eq!(kind_of(call), Some(kind), "{call}");
        }
        // Every call names its type back, and no two types share one.
        let mut calls = std::collections::BTreeSet::new();
        for kind in 0..141 {
            if let Some(call) = call_name(kind) {
                assert_eq!(kind_of(&call), Some(kind), "{call}");
                assert!(calls.insert(call));
            }
        }
    }
}
