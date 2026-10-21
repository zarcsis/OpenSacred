//! A 32-bit PE image laid out as the Windows loader lays it out, as far as the oracle needs it:
//! the headers and the sections at their virtual addresses, the uninitialised rest of a section
//! zeros, and the imports by the slots of the import address table that the code calls through.
//! The image of `Sacred.exe` has a fixed base and no relocations, TLS or delay imports (docs/01).

use anyhow::{Context, Result, bail, ensure};

/// An image in memory.
pub struct Image {
    /// The address the image is laid out at (`ImageBase`).
    pub base: u32,
    /// The image's bytes from its base, `SizeOfImage` long.
    pub memory: Vec<u8>,
    pub sections: Vec<Section>,
    /// The imported functions, in the order of the import directory.
    pub imports: Vec<Import>,
}

/// A section: its name, its virtual address relative to the base and its size in memory.
pub struct Section {
    pub name: String,
    pub address: u32,
    pub size: u32,
}

/// An imported function: its DLL, its name (or `#` and its ordinal) and the virtual address,
/// relative to the base, of its slot of the import address table.
pub struct Import {
    pub dll: String,
    #[cfg_attr(
        all(not(feature = "emulator"), not(test)),
        expect(dead_code, reason = "the emulator's thunks take them")
    )]
    pub name: String,
    #[cfg_attr(
        all(not(feature = "emulator"), not(test)),
        expect(dead_code, reason = "the emulator's thunks take them")
    )]
    pub slot: u32,
}

/// The machine of a 32-bit x86 image.
const I386: u16 = 0x14C;
/// The magic of a PE32 optional header.
const PE32: u16 = 0x10B;
/// The index of the import directory among the data directories.
const IMPORT_DIRECTORY: usize = 1;

impl Image {
    /// Lays out the image of the file `bytes`.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.get(..2) == Some(b"MZ"),
            "not an executable: no MZ header"
        );
        let pe = usize::try_from(u32_at(bytes, 0x3C)?)?;
        ensure!(
            bytes.get(pe..pe + 4) == Some(b"PE\0\0"),
            "no PE header at {pe:#x}"
        );
        let coff = pe + 4;
        let machine = u16_at(bytes, coff)?;
        ensure!(
            machine == I386,
            "machine {machine:#x}: not a 32-bit x86 image"
        );
        let section_count = usize::from(u16_at(bytes, coff + 2)?);
        let optional_size = usize::from(u16_at(bytes, coff + 16)?);
        let optional = coff + 20;
        let magic = u16_at(bytes, optional)?;
        ensure!(magic == PE32, "optional header {magic:#x}: not PE32");
        let base = u32_at(bytes, optional + 28)?;
        let image_size = usize::try_from(u32_at(bytes, optional + 56)?)?;
        let headers_size = usize::try_from(u32_at(bytes, optional + 60)?)?;
        let directory_count = usize::try_from(u32_at(bytes, optional + 92)?)?;
        ensure!(
            headers_size <= image_size && headers_size <= bytes.len(),
            "headers of {headers_size:#x} bytes beyond the image or the file"
        );
        let mut memory = vec![0; image_size];
        memory[..headers_size].copy_from_slice(&bytes[..headers_size]);

        let table = optional + optional_size;
        let mut sections = Vec::with_capacity(section_count);
        for i in 0..section_count {
            let header = table + 40 * i;
            let raw_name = bytes
                .get(header..header + 8)
                .with_context(|| format!("section {i}: its header beyond the file"))?;
            let name = String::from_utf8_lossy(raw_name)
                .trim_end_matches('\0')
                .to_owned();
            let size = u32_at(bytes, header + 8)?;
            let address = u32_at(bytes, header + 12)?;
            let raw_size = u32_at(bytes, header + 16)?;
            let raw_offset = usize::try_from(u32_at(bytes, header + 20)?)?;
            // What the file holds of the section, at most its size in memory; the rest is zeros.
            let stored = usize::try_from(raw_size.min(size))?;
            let from = usize::try_from(address)?;
            let target = memory
                .get_mut(from..from + stored)
                .with_context(|| format!("section {name}: beyond the image's size"))?;
            let source = bytes
                .get(raw_offset..raw_offset + stored)
                .with_context(|| format!("section {name}: its data beyond the file"))?;
            target.copy_from_slice(source);
            sections.push(Section {
                name,
                address,
                size,
            });
        }

        let imports = if directory_count > IMPORT_DIRECTORY {
            let directory = optional + 96 + 8 * IMPORT_DIRECTORY;
            imports(&memory, u32_at(bytes, directory)?)?
        } else {
            Vec::new()
        };
        Ok(Self {
            base,
            memory,
            sections,
            imports,
        })
    }
}

