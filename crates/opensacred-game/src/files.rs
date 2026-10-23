//! The game's files (#699; PLAN.md ADR-05): the converter's `assets/` in layers. A path takes the
//! first layer that has it: the loose directory itself (debugging, HD packs and mods in the
//! making), then the overlay archives in its `overlays/` (HD packs, mods), then the base archives
//! beside the loose files, each set in the order of the archives' names. An archive is a ZIP of
//! stored and deflated entries whose paths are the loose files', so that Explorer and 7-Zip open it
//! and a modder can see and replace a file.
//!
//! Both readers go through the layers: Bevy's asset server through [`AssetFiles`], the default
//! asset source, and the game's own reads of tables, sectors and atlases through [`read`],
//! [`read_to_string`], [`open`], [`exists`] and [`read_dir`], which fall back on the file system
//! for a path outside the assets or before [`init`].

use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use bevy::asset::io::{
    AssetReader, AssetReaderError, PathStream, Reader, VecReader, file::FileAssetReader,
};
use bevy::tasks::futures_lite::stream;

/// The overlay archives' directory under the assets.
pub const OVERLAYS: &str = "overlays";

/// ZIP's signatures: the end of the central directory, a central directory's entry and a local
/// header.
const END: u32 = 0x0605_4B50;
const CENTRAL: u32 = 0x0201_4B50;
const LOCAL: u32 = 0x0403_4B50;
/// The compression methods the archives use.
const STORED: u16 = 0;
const DEFLATED: u16 = 8;

/// An archive's entry: its local header's offset, its sizes and whether it is deflated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    header: u64,
    compressed: u64,
    size: u64,
    deflated: bool,
}

/// An archive's central directory, its entries by their paths.
#[derive(Debug)]
struct Archive {
    path: PathBuf,
    entries: HashMap<String, Entry>,
}

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

impl Archive {
    /// Reads the central directory of the ZIP at `path`.
    fn open(path: &Path) -> io::Result<Self> {
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
            let compressed = u32_at(&directory, at + 20)?;
            let size = u32_at(&directory, at + 24)?;
            let name_length = usize::from(u16_at(&directory, at + 28)?);
            let extra = usize::from(u16_at(&directory, at + 30)?);
            let comment = usize::from(u16_at(&directory, at + 32)?);
            let header = u32_at(&directory, at + 42)?;
            let name_bytes = directory
                .get(at + 46..at + 46 + name_length)
                .ok_or_else(|| invalid("a ZIP entry's name cut short"))?;
            let name = String::from_utf8_lossy(name_bytes).replace('\\', "/");
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
                    deflated: method == DEFLATED,
                },
            );
        }
        Ok(Self {
            path: path.to_owned(),
            entries,
        })
    }

    /// The data of `entry`, inflated where it is deflated.
    fn read(&self, entry: Entry) -> io::Result<Vec<u8>> {
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(entry.header))?;
        let mut local = [0; 30];
        file.read_exact(&mut local)?;
        if u32_at(&local, 0)? != LOCAL {
            return Err(invalid("a ZIP entry without its local header"));
        }
        let skip = u64::from(u16_at(&local, 26)?) + u64::from(u16_at(&local, 28)?);
        file.seek(SeekFrom::Current(i64::try_from(skip).unwrap_or(0)))?;
        let mut data = vec![0; usize_of(entry.compressed)?];
        file.read_exact(&mut data)?;
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
    }
}

/// The assets' layers: the loose directory, then the overlay and the base archives.
#[derive(Debug)]
pub struct Layers {
    root: PathBuf,
    archives: Vec<Archive>,
}

