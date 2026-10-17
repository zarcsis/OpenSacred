//! Test helpers: building synthetic containers and reaching the game files and the Python
//! prototypes. The crate writes nothing, so the builder exists only in tests.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::pak::{DIR_ENTRY_SIZE, HEADER_SIZE};
use crate::tex::RECORD_HEADER_SIZE;
use crate::{grn, mdl};

/// Record of a synthetic container: type and size for the directory, and the record bytes.
pub struct PakRecord {
    pub kind: u32,
    pub size: u32,
    pub bytes: Vec<u8>,
}

impl PakRecord {
    pub fn new(kind: u32, bytes: &[u8]) -> Self {
        Self {
            kind,
            size: u32::try_from(bytes.len()).unwrap(),
            bytes: bytes.to_vec(),
        }
    }
}

/// Container: 0x100-byte header, directory, records back to back; `None` is an empty
/// `(0, 0, 0)` entry.
pub fn build_pak(magic: [u8; 3], version: u8, records: &[Option<PakRecord>]) -> Vec<u8> {
    let mut data = vec![0; HEADER_SIZE];
    data[..3].copy_from_slice(&magic);
    data[3] = version;
    data[4..8].copy_from_slice(&u32::try_from(records.len()).unwrap().to_le_bytes());
    let mut offset = HEADER_SIZE + records.len() * DIR_ENTRY_SIZE;
    for record in records {
        let (kind, at, size) = match record {
            Some(r) => {
                let at = offset;
                offset += r.bytes.len();
                (r.kind, u32::try_from(at).unwrap(), r.size)
            }
            None => (0, 0, 0),
        };
        for value in [kind, at, size] {
            data.extend_from_slice(&value.to_le_bytes());
        }
    }
    for record in records.iter().flatten() {
        data.extend_from_slice(&record.bytes);
    }
    data
}

/// TEX record: the 80-byte header and the data; the directory gets `size = data_size`
/// (docs/02 §3.1).
pub fn tex_record(
    name: &[u8],
    width: u16,
    height: u16,
    format: u8,
    data_size: u32,
    payload: &[u8],
) -> PakRecord {
    let mut bytes = vec![0; RECORD_HEADER_SIZE];
    bytes[..name.len()].copy_from_slice(name);
    bytes[0x20..0x22].copy_from_slice(&width.to_le_bytes());
    bytes[0x22..0x24].copy_from_slice(&height.to_le_bytes());
    bytes[0x24] = format;
    bytes[0x25..0x29].copy_from_slice(&data_size.to_le_bytes());
    bytes.extend_from_slice(payload);
    PakRecord {
        kind: u32::from(format),
        size: data_size,
        bytes,
    }
}

/// MDL record: the descriptor and the GRN data, with the directory size of docs/02 §4.1.
/// `motions` fills `ModelDesc` slots; [`build_mdl`] sets `dataOffset`.
pub fn mdl_record(
    kind: mdl::Kind,
    name: &[u8],
    flags: u16,
    motions: &[(usize, u32)],
    grn: &[u8],
) -> PakRecord {
    let desc_size = usize::try_from(kind.descriptor_size()).unwrap();
    let mut bytes = vec![0; desc_size];
    bytes[..name.len()].copy_from_slice(name);
    bytes[0x20..0x24].copy_from_slice(&0xFFu32.to_le_bytes());
    let data_size = u32::try_from(grn.len()).unwrap();
    bytes[0x28..0x2C].copy_from_slice(&data_size.to_le_bytes());
    bytes[0x2C..0x2E].copy_from_slice(&flags.to_le_bytes());
    for &(slot, motion) in motions {
        bytes[0x70 + 4 * slot..0x74 + 4 * slot].copy_from_slice(&motion.to_le_bytes());
    }
    bytes.extend_from_slice(grn);
    let copies = if kind == mdl::Kind::Model { 2 } else { 1 };
    PakRecord {
        kind: kind.code(),
        size: kind.descriptor_size() + copies * data_size,
        bytes,
    }
}

