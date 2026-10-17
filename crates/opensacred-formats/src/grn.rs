//! Granny 1.2b files (`.GRN`) of the MDL records in `PAK/models*.pak`: the container level
//! (docs/02 §4.2).
//!
//! A file is a 64-byte signature, a 32-byte root header and a table of sections. Every section is
//! a tree of nodes in pre-order: a node is a tag, its own data and the number of its descendants.
//! The root header carries the CRC-32 of the section table and the table the CRC-32 of every
//! section, so a file is verified as a whole. The contents of the nodes are read on top of this
//! level: the tables of the object section ([`Objects`]), the [`Skeleton`] and the [`Model`] (meshes
//! and what draws them) and the [`Animation`] (curves of the bones' local transforms). The layout
//! follows the openheilig description (`formats/granny-grn.md`) and is confirmed on every file by
//! `tools/analysis/formats/grn_dump.py`.

use crate::error::{Error, ErrorKind, Result, field, to_usize};
use crate::reader::Reader;

mod animation;
mod model;
mod objects;
mod skeleton;

pub use animation::{Animation, Curve, Interpolation, Layout, Track};
pub use model::{
    Influence, Material, Mesh, MeshBone, Model, Pass, PassTriangle, Source, Surface, Texture,
    TextureFormat, Triangle,
};
pub use objects::{OBJECT_NAME, Object, Objects, Property, Value};
pub use skeleton::{Bone, Skeleton};

pub const SIGNATURE_SIZE: usize = 0x40;
/// The first 64 bytes of every GRN file.
pub const SIGNATURE: [u8; SIGNATURE_SIZE] = [
    0x2A, 0x30, 0x39, 0x04, 0x18, 0x46, 0x6C, 0x66, 0x8D, 0x6D, 0x26, 0x23, 0x9A, 0x52, 0xC1, 0x7A,
    0x70, 0x10, 0xE0, 0x44, 0x84, 0x23, 0x26, 0x32, 0x25, 0x3C, 0x0A, 0x64, 0xF7, 0x26, 0x61, 0x1F,
    0x25, 0x3C, 0x0A, 0x64, 0xF7, 0x26, 0x61, 0x1F, 0x44, 0x68, 0x2A, 0x3B, 0xA8, 0x78, 0xE4, 0x61,
    0x58, 0x58, 0x71, 0x5F, 0x08, 0x39, 0xAC, 0x1D, 0x7A, 0x3D, 0x21, 0x7F, 0x60, 0x4A, 0xF6, 0x37,
];
const ROOT_HEADER_SIZE: usize = 0x20;
const SECTION_ENTRY_SIZE: usize = 20;
/// The node table of a section follows its node count and 12 unused bytes.
const NODE_TABLE_AT: usize = 16;
const NODE_SIZE: usize = 12;

/// Tag of the root header.
pub const TAG_ROOT: u32 = 0xCA5E_0000;
/// Tag of the terminator, the last node of every section.
pub const TAG_END: u32 = 0xCA5E_FFFF;
/// The section that holds the model or the animation; the other two hold a few fixed nodes.
pub const SECTION_OBJECT: u32 = 0xCA5E_0103;
/// Node tags named by openheilig and counted in docs/02 §4.2.
pub const TAG_BONE: u32 = 0xCA5E_0506;
pub const TAG_MESH: u32 = 0xCA5E_0601;
pub const TAG_TRACK: u32 = 0xCA5E_1204;

/// GRN file parsed to the container level.
#[derive(Debug, Clone)]
pub struct Grn<'a> {
    /// The whole file, for errors about a missing part.
    file: Reader<'a>,
    sections: Vec<Section<'a>>,
}

