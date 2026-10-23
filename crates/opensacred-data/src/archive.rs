//! The game's asset archives (#699; PLAN.md ADR-05): ZIP files of stored and deflated entries
//! whose paths are the loose files' relative to `assets/`, `/` between the parts, so that Explorer
//! and 7-Zip open them and a modder can see and replace a file. The converter's `build` writes
//! them reproducibly: the entries in the order given, a fixed date, the data of a stored entry
//! aligned to [`ALIGNMENT`] bytes by a padding field of Android's `zipalign` (`0xD935`) and no
//! other extra field. Neither ZIP64 nor encryption: an archive stays below 4 GiB and 65,535
//! entries.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// The overlay archives' directory under the assets (HD packs, mods).
pub const OVERLAYS: &str = "overlays";
/// The archives' extension.
pub const EXTENSION: &str = "zip";
/// The alignment of a stored entry's data in its archive, so that it can be read from a memory map
/// without a copy.
pub const ALIGNMENT: u64 = 16;

/// ZIP's signatures: the end of the central directory, a central directory's entry and a local
/// header.
const END: u32 = 0x0605_4B50;
const CENTRAL: u32 = 0x0201_4B50;
const LOCAL: u32 = 0x0403_4B50;
/// The compression methods the archives use.
const STORED: u16 = 0;
const DEFLATED: u16 = 8;
/// ZIP 2.0, the version that deflate needs.
const VERSION: u16 = 20;
/// The general purpose flags: the names in UTF-8.
const UTF8: u16 = 0x0800;
/// 1980-01-01 00:00, the earliest MS-DOS date, for every entry.
const DATE: u16 = (1 << 5) | 1;
/// The extra field that pads a stored entry's data to its alignment (Android's `zipalign`).
const PADDING: u16 = 0xD935;
/// The deflate level of the deflated entries.
const LEVEL: u8 = 6;

fn u16_at(data: &[u8], at: usize) -> io::Result<u16> {
    data.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| invalid("a ZIP record cut short"))
}

fn u32_at(data: &[u8], at: usize) -> io::Result<u32> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| invalid("a ZIP record cut short"))
}

fn invalid(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.to_owned())
}

fn usize_of(value: u64) -> io::Result<usize> {
    usize::try_from(value).map_err(|_| invalid("a ZIP size beyond memory"))
}

/// An archive's entry as its central directory records it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    header: u64,
    compressed: u64,
    size: u64,
    crc: u32,
    deflated: bool,
}

/// An entry's data as the archive holds it, stored or deflated, to copy into another archive
/// without inflating and deflating it again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raw {
    data: Vec<u8>,
    size: u64,
    crc: u32,
    deflated: bool,
}

impl Raw {
    /// The size of the entry's data once inflated.
    pub fn size(&self) -> u64 {
        self.size
    }
}

/// A ZIP archive's central directory, its entries by their paths.
#[derive(Debug)]
pub struct Archive {
    path: PathBuf,
    entries: HashMap<String, Entry>,
}