/// MDL container: the models, then the motions; every descriptor gets the `dataOffset` of its
/// GRN data. The descriptor size follows the directory type of each record.
pub fn build_mdl(models: Vec<PakRecord>, motions: Vec<PakRecord>) -> Vec<u8> {
    let counts = [models.len(), motions.len()].map(|n| u32::try_from(n).unwrap());
    let records: Vec<_> = models.into_iter().chain(motions).map(Some).collect();
    let mut data = build_pak(mdl::MAGIC, mdl::VERSION, &records);
    let fields = [
        counts[0],
        counts[1],
        mdl::MODEL_DESC_SIZE,
        mdl::MOTION_DESC_SIZE,
    ];
    for (i, value) in fields.into_iter().enumerate() {
        data[8 + 4 * i..12 + 4 * i].copy_from_slice(&value.to_le_bytes());
    }
    for i in 0..records.len() {
        let entry = HEADER_SIZE + i * DIR_ENTRY_SIZE;
        let field = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let (kind, offset) = (field(entry), field(entry + 4));
        let desc_size = if kind == mdl::Kind::Model.code() {
            mdl::MODEL_DESC_SIZE
        } else {
            mdl::MOTION_DESC_SIZE
        };
        let at = usize::try_from(offset).unwrap() + 0x24;
        data[at..at + 4].copy_from_slice(&(offset + desc_size).to_le_bytes());
    }
    data
}

/// Node of a synthetic GRN section, listed in pre-order.
#[derive(Clone)]
pub struct GrnNode {
    pub tag: u32,
    pub descendants: u32,
    pub data: Vec<u8>,
}

impl GrnNode {
    pub fn new(tag: u32, descendants: u32, data: &[u8]) -> Self {
        Self {
            tag,
            descendants,
            data: data.to_vec(),
        }
    }
}

/// GRN file (docs/02 §4.2): signature, root header, section table, then the sections, each with
/// its node table, the node data and the terminator; the CRCs are correct.
pub fn build_grn(sections: &[(u32, Vec<GrnNode>)]) -> Vec<u8> {
    let le = |value: usize| u32::try_from(value).unwrap().to_le_bytes();
    let mut data = grn::SIGNATURE.to_vec();
    data.resize(0x60 + 20 * sections.len(), 0);
    for (i, (tag, nodes)) in sections.iter().enumerate() {
        let (entry, start) = (0x60 + 20 * i, data.len());
        data[entry..entry + 4].copy_from_slice(&tag.to_le_bytes());
        data[entry + 8..entry + 12].copy_from_slice(&le(start));
        let count = nodes.len() + 1;
        data.extend_from_slice(&le(count));
        data.extend_from_slice(&[0; 12]);
        let mut offset = 16 + 12 * count;
        for node in nodes {
            data.extend_from_slice(&node.tag.to_le_bytes());
            data.extend_from_slice(&le(offset));
            data.extend_from_slice(&node.descendants.to_le_bytes());
            offset += node.data.len();
        }
        data.extend_from_slice(&grn::TAG_END.to_le_bytes());
        data.extend_from_slice(&le(offset));
        data.extend_from_slice(&[0; 4]);
        for node in nodes {
            data.extend_from_slice(&node.data);
        }
    }
    let size = le(data.len() - 0x40);
    data[0x40..0x44].copy_from_slice(&grn::TAG_ROOT.to_le_bytes());
    data[0x44..0x48].copy_from_slice(&le(sections.len()));
    data[0x50..0x54].copy_from_slice(&size);
    fix_grn_crcs(&mut data);
    data
}

/// The `nth` node with `tag`, counted from 0, for a test to change.
pub fn grn_node(nodes: &mut [GrnNode], tag: u32, nth: usize) -> &mut GrnNode {
    nodes
        .iter_mut()
        .filter(|node| node.tag == tag)
        .nth(nth)
        .unwrap_or_else(|| panic!("no node {tag:#010x} number {nth}"))
}