impl<'a> Grn<'a> {
    /// Parses a GRN file held in `data`; error offsets are from its start.
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        Self::from_reader(Reader::new(data))
    }

    /// Parses the GRN file that fills the window `file`, for example the data of an MDL record:
    /// errors then carry the record and offsets from the start of the container.
    pub fn from_reader(file: Reader<'a>) -> Result<Self> {
        let len = file.rest().len();
        let mut r = file.clone();
        let signature: [u8; SIGNATURE_SIZE] = r.array("GRN signature")?;
        if signature != SIGNATURE {
            return Err(file.error(ErrorKind::Signature { what: "GRN" }));
        }

        let mut h = r.take(ROOT_HEADER_SIZE, "GRN root header")?;
        let tag_at = h.offset();
        let tag = h.u32()?; // +0x40
        if tag != TAG_ROOT {
            return Err(h.error_at(
                tag_at,
                field("root tag", tag.into(), format!("{TAG_ROOT:#x}")),
            ));
        }
        let count = h.u32()?; // +0x44: 3 in every file of the game
        let table_crc_at = h.offset();
        let table_crc = h.u32()?; // +0x48
        h.u32()?; // +0x4C: 0
        let size_at = h.offset();
        let size = h.u32()?; // +0x50; zeros up to +0x60
        let expected = len - SIGNATURE_SIZE; // the header was read, so `len` > 0x40
        if to_usize(size) != expected {
            return Err(h.error_at(
                size_at,
                field(
                    "root size",
                    size.into(),
                    format!("{expected}, the file length from +0x40"),
                ),
            ));
        }

        let table_len = to_usize(count).saturating_mul(SECTION_ENTRY_SIZE);
        let mut table = r.take(table_len, "GRN section table")?;
        check_crc(&table, table_crc, table_crc_at, "GRN section table")?;
        let mut entries = Vec::with_capacity(to_usize(count));
        for _ in 0..count {
            let tag = table.u32()?;
            table.u32()?; // +0x04: 0
            let offset_at = table.offset();
            let offset = to_usize(table.u32()?);
            let crc_at = table.offset();
            let crc = table.u32()?;
            table.u32()?; // +0x10: 0
            entries.push((tag, offset_at, offset, crc_at, crc));
        }

        // Offsets are from the start of the file; the sections follow the table back to back,
        // each running to the next one, the last to the end of the file.
        let mut previous = SIGNATURE_SIZE + ROOT_HEADER_SIZE + table_len;
        for (i, &(_, offset_at, offset, _, _)) in entries.iter().enumerate() {
            let fits = if i == 0 {
                offset == previous
            } else {
                (previous..=len).contains(&offset)
            };
            if !fits {
                let expected = if i == 0 {
                    format!("{previous:#x}, right after the section table")
                } else {
                    format!("from {previous:#x} to the file length {len:#x}")
                };
                return Err(
                    file.error_at(offset_at, field("section offset", as_u64(offset), expected))
                );
            }
            previous = offset;
        }
        let mut sections = Vec::with_capacity(entries.len());
        for (i, &(tag, _, offset, crc_at, crc)) in entries.iter().enumerate() {
            let end = entries.get(i + 1).map_or(len, |&(_, _, next, _, _)| next);
            let section = file.window(offset, end - offset, "GRN section")?;
            check_crc(&section, crc, crc_at, "GRN section")?;
            sections.push(Section::parse(tag, section)?);
        }
        Ok(Self { file, sections })
    }

    /// Sections in file order: `0xCA5E0102`, [`SECTION_OBJECT`], `0xCA5E0101` in every file of
    /// the game.
    pub fn sections(&self) -> &[Section<'a>] {
        &self.sections
    }

    /// The first section with `tag`.
    pub fn section(&self, tag: u32) -> Option<&Section<'a>> {
        self.sections.iter().find(|s| s.tag == tag)
    }

    /// The only section with the model or the animation, [`SECTION_OBJECT`].
    pub fn object_section(&self) -> Result<&Section<'a>> {
        let mut found = self.sections.iter().filter(|s| s.tag == SECTION_OBJECT);
        match (found.next(), found.count()) {
            (Some(section), 0) => Ok(section),
            (first, more) => Err(self.file.error(ErrorKind::TagCount {
                what: "GRN object section",
                tag: SECTION_OBJECT,
                found: usize::from(first.is_some()) + more,
            })),
        }
    }

    /// The skeleton of the object section with the names of its bones.
    pub fn skeleton(&self) -> Result<Skeleton<'a>> {
        let section = self.object_section()?;
        Skeleton::parse(section, &Objects::parse(section)?)
    }

    /// The meshes of the object section bound to its skeleton, with the textures, the materials
    /// and the passes that draw them.
    pub fn model(&self) -> Result<Model<'a>> {
        let section = self.object_section()?;
        let objects = Objects::parse(section)?;
        let skeleton = Skeleton::parse(section, &objects)?;
        Model::parse(section, &objects, skeleton.bones.len())
    }

    /// The animation tracks of the object section; empty for a file without tracks.
    pub fn animation(&self) -> Result<Animation> {
        let section = self.object_section()?;
        Animation::parse(section, Objects::parse(section)?.channels())
    }
}

