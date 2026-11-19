//! Where a virtual address of a 32-bit PE image lies in its file: the section table of
//! `Sacred.exe`, as far as reading a table compiled into it needs (docs/02 §6.5). The image of
//! GOG 2.28 has its base at `0x400000` and its sections stored at their virtual addresses
//! (docs/01). Its resources too, by their type and number, as `LoadImageA` finds a bitmap
//! (docs/02 §11, #2891).

use crate::error::{ErrorKind, Result, field, to_usize};
use crate::reader::Reader;

/// The magic of a PE32 optional header.
const PE32: u16 = 0x10B;
const SECTION_HEADER: usize = 40;
/// Where a PE32 optional header holds the resources' directory: the third entry of the data
/// directories, which begin 96 bytes in, each an address and a size.
const RESOURCES_ENTRY: usize = 96 + 2 * 8;
/// The resources' type of the bitmaps (`RT_BITMAP`).
pub const BITMAP: u32 = 2;
/// A resources' directory's header, and one of its entries.
const DIRECTORY: usize = 16;
const DIRECTORY_ENTRY: usize = 8;
/// The high bit of an entry's second word: the offset of a directory of the next level, rather
/// than of a data entry.
const SUBDIRECTORY: u32 = 0x8000_0000;

/// A section: its virtual address relative to the image base, its size in memory, and the bytes
/// of it that the file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Section {
    address: u32,
    size: u32,
    raw_offset: u32,
    raw_size: u32,
}

/// The sections of a PE image held in memory as its file.
#[derive(Debug, Clone)]
pub struct Image<'a> {
    data: &'a [u8],
    /// The address the image is laid out at (`ImageBase`).
    base: u32,
    sections: Vec<Section>,
    /// The resources' directory: its address relative to the base and its size, none where the
    /// optional header has no such entry or it is empty.
    resources: Option<(u32, u32)>,
}

impl<'a> Image<'a> {
    /// Reads the headers and the section table of the file `data`.
    pub fn parse(data: &'a [u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        if r.array::<2>("MZ header")? != *b"MZ" {
            return Err(r.error_at(0, ErrorKind::Signature { what: "MZ header" }));
        }
        let pe = to_usize(Reader::at(data, 0x3C).u32()?);
        let mut r = Reader::at(data, pe);
        if r.array::<4>("PE header")? != *b"PE\0\0" {
            return Err(r.error_at(pe, ErrorKind::Signature { what: "PE header" }));
        }
        let _machine = r.u16()?;
        let count = usize::from(r.u16()?);
        // The time stamp, the symbol table and its count.
        r.bytes(12, "COFF header")?;
        let optional_size = usize::from(r.u16()?);
        let _characteristics = r.u16()?;
        let optional = r.offset();
        let magic = r.u16()?;
        if magic != PE32 {
            return Err(r.error_at(
                optional,
                field(
                    "optional header magic",
                    magic.into(),
                    format!("{PE32:#x}, PE32"),
                ),
            ));
        }
        let base = Reader::at(data, optional + 28).u32()?;
        let resources = if optional_size >= RESOURCES_ENTRY + 8 {
            let mut entry = Reader::at(data, optional + RESOURCES_ENTRY);
            Some((entry.u32()?, entry.u32()?)).filter(|&(_, size)| size > 0)
        } else {
            None
        };
        let mut r = Reader::at(data, optional + optional_size);
        let sections = (0..count)
            .map(|_| {
                let mut header = r.take(SECTION_HEADER, "section header")?;
                let _name = header.bytes(8, "section name")?;
                Ok(Section {
                    size: header.u32()?,
                    address: header.u32()?,
                    raw_size: header.u32()?,
                    raw_offset: header.u32()?,
                })
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            data,
            base,
            sections,
            resources,
        })
    }

    /// The bytes of the resource of the type `kind` and the number `number`, in its first
    /// language: the directory of the types, then of the numbers, then of the languages, each
    /// entry's offset from the directory's start, and the data entry's address and size.
    pub fn resource(&self, kind: u32, number: u32) -> Result<&'a [u8]> {
        let none = || {
            Reader::new(self.data).error_at(
                0,
                field(
                    "resource",
                    u64::from(number),
                    format!("a resource of the type {kind}"),
                ),
            )
        };
        let (address, size) = self.resources.ok_or_else(none)?;
        let va = self.base.checked_add(address).ok_or_else(none)?;
        let directory = self.at(va, to_usize(size), "resources")?;
        let types = find(&directory, 0, Some(kind))?.ok_or_else(none)?;
        let numbers = find(
            &directory,
            subdirectory(types).ok_or_else(none)?,
            Some(number),
        )?
        .ok_or_else(none)?;
        let language =
            find(&directory, subdirectory(numbers).ok_or_else(none)?, None)?.ok_or_else(none)?;
        if subdirectory(language).is_some() {
            return Err(none());
        }
        let mut data = directory.window(to_usize(language), 8, "resource data entry")?;
        let (at, len) = (data.u32()?, data.u32()?);
        let va = self.base.checked_add(at).ok_or_else(none)?;
        Ok(self.at(va, to_usize(len), "resource")?.rest())
    }

    /// A reader over the `len` bytes at the virtual address `va`, which must all lie in the file
    /// part of one section; its offsets, and so its errors', are the file's.
    pub fn at(&self, va: u32, len: usize, what: &'static str) -> Result<Reader<'a>> {
        let not_stored = || {
            Reader::new(self.data).error_at(
                0,
                field(what, va.into(), "a virtual address the file stores".into()),
            )
        };
        let rva = va.checked_sub(self.base).ok_or_else(not_stored)?;
        let section = self
            .sections
            .iter()
            .find(|s| rva >= s.address && rva - s.address < s.size.max(s.raw_size))
            .ok_or_else(not_stored)?;
        let from = to_usize(rva - section.address);
        let stored = to_usize(section.raw_size.min(section.size));
        if from.checked_add(len).is_none_or(|end| end > stored) {
            return Err(not_stored());
        }
        Reader::at(self.data, to_usize(section.raw_offset).saturating_add(from)).take(len, what)
    }
}