/// The imports of the directory at `directory` (a virtual address relative to the base) of the
/// laid-out `memory`: its descriptors, each a DLL's name and its lookup and address tables, up to
/// the descriptor of zeros.
fn imports(memory: &[u8], directory: u32) -> Result<Vec<Import>> {
    let mut imports = Vec::new();
    if directory == 0 {
        return Ok(imports);
    }
    let mut descriptor = usize::try_from(directory)?;
    loop {
        let lookup = u32_at(memory, descriptor)?;
        let name = u32_at(memory, descriptor + 12)?;
        let slots = u32_at(memory, descriptor + 16)?;
        if lookup == 0 && name == 0 && slots == 0 {
            return Ok(imports);
        }
        let dll = c_string(memory, name).context("an import descriptor's DLL name")?;
        // The lookup table names the functions; an unbound address table holds the same.
        let mut entry = if lookup != 0 { lookup } else { slots };
        let mut slot = slots;
        loop {
            let value = u32_at(memory, usize::try_from(entry)?)?;
            if value == 0 {
                break;
            }
            let name = if value & 0x8000_0000 != 0 {
                format!("#{}", value & 0xFFFF)
            } else {
                c_string(memory, value + 2).with_context(|| format!("an import of {dll}"))?
            };
            imports.push(Import {
                dll: dll.clone(),
                name,
                slot,
            });
            entry += 4;
            slot += 4;
        }
        descriptor += 20;
    }
}

/// The zero-terminated string at `address` of `memory`.
fn c_string(memory: &[u8], address: u32) -> Result<String> {
    let from = usize::try_from(address)?;
    let rest = memory
        .get(from..)
        .with_context(|| format!("a string at {address:#x} beyond the image"))?;
    let Some(end) = rest.iter().position(|&b| b == 0) else {
        bail!("the string at {address:#x} has no end");
    };
    Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
}