impl Archive {
    /// Reads the central directory of the ZIP at `path`.
    pub fn open(path: &Path) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let length = file.metadata()?.len();
        // The end record, 22 bytes and a comment of up to 65,535, at the file's end.
        let tail = length.min(22 + 0xFFFF);
        file.seek(SeekFrom::Start(length - tail))?;
        let mut end = vec![0; usize_of(tail)?];
        file.read_exact(&mut end)?;
        let at = (0..end.len().saturating_sub(21))
            .rev()
            .find(|&at| u32_at(&end, at).is_ok_and(|s| s == END))
            .ok_or_else(|| invalid("no end of a ZIP's central directory"))?;
        let count = u16_at(&end, at + 10)?;
        let size = u32_at(&end, at + 12)?;
        let offset = u32_at(&end, at + 16)?;
        file.seek(SeekFrom::Start(u64::from(offset)))?;
        let mut directory = vec![0; usize_of(u64::from(size))?];
        file.read_exact(&mut directory)?;
        let mut entries = HashMap::with_capacity(usize::from(count));
        let mut at = 0;
        for _ in 0..count {
            if u32_at(&directory, at)? != CENTRAL {
                return Err(invalid(
                    "a ZIP central directory's entry without its signature",
                ));
            }
            let method = u16_at(&directory, at + 10)?;
            let crc = u32_at(&directory, at + 16)?;
            let compressed = u32_at(&directory, at + 20)?;
            let size = u32_at(&directory, at + 24)?;
            let name_length = usize::from(u16_at(&directory, at + 28)?);
            let extra = usize::from(u16_at(&directory, at + 30)?);
            let comment = usize::from(u16_at(&directory, at + 32)?);
            let header = u32_at(&directory, at + 42)?;
            let name = directory
                .get(at + 46..at + 46 + name_length)
                .ok_or_else(|| invalid("a ZIP entry's name cut short"))?;
            let name = String::from_utf8_lossy(name).replace('\\', "/");
            at += 46 + name_length + extra + comment;
            if name.ends_with('/') {
                continue;
            }
            if method != STORED && method != DEFLATED {
                return Err(invalid("a ZIP entry neither stored nor deflated"));
            }
            entries.insert(
                name,
                Entry {
                    header: u64::from(header),
                    compressed: u64::from(compressed),
                    size: u64::from(size),
                    crc,
                    deflated: method == DEFLATED,
                },
            );
        }
        Ok(Self {
            path: path.to_owned(),
            entries,
        })
    }

    /// The archive's file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The paths of the archive's files, in no order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// Whether the archive has the file `name`.
    pub fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    /// The size of the file `name` once inflated, if the archive has it.
    pub fn size(&self, name: &str) -> Option<u64> {
        self.entries.get(name).map(|e| e.size)
    }

    /// The data of `entry` as the archive holds it.
    fn compressed(&self, entry: Entry) -> io::Result<Vec<u8>> {
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(entry.header))?;
        let mut local = [0; 30];
        file.read_exact(&mut local)?;
        if u32_at(&local, 0)? != LOCAL {
            return Err(invalid("a ZIP entry without its local header"));
        }
        let skip = i64::from(u16_at(&local, 26)?) + i64::from(u16_at(&local, 28)?);
        file.seek(SeekFrom::Current(skip))?;
        let mut data = vec![0; usize_of(entry.compressed)?];
        file.read_exact(&mut data)?;
        Ok(data)
    }

    /// The data of the file `name`, inflated where it is deflated, if the archive has it.
    pub fn read(&self, name: &str) -> Option<io::Result<Vec<u8>>> {
        let entry = *self.entries.get(name)?;
        Some(self.compressed(entry).and_then(|data| {
            if !entry.deflated {
                return Ok(data);
            }
            let size = usize_of(entry.size)?;
            let inflated = miniz_oxide::inflate::decompress_to_vec_with_limit(&data, size)
                .map_err(|_| invalid("a ZIP entry that does not inflate"))?;
            if inflated.len() != size {
                return Err(invalid("a ZIP entry of another size than its record's"));
            }
            Ok(inflated)
        }))
    }

    /// The file `name` as the archive holds it, to copy into another, if the archive has it.
    pub fn raw(&self, name: &str) -> Option<io::Result<Raw>> {
        let entry = *self.entries.get(name)?;
        Some(self.compressed(entry).map(|data| Raw {
            data,
            size: entry.size,
            crc: entry.crc,
            deflated: entry.deflated,
        }))
    }
}

/// Writes an archive, an entry at a time in the order given.
pub struct Writer {
    file: BufWriter<File>,
    offset: u64,
    central: Vec<u8>,
    count: u16,
}

fn too_large() -> io::Error {
    invalid("an archive beyond ZIP's 4 GiB or 65,535 entries")
}

fn u32_of(value: u64) -> io::Result<u32> {
    u32::try_from(value).map_err(|_| too_large())
}

impl Writer {
    /// Creates the archive at `path`.
    pub fn create(path: &Path) -> io::Result<Self> {
        Ok(Self {
            file: BufWriter::new(File::create(path)?),
            offset: 0,
            central: Vec::new(),
            count: 0,
        })
    }

    /// Adds the file `name` with `data`, deflated where `deflate` and where that makes it smaller.
    pub fn add(&mut self, name: &str, data: &[u8], deflate: bool) -> io::Result<()> {
        let size = u64::try_from(data.len()).map_err(|_| too_large())?;
        let crc = crc32fast::hash(data);
        if deflate {
            let deflated = miniz_oxide::deflate::compress_to_vec(data, LEVEL);
            if deflated.len() < data.len() {
                return self.entry(name, &deflated, size, crc, true);
            }
        }
        self.entry(name, data, size, crc, false)
    }

    /// Adds the file `name` as another archive holds it.
    pub fn add_raw(&mut self, name: &str, raw: &Raw) -> io::Result<()> {
        self.entry(name, &raw.data, raw.size, raw.crc, raw.deflated)
    }