/// The second word of the entry of the directory at `offset` of `directory` whose number is
/// `number`, or of its first entry for none; none where no entry has it.
fn find(directory: &Reader<'_>, offset: u32, number: Option<u32>) -> Result<Option<u32>> {
    let mut header = directory.window(to_usize(offset), DIRECTORY, "resource directory")?;
    header.bytes(12, "resource directory")?;
    let named = usize::from(header.u16()?);
    let numbered = usize::from(header.u16()?);
    let mut entries = directory.window(
        to_usize(offset) + DIRECTORY,
        (named + numbered) * DIRECTORY_ENTRY,
        "resource directory entries",
    )?;
    for i in 0..named + numbered {
        let (name, data) = (entries.u32()?, entries.u32()?);
        // The named entries come first; a number is searched among the numbered ones alone.
        if number.is_none_or(|number| i >= named && name == number) {
            return Ok(Some(data));
        }
    }
    Ok(None)
}

/// The offset of the directory of the next level that an entry's second word names.
fn subdirectory(data: u32) -> Option<u32> {
    (data & SUBDIRECTORY != 0).then_some(data & !SUBDIRECTORY)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A PE32 file with its image at `0x400000` and two sections: `.text` at `0x1000` of
    /// `0x200` bytes stored at `0x200`, and `.data` at the virtual address `address`, whose file
    /// part is `data` from `0x400` and which memory holds `0x1000` bytes further.
    pub(crate) fn image(address: u32, data: &[u8]) -> Vec<u8> {
        sized_image(0xE0, address, data)
    }

    /// [`image`] with an optional header of `optional` bytes, its section table after it.
    fn sized_image(optional: u16, address: u32, data: &[u8]) -> Vec<u8> {
        let mut file = vec![0_u8; 0x400];
        let put16 = |file: &mut Vec<u8>, at: usize, value: u16| {
            file[at..at + 2].copy_from_slice(&value.to_le_bytes());
        };
        let put32 = |file: &mut Vec<u8>, at: usize, value: u32| {
            file[at..at + 4].copy_from_slice(&value.to_le_bytes());
        };
        file[..2].copy_from_slice(b"MZ");
        put32(&mut file, 0x3C, 0x40);
        file[0x40..0x44].copy_from_slice(b"PE\0\0");
        put16(&mut file, 0x44, 0x14C);
        put16(&mut file, 0x46, 2);
        put16(&mut file, 0x54, optional);
        put16(&mut file, 0x58, PE32);
        put32(&mut file, 0x58 + 28, 0x40_0000);
        let table = 0x58 + usize::from(optional);
        let stored = u32::try_from(data.len()).unwrap();
        for (i, (name, size, rva, raw_size, offset)) in [
            (b".text", 0x200, 0x1000, 0x200, 0x200),
            (
                b".data",
                stored + 0x1000,
                address - 0x40_0000,
                stored,
                0x400,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let header = table + SECTION_HEADER * i;
            file[header..header + 5].copy_from_slice(name);
            put32(&mut file, header + 8, size);
            put32(&mut file, header + 12, rva);
            put32(&mut file, header + 16, raw_size);
            put32(&mut file, header + 20, offset);
        }
        file.extend_from_slice(data);
        file
    }

    #[test]
    fn a_virtual_address_reads_from_its_sections_place_in_the_file() {
        let file = image(0x40_2000, &[[1, 2, 3, 4].as_slice(), &[0; 0x1FC]].concat());
        let image = Image::parse(&file).unwrap();
        let mut r = image.at(0x40_2001, 2, "table").unwrap();
        assert_eq!(r.offset(), 0x401);
        assert_eq!(r.u16().unwrap(), 0x0302);
        assert!(r.u8().is_err(), "the window ends with its length");
        assert_eq!(image.at(0x40_21FC, 4, "table").unwrap().offset(), 0x5FC);
    }

    #[test]
    fn an_address_the_file_does_not_store_is_refused() {
        let file = image(0x40_2000, &[0; 0x200]);
        let image = Image::parse(&file).unwrap();
        // Below the base, in no section, and in the part of `.data` that only memory holds.
        for (va, len) in [
            (0x3F_0000, 4),
            (0x40_5000, 4),
            (0x40_2300, 4),
            (0x40_21FE, 4),
        ] {
            let error = image.at(va, len, "table").unwrap_err().to_string();
            assert!(
                error.contains("a virtual address the file stores"),
                "{error}"
            );
        }
    }

    /// A resources' section at `0x2000`: the bitmap 125 holds 1 to 4, and a named entry before
    /// it, whose name's word is 125 too, holds 9.
    fn resources(optional: u16) -> Vec<u8> {
        let mut section = Vec::new();
        let directory = |section: &mut Vec<u8>, named: u16, entries: &[(u32, u32)]| {
            section.extend_from_slice(&[0; 12]);
            section.extend_from_slice(&named.to_le_bytes());
            let numbered = u16::try_from(entries.len()).unwrap() - named;
            section.extend_from_slice(&numbered.to_le_bytes());
            for &(name, data) in entries {
                section.extend_from_slice(&name.to_le_bytes());
                section.extend_from_slice(&data.to_le_bytes());
            }
        };
        // The types at 0, the numbers at 24, two languages' directories at 56 and 80, the data
        // entries at 104 and 120, the bytes at 136 and 140.
        directory(&mut section, 0, &[(BITMAP, SUBDIRECTORY | 24)]);
        directory(
            &mut section,
            1,
            &[(125, SUBDIRECTORY | 80), (125, SUBDIRECTORY | 56)],
        );
        directory(&mut section, 0, &[(1031, 104)]);
        directory(&mut section, 0, &[(1031, 120)]);
        for (at, len) in [(136_u32, 4_u32), (140, 1)] {
            section.extend_from_slice(&(0x2000_u32 + at).to_le_bytes());
            section.extend_from_slice(&len.to_le_bytes());
            section.extend_from_slice(&[0; 8]);
        }
        section.extend_from_slice(&[1, 2, 3, 4, 9]);
        section.resize(0x100, 0);
        let mut file = sized_image(optional, 0x40_2000, &section);
        let entry = 0x58 + RESOURCES_ENTRY;
        if usize::from(optional) >= RESOURCES_ENTRY + 8 {
            file[entry..entry + 4].copy_from_slice(&0x2000_u32.to_le_bytes());
            file[entry + 4..entry + 8].copy_from_slice(&0x100_u32.to_le_bytes());
        }
        file
    }

    #[test]
    fn a_resource_is_found_by_its_type_and_number() {
        let file = resources(0xE0);
        let parsed = Image::parse(&file).unwrap();
        assert_eq!(parsed.resource(BITMAP, 125).unwrap(), [1, 2, 3, 4]);
        // No other number, nor another type.
        for (kind, number) in [(BITMAP, 124), (3, 125)] {
            let error = parsed.resource(kind, number).unwrap_err().to_string();
            assert!(error.contains("a resource of the type"), "{error}");
        }
        // An optional header just long enough for the entry, and one too short.
        assert!(
            Image::parse(&resources(120))
                .unwrap()
                .resource(BITMAP, 125)
                .is_ok()
        );
        assert!(
            Image::parse(&resources(119))
                .unwrap()
                .resource(BITMAP, 125)
                .is_err()
        );
        // An entry of no size, and an image of no resources.
        let mut file = resources(0xE0);
        let size = 0x58 + RESOURCES_ENTRY + 4;
        file[size..size + 4].copy_from_slice(&[0; 4]);
        assert!(Image::parse(&file).unwrap().resource(BITMAP, 125).is_err());
        let file = image(0x40_2000, &[0; 0x100]);
        assert!(Image::parse(&file).unwrap().resource(BITMAP, 125).is_err());
    }

    #[test]
    fn a_resource_is_refused_where_its_levels_are_not_directories() {
        // The type's entry naming a data entry, the number's too, and the language's a directory.
        for (at, word) in [(20, 24), (52, 56), (76, SUBDIRECTORY | 104)] {
            let mut file = resources(0xE0);
            let at = 0x400 + at;
            file[at..at + 4].copy_from_slice(&u32::to_le_bytes(word));
            let error = Image::parse(&file)
                .unwrap()
                .resource(BITMAP, 125)
                .unwrap_err()
                .to_string();
            assert!(error.contains("a resource of the type"), "{error}");
        }
    }

    #[test]
    fn a_file_that_is_no_pe32_image_is_refused() {
        assert!(Image::parse(b"ELF").is_err());
        let mut file = image(0x40_2000, &[]);
        file[0x40] = b'N';
        assert!(Image::parse(&file).is_err());
        let mut file = image(0x40_2000, &[]);
        file[0x58] = 0x0B;
        file[0x59] = 0x02;
        let error = Image::parse(&file).unwrap_err().to_string();
        assert!(error.contains("PE32"), "{error}");
    }
}