/// The ZIP archives directly in `dir`, in the order of their names.
fn archives_in(dir: &Path) -> io::Result<Vec<Archive>> {
    let mut paths: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")))
            .collect(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    paths.sort();
    paths.iter().map(|p| Archive::open(p)).collect()
}

impl Layers {
    /// The layers of the assets at `root`.
    pub fn open(root: &Path) -> io::Result<Self> {
        let mut archives = archives_in(&root.join(OVERLAYS))?;
        archives.extend(archives_in(root)?);
        Ok(Self {
            root: root.to_owned(),
            archives,
        })
    }

    /// The archives' count.
    pub fn archives(&self) -> usize {
        self.archives.len()
    }

    /// `path` relative to the assets, as the archives name it, if it is in them.
    fn relative(&self, path: &Path) -> Option<String> {
        let relative = path.strip_prefix(&self.root).unwrap_or(path);
        if relative.is_absolute() {
            return None;
        }
        Some(relative.to_string_lossy().replace('\\', "/"))
    }

    /// The first archive's entry for the relative path `name`.
    fn find(&self, name: &str) -> Option<(&Archive, Entry)> {
        self.archives
            .iter()
            .find_map(|archive| archive.entries.get(name).map(|&e| (archive, e)))
    }

    /// The data of `name`, relative to the assets, from the archives.
    fn archived(&self, name: &str) -> Option<io::Result<Vec<u8>>> {
        self.find(name).map(|(archive, entry)| archive.read(entry))
    }

    /// The data of `path`: the loose file's, else an archive's.
    pub fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        if path.is_file() {
            return fs::read(path);
        }
        self.relative(path)
            .and_then(|name| self.archived(&name))
            .unwrap_or_else(|| Err(not_found(path)))
    }

    /// Whether `path` is a loose file or an archive's entry.
    pub fn exists(&self, path: &Path) -> bool {
        path.exists()
            || self
                .relative(path)
                .is_some_and(|name| self.find(&name).is_some())
    }

    /// The names directly in the directory `dir`, loose or archived, sorted.
    fn names_in(&self, dir: &Path) -> io::Result<BTreeSet<String>> {
        let mut names = BTreeSet::new();
        let mut found = false;
        if let Ok(entries) = fs::read_dir(dir) {
            found = true;
            for entry in entries.filter_map(Result::ok) {
                names.insert(entry.file_name().to_string_lossy().into_owned());
            }
        }
        if let Some(prefix) = self.relative(dir) {
            let prefix = if prefix.is_empty() {
                prefix
            } else {
                format!("{}/", prefix.trim_end_matches('/'))
            };
            for archive in &self.archives {
                for name in archive.entries.keys() {
                    if let Some(rest) = name.strip_prefix(&prefix) {
                        found = true;
                        let first = rest.split('/').next().unwrap_or(rest);
                        names.insert(first.to_owned());
                    }
                }
            }
        }
        if found {
            Ok(names)
        } else {
            Err(not_found(dir))
        }
    }
}

fn not_found(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("{}: no such file in the assets", path.display()),
    )
}

static LAYERS: OnceLock<Layers> = OnceLock::new();

/// Opens the assets' layers at `root`, once; the number of archives.
pub fn init(root: &Path) -> io::Result<usize> {
    let layers = Layers::open(root)?;
    let count = layers.archives();
    // A second call keeps the first layers.
    let _ = LAYERS.set(layers);
    Ok(count)
}

fn layers() -> Option<&'static Layers> {
    LAYERS.get()
}

/// The data of the asset file at `path`.
pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    match layers() {
        Some(layers) => layers.read(path.as_ref()),
        None => fs::read(path),
    }
}

/// The text of the asset file at `path`.
pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    String::from_utf8(read(path)?).map_err(|_| invalid("an asset file not in UTF-8"))
}

/// Whether the asset file at `path` exists, loose or archived.
pub fn exists(path: impl AsRef<Path>) -> bool {
    match layers() {
        Some(layers) => layers.exists(path.as_ref()),
        None => path.as_ref().exists(),
    }
}

/// The paths directly in the asset directory `dir`, loose or archived, sorted.
pub fn read_dir(dir: impl AsRef<Path>) -> io::Result<Vec<PathBuf>> {
    let dir = dir.as_ref();
    let names = match layers() {
        Some(layers) => layers.names_in(dir)?,
        None => fs::read_dir(dir)?
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect(),
    };
    Ok(names.into_iter().map(|name| dir.join(name)).collect())
}

/// An asset file opened for reading: the loose file itself, or an archived one in memory.
pub enum Opened {
    File(BufReader<File>),
    Memory(Cursor<Vec<u8>>),
}

impl Read for Opened {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Opened::File(file) => file.read(buf),
            Opened::Memory(memory) => memory.read(buf),
        }
    }
}

impl BufRead for Opened {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        match self {
            Opened::File(file) => file.fill_buf(),
            Opened::Memory(memory) => memory.fill_buf(),
        }
    }

    fn consume(&mut self, amount: usize) {
        match self {
            Opened::File(file) => file.consume(amount),
            Opened::Memory(memory) => memory.consume(amount),
        }
    }
}

impl Seek for Opened {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        match self {
            Opened::File(file) => file.seek(to),
            Opened::Memory(memory) => memory.seek(to),
        }
    }
}

/// Opens the asset file at `path` for reading, buffered.
pub fn open(path: impl AsRef<Path>) -> io::Result<Opened> {
    let path = path.as_ref();
    if path.is_file() || layers().is_none() {
        return Ok(Opened::File(BufReader::new(File::open(path)?)));
    }
    read(path).map(|data| Opened::Memory(Cursor::new(data)))
}

/// The default asset source of Bevy's asset server over the layers: a loose file first, through
/// Bevy's own file reader, then the archives.
pub struct AssetFiles {
    loose: FileAssetReader,
}

impl AssetFiles {
    /// The asset files under `root`, which [`init`] opened.
    pub fn new(root: &Path) -> Self {
        Self {
            loose: FileAssetReader::new(root),
        }
    }
}

/// An archived asset's data, by its path relative to the assets.
fn archived(path: &Path) -> Option<Vec<u8>> {
    let layers = layers()?;
    let name = path.to_string_lossy().replace('\\', "/");
    layers.archived(&name)?.ok()
}