    fn entry(
        &mut self,
        name: &str,
        data: &[u8],
        size: u64,
        crc: u32,
        deflated: bool,
    ) -> io::Result<()> {
        let name_length = u16::try_from(name.len()).map_err(|_| invalid("a name too long"))?;
        let header = u32_of(self.offset)?;
        let compressed = u32_of(u64::try_from(data.len()).map_err(|_| too_large())?)?;
        let size = u32_of(size)?;
        let method = if deflated { DEFLATED } else { STORED };
        // A stored entry's data aligned: the padding field's 4 bytes of header, 2 of the alignment
        // and the zeros up to it.
        let extra: Vec<u8> = if deflated {
            Vec::new()
        } else {
            let unpadded = self.offset + 30 + u64::from(name_length) + 6;
            let padding = (ALIGNMENT - unpadded % ALIGNMENT) % ALIGNMENT;
            let length = u16::try_from(2 + padding).map_err(|_| too_large())?;
            let alignment = u16::try_from(ALIGNMENT).map_err(|_| too_large())?;
            let mut extra = Vec::with_capacity(usize::from(length) + 4);
            extra.extend_from_slice(&PADDING.to_le_bytes());
            extra.extend_from_slice(&length.to_le_bytes());
            extra.extend_from_slice(&alignment.to_le_bytes());
            extra.resize(usize::from(length) + 4, 0);
            extra
        };
        let extra_length = u16::try_from(extra.len()).map_err(|_| too_large())?;
        // The fields that the local header and the central directory's entry share.
        let mut common = Vec::with_capacity(26);
        common.extend_from_slice(&VERSION.to_le_bytes());
        common.extend_from_slice(&UTF8.to_le_bytes());
        common.extend_from_slice(&method.to_le_bytes());
        common.extend_from_slice(&0u16.to_le_bytes());
        common.extend_from_slice(&DATE.to_le_bytes());
        common.extend_from_slice(&crc.to_le_bytes());
        common.extend_from_slice(&compressed.to_le_bytes());
        common.extend_from_slice(&size.to_le_bytes());
        common.extend_from_slice(&name_length.to_le_bytes());
        self.file.write_all(&LOCAL.to_le_bytes())?;
        self.file.write_all(&common)?;
        self.file.write_all(&extra_length.to_le_bytes())?;
        self.file.write_all(name.as_bytes())?;
        self.file.write_all(&extra)?;
        self.file.write_all(data)?;
        self.offset +=
            30 + u64::from(name_length) + u64::from(extra_length) + u64::from(compressed);
        self.central.extend_from_slice(&CENTRAL.to_le_bytes());
        // Made by ZIP 2.0 on MS-DOS, whose attributes are none here.
        self.central.extend_from_slice(&VERSION.to_le_bytes());
        self.central.extend_from_slice(&common);
        // No extra field, comment, disk or attributes.
        self.central.extend_from_slice(&[0; 12]);
        self.central.extend_from_slice(&header.to_le_bytes());
        self.central.extend_from_slice(name.as_bytes());
        self.count = self.count.checked_add(1).ok_or_else(too_large)?;
        Ok(())
    }

    /// Writes the central directory and closes the archive.
    pub fn finish(mut self) -> io::Result<()> {
        let size = u32_of(u64::try_from(self.central.len()).map_err(|_| too_large())?)?;
        let offset = u32_of(self.offset)?;
        u32_of(self.offset + u64::from(size) + 22)?;
        self.file.write_all(&self.central)?;
        self.file.write_all(&END.to_le_bytes())?;
        self.file.write_all(&[0; 4])?;
        self.file.write_all(&self.count.to_le_bytes())?;
        self.file.write_all(&self.count.to_le_bytes())?;
        self.file.write_all(&size.to_le_bytes())?;
        self.file.write_all(&offset.to_le_bytes())?;
        self.file.write_all(&[0, 0])?;
        self.file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("opensacred-archive-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_archive_reads_back_what_was_written_and_copies_it_byte_for_byte() {
        let dir = scratch("roundtrip");
        let text = b"( voices: [] )".repeat(40);
        let image = [0x89, b'P', b'N', b'G', 1, 2, 3];
        let write = |path: &Path| {
            let mut writer = Writer::create(path).unwrap();
            writer.add("tables/voices.ron", &text, true).unwrap();
            writer.add("textures/a.png", &image, false).unwrap();
            // Deflate that does not shrink it stores it.
            writer.add("tables/tiny.ron", b"[]", true).unwrap();
            writer.finish().unwrap();
        };
        write(&dir.join("a.zip"));
        write(&dir.join("b.zip"));
        let a = std::fs::read(dir.join("a.zip")).unwrap();
        assert_eq!(a, std::fs::read(dir.join("b.zip")).unwrap());
        let archive = Archive::open(&dir.join("a.zip")).unwrap();
        let mut names: Vec<&str> = archive.names().collect();
        names.sort_unstable();
        assert_eq!(
            names,
            ["tables/tiny.ron", "tables/voices.ron", "textures/a.png"]
        );
        assert_eq!(archive.read("tables/voices.ron").unwrap().unwrap(), text);
        assert_eq!(archive.read("textures/a.png").unwrap().unwrap(), image);
        assert_eq!(archive.read("tables/tiny.ron").unwrap().unwrap(), b"[]");
        assert!(archive.read("tables/none.ron").is_none());
        assert!(archive.entries["tables/voices.ron"].deflated);
        assert!(!archive.entries["tables/tiny.ron"].deflated);
        // A stored entry's data starts aligned.
        let header = usize::try_from(archive.entries["textures/a.png"].header).unwrap();
        let skip = usize::from(u16_at(&a, header + 26).unwrap() + u16_at(&a, header + 28).unwrap());
        assert_eq!(u64::try_from(header + 30 + skip).unwrap() % ALIGNMENT, 0);
        // The raw entries copied make the same archive.
        let mut copy = Writer::create(&dir.join("c.zip")).unwrap();
        for name in ["tables/voices.ron", "textures/a.png", "tables/tiny.ron"] {
            copy.add_raw(name, &archive.raw(name).unwrap().unwrap())
                .unwrap();
        }
        copy.finish().unwrap();
        assert_eq!(std::fs::read(dir.join("c.zip")).unwrap(), a);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
