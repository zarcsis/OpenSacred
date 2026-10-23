//! Where a virtual address of a 32-bit PE image lies in its file: the section table of
//! `Sacred.exe`, as far as reading a table compiled into it needs (docs/02 §6.5). The image of
//! GOG 2.28 has its base at `0x400000` and its sections stored at their virtual addresses
//! (docs/01).

use crate::error::{ErrorKind, Result, field, to_usize};
use crate::reader::Reader;

/// The magic of a PE32 optional header.
const PE32: u16 = 0x10B;
const SECTION_HEADER: usize = 40;

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
        })
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A PE32 file with its image at `0x400000` and two sections: `.text` at `0x1000` of
    /// `0x200` bytes stored at `0x200`, and `.data` at the virtual address `address`, whose file
    /// part is `data` from `0x400` and which memory holds `0x1000` bytes further.
    pub(crate) fn image(address: u32, data: &[u8]) -> Vec<u8> {
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
        put16(&mut file, 0x54, 0xE0);
        put16(&mut file, 0x58, PE32);
        put32(&mut file, 0x58 + 28, 0x40_0000);
        let table = 0x58 + 0xE0;
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