impl AssetReader for AssetFiles {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<Box<dyn Reader + 'a>, AssetReaderError> {
        match self.loose.read(path).await {
            Ok(reader) => Ok(Box::new(reader)),
            Err(AssetReaderError::NotFound(missing)) => match archived(path) {
                Some(data) => Ok(Box::new(VecReader::new(data))),
                None => Err(AssetReaderError::NotFound(missing)),
            },
            Err(e) => Err(e),
        }
    }

    async fn read_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<dyn Reader + 'a>, AssetReaderError> {
        let reader = self.loose.read_meta(path).await?;
        Ok(Box::new(reader))
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        match self.loose.read_directory(path).await {
            Ok(entries) => Ok(entries),
            Err(AssetReaderError::NotFound(missing)) => {
                let names = layers()
                    .and_then(|layers| layers.names_in(&layers.root.join(path)).ok())
                    .ok_or(AssetReaderError::NotFound(missing))?;
                let paths: Vec<PathBuf> = names.into_iter().map(|n| path.join(n)).collect();
                Ok(Box::new(stream::iter(paths)))
            }
            Err(e) => Err(e),
        }
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        if self.loose.is_directory(path).await.unwrap_or(false) {
            return Ok(true);
        }
        Ok(layers().is_some_and(|layers| {
            let name = format!("{}/", path.to_string_lossy().replace('\\', "/"));
            layers
                .archives
                .iter()
                .any(|a| a.entries.keys().any(|k| k.starts_with(&name)))
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ZIP of `files`, the even ones stored and the odd ones deflated.
    fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (index, (name, data)) in files.iter().enumerate() {
            let deflated = index % 2 == 1;
            let stored = if deflated {
                miniz_oxide::deflate::compress_to_vec(data, 6)
            } else {
                data.to_vec()
            };
            let method: u16 = if deflated { DEFLATED } else { STORED };
            let header = u32::try_from(out.len()).unwrap();
            let name_length = u16::try_from(name.len()).unwrap();
            let compressed = u32::try_from(stored.len()).unwrap();
            let size = u32::try_from(data.len()).unwrap();
            out.extend_from_slice(&LOCAL.to_le_bytes());
            out.extend_from_slice(&[20, 0, 0, 0]);
            out.extend_from_slice(&method.to_le_bytes());
            out.extend_from_slice(&[0; 8]);
            out.extend_from_slice(&compressed.to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&name_length.to_le_bytes());
            out.extend_from_slice(&[0, 0]);
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&stored);
            central.extend_from_slice(&CENTRAL.to_le_bytes());
            central.extend_from_slice(&[20, 0, 20, 0, 0, 0]);
            central.extend_from_slice(&method.to_le_bytes());
            central.extend_from_slice(&[0; 8]);
            central.extend_from_slice(&compressed.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&name_length.to_le_bytes());
            central.extend_from_slice(&[0; 12]);
            central.extend_from_slice(&header.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
        }
        let offset = u32::try_from(out.len()).unwrap();
        let count = u16::try_from(files.len()).unwrap();
        out.extend_from_slice(&central);
        out.extend_from_slice(&END.to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&u32::try_from(central.len()).unwrap().to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("opensacred-files-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_path_takes_the_first_layer_that_has_it() {
        let root = scratch("layers");
        let text = b"( voices: [] )".repeat(20);
        fs::write(
            root.join("data.zip"),
            zip(&[
                ("tables/items.ron", b"[]"),
                ("tables/voices.ron", &text),
                ("tables/balance.ron", b"base"),
            ]),
        )
        .unwrap();
        fs::create_dir_all(root.join(OVERLAYS)).unwrap();
        fs::write(
            root.join(OVERLAYS).join("mod.zip"),
            zip(&[("tables/balance.ron", b"overlay")]),
        )
        .unwrap();
        fs::create_dir_all(root.join("tables")).unwrap();
        fs::write(root.join("tables/items.ron"), "loose").unwrap();
        let layers = Layers::open(&root).unwrap();
        assert_eq!(layers.archives(), 2);
        // The loose file before the archives, an overlay before the base, a deflated entry.
        assert_eq!(
            layers.read(&root.join("tables/items.ron")).unwrap(),
            b"loose"
        );
        assert_eq!(
            layers.read(&root.join("tables/balance.ron")).unwrap(),
            b"overlay"
        );
        assert_eq!(layers.read(&root.join("tables/voices.ron")).unwrap(), text);
        assert!(layers.exists(&root.join("tables/voices.ron")));
        assert!(!layers.exists(&root.join("tables/none.ron")));
        assert!(layers.read(&root.join("tables/none.ron")).is_err());
        let names: Vec<String> = layers
            .names_in(&root.join("tables"))
            .unwrap()
            .into_iter()
            .collect();
        assert_eq!(names, ["balance.ron", "items.ron", "voices.ron"]);
        fs::remove_dir_all(root).unwrap();
    }
}