/// Section: a tree of nodes in pre-order.
#[derive(Debug, Clone)]
pub struct Section<'a> {
    pub tag: u32,
    /// The whole section, for errors about its top level.
    window: Reader<'a>,
    /// Without the terminator.
    nodes: Vec<Node<'a>>,
}

impl<'a> Section<'a> {
    /// Section layout: `u32` node count, 12 unused bytes (zeros or stale pointers), then the nodes
    /// `{u32 tag, u32 offset from the section start, u32 descendants}`. A node's own data runs to
    /// the next node's offset; the first node's data follows the node table; the last node is the
    /// terminator at the end of the section and lies at the top level.
    fn parse(tag: u32, section: Reader<'a>) -> Result<Self> {
        let len = section.rest().len();
        let mut r = section.clone();
        let count_at = r.offset();
        let count = to_usize(r.u32()?);
        r.take(NODE_TABLE_AT - 4, "GRN section prologue")?;
        let mut table = r.take(count.saturating_mul(NODE_SIZE), "GRN node table")?;
        let mut raw = Vec::with_capacity(count);
        for _ in 0..count {
            let at = table.offset();
            let tag = table.u32()?;
            let offset = to_usize(table.u32()?);
            let descendants = to_usize(table.u32()?);
            raw.push((at, tag, offset, descendants));
        }

        let Some(&(end_at, end_tag, end_offset, end_descendants)) = raw.last() else {
            return Err(section.error_at(
                count_at,
                field("node count", 0, "at least 1, the terminator".into()),
            ));
        };
        for (name, found, expected) in [
            ("last node tag", u64::from(end_tag), u64::from(TAG_END)),
            ("terminator offset", as_u64(end_offset), as_u64(len)),
            ("terminator descendants", as_u64(end_descendants), 0),
        ] {
            if found != expected {
                return Err(section.error_at(end_at, field(name, found, expected.to_string())));
            }
        }

        // Every subtree lies inside its parent's: `open` holds, for each open ancestor, the index
        // one past its last descendant.
        let mut open: Vec<usize> = Vec::new();
        let mut previous = NODE_TABLE_AT + count * NODE_SIZE;
        for (i, &(at, tag, offset, descendants)) in raw.iter().enumerate() {
            if tag >> 16 != TAG_ROOT >> 16 {
                return Err(section.error_at(
                    at,
                    field("node tag", tag.into(), "0xCA5E0000..=0xCA5EFFFF".into()),
                ));
            }
            let fits = if i == 0 {
                offset == previous
            } else {
                offset >= previous
            };
            if !fits {
                let expected = if i == 0 {
                    format!("{previous:#x}, right after the node table")
                } else {
                    format!("at least {previous:#x}, the previous node's offset")
                };
                return Err(section.error_at(at, field("node offset", as_u64(offset), expected)));
            }
            previous = offset;

            while open.last().is_some_and(|&end| end <= i) {
                open.pop();
            }
            // Subtrees end before the terminator (index `count - 1`), so the terminator stays at
            // the top level; its own descendant count is checked above.
            let room = open
                .last()
                .copied()
                .unwrap_or(count - 1)
                .saturating_sub(i + 1);
            if i + 1 < count && descendants > room {
                return Err(section.error_at(
                    at + 8,
                    field(
                        "descendants",
                        as_u64(descendants),
                        format!("at most {room}, the rest of the parent's subtree"),
                    ),
                ));
            }
            open.push(i + 1 + descendants);
        }

        let mut nodes = Vec::with_capacity(count - 1);
        for (index, pair) in raw.windows(2).enumerate() {
            let (_, tag, offset, descendants) = pair[0];
            let next = pair[1].2;
            let data = section.window(offset, next - offset, "GRN node data")?;
            nodes.push(Node {
                tag,
                index,
                descendants,
                data,
            });
        }
        Ok(Self {
            tag,
            window: section,
            nodes,
        })
    }

    /// Nodes in pre-order, without the terminator: a node's descendants follow it.
    pub fn nodes(&self) -> &[Node<'a>] {
        &self.nodes
    }

