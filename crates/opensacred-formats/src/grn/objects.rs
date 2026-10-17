//! Tables of the GRN object section that its other parts refer to by number (docs/02 §4.2): the
//! string table; the data extension objects, which carry the exporter's properties of every scene
//! object (`__ObjectName`, `__FileName`, `__Description`); and the transform channels, each
//! referring to one object. Tag names follow grn2glb (`src/core/grn_types.h`, MIT).

use super::{Node, Section, as_u64, one_based};
use crate::error::{Result, field, to_usize};
use crate::reader::Reader;

const TAG_STRINGS: u32 = 0xCA5E_0200;
const TAG_OBJECTS: u32 = 0xCA5E_0F03;
const TAG_OBJECT: u32 = 0xCA5E_0F00;
const TAG_PROPERTIES: u32 = 0xCA5E_0F05;
const TAG_PROPERTY: u32 = 0xCA5E_0F01;
const TAG_VALUES: u32 = 0xCA5E_0F06;
const TAG_VALUE: u32 = 0xCA5E_0F02;
pub(super) const TAG_OBJECT_REF: u32 = 0xCA5E_0F04;
const TAG_CHANNELS: u32 = 0xCA5E_0B01;
const TAG_CHANNEL: u32 = 0xCA5E_0B00;

/// Key of the property that names a scene object.
pub const OBJECT_NAME: &[u8] = b"__ObjectName";

/// String table, data extension objects and transform channels of an object section.
#[derive(Debug, Clone)]
pub struct Objects<'a> {
    strings: Vec<&'a [u8]>,
    objects: Vec<Object<'a>>,
    /// Index into `objects` of every transform channel.
    channels: Vec<usize>,
}

impl<'a> Objects<'a> {
    pub fn parse(section: &Section<'a>) -> Result<Self> {
        let strings = read_strings(section.child(None, TAG_STRINGS, "GRN string table")?)?;

        let list = section.child(None, TAG_OBJECTS, "GRN data extensions")?;
        let mut objects = Vec::new();
        for object in section.list(Some(list), TAG_OBJECT, "GRN data extensions")? {
            // The object's own 8 bytes are 1, 1 in every file; their meaning is unknown.
            let list = section.child(Some(object), TAG_PROPERTIES, "GRN object")?;
            let mut properties = Vec::new();
            for property in section.list(Some(list), TAG_PROPERTY, "GRN object properties")? {
                let key = string(&strings, &mut property.sized(4, "property size")?)?;
                let list = section.child(Some(property), TAG_VALUES, "GRN property")?;
                let mut values = Vec::new();
                for value in section.list(Some(list), TAG_VALUE, "GRN property values")? {
                    let mut r = value.sized(8, "property value size")?;
                    let number = r.f32()?;
                    let text = string(&strings, &mut r)?;
                    values.push(Value { number, text });
                }
                properties.push(Property { key, values });
            }
            objects.push(Object { properties });
        }

        let list = section.child(None, TAG_CHANNELS, "GRN transform channels")?;
        let mut channels = Vec::new();
        for channel in section.list(Some(list), TAG_CHANNEL, "GRN transform channels")? {
            let reference =
                section.child(Some(channel), TAG_OBJECT_REF, "GRN transform channel")?;
            let mut r = reference.sized(4, "object reference size")?;
            channels.push(one_based(&mut r, objects.len(), "channel object")?);
        }
        Ok(Self {
            strings,
            objects,
            channels,
        })
    }

    /// The string table; string 0 is empty.
    pub fn strings(&self) -> &[&'a [u8]] {
        &self.strings
    }

    /// Data extension objects in file order; files refer to them from 1.
    pub fn objects(&self) -> &[Object<'a>] {
        &self.objects
    }

    /// Number of transform channels.
    pub fn channels(&self) -> usize {
        self.channels.len()
    }

    /// The object of transform channel `index`, counted from 0.
    pub fn channel(&self, index: usize) -> Option<&Object<'a>> {
        self.objects.get(*self.channels.get(index)?)
    }
}

/// Data extension object: the exporter's properties of one scene object (a bone, a mesh, a
/// material).
#[derive(Debug, Clone, PartialEq)]
pub struct Object<'a> {
    pub properties: Vec<Property<'a>>,
}

