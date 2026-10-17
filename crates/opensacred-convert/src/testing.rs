//! Test helpers.

use std::fs;
use std::path::{Path, PathBuf};

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