/// GRN file with the object section `object` between the fixed sections `0102` and `0101`.
pub fn object_grn(object: Vec<GrnNode>) -> Vec<u8> {
    build_grn(&[
        (0xCA5E_0102, vec![]),
        (grn::SECTION_OBJECT, object),
        (0xCA5E_0101, vec![]),
    ])
}

/// Bone of a synthetic skeleton: the name of its object, the parent field and the transform
/// (translation, rotation, scale/shear).
pub struct GrnBone {
    pub name: &'static [u8],
    pub parent: u32,
    pub transform: [f32; 16],
}

/// Nodes of an object section with a skeleton (docs/02 §4.2): the string table `"",
/// "__ObjectName"` and the names; a data extension object per bone, in bone order; a transform
/// channel per object, in reverse order; the skeleton; a form that binds every bone to the channel
/// of its object.
pub fn skeleton_nodes(bones: &[GrnBone]) -> Vec<GrnNode> {
    let le = |value: usize| u32::try_from(value).unwrap().to_le_bytes();
    let n = bones.len();
    let mut text = b"\0__ObjectName\0".to_vec();
    for bone in bones {
        text.extend_from_slice(bone.name);
        text.push(0);
    }
    let mut table = [le(n + 2), le(text.len())].concat();
    table.extend_from_slice(&text);
    table.resize(table.len().next_multiple_of(4), 0);

    let tag = |low: u32| 0xCA5E_0000 | low;
    let count = |value: usize| u32::try_from(value).unwrap();
    let mut nodes = vec![GrnNode::new(tag(0x0200), 0, &table)];
    nodes.push(GrnNode::new(tag(0x0F03), count(5 * n), b""));
    for i in 0..n {
        nodes.push(GrnNode::new(tag(0x0F00), 4, &[le(1), le(1)].concat()));
        nodes.push(GrnNode::new(tag(0x0F05), 3, b""));
        nodes.push(GrnNode::new(tag(0x0F01), 2, &le(1)));
        nodes.push(GrnNode::new(tag(0x0F06), 1, b""));
        nodes.push(GrnNode::new(tag(0x0F02), 0, &[le(0), le(2 + i)].concat()));
    }
    nodes.push(GrnNode::new(tag(0x0B01), count(2 * n), b""));
    for channel in 0..n {
        nodes.push(GrnNode::new(tag(0x0B00), 1, b""));
        nodes.push(GrnNode::new(tag(0x0F04), 0, &le(n - channel)));
    }
    nodes.push(GrnNode::new(tag(0x0507), count(n + 2), b""));
    nodes.push(GrnNode::new(tag(0x0505), count(n + 1), b""));
    nodes.push(GrnNode::new(tag(0x0508), count(n), b""));
    for bone in bones {
        let mut record = bone.parent.to_le_bytes().to_vec();
        for value in bone.transform {
            record.extend_from_slice(&value.to_le_bytes());
        }
        nodes.push(GrnNode::new(grn::TAG_BONE, 0, &record));
    }
    // Bone `i` is the object `i + 1`, which channel `n - i` refers to.
    let channels: Vec<u8> = (0..n).flat_map(|i| le(n - i)).collect();
    nodes.push(GrnNode::new(tag(0x0C01), 5, b""));
    nodes.push(GrnNode::new(tag(0x0C00), 4, b""));
    nodes.push(GrnNode::new(tag(0x0C07), 3, b""));
    nodes.push(GrnNode::new(tag(0x0C08), 2, &le(1)));
    nodes.push(GrnNode::new(tag(0x0C02), 0, &channels));
    nodes.push(GrnNode::new(tag(0x0C05), 0, b""));
    nodes
}

/// Subtree of a synthetic section in pre-order: the node with `tag` and `data`, then the
/// subtrees of its children; the descendant count follows from them.
pub fn grn_tree(tag: u32, data: &[u8], children: Vec<Vec<GrnNode>>) -> Vec<GrnNode> {
    let descendants = children.iter().map(Vec::len).sum::<usize>();
    let mut nodes = vec![GrnNode::new(tag, u32::try_from(descendants).unwrap(), data)];
    nodes.extend(children.into_iter().flatten());
    nodes
}

