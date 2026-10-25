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
