//! Test helpers.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// glTF `componentType` of `f32` accessors.
const FLOAT: u64 = 5126;

/// Repository root.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Empty temporary directory, unique per test process.
pub fn scratch_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("opensacred-convert-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Width, height and RGBA pixels of an 8-bit RGBA PNG held in memory.
pub fn read_png_bytes(data: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut reader = png::Decoder::new(std::io::Cursor::new(data))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(
        (info.color_type, info.bit_depth),
        (png::ColorType::Rgba, png::BitDepth::Eight)
    );
    pixels.truncate(info.buffer_size());
    (info.width, info.height, pixels)
}

/// Record of a synthetic TEX container.
pub struct Tex<'a> {
    pub name: &'a [u8],
    pub width: u16,
    pub height: u16,
    pub format: u8,
    pub data: Vec<u8>,
}

/// TEX v3 container (docs/02 §1, §3.1); `None` is an empty entry. The directory size
/// excludes the 80-byte record header.
pub fn tex_pak(records: &[Option<Tex>]) -> Vec<u8> {
    let mut data = vec![0; 0x100];
    data[..4].copy_from_slice(b"TEX\x03");
    data[4..8].copy_from_slice(&u32::try_from(records.len()).unwrap().to_le_bytes());
    let mut body = Vec::new();
    let first = 0x100 + 12 * records.len();
    for record in records {
        let entry = match record {
            Some(tex) => {
                let offset = u32::try_from(first + body.len()).unwrap();
                let size = u32::try_from(tex.data.len()).unwrap();
                let mut header = [0; 0x50];
                header[..tex.name.len()].copy_from_slice(tex.name);
                header[0x20..0x22].copy_from_slice(&tex.width.to_le_bytes());
                header[0x22..0x24].copy_from_slice(&tex.height.to_le_bytes());
                header[0x24] = tex.format;
                header[0x25..0x29].copy_from_slice(&size.to_le_bytes());
                body.extend_from_slice(&header);
                body.extend_from_slice(&tex.data);
                [u32::from(tex.format), offset, size]
            }
            None => [0; 3],
        };
        for value in entry {
            data.extend_from_slice(&value.to_le_bytes());
        }
    }
    data.extend_from_slice(&body);
    data
}

/// A GLB file split into its JSON and its binary chunk.
pub struct Glb {
    pub json: Value,
    pub bin: Vec<u8>,
}

impl Glb {
    pub fn read(data: &[u8]) -> Self {
        let word = |at: usize| {
            usize::try_from(u32::from_le_bytes(data[at..at + 4].try_into().unwrap())).unwrap()
        };
        assert_eq!(&data[..4], b"glTF");
        assert_eq!((word(4), word(8)), (2, data.len()));
        let json_len = word(12);
        assert_eq!(&data[16..20], b"JSON");
        let json = serde_json::from_slice(&data[20..20 + json_len]).unwrap();
        let bin_at = 20 + json_len;
        assert_eq!(&data[bin_at + 4..bin_at + 8], b"BIN\0");
        let bin = data[bin_at + 8..bin_at + 8 + word(bin_at)].to_vec();
        assert_eq!(bin_at + 8 + bin.len(), data.len());
        Self { json, bin }
    }

    pub fn field(&self, value: &Value) -> usize {
        usize::try_from(value.as_u64().unwrap()).unwrap()
    }

    /// Bytes of accessor `index` and its component type.
    pub fn bytes(&self, index: usize) -> (&[u8], u64) {
        let accessor = &self.json["accessors"][index];
        let view = &self.json["bufferViews"][self.field(&accessor["bufferView"])];
        let start = self.field(&view["byteOffset"]);
        let end = start + self.field(&view["byteLength"]);
        (
            &self.bin[start..end],
            accessor["componentType"].as_u64().unwrap(),
        )
    }

    pub fn floats(&self, index: &Value) -> Vec<f32> {
        let (bytes, kind) = self.bytes(self.field(index));
        assert_eq!(kind, FLOAT);
        bytes
            .chunks(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect()
    }

    pub fn ints(&self, index: &Value) -> Vec<u32> {
        match self.bytes(self.field(index)) {
            (bytes, 5123) => bytes
                .chunks(2)
                .map(|c| u32::from(u16::from_le_bytes(c.try_into().unwrap())))
                .collect(),
            (bytes, 5125) => bytes
                .chunks(4)
                .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                .collect(),
            (_, kind) => panic!("component type {kind}"),
        }
    }
}
