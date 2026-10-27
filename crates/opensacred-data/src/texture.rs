//! The game's textures (docs/02 §12.3, #700): KTX2 files (Khronos KTX 2.0), each level
//! supercompressed by zstd, of one of two encodings. [`Encoding::Bc7`] is the GPU's block
//! compression, a byte a texel, for the opaque images that the GPU alone samples;
//! [`Encoding::Rgba8`] keeps the texels as they are, for the images that the game composes or reads
//! on the CPU and those with a texel of alpha under 255, whose alpha the game reads
//! (docs/decisions/2026-10-23-textures.md). The converter's `build` writes them ([`write`]); the
//! game hands them to Bevy's loader, or reads the 8-bit ones itself ([`read`]).

use std::io;

/// The textures' extension.
pub const EXTENSION: &str = "ktx2";

/// The slots of `texture.pak`, from which the client numbers the textures of `texture03.pak`, the
/// overlay's, in one numbering with `texture.pak`'s (`0x429190`; docs/02 §6.1; #1152): its record
/// `n` is the texture `TEXTURE03_BASE + n`, as `Items03.pak`'s records name it in `items.ron`.
pub const TEXTURE03_BASE: u32 = 25_535;
/// The texture (by its name in `texture.pak`) that the game reads on the CPU besides those the
/// world names: the cursor's.
pub const CURSOR: &str = "mouse_main";

/// KTX2's identifier, the file's first 12 bytes.
const IDENTIFIER: [u8; 12] = [
    0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
];
/// The header and the index of the sections, up to the levels' index.
pub const HEADER: usize = 80;
/// A level's entry in the levels' index: its offset, its length and its length uncompressed.
const LEVEL_ENTRY: usize = 24;
/// `VK_FORMAT_R8G8B8A8_SRGB` and `VK_FORMAT_BC7_SRGB_BLOCK`.
const RGBA8_SRGB: u32 = 43;
const BC7_SRGB: u32 = 146;
/// The supercompression scheme Zstandard.
const ZSTANDARD: u32 = 2;
/// The zstd level of every level's data.
const ZSTD_LEVEL: i32 = 19;

/// How a texture's texels are stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// 8-bit sRGB red, green and blue and linear alpha, not premultiplied.
    Rgba8,
    /// BC7 in sRGB, 16 bytes a block of 4 × 4 texels.
    Bc7,
}

/// The basic data format descriptor's samples (Khronos Data Format 1.3): each its bit offset, its
/// bit length less one, its channel with its qualifiers, and its upper value.
type Sample = (u32, u32, u32, u32);

impl Encoding {
    fn vk_format(self) -> u32 {
        match self {
            Self::Rgba8 => RGBA8_SRGB,
            Self::Bc7 => BC7_SRGB,
        }
    }

    /// The bytes of a level of `width` × `height` texels.
    pub fn level_size(self, width: u32, height: u32) -> u64 {
        let (width, height) = (u64::from(width.max(1)), u64::from(height.max(1)));
        match self {
            Self::Rgba8 => 4 * width * height,
            Self::Bc7 => 16 * width.div_ceil(4) * height.div_ceil(4),
        }
    }

    /// The colour model, the texel block's dimensions less one, the bytes of a block, and the
    /// samples.
    fn layout(self) -> (u32, u32, u32, &'static [Sample]) {
        match self {
            // KHR_DF_MODEL_RGBSDA: red, green, blue, and alpha (15) qualified linear (0x10).
            Self::Rgba8 => (
                1,
                0,
                4,
                &[
                    (0, 7, 0, 255),
                    (8, 7, 1, 255),
                    (16, 7, 2, 255),
                    (24, 7, 0x1F, 255),
                ],
            ),
            // KHR_DF_MODEL_BC7: one sample of the whole block's 128 bits.
            Self::Bc7 => (134, 3 | (3 << 8), 16, &[(0, 127, 0, u32::MAX)]),
        }
    }

    /// The data format descriptor, its total size first: one basic block of sRGB colours of
    /// BT.709's primaries, the alpha straight.
    fn descriptor(self) -> Vec<u8> {
        let (model, block, bytes, samples) = self.layout();
        let mut sample_words = Vec::new();
        for &(offset, length, channel, upper) in samples {
            sample_words.extend([offset | (length << 16) | (channel << 24), 0, 0, upper]);
        }
        let mut words = vec![
            // Vendor Khronos, the basic descriptor.
            0,
            // Version 1.3 (2), the block's size in the high half.
            2,
            // Primaries BT.709 (1), transfer sRGB (2), flags 0 (alpha straight).
            model | (1 << 8) | (2 << 16),
            block,
            bytes,
            0,
        ];
        words.extend(sample_words);
        let block_size: u32 = (4 * words.len()).try_into().unwrap_or(u32::MAX);
        words[1] |= block_size << 16;
        std::iter::once(4 + block_size)
            .chain(words)
            .flat_map(u32::to_le_bytes)
            .collect()
    }
}