impl<'a> Object<'a> {
    /// Values of the first property with `key`.
    pub fn property(&self, key: &[u8]) -> Option<&[Value<'a>]> {
        let property = self.properties.iter().find(|p| p.key == key)?;
        Some(&property.values)
    }

    /// Text of `__ObjectName`, if the property has exactly one value.
    pub fn name(&self) -> Option<&'a [u8]> {
        match self.property(OBJECT_NAME)? {
            [value] => Some(value.text),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Property<'a> {
    pub key: &'a [u8],
    pub values: Vec<Value<'a>>,
}

/// Property value: a number and a string from the string table. The number is 0 everywhere except
/// 4 properties of motions, whose several values pair numbers with marker texts (hypothesis:
/// note-track keys with their times in seconds).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Value<'a> {
    pub number: f32,
    pub text: &'a [u8],
}

/// String table: `u32` count, `u32` length, then `count` NUL-terminated strings that fill
/// `length` bytes; the node pads them to 4 bytes.
fn read_strings<'a>(node: &Node<'a>) -> Result<Vec<&'a [u8]>> {
    let mut r = node.data();
    let count_at = r.offset();
    let count = to_usize(r.u32()?);
    let len_at = r.offset();
    let len = to_usize(r.u32()?);
    let size = node.bytes().len();
    if len
        .checked_next_multiple_of(4)
        .and_then(|n| n.checked_add(8))
        != Some(size)
    {
        return Err(r.error_at(
            len_at,
            field(
                "string table length",
                as_u64(len),
                format!(
                    "{} after padding to 4, the node size {size} less 8",
                    size.saturating_sub(8)
                ),
            ),
        ));
    }
    let mut text = r.take(len, "GRN string table")?;
    let mut strings = Vec::new();
    while !text.rest().is_empty() {
        let end = text.rest().iter().position(|&b| b == 0);
        let string = text.take(end.unwrap_or(text.rest().len()), "GRN string")?;
        text.take(1, "GRN string terminator")?;
        strings.push(string.rest());
    }
    if strings.len() != count {
        return Err(r.error_at(
            count_at,
            field(
                "string count",
                as_u64(count),
                format!("{}, the strings in {len} bytes", strings.len()),
            ),
        ));
    }
    Ok(strings)
}