    /// Indices of the top-level nodes.
    pub fn top_level(&self) -> Children<'_, 'a> {
        Children {
            nodes: &self.nodes,
            next: 0,
            end: self.nodes.len(),
        }
    }

    /// Indices of the direct children of node `index`; none for an index past the end.
    pub fn children(&self, index: usize) -> Children<'_, 'a> {
        let end = self.nodes.get(index).map_or(0, |node| {
            index
                .saturating_add(1)
                .saturating_add(node.descendants)
                .min(self.nodes.len())
        });
        Children {
            nodes: &self.nodes,
            next: index.saturating_add(1),
            end,
        }
    }

    /// The only child of `parent` with `tag`, or the only top-level node with it for `None`;
    /// siblings with other tags are allowed. `parent` must be a node of this section.
    pub fn child(
        &self,
        parent: Option<&Node<'a>>,
        tag: u32,
        what: &'static str,
    ) -> Result<&Node<'a>> {
        self.optional_child(parent, tag, what)?.ok_or_else(|| {
            self.error_in(
                parent,
                ErrorKind::TagCount {
                    what,
                    tag,
                    found: 0,
                },
            )
        })
    }

    /// Like [`Section::child`], but the child may be missing.
    pub fn optional_child(
        &self,
        parent: Option<&Node<'a>>,
        tag: u32,
        what: &'static str,
    ) -> Result<Option<&Node<'a>>> {
        let mut found = self.children_of(parent).filter(|node| node.tag == tag);
        match (found.next(), found.count()) {
            (first, 0) => Ok(first),
            (_, more) => Err(self.error_in(
                parent,
                ErrorKind::TagCount {
                    what,
                    tag,
                    found: 1 + more,
                },
            )),
        }
    }

    /// The children of `parent` (the top-level nodes for `None`), whatever their tags.
    pub fn children_of(&self, parent: Option<&Node<'a>>) -> impl Iterator<Item = &Node<'a>> {
        match parent {
            Some(node) => self.children(node.index),
            None => self.top_level(),
        }
        .filter_map(|index| self.nodes.get(index))
    }

    /// The children of `parent` (the top-level nodes for `None`), all of which must carry `tag`:
    /// lists such as the bones of a skeleton are addressed by position.
    pub fn list(
        &self,
        parent: Option<&Node<'a>>,
        tag: u32,
        what: &'static str,
    ) -> Result<Vec<&Node<'a>>> {
        let nodes: Vec<_> = self.children_of(parent).collect();
        match nodes.iter().find(|node| node.tag != tag) {
            Some(other) => Err(other.data.error(ErrorKind::Tag {
                what,
                found: other.tag,
                expected: tag,
            })),
            None => Ok(nodes),
        }
    }

    /// Error at the start of `parent`'s data, or of the section for `None`.
    fn error_in(&self, parent: Option<&Node<'a>>, kind: ErrorKind) -> Error {
        match parent {
            Some(node) => node.data.error(kind),
            None => self.window.error(kind),
        }
    }
}

/// Indices of sibling nodes: each step skips the subtree of the current one.
#[derive(Debug, Clone)]
pub struct Children<'s, 'a> {
    nodes: &'s [Node<'a>],
    next: usize,
    end: usize,
}

impl Iterator for Children<'_, '_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        if self.next >= self.end {
            return None;
        }
        let index = self.next;
        let descendants = self.nodes.get(index).map_or(0, |n| n.descendants);
        self.next = index.saturating_add(1).saturating_add(descendants);
        Some(index)
    }
}

/// Node of a section tree.
#[derive(Debug, Clone)]
pub struct Node<'a> {
    pub tag: u32,
    /// Position in [`Section::nodes`].
    pub index: usize,
    /// Number of nodes in the subtree below this one.
    pub descendants: usize,
    data: Reader<'a>,
}

impl<'a> Node<'a> {
    /// Reader over the node's own data, which ends where the next node's data starts; the data of
    /// the node's children follows it.
    pub fn data(&self) -> Reader<'a> {
        self.data.clone()
    }

    pub fn bytes(&self) -> &'a [u8] {
        self.data.rest()
    }

    /// [`Node::data`] of a node whose own data must be exactly `len` bytes; `what` names the size
    /// in the error.
    pub fn sized(&self, len: usize, what: &'static str) -> Result<Reader<'a>> {
        let found = self.bytes().len();
        if found != len {
            return Err(self.data.error(field(what, as_u64(found), len.to_string())));
        }
        Ok(self.data())
    }
}