fn invalid(what: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what)
}

fn to_u64(n: usize) -> io::Result<u64> {
    u64::try_from(n).map_err(|_| invalid(format!("{n} bytes")))
}

/// A KTX2 file of `levels`, the largest first, of a texture `width` × `height` texels encoded as
/// `encoding`; each level halves the one before, to a side of 1.
pub fn write(
    encoding: Encoding,
    width: u32,
    height: u32,
    levels: &[Vec<u8>],
) -> io::Result<Vec<u8>> {
    if levels.is_empty() || width == 0 || height == 0 {
        return Err(invalid(format!(
            "a texture of {width} x {height} texels and {} levels",
            levels.len()
        )));
    }
    for (index, level) in levels.iter().enumerate() {
        let shift = u32::try_from(index).unwrap_or(u32::MAX);
        let side = |n: u32| n.checked_shr(shift).unwrap_or(0);
        if side(width.max(height)) == 0
            || to_u64(level.len())? != encoding.level_size(side(width), side(height))
        {
            return Err(invalid(format!(
                "level {index} of a texture of {width} x {height} texels holds {} bytes",
                level.len()
            )));
        }
    }
    let packed: Vec<Vec<u8>> = levels
        .iter()
        .map(|level| zstd::bulk::compress(level, ZSTD_LEVEL))
        .collect::<io::Result<_>>()?;
    let descriptor = encoding.descriptor();
    let count = levels.len();
    let descriptor_at = HEADER + LEVEL_ENTRY * count;
    // The levels' data from the smallest to the largest, unaligned as a supercompressed file's.
    let mut offsets = vec![0; count];
    let mut at = descriptor_at + descriptor.len();
    for index in (0..count).rev() {
        offsets[index] = at;
        at += packed[index].len();
    }
    let word = |n: usize| {
        u32::try_from(n)
            .map(u32::to_le_bytes)
            .map_err(|_| invalid(format!("a texture of {n} bytes")))
    };
    let mut out = Vec::with_capacity(at);
    out.extend_from_slice(&IDENTIFIER);
    for value in [encoding.vk_format(), 1, width, height, 0, 0, 1] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out.extend_from_slice(&word(count)?);
    out.extend_from_slice(&ZSTANDARD.to_le_bytes());
    out.extend_from_slice(&word(descriptor_at)?);
    out.extend_from_slice(&word(descriptor.len())?);
    // No key and value data, no supercompression's global data.
    out.extend_from_slice(&[0; 24]);
    for ((&offset, data), level) in offsets.iter().zip(&packed).zip(levels) {
        for n in [offset, data.len(), level.len()] {
            out.extend_from_slice(&to_u64(n)?.to_le_bytes());
        }
    }
    out.extend_from_slice(&descriptor);
    for data in packed.iter().rev() {
        out.extend_from_slice(data);
    }
    Ok(out)
}

/// An image of 8-bit RGBA texels, row by row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

fn u32_at(data: &[u8], at: usize) -> io::Result<u32> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| invalid("a KTX2 header cut short".to_owned()))
}

fn usize_at(data: &[u8], at: usize) -> io::Result<usize> {
    data.get(at..at + 8)
        .and_then(|b| usize::try_from(u64::from_le_bytes(b.try_into().ok()?)).ok())
        .ok_or_else(|| invalid("a KTX2 level index cut short".to_owned()))
}

/// The width and height of the texture whose file begins with `header` ([`HEADER`] bytes
/// suffice).
pub fn size(header: &[u8]) -> io::Result<(u32, u32)> {
    if !header.starts_with(&IDENTIFIER) {
        return Err(invalid("not a KTX2 file".to_owned()));
    }
    Ok((u32_at(header, 20)?, u32_at(header, 24)?))
}