/// Removes the subtree of node `at` and shrinks its ancestors.
pub fn remove_grn_subtree(nodes: &mut Vec<GrnNode>, at: usize) {
    let len = 1 + usize::try_from(nodes[at].descendants).unwrap();
    let removed = u32::try_from(len).unwrap();
    for (i, node) in nodes.iter_mut().enumerate().take(at) {
        if i + usize::try_from(node.descendants).unwrap() >= at {
            node.descendants -= removed;
        }
    }
    nodes.drain(at..at + len);
}

/// Nodes of an object section with the skeleton of [`skeleton_nodes`], the subtree `form_meshes`
/// added to its form and the subtrees `top` added to the top level.
pub fn model_nodes(
    bones: &[GrnBone],
    form_meshes: Vec<GrnNode>,
    top: Vec<GrnNode>,
) -> Vec<GrnNode> {
    let mut nodes = skeleton_nodes(bones);
    let form = nodes.iter().position(|n| n.tag == 0xCA5E_0C00).unwrap();
    let end = form + 1 + usize::try_from(nodes[form].descendants).unwrap();
    let added = u32::try_from(form_meshes.len()).unwrap();
    // The form list `0C01` is the form's parent.
    nodes[form - 1].descendants += added;
    nodes[form].descendants += added;
    nodes.splice(end..end, form_meshes);
    nodes.extend(top);
    nodes
}

/// Recomputes the section CRCs and the CRC of the section table, after a test changed the file.
pub fn fix_grn_crcs(data: &mut [u8]) {
    let u32_at = |data: &[u8], at: usize| {
        usize::try_from(u32::from_le_bytes(data[at..at + 4].try_into().unwrap())).unwrap()
    };
    let count = u32_at(data, 0x44);
    let starts: Vec<_> = (0..count).map(|i| u32_at(data, 0x68 + 20 * i)).collect();
    for (i, &start) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(data.len());
        let crc = crc32fast::hash(data.get(start..end).unwrap_or_default());
        data[0x6C + 20 * i..0x70 + 20 * i].copy_from_slice(&crc.to_le_bytes());
    }
    let crc = crc32fast::hash(&data[0x60..0x60 + 20 * count]);
    data[0x48..0x4C].copy_from_slice(&crc.to_le_bytes());
}

/// Repository root.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// File of the GOG installation in `OriginalGame/` (path cased as in the manifest).
pub fn game_file(path: &str) -> Vec<u8> {
    let path = repo_root().join("OriginalGame").join(path);
    fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Runs a Python prototype from `tools/` and returns its stdout. The interpreter is `$PYTHON`,
/// otherwise `python` (Windows) or `python3`; dependencies are in `tools/requirements.txt`.
pub fn run_python(script: &str, args: &[&OsStr]) -> String {
    let python = std::env::var("PYTHON")
        .unwrap_or_else(|_| if cfg!(windows) { "python" } else { "python3" }.to_owned());
    let root = repo_root();
    let output = Command::new(&python)
        .arg("-I")
        .arg(root.join(script))
        .args(args)
        .current_dir(&root)
        .output()
        .unwrap_or_else(|e| panic!("cannot run `{python}` (set PYTHON): {e}"));
    assert!(
        output.status.success(),
        "{script} failed: {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Empty temporary directory for prototype outputs.
pub fn scratch_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("opensacred-formats-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Decoded 8-bit PNG.
pub struct Png {
    pub width: u32,
    pub height: u32,
    pub color: png::ColorType,
    pub pixels: Vec<u8>,
}

pub fn read_png(path: &Path) -> Png {
    let file = fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(info.bit_depth, png::BitDepth::Eight, "{}", path.display());
    pixels.truncate(info.buffer_size());
    Png {
        width: info.width,
        height: info.height,
        color: info.color_type,
        pixels,
    }
}