/// Reads a `u32` reference to one of `count` items, numbered from 1 as GRN files number them,
/// and returns its index from 0.
fn one_based(r: &mut Reader, count: usize, what: &'static str) -> Result<usize> {
    let at = r.offset();
    let number = r.u32()?;
    match to_usize(number).checked_sub(1) {
        Some(index) if index < count => Ok(index),
        _ => Err(r.error_at(at, field(what, number.into(), format!("1..={count}")))),
    }
}

fn check_crc(window: &Reader, stored: u32, at: usize, what: &'static str) -> Result<()> {
    let computed = crc32fast::hash(window.rest());
    if computed != stored {
        return Err(window.error_at(
            at,
            ErrorKind::Checksum {
                what,
                stored,
                computed,
            },
        ));
    }
    Ok(())
}

/// For error messages: a length or offset that fits in a file fits in `u64`.
fn as_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod game_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Place;
    use crate::testing::{GrnNode, build_grn, fix_grn_crcs};

    const STRINGS: u32 = 0xCA5E_0200;

    /// Three sections shaped like the game's: the fixed nodes of `0102`, a small tree in `0103`
    /// and an empty `0101`.
    fn sample() -> Vec<u8> {
        build_grn(&[
            (
                0xCA5E_0102,
                vec![
                    GrnNode::new(STRINGS, 0, b"strings"),
                    GrnNode::new(0xCA5E_1003, 3, b""),
                    GrnNode::new(0xCA5E_1001, 1, b"ab"),
                    GrnNode::new(0xCA5E_1002, 0, b"cd"),
                    GrnNode::new(0xCA5E_1000, 0, b"e"),
                ],
            ),
            (
                SECTION_OBJECT,
                vec![
                    GrnNode::new(0xCA5E_0505, 3, b"skeleton"),
                    GrnNode::new(TAG_BONE, 0, b"bone 1"),
                    GrnNode::new(TAG_BONE, 1, b"bone 2"),
                    GrnNode::new(0xCA5E_0F00, 0, b"name"),
                    GrnNode::new(TAG_MESH, 0, b"mesh"),
                ],
            ),
            (0xCA5E_0101, vec![]),
        ])
    }

    /// Offset of the node table entry `index` in section `section` of `data`.
    fn node_entry(data: &[u8], section: usize, index: usize) -> usize {
        let at = 0x60 + 20 * section + 8;
        usize::try_from(u32::from_le_bytes(data[at..at + 4].try_into().unwrap())).unwrap()
            + NODE_TABLE_AT
            + index * NODE_SIZE
    }

    #[test]
    fn reads_sections_and_node_trees() {
        let data = sample();
        let grn = Grn::parse(&data).unwrap();
        let tags: Vec<_> = grn.sections().iter().map(|s| s.tag).collect();
        assert_eq!(tags, [0xCA5E_0102, SECTION_OBJECT, 0xCA5E_0101]);
        assert!(grn.sections()[2].nodes().is_empty());

        let fixed = &grn.sections()[0];
        assert_eq!(fixed.nodes().len(), 5);
        assert_eq!(fixed.nodes()[0].bytes(), b"strings");
        assert_eq!(fixed.top_level().collect::<Vec<_>>(), [0, 1]);
        assert_eq!(fixed.children(1).collect::<Vec<_>>(), [2, 4]);
        assert_eq!(fixed.children(2).collect::<Vec<_>>(), [3]);
        assert_eq!(fixed.nodes()[3].bytes(), b"cd");

        let object = grn.section(SECTION_OBJECT).unwrap();
        let skeleton = object.top_level().next().unwrap();
        let bones: Vec<_> = object
            .children(skeleton)
            .map(|i| object.nodes()[i].bytes())
            .collect();
        assert_eq!(bones, [b"bone 1".as_slice(), b"bone 2"]);
        assert_eq!(object.top_level().collect::<Vec<_>>(), [0, 4]);
        assert_eq!(object.nodes()[4].tag, TAG_MESH);
        assert_eq!(object.children(99).count(), 0);
        let mut name = object.nodes()[3].data();
        assert_eq!(name.bytes(4, "name").unwrap(), b"name");
    }

    #[test]
    fn child_and_list_check_the_tags_they_look_for() {
        let data = sample();
        let grn = Grn::parse(&data).unwrap();
        let object = grn.object_section().unwrap();
        let skeleton = object.child(None, 0xCA5E_0505, "skeletons").unwrap();
        assert_eq!(skeleton.index, 0);
        assert_eq!(object.child(None, TAG_MESH, "meshes").unwrap().index, 4);
        let bones = object.list(Some(skeleton), TAG_BONE, "bones").unwrap();
        assert_eq!(bones.iter().map(|b| b.index).collect::<Vec<_>>(), [1, 2]);
        let name = object.child(Some(bones[1]), 0xCA5E_0F00, "bone").unwrap();
        assert_eq!(name.bytes(), b"name");

        // Bones are not top-level nodes; the skeleton holds two.
        let err = object.child(None, TAG_BONE, "bones").unwrap_err();
        assert_eq!(
            err.to_string(),
            format!(
                "offset {:#x}: bones: 0 with tag 0xca5e0506, expected one",
                object.window.offset()
            )
        );
        let err = object.child(Some(skeleton), TAG_BONE, "bone").unwrap_err();
        assert!(matches!(err.kind, ErrorKind::TagCount { found: 2, .. }));
        assert_eq!(err.place.offset, skeleton.data().offset());
        let err = object.list(None, TAG_MESH, "meshes").unwrap_err();
        assert_eq!(
            err.to_string(),
            format!(
                "offset {:#x}: meshes: tag 0xca5e0505, expected 0xca5e0601",
                skeleton.data().offset()
            )
        );
    }

    #[test]
    fn sized_checks_the_length_of_the_own_data() {
        let data = sample();
        let grn = Grn::parse(&data).unwrap();
        let mesh = &grn.object_section().unwrap().nodes()[4];
        assert_eq!(mesh.sized(4, "mesh size").unwrap().rest(), b"mesh");
        let err = mesh.sized(8, "mesh size").unwrap_err();
        assert_eq!(err.place.offset, mesh.data().offset());
        assert!(
            err.to_string()
                .ends_with("field `mesh size` is 4, expected 8"),
            "{err}"
        );
    }

    #[test]
    fn object_section_must_be_there_once() {
        let data = build_grn(&[(0xCA5E_0102, vec![]), (0xCA5E_0101, vec![])]);
        let err = Grn::parse(&data).unwrap().object_section().unwrap_err();
        assert_eq!(
            err.to_string(),
            "offset 0x0: GRN object section: 0 with tag 0xca5e0103, expected one"
        );
        let data = build_grn(&[(SECTION_OBJECT, vec![]), (SECTION_OBJECT, vec![])]);
        let err = Grn::parse(&data).unwrap().skeleton().unwrap_err();
        assert!(matches!(err.kind, ErrorKind::TagCount { found: 2, .. }));
    }

    #[test]
    fn signature_must_match() {
        let mut data = sample();
        data[5] ^= 1;
        let err = Grn::parse(&data).unwrap_err();
        assert_eq!(err.to_string(), "offset 0x0: GRN signature does not match");
    }

    #[test]
    fn root_tag_and_size_are_checked() {
        let mut data = sample();
        data[0x40] = 1;
        let err = Grn::parse(&data).unwrap_err();
        assert_eq!(err.place.offset, 0x40);
        assert!(matches!(
            err.kind,
            ErrorKind::Field {
                field: "root tag",
                ..
            }
        ));

        let mut data = sample();
        data.push(0);
        let err = Grn::parse(&data).unwrap_err();
        assert_eq!(err.place.offset, 0x50);
        assert!(matches!(
            err.kind,
            ErrorKind::Field {
                field: "root size",
                ..
            }
        ));
    }

    #[test]
    fn section_table_crc_is_checked() {
        let mut data = sample();
        data[0x60 + 4] = 1; // a reserved word of the first entry
        let err = Grn::parse(&data).unwrap_err();
        assert_eq!(err.place.offset, 0x48);
        assert!(matches!(
            err.kind,
            ErrorKind::Checksum {
                what: "GRN section table",
                ..
            }
        ));
    }

    #[test]
    fn section_crc_is_checked() {
        let mut data = sample();
        let last = data.len() - 1;
        data[last] ^= 1;
        let err = Grn::parse(&data).unwrap_err();
        // The CRC field of the third table entry.
        assert_eq!(err.place.offset, 0x60 + 2 * 20 + 12);
        assert!(
            err.to_string().contains("GRN section: stored CRC-32"),
            "{err}"
        );
    }

    #[test]
    fn first_section_must_follow_the_table() {
        let mut data = sample();
        data[0x68..0x6C].copy_from_slice(&0x9Du32.to_le_bytes());
        fix_grn_crcs(&mut data);
        let err = Grn::parse(&data).unwrap_err();
        assert_eq!(err.place.offset, 0x68);
        assert_eq!(
            err.to_string(),
            "offset 0x68: field `section offset` is 157, expected 0x9c, right after the section table"
        );
    }

    #[test]
    fn last_node_must_be_the_terminator_at_the_section_end() {
        let mut data = sample();
        let at = node_entry(&data, 1, 5);
        data[at] = 0x55;
        fix_grn_crcs(&mut data);
        let err = Grn::parse(&data).unwrap_err();
        assert_eq!(err.place.offset, at);
        assert!(matches!(
            err.kind,
            ErrorKind::Field {
                field: "last node tag",
                ..
            }
        ));
    }

    #[test]
    fn node_offsets_must_not_decrease() {
        let mut data = sample();
        let at = node_entry(&data, 1, 2);
        let offset = u32::from_le_bytes(data[at + 4..at + 8].try_into().unwrap());
        data[at + 4..at + 8].copy_from_slice(&(offset - 7).to_le_bytes());
        fix_grn_crcs(&mut data);
        let err = Grn::parse(&data).unwrap_err();
        assert_eq!(err.place.offset, at);
        assert!(matches!(
            err.kind,
            ErrorKind::Field {
                field: "node offset",
                ..
            }
        ));
    }

    #[test]
    fn a_subtree_must_fit_in_its_parent() {
        let mut data = sample();
        // Bone 2 claims the mesh too, although the skeleton ends after bone 2's child.
        let at = node_entry(&data, 1, 2);
        data[at + 8] = 2;
        fix_grn_crcs(&mut data);
        let err = Grn::parse(&data).unwrap_err();
        assert_eq!(err.place.offset, at + 8);
        assert_eq!(
            err.to_string(),
            format!(
                "offset {:#x}: field `descendants` is 2, expected at most 1, the rest of the parent's subtree",
                at + 8
            )
        );
    }

    #[test]
    fn the_terminator_must_not_belong_to_a_subtree() {
        let mut data = sample();
        // The mesh claims the terminator as its child.
        let at = node_entry(&data, 1, 4);
        data[at + 8] = 1;
        fix_grn_crcs(&mut data);
        let err = Grn::parse(&data).unwrap_err();
        assert_eq!(err.place.offset, at + 8);
        assert!(matches!(
            err.kind,
            ErrorKind::Field {
                field: "descendants",
                found: 1,
                ..
            }
        ));
    }

    #[test]
    fn tags_must_be_in_the_granny_range() {
        let mut data = sample();
        let at = node_entry(&data, 0, 1);
        data[at + 3] = 0xCB;
        fix_grn_crcs(&mut data);
        let err = Grn::parse(&data).unwrap_err();
        assert_eq!(
            err.place,
            Place {
                record: None,
                offset: at
            }
        );
        assert!(matches!(
            err.kind,
            ErrorKind::Field {
                field: "node tag",
                found: 0xCB5E_1003,
                ..
            }
        ));
    }

    #[test]
    fn node_table_past_the_section_is_an_error() {
        let mut data = sample();
        let at = node_entry(&data, 2, 0) - NODE_TABLE_AT;
        data[at..at + 4].copy_from_slice(&1000u32.to_le_bytes());
        fix_grn_crcs(&mut data);
        let err = Grn::parse(&data).unwrap_err();
        assert!(matches!(
            err.kind,
            ErrorKind::OutOfBounds {
                what: "GRN node table",
                len: 12_000,
                ..
            }
        ));
    }

    #[test]
    fn truncated_file_is_an_error() {
        let data = sample();
        let err = Grn::parse(&data[..0x50]).unwrap_err();
        assert!(matches!(
            err.kind,
            ErrorKind::OutOfBounds {
                what: "GRN root header",
                ..
            }
        ));
    }
}