/// The largest level of an [`Encoding::Rgba8`] texture.
pub fn read(data: &[u8]) -> io::Result<Rgba> {
    let (width, height) = size(data)?;
    let format = u32_at(data, 12)?;
    if format != RGBA8_SRGB {
        return Err(invalid(format!(
            "a texture of format {format}, not 8-bit RGBA"
        )));
    }
    let scheme = u32_at(data, 44)?;
    let (offset, length, unpacked) = (
        usize_at(data, HEADER)?,
        usize_at(data, HEADER + 8)?,
        usize_at(data, HEADER + 16)?,
    );
    let stored = data
        .get(offset..offset.saturating_add(length))
        .ok_or_else(|| invalid("a KTX2 level beyond the file".to_owned()))?;
    let pixels = match scheme {
        0 => stored.to_vec(),
        ZSTANDARD => zstd::bulk::decompress(stored, unpacked)?,
        other => return Err(invalid(format!("supercompression scheme {other}"))),
    };
    let rgba = Rgba {
        width: usize::try_from(width).map_err(|_| invalid(format!("{width} texels")))?,
        height: usize::try_from(height).map_err(|_| invalid(format!("{height} texels")))?,
        pixels,
    };
    if rgba.pixels.len() != 4 * rgba.width * rgba.height {
        return Err(invalid(format!(
            "{} bytes for {width} x {height} texels",
            rgba.pixels.len()
        )));
    }
    Ok(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gradient(width: u32, height: u32) -> Vec<u8> {
        (0..width * height)
            .flat_map(|i| {
                let [a, b, ..] = i.to_le_bytes();
                [a, b, a ^ b, 255 - a]
            })
            .collect()
    }

    #[test]
    fn an_rgba_texture_reads_back_and_as_khronos_reader_reads_it() {
        let levels = vec![
            gradient(8, 4),
            gradient(4, 2),
            gradient(2, 1),
            gradient(1, 1),
        ];
        let file = write(Encoding::Rgba8, 8, 4, &levels).unwrap();
        assert_eq!(size(&file[..HEADER]).unwrap(), (8, 4));
        let image = read(&file).unwrap();
        assert_eq!((image.width, image.height), (8, 4));
        assert_eq!(image.pixels, levels[0]);
        // The `ktx2` crate, Bevy's reader, sees the same.
        let reader = ktx2::Reader::new(&file[..]).unwrap();
        let header = reader.header();
        assert_eq!(header.format, Some(ktx2::Format::R8G8B8A8_SRGB));
        assert_eq!((header.pixel_width, header.pixel_height), (8, 4));
        assert_eq!(header.level_count, 4);
        assert_eq!(
            header.supercompression_scheme,
            Some(ktx2::SupercompressionScheme::Zstandard)
        );
        assert_eq!(reader.color_model(), Some(ktx2::ColorModel::RGBSDA));
        assert_eq!(
            reader.transfer_function(),
            Some(ktx2::TransferFunction::SRGB)
        );
        assert_eq!(reader.is_alpha_premultiplied(), Some(false));
        for (level, expected) in reader.levels().zip(&levels) {
            assert_eq!(
                level.uncompressed_byte_length,
                u64::try_from(expected.len()).unwrap()
            );
            assert_eq!(&zstd::decode_all(level.data).unwrap(), expected);
        }
    }

    #[test]
    fn a_bc7_texture_carries_its_blocks_and_is_not_read_as_rgba() {
        // 6 × 6 texels: 2 × 2 blocks, then 3 × 3 (one block) and 1 × 1.
        let levels = vec![vec![7; 64], vec![8; 16], vec![9; 16]];
        let file = write(Encoding::Bc7, 6, 6, &levels).unwrap();
        let reader = ktx2::Reader::new(&file[..]).unwrap();
        assert_eq!(reader.header().format, Some(ktx2::Format::BC7_SRGB_BLOCK));
        assert_eq!(reader.color_model(), Some(ktx2::ColorModel::BC7));
        let blocks: Vec<Vec<u8>> = reader
            .levels()
            .map(|level| zstd::decode_all(level.data).unwrap())
            .collect();
        assert_eq!(blocks, levels);
        assert!(read(&file).is_err());
    }

    #[test]
    fn a_level_of_the_wrong_size_is_refused() {
        assert!(write(Encoding::Rgba8, 2, 2, &[vec![0; 15]]).is_err());
        assert!(write(Encoding::Bc7, 4, 4, &[vec![0; 16], vec![0; 32]]).is_err());
        assert!(write(Encoding::Rgba8, 1, 1, &[vec![0; 4], vec![0; 4]]).is_err());
        assert!(size(b"not a texture").is_err());
    }
}