fn u16_at(bytes: &[u8], at: usize) -> Result<u16> {
    let word = bytes
        .get(at..at + 2)
        .with_context(|| format!("a word at {at:#x} beyond the data"))?;
    Ok(u16::from_le_bytes([word[0], word[1]]))
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32> {
    let word = bytes
        .get(at..at + 4)
        .with_context(|| format!("a double word at {at:#x} beyond the data"))?;
    Ok(u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A small PE32 file: an image at 0x400000 of 0x3000 bytes, a `.text` section at 0x1000 of
    /// `code`, and an `.rdata` at 0x2000 that imports `KERNEL32.dll`'s `GetTickCount` and the
    /// ordinal 7 of `WS2_32.dll`, their slots at 0x2100 and 0x2108.
    pub(crate) fn synthetic(code: &[u8]) -> Vec<u8> {
        let mut file = vec![0_u8; 0x600];
        let put16 = |file: &mut Vec<u8>, at: usize, value: u16| {
            file[at..at + 2].copy_from_slice(&value.to_le_bytes());
        };
        let put32 = |file: &mut Vec<u8>, at: usize, value: u32| {
            file[at..at + 4].copy_from_slice(&value.to_le_bytes());
        };
        file[..2].copy_from_slice(b"MZ");
        put32(&mut file, 0x3C, 0x40);
        file[0x40..0x44].copy_from_slice(b"PE\0\0");
        let coff = 0x44;
        put16(&mut file, coff, I386);
        put16(&mut file, coff + 2, 2);
        put16(&mut file, coff + 16, 0xE0);
        let optional = coff + 20;
        put16(&mut file, optional, PE32);
        put32(&mut file, optional + 28, 0x40_0000);
        put32(&mut file, optional + 56, 0x3000);
        put32(&mut file, optional + 60, 0x200);
        put32(&mut file, optional + 92, 16);
        put32(&mut file, optional + 96 + 8, 0x2000);
        let table = optional + 0xE0;
        for (i, (name, address, offset)) in [(b".text", 0x1000, 0x200), (b".rdat", 0x2000, 0x400)]
            .into_iter()
            .enumerate()
        {
            let header = table + 40 * i;
            file[header..header + 5].copy_from_slice(name);
            put32(&mut file, header + 8, 0x1000);
            put32(&mut file, header + 12, address);
            put32(&mut file, header + 16, 0x200);
            put32(&mut file, header + 20, offset);
        }
        file[0x200..0x200 + code.len()].copy_from_slice(code);
        // .rdata at file 0x400, memory 0x2000: two descriptors and their tables and names.
        let rdata = |rva: usize| 0x400 + rva - 0x2000;
        put32(&mut file, rdata(0x2000), 0x2080); // lookup
        put32(&mut file, rdata(0x2000) + 12, 0x2040); // name
        put32(&mut file, rdata(0x2000) + 16, 0x2100); // slots
        put32(&mut file, rdata(0x2014), 0x2088);
        put32(&mut file, rdata(0x2014) + 12, 0x2050);
        put32(&mut file, rdata(0x2014) + 16, 0x2108);
        file[rdata(0x2040)..rdata(0x2040) + 12].copy_from_slice(b"KERNEL32.dll");
        file[rdata(0x2050)..rdata(0x2050) + 10].copy_from_slice(b"WS2_32.dll");
        put32(&mut file, rdata(0x2080), 0x20C0);
        put32(&mut file, rdata(0x2088), 0x8000_0007);
        file[rdata(0x20C2)..rdata(0x20C2) + 12].copy_from_slice(b"GetTickCount");
        put32(&mut file, rdata(0x2100), 0x20C0);
        put32(&mut file, rdata(0x2108), 0x8000_0007);
        file
    }

    #[test]
    fn an_image_is_laid_out_at_its_sections_addresses_with_its_imports() {
        let image = Image::parse(&synthetic(&[0x90, 0xC3])).unwrap();
        assert_eq!(image.base, 0x40_0000);
        assert_eq!(image.memory.len(), 0x3000);
        assert_eq!(&image.memory[0x1000..0x1002], &[0x90, 0xC3]);
        // The section's memory beyond its data is zeros.
        assert!(image.memory[0x1200..0x2000].iter().all(|&b| b == 0));
        let names: Vec<_> = image.sections.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, [".text", ".rdat"]);
        let imports: Vec<_> = image
            .imports
            .iter()
            .map(|i| (i.dll.as_str(), i.name.as_str(), i.slot))
            .collect();
        assert_eq!(
            imports,
            [
                ("KERNEL32.dll", "GetTickCount", 0x2100),
                ("WS2_32.dll", "#7", 0x2108)
            ]
        );
    }

    #[test]
    fn a_file_that_is_no_32_bit_image_is_refused() {
        assert!(Image::parse(b"ELF").is_err());
        let mut file = synthetic(&[]);
        file[0x44] = 0x64;
        file[0x45] = 0x86;
        let error = Image::parse(&file).err().unwrap().to_string();
        assert!(error.contains("not a 32-bit x86 image"), "{error}");
    }
}