/// Reads a `u32` index into the string table.
fn string<'a>(strings: &[&'a [u8]], r: &mut Reader) -> Result<&'a [u8]> {
    let at = r.offset();
    let index = r.u32()?;
    strings.get(to_usize(index)).copied().ok_or_else(|| {
        r.error_at(
            at,
            field(
                "string index",
                index.into(),
                format!("below {}, the string count", strings.len()),
            ),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;
    use crate::grn::Grn;
    use crate::testing::{GrnBone, GrnNode, grn_node, object_grn, skeleton_nodes};

    fn sample() -> Vec<GrnNode> {
        let bone = |name, parent| GrnBone {
            name,
            parent,
            transform: [0.0; 16],
        };
        skeleton_nodes(&[bone(b"__Root".as_slice(), 0), bone(b"Bip01", 0)])
    }

    fn error(nodes: Vec<GrnNode>) -> crate::Error {
        let data = object_grn(nodes);
        let grn = Grn::parse(&data).unwrap();
        Objects::parse(grn.object_section().unwrap()).unwrap_err()
    }

    #[test]
    fn reads_strings_objects_and_channels() {
        let data = object_grn(sample());
        let grn = Grn::parse(&data).unwrap();
        let objects = Objects::parse(grn.object_section().unwrap()).unwrap();
        assert_eq!(
            objects.strings(),
            [b"".as_slice(), b"__ObjectName", b"__Root", b"Bip01"]
        );
        let names: Vec<_> = objects.objects().iter().map(Object::name).collect();
        assert_eq!(names, [Some(b"__Root".as_slice()), Some(b"Bip01")]);
        assert_eq!(
            objects.objects()[1].property(OBJECT_NAME),
            Some(
                [Value {
                    number: 0.0,
                    text: b"Bip01"
                }]
                .as_slice()
            )
        );
        assert_eq!(objects.objects()[1].property(b"__FileName"), None);
        // The channels refer to the objects in reverse order.
        assert_eq!(objects.channels(), 2);
        let channel = |i| objects.channel(i).and_then(Object::name);
        assert_eq!(channel(0), Some(b"Bip01".as_slice()));
        assert_eq!(channel(1), Some(b"__Root".as_slice()));
        assert_eq!(objects.channel(2), None);
    }

    #[test]
    fn a_property_may_hold_several_values() {
        // A second value of the first object's name, with a number: the name is then ambiguous.
        let mut nodes = sample();
        for (tag, extra) in [(TAG_OBJECTS, 1), (TAG_OBJECT, 1), (TAG_PROPERTIES, 1)] {
            grn_node(&mut nodes, tag, 0).descendants += extra;
        }
        grn_node(&mut nodes, TAG_PROPERTY, 0).descendants += 1;
        grn_node(&mut nodes, TAG_VALUES, 0).descendants += 1;
        let at = nodes.iter().position(|n| n.tag == TAG_VALUE).unwrap();
        let value = [0.5f32.to_le_bytes(), 3u32.to_le_bytes()].concat();
        nodes.insert(at + 1, GrnNode::new(TAG_VALUE, 0, &value));

        let data = object_grn(nodes);
        let grn = Grn::parse(&data).unwrap();
        let objects = Objects::parse(grn.object_section().unwrap()).unwrap();
        let root = &objects.objects()[0];
        assert_eq!(
            root.property(OBJECT_NAME).unwrap(),
            [
                Value {
                    number: 0.0,
                    text: b"__Root"
                },
                Value {
                    number: 0.5,
                    text: b"Bip01"
                }
            ]
        );
        assert_eq!(root.name(), None);
        let err = grn.skeleton().unwrap_err();
        assert!(
            matches!(
                err.kind,
                ErrorKind::NoProperty {
                    key: "__ObjectName",
                    ..
                }
            ),
            "{err}"
        );
    }

    #[test]
    fn string_count_must_match_the_strings() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_STRINGS, 0).data[0] = 9;
        let err = error(nodes);
        assert!(
            err.to_string()
                .ends_with("field `string count` is 9, expected 4, the strings in 27 bytes"),
            "{err}"
        );
    }

    #[test]
    fn string_table_must_fill_its_node() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_STRINGS, 0).data[4] = 30;
        let err = error(nodes);
        assert!(
            err.to_string().ends_with(
                "field `string table length` is 30, expected 28 after padding to 4, \
                 the node size 36 less 8"
            ),
            "{err}"
        );
    }

    #[test]
    fn a_string_ends_with_nul() {
        let mut nodes = sample();
        let table = &mut grn_node(&mut nodes, TAG_STRINGS, 0).data;
        *table = [1u32.to_le_bytes(), 3u32.to_le_bytes()].concat();
        table.extend_from_slice(b"abc\0");
        let err = error(nodes);
        assert!(
            matches!(
                err.kind,
                ErrorKind::OutOfBounds {
                    what: "GRN string terminator",
                    ..
                }
            ),
            "{err}"
        );
    }

    #[test]
    fn string_indices_stay_in_the_table() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_VALUE, 1).data[4] = 99;
        let err = error(nodes);
        assert!(
            err.to_string()
                .ends_with("field `string index` is 99, expected below 4, the string count"),
            "{err}"
        );
    }

    #[test]
    fn channels_refer_to_objects() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_OBJECT_REF, 1).data[0] = 0;
        let err = error(nodes);
        assert!(
            err.to_string()
                .ends_with("field `channel object` is 0, expected 1..=2"),
            "{err}"
        );
    }

    #[test]
    fn a_property_list_holds_only_properties() {
        let mut nodes = sample();
        grn_node(&mut nodes, TAG_PROPERTY, 1).tag = TAG_OBJECT;
        let err = error(nodes);
        assert_eq!(
            err.kind.to_string(),
            "GRN object properties: tag 0xca5e0f00, expected 0xca5e0f01"
        );
    }
}
